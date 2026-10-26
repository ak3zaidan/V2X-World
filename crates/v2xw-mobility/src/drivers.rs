//! Human drivers: per-vehicle parameter spread, reaction time and perception errors.
//!
//! The car-following models are *one* driver each: every car of a class keeps the same
//! time gap `T`, accelerates at the same `a`, brakes comfortably at the same `b` and stops
//! the same `s0` behind. [`crate::style`] already sorts drivers into three styles read from
//! the free-speed draw; this module adds what makes a *real* fleet unstable where an ideal
//! one is not, each switchable on its own:
//!
//! # 1. Parameter spread
//!
//! Each driver's `T`, `a`, `b` and `s0` are the class's values times a mean-one lognormal
//! factor, `exp(σz − σ²/2)` with `σ² = ln(1 + cv²)`, so the fleet's mean — and with it the
//! HCM saturation flow the presets were calibrated to — is kept while the spread is that of
//! drivers fitted one by one. The coefficients of variation per class:
//!
//! | class | cv `T` | cv `a` | cv `b` | cv `s0` | source |
//! |---|---|---|---|---|---|
//! | car | 0.25 | 0.25 | 0.20 | 0.25 | the spread of IDM parameters fitted to individual drivers' trajectories: Kesting & Treiber 2008 (*TRR* 2088, Table 2, 10 drivers; **secondary**, the order of magnitude), and per-driver IDM fits to NGSIM I-80 / highD trajectories (Punzo, Montanino & Ciuffo 2015, *Transp. Sci.* 49(4); Sharma, Zheng & Bhaskar 2019, *Transp. Res. Rec.*), which report inter-driver cvs of 0.2–0.4 for `T` and `a`; **0.25 is a choice** inside that range |
//! | truck, trailer, coach | 0.20 | 0.20 | 0.15 | 0.20 | **a choice**: professional drivers of governed vehicles spread less (the same fits, truck subsets) |
//! | bus | 0.15 | 0.15 | 0.15 | 0.15 | **a choice**: operator training and a schedule |
//! | motorcycle, moped | 0.30 | 0.30 | 0.25 | 0.30 | **a choice**: the widest of the motorised classes |
//!
//! `a` and `b` share half their variation (correlation 0.5): a driver who accelerates hard
//! brakes hard (Evans & Wasielewski 1983, *AA&P* 15(2): the direction of effect; the 0.5 is
//! **a choice**). The factors are clamped to `[0.5, 2]`. `parameter_spread` scales every cv
//! (0 turns the spread off, 1 is the table, 2 doubles it).
//!
//! # 2. Reaction time
//!
//! The IDM reacts in the same step it perceives. A human acts on what they saw `T'` ago
//! (Treiber, Kesting & Helbing 2006, *Physica A* 360, 71–88, the Human Driver Model: a
//! delay `T'` of the order of the time gap makes homogeneous traffic break into stop-and-go
//! waves at densities where the ODE model is string stable). The car-following
//! acceleration a driver applies is the one they decided `T'` before — except in an
//! **alarm**, when what they see now asks for harder braking than their comfortable
//! deceleration `b`: then they brake at once, as Wiedemann's perception thresholds have a
//! closing driver do (Wiedemann 1974; **a choice** of threshold). Brake reaction times in
//! car following are 0.7–0.75 s for an expected event and about 1.25 s for an unexpected
//! one (Green 2000, *Transportation Human Factors* 2(3), 195–216).
//!
//! # 3. Perception errors
//!
//! Drivers misjudge the gap and the closing speed, and the error persists: the HDM's
//! Wiener-process estimation errors (Treiber et al. 2006, §2.2). The perceived gap is
//! `s·exp(V_s·w_s)` and the perceived approach rate `Δv + s·σ_r·w_r`, with `w` two
//! independent Ornstein-Uhlenbeck processes of unit variance and correlation time `τ̃`.
//! The paper's values: `V_s = 0.05` (5 % relative distance error), `σ_r = 0.01 s⁻¹`
//! (inverse time-to-collision error), `τ̃ = 20 s`. The noise is keyed by the driver and the
//! step index ([`std_normal`]), so a run is reproducible and one driver's errors do not
//! depend on anyone else's.
//!
//! # 4. Heavy vehicles out of the left lane
//!
//! Where a carriageway has three or more lanes, many jurisdictions keep heavy goods
//! vehicles out of the leftmost one: Germany's StVO §18(11) (lorries over 7.5 t on a
//! motorway), and the left-lane truck restrictions of US states (FHWA, *Synthesis of
//! Truck Lane Restriction Studies*, 2004: 26 states). With `truck_lane_restriction` a
//! truck or a trailer on a motorway or trunk edge of three or more lanes makes no
//! discretionary change into its leftmost lane.
//!
//! # 5. Rule violations and crash states
//!
//! [`ViolationParams`], off by default: red-light running after the red onset, amber
//! running beyond the dilemma zone, failure to yield to a slow foe, and collisions that
//! become crash states (stopped, lane blocked, cleared after a duration). The red-light
//! numbers are fitted to Bonneson, Zimmerman & Quiroga, *An Engineer's Guide to Reducing
//! Red-Light-Related Crashes* (TTI 0-4196-P1): 1.0 violation per 10,000 vehicle-cycles
//! (3-6 per 1,000 vehicles), median entry under 0.5 s into red, 98 % within 4.0 s. The
//! failure-to-yield semantics follow SUMO's documented `jmIgnoreFoeProb` /
//! `jmIgnoreFoeSpeed`; re-implemented from the documentation and the field data — SUMO was
//! read for behaviour only and no code was copied. Every violation committed on purpose is
//! tagged ([`crate::audit::intent`]) so the auditor tells it from a model defect.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use v2xw_core::math;

use crate::classes::VehicleClass;
use crate::views::DriverProfile;

/// The HDM's relative distance error `V_s` (Treiber, Kesting & Helbing 2006, Table 1).
pub const HDM_DISTANCE_ERROR: f64 = 0.05;
/// The HDM's inverse time-to-collision error `σ_r`, s⁻¹ (ibid.).
pub const HDM_RATE_ERROR_PER_S: f64 = 0.01;
/// The HDM's estimation-error correlation time `τ̃`, seconds (ibid.).
pub const HDM_ERROR_CORRELATION_S: f64 = 20.0;
/// The reaction time of the `human` preset, seconds: Green 2000's expected-event brake
/// reaction (0.7–0.75 s), rounded up to the engine's step.
pub const HUMAN_REACTION_S: f64 = 0.8;
/// The longest reaction time accepted, seconds: Green 2000's surprise case (1.5 s) and a
/// margin.
pub const MAX_REACTION_S: f64 = 2.0;

/// How human the drivers are. Every field off (zero) is the engine without this module.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HumanDriverParams {
    /// The multiplier on the per-class coefficients of variation of `T`, `a`, `b`, `s0`.
    pub parameter_spread: f64,
    /// The reaction time `T'`, seconds; 0 reacts in the step it perceives.
    pub reaction_time_s: f64,
    /// The relative distance error `V_s`; 0 perceives the gap exactly.
    pub distance_error: f64,
    /// The inverse time-to-collision error `σ_r`, s⁻¹; 0 perceives the closing speed exactly.
    pub rate_error_per_s: f64,
    /// The estimation errors' correlation time `τ̃`, seconds.
    pub error_correlation_s: f64,
    /// Whether trucks and trailers keep out of the leftmost lane of a motorway or trunk
    /// carriageway of three or more lanes.
    pub truck_lane_restriction: bool,
}

impl Default for HumanDriverParams {
    fn default() -> Self {
        Self {
            parameter_spread: 0.0,
            reaction_time_s: 0.0,
            distance_error: 0.0,
            rate_error_per_s: 0.0,
            error_correlation_s: HDM_ERROR_CORRELATION_S,
            truck_lane_restriction: false,
        }
    }
}

impl HumanDriverParams {
    /// Whether the reaction delay or the perception errors act.
    pub fn imperfect(&self) -> bool {
        self.reaction_time_s > 0.0 || self.distance_error > 0.0 || self.rate_error_per_s > 0.0
    }

    /// Every driver its own parameters, no delay, no errors.
    pub fn heterogeneous() -> Self {
        Self {
            parameter_spread: 1.0,
            ..Self::default()
        }
    }

    /// The Human Driver Model: spread, [`HUMAN_REACTION_S`] and the HDM's errors.
    pub fn human() -> Self {
        Self {
            parameter_spread: 1.0,
            reaction_time_s: HUMAN_REACTION_S,
            distance_error: HDM_DISTANCE_ERROR,
            rate_error_per_s: HDM_RATE_ERROR_PER_S,
            error_correlation_s: HDM_ERROR_CORRELATION_S,
            truck_lane_restriction: false,
        }
    }
}

/// The per-class coefficients of variation `(T, a, b, s0)` (module table).
pub fn spread_cv(class: VehicleClass) -> [f64; 4] {
    match class {
        VehicleClass::Truck | VehicleClass::Trailer | VehicleClass::Coach => {
            [0.20, 0.20, 0.15, 0.20]
        }
        VehicleClass::Bus => [0.15, 0.15, 0.15, 0.15],
        VehicleClass::Motorcycle | VehicleClass::Moped => [0.30, 0.30, 0.25, 0.30],
        _ => [0.25, 0.25, 0.20, 0.25],
    }
}

/// A mean-one lognormal factor for coefficient of variation `cv` at standard normal `z`,
/// clamped to `[0.5, 2]`.
pub fn lognormal_factor(cv: f64, z: f64) -> f64 {
    if cv <= 0.0 {
        return 1.0;
    }
    let s2 = math::ln(1.0 + cv * cv);
    math::exp(math::sqrt(s2) * z - 0.5 * s2).clamp(0.5, 2.0)
}

/// `d` with its own `T`, `a`, `b` and `s0`, from four independent standard normals `z`
/// (`a` and `b` share half their variation), scaled by `spread`.
pub fn spread_profile(
    d: DriverProfile,
    class: VehicleClass,
    z: [f64; 4],
    spread: f64,
) -> DriverProfile {
    if spread <= 0.0 {
        return d;
    }
    let cv = spread_cv(class).map(|c| c * spread);
    // Correlation 0.5 between the acceleration and the braking draws.
    let z_b = 0.5 * z[1] + math::sqrt(0.75) * z[2];
    DriverProfile {
        time_headway_s: d.time_headway_s * lognormal_factor(cv[0], z[0]),
        max_accel_mps2: d.max_accel_mps2 * lognormal_factor(cv[1], z[1]),
        comfort_decel_mps2: d.comfort_decel_mps2 * lognormal_factor(cv[2], z_b),
        min_gap_m: d.min_gap_m * lognormal_factor(cv[3], z[3]),
        ..d
    }
}

/// SplitMix64 (Steele, Lea & Flood 2014): the mixing step that turns a driver's key and a
/// step index into an independent 64-bit word.
pub const fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A standard normal keyed by `(key, step, slot)`: Box–Muller over two SplitMix64 words,
/// through `v2xw_core::math` so it is bit-identical on every platform.
pub fn std_normal(key: u64, step: u64, slot: u64) -> f64 {
    let h1 = splitmix64(key ^ splitmix64(step.wrapping_mul(4).wrapping_add(slot)));
    let h2 = splitmix64(h1 ^ 0xD1B5_4A32_D192_ED03);
    let u1 = ((h1 >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
    let u2 = (h2 >> 11) as f64 / (1u64 << 53) as f64;
    math::sqrt(-2.0 * math::ln(u1)) * math::cos(2.0 * core::f64::consts::PI * u2)
}

/// One driver's imperfection state, carried from step to step.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HumanState {
    /// The distance-error process `w_s` (unit variance).
    pub w_gap: f64,
    /// The rate-error process `w_r` (unit variance).
    pub w_rate: f64,
    /// The car-following accelerations decided over the last `T'`, oldest first, m/s².
    pub decided: VecDeque<f64>,
}

/// What one step of perception gives: the perceived leader and the next error states.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Perceived {
    /// The perceived net gap, metres.
    pub gap_m: f64,
    /// The perceived leader speed, m/s.
    pub leader_speed_mps: f64,
    /// The next `w_s`.
    pub w_gap: f64,
    /// The next `w_r`.
    pub w_rate: f64,
}

/// Advances the driver's estimation errors one step and returns the leader as perceived.
///
/// The Ornstein-Uhlenbeck update `w ← w·e^{−dt/τ̃} + √(1 − e^{−2dt/τ̃})·η` keeps the
/// stationary variance at one whatever the step (the exact discretisation of the HDM's
/// Wiener process, Treiber et al. 2006 Eq. 9).
#[allow(clippy::too_many_arguments)]
pub fn perceive(
    p: &HumanDriverParams,
    s: &HumanState,
    key: u64,
    step: u64,
    dt_s: f64,
    gap_m: f64,
    ego_speed_mps: f64,
    leader_speed_mps: f64,
) -> Perceived {
    let tau = p.error_correlation_s.max(dt_s);
    let e = math::exp(-dt_s / tau);
    let k = math::sqrt((1.0 - e * e).max(0.0));
    let w_gap = s.w_gap * e + k * std_normal(key, step, 0);
    let w_rate = s.w_rate * e + k * std_normal(key, step, 1);
    let gap = gap_m.max(0.0);
    let gap_est = gap * math::exp(p.distance_error * w_gap);
    // Δv_est = Δv + s·σ_r·w_r, with Δv = v − v_l: the leader seen that much slower.
    let dv_est = (ego_speed_mps - leader_speed_mps) + gap * p.rate_error_per_s * w_rate;
    Perceived {
        gap_m: gap_est,
        leader_speed_mps: (ego_speed_mps - dv_est).max(0.0),
        w_gap,
        w_rate,
    }
}

/// The car-following acceleration a driver with reaction time `reaction_s` applies now,
/// given the one they decide now (`decided_now`) and their history; braking harder than
/// `comfort_decel_mps2` is an alarm and acts at once.
pub fn delayed(
    s: &HumanState,
    reaction_s: f64,
    dt_s: f64,
    decided_now: f64,
    comfort_decel_mps2: f64,
) -> f64 {
    let n = delay_steps(reaction_s, dt_s);
    let applied = if n == 0 {
        decided_now
    } else if s.decided.len() >= n {
        s.decided[s.decided.len() - n]
    } else {
        // Not yet seen long enough: the oldest decision there is (a new vehicle starts as
        // it was placed, at its first decision).
        s.decided.front().copied().unwrap_or(decided_now)
    };
    if decided_now < -comfort_decel_mps2.max(0.5) {
        applied.min(decided_now)
    } else {
        applied
    }
}

/// How many steps of `dt_s` the reaction time is.
pub fn delay_steps(reaction_s: f64, dt_s: f64) -> usize {
    if reaction_s <= 0.0 || dt_s <= 0.0 {
        0
    } else {
        (reaction_s / dt_s).round() as usize
    }
}

/// Records this step's decision in `s`, keeping only the last `T'` of them.
pub fn remember(s: &mut HumanState, reaction_s: f64, dt_s: f64, decided_now: f64) {
    let n = delay_steps(reaction_s, dt_s);
    if n == 0 {
        s.decided.clear();
        return;
    }
    s.decided.push_back(decided_now);
    while s.decided.len() > n {
        s.decided.pop_front();
    }
}

/// The lowest posted limit read as a freeway's where the world gives no road class, m/s:
/// 55 mph, the bottom of the range US limited-access highways are posted at, against
/// 25-45 mph on urban arterials (FHWA, Methods and Practices for Setting Speed Limits,
/// FHWA-SA-12-004, 2012). A procedural world tags every edge `Residential`, so without
/// this the six-lane procedural freeway never had a restricted lane.
pub const FREEWAY_MIN_LIMIT_MPS: f64 = 24.587;

/// Whether `lane` is the leftmost driving lane of a motorway or trunk edge with three or
/// more driving lanes: the lane a restricted truck may not change into. An edge of a
/// world without road classes (procedural, tagged `Residential`) counts as a motorway
/// when its limit is a freeway's ([`FREEWAY_MIN_LIMIT_MPS`]).
pub fn is_restricted_left_lane(world: &v2xw_world::World, lane: v2xw_core::ids::LaneId) -> bool {
    use v2xw_world::{LaneKind, RoadClass};
    let Some(l) = world.try_lane(lane) else {
        return false;
    };
    if l.kind != LaneKind::Driving {
        return false;
    }
    let edge = world.edge(l.edge);
    let freeway = match edge.road_class {
        RoadClass::Motorway | RoadClass::Trunk => true,
        RoadClass::Residential => l.speed_limit_mps >= FREEWAY_MIN_LIMIT_MPS,
        _ => false,
    };
    if !freeway {
        return false;
    }
    let driving: Vec<u8> = edge
        .lanes
        .iter()
        .filter_map(|id| world.try_lane(*id))
        .filter(|x| x.kind == LaneKind::Driving)
        .map(|x| x.index)
        .collect();
    driving.len() >= 3 && driving.iter().max() == Some(&l.index)
}

// ---------------------------------------------------------------------------
// Rule violations and crash states (WP-08)
// ---------------------------------------------------------------------------

/// Bonneson et al.'s red-light violation rate at Texas urban signals: 1.0 violation per
/// 10,000 vehicle-cycles (Bonneson, Zimmerman & Quiroga, *An Engineer's Guide to
/// Reducing Red-Light-Related Crashes*, TTI 0-4196-P1, Table 1: Texas, 20 approaches;
/// direct observation, every entry after the red onset counted).
pub const RED_RUN_PER_VEH_CYCLE: f64 = 1.0e-4;

/// The field band of red-light violations per 1,000 vehicles entering, read from the
/// same Table 1: Iowa 3.0, Texas 4.1, United Kingdom 5.3, and the guide's statement that
/// 1.0 per 10,000 vehicle-cycles "equates to 3 to 6 violations per 1000 vehicles for
/// cycle lengths in the range of 60 to 120 s".
pub const RED_RUN_PER_1000_VEH_BAND: (f64, f64) = (3.0, 6.0);

/// The mean of a red-light runner's tolerance, seconds: how late after the red onset they
/// still enter, exponential across runners. The guide (Figure 4, 541 phases with a
/// through vehicle entering on red) puts the median entry under 0.5 s into red; with
/// arrivals uniform in time an exponential tolerance gives an exponential entry time, and
/// 0.6 s gives a median of 0.6 ln 2 = 0.42 s. **Calibrated** to that median.
pub const RED_RUN_TOLERANCE_MEAN_S: f64 = 0.6;

/// The share of drivers who will enter on red within their tolerance, by default. With
/// arrivals uniform over a cycle of length `C`, a share `p` with mean tolerance `μ` makes
/// `p·μ/C` violations per vehicle, so `p·μ = 3600 s · 1.0e-4 = 0.36 s` reproduces
/// [`RED_RUN_PER_VEH_CYCLE`]: `0.36 / 0.6 = 0.6`. **Calibrated**, not measured; the
/// guide's driver survey (Porter & Berry, its ref. 15) says most early-red entries are
/// intentional, which is what a per-driver propensity models.
pub const RED_RUN_SHARE: f64 = 0.6;

/// The longest tolerance drawn, seconds: the guide's 98 % of violations within 4.0 s of
/// the red onset (Table 3).
pub const RED_RUN_TOLERANCE_MAX_S: f64 = 4.0;

/// How fast a foe may be moving for a driver who fails to yield to ignore it, m/s: SUMO's
/// `jmIgnoreFoeSpeed` semantics (only foes at or below this speed are ignored; SUMO's
/// documented default is 0). **A choice**: a vehicle still rolling up to its own line,
/// which a failing driver takes to be stopping.
pub const IGNORE_FOE_SPEED_MPS: f64 = 5.0;

/// How long a crashed vehicle stands in its lane before it is cleared, seconds. **A
/// choice, todo-calibrate**: the order of a minor-crash clearance in incident-management
/// practice (a quarter of an hour); no field distribution was read.
pub const CRASH_CLEARANCE_S: f64 = 900.0;

/// The rule violations the drivers commit, and what a collision turns into
/// (`actors.vehicles.drivers.violations`). Off by default: with `enabled` false no draw is
/// made and nothing in a run changes.
///
/// * **Red-light running after the red onset** (SUMO's `jmDriveAfterRedTime`, re-
///   implemented from the field data): a share [`RED_RUN_SHARE`] of drivers each carry a
///   tolerance, exponential with mean [`RED_RUN_TOLERANCE_MEAN_S`] and capped at
///   [`RED_RUN_TOLERANCE_MAX_S`]. At the amber onset they go if, at their present speed,
///   they reach the line no later than their tolerance after the red; seeing a red first,
///   they go if the red began no longer ago than their tolerance less the time to the line.
/// * **Amber running beyond the dilemma zone**: each driver's amber threshold is the
///   Zegeer & Deen logistic without the clamp at its far edge (5.5 s), up to
///   `amber_tail_max_s`: the logistic's own tail of drivers who go from further back.
/// * **Failure to yield**: a share of drivers, at each unsignalised junction or permissive
///   green they reach (a keyed draw per driver and junction), ignore conflicting foes moving
///   at or below `ignore_foe_speed_mps` (SUMO `jmIgnoreFoeProb` / `jmIgnoreFoeSpeed`).
/// * **Crash states**: with `crashes` on, two vehicles whose footprints intersect stop where
///   they are, block their lanes, and are removed after `crash_clearance_s` with
///   [`crate::DespawnCause::Collision`].
///
/// Every violation the engine commits on purpose is **tagged** on the actor
/// ([`crate::audit::intent`]), so the traffic auditor counts it as behaviour and still
/// flags an untagged one as a model defect.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ViolationParams {
    /// Whether any of this acts.
    pub enabled: bool,
    /// The share of drivers who run reds within their tolerance.
    pub red_run_share: f64,
    /// The mean of a runner's tolerance after the red onset, seconds.
    pub red_run_tolerance_mean_s: f64,
    /// The longest tolerance, seconds.
    pub red_run_tolerance_max_s: f64,
    /// The longest amber threshold drawn, seconds; the dilemma zone's far edge (5.5 s)
    /// keeps the clamp, so nobody goes from beyond it.
    pub amber_tail_max_s: f64,
    /// The probability that a driver fails to yield at a junction where they must give way.
    pub fail_to_yield_share: f64,
    /// Foes at or below this speed may be ignored by a driver who fails to yield, m/s.
    pub ignore_foe_speed_mps: f64,
    /// Whether a collision resolves to a crash state.
    pub crashes: bool,
    /// How long a crashed vehicle stands before it is cleared, seconds.
    pub crash_clearance_s: f64,
}

impl Default for ViolationParams {
    fn default() -> Self {
        Self {
            enabled: false,
            red_run_share: RED_RUN_SHARE,
            red_run_tolerance_mean_s: RED_RUN_TOLERANCE_MEAN_S,
            red_run_tolerance_max_s: RED_RUN_TOLERANCE_MAX_S,
            amber_tail_max_s: 8.0,
            fail_to_yield_share: 0.0,
            ignore_foe_speed_mps: IGNORE_FOE_SPEED_MPS,
            crashes: true,
            crash_clearance_s: CRASH_CLEARANCE_S,
        }
    }
}

impl ViolationParams {
    /// Whether this is the default (off) block, which serialises to nothing.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The red-light tolerance of a driver from two uniforms: whether they run at all
    /// (`u_share` below the share) and how late (`u_tol`, inverse exponential CDF), seconds;
    /// 0 for a driver who never enters on red.
    pub fn red_tolerance_s(&self, u_share: f64, u_tol: f64) -> f64 {
        if !self.enabled || u_share >= self.red_run_share || self.red_run_tolerance_mean_s <= 0.0
        {
            return 0.0;
        }
        let u = u_tol.clamp(1e-12, 1.0 - 1e-12);
        (-self.red_run_tolerance_mean_s * math::ln(1.0 - u))
            .min(self.red_run_tolerance_max_s.max(0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_red_tolerance_is_exponential_for_the_share_and_zero_otherwise() {
        let p = ViolationParams {
            enabled: true,
            ..ViolationParams::default()
        };
        let n = 20_000;
        let mut runners = 0;
        let mut tol = Vec::new();
        for i in 0..n {
            let u1 = (i as f64 + 0.5) / n as f64;
            let u2 = ((i * 7919) % n) as f64 / n as f64 + 0.5 / n as f64;
            let t = p.red_tolerance_s(u1, u2);
            if t > 0.0 {
                runners += 1;
                tol.push(t);
            }
        }
        let share = runners as f64 / n as f64;
        assert!((share - RED_RUN_SHARE).abs() < 0.01, "share {share}");
        tol.sort_by(f64::total_cmp);
        let median = tol[tol.len() / 2];
        assert!(
            (median - RED_RUN_TOLERANCE_MEAN_S * math::ln(2.0)).abs() < 0.02,
            "median {median}"
        );
        assert!(tol.iter().all(|t| *t <= RED_RUN_TOLERANCE_MAX_S));
        let off = ViolationParams::default();
        assert_eq!(off.red_tolerance_s(0.0, 0.9), 0.0, "disabled: nobody runs");
    }

    #[test]
    fn the_spread_keeps_the_mean_and_has_the_cv() {
        let n = 20_000u64;
        let (mut sum, mut sq) = (0.0, 0.0);
        for i in 0..n {
            let f = lognormal_factor(0.25, std_normal(7, i, 0));
            sum += f;
            sq += f * f;
        }
        let mean = sum / n as f64;
        let cv = (sq / n as f64 - mean * mean).sqrt() / mean;
        assert!((mean - 1.0).abs() < 0.01, "mean {mean}");
        assert!((cv - 0.25).abs() < 0.02, "cv {cv}");
    }

    #[test]
    fn the_errors_are_unit_variance_and_correlated_over_tau() {
        let p = HumanDriverParams::human();
        let mut s = HumanState::default();
        let (mut sq, mut lag) = (0.0, 0.0);
        let mut prev = 0.0;
        let n = 200_000u64;
        for k in 0..n {
            let r = perceive(&p, &s, 99, k, 0.1, 30.0, 10.0, 10.0);
            s.w_gap = r.w_gap;
            s.w_rate = r.w_rate;
            sq += r.w_gap * r.w_gap;
            lag += r.w_gap * prev;
            prev = r.w_gap;
        }
        let var = sq / n as f64;
        assert!((var - 1.0).abs() < 0.15, "variance {var}");
        // One-step autocorrelation exp(−0.1/20).
        let rho = lag / sq;
        assert!((rho - (-0.005f64).exp()).abs() < 0.01, "rho {rho}");
    }

    #[test]
    fn the_delay_applies_the_decision_of_t_prime_ago_except_in_an_alarm() {
        let mut s = HumanState::default();
        for k in 0..20 {
            remember(&mut s, 0.8, 0.1, k as f64 * 0.1);
        }
        // 8 steps back from the 20th decision (index 19 = 1.9) is 1.2.
        assert!((delayed(&s, 0.8, 0.1, 2.0, 2.0) - 1.2).abs() < 1e-12);
        assert_eq!(delayed(&s, 0.8, 0.1, -5.0, 2.0), -5.0);
        assert_eq!(delayed(&s, 0.0, 0.1, 0.3, 2.0), 0.3);
    }
}
