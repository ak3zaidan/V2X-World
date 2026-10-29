//! When the node transmits: BSM at 10 Hz and CAM on the EN 302 637-2 triggers.
//!
//! The rules themselves live in `v2xw-msg` — [`v2xw_msg::generator::CamTriggerState`] is
//! the EN 302 637-2 §6.1.3 state machine and [`v2xw_msg::generator::BsmGenerator`] the
//! J2945/1 10 Hz cadence — and this module is the node-side wiring. It is a thin module
//! with one job, and the job is the one thing a generator can get catastrophically wrong:
//! **what it reads.**
//!
//! # The input is the belief, and only the belief
//!
//! [`MessageSchedule::due`] takes a [`PositionEstimate`] and a believed instant. It cannot
//! take ground truth, because ground truth is not in its signature and this crate has no
//! way to reach it (see [`crate::ctx`] and [`crate::firewall`]).
//!
//! That is not pedantry. A CAM is triggered when the vehicle has moved more than 4 m,
//! turned more than 4 degrees or changed speed by more than 0.5 m/s. Evaluate those
//! thresholds against the truth and the generation rate is a clean function of the
//! vehicle's motion; evaluate them against a GNSS belief with metre-scale noise and the
//! rate rises, because noise crosses a 4 m threshold on its own. The second is what
//! happens on a road. A simulator that produced the first would under-report channel load
//! in exactly the conditions — poor fix, urban canyon — where load matters most, and the
//! error would be invisible because both numbers look reasonable.
//!
//! The same argument applies to the clock: the intervals are measured on the sender's
//! believed time, so a node whose clock has been stepped generates at a visibly wrong
//! rate rather than at a correct one.
//!
//! # Provenance of the trigger block
//!
//! The CAM rule set is EN 302 637-2 V1.4.1 §6.1.3: dynamics triggers at Δposition > 4 m,
//! Δheading > 4°, Δspeed > 0.5 m/s, floored at `T_GenCamMin` (100 ms) and heart-beating
//! at `T_GenCamMax` (1,000 ms). A verified reference implementation of exactly this block
//! exists in the retired legacy engine at `legacy/reference/jvm/ScmsBeaconApp.java`
//! lines 140-154, and 01-inventory.md §3 singles it out as "a correct ETSI EN 302 637-2
//! CAM trigger block (heading > 4°, position > 4 m, speed > 0.5 m/s, 100 ms-1 s) worth
//! citing when the CAM generator is written". It is cited, and two of its details are
//! deliberately *not* reproduced:
//!
//! * the Java block compares the heading difference in degrees against `4.0` while
//!   reading a heading in degrees; this crate works in ENU radians throughout (build
//!   decision D6) and the threshold is `CamGenParams::FOUR_DEGREES_RAD`;
//! * the Java block reads `p.getX()`/`p.getY()` from the simulator's own position, which
//!   is ground truth. That is the legacy defect 01-inventory §3.3 records and the reason
//!   the firewall exists. The rule set is worth citing; the input is not.

use v2xw_core::belief::PositionEstimate;
use v2xw_core::time::{Duration, SimTime};
use v2xw_msg::MsgType;
use v2xw_msg::generator::{
    BsmGenParams, BsmGenerator, CamDynamics, CamGenParams, CamTriggerState, DccState, GenReason,
    GenRequest,
};

/// Which message services a node runs.
///
/// Each flag is a service the node *may* run; whether a message goes out is the service's
/// own trigger. A roadside unit runs the intersection services (SPaT, MAP, SSM), a vehicle
/// the awareness ones (CAM, BSM) and the event ones (DENM, and SRM for a vehicle entitled
/// to signal priority) — the engine's wiring decides which node runs which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ServiceSet {
    /// Generate CAMs on the EN 302 637-2 triggers.
    pub cam: bool,
    /// Generate BSMs at the J2945/1 cadence.
    pub bsm: bool,
    /// Raise DENMs on the triggers this crate implements ([`crate::runtime`]'s event
    /// services): hard braking, as `dangerousSituation`.
    pub denm: bool,
    /// Broadcast SPaT (or SPATEM) at [`SPAT_INTERVAL`], from the payload the unit's
    /// controller feed installed.
    pub spat: bool,
    /// Broadcast MAP (or MAPEM) at [`MAP_INTERVAL`], likewise.
    pub map: bool,
    /// Request signal priority (SRM) when approaching a junction whose MAP it heard.
    pub srm: bool,
    /// Answer a signal request with a status (SSM).
    pub ssm: bool,
    /// Share what the vehicle's sensors perceive (CPM, ETSI TS 103 324), at the
    /// generation rules of [`crate::cpm`].
    pub cpm: bool,
}

impl ServiceSet {
    /// Nothing at all: a unit that only receives.
    pub const NONE: ServiceSet = ServiceSet {
        cam: false,
        bsm: false,
        denm: false,
        spat: false,
        map: false,
        srm: false,
        ssm: false,
        cpm: false,
    };
    /// The ETSI stack: CAM only.
    pub const ETSI: ServiceSet = ServiceSet {
        cam: true,
        ..ServiceSet::NONE
    };
    /// The SAE stack: BSM only.
    pub const SAE: ServiceSet = ServiceSet {
        bsm: true,
        ..ServiceSet::NONE
    };
    /// Both, for a dual-stack unit.
    pub const BOTH: ServiceSet = ServiceSet {
        cam: true,
        bsm: true,
        ..ServiceSet::NONE
    };
}

/// The SPaT broadcast interval: 10 Hz (04-models.md §8.1's default, which cites CTI 4501).
pub const SPAT_INTERVAL: Duration = Duration::from_millis(100);

/// The MAP broadcast interval: 1 Hz (04-models.md §8.1's default, which cites CTI 4501).
pub const MAP_INTERVAL: Duration = Duration::from_secs(1);

/// The node's message-generation timers.
#[derive(Debug, Clone)]
pub struct MessageSchedule {
    services: ServiceSet,
    cam: CamTriggerState,
    bsm: BsmGenerator,
    /// The instant, on the node's clock, the next BSM is aligned to (a sidelink
    /// reservation, WP-34: [`MessageSchedule::align_to`]). `None`: the J2945/1 cadence.
    bsm_aligned: Option<SimTime>,
    /// When the last SPaT and MAP were asked for, on the node's clock.
    last_spat: Option<SimTime>,
    last_map: Option<SimTime>,
    /// The SPaT and MAP periods (network audit: `messages.spat_interval_ms`,
    /// `messages.map_interval_ms`); CTI 4501's 10 Hz and 1 Hz by default.
    spat_interval: Duration,
    map_interval: Duration,
    /// The WSA period, when the unit advertises a service (IEEE 1609.3 `RepeatRate`;
    /// WP-19). `None`: no WSA.
    wsa_interval: Option<Duration>,
    last_wsa: Option<SimTime>,
    generated: u32,
}

impl MessageSchedule {
    /// A schedule running `services` with the standards' own defaults.
    pub fn new(services: ServiceSet) -> Self {
        MessageSchedule {
            services,
            cam: CamTriggerState::new(CamGenParams::en302637_2()),
            bsm: BsmGenerator::new(BsmGenParams::j2945_1()),
            bsm_aligned: None,
            last_spat: None,
            last_map: None,
            spat_interval: SPAT_INTERVAL,
            map_interval: MAP_INTERVAL,
            wsa_interval: None,
            last_wsa: None,
            generated: 0,
        }
    }

    /// The same schedule with a WSA every `interval` (a roadside unit advertising a
    /// service, IEEE 1609.3; WP-19 P19.2). Periodic like the SPaT, and not DCC-gated.
    #[must_use]
    pub fn with_wsa(mut self, interval: Duration) -> Self {
        self.wsa_interval = Some(interval);
        self
    }

    /// The same schedule with other SPaT and MAP periods (network audit). A period
    /// shorter than [`MessageSchedule::check_interval`] is served at the check interval.
    #[must_use]
    pub fn with_infra_intervals(mut self, spat: Duration, map: Duration) -> Self {
        self.spat_interval = spat;
        self.map_interval = map;
        self
    }

    /// The services this schedule runs.
    pub fn services(&self) -> ServiceSet {
        self.services
    }

    /// The same schedule with non-default parameters.
    #[must_use]
    pub fn with_params(mut self, cam: CamGenParams, bsm: BsmGenParams) -> Self {
        self.cam = CamTriggerState::new(cam);
        self.bsm = BsmGenerator::new(bsm);
        self
    }

    /// How often the engine must call [`MessageSchedule::due`].
    ///
    /// `T_CheckCamGen` in EN 302 637-2, which the standard requires to be no greater than
    /// `T_GenCamMin`: a check interval coarser than the minimum period could not honour
    /// the minimum. The BSM cadence is checked at the same interval, and 100 ms is both
    /// standards' floor, so one timer serves both.
    pub fn check_interval(&self) -> Duration {
        self.cam.params().t_gen_cam_min
    }

    /// The CAM state machine, for a detector that wants to ask whether a peer's CAM
    /// would have been triggered.
    pub fn cam(&self) -> &CamTriggerState {
        &self.cam
    }

    /// The `msgCnt` the next BSM will carry.
    pub fn bsm_msg_count(&self) -> u8 {
        self.bsm.msg_count()
    }

    /// Restarts the BSM counter at `value`, for a change of identity.
    pub fn restart_bsm_count(&mut self, value: u8) {
        self.bsm.restart_count(value);
    }

    /// How many messages this schedule has asked for.
    pub fn generated(&self) -> u32 {
        self.generated
    }

    /// Clears the window counter.
    pub fn reset_window(&mut self) {
        self.generated = 0;
    }

    /// What, if anything, to send now.
    ///
    /// `believed_now` is the node's own clock and `belief` its own position estimate.
    /// Neither argument can be ground truth, and that is the module's entire contract.
    pub fn due(
        &mut self,
        believed_now: SimTime,
        belief: &PositionEstimate,
        dcc: &DccState,
    ) -> Vec<GenRequest> {
        let mut out = Vec::new();
        if self.services.cam {
            let dynamics = CamDynamics::from_estimate(belief);
            if let Some(d) = self.cam.check(believed_now, &dynamics, dcc) {
                out.push(GenRequest {
                    msg_type: MsgType::Cam,
                    reason: d.reason,
                    include_low_frequency: d.include_low_frequency,
                    at: believed_now,
                });
            }
        }
        if self.services.bsm
            && let Some(reason) = self.bsm_check(believed_now, dcc)
        {
            out.push(GenRequest {
                msg_type: MsgType::Bsm,
                reason,
                // J2735 Part II is the BSM's low-cadence content; this generator emits
                // Part I only, which 04-models.md §8.1 records as the modelled subset.
                include_low_frequency: false,
                at: believed_now,
            });
        }
        // The intersection broadcasts are periodic and nothing else: a controller's state
        // changes on its own schedule, and CTI 4501's rates do not adapt to it. DCC does not
        // gate them — J2945/1's rate control is the BSM's — which is stated on the card.
        // `last` is the instant the previous message was *due*, not when the check that
        // sent it ran: the next one is due a whole interval after that. Taking the check's
        // instant instead rounded every interval up to the check grid, so a 150 ms SPaT
        // went out every 200 ms (5 Hz, not 6.7) and a 250 ms one every 300 ms. A unit that
        // fell more than an interval behind restarts from now rather than bursting.
        let periodic = |last: &mut Option<SimTime>, interval: Duration, ty: MsgType| {
            let i = interval.as_nanos();
            let due = last.is_none_or(|t| believed_now.saturating_sub(t) >= i);
            if due {
                *last = Some(match *last {
                    Some(t) if believed_now.saturating_sub(t) < 2 * i => t + i,
                    _ => believed_now,
                });
            }
            due.then_some(GenRequest {
                msg_type: ty,
                reason: GenReason::Periodic,
                include_low_frequency: false,
                at: believed_now,
            })
        };
        if self.services.spat
            && let Some(r) = periodic(&mut self.last_spat, self.spat_interval, MsgType::Spat)
        {
            out.push(r);
        }
        if self.services.map
            && let Some(r) = periodic(&mut self.last_map, self.map_interval, MsgType::Map)
        {
            out.push(r);
        }
        if let Some(i) = self.wsa_interval
            && let Some(r) = periodic(&mut self.last_wsa, i, MsgType::Wsa)
        {
            out.push(r);
        }
        self.generated = self.generated.saturating_add(out.len() as u32);
        out
    }
}

/// How early an aligned BSM may go out: the check at `due − ALIGN_TOLERANCE` sends it.
///
/// The engine computes the aligned instant on the node's clock and wakes the node on the
/// simulation's timeline; between the two the clock can move by its drift (a 100 ppm
/// crystal moves 10 µs in 100 ms), and a check a few nanoseconds short of the instant
/// would otherwise hold the BSM a whole step. Half a millisecond is far above any drift
/// and far below `vMinITT`, so it can neither miss the instant nor send two BSMs.
pub const ALIGN_TOLERANCE: Duration = Duration::from_micros(500);

/// BSM generation aligned to a sidelink reservation (WP-34).
///
/// On LTE-V2X the BSM is carried on a semi-persistent reservation (TS 36.213 V14.4.0
/// §14.1.1.6), and SAE J3161/1 has the application generate it just before the reserved
/// subframe, so a BSM does not wait up to a whole period for its resource [J3161/1,
/// second-hand via Abrar et al. 2026, arXiv 2608.05087; Molina-Masegosa & Gozalvez 2017
/// on the mismatch]. The engine says where the reservation is; this moves the next
/// periodic BSM there.
impl MessageSchedule {
    /// Aligns the next periodic BSM to a reservation: `at` is the instant, on the node's
    /// clock, a BSM generated then reaches the MAC just before its reserved resource, and
    /// `period` the reservation period. Returns the instant the next BSM is due, or `None`
    /// when the node runs no BSM service.
    ///
    /// The next BSM is due at the first `at + k·period` (any integer `k`) at or after the
    /// previous BSM's due instant plus the current interval ([`BsmGenerator::interval`],
    /// `vMinITT` at least), so a realignment never shortens an interval below J2945/1's
    /// floor or the DCC's stretch; and never later than the previous due instant plus
    /// `vMaxITT`, where it is clamped and no longer aligned. Before the first BSM `at`
    /// is taken as it is.
    pub fn align_to(&mut self, at: SimTime, period: Duration, dcc: &DccState) -> Option<SimTime> {
        if !self.services.bsm {
            return None;
        }
        let due = match self.bsm.due_anchor() {
            None => at,
            Some(anchor) => {
                let (_, max_itt) = self.bsm.itt_bounds();
                let lo = self.bsm.interval(dcc).after(anchor);
                let hi = max_itt.after(anchor);
                let p = period.as_nanos();
                let first = if p == 0 {
                    at.max(lo)
                } else if at >= lo {
                    lo + (at - lo) % p
                } else {
                    let d = (lo - at) % p;
                    if d == 0 { lo } else { lo + (p - d) }
                };
                first.min(hi)
            }
        };
        self.bsm_aligned = Some(due);
        Some(due)
    }

    /// The instant the next BSM is aligned to, if any.
    pub fn aligned_bsm_due(&self) -> Option<SimTime> {
        self.bsm_aligned
    }

    /// Drops the alignment: the next BSM follows the J2945/1 cadence again.
    pub fn clear_alignment(&mut self) {
        self.bsm_aligned = None;
    }

    /// The BSM decision: at the aligned instant when there is one, else the J2945/1
    /// cadence ([`BsmGenerator::check`]).
    fn bsm_check(&mut self, believed_now: SimTime, dcc: &DccState) -> Option<GenReason> {
        match self.bsm_aligned {
            None => self.bsm.check(believed_now, dcc),
            Some(due) if believed_now.saturating_add(ALIGN_TOLERANCE.as_nanos()) >= due => {
                self.bsm_aligned = None;
                Some(self.bsm.fire_at(believed_now, due))
            }
            Some(_) => None,
        }
    }
}

/// Whether a request was triggered by the vehicle's dynamics rather than by a timer.
pub fn is_dynamics_triggered(r: &GenRequest) -> bool {
    matches!(r.reason, GenReason::Dynamics(t) if t.any())
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_core::geom::Vec3;
    use v2xw_core::time::{NS_PER_MS, NS_PER_S};

    fn belief_at(pos: Vec3, heading: f64, speed: f64) -> PositionEstimate {
        let mut p = PositionEstimate::no_fix(0);
        p.pos = pos;
        p.heading_rad = heading;
        p.vel = Vec3::new(
            speed * v2xw_core::math::cos(heading),
            speed * v2xw_core::math::sin(heading),
            0.0,
        );
        p.fix = v2xw_core::belief::FixQuality::ThreeD;
        p
    }

    /// A stationary node on the SAE stack transmits at 10 Hz and no faster, which is the
    /// J2945/1 cadence.
    #[test]
    fn bsm_runs_at_ten_hertz() {
        let mut s = MessageSchedule::new(ServiceSet::SAE);
        let b = belief_at(Vec3::ZERO, 0.0, 0.0);
        let dcc = DccState::UNRESTRICTED;
        let mut sent = 0;
        // Check every 10 ms for one second: 100 checks, 10 transmissions plus the first.
        for k in 0..=100u64 {
            sent += s.due(k * 10 * NS_PER_MS, &b, &dcc).len();
        }
        assert_eq!(sent, 11, "the first, then one per 100 ms");
        assert_eq!(s.generated(), 11);
    }

    /// A stationary node on the ETSI stack heart-beats at `T_GenCamMax`, one per second,
    /// because no dynamics trigger fires.
    #[test]
    fn a_stationary_node_heart_beats_at_one_hertz() {
        let mut s = MessageSchedule::new(ServiceSet::ETSI);
        let b = belief_at(Vec3::ZERO, 0.0, 0.0);
        let dcc = DccState::UNRESTRICTED;
        let mut sent = 0;
        for k in 0..=50u64 {
            sent += s.due(k * 100 * NS_PER_MS, &b, &dcc).len();
        }
        assert_eq!(sent, 6, "the first, then one per second for five seconds");
    }

    /// The position trigger: 4 m of movement inside one heart-beat period fires trigger 1
    /// rather than waiting for `T_GenCamMax`. EN 302 637-2 §6.1.3, and the block at
    /// `legacy/reference/jvm/ScmsBeaconApp.java:140-154`.
    #[test]
    fn four_metres_of_movement_triggers_a_cam() {
        let mut s = MessageSchedule::new(ServiceSet::ETSI);
        let dcc = DccState::UNRESTRICTED;
        assert_eq!(s.due(0, &belief_at(Vec3::ZERO, 0.0, 0.0), &dcc).len(), 1);

        // 3.9 m at t = 200 ms: past T_GenCamMin, under the threshold, no CAM.
        let near = belief_at(Vec3::new(3.9, 0.0, 0.0), 0.0, 0.0);
        assert!(s.due(200 * NS_PER_MS, &near, &dcc).is_empty());

        // 4.1 m at t = 300 ms: trigger 1.
        let far = belief_at(Vec3::new(4.1, 0.0, 0.0), 0.0, 0.0);
        let out = s.due(300 * NS_PER_MS, &far, &dcc);
        assert_eq!(out.len(), 1);
        assert!(is_dynamics_triggered(&out[0]));
        assert!(matches!(
            out[0].reason,
            GenReason::Dynamics(t) if t.position && !t.heading && !t.speed
        ));
    }

    /// The heading and speed triggers, each on its own, with the other two quantities
    /// held still so the attribution is unambiguous.
    #[test]
    fn heading_and_speed_each_trigger_on_their_own() {
        let dcc = DccState::UNRESTRICTED;

        let mut s = MessageSchedule::new(ServiceSet::ETSI);
        s.due(0, &belief_at(Vec3::ZERO, 0.0, 10.0), &dcc);
        let five_degrees = 5.0 * core::f64::consts::PI / 180.0;
        let out = s.due(
            200 * NS_PER_MS,
            &belief_at(Vec3::ZERO, five_degrees, 10.0),
            &dcc,
        );
        assert!(matches!(
            out[0].reason,
            GenReason::Dynamics(t) if t.heading && !t.position
        ));

        let mut s = MessageSchedule::new(ServiceSet::ETSI);
        s.due(0, &belief_at(Vec3::ZERO, 0.0, 10.0), &dcc);
        let out = s.due(200 * NS_PER_MS, &belief_at(Vec3::ZERO, 0.0, 10.6), &dcc);
        assert!(matches!(
            out[0].reason,
            GenReason::Dynamics(t) if t.speed && !t.position && !t.heading
        ));
    }

    /// `T_GenCamMin` is a floor even when the vehicle is manoeuvring hard: two 10 m jumps
    /// 50 ms apart produce one CAM, not two.
    #[test]
    fn t_gen_cam_min_floors_the_rate() {
        let mut s = MessageSchedule::new(ServiceSet::ETSI);
        let dcc = DccState::UNRESTRICTED;
        s.due(0, &belief_at(Vec3::ZERO, 0.0, 0.0), &dcc);
        assert!(
            s.due(
                50 * NS_PER_MS,
                &belief_at(Vec3::new(10.0, 0.0, 0.0), 0.0, 0.0),
                &dcc
            )
            .is_empty()
        );
        assert_eq!(
            s.due(
                100 * NS_PER_MS,
                &belief_at(Vec3::new(20.0, 0.0, 0.0), 0.0, 0.0),
                &dcc
            )
            .len(),
            1
        );
    }

    /// DCC raises the floor: with `t_off` at 500 ms a manoeuvring node cannot transmit at
    /// 10 Hz however much it moves.
    #[test]
    fn dcc_raises_the_floor_above_the_standards_minimum() {
        let mut s = MessageSchedule::new(ServiceSet::BOTH);
        let dcc = DccState {
            t_off: Duration::from_millis(500),
            cbr: Some(0.7),
        };
        s.due(0, &belief_at(Vec3::ZERO, 0.0, 0.0), &dcc);
        let mut sent = 0;
        for k in 1..=10u64 {
            let moved = belief_at(Vec3::new(k as f64 * 10.0, 0.0, 0.0), 0.0, 0.0);
            sent += s.due(k * 100 * NS_PER_MS, &moved, &dcc).len();
        }
        // One second of checks, two services, one transmission each per 500 ms.
        assert_eq!(sent, 4);
    }

    /// The generation clock is the node's, so a node whose clock has been stepped forward
    /// generates a message it would not otherwise have been due for. A simulator reading
    /// the true clock would show nothing at all here.
    #[test]
    fn a_stepped_clock_changes_the_generation_pattern() {
        let dcc = DccState::UNRESTRICTED;
        let b = belief_at(Vec3::ZERO, 0.0, 0.0);

        let mut honest = MessageSchedule::new(ServiceSet::SAE);
        honest.due(0, &b, &dcc);
        assert!(honest.due(50 * NS_PER_MS, &b, &dcc).is_empty());

        let mut stepped = MessageSchedule::new(ServiceSet::SAE);
        stepped.due(0, &b, &dcc);
        // The same true instant, but this node believes a second has passed.
        assert_eq!(stepped.due(50 * NS_PER_MS + NS_PER_S, &b, &dcc).len(), 1);
    }

    /// Counts the SPaT and MAP a roadside schedule asks for over `seconds`, checked every
    /// `check_ms` on a clock that runs `rate` times true time.
    fn infra_counts(
        spat_ms: u64,
        map_ms: u64,
        check_ms: u64,
        seconds: u64,
        rate: f64,
    ) -> (usize, usize) {
        let unit = ServiceSet {
            spat: true,
            map: true,
            ..ServiceSet::NONE
        };
        let mut s = MessageSchedule::new(unit).with_infra_intervals(
            Duration::from_millis(spat_ms),
            Duration::from_millis(map_ms),
        );
        let b = belief_at(Vec3::ZERO, 0.0, 0.0);
        let dcc = DccState::UNRESTRICTED;
        let (mut spat, mut map) = (0, 0);
        for k in 0..(seconds * 1000 / check_ms) {
            let t = ((k * check_ms * NS_PER_MS) as f64 * rate) as SimTime;
            for r in s.due(t, &b, &dcc) {
                match r.msg_type {
                    MsgType::Spat => spat += 1,
                    MsgType::Map => map += 1,
                    _ => {}
                }
            }
        }
        (spat, map)
    }

    /// CTI 4501's 10 Hz SPaT and 1 Hz MAP on the 100 ms check grid: one each per period.
    #[test]
    fn a_roadside_unit_sends_spat_at_ten_hertz_and_map_at_one() {
        assert_eq!(infra_counts(100, 1000, 100, 10, 1.0), (100, 10));
    }

    /// A period that is not a whole number of checks keeps its rate on average: a 150 ms
    /// SPaT is 6.7 Hz, not the 5 Hz that rounding each interval up to the grid gave, and a
    /// 250 ms one 4 Hz, not 3.3.
    #[test]
    fn a_spat_period_off_the_check_grid_keeps_its_rate() {
        assert_eq!(infra_counts(150, 1000, 100, 30, 1.0).0, 200);
        assert_eq!(infra_counts(250, 1000, 100, 30, 1.0).0, 120);
    }

    /// A unit whose clock runs slow by 50 ppm still sends ten SPaT a second: its believed
    /// gap between two checks is a hair under 100 ms, which used to skip every other check.
    #[test]
    fn a_slow_clock_does_not_halve_the_spat_rate() {
        let (spat, map) = infra_counts(100, 1000, 100, 10, 1.0 - 50e-6);
        assert!((99..=100).contains(&spat), "{spat} SPaT in 10 s");
        assert!((9..=10).contains(&map), "{map} MAP in 10 s");
    }

    /// The instants (ms) a schedule checked every millisecond sends a BSM at, from
    /// `from_ms` to `to_ms`.
    fn bsm_instants(s: &mut MessageSchedule, from_ms: u64, to_ms: u64) -> Vec<u64> {
        let b = belief_at(Vec3::ZERO, 0.0, 0.0);
        let dcc = DccState::UNRESTRICTED;
        (from_ms..=to_ms)
            .filter(|ms| !s.due(ms * NS_PER_MS, &b, &dcc).is_empty())
            .collect()
    }

    /// WP-34: aligned to a reservation, the next BSM goes out at the reserved instant and
    /// the cadence continues from there, not from the old 100 ms grid.
    #[test]
    fn align_to_moves_the_next_bsm_to_the_reserved_instant() {
        let mut s = MessageSchedule::new(ServiceSet::SAE);
        assert_eq!(bsm_instants(&mut s, 0, 150), vec![0, 100]);
        let dcc = DccState::UNRESTRICTED;
        // A reservation whose BSM must be generated at 237 ms (period 100 ms): the first
        // instant of that phase at least 100 ms after the last BSM (100 ms) is 237 ms.
        let due = s.align_to(37 * NS_PER_MS, Duration::from_millis(100), &dcc);
        assert_eq!(due, Some(237 * NS_PER_MS));
        assert_eq!(s.aligned_bsm_due(), Some(237 * NS_PER_MS));
        assert_eq!(bsm_instants(&mut s, 151, 400), vec![237, 337]);
        assert_eq!(
            s.aligned_bsm_due(),
            None,
            "an aligned BSM uses its alignment up"
        );
    }

    /// WP-34: a realignment changes one interval, and keeps it inside J2945/1's bounds:
    /// never shorter than `vMinITT` (100 ms), never longer than `vMaxITT` (600 ms).
    #[test]
    fn the_realigned_interval_stays_within_the_j2945_bounds() {
        let dcc = DccState::UNRESTRICTED;
        let p = Duration::from_millis(100);
        // A reservation 30 ms after the last BSM: 30 ms is below vMinITT, so the BSM waits
        // for the same phase one period later, 130 ms.
        let mut s = MessageSchedule::new(ServiceSet::SAE);
        assert_eq!(bsm_instants(&mut s, 0, 0), vec![0]);
        assert_eq!(s.align_to(30 * NS_PER_MS, p, &dcc), Some(130 * NS_PER_MS));
        // A reservation period longer than vMaxITT: the first instant of its phase at least
        // 100 ms out is 900 ms, past vMaxITT, so it is clamped to 600 ms.
        let mut s = MessageSchedule::new(ServiceSet::SAE);
        assert_eq!(bsm_instants(&mut s, 0, 0), vec![0]);
        let long = Duration::from_millis(1000);
        assert_eq!(
            s.align_to(900 * NS_PER_MS, long, &dcc),
            Some(600 * NS_PER_MS)
        );
        // A DCC stretch of 150 ms is honoured: 70 ms + k·100 ms at or after 150 ms is 170 ms.
        let mut s = MessageSchedule::new(ServiceSet::SAE);
        assert_eq!(bsm_instants(&mut s, 0, 0), vec![0]);
        let stretched = DccState {
            t_off: Duration::from_millis(150),
            ..DccState::UNRESTRICTED
        };
        assert_eq!(
            s.align_to(70 * NS_PER_MS, p, &stretched),
            Some(170 * NS_PER_MS)
        );
        // No BSM service, no alignment.
        let mut cam = MessageSchedule::new(ServiceSet::ETSI);
        assert_eq!(cam.align_to(50 * NS_PER_MS, p, &dcc), None);
    }

    /// WP-34: a check a drift's width short of the aligned instant still sends it, and the
    /// tolerance cannot send two BSMs inside `vMinITT`.
    #[test]
    fn an_aligned_bsm_tolerates_clock_drift_and_is_sent_once() {
        let mut s = MessageSchedule::new(ServiceSet::SAE);
        let b = belief_at(Vec3::ZERO, 0.0, 0.0);
        let dcc = DccState::UNRESTRICTED;
        assert_eq!(s.due(0, &b, &dcc).len(), 1);
        let due = s
            .align_to(150 * NS_PER_MS, Duration::from_millis(100), &dcc)
            .expect("aligned");
        assert!(s.due(due - 200 * NS_PER_MS / 1000, &b, &dcc).len() == 1);
        assert!(s.due(due, &b, &dcc).is_empty(), "sent once");
        // With the alignment cleared, the cadence is the J2945/1 one from the last due.
        let mut s = MessageSchedule::new(ServiceSet::SAE);
        assert_eq!(s.due(0, &b, &dcc).len(), 1);
        s.align_to(150 * NS_PER_MS, Duration::from_millis(100), &dcc);
        s.clear_alignment();
        assert_eq!(bsm_instants(&mut s, 1, 250), vec![100, 200]);
    }
}
