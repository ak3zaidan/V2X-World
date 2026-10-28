//! Three more C2C-CC DENM triggering conditions (WP-12): **traffic condition — sudden
//! speed drop** (the former "dangerous end of queue"), **traffic condition — local slow
//! down** (the former "traffic jam ahead"), and **adverse weather condition — traction
//! loss**. Each is off unless the scenario switches it on
//! (`messages.denm_traffic_condition`, `messages.denm_traction_loss`).
//!
//! Re-implemented from the CAR 2 CAR Communication Consortium's *Triggering Conditions and
//! Data Quality* documents, read on 2026-10-27 from car-2-car.org and cached as
//! `research/papers/c2ccc2026rs2007trafficcondition.pdf` (C2CCC_RS_2007_TrafficCondition
//! R1.6.10, 2026-07-24) and `research/papers/c2ccc2026rs2002adverseweather.pdf`
//! (C2CCC_RS_2002_AdverseWeather R1.6.10, 2026-07-24). Artery's `DenService` use cases
//! were read for behaviour only and no code was copied.
//!
//! Every input is the vehicle's own: its wheel speed, accelerometer and hazard switch, its
//! navigation map's road class, its ABS, its own sensors' object list, and the CAMs and
//! DENMs it received and verified. Nothing here reads another road user's truth.
//!
//! # Sudden speed drop (RS 2007 §3.1): `dangerousEndOfQueue(27)`
//!
//! | Rule | Value | Requirement |
//! |---|---|---|
//! | Precondition: non-urban — the speed was above 80 km/h for a block of at least 30 s in the 60 s before, or the digital map says non-urban | 80 km/h, 30 s, 60 s | RS_tcTrJa_94 1.1, 1.3 |
//! | TRCO_0: from an initial speed above 80 km/h at a deceleration of at most 0.1 m/s², down to 60 km/h or less, a drop of at least 50 km/h, within 10 s, with an instant deceleration above 3.5 m/s² | | RS_tcTrJa_105 Table 3 |
//! | TRCO_1: own hazard lights for at least 3 s; TRCO_3/TRCO_4: a relevant sudden-speed-drop / local-slow-down DENM received; TRCO_6: own sensors see the lane blocked by traffic slower than 50 km/h | 3 s, 50 km/h | Table 3 |
//! | Trigger: TRCO_0 AND (TRCO_1 OR 3 OR 4 OR 6), or TRCO_1 AND (3 OR 4 OR 6) | | RS_tcTrJa_105 |
//! | A condition stays valid 10 s after it ends; a new DENM no sooner than 60 s after the last | 10 s, 60 s | RS_tcTrJa_107, _151 |
//! | Relevant messages: within 1,000 m, in the ±45° sector ahead | 1,000 m | RS_tcTrJa_108 c |
//! | Information quality: driver reaction AND environment 1, AND sensors 2, all three 3 | | RS_tcTrJa_109 Table 4 |
//! | No update, cancellation or negation; repeated every 0.5 s for 20 s; validity 20 s; lessThan1000m; upstream traffic | | RS_tcTrJa_110-116 |
//!
//! # Local slow down (RS 2007 §3.2): `trafficCondition(1)`
//!
//! | Rule | Value | Requirement |
//! |---|---|---|
//! | Preconditions: no stationary-vehicle event of its own; non-urban — above 80 km/h for a 30 s block in the 180 s before, or the map says non-urban | 80 km/h, 30 s, 180 s | RS_tcTrJa_122 |
//! | TRCO_0: the average speed over T1, stationary periods left out, is 30 km/h or less; a stationary period longer than T2 restarts the average | T1 120 s, T2 30 s | RS_tcTrJa_131 Table 7 |
//! | TRCO_1: stationary for at least T2; TRCO_2: a relevant local-slow-down DENM received; TRCO_4: relevant CAMs from at least five other vehicles at 30 km/h or less; TRCO_5: own sensors see at least five vehicles at 30 km/h or less within 100 m, same direction; TRCO_6: the map says non-urban throughout T1 (or T2) | 5 vehicles, 100 m | Table 7 |
//! | Trigger: TRCO_0, or TRCO_1 AND (TRCO_2 OR 3 OR 4 OR 5) | | RS_tcTrJa_131 |
//! | A condition stays valid 5 s after it ends; a new DENM no sooner than 180 s after the last | 5 s, 180 s | RS_tcTrJa_133, _156 |
//! | Relevant messages: within 500 m, heading within 10°, DENM positions in the ±45° sector ahead | 500 m, 10°, 45° | RS_tcTrJa_134 c |
//! | Information quality: dynamics 1, + environment 2, + sensors 3, + both 4, + map 5 | | RS_tcTrJa_135 Table 8 |
//! | No update, cancellation or negation; repeated every 1 s for 60 s; validity 60 s; lessThan1000m; upstream traffic | | RS_tcTrJa_136-142 |
//!
//! # Traction loss (RS 2002 §3.3): `adhesion(6)`
//!
//! | Rule | Value | Requirement |
//! |---|---|---|
//! | Trigger: the friction coefficient is below 0.3 (condition i) or below 0.2 (condition j) for at least 5 s | 0.3, 0.2, 5 s | RS_tcAdWe_150 i, j |
//! | Information quality: 6 for i, 7 for j | | RS_tcAdWe_164 Table 7 |
//! | Update evaluated every 0.1 s: while the condition holds an update at least 0.1 s after the last; one last update when it ends, then none | 0.1 s | RS_tcAdWe_169 |
//! | No cancellation or negation; repeated every 1 s for 300 s (urban: every 4 s for 180 s); validity 600 s (urban 300 s); lessThan1000m; all traffic directions | | RS_tcAdWe_166-177 |
//!
//! The friction coefficient is the vehicle's own estimate. An ABS that has regulated for
//! at least 200 ms (the condition-e duration) holds the wheels at the adhesion limit, so
//! the deceleration it achieves there, `|a|/g`, *is* the coefficient; the estimate is
//! kept for [`FRICTION_ESTIMATE_HOLD`] after its last refresh and dropped at once when the
//! vehicle brakes harder than `0.3·g` without ABS, which proves the grip is higher.
//!
//! # Choices and gaps, stated
//!
//! * The steering-wheel-angle half of the non-urban precondition (below 90° for 30 s) is
//!   implied by the speed half: at more than 80 km/h a 90° steering-wheel angle (a road
//!   wheel angle of about 6° at a 15:1 ratio, a 25 m radius on a 2.7 m wheelbase) would
//!   need some 20 m/s² of lateral acceleration, which no car achieves.
//! * "Stationary" (RS_BSP_511, not cached) is a wheel speed below [`STATIONARY_MPS`],
//!   the stationary-vehicle service's own threshold.
//! * A received DENM counts while it is being repeated: within [`DENM_HEARD_FRESH`] of
//!   its last reception. A CAM counts within [`CAM_FRESH`], one CAM interval and a step.
//!   A pseudonym change can count one vehicle twice inside that window (RS_tcTrJa_108's
//!   note asks to avoid it; not modelled).
//! * Not modelled: TRCO_2 of the speed drop (other vehicles' hazard lights in their CAMs:
//!   the CAM's exterior lights are not decoded here), TRCO_5 (special vehicles), TRCO_3 of
//!   the slow down (mobile radio), the traction-loss conditions a-h (the car-following
//!   models do not expose the throttle or brake demand that the µ-high comparison needs),
//!   the AT-changeover blocking of RS_tcTrJa_120/146, `eventHistory` (RS_BSP_544), and
//!   the DENM's `roadType` and `lanePosition`.

use std::collections::{BTreeMap, VecDeque};

use v2xw_core::geom::Vec3;
use v2xw_core::time::{Duration, SimTime};

/// km/h to m/s.
const KMH: f64 = 1.0 / 3.6;

/// The optional DENM triggers a node runs (WP-12). Both off by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DenmTriggers {
    /// Sudden speed drop and local slow down (`messages.denm_traffic_condition`).
    pub traffic_condition: bool,
    /// Traction loss (`messages.denm_traction_loss`).
    pub traction_loss: bool,
}

/// The non-urban speed, 80 km/h (RS_tcTrJa_94 1.1, _122 3.1).
pub const NON_URBAN_SPEED_MPS: f64 = 80.0 * KMH;
/// ... held for a block this long (RS_tcTrJa_94 1.1, _122 3.1).
pub const NON_URBAN_BLOCK: Duration = Duration::from_secs(30);
/// ... within this much before a sudden speed drop (RS_tcTrJa_94 1.1).
pub const SPEED_DROP_LOOKBACK: Duration = Duration::from_secs(60);
/// ... within this much before a local slow down (RS_tcTrJa_122 3.1).
pub const SLOW_DOWN_LOOKBACK: Duration = Duration::from_secs(180);

/// Sudden speed drop TRCO_0: the initial speed exceeds 80 km/h ...
pub const DROP_INITIAL_MPS: f64 = 80.0 * KMH;
/// ... at an initial deceleration of at most 0.1 m/s² ...
pub const DROP_INITIAL_DECEL_MPS2: f64 = 0.1;
/// ... down to a target of 60 km/h or less ...
pub const DROP_TARGET_MPS: f64 = 60.0 * KMH;
/// ... a drop of at least 50 km/h ...
pub const DROP_MIN_MPS: f64 = 50.0 * KMH;
/// ... within 10 s ...
pub const DROP_WITHIN: Duration = Duration::from_secs(10);
/// ... with an instant deceleration above 3.5 m/s² (RS_tcTrJa_105 Table 3).
pub const DROP_INSTANT_DECEL_MPS2: f64 = 3.5;
/// TRCO_1: hazard lights on for at least 3 s.
pub const HAZARD_MIN: Duration = Duration::from_secs(3);
/// TRCO_6: the lane is blocked by traffic slower than 50 km/h.
pub const BLOCKED_LANE_MPS: f64 = 50.0 * KMH;
/// How far ahead the lane-blocked test looks: the long-range radar's 250 m.
pub const BLOCKED_LANE_AHEAD_M: f64 = 250.0;
/// Half a lane: an object this close to the own path's centre line is in the own lane.
pub const HALF_LANE_M: f64 = 1.8;
/// A sudden-speed-drop condition stays valid this long after it ends (RS_tcTrJa_107).
pub const DROP_CONDITION_HOLD: Duration = Duration::from_secs(10);
/// Detection blocking time of the sudden speed drop (RS_tcTrJa_151).
pub const DROP_BLOCKING: Duration = Duration::from_secs(60);
/// Relevance distance of received messages for the sudden speed drop (RS_tcTrJa_108).
pub const DROP_RELEVANCE_M: f64 = 1_000.0;
/// Sudden speed drop DENM: repetition interval (RS_tcTrJa_114) ...
pub const DROP_REPETITION: Duration = Duration::from_millis(500);
/// ... repetition duration (RS_tcTrJa_114) ...
pub const DROP_REPETITION_DURATION: Duration = Duration::from_secs(20);
/// ... and validity (RS_tcTrJa_116).
pub const DROP_VALIDITY: Duration = Duration::from_secs(20);

/// Local slow down T1, the averaging period (RS_tcTrJa_131).
pub const SLOW_T1: Duration = Duration::from_secs(120);
/// Local slow down T2, the stationary period (RS_tcTrJa_131).
pub const SLOW_T2: Duration = Duration::from_secs(30);
/// Local slow down: 30 km/h or less (TRCO_0, TRCO_4, TRCO_5).
pub const SLOW_MPS: f64 = 30.0 * KMH;
/// TRCO_4, TRCO_5: at least five other vehicles.
pub const SLOW_VEHICLES: usize = 5;
/// TRCO_5: within 100 m.
pub const SLOW_SENSOR_RANGE_M: f64 = 100.0;
/// Relevance distance of received messages for the local slow down (RS_tcTrJa_134).
pub const SLOW_RELEVANCE_M: f64 = 500.0;
/// Relevance: the heading difference is less than 10° (RS_tcTrJa_108 c, _134 c).
pub const RELEVANCE_HEADING_RAD: f64 = 10.0 * core::f64::consts::PI / 180.0;
/// Relevance: DENM positions within ±45° of the longitudinal axis (RS_tcTrJa_108 c,
/// _134 c).
pub const RELEVANCE_SECTOR_RAD: f64 = 45.0 * core::f64::consts::PI / 180.0;
/// A local-slow-down condition stays valid this long after it ends (RS_tcTrJa_133).
pub const SLOW_CONDITION_HOLD: Duration = Duration::from_secs(5);
/// Detection blocking time of the local slow down (RS_tcTrJa_156).
pub const SLOW_BLOCKING: Duration = Duration::from_secs(180);
/// Local slow down DENM: repetition interval (RS_tcTrJa_140) ...
pub const SLOW_REPETITION: Duration = Duration::from_secs(1);
/// ... repetition duration (RS_tcTrJa_140) ...
pub const SLOW_REPETITION_DURATION: Duration = Duration::from_secs(60);
/// ... and validity (RS_tcTrJa_142).
pub const SLOW_VALIDITY: Duration = Duration::from_secs(60);

/// A wheel speed below this is stationary (the stationary-vehicle service's threshold).
pub const STATIONARY_MPS: f64 = 0.1;
/// A received DENM counts this long after its last reception: two of the slower
/// service's 1 s repetitions. This build's choice.
pub const DENM_HEARD_FRESH: Duration = Duration::from_secs(2);
/// A received CAM counts this long: `T_GenCamMax` (1 s) and a step. This build's choice.
pub const CAM_FRESH: Duration = Duration::from_millis(1_100);

/// Traction loss condition i: the friction coefficient is below 0.3 ...
pub const MU_CONDITION_I: f64 = 0.3;
/// ... condition j: below 0.2 ...
pub const MU_CONDITION_J: f64 = 0.2;
/// ... for at least 5 s (RS_tcAdWe_150 i, j).
pub const MU_FOR: Duration = Duration::from_secs(5);
/// ABS regulating at least this long holds the wheels at the adhesion limit (the
/// condition-e duration, RS_tcAdWe_150 e).
pub const ABS_MIN: Duration = Duration::from_millis(200);
/// How long a friction estimate stays usable without a new ABS intervention. This
/// build's choice: an estimate needs excitation and goes stale without it.
pub const FRICTION_ESTIMATE_HOLD: Duration = Duration::from_secs(30);
/// Traction loss: the minimum update interval (RS_tcAdWe_169 b).
pub const TRACTION_UPDATE: Duration = Duration::from_millis(100);
/// Traction loss DENM: repetition every 1 s for 300 s (RS_tcAdWe_174) ...
pub const TRACTION_REPETITION: Duration = Duration::from_secs(1);
/// ... for this long ...
pub const TRACTION_REPETITION_DURATION: Duration = Duration::from_secs(300);
/// ... and in an urban area every 4 s for 180 s (RS_tcAdWe_175).
pub const TRACTION_REPETITION_URBAN: Duration = Duration::from_secs(4);
/// See [`TRACTION_REPETITION_URBAN`].
pub const TRACTION_REPETITION_DURATION_URBAN: Duration = Duration::from_secs(180);
/// Traction loss DENM validity, 600 s (RS_tcAdWe_177) ...
pub const TRACTION_VALIDITY: Duration = Duration::from_secs(600);
/// ... and 300 s in an urban area.
pub const TRACTION_VALIDITY_URBAN: Duration = Duration::from_secs(300);
/// Braking harder than this share of `g` without ABS proves the grip is higher than
/// condition i's 0.3 and drops a low estimate.
pub const MU_CONTRADICTED: f64 = 0.3;

/// Standard gravity, m/s².
const G_MPS2: f64 = 9.806_65;

/// One reading of the own vehicle.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Sample {
    t: SimTime,
    v: f64,
    a: f64,
    non_urban: Option<bool>,
}

/// What a received message told this vehicle, for the relevance tests.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Heard {
    pos: Vec3,
    speed: f64,
    heading: f64,
    at: SimTime,
}

/// A trigger's decision for this step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TractionAction {
    /// Raise a new event with this information quality.
    New(u8),
    /// Update the live event, with this information quality.
    Update(u8),
}

/// The traffic-condition and traction-loss detectors' state.
#[derive(Debug, Clone, Default)]
pub struct TriggerState {
    history: VecDeque<Sample>,
    hazard_since: Option<SimTime>,
    cams: BTreeMap<[u8; 8], Heard>,
    /// Received DENMs: (signer, cause code) → the event position and when heard.
    denms: BTreeMap<([u8; 8], u8), Heard>,
    /// The own sensors' vehicles: position, velocity, heading.
    perceived: Vec<(Vec3, Vec3, f64)>,
    /// When each condition was last true, for the validity extension.
    drop_last: [Option<SimTime>; 5],
    slow_last: [Option<SimTime>; 6],
    drop_blocked_until: Option<SimTime>,
    slow_blocked_until: Option<SimTime>,
    abs_since: Option<SimTime>,
    mu_estimate: Option<(f64, SimTime)>,
    below_i_since: Option<SimTime>,
    below_j_since: Option<SimTime>,
    traction_last_update: Option<SimTime>,
    traction_finished: bool,
}

/// `received DENM cause code`: sudden speed drop.
pub const CAUSE_DANGEROUS_END_OF_QUEUE: u8 = 27;
/// `received DENM cause code`: traffic condition.
pub const CAUSE_TRAFFIC_CONDITION: u8 = 1;

fn wrap_pi(a: f64) -> f64 {
    v2xw_msg::j2945::wrap_pi(a)
}

fn within(now: SimTime, at: SimTime, d: Duration) -> bool {
    now.saturating_sub(at) <= d.as_nanos()
}

impl TriggerState {
    /// One reading of the own vehicle's bus.
    pub fn on_bus(&mut self, bus: &crate::vehicle::VehicleBus, keep_history: bool) {
        let t = bus.t;
        if keep_history {
            if self.history.back().is_none_or(|s| t > s.t) {
                self.history.push_back(Sample {
                    t,
                    v: bus.speed_mps,
                    a: bus.a_long_mps2,
                    non_urban: bus.non_urban,
                });
            }
            while self
                .history
                .front()
                .is_some_and(|s| !within(t, s.t, SLOW_DOWN_LOOKBACK))
            {
                self.history.pop_front();
            }
        }
        if bus.hazard_lights {
            self.hazard_since.get_or_insert(t);
        } else {
            self.hazard_since = None;
        }
        // The friction estimate (module docs).
        if bus.abs_active {
            let since = *self.abs_since.get_or_insert(t);
            if t.saturating_sub(since) >= ABS_MIN.as_nanos() {
                self.mu_estimate = Some((-bus.a_long_mps2 / G_MPS2, t));
            }
        } else {
            self.abs_since = None;
            if -bus.a_long_mps2 >= MU_CONTRADICTED * G_MPS2 {
                self.mu_estimate = None;
            }
        }
        if self
            .mu_estimate
            .is_some_and(|(_, at)| !within(t, at, FRICTION_ESTIMATE_HOLD))
        {
            self.mu_estimate = None;
        }
        let mu = self.mu_estimate.map(|(m, _)| m);
        if mu.is_some_and(|m| m < MU_CONDITION_I) {
            self.below_i_since.get_or_insert(t);
        } else {
            self.below_i_since = None;
        }
        if mu.is_some_and(|m| m < MU_CONDITION_J) {
            self.below_j_since.get_or_insert(t);
        } else {
            self.below_j_since = None;
        }
    }

    /// A verified CAM: its sender's claimed position, speed and heading.
    pub fn on_cam(&mut self, signer: [u8; 8], pos: Vec3, speed: f64, heading: f64, at: SimTime) {
        self.cams.insert(
            signer,
            Heard {
                pos,
                speed,
                heading,
                at,
            },
        );
    }

    /// A verified DENM of `cause` (1 or 27) at `pos`.
    pub fn on_denm(&mut self, signer: [u8; 8], cause: u8, pos: Vec3, at: SimTime) {
        self.denms.insert(
            (signer, cause),
            Heard {
                pos,
                speed: 0.0,
                heading: 0.0,
                at,
            },
        );
    }

    /// The vehicles the own sensors report now: position, velocity, heading.
    pub fn set_perceived(&mut self, objects: Vec<(Vec3, Vec3, f64)>) {
        self.perceived = objects;
    }

    fn forget_stale(&mut self, now: SimTime) {
        self.cams.retain(|_, h| within(now, h.at, CAM_FRESH));
        self.denms
            .retain(|_, h| within(now, h.at, DENM_HEARD_FRESH));
    }

    /// Whether the speed was above 80 km/h for a block of at least 30 s within
    /// `lookback` before `now` (the block clipped to the window).
    fn fast_block(&self, now: SimTime, lookback: Duration) -> bool {
        let from = now.saturating_sub(lookback.as_nanos());
        let mut start: Option<SimTime> = None;
        for s in self.history.iter().filter(|s| s.t >= from) {
            if s.v > NON_URBAN_SPEED_MPS {
                let st = *start.get_or_insert(s.t);
                if s.t.saturating_sub(st) >= NON_URBAN_BLOCK.as_nanos() {
                    return true;
                }
            } else {
                start = None;
            }
        }
        false
    }

    fn map_non_urban(&self) -> bool {
        self.history
            .back()
            .is_some_and(|s| s.non_urban == Some(true))
    }

    /// Whether the map said non-urban at every reading of the last `d`.
    fn map_non_urban_throughout(&self, now: SimTime, d: Duration) -> bool {
        let from = now.saturating_sub(d.as_nanos());
        let covered = self.history.front().is_some_and(|s| s.t <= from);
        covered
            && self
                .history
                .iter()
                .filter(|s| s.t >= from)
                .all(|s| s.non_urban == Some(true))
    }

    /// Sudden speed drop TRCO_0 (module table).
    fn speed_drop(&self, now: SimTime) -> bool {
        let Some(last) = self.history.back() else {
            return false;
        };
        if last.v > DROP_TARGET_MPS {
            return false;
        }
        let from = now.saturating_sub(DROP_WITHIN.as_nanos());
        let window: Vec<&Sample> = self.history.iter().filter(|s| s.t >= from).collect();
        for (i, s) in window.iter().enumerate() {
            if s.v > DROP_INITIAL_MPS
                && s.a >= -DROP_INITIAL_DECEL_MPS2
                && s.v - last.v >= DROP_MIN_MPS
                && window[i + 1..]
                    .iter()
                    .any(|x| x.a < -DROP_INSTANT_DECEL_MPS2)
            {
                return true;
            }
        }
        false
    }

    /// How long the vehicle has stood, up to now.
    fn standing_for(&self, now: SimTime) -> u64 {
        let mut since = None;
        for s in self.history.iter().rev() {
            if s.v < STATIONARY_MPS {
                since = Some(s.t);
            } else {
                break;
            }
        }
        since.map_or(0, |t| now.saturating_sub(t))
    }

    /// Local slow down TRCO_0: the average moving speed over T1, restarted after a
    /// stationary period longer than T2, is 30 km/h or less.
    fn slow_average(&self, now: SimTime) -> bool {
        // The restart: the end of the last stationary run longer than T2.
        let mut restart: Option<SimTime> = None;
        let mut run_start: Option<SimTime> = None;
        for s in &self.history {
            if s.v < STATIONARY_MPS {
                run_start.get_or_insert(s.t);
            } else {
                if let Some(st) = run_start.take()
                    && s.t.saturating_sub(st) > SLOW_T2.as_nanos()
                {
                    restart = Some(s.t);
                }
            }
        }
        if run_start.is_some_and(|st| now.saturating_sub(st) > SLOW_T2.as_nanos()) {
            return false;
        }
        let Some(first) = self.history.front() else {
            return false;
        };
        let origin = restart.unwrap_or(first.t).max(first.t);
        if now.saturating_sub(origin) < SLOW_T1.as_nanos() {
            return false;
        }
        let from = now.saturating_sub(SLOW_T1.as_nanos()).max(origin);
        let (mut sum, mut n) = (0.0, 0u32);
        for s in self
            .history
            .iter()
            .filter(|s| s.t >= from && s.v >= STATIONARY_MPS)
        {
            sum += s.v;
            n += 1;
        }
        n > 0 && sum / f64::from(n) <= SLOW_MPS
    }

    fn relevant_denm(&self, own: &crate::events::Ego, cause: u8, range_m: f64) -> bool {
        self.denms.iter().any(|((_, c), h)| {
            *c == cause && {
                let (dx, dy) = (h.pos.x - own.pos.x, h.pos.y - own.pos.y);
                let d = v2xw_core::math::hypot(dx, dy);
                let bearing = wrap_pi(v2xw_core::math::atan2(dy, dx) - own.heading);
                d < range_m && bearing.abs() <= RELEVANCE_SECTOR_RAD
            }
        })
    }

    fn slow_cams(&self, own: &crate::events::Ego) -> usize {
        self.cams
            .values()
            .filter(|h| {
                h.speed <= SLOW_MPS
                    && v2xw_core::math::hypot(h.pos.x - own.pos.x, h.pos.y - own.pos.y)
                        < SLOW_RELEVANCE_M
                    && wrap_pi(h.heading - own.heading).abs() < RELEVANCE_HEADING_RAD
            })
            .count()
    }

    fn slow_perceived(&self, own: &crate::events::Ego) -> usize {
        self.perceived
            .iter()
            .filter(|(p, v, heading)| {
                v2xw_core::math::hypot(v.x, v.y) <= SLOW_MPS
                    && v2xw_core::math::hypot(p.x - own.pos.x, p.y - own.pos.y)
                        <= SLOW_SENSOR_RANGE_M
                    && wrap_pi(*heading - own.heading).abs() < RELEVANCE_HEADING_RAD
            })
            .count()
    }

    fn lane_blocked(&self, own: &crate::events::Ego) -> bool {
        let (s, c) = v2xw_core::math::sin_cos(own.heading);
        self.perceived.iter().any(|(p, v, _)| {
            let (dx, dy) = (p.x - own.pos.x, p.y - own.pos.y);
            let ahead = dx * c + dy * s;
            let lateral = -dx * s + dy * c;
            ahead > 0.0
                && ahead <= BLOCKED_LANE_AHEAD_M
                && lateral.abs() <= HALF_LANE_M
                && v2xw_core::math::hypot(v.x, v.y) < BLOCKED_LANE_MPS
        })
    }

    /// Sudden speed drop: `Some(information quality)` when a DENM is to be raised now.
    pub fn sudden_speed_drop(&mut self, now: SimTime, own: &crate::events::Ego) -> Option<u8> {
        self.forget_stale(now);
        let pre = self.map_non_urban() || self.fast_block(now, SPEED_DROP_LOOKBACK);
        // TRCO_0, 1, 3, 4, 6.
        let raw = [
            self.speed_drop(now),
            self.hazard_since
                .is_some_and(|t| now.saturating_sub(t) >= HAZARD_MIN.as_nanos()),
            self.relevant_denm(own, CAUSE_DANGEROUS_END_OF_QUEUE, DROP_RELEVANCE_M),
            self.relevant_denm(own, CAUSE_TRAFFIC_CONDITION, DROP_RELEVANCE_M),
            self.lane_blocked(own),
        ];
        let valid = hold(&mut self.drop_last, raw, now, DROP_CONDITION_HOLD);
        let [t0, t1, t3, t4, t6] = valid;
        if !pre || self.drop_blocked_until.is_some_and(|t| now < t) {
            return None;
        }
        let fires = (t0 && (t1 || t3 || t4 || t6)) || (t1 && (t3 || t4 || t6));
        if !fires {
            return None;
        }
        self.drop_blocked_until = Some(DROP_BLOCKING.after(now));
        // Table 4: driver reaction (TRCO_0, 1), environment (3, 4), sensors (6).
        let env = t3 || t4;
        Some(match (env, t6) {
            (true, true) => 3,
            (false, true) => 2,
            _ => 1,
        })
    }

    /// Local slow down: `Some(information quality)` when a DENM is to be raised now.
    /// `stationary_event` is whether the vehicle's own stationary-vehicle service is live.
    pub fn local_slow_down(
        &mut self,
        now: SimTime,
        own: &crate::events::Ego,
        stationary_event: bool,
    ) -> Option<u8> {
        self.forget_stale(now);
        let pre =
            !stationary_event && (self.map_non_urban() || self.fast_block(now, SLOW_DOWN_LOOKBACK));
        let stood = self.standing_for(now) >= SLOW_T2.as_nanos();
        // TRCO_0, 1, 2, 4, 5, 6.
        let raw = [
            self.slow_average(now),
            stood,
            self.relevant_denm(own, CAUSE_TRAFFIC_CONDITION, SLOW_RELEVANCE_M),
            self.slow_cams(own) >= SLOW_VEHICLES,
            self.slow_perceived(own) >= SLOW_VEHICLES,
            false,
        ];
        let valid = hold(&mut self.slow_last, raw, now, SLOW_CONDITION_HOLD);
        let [t0, t1, t2, t4, t5, _] = valid;
        if !pre || self.slow_blocked_until.is_some_and(|t| now < t) {
            return None;
        }
        let by_1 = t0;
        let by_2 = t1 && (t2 || t4 || t5);
        if !(by_1 || by_2) {
            return None;
        }
        self.slow_blocked_until = Some(SLOW_BLOCKING.after(now));
        // TRCO_6: the map said non-urban throughout T1 (condition 1) or T2 (condition 2).
        let t6 = (by_1 && self.map_non_urban_throughout(now, SLOW_T1))
            || (by_2 && self.map_non_urban_throughout(now, SLOW_T2));
        // Table 8, the highest value that applies.
        let env = by_2 && (t2 || t4);
        let sensor = by_2 && t5;
        Some(if t6 {
            5
        } else if env && sensor {
            4
        } else if sensor {
            3
        } else if env {
            2
        } else {
            1
        })
    }

    /// Traction loss at `now`, given whether this node's traction event is still live.
    pub fn traction_loss(&mut self, now: SimTime, live: bool) -> Option<TractionAction> {
        let held = |since: Option<SimTime>| {
            since.is_some_and(|t| now.saturating_sub(t) >= MU_FOR.as_nanos())
        };
        let j = held(self.below_j_since);
        let i = held(self.below_i_since);
        let quality = if j { 7 } else { 6 };
        let fulfilled = i || j;
        if !live {
            self.traction_last_update = None;
            self.traction_finished = false;
            if fulfilled {
                self.traction_last_update = Some(now);
                return Some(TractionAction::New(quality));
            }
            return None;
        }
        if fulfilled {
            self.traction_finished = false;
            if self
                .traction_last_update
                .is_none_or(|t| now.saturating_sub(t) >= TRACTION_UPDATE.as_nanos())
            {
                self.traction_last_update = Some(now);
                return Some(TractionAction::Update(quality));
            }
            return None;
        }
        // RS_tcAdWe_169: one last update when the condition ends, then none.
        if !self.traction_finished {
            self.traction_finished = true;
            self.traction_last_update = Some(now);
            return Some(TractionAction::Update(6));
        }
        None
    }

    /// The friction estimate, if the vehicle has one.
    pub fn friction_estimate(&self) -> Option<f64> {
        self.mu_estimate.map(|(m, _)| m)
    }
}

/// Applies a condition's validity extension: true while active and for `extra` after.
fn hold<const N: usize>(
    last: &mut [Option<SimTime>; N],
    raw: [bool; N],
    now: SimTime,
    extra: Duration,
) -> [bool; N] {
    let mut out = [false; N];
    for k in 0..N {
        if raw[k] {
            last[k] = Some(now);
        }
        out[k] = last[k].is_some_and(|t| within(now, t, extra));
    }
    out
}
