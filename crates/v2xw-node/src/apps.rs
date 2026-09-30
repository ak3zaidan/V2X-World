//! The V2X applications an on-board unit runs, with their standard triggers: forward
//! collision warning, emergency electronic brake light, intersection movement assist,
//! left-turn assist, blind-spot and lane-change warning, pedestrian collision warning,
//! red-light violation warning, and green-light optimal speed advisory.
//!
//! # What an application may read
//!
//! What the unit heard and what the vehicle knows about itself — nothing else:
//!
//! * the messages its stack delivered, decoded from their own octets: a BSM's position,
//!   speed, heading, acceleration, event flags and path prediction; a CAM's; a PSM's; a
//!   DENM's event; a SPaT's signal states and timing; a MAP's lanes;
//! * its own position belief, and its own vehicle bus ([`crate::vehicle`]) — speed,
//!   acceleration, yaw rate, turn signal, the navigation's next movement, the speed limit.
//!
//! So a lost BSM is a late warning, a ghost vehicle a false one, and a stale SPaT a wrong
//! advisory, exactly as in the field. The same decision functions are exported so the
//! engine can run them over ground truth ([`Track`]) and label each warning true or false
//! and find the ones that were missed.
//!
//! # The applications, their triggers and where each threshold comes from
//!
//! | App | Fires when | Threshold | Source |
//! |---|---|---|---|
//! | FCW | a vehicle ahead in the ego's path would be hit within the TTC threshold, its own deceleration included | TTC ≤ 2.4 s | NHTSA's FCW confirmation test (NCAP, 2013): an alert no later than 2.4 s TTC for a decelerating lead (2.1 s stopped, 2.0 s slower) — the most demanding case |
//! | EEBL | a vehicle ahead in the same direction brakes hard (its BSM hard-braking flag or accelSet, or an EEBL DENM) | ≤ 300 m ahead, within one adjacent lane | the 0.4 g flag is J2735 §7.234; the 300 m reach is CAMP VSC-A's EEBL range as recalled, a parameter |
//! | IMA | a crossing vehicle and the ego would reach the conflict point within a short time of each other | TTI ≤ 4 s and ΔT ≤ 1.5 s | ΔT is FHWA SSAM's 1.5 s conflict threshold; the 4 s look-ahead is this build's choice |
//! | LTA | the ego is about to turn left and an opposing vehicle arrives inside the critical gap | 4.1 s | HCM 6th ed. Exhibit 20-11: base critical headway for a major-street left turn |
//! | BSW | a same-direction vehicle is in the ego's blind zone | 0.5–3.0 m beside, up to 3 m behind the rear | ISO 17387 (LCDAS) adjacent-zone definition |
//! | LCW | as BSW, while the ego signals toward that side | — | ISO 17387 |
//! | PCW | a pedestrian or cyclist would be in the ego's path | TTC ≤ 3 s, ± 1 m | this build's choice: no V2P warning timing is standardised |
//! | RLVW | the ego would enter on red and cannot stop comfortably | required deceleration ≥ 3.4 m/s² after a 1 s reaction | AASHTO's 3.4 m/s² comfortable deceleration; ITE's 1.0 s perception-reaction time |
//! | GLOSA | (advisory) the speed that reaches the stop line on green | 20 km/h minimum advice | Katsaros et al. 2011's GLOSA; the minimum is this build's choice |
//!
//! A warning is issued once (`issue`), refreshed at most once a second (`update`) and
//! cleared when the condition has been gone for [`CLEAR_HOLD`] (`clear`), on `app.warning`.
//! GLOSA writes `app.advice`.

use std::collections::BTreeMap;

use v2xw_core::belief::PositionEstimate;
use v2xw_core::geo::GeoOrigin;
use v2xw_core::geom::Vec3;
use v2xw_core::ids::NodeId;
use v2xw_core::math;
use v2xw_core::time::{Duration, SimTime, WallClock};
use v2xw_msg::MsgType;
use v2xw_msg::j2735::spat::MovementPhaseState;
use v2xw_msg::sec_types::HashedId8;

use crate::ctx::{NodeCtx, NodeCtxExt};
use crate::runtime::VerifiedMessage;
use crate::safety::{Severity, Surrogates, WarningKind, digest_hex, digest_key, q};
use crate::stores::VerificationState;
use crate::vehicle::{OwnVehicle, TurnIntent};

/// How long a condition must be gone before its warning clears.
pub const CLEAR_HOLD: Duration = Duration::from_millis(300);

/// How often a standing warning is refreshed on `app.warning`.
pub const UPDATE_EVERY: Duration = Duration::from_secs(1);

/// A heard vehicle, pedestrian or ego, in the terms the applications use. World ENU
/// metres, ENU radians, m/s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Track {
    /// Where.
    pub pos: Vec3,
    /// Heading, ENU radians.
    pub heading_rad: f64,
    /// Speed, m/s.
    pub speed_mps: f64,
    /// Longitudinal acceleration, m/s², when known.
    pub accel_mps2: f64,
    /// Yaw rate, rad/s (positive left), when known.
    pub yaw_rate_rad_s: f64,
    /// Body length, metres.
    pub length_m: f64,
    /// Body width, metres.
    pub width_m: f64,
    /// Whether it says it is braking hard (J2735 `eventHardBraking`, or its acceleration
    /// past 0.4 g, or an EEBL DENM).
    pub hard_braking: bool,
    /// Whether it is a vulnerable road user.
    pub vru: bool,
}

impl Track {
    fn dir(&self) -> (f64, f64) {
        let (s, c) = math::sin_cos(self.heading_rad);
        (c, s)
    }

    /// Where `p` is in this track's frame: `(ahead, left)`.
    pub fn frame(&self, p: Vec3) -> (f64, f64) {
        let (c, s) = self.dir();
        let (dx, dy) = (p.x - self.pos.x, p.y - self.pos.y);
        (dx * c + dy * s, -dx * s + dy * c)
    }

    /// The track moved on by `dt` seconds at its speed and acceleration, never reversing:
    /// the extrapolation a V2V application applies to a message's age.
    #[must_use]
    pub fn advanced(&self, dt: f64) -> Track {
        if dt <= 0.0 {
            return *self;
        }
        let a = self.accel_mps2;
        let (t_stop, dist) = if a < 0.0 && self.speed_mps > 0.0 {
            let t = self.speed_mps / -a;
            if t < dt {
                (t, self.speed_mps * t + 0.5 * a * t * t)
            } else {
                (dt, self.speed_mps * dt + 0.5 * a * dt * dt)
            }
        } else {
            (dt, self.speed_mps * dt + 0.5 * a * dt * dt)
        };
        let _ = t_stop;
        let (c, s) = self.dir();
        Track {
            pos: Vec3::new(self.pos.x + c * dist, self.pos.y + s * dist, self.pos.z),
            speed_mps: (self.speed_mps + a * dt).max(0.0),
            ..*self
        }
    }
}

/// `|a − b|` wrapped to `[0, π]`.
pub fn heading_gap(a: f64, b: f64) -> f64 {
    v2xw_msg::j2945::wrap_pi(a - b).abs()
}

fn deg(d: f64) -> f64 {
    d * core::f64::consts::PI / 180.0
}

/// Time until `ego` (at constant speed) closes on `lead` (at its own deceleration, never
/// reversing) across `gap_m`, seconds; infinity when it never does within 10 s.
pub fn time_to_collision(gap_m: f64, ego_v: f64, lead_v: f64, lead_a: f64) -> f64 {
    if gap_m <= 0.0 {
        return 0.0;
    }
    let mut t = 0.0;
    let dt = 0.05;
    while t <= 10.0 {
        let lead_travel = if lead_a < 0.0 && lead_v > 0.0 {
            let ts = lead_v / -lead_a;
            let tt = t.min(ts);
            lead_v * tt + 0.5 * lead_a * tt * tt
        } else if lead_v <= 0.0 && lead_a <= 0.0 {
            0.0
        } else {
            lead_v * t + 0.5 * lead_a.max(0.0) * t * t
        };
        if ego_v * t >= gap_m + lead_travel {
            return t;
        }
        t += dt;
    }
    f64::INFINITY
}

/// Lateral offset of a point at `(ahead, left)` from the ego's predicted arc of curvature
/// `kappa` (1/m, positive left).
fn off_path(ahead: f64, left: f64, kappa: f64) -> f64 {
    left - 0.5 * kappa * ahead * ahead
}

/// FCW parameters.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FcwParams {
    /// Warn at or below this TTC, seconds.
    pub ttc_s: f64,
    /// How far ahead a lead counts, metres.
    pub range_m: f64,
    /// Half the in-path corridor, metres.
    pub half_width_m: f64,
    /// Below this ego speed FCW is inactive, m/s.
    pub min_speed_mps: f64,
}

impl Default for FcwParams {
    fn default() -> Self {
        Self {
            ttc_s: 2.4,
            range_m: 150.0,
            half_width_m: 1.8,
            min_speed_mps: 2.8,
        }
    }
}

/// FCW's decision about one lead.
pub fn fcw(ego: &Track, lead: &Track, p: &FcwParams) -> Option<Surrogates> {
    if ego.speed_mps < p.min_speed_mps || lead.vru {
        return None;
    }
    if heading_gap(ego.heading_rad, lead.heading_rad) > deg(30.0) {
        return None;
    }
    let (ahead, left) = ego.frame(lead.pos);
    if ahead <= 0.0 || ahead > p.range_m {
        return None;
    }
    let kappa = if ego.speed_mps > 1.0 {
        ego.yaw_rate_rad_s / ego.speed_mps
    } else {
        0.0
    };
    if off_path(ahead, left, kappa).abs() > p.half_width_m {
        return None;
    }
    let gap = (ahead - 0.5 * (ego.length_m + lead.length_m)).max(0.0);
    let ttc = time_to_collision(gap, ego.speed_mps, lead.speed_mps, lead.accel_mps2);
    if ttc > p.ttc_s {
        return None;
    }
    let closing = ego.speed_mps - lead.speed_mps;
    Some(Surrogates {
        ttc_s: ttc,
        pet_s: f64::NAN,
        required_decel_mps2: if gap > 0.0 && closing > 0.0 {
            closing * closing / (2.0 * gap)
        } else {
            f64::NAN
        },
        distance_m: ahead,
        closing_mps: closing,
    })
}

/// EEBL parameters.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EeblParams {
    /// How far ahead, metres.
    pub range_m: f64,
    /// How far to either side of the ego's path, metres: the ego's lane and the next one.
    pub lateral_m: f64,
}

impl Default for EeblParams {
    fn default() -> Self {
        Self {
            range_m: 300.0,
            lateral_m: 5.4,
        }
    }
}

/// EEBL's decision about one remote vehicle.
pub fn eebl(ego: &Track, remote: &Track, p: &EeblParams) -> Option<Surrogates> {
    if !remote.hard_braking || remote.vru {
        return None;
    }
    if heading_gap(ego.heading_rad, remote.heading_rad) > deg(45.0) {
        return None;
    }
    let (ahead, left) = ego.frame(remote.pos);
    if ahead <= 0.0 || ahead > p.range_m || left.abs() > p.lateral_m {
        return None;
    }
    let gap = (ahead - 0.5 * (ego.length_m + remote.length_m)).max(0.0);
    let ttc = time_to_collision(gap, ego.speed_mps, remote.speed_mps, remote.accel_mps2);
    Some(Surrogates {
        ttc_s: ttc,
        pet_s: f64::NAN,
        required_decel_mps2: f64::NAN,
        distance_m: ahead,
        closing_mps: ego.speed_mps - remote.speed_mps,
    })
}

/// IMA parameters.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ImaParams {
    /// Look-ahead: the ego's time to the conflict point, seconds.
    pub tti_s: f64,
    /// The largest difference between the two arrival times, seconds.
    pub window_s: f64,
}

impl Default for ImaParams {
    fn default() -> Self {
        Self {
            tti_s: 4.0,
            window_s: crate::safety::TTC_CONFLICT_THRESHOLD_S,
        }
    }
}

/// Where two headings cross: distances along each to the crossing point, if both are
/// ahead.
fn crossing(a: &Track, b: &Track) -> Option<(f64, f64)> {
    let (ax, ay) = a.dir();
    let (bx, by) = b.dir();
    let den = ax * by - ay * bx;
    if den.abs() < 1e-6 {
        return None;
    }
    let (dx, dy) = (b.pos.x - a.pos.x, b.pos.y - a.pos.y);
    let sa = (dx * by - dy * bx) / den;
    let sb = (dx * ay - dy * ax) / den;
    (sa > 0.0 && sb > 0.0).then_some((sa, sb))
}

/// IMA's decision about one crossing vehicle.
pub fn ima(ego: &Track, remote: &Track, p: &ImaParams) -> Option<Surrogates> {
    if remote.vru || ego.speed_mps < 1.0 || remote.speed_mps < 1.0 {
        return None;
    }
    let angle = heading_gap(ego.heading_rad, remote.heading_rad);
    if !(deg(45.0)..=deg(135.0)).contains(&angle) {
        return None;
    }
    let (se, sr) = crossing(ego, remote)?;
    let te = se / ego.speed_mps;
    let tr = sr / remote.speed_mps;
    if te > p.tti_s || (te - tr).abs() > p.window_s {
        return None;
    }
    Some(Surrogates {
        ttc_s: te.max(tr),
        pet_s: (te - tr).abs(),
        required_decel_mps2: f64::NAN,
        distance_m: ego.pos.distance_2d(remote.pos),
        closing_mps: f64::NAN,
    })
}

/// LTA parameters.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LtaParams {
    /// The critical gap an opposing vehicle must leave, seconds.
    pub gap_s: f64,
    /// How close to the turn the ego must be for LTA to look, metres.
    pub approach_m: f64,
}

impl Default for LtaParams {
    fn default() -> Self {
        Self {
            gap_s: 4.1,
            approach_m: 30.0,
        }
    }
}

/// LTA's decision about one opposing vehicle, for an ego about to turn left at
/// `junction` in `distance_m`.
pub fn lta(
    ego: &Track,
    junction: Vec3,
    distance_m: f64,
    remote: &Track,
    p: &LtaParams,
) -> Option<Surrogates> {
    if remote.vru || distance_m > p.approach_m || remote.speed_mps < 1.0 {
        return None;
    }
    if heading_gap(ego.heading_rad, remote.heading_rad) < deg(135.0) {
        return None;
    }
    // The opposing vehicle is approaching the junction, not leaving it.
    let (along, _) = remote.frame(junction);
    if along <= 0.0 || along > 150.0 {
        return None;
    }
    let tr = along / remote.speed_mps;
    let te = distance_m.max(0.0) / ego.speed_mps.max(1.0);
    if tr - te > p.gap_s {
        return None;
    }
    Some(Surrogates {
        ttc_s: tr,
        pet_s: (tr - te).max(0.0),
        required_decel_mps2: f64::NAN,
        distance_m: along,
        closing_mps: remote.speed_mps,
    })
}

/// Blind-spot zone parameters (ISO 17387).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BswParams {
    /// Nearest lateral edge of the zone beyond the ego's side, metres.
    pub lateral_min_m: f64,
    /// Farthest lateral edge, metres.
    pub lateral_max_m: f64,
    /// How far behind the ego's rear the zone reaches, metres.
    pub behind_m: f64,
}

impl Default for BswParams {
    fn default() -> Self {
        Self {
            lateral_min_m: 0.5,
            lateral_max_m: 3.0,
            behind_m: 3.0,
        }
    }
}

/// Whether `remote` is in the ego's blind zone, and on which side (`true` = left).
pub fn blind_spot(ego: &Track, remote: &Track, p: &BswParams) -> Option<(bool, Surrogates)> {
    if remote.vru || heading_gap(ego.heading_rad, remote.heading_rad) > deg(30.0) {
        return None;
    }
    let (ahead, left) = ego.frame(remote.pos);
    let half = 0.5 * ego.width_m;
    let side_gap = left.abs() - half - 0.5 * remote.width_m;
    if side_gap < p.lateral_min_m - 0.5 * remote.width_m || side_gap > p.lateral_max_m {
        return None;
    }
    // The zone runs from the driver's eye point (about 1.5 m behind the front) back to
    // `behind_m` behind the rear; the remote is in it when its body overlaps that span.
    let front = 0.5 * ego.length_m - 1.5;
    let back = -(0.5 * ego.length_m + p.behind_m);
    let (r_front, r_back) = (ahead + 0.5 * remote.length_m, ahead - 0.5 * remote.length_m);
    if r_back > front || r_front < back {
        return None;
    }
    Some((
        left > 0.0,
        Surrogates {
            ttc_s: f64::NAN,
            pet_s: f64::NAN,
            required_decel_mps2: f64::NAN,
            distance_m: ego.pos.distance_2d(remote.pos),
            closing_mps: remote.speed_mps - ego.speed_mps,
        },
    ))
}

/// PCW parameters.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PcwParams {
    /// Warn at or below this time, seconds.
    pub ttc_s: f64,
    /// Margin beyond the ego's half width, metres.
    pub margin_m: f64,
}

impl Default for PcwParams {
    fn default() -> Self {
        Self {
            ttc_s: 3.0,
            margin_m: 1.0,
        }
    }
}

/// PCW's decision about one pedestrian or cyclist.
pub fn pcw(ego: &Track, vru: &Track, p: &PcwParams) -> Option<Surrogates> {
    if !vru.vru || ego.speed_mps < 1.0 {
        return None;
    }
    let (ahead, left) = ego.frame(vru.pos);
    if ahead <= 0.0 {
        return None;
    }
    // The VRU's velocity in the ego's frame.
    let (c, s) = ego.dir();
    let (vc, vs) = vru.dir();
    let (vx, vy) = (vc * vru.speed_mps, vs * vru.speed_mps);
    let v_ahead = vx * c + vy * s;
    let v_left = -vx * s + vy * c;
    let closing = ego.speed_mps - v_ahead;
    if closing <= 0.1 {
        return None;
    }
    let t = (ahead - 0.5 * ego.length_m).max(0.0) / closing;
    if t > p.ttc_s {
        return None;
    }
    let kappa = if ego.speed_mps > 1.0 {
        ego.yaw_rate_rad_s / ego.speed_mps
    } else {
        0.0
    };
    let y_then = left + v_left * t;
    if off_path(ahead, y_then, kappa).abs() > 0.5 * ego.width_m + p.margin_m {
        return None;
    }
    Some(Surrogates {
        ttc_s: t,
        pet_s: f64::NAN,
        required_decel_mps2: f64::NAN,
        distance_m: ahead,
        closing_mps: closing,
    })
}

/// RLVW and GLOSA parameters.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SignalAppParams {
    /// RLVW: the deceleration past which a stop is not comfortable, m/s².
    pub rlvw_decel_mps2: f64,
    /// RLVW: the driver's reaction time, seconds.
    pub rlvw_reaction_s: f64,
    /// GLOSA: how far from the stop line advice starts, metres.
    pub glosa_range_m: f64,
    /// GLOSA: the lowest speed it advises, m/s (below it, it advises a stop).
    pub glosa_min_speed_mps: f64,
    /// GLOSA: the margin kept before a phase ends, seconds.
    pub glosa_margin_s: f64,
}

impl Default for SignalAppParams {
    fn default() -> Self {
        Self {
            rlvw_decel_mps2: 3.4,
            rlvw_reaction_s: 1.0,
            glosa_range_m: 300.0,
            glosa_min_speed_mps: 20.0 / 3.6,
            glosa_margin_s: 1.0,
        }
    }
}

/// Which applications run, and their parameters.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AppParams {
    /// FCW on.
    pub fcw: bool,
    /// EEBL on.
    pub eebl: bool,
    /// IMA on.
    pub ima: bool,
    /// LTA on.
    pub lta: bool,
    /// BSW and LCW on.
    pub bsw: bool,
    /// PCW on.
    pub pcw: bool,
    /// RLVW on.
    pub rlvw: bool,
    /// GLOSA on.
    pub glosa: bool,
    /// FCW's parameters.
    pub fcw_params: FcwParams,
    /// EEBL's.
    pub eebl_params: EeblParams,
    /// IMA's.
    pub ima_params: ImaParams,
    /// LTA's.
    pub lta_params: LtaParams,
    /// BSW's.
    pub bsw_params: BswParams,
    /// PCW's.
    pub pcw_params: PcwParams,
    /// RLVW's and GLOSA's.
    pub signal_params: SignalAppParams,
    /// How long a heard vehicle stays a candidate without a new message, seconds.
    pub track_timeout_s: f64,
}

impl Default for AppParams {
    fn default() -> Self {
        Self {
            fcw: true,
            eebl: true,
            ima: true,
            lta: true,
            bsw: true,
            pcw: true,
            rlvw: true,
            glosa: true,
            fcw_params: FcwParams::default(),
            eebl_params: EeblParams::default(),
            ima_params: ImaParams::default(),
            lta_params: LtaParams::default(),
            bsw_params: BswParams::default(),
            pcw_params: PcwParams::default(),
            signal_params: SignalAppParams::default(),
            // A BSM every 100 ms and J2945/1's 1 s staleness: a vehicle unheard for a
            // second is not tracked.
            track_timeout_s: 1.0,
        }
    }
}

/// A heard station.
#[derive(Debug, Clone, PartialEq)]
struct Heard {
    signer: HashedId8,
    track: Track,
    /// The generation time it claimed, on its clock.
    generated: SimTime,
    /// When this node heard it.
    heard: SimTime,
    /// `Some("cpm")` for an object another station perceived and reported.
    via: Option<&'static str>,
}

/// How close a perceived object may be to a station this node hears from its own
/// awareness messages, or to this node itself, to be the same road user, metres: two
/// reports of one vehicle, its BSM or CAM and another station's CPM, are fused into the
/// one the vehicle sends itself (the self-announced one is the better estimate).
pub const FUSION_GATE_M: f64 = 3.0;

/// One ingress lane of a heard MAP, in world coordinates: the stop line first.
#[derive(Debug, Clone, PartialEq)]
struct Ingress {
    points: Vec<Vec3>,
    half_width_m: f64,
    /// (egress lane id, signal group).
    connections: Vec<(u8, Option<u8>)>,
}

/// One heard intersection: its MAP and its latest SPaT.
#[derive(Debug, Clone, PartialEq, Default)]
struct Intersection {
    signer: Option<HashedId8>,
    ingress: Vec<Ingress>,
    /// Egress lanes' first two points, for classifying a connection's turn.
    egress: BTreeMap<u8, (Vec3, Vec3)>,
    map_heard: SimTime,
    /// Signal group → (state, min end, max end), TimeMarks.
    states: BTreeMap<u8, (MovementPhaseState, u16, Option<u16>)>,
    /// Seconds into the UTC hour when the SPaT was heard, and when (node clock).
    spat_heard: Option<SimTime>,
}

/// A DENM event heard.
#[derive(Debug, Clone, PartialEq)]
struct DenmHeard {
    signer: HashedId8,
    pos: Vec3,
    eebl: bool,
    heard: SimTime,
}

/// GLOSA's advice, for the driver (and the engine's compliance model).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeedAdvice {
    /// The speed advised, m/s; `None` advises a stop at the line.
    pub target_mps: Option<f64>,
    /// Metres to the stop line.
    pub distance_m: f64,
}

/// `app.advice` — a GLOSA advisory at one node (NODE).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AdviceRecord {
    /// The node's own believed instant.
    pub t: SimTime,
    /// The advised node.
    pub node: NodeId,
    /// `glosa`.
    pub app: &'static str,
    /// The intersection's J2735 id.
    pub intersection: u16,
    /// The signal group of the ego's movement.
    pub signal_group: u8,
    /// Metres to the stop line.
    pub distance_m: f64,
    /// The movement's state, as J2735 names it.
    pub state: &'static str,
    /// Seconds until it changes, as the SPaT said.
    pub time_to_change_s: f64,
    /// The advised speed, m/s; `NaN` advises a stop.
    pub advised_mps: f64,
    /// The ego's speed, m/s.
    pub speed_mps: f64,
}

impl v2xw_core::ctx::Record for AdviceRecord {
    const CHANNEL: &'static str = "app.advice";
    const VISIBILITY: v2xw_core::ctx::Visibility = v2xw_core::ctx::Visibility::Node;
}

/// Per (app, subject) warning state.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Standing {
    issued: SimTime,
    last_seen: SimTime,
    last_update: SimTime,
}

/// The application layer of one on-board unit.
#[derive(Debug, Clone)]
pub struct AppLayer {
    params: AppParams,
    heard: BTreeMap<[u8; 8], Heard>,
    intersections: BTreeMap<u16, Intersection>,
    denms: BTreeMap<[u8; 8], DenmHeard>,
    standing: BTreeMap<(&'static str, [u8; 8]), (HashedId8, Standing)>,
    last_advice: Option<(SimTime, Option<f64>)>,
    /// Where this node last believed it was, for fusing reported objects.
    ego_pos: Option<Vec3>,
    origin: GeoOrigin,
    wall: WallClock,
    issued: u64,
}

impl AppLayer {
    /// A layer running `params` for a vehicle in a world anchored at `origin`, whose
    /// clock is `wall`.
    pub fn new(params: AppParams, origin: GeoOrigin, wall: WallClock) -> Self {
        Self {
            params,
            heard: BTreeMap::new(),
            intersections: BTreeMap::new(),
            denms: BTreeMap::new(),
            standing: BTreeMap::new(),
            last_advice: None,
            ego_pos: None,
            origin,
            wall,
            issued: 0,
        }
    }

    /// How many warnings this layer has issued.
    pub fn issued(&self) -> u64 {
        self.issued
    }

    /// Takes one delivered message.
    pub fn on_message(&mut self, m: &VerifiedMessage) {
        if matches!(
            m.verification,
            VerificationState::Invalid | VerificationState::Revoked
        ) {
            return;
        }
        let Some(signer) = m.signer.clone() else {
            return;
        };
        match m.msg_type {
            MsgType::Bsm | MsgType::Cam | MsgType::Psm | MsgType::Vam => {
                let Some(pos) = m.claimed_pos else { return };
                let mut track = Track {
                    pos,
                    heading_rad: m.claimed_heading_rad,
                    speed_mps: m.claimed_speed_mps,
                    accel_mps2: 0.0,
                    yaw_rate_rad_s: 0.0,
                    length_m: 4.5,
                    width_m: 1.8,
                    hard_braking: false,
                    vru: matches!(m.msg_type, MsgType::Psm | MsgType::Vam),
                };
                if track.vru {
                    track.length_m = 0.5;
                    track.width_m = 0.5;
                }
                if let Some(payload) = m.payload.as_deref() {
                    decode_dynamics(m.msg_type, payload, &mut track);
                }
                self.heard.insert(
                    digest_key(&signer),
                    Heard {
                        signer,
                        track,
                        generated: m.claimed_generation_time,
                        heard: m.received_at,
                        via: None,
                    },
                );
            }
            MsgType::Cpm => {
                if let Some(payload) = m.payload.as_deref() {
                    self.on_cpm(payload, &signer, m);
                }
            }
            MsgType::Map => {
                if let Some(payload) = m.payload.as_deref() {
                    self.on_map(payload, signer, m.received_at);
                }
            }
            MsgType::Spat => {
                if let Some(payload) = m.payload.as_deref() {
                    self.on_spat(payload, signer, m.received_at);
                }
            }
            MsgType::Denm => {
                if let Some(payload) = m.payload.as_deref() {
                    self.on_denm(payload, signer, m.received_at);
                }
            }
            _ => {}
        }
    }

    /// Fuses the objects a CPM reports: each becomes a track unless a station this node
    /// hears directly, or this node itself, is within [`FUSION_GATE_M`] of it — the same
    /// road user, already known better.
    fn on_cpm(&mut self, payload: &[u8], signer: &HashedId8, m: &VerifiedMessage) {
        let Ok(cpm) = v2xw_msg::cpm::decode_cpm(payload) else {
            return;
        };
        let Some((_, objects)) = v2xw_msg::cpm::objects_of(&cpm, self.origin) else {
            return;
        };
        for (id, pos, vel, class, length_m, width_m) in objects {
            if self
                .ego_pos
                .is_some_and(|e| e.distance_2d(pos) < FUSION_GATE_M)
            {
                continue;
            }
            let known = self.heard.values().any(|h| {
                h.via.is_none() && h.track.pos.distance_2d(pos) < FUSION_GATE_M
            });
            if known {
                continue;
            }
            // The object's key: the reporting station's pseudonym with the object id in
            // its last two octets — stable while both are.
            let mut key = digest_key(signer);
            key[6] = (id >> 8) as u8;
            key[7] = id as u8;
            let speed = math::hypot(vel.x, vel.y);
            let vru = !matches!(class, v2xw_msg::cpm::CpmObjectClass::Vehicle);
            self.heard.insert(
                key,
                Heard {
                    signer: HashedId8(key.into()),
                    track: Track {
                        pos,
                        heading_rad: if speed > 0.2 {
                            math::atan2(vel.y, vel.x)
                        } else {
                            0.0
                        },
                        speed_mps: speed,
                        accel_mps2: 0.0,
                        yaw_rate_rad_s: 0.0,
                        length_m,
                        width_m,
                        hard_braking: false,
                        vru,
                    },
                    generated: m.claimed_generation_time,
                    heard: m.received_at,
                    via: Some("cpm"),
                },
            );
        }
    }

    fn on_map(&mut self, payload: &[u8], signer: HashedId8, at: SimTime) {
        let map = match v2xw_msg::j2735::map::decode_message_frame(payload) {
            Ok(m) => m,
            Err(_) => match v2xw_msg::j2735::infra::its_unwrap(payload)
                .and_then(|(_, _, body)| v2xw_msg::j2735::map::decode_map(body).ok())
            {
                Some(m) => m,
                None => return,
            },
        };
        for g in &map.intersections {
            let reference = self.origin.to_enu(
                f64::from(g.ref_point.lat) * 1e-7,
                f64::from(g.ref_point.lon) * 1e-7,
                0.0,
            );
            let default_half = f64::from(g.lane_width_cm.unwrap_or(366)) * 0.005;
            let mut entry = self
                .intersections
                .remove(&g.id.id)
                .unwrap_or_default();
            entry.signer = Some(signer.clone());
            entry.map_heard = at;
            entry.ingress.clear();
            entry.egress.clear();
            for lane in &g.lanes {
                let mut points = Vec::with_capacity(lane.nodes.len());
                let mut at_p = reference;
                for n in &lane.nodes {
                    match n.delta {
                        v2xw_msg::j2735::map::NodeOffset::Xy(o) => {
                            at_p = Vec3::new(
                                at_p.x + f64::from(o.x_cm) * 0.01,
                                at_p.y + f64::from(o.y_cm) * 0.01,
                                reference.z,
                            );
                        }
                        v2xw_msg::j2735::map::NodeOffset::LatLon { lon, lat } => {
                            at_p = self.origin.to_enu(
                                f64::from(lat) * 1e-7,
                                f64::from(lon) * 1e-7,
                                0.0,
                            );
                        }
                    }
                    points.push(at_p);
                }
                if points.len() < 2 {
                    continue;
                }
                let ingress = lane.attributes.directional_use.0
                    & v2xw_msg::j2735::map::LaneDirection::INGRESS.0
                    != 0;
                if ingress {
                    entry.ingress.push(Ingress {
                        points,
                        half_width_m: default_half,
                        connections: lane
                            .connects_to
                            .iter()
                            .map(|c| (c.connecting_lane.lane, c.signal_group))
                            .collect(),
                    });
                } else {
                    entry.egress.insert(lane.lane_id, (points[0], points[1]));
                }
            }
            self.intersections.insert(g.id.id, entry);
        }
    }

    fn on_spat(&mut self, payload: &[u8], _signer: HashedId8, at: SimTime) {
        let spat = match v2xw_msg::j2735::spat::decode_message_frame(payload) {
            Ok(s) => s,
            Err(_) => match v2xw_msg::j2735::infra::its_unwrap(payload)
                .and_then(|(_, _, body)| v2xw_msg::j2735::spat::decode_spat(body).ok())
            {
                Some(s) => s,
                None => return,
            },
        };
        for i in &spat.intersections {
            let entry = self.intersections.entry(i.id.id).or_default();
            entry.states.clear();
            for s in &i.states {
                if let Some(e) = s.events.first() {
                    let (min_end, max_end) = e.timing.map_or(
                        (v2xw_msg::j2735::spat::TIME_MARK_UNKNOWN, None),
                        |t| (t.min_end_time, t.max_end_time),
                    );
                    entry
                        .states
                        .insert(s.signal_group, (e.event_state, min_end, max_end));
                }
            }
            entry.spat_heard = Some(at);
        }
    }

    fn on_denm(&mut self, payload: &[u8], signer: HashedId8, at: SimTime) {
        let Ok(denm) = v2xw_msg::denm::decode_denm(payload) else {
            return;
        };
        let m = &denm.denm.management;
        let key = digest_key(&signer);
        if m.termination.is_some() {
            self.denms.remove(&key);
            return;
        }
        let lat = f64::from(m.event_position.latitude.0) * 1e-7;
        let lon = f64::from(m.event_position.longitude.0) * 1e-7;
        let pos = self.origin.to_enu(lat, lon, 0.0);
        let eebl = denm.denm.situation.as_ref().is_some_and(|s| {
            matches!(
                s.event_type.cc_and_scc,
                v2xw_msg::asn1::cdd::CauseCodeChoice::dangerousSituation99(_)
            )
        });
        self.denms.insert(
            key,
            DenmHeard {
                signer,
                pos,
                eebl,
                heard: at,
            },
        );
    }

    /// Runs every application at `now` (this node's clock) for an ego at `belief` with
    /// `own` vehicle state, emitting `app.warning` and `app.advice` records. Returns
    /// GLOSA's advice, if any.
    pub fn evaluate(
        &mut self,
        ctx: &mut dyn NodeCtx,
        node: NodeId,
        now: SimTime,
        belief: &PositionEstimate,
        own: &OwnVehicle,
        dims: v2xw_core::geom::Dims,
    ) -> Option<SpeedAdvice> {
        let timeout = (self.params.track_timeout_s * 1e9) as u64;
        self.heard
            .retain(|_, h| now.saturating_sub(h.heard) <= timeout);
        self.denms
            .retain(|_, d| now.saturating_sub(d.heard) <= 2_000_000_000);
        let Some(bus) = own.bus().copied() else {
            return None;
        };
        if !belief.fix.has_position() {
            return None;
        }
        self.ego_pos = Some(belief.pos);
        let ego = Track {
            pos: belief.pos,
            heading_rad: belief.heading_rad,
            speed_mps: bus.speed_mps,
            accel_mps2: bus.a_long_mps2,
            yaw_rate_rad_s: bus.yaw_rate_rad_s,
            length_m: dims.length_m,
            width_m: dims.width_m,
            hard_braking: bus.hard_braking(),
            vru: false,
        };
        let p = self.params;
        let mut fired: Vec<(&'static str, HashedId8, Severity, Surrogates)> = Vec::new();
        for h in self.heard.values() {
            let age = (now.saturating_sub(h.generated)) as f64 * 1e-9;
            let t = h.track.advanced(age.min(1.0));
            if p.fcw
                && let Some(s) = fcw(&ego, &t, &p.fcw_params)
            {
                fired.push(("fcw", h.signer.clone(), Severity::Imminent, s));
            }
            if p.eebl
                && let Some(s) = eebl(&ego, &t, &p.eebl_params)
            {
                fired.push(("eebl", h.signer.clone(), Severity::Warning, s));
            }
            if p.ima
                && let Some(s) = ima(&ego, &t, &p.ima_params)
            {
                fired.push(("ima", h.signer.clone(), Severity::Warning, s));
            }
            if p.lta
                && let Some(i) = bus.intent
                && i.turn == TurnIntent::Left
                && let Some(s) = lta(&ego, i.junction_pos, i.distance_m, &t, &p.lta_params)
            {
                fired.push(("lta", h.signer.clone(), Severity::Warning, s));
            }
            if p.bsw
                && let Some((left, s)) = blind_spot(&ego, &t, &p.bsw_params)
            {
                let signalling = match bus.turn_signal() {
                    Some(TurnIntent::Left) => left,
                    Some(TurnIntent::Right) => !left,
                    _ => false,
                };
                if signalling {
                    fired.push(("lcw", h.signer.clone(), Severity::Warning, s));
                } else {
                    fired.push(("bsw", h.signer.clone(), Severity::Caution, s));
                }
            }
            if p.pcw
                && let Some(s) = pcw(&ego, &t, &p.pcw_params)
            {
                fired.push(("pcw", h.signer.clone(), Severity::Imminent, s));
            }
        }
        // An EEBL DENM from a vehicle this node does not track from its awareness
        // messages (a CAM lost, or out of range for them) still warns.
        if p.eebl {
            for d in self.denms.values() {
                if !d.eebl || self.heard.contains_key(&digest_key(&d.signer)) {
                    continue;
                }
                let t = Track {
                    pos: d.pos,
                    heading_rad: ego.heading_rad,
                    speed_mps: 0.0,
                    accel_mps2: 0.0,
                    yaw_rate_rad_s: 0.0,
                    length_m: 4.5,
                    width_m: 1.8,
                    hard_braking: true,
                    vru: false,
                };
                if let Some(s) = eebl(&ego, &t, &p.eebl_params) {
                    fired.push(("eebl", d.signer.clone(), Severity::Warning, s));
                }
            }
        }
        // The signalised-intersection applications.
        let mut advice = None;
        if (p.rlvw || p.glosa)
            && let Some((signer, sit)) = self.signal_situation(&ego, bus.intent.map(|i| i.turn), now)
        {
            if p.rlvw
                && let Some(s) = rlvw(&ego, &sit, &p.signal_params)
            {
                fired.push(("rlvw", signer.clone(), Severity::Warning, s));
            }
            if p.glosa
                && let Some(a) = glosa(&ego, &sit, bus_limit(&bus), &p.signal_params)
            {
                advice = Some(a);
                let changed = self.last_advice.is_none_or(|(t, v)| {
                    now.saturating_sub(t) >= 1_000_000_000
                        || match (v, a.target_mps) {
                            (Some(x), Some(y)) => (x - y).abs() > 0.5,
                            (None, None) => false,
                            _ => true,
                        }
                });
                if changed {
                    self.last_advice = Some((now, a.target_mps));
                    ctx.emit(AdviceRecord {
                        t: now,
                        node,
                        app: "glosa",
                        intersection: sit.intersection,
                        signal_group: sit.signal_group,
                        distance_m: q(sit.distance_m),
                        state: phase_name(sit.state),
                        time_to_change_s: q(sit.time_left_s),
                        advised_mps: a.target_mps.map_or(f64::NAN, q),
                        speed_mps: q(ego.speed_mps),
                    });
                }
            }
        }
        if advice.is_none() {
            self.last_advice = None;
        }
        self.settle(ctx, node, now, fired);
        advice
    }

    /// Issue, update and clear, per (app, subject).
    fn settle(
        &mut self,
        ctx: &mut dyn NodeCtx,
        node: NodeId,
        now: SimTime,
        fired: Vec<(&'static str, HashedId8, Severity, Surrogates)>,
    ) {
        let mut seen: Vec<(&'static str, [u8; 8])> = Vec::new();
        for (app, subject, severity, s) in fired {
            let key = (app, digest_key(&subject));
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            let kind = match self.standing.get_mut(&key) {
                Some((_, st)) => {
                    st.last_seen = now;
                    if now.saturating_sub(st.last_update) >= UPDATE_EVERY.as_nanos() {
                        st.last_update = now;
                        Some(WarningKind::Update)
                    } else {
                        None
                    }
                }
                None => {
                    self.standing.insert(
                        key,
                        (
                            subject.clone(),
                            Standing {
                                issued: now,
                                last_seen: now,
                                last_update: now,
                            },
                        ),
                    );
                    self.issued += 1;
                    Some(WarningKind::Issue)
                }
            };
            if let Some(kind) = kind {
                let via = self.heard.get(&key.1).and_then(|h| h.via);
                ctx.emit(record(node, now, app, &subject, kind, severity, s, via));
            }
        }
        let hold = CLEAR_HOLD.as_nanos();
        let gone: Vec<(&'static str, [u8; 8])> = self
            .standing
            .iter()
            .filter(|(_, (_, st))| now.saturating_sub(st.last_seen) >= hold)
            .map(|(k, _)| *k)
            .collect();
        for key in gone {
            if let Some((subject, _)) = self.standing.remove(&key) {
                let via = self.heard.get(&key.1).and_then(|h| h.via);
                ctx.emit(record(
                    node,
                    now,
                    key.0,
                    &subject,
                    WarningKind::Clear,
                    Severity::Info,
                    Surrogates::NONE,
                    via,
                ));
            }
        }
    }

    /// The ego's signalised approach: the heard intersection whose ingress lane it is on,
    /// the signal group of its movement and that group's state and timing.
    fn signal_situation(
        &self,
        ego: &Track,
        turn: Option<TurnIntent>,
        now: SimTime,
    ) -> Option<(HashedId8, SignalSituation)> {
        let range = self.params.signal_params.glosa_range_m;
        let mut best: Option<(f64, HashedId8, SignalSituation)> = None;
        let now_hour_s = seconds_into_hour(self.wall, now);
        for (id, x) in &self.intersections {
            let Some(signer) = x.signer.clone() else {
                continue;
            };
            if x.spat_heard.is_none_or(|t| now.saturating_sub(t) > 1_000_000_000) {
                continue;
            }
            for lane in &x.ingress {
                let Some((dist, lateral, dir)) = project_ingress(&lane.points, ego.pos, range)
                else {
                    continue;
                };
                if lateral.abs() > lane.half_width_m.max(1.5)
                    || heading_gap(ego.heading_rad, dir) > deg(30.0)
                {
                    continue;
                }
                let Some(group) = pick_group(lane, &x.egress, turn) else {
                    continue;
                };
                let Some(&(state, min_end, _)) = x.states.get(&group) else {
                    continue;
                };
                let time_left =
                    v2xw_msg::j2735::spat::seconds_until(min_end, now_hour_s).unwrap_or(f64::NAN);
                let sit = SignalSituation {
                    intersection: *id,
                    signal_group: group,
                    distance_m: dist,
                    state,
                    time_left_s: time_left,
                };
                if best.as_ref().is_none_or(|(l, ..)| lateral.abs() < *l) {
                    best = Some((lateral.abs(), signer.clone(), sit));
                }
            }
        }
        best.map(|(_, s, sit)| (s, sit))
    }
}

fn bus_limit(bus: &crate::vehicle::VehicleBus) -> f64 {
    if bus.speed_limit_mps > 0.0 {
        bus.speed_limit_mps
    } else {
        // No map limit known: the urban default of most US cities, 25 mph.
        11.176
    }
}

/// The ego's signalised approach, as its applications see it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignalSituation {
    /// The intersection's J2735 id.
    pub intersection: u16,
    /// The signal group governing the ego's movement.
    pub signal_group: u8,
    /// Metres to the stop line.
    pub distance_m: f64,
    /// The movement's state now.
    pub state: MovementPhaseState,
    /// Seconds until it changes (its `minEndTime`), `NaN` when unknown.
    pub time_left_s: f64,
}

/// Whether a state lets the movement go.
pub fn is_go(s: MovementPhaseState) -> bool {
    matches!(
        s,
        MovementPhaseState::ProtectedMovementAllowed | MovementPhaseState::PermissiveMovementAllowed
    )
}

/// Whether a state is a clearance (amber).
pub fn is_amber(s: MovementPhaseState) -> bool {
    matches!(
        s,
        MovementPhaseState::ProtectedClearance | MovementPhaseState::PermissiveClearance
    )
}

/// Whether a state is a red.
pub fn is_red(s: MovementPhaseState) -> bool {
    matches!(
        s,
        MovementPhaseState::StopAndRemain | MovementPhaseState::StopThenProceed
    )
}

fn phase_name(s: MovementPhaseState) -> &'static str {
    match s {
        MovementPhaseState::Unavailable => "unavailable",
        MovementPhaseState::Dark => "dark",
        MovementPhaseState::StopThenProceed => "stop-then-proceed",
        MovementPhaseState::StopAndRemain => "stop-and-remain",
        MovementPhaseState::PreMovement => "pre-movement",
        MovementPhaseState::PermissiveMovementAllowed => "permissive-movement-allowed",
        MovementPhaseState::ProtectedMovementAllowed => "protected-movement-allowed",
        MovementPhaseState::PermissiveClearance => "permissive-clearance",
        MovementPhaseState::ProtectedClearance => "protected-clearance",
        MovementPhaseState::CautionConflictingTraffic => "caution-conflicting-traffic",
    }
}

/// RLVW: the ego would enter on red and cannot stop comfortably, and is not braking
/// enough to.
pub fn rlvw(ego: &Track, sit: &SignalSituation, p: &SignalAppParams) -> Option<Surrogates> {
    let v = ego.speed_mps;
    let d = sit.distance_m;
    if v < 2.0 || d <= 0.0 || d > 150.0 {
        return None;
    }
    let t_arrive = d / v;
    let violation = if is_red(sit.state) {
        // Red now: a violation unless the red ends before the vehicle gets there.
        !(sit.time_left_s.is_finite() && sit.time_left_s < t_arrive)
    } else if is_amber(sit.state) {
        sit.time_left_s.is_finite() && t_arrive > sit.time_left_s
    } else {
        false
    };
    if !violation {
        return None;
    }
    let room = d - v * p.rlvw_reaction_s;
    let need = if room > 0.1 {
        v * v / (2.0 * room)
    } else {
        f64::INFINITY
    };
    // Warn once a comfortable stop is no longer possible and the driver is not already
    // braking as hard as the stop needs.
    if need < p.rlvw_decel_mps2 || -ego.accel_mps2 >= 0.9 * v * v / (2.0 * d) {
        return None;
    }
    Some(Surrogates {
        ttc_s: t_arrive,
        pet_s: f64::NAN,
        required_decel_mps2: need,
        distance_m: d,
        closing_mps: v,
    })
}

/// GLOSA: the speed that reaches the stop line on green, within `[min, limit]`.
pub fn glosa(
    ego: &Track,
    sit: &SignalSituation,
    limit_mps: f64,
    p: &SignalAppParams,
) -> Option<SpeedAdvice> {
    let d = sit.distance_m;
    if d <= 5.0 || d > p.glosa_range_m || !sit.time_left_s.is_finite() {
        return None;
    }
    let v = ego.speed_mps.max(0.1);
    let t_left = sit.time_left_s;
    if is_go(sit.state) {
        // Green: can the ego clear before it ends, at up to the limit?
        let t_need = d / limit_mps;
        if t_need <= t_left - p.glosa_margin_s {
            let needed = d / (t_left - p.glosa_margin_s).max(0.1);
            let target = if d / v <= t_left - p.glosa_margin_s {
                v.min(limit_mps)
            } else {
                needed.min(limit_mps)
            };
            return Some(SpeedAdvice {
                target_mps: Some(target.max(p.glosa_min_speed_mps.min(limit_mps))),
                distance_m: d,
            });
        }
        // It cannot make this green: it will stop.
        return Some(SpeedAdvice {
            target_mps: None,
            distance_m: d,
        });
    }
    if is_red(sit.state) || sit.state == MovementPhaseState::PreMovement {
        // Red: arrive as the green starts.
        let target = d / (t_left + p.glosa_margin_s).max(0.1);
        if target > limit_mps {
            return Some(SpeedAdvice {
                target_mps: Some(limit_mps),
                distance_m: d,
            });
        }
        if target >= p.glosa_min_speed_mps {
            return Some(SpeedAdvice {
                target_mps: Some(target),
                distance_m: d,
            });
        }
        return Some(SpeedAdvice {
            target_mps: None,
            distance_m: d,
        });
    }
    None
}

/// Seconds since the top of the UTC hour at `t` on `wall`.
pub fn seconds_into_hour(wall: WallClock, t: SimTime) -> f64 {
    let ns = wall.unix_nanos_at(t);
    let hour = 3_600_i128 * 1_000_000_000;
    (ns.rem_euclid(hour)) as f64 * 1e-9
}

/// The ego projected onto an ingress lane extended upstream to `range`: distance to the
/// stop line along the lane, lateral offset, and the lane's direction of travel.
fn project_ingress(points: &[Vec3], p: Vec3, range: f64) -> Option<(f64, f64, f64)> {
    // points[0] is the stop line; the lane is driven from points[n-1] towards points[0].
    let mut pts: Vec<Vec3> = points.to_vec();
    // Extend the far end upstream so the approach is covered to the advisory range.
    let n = pts.len();
    let (a, b) = (pts[n - 2], pts[n - 1]);
    let seg = a.distance_2d(b).max(1e-6);
    let covered: f64 = pts.windows(2).map(|w| w[0].distance_2d(w[1])).sum();
    if covered < range {
        let ext = range - covered;
        pts.push(Vec3::new(
            b.x + (b.x - a.x) / seg * ext,
            b.y + (b.y - a.y) / seg * ext,
            b.z,
        ));
    }
    let mut best: Option<(f64, f64, f64, f64)> = None; // (|lat|, along, lat, dir)
    let mut along0 = 0.0;
    for w in pts.windows(2) {
        let (s0, s1) = (w[0], w[1]);
        let (dx, dy) = (s1.x - s0.x, s1.y - s0.y);
        let len = math::hypot(dx, dy);
        if len < 1e-6 {
            continue;
        }
        let u = (((p.x - s0.x) * dx + (p.y - s0.y) * dy) / (len * len)).clamp(0.0, 1.0);
        let (cx, cy) = (s0.x + u * dx, s0.y + u * dy);
        // Travel runs from s1 to s0; left of travel is positive.
        let (tx, ty) = (-dx / len, -dy / len);
        let lat = -(p.x - cx) * ty + (p.y - cy) * tx;
        let dist = along0 + u * len;
        if best.is_none_or(|(l, ..)| lat.abs() < l) {
            best = Some((lat.abs(), dist, lat, math::atan2(ty, tx)));
        }
        along0 += len;
    }
    best.map(|(_, d, l, dir)| (d, l, dir))
}

/// The signal group of the ego's movement from `lane`: the connection whose turn matches
/// the ego's intent, else the straight one, else the first.
fn pick_group(
    lane: &Ingress,
    egress: &BTreeMap<u8, (Vec3, Vec3)>,
    turn: Option<TurnIntent>,
) -> Option<u8> {
    let (s0, s1) = (lane.points[0], lane.points[1]);
    let travel = math::atan2(s0.y - s1.y, s0.x - s1.x);
    let classify = |to: u8| -> Option<TurnIntent> {
        let (e0, e1) = egress.get(&to)?;
        let out = math::atan2(e1.y - e0.y, e1.x - e0.x);
        let d = v2xw_msg::j2945::wrap_pi(out - travel);
        Some(if d > deg(45.0) {
            TurnIntent::Left
        } else if d < -deg(45.0) {
            TurnIntent::Right
        } else {
            TurnIntent::Straight
        })
    };
    let want = turn.unwrap_or(TurnIntent::Straight);
    lane.connections
        .iter()
        .find(|(to, g)| g.is_some() && classify(*to) == Some(want))
        .or_else(|| {
            lane.connections
                .iter()
                .find(|(to, g)| g.is_some() && classify(*to) == Some(TurnIntent::Straight))
        })
        .or_else(|| lane.connections.iter().find(|(_, g)| g.is_some()))
        .and_then(|(_, g)| *g)
}

/// Fills a track's acceleration, yaw rate, size and hard-braking flag from a BSM's or a
/// CAM's own octets.
fn decode_dynamics(ty: MsgType, payload: &[u8], t: &mut Track) {
    match ty {
        MsgType::Bsm => {
            let Ok(b) = v2xw_msg::j2735::bsm::decode_message_frame(payload) else {
                return;
            };
            let a = b.core.accel_set;
            if a.long != v2xw_msg::j2735::bsm::ACCELERATION_UNAVAILABLE {
                t.accel_mps2 = f64::from(a.long) * 0.01;
            }
            t.yaw_rate_rad_s = f64::from(a.yaw) * 0.01 * core::f64::consts::PI / 180.0;
            if b.core.size.length > 0 {
                t.length_m = f64::from(b.core.size.length) * 0.01;
            }
            if b.core.size.width > 0 {
                t.width_m = f64::from(b.core.size.width) * 0.01;
            }
            t.hard_braking = t.accel_mps2 <= -crate::safety::EEBL_DECEL_THRESHOLD_MPS2;
            for c in &b.part_ii {
                if let v2xw_msg::j2735::bsm::PartIIValue::VehicleSafety(ext) = &c.value
                    && let Some(ev) = ext.events
                    && ev.contains(v2xw_msg::j2735::bsm::VehicleEventFlags::HARD_BRAKING)
                {
                    t.hard_braking = true;
                }
            }
        }
        MsgType::Cam => {
            let Ok(cam) = v2xw_msg::cam::decode_cam(payload) else {
                return;
            };
            if let v2xw_msg::asn1::cam_asn1::HighFrequencyContainer::basicVehicleContainerHighFrequency(hf) =
                &cam.cam.cam_parameters.high_frequency_container
            {
                let a = &hf.longitudinal_acceleration.value.0;
                // `AccelerationValue`, 0.1 m/s², 161 unavailable.
                if *a != 161 {
                    t.accel_mps2 = f64::from(*a) * 0.1;
                }
                let yaw = hf.yaw_rate.yaw_rate_value.0;
                if yaw != 32_767 {
                    t.yaw_rate_rad_s = f64::from(yaw) * 0.01 * core::f64::consts::PI / 180.0;
                }
                t.length_m = f64::from(hf.vehicle_length.vehicle_length_value.0) * 0.1;
                t.width_m = f64::from(hf.vehicle_width.0) * 0.1;
                t.hard_braking = t.accel_mps2 <= -crate::safety::EEBL_DECEL_THRESHOLD_MPS2;
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn record(
    node: NodeId,
    t: SimTime,
    app: &'static str,
    subject: &HashedId8,
    kind: WarningKind,
    severity: Severity,
    s: Surrogates,
    via: Option<&'static str>,
) -> crate::safety::WarningRecord {
    let s = s.quantised();
    crate::safety::WarningRecord {
        t,
        node,
        app,
        subject: digest_hex(subject),
        fired: kind != WarningKind::Clear,
        kind: kind.as_str(),
        severity: severity.as_str(),
        ttc_s: s.ttc_s,
        pet_s: s.pet_s,
        required_decel_mps2: s.required_decel_mps2,
        distance_m: s.distance_m,
        closing_mps: s.closing_mps,
        via,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car(x: f64, y: f64, heading_deg: f64, v: f64) -> Track {
        Track {
            pos: Vec3::new(x, y, 0.0),
            heading_rad: deg(heading_deg),
            speed_mps: v,
            accel_mps2: 0.0,
            yaw_rate_rad_s: 0.0,
            length_m: 4.5,
            width_m: 1.8,
            hard_braking: false,
            vru: false,
        }
    }

    /// NHTSA's decelerating-lead case: 13.4 m/s both, 30 m apart, the lead braking at
    /// 0.3 g. FCW must fire by TTC 2.4 s, and does not fire while the lead holds speed.
    #[test]
    fn fcw_fires_for_a_braking_lead_in_the_path_and_not_otherwise() {
        let ego = car(0.0, 0.0, 0.0, 13.4);
        let mut lead = car(30.0, 0.0, 0.0, 13.4);
        assert!(fcw(&ego, &lead, &FcwParams::default()).is_none());
        lead.accel_mps2 = -0.3 * 9.81;
        lead.speed_mps = 6.0;
        let s = fcw(&ego, &lead, &FcwParams::default()).expect("warns");
        assert!(s.ttc_s <= 2.4, "{s:?}");
        // One lane over, no.
        let mut beside = lead;
        beside.pos.y = 3.6;
        assert!(fcw(&ego, &beside, &FcwParams::default()).is_none());
        // Oncoming, no.
        let mut oncoming = lead;
        oncoming.heading_rad = deg(180.0);
        assert!(fcw(&ego, &oncoming, &FcwParams::default()).is_none());
    }

    #[test]
    fn eebl_fires_for_hard_braking_ahead_only() {
        let ego = car(0.0, 0.0, 0.0, 13.0);
        let mut ahead = car(120.0, 3.5, 0.0, 13.0);
        assert!(eebl(&ego, &ahead, &EeblParams::default()).is_none());
        ahead.hard_braking = true;
        assert!(eebl(&ego, &ahead, &EeblParams::default()).is_some());
        let mut behind = ahead;
        behind.pos.x = -50.0;
        assert!(eebl(&ego, &behind, &EeblParams::default()).is_none());
    }

    #[test]
    fn ima_fires_when_both_reach_the_crossing_together() {
        let ego = car(-30.0, 0.0, 0.0, 10.0);
        let cross = car(0.0, -32.0, 90.0, 10.0);
        let s = ima(&ego, &cross, &ImaParams::default()).expect("conflict");
        assert!(s.pet_s < 0.5);
        let late = car(0.0, -80.0, 90.0, 10.0);
        assert!(ima(&ego, &late, &ImaParams::default()).is_none());
    }

    #[test]
    fn lta_fires_inside_the_critical_gap() {
        let ego = car(0.0, -10.0, 90.0, 3.0);
        let junction = Vec3::new(0.0, 0.0, 0.0);
        let near = car(1.8, 30.0, 270.0, 12.0); // 2.5 s out
        assert!(lta(&ego, junction, 10.0, &near, &LtaParams::default()).is_some());
        let far = car(1.8, 120.0, 270.0, 12.0); // 10 s out
        assert!(lta(&ego, junction, 10.0, &far, &LtaParams::default()).is_none());
    }

    #[test]
    fn a_car_beside_the_rear_quarter_is_in_the_blind_spot() {
        let ego = car(0.0, 0.0, 0.0, 12.0);
        let beside = car(-3.0, 3.5, 0.0, 12.0);
        let (left, _) = blind_spot(&ego, &beside, &BswParams::default()).expect("in zone");
        assert!(left);
        let far_back = car(-15.0, 3.5, 0.0, 12.0);
        assert!(blind_spot(&ego, &far_back, &BswParams::default()).is_none());
        let two_lanes = car(-3.0, 7.2, 0.0, 12.0);
        assert!(blind_spot(&ego, &two_lanes, &BswParams::default()).is_none());
    }

    #[test]
    fn pcw_fires_for_a_pedestrian_stepping_into_the_path() {
        let ego = car(0.0, 0.0, 0.0, 10.0);
        let mut ped = car(20.0, -3.0, 90.0, 1.4);
        ped.vru = true;
        ped.length_m = 0.5;
        ped.width_m = 0.5;
        assert!(pcw(&ego, &ped, &PcwParams::default()).is_some());
        let mut away = ped;
        away.heading_rad = deg(-90.0);
        assert!(pcw(&ego, &away, &PcwParams::default()).is_none());
    }

    #[test]
    fn rlvw_and_glosa_read_the_signal() {
        let p = SignalAppParams::default();
        let ego = car(0.0, 0.0, 0.0, 12.0);
        // 25 m from a red that lasts 10 s more, not braking: warn.
        let red = SignalSituation {
            intersection: 1,
            signal_group: 2,
            distance_m: 25.0,
            state: MovementPhaseState::StopAndRemain,
            time_left_s: 10.0,
        };
        assert!(rlvw(&ego, &red, &p).is_some());
        // Braking as needed: no warning.
        let mut braking = ego;
        braking.accel_mps2 = -3.2;
        assert!(rlvw(&braking, &red, &p).is_none());
        // 200 m out from the same red: GLOSA advises arriving at the green.
        let far = SignalSituation {
            distance_m: 200.0,
            time_left_s: 20.0,
            ..red
        };
        let a = glosa(&ego, &far, 13.4, &p).expect("advice");
        let v = a.target_mps.expect("a speed");
        assert!((v - 200.0 / 21.0).abs() < 1e-9, "{v}");
        // On a green it can clear, it keeps its speed.
        let green = SignalSituation {
            state: MovementPhaseState::ProtectedMovementAllowed,
            distance_m: 60.0,
            time_left_s: 15.0,
            ..red
        };
        assert_eq!(glosa(&ego, &green, 13.4, &p).and_then(|a| a.target_mps), Some(12.0));
    }

    #[test]
    fn a_stale_message_is_extrapolated_and_never_reversed() {
        let mut t = car(0.0, 0.0, 0.0, 10.0);
        t.accel_mps2 = -5.0;
        let later = t.advanced(4.0);
        assert!((later.pos.x - 10.0).abs() < 1e-9, "{:?}", later.pos);
        assert_eq!(later.speed_mps, 0.0);
    }
}
