//! The native mobility engine: the [`Mobility`] implementation that ties the models
//! together (04-models.md §2, 03-interfaces.md §3).
//!
//! # What one step does
//!
//! Seven passes, in this order, and the order is the whole design:
//!
//! 1. **Commands.** Whatever arrived through [`Mobility::command`] since the last step is
//!    applied — a reroute, a speed cap, a stop, a closure, a despawn, an injected trip.
//! 2. **Spawn.** The demand model is asked for the trips that start in this step's window,
//!    each one is routed, and an actor id is assigned **in demand-stream order**
//!    (invariant I-M2).
//! 3. **Freeze.** A [`ActorSnapshot`] of every actor's start-of-step state is built. From
//!    here to the end of the step, *nothing* reads a mutable actor.
//! 4. **Signals.** Every plan's phase at this instant is computed once, so two vehicles at
//!    the same junction cannot see different lights.
//! 5. **Claims.** Every actor's claim on the junction it is approaching is collected once,
//!    grouped by junction, ordered by actor id — the [`ConflictView`] list the
//!    intersection model reads.
//! 6. **Decide.** For each actor, in id order: one neighbour query, one intersection
//!    decision, one car-following acceleration, one lane-change decision. Every input comes
//!    from the frozen snapshot; every output goes into a buffer.
//! 7. **Integrate and publish.** The buffered decisions are applied, lane boundaries are
//!    crossed, lateral transitions advance, VRUs step, and the whole thing is published as
//!    one [`MobilityUpdate`] ordered by actor id (invariant I-M1).
//!
//! # The Jacobi update
//!
//! Passes 3 and 6 are where ADR 0004's determinism requirement lives. Every actor's new
//! state is a function of the *frozen* state of every other actor, so the result does not
//! depend on the order the actors are visited in. That is testable, and it is tested
//! directly: [`EngineParams::reverse_order`] makes pass 6 walk the actors backwards, and
//! `engine::tests::the_jacobi_update_is_order_independent` asserts that a whole run in
//! reverse order is **bit-identical** to the same run forwards. A Gauss-Seidel update — one
//! that let a follower see its leader's already-updated speed — would fail that test on the
//! first step.
//!
//! # The reference point
//!
//! An actor's longitudinal coordinate `s_m` is its **front bumper**, because a gap is a
//! bumper-to-bumper quantity. [`Kinematics::pos`] is the **rear-axle centre**
//! (03-interfaces.md §1), and the class table of §2.7 gives no wheelbase, so the published
//! position is taken at `s_m − length`: the rear bumper. The approximation is recorded on
//! the card, and it is the only place in the crate where the two conventions meet.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Determinism, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::geom::{LanePos, Vec3};
use v2xw_core::ids::{ActorId, JunctionId, LaneId, SignalId};
use v2xw_core::kinematics::Kinematics;
use v2xw_core::math;
use v2xw_core::time::{Duration, SimTime, ns_to_secs, secs_to_ns};
use v2xw_core::weather::WeatherState;
use v2xw_world::{ClassMask, JunctionControl, LaneKind, SignalState, TurnDirection, World};

use crate::carfollowing::idm::{Idm, IdmPreset};
use crate::classes::VehicleClass;
use crate::ctx::MobCtx;
use crate::error::{MobError, Result};
use crate::intersection::gap_acceptance::{GapAcceptance, GapAcceptanceParams};
use crate::intersection::signal_fixed_time::{FixedTimeSignals, SignalPlanParams};
use crate::intersection::two_coloring::{TwoColoring, TwoColoringParams};
use crate::intersection::zones::ConflictZones;
use crate::intersection::{STOP_LINE_OFFSET_M, headings_conflict};
use crate::lanechange::mobil::{Mobil, MobilParams, MobilPreset, smoothstep};
use crate::routing::dijkstra::DijkstraParams;
use crate::routing::dynamic::{DynamicCost, DynamicReroute, RerouteSource};
use crate::snapshot::{ActorSnapshot, NeighborOptions};
use crate::traits::{
    CarFollowing, ClockModel, Demand, IntersectionControl, LaneChange, Mobility, VruMobility,
};
use crate::views::{
    ActorSpawn, ConflictView, DespawnCause, DriverProfile, EMERGENCY_BRAKE_ONSET_JERK_MPS3,
    EntryDecision, JunctionView, LaneChangeDecision, LaneNeighbors, LaneView, LeaderView,
    MobilityCommand, MobilityUpdate, PhaseState, Route, Side, TripRequest, VehicleView,
};
use crate::vru::crosswalk::{CrosswalkIndex, VehiclePath};
use crate::vru::social_force::SocialForce;
use v2xw_core::rng::{EntityRef, RngDomain};

#[cfg(test)]
mod reroute_tests;
mod tiering;
pub use tiering::{Handoff, TripAdmission};

/// The model id.
pub const MODEL_ID: &str = "mobility/native/medium";

/// The model version.
pub const MODEL_VERSION: &str = "1.0.0";

/// How many lanes of its route ahead a driver without a navigation service sees a closure
/// on: through the junction ahead and the one after it — a closed road is signed at the
/// junction before it (MUTCD 2009 §6F.08, ROAD CLOSED AHEAD), which is where a driver can
/// still turn off. **This crate's choice**: with only the connector and road beyond the
/// next junction in sight, a driver who reached an approach whose only way on was the
/// closed road had already passed the last turn and waited at the barrier for good.
const CLOSURE_SIGHT_LANES: usize = 4;

/// How far short of a stop line the first car of a queue stops, metres (front bumper).
///
/// MUTCD 2009 §3B.16 places a stop line 4 ft (1.2 m) before the crosswalk it protects, and
/// a driver stops *at* the line; half a metre short of it is **this crate's choice** for
/// "at". It sets where a queue starts, and so the first discharge headway.
pub const STOP_LINE_MARGIN_M: f64 = 0.5;

/// How far before a stop line a discretionary lane change may no longer start, metres.
///
/// MUTCD 2009 §3B.04 marks the approach to a junction with a solid lane line where
/// "crossing the lane line markings is discouraged"; the manual does not fix its length.
/// 30 m is **this crate's choice**: about two car lengths more than the longest
/// lane-change transition covers at the 25 mph urban limit (2.5 s × 11.2 m/s = 28 m), so a
/// change that starts outside the zone has finished before the stop line.
pub const DEFAULT_NO_CHANGE_ZONE_M: f64 = 30.0;

/// The average lateral acceleration a junction turn is driven at, m/s²: SUMO `netconvert
/// --junctions.limit-turn-speed`'s default.
pub const DEFAULT_TURN_LATERAL_ACCEL_MPS2: f64 = 5.5;

/// A suggested window for [`EngineParams::turn_curve_window_m`], metres: **this crate's
/// choice**, one passenger-car length plus a metre (4.5 m + 1 m, rounded up to 6 m), the arc
/// a car's wheelbase spans through the tightest part of a corner; the traffic validation's
/// turning check averages over 1 s, which at 5-7 m/s turning speeds is the same 5-7 m.
pub const DEFAULT_TURN_CURVE_WINDOW_M: f64 = 6.0;

/// The card's note on how a stop line is braked for (see [`static_obstacle_accel`]).
const STOP_LINE_ONSET_NOTE: &str = "a virtual stop-line obstacle is braked for at \
    max(a_IDM, −v²/(2·(s − s0))): the constant-acceleration heuristic (Kesting, Treiber & \
    Helbing 2010, Phil. Trans. R. Soc. A 368:4585) for a standing obstacle, which stops at \
    the line with the least deceleration that does, instead of the IDM's over-reaction to \
    an obstacle that appears at a signal change";

/// Above this speed the tail of the queue on a junction's exit counts as moving, so the
/// room behind it is opening and a vehicle may enter, m/s. **This crate's choice**: a
/// walking pace; below it the queue is standing or creeping.
const EXIT_QUEUE_MOVING_MPS: f64 = 2.0;

/// How many lanes behind its front a vehicle's body is tracked across. A car is shorter
/// than any three consecutive lanes; a connector can be shorter than a car.
const TRAIL_LANES: usize = 3;

/// How fast a braking vehicle may come off the brake, m/s³.
///
/// **This crate's choice, bracketed by the literature**: Bagdadi & Várhelyi (2011,
/// *Accident Analysis & Prevention* 43(4), "Jerky driving — an indicator of accident
/// proneness?") treat jerk beyond about 10 m/s³ as harsh; ordinary driving stays well
/// under it. The IDM has no jerk term of its own, so a light turning green stepped a
/// braking car to full throttle in one 0.1 s step (−3 → +1.4 m/s², 44 m/s³). Only the
/// brake-to-throttle transition is limited: from zero upward the IDM's acceleration
/// applies unchanged. Not applied in the legacy parity mode.
const RELEASE_JERK_MPS3: f64 = 10.0;

/// How fast a *planned* stop's deceleration may build, m/s³ — see [`RELEASE_JERK_MPS3`];
/// a service brake application, twice the release rate.
const PLANNED_BRAKE_JERK_MPS3: f64 = 20.0;

/// Above this deceleration a stop is an emergency and its onset is not limited, m/s²:
/// the 3.4 m/s² AASHTO *Green Book* 2018 §3.2.2 takes as the deceleration most drivers
/// brake at when they must stop for something unexpected.
const PLANNED_STOP_MAX_DECEL_MPS2: f64 = 3.4;

/// The hardest braking the standstill-gap advance cap may apply, m/s²: 0.9 g, the
/// dry-asphalt peak friction the auditor's deceleration bound is set at (AASHTO 2018
/// §3.2.2's emergency envelope, `audit::AuditParams::max_decel_mps2` = 9 m/s²), less a
/// margin for the quantised published speed.
const CAP_MAX_DECEL_MPS2: f64 = 8.5;

/// From what deceleration on the stop-at-the-line floor of [`static_obstacle_accel`]
/// binds, m/s². **This crate's choice**: a light lift off the throttle; below it the
/// car-following model's own approach is kept. Any value up to a planned stop's
/// deceleration is safe, since once bound the floor is the constant deceleration that
/// ends at the line.
const STOP_FLOOR_BIND_MPS2: f64 = 0.3;

/// The model id a driver's own traits are drawn under ([`DriverTraits`]).
const DRIVER_TRAITS_ID: &str = "mobility/native/driver";

/// The model id a driver's own car-following parameters and perception noise are drawn
/// under ([`crate::drivers`]).
const DRIVER_PARAMS_ID: &str = "mobility/native/driver/params";

/// The median of a queue head's reaction to the green onset, seconds.
///
/// Bonneson 1992 (TRR 1365, "Modeling queued driver behavior at signalized junctions")
/// measured the first queued driver's start-response time to the green at a mean of
/// 1.22 s (**secondary**: as Çalışkanelli & Tanyel 2018 tabulate it, which also lists
/// ITE 1994's 1.0 s for signalised intersections). With the lognormal shape
/// [`REACTION_SIGMA`] that mean is a median of 1.22 / e^(σ²/2) = 1.08 s.
///
/// Until 2026-10-13 this was Taoka 1989's 1.15 s (TRR 1213), a fit to responses at the
/// *amber* onset (mean 1.30 s with this shape). On the saturation-flow experiment
/// ([`crate::calibration::SaturationExperiment`], seed default, 450 s) the re-fit moves
/// start-up lost time 3.15 → 2.95 s (HCM 2.0 s, band 1-3) and the first discharge headway
/// 3.87 → 3.80 s (Greenshields 1947: 3.8 s); saturation headway 2.14 → 2.15 s.
pub const REACTION_MEDIAN_S: f64 = 1.08;

/// The lognormal shape of a driver's reaction time: σ = 0.5, the middle of Taoka 1989's
/// spread (standard deviation 0.55-0.75 s at a 1.1-1.2 s median); with the median above
/// it gives Bonneson's 1.22 s mean.
pub const REACTION_SIGMA: f64 = 0.5;

/// The median of a queued driver's start-up delay once the car in front has moved off,
/// seconds ([`DriverTraits::follow_reaction_s`]).
///
/// Taoka's distribution is a response to a *signal change*: the first driver of a queue
/// responds to the green, but every driver behind responds to the brake lights of the car
/// in front going out and to it rolling, having watched the queue start. Field studies
/// time that start-up wave at about one vehicle a second, and no primary measurement of
/// its distribution was read, so the median is **calibrated, not measured**: with the
/// [`crate::carfollowing::idm::IdmPreset::UrbanHcm`] drivers, 0.5 s gives discharge
/// headways of 3.7, 2.9, 2.5 and 2.4 s for the first four queued vehicles and 2.17 s
/// after them on the saturation-flow experiment
/// ([`crate::calibration::SaturationExperiment`]), against the 3.8, 3.1, 2.7 and 2.4 s
/// of Greenshields, Schapiro & Ericksen's 1947 study (**secondary**: as traffic
/// engineering textbooks tabulate it) and the HCM's 2.1 s. The shape and the driver's
/// quantile are the same as for the signal response, so a slow driver is slow at both.
pub const FOLLOW_REACTION_MEDIAN_S: f64 = 0.5;

/// The amber dilemma zone, as time to the stop line at the onset of amber, seconds: at its
/// near edge 10 % of drivers stop and at its far edge 90 % do.
///
/// Zegeer & Deen 1978 ("Green-extension systems at high-speed intersections", ITE
/// Journal) define the zone by those two percentages and measured it at 2.5-5.5 s before
/// the stop line (**secondary**: the figures as the FHWA Signal Timing Manual and the
/// dilemma-zone literature quote them; the paper itself was not read).
pub const AMBER_DILEMMA_TTI_S: (f64, f64) = (2.5, 5.5);

/// The model id a driver's amber threshold is drawn under ([`DriverTraits`]).
const AMBER_TRAIT_ID: &str = "mobility/native/driver/amber";

/// The [`EntityRef::custom`] kind keying a driver's critical-gap quantile
/// ([`DriverTraits::gap_z`]).
const GAP_TRAIT_ID: &str = "mobility/native/driver/gap";

/// The model id a driver's red-light-running habit is drawn under
/// ([`EngineParams::red_runner_share`]).
const AMBER_RUNNER_ID: &str = "mobility/native/driver/amber-runner";

/// The model id a driver's red-light tolerance is drawn under
/// ([`crate::drivers::ViolationParams`]).
const RED_RUN_ID: &str = "mobility/native/driver/red-run";

/// The salt of a driver's failure-to-yield draw at one junction
/// ([`crate::drivers::ViolationParams::fail_to_yield_share`]).
const FAIL_TO_YIELD_SALT: u64 = 0x6661_696c_7969_656c; // "failyiel"

/// The slowest a driver who sees a red first must be going to run it, m/s: a vehicle
/// rolling up to the line at a walking pace stops. **A choice.**
const RED_RUN_MIN_SPEED_MPS: f64 = 3.0;

/// A uniform in [0, 1) keyed by the trip `seq` and the junction: the failure-to-yield draw.
fn fail_to_yield_uniform(seq: u64, junction: u64) -> f64 {
    use crate::drivers::splitmix64;
    let h = splitmix64(splitmix64(seq ^ FAIL_TO_YIELD_SALT) ^ junction);
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// A driver's amber threshold from the uniform `u`: the logistic across the dilemma zone
/// [`AMBER_DILEMMA_TTI_S`], 10 % going at its far edge and 90 % at its near edge (median
/// 4.0 s, scale 1.5 s / ln 9), clamped to the near edge and to `far_s` — the zone's far
/// edge, or with violations on the logistic's tail up to
/// [`crate::drivers::ViolationParams::amber_tail_max_s`].
fn amber_threshold(u: f64, far_s: f64) -> f64 {
    let (near, far) = AMBER_DILEMMA_TTI_S;
    let u = u.clamp(1e-9, 1.0 - 1e-9);
    let median = 0.5 * (near + far);
    let scale = 0.5 * (far - near) / math::ln(9.0);
    (median + scale * math::ln(u / (1.0 - u))).clamp(near, far_s.max(near))
}

/// The keyed stream a driver's signalling habit is drawn from ([`crate::lamps`]).
const LAMP_HABIT_ID: &str = "mobility/native/driver/lamps";

/// How much of the amber a driver who goes leaves in hand, seconds: they go only if they
/// reach the line this long before it turns red at their present speed. **This crate's
/// choice**, one 0.1 s step and a little more, so a vehicle that goes never meets the red.
const AMBER_GO_MARGIN_S: f64 = 0.3;

/// How soon a conflicting car still to enter on the end of its amber must reach its stop
/// line for a driver on a fresh green to wait for it, seconds ([`NativeMobility::
/// clearing_foe`]). **This crate's choice**: a little more than the dilemma zone's far
/// edge at the speeds of a city street, beyond which that car can still stop.
const CLEARING_WINDOW_S: f64 = 3.0;

/// How close the car in front of a standing vehicle must be for its moving off to be
/// what releases the vehicle — a queue — rather than a signal or a crosswalk, metres.
/// **This crate's choice**: a queue's standstill gap is 2 m, and 10 m leaves room for
/// the car in front having rolled a little before the follower's model says go.
const QUEUE_FOLLOW_GAP_M: f64 = 10.0;

/// A vehicle standing still moves off only once the car-following model asks for more
/// than this, m/s²: below it is the creep of a queue closing its last centimetres, which is
/// not a decision to go. **This crate's choice.**
const GO_ACCEL_MPS2: f64 = 0.2;

/// Standing still, for the start-up reaction, m/s.
const STANDSTILL_MPS: f64 = 0.1;

/// Below this speed a leader counts as standing for the standstill-gap cap of a driver
/// whose car-following acceleration is under 1 m/s² (the heavy-vehicle columns), m/s.
/// **A choice**: a walking pace, at which the cap's one-step slow-down stays gentle.
const HEAVY_CRAWL_MPS: f64 = 1.0;

/// How far ahead a vehicle standing at the kerb is seen as something to go round, metres:
/// about a short block's worth of sight. **A choice.**
const KERB_PASS_LOOK_M: f64 = 60.0;

/// The hardest the traffic in the lane beside may have to brake for a vehicle pulling out
/// round one at the kerb, m/s²: the IDM's comfortable deceleration of a city driver (2.0,
/// [`crate::carfollowing::idm::IdmPreset::UrbanHcm`]). A pull-out that asks more waits
/// for the next gap.
const PULL_OUT_FOLLOWER_DECEL_MPS2: f64 = 2.0;

/// The speed a pull-out from behind a vehicle at the kerb is planned and shown at, at
/// least, m/s: the transition is stretched to MOBIL's 12° heading cap at this speed (about
/// 12 s across a 3.35 m lane), so a car nosing out from a standstill angles out over that
/// time rather than swinging round on the spot. **A choice**: a walking-to-jogging pace.
const PULL_OUT_SPEED_MPS: f64 = 2.0;

/// What makes one driver unlike another, beyond the car-following parameters.
///
/// * `speed_factor` — the speed the driver chooses on a free road as a multiple of the
///   posted limit: SUMO's `speedFactor`, drawn per vehicle from `normc(1, speedDev, 0.2,
///   2)` with the class's `speedDev` (0.1 for a passenger car) — the SUMO vType default
///   (R10 §B4), and the same law SUMO applies to every lane limit. No New York field
///   distribution of free speeds against the 25 mph limit was available to calibrate it.
/// * `reaction_s` — how long the driver takes to move off once a signal or a crosswalk
///   that held them lets them go: lognormal with median [`REACTION_MEDIAN_S`] and shape
///   [`REACTION_SIGMA`] (Bonneson 1992 mean, Taoka 1989 shape), clamped to 0.3-3 s.
/// * `amber_go_tti_s` — the time to the stop line at the onset of amber below which the
///   driver goes on rather than stops, where stopping at 3 m/s² would still be possible:
///   logistic across the dilemma zone [`AMBER_DILEMMA_TTI_S`] (10 % of drivers stop at its
///   near edge, 90 % at its far edge; Zegeer & Deen 1978), clamped to it. A driver goes only
///   if they also reach the line before the amber ends.
/// * `follow_reaction_s` — the same driver's delay in moving off once the car in front of
///   them in a queue has: median [`FOLLOW_REACTION_MEDIAN_S`], the same shape and the same
///   quantile of the one normal draw, clamped to 0.2-3 s. The two are what make a queue
///   start as a wave instead of all at once, and where the HCM's start-up lost time comes
///   from.
///
/// Drawn once per trip from the trip's own streams (`DesiredSpeed` and `ReactionTime`,
/// keyed by the demand sequence number), so one driver's draws do not depend on anybody
/// else's. The legacy parity mode and a vehicle placed by
/// [`NativeMobility::spawn_with_route`] get [`DriverTraits::NEUTRAL`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DriverTraits {
    /// Free speed as a multiple of the posted limit.
    pub speed_factor: f64,
    /// Start-up reaction to a signal or crosswalk, seconds.
    pub reaction_s: f64,
    /// Start-up delay behind a car in a queue, seconds.
    pub follow_reaction_s: f64,
    /// The time to the stop line below which this driver goes on amber rather than stops,
    /// seconds; zero never goes where it could stop.
    pub amber_go_tti_s: f64,
    /// Whether this driver goes on an amber inside their threshold without checking they
    /// reach the line before the red, and so may enter on red
    /// ([`EngineParams::red_runner_share`]); false for every driver by default.
    pub amber_runner: bool,
    /// How late after the red onset this driver still enters, seconds
    /// ([`crate::drivers::ViolationParams`]); zero for every driver unless violations are on.
    pub red_tolerance_s: f64,
    /// How the driver follows ([`crate::style::DriverStyle`]), read from the same latent
    /// as `speed_factor`.
    pub style: crate::style::DriverStyle,
    /// Four standard normals for this driver's own `T`, `a`, `b`, `s0`
    /// ([`crate::drivers::spread_profile`]); zero for the neutral driver.
    pub params_z: [f64; 4],
    /// The key of this driver's perception-error noise ([`crate::drivers::std_normal`]).
    pub noise_key: u64,
    /// Whether this driver re-routes on information ([`EngineParams::reroute_share`]);
    /// true for every driver by default.
    pub reroutes: bool,
    /// This driver's standard normal for the per-driver critical gap
    /// ([`crate::intersection::gap_acceptance::JunctionPatience`]); zero for the neutral
    /// driver. Read only when `EngineParams::junction_patience` spreads the gaps.
    pub gap_z: f64,
}

impl DriverTraits {
    /// The limit exactly and no reaction time: the model without heterogeneity.
    pub const NEUTRAL: DriverTraits = DriverTraits {
        speed_factor: 1.0,
        reaction_s: 0.0,
        follow_reaction_s: 0.0,
        amber_go_tti_s: 0.0,
        amber_runner: false,
        red_tolerance_s: 0.0,
        style: crate::style::DriverStyle::Normal,
        params_z: [0.0; 4],
        noise_key: 0,
        reroutes: true,
        gap_z: 0.0,
    };

    /// The traits of the driver of trip `seq` in a vehicle of `class`, with the queue
    /// start-up delay's median [`FOLLOW_REACTION_MEDIAN_S`].
    pub fn draw(ctx: &dyn MobCtx, seq: u64, class: VehicleClass) -> DriverTraits {
        Self::draw_with(ctx, seq, class, FOLLOW_REACTION_MEDIAN_S)
    }

    /// [`DriverTraits::draw`] with the queue start-up delay's median given, seconds.
    pub fn draw_with(
        ctx: &dyn MobCtx,
        seq: u64,
        class: VehicleClass,
        follow_median_s: f64,
    ) -> DriverTraits {
        Self::draw_law(
            ctx,
            seq,
            class,
            &TraitLaw {
                follow_median_s,
                ..TraitLaw::default()
            },
        )
    }

    /// [`DriverTraits::draw`] under the population law `law` (the scenario's
    /// `actors.vehicles.drivers` speed and reaction numbers).
    pub fn draw_law(
        ctx: &dyn MobCtx,
        seq: u64,
        class: VehicleClass,
        law: &TraitLaw,
    ) -> DriverTraits {
        let key = EntityRef::custom(DRIVER_TRAITS_ID, seq);
        let speed_dev = class.spec().speed_dev * law.speed_spread;
        let speed_factor = {
            let mut rng = ctx.rng(RngDomain::DesiredSpeed, key);
            rng.normal(law.speed_factor_mean, speed_dev).clamp(0.2, 2.0)
        };
        let follow_median_s = law.follow_median_s;
        // One standard-normal draw sets how quick this driver is, for both responses.
        let z = ctx.rng(RngDomain::ReactionTime, key).normal(0.0, 1.0);
        let reaction_s = math::exp(math::ln(law.reaction_median_s.max(0.05)) + REACTION_SIGMA * z)
            .clamp(0.3, 3.0);
        let follow_reaction_s =
            math::exp(math::ln(follow_median_s.max(0.05)) + REACTION_SIGMA * z).clamp(0.2, 3.0);
        // The amber threshold: logistic across the dilemma zone, 10 % at its near edge
        // and 90 % at its far edge (median 4.0 s, scale 1.5 s / ln 9), clamped to it.
        let amber_go_tti_s = amber_threshold(
            ctx.rng(
                RngDomain::ReactionTime,
                EntityRef::custom(AMBER_TRAIT_ID, seq),
            )
            .uniform(0.0, 1.0),
            AMBER_DILEMMA_TTI_S.1,
        );
        // The driver's own car-following parameters and perception noise come from a
        // stream of their own, so drawing them leaves every other draw as it was.
        let (params_z, noise_key) = {
            let mut rng = ctx.rng(
                RngDomain::DesiredSpeed,
                EntityRef::custom(DRIVER_PARAMS_ID, seq),
            );
            let z = [
                rng.normal(0.0, 1.0),
                rng.normal(0.0, 1.0),
                rng.normal(0.0, 1.0),
                rng.normal(0.0, 1.0),
            ];
            (z, rng.u64())
        };
        // The critical-gap quantile: a stream of its own, so drawing it leaves every other
        // draw as it was.
        let gap_z = ctx
            .rng(
                RngDomain::ReactionTime,
                EntityRef::custom(GAP_TRAIT_ID, seq),
            )
            .normal(0.0, 1.0);
        DriverTraits {
            speed_factor,
            reaction_s,
            follow_reaction_s,
            amber_go_tti_s,
            amber_runner: false,
            red_tolerance_s: 0.0,
            style: crate::style::DriverStyle::of_speed_factor_around(
                speed_factor,
                law.speed_factor_mean,
                speed_dev,
            ),
            params_z,
            noise_key,
            reroutes: true,
            gap_z,
        }
    }
}

/// The population law a driver's [`DriverTraits`] are drawn under: the scenario's
/// `actors.vehicles.drivers` speed and reaction numbers. The default is the calibration
/// every scenario ran before these were exposed, so a scenario that does not set them
/// draws the same drivers bit for bit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TraitLaw {
    /// The mean free speed as a multiple of the posted limit (SUMO `speedFactor`'s mean,
    /// 1.0 by default).
    pub speed_factor_mean: f64,
    /// A multiplier on each class's speed deviation (SUMO `speedDev`: 0.1 for a car):
    /// 0 gives every driver the mean, 1 is SUMO's spread.
    pub speed_spread: f64,
    /// The median start-up reaction to a signal or crosswalk, seconds
    /// ([`REACTION_MEDIAN_S`], Bonneson 1992).
    pub reaction_median_s: f64,
    /// The median start-up delay behind the car in front, seconds
    /// ([`FOLLOW_REACTION_MEDIAN_S`]).
    pub follow_median_s: f64,
}

impl Default for TraitLaw {
    fn default() -> Self {
        TraitLaw {
            speed_factor_mean: 1.0,
            speed_spread: 1.0,
            reaction_median_s: REACTION_MEDIAN_S,
            follow_median_s: FOLLOW_REACTION_MEDIAN_S,
        }
    }
}

/// How many vulnerable road users the engine keeps in the world (`actors.vru`).
///
/// Pedestrians walk the world's sidewalk and crossing lanes with the social-force model
/// ([`SocialForce`]); cyclists ride the lanes that admit bicycles with the car-following
/// and lane-change models, on the SUMO bicycle vType ([`bicycle_driver`]). Each one that
/// finishes its walk or ride is replaced, so the population holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VruPopulation {
    /// Pedestrians.
    pub pedestrians: u32,
    /// Cyclists.
    pub cyclists: u32,
}

/// The model id VRU placement draws its streams under.
const VRU_PLACEMENT_ID: &str = "mobility/vru/population";

/// How far apart two connectors from one approach lane must be before a road user on one no
/// longer occupies the other, metres: half the widest class (a coach, 2.6 m) plus half a
/// car (1.8 m) plus a margin — and so also clear of a bicycle (0.65 m), for which it was
/// first set.
const DIVERGED_M: f64 = 2.5;

/// The same for a road user wider than 2 m (a truck, bus or coach): half of 2.6 m on each
/// side plus 0.3 m.
const DIVERGED_WIDE_M: f64 = 2.9;

/// How many walkable lanes a pedestrian's walk strings together at most.
const PEDESTRIAN_WALK_LANES: usize = 12;

/// The custom entity a pedestrian group's size is drawn under.
const VRU_GROUP_ID: &str = "mobility/vru/group";

/// How far apart the members of a group walk side by side, metres: shoulder to shoulder
/// with a little room (a body is 0.48 m wide). **A choice.**
const GROUP_SPACING_M: f64 = 0.7;

/// The driver of a bicycle: the SUMO vType defaults for vClass `bicycle` (the class
/// table's [`crate::classes::ClassSpec`]: desired 20 km/h, acceleration 1.2 m/s²,
/// deceleration 3.0 m/s², gap 0.5 m) with SUMO's default `tau` of 1 s as the time
/// headway. The car-following presets calibrate cars and trucks, not riders.
pub fn bicycle_driver() -> DriverProfile {
    let spec = VehicleClass::Bicycle.spec();
    DriverProfile {
        desired_speed_mps: spec.desired_max_speed_mps.unwrap_or(spec.max_speed_mps),
        max_accel_mps2: spec.accel_mps2,
        comfort_decel_mps2: spec.decel_mps2,
        time_headway_s: 1.0,
        min_gap_m: spec.min_gap_m,
        sigma: None,
    }
}

/// How motorcycles and mopeds ride, where they differ from a car.
///
/// | What | Default | Source |
/// |---|---|---|
/// | acceleration in traffic | motorcycle 2.5 m/s², moped 1.1 m/s² | the moped's is the SUMO vType default (R10 §B4); for the motorcycle, French naturalistic riding data (26 instrumented motorcycles over 18 months) found accelerations beyond ±4 m/s² only 0.5 % of the time, and the SUMO default of 6 m/s² is a sports machine's capability, not how a rider moves off in a city: 2.5 m/s² is **a choice** inside that envelope |
/// | comfortable deceleration | 3.0 m/s² (both) | **a choice** inside the same ±4 m/s² envelope; emergency braking stays the class's 10 m/s² (SUMO) |
/// | time headway, standstill gap | 1.2 s, 1.5 m | **a choice**: a smaller vehicle stops closer behind a car; no urban motorcycle headway distribution was read |
/// | cornering | `v = sqrt(g·R·tan φ)`, `φ` ≤ 25° | the steady-turn balance of a leaning two-wheeler (Cossalter, *Motorcycle Dynamics*, 2006); riders keep well inside the tyres' 45–50° limit in traffic, and 25° (0.47 g) is **a choice** |
/// | lane position | the left tyre track, 0.5 m left of the centreline | the Motorcycle Safety Foundation's default lane position (the left third of the lane) in right-hand traffic |
///
/// Filtering between stopped vehicles is a jurisdiction's rule
/// ([`crate::rules::TrafficRules::motorcycle_filtering`]): every jurisdiction this engine
/// has rules for prohibits it, so a motorcycle keeps its place in the queue.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TwoWheelerParams {
    /// Whether motorcycles and mopeds get their own dynamics, cornering and lane position
    /// (otherwise they drive as the car-following preset's cars do).
    pub enabled: bool,
    /// A motorcycle's acceleration, m/s².
    pub motorcycle_accel_mps2: f64,
    /// A moped's acceleration, m/s².
    pub moped_accel_mps2: f64,
    /// The comfortable deceleration of either, m/s².
    pub comfort_decel_mps2: f64,
    /// Their time headway, seconds.
    pub time_headway_s: f64,
    /// Their standstill gap, metres.
    pub min_gap_m: f64,
    /// The largest lean angle a rider takes a junction turn at, degrees.
    pub max_lean_deg: f64,
    /// How far left of the lane centreline they ride, metres.
    pub lane_position_m: f64,
}

impl Default for TwoWheelerParams {
    fn default() -> Self {
        Self {
            enabled: true,
            motorcycle_accel_mps2: 2.5,
            moped_accel_mps2: VehicleClass::Moped.spec().accel_mps2,
            comfort_decel_mps2: 3.0,
            time_headway_s: 1.2,
            min_gap_m: 1.5,
            max_lean_deg: 25.0,
            lane_position_m: 0.5,
        }
    }
}

/// Standard gravity, m/s².
const GRAVITY_MPS2: f64 = 9.80665;

/// [`NativeMobility::lane_position_m`] from the parameters alone.
fn lane_position_for(params: &EngineParams, class: VehicleClass) -> f64 {
    let tw = params.two_wheelers;
    if tw.enabled
        && params.intersections != IntersectionMode::TwoColoringLegacy
        && matches!(class, VehicleClass::Motorcycle | VehicleClass::Moped)
    {
        tw.lane_position_m
    } else {
        0.0
    }
}

/// How cyclists' desired speeds are drawn.
///
/// The German Naturalistic Cycling Study (Schleinitz, Petzoldt, Franke-Bartholdt, Krems &
/// Gehlert, *Safety Science* 92:290–297, 2017; 90 riders, nearly 17,000 km) measured mean
/// speeds of 15.3 km/h on conventional bicycles and 17.4 km/h on pedelecs (assisted to
/// 25 km/h) — the abstract states the 2 km/h difference. Each cyclist draws a bicycle type
/// with [`CyclistSpeeds::ebike_share`] and a speed around its type's mean with
/// [`CyclistSpeeds::relative_sd`]. The share of e-bikes in a city's cycle traffic and the
/// spread are **choices**: no count of either for Midtown was read.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CyclistSpeeds {
    /// Whether cyclists draw their own speeds (otherwise every one rides at the SUMO
    /// bicycle vType's 20 km/h).
    pub enabled: bool,
    /// Mean speed on a conventional bicycle, m/s.
    pub conventional_mps: f64,
    /// Mean speed on an e-bike, m/s.
    pub ebike_mps: f64,
    /// The share of cyclists on e-bikes.
    pub ebike_share: f64,
    /// The spread of one rider's speed around their type's mean, as a fraction of it.
    pub relative_sd: f64,
    /// How much longer a metre of lane shared with motor traffic feels to a cyclist than a
    /// metre of cycle lane, for the route choice: revealed-preference studies of cyclists'
    /// GPS routes (Broach, Dill & Gliebe, *Transp. Res. A* 46, 2012, in Portland) find
    /// riders detour to use bicycle facilities. 1.5 is **a choice** in that direction; the
    /// study's own coefficients depend on traffic volume and were not transcribed.
    pub shared_lane_cost: f64,
}

impl Default for CyclistSpeeds {
    fn default() -> Self {
        Self {
            enabled: true,
            conventional_mps: 15.3 / 3.6,
            ebike_mps: 17.4 / 3.6,
            ebike_share: 0.3,
            relative_sd: 0.2,
            shared_lane_cost: 1.5,
        }
    }
}

/// A cyclist's route cost: free-flow (or observed) time, with a lane shared with motor
/// traffic weighted by [`CyclistSpeeds::shared_lane_cost`] and a cycle lane not.
struct CyclistCost<'a> {
    inner: &'a DynamicCost<'a>,
    world: &'a World,
    shared_factor: f64,
}

impl crate::views::EdgeCost for CyclistCost<'_> {
    fn lane_cost_s(&self, lane: LaneId, at: SimTime) -> Option<f64> {
        let base = crate::views::EdgeCost::lane_cost_s(self.inner, lane, at)?;
        let shared = self
            .world
            .try_lane(lane)
            .is_some_and(|l| matches!(l.kind, LaneKind::Driving | LaneKind::Bus));
        Some(if shared {
            base * self.shared_factor.max(1.0)
        } else {
            base
        })
    }
}

/// The fastest a cyclist wants to ride, m/s: 25 km/h, where a pedelec's assistance stops
/// (EU Regulation 168/2013, art. 2(2)(h)); few riders on a conventional bicycle hold more
/// in city traffic. A cap on the draw, not a cited distribution's tail.
const CYCLIST_MAX_DESIRED_MPS: f64 = 25.0 / 3.6;

/// Below this speed, short of the stop line, a driver who committed to an amber is no
/// longer clearing the junction, m/s: walking pace.
const AMBER_COMMIT_LAPSE_MPS: f64 = 1.0;

/// The custom entity a cyclist's speed is drawn under.
const CYCLIST_SPEED_ID: &str = "mobility/vru/cyclist-speed";

/// Which intersection rule the engine applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntersectionMode {
    /// The medium tier: the fixed-time signal model at signalised junctions and HCM gap
    /// acceptance everywhere else.
    #[default]
    SignalsAndGapAcceptance,
    /// Signals only: an unsignalised junction is uncontrolled.
    SignalsOnly,
    /// Gap acceptance only, whatever the junction says.
    GapAcceptanceOnly,
    /// The legacy parity mode: the abstract two-colouring everywhere, and the legacy
    /// curve-speed cap (04-models.md §2.3).
    TwoColoringLegacy,
    /// Nothing: vehicles stop for nothing at junctions. What the abstract
    /// `mobility/kinematic/lane-follow` tier does.
    None,
}

/// A passenger-car driver's car-following parameters set by hand
/// ([`NativeMobility::with_driver_override`]); `None` keeps the model's own.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DriverOverride {
    /// Comfortable maximum acceleration `a`, m/s².
    pub max_accel_mps2: Option<f64>,
    /// Comfortable deceleration `b`, m/s².
    pub comfort_decel_mps2: Option<f64>,
    /// Desired time headway `T`, s.
    pub time_headway_s: Option<f64>,
    /// Standstill distance `s0`, m.
    pub min_gap_m: Option<f64>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_one(v: &f64) -> bool {
    *v == 1.0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_navigation(s: &crate::routing::dynamic::RerouteSource) -> bool {
    *s == crate::routing::dynamic::RerouteSource::Navigation
}

/// How far from the world's bounding box, metres, a lane may start and still count as
/// entering the map from outside it ([`is_boundary_entry`]). An import clips its roads at
/// the box, so a cut road starts on it; the band absorbs the junction set-back the
/// importer pulls a lane's end back by. This crate's choice.
pub const BOUNDARY_BAND_M: f64 = 30.0;

/// Whether `lane` brings traffic in from outside the map: a motorised lane outside any
/// junction that no motorised lane feeds except by a U-turn, starting within
/// [`BOUNDARY_BAND_M`] of the world's bounding box. A dead end inside the map (a garage, a
/// cul-de-sac) is not one: traffic there really does start from rest. A procedural grid
/// has none, every lane being fed by the junction it leaves.
pub fn is_boundary_entry(world: &World, lane: LaneId) -> bool {
    let Some(l) = world.try_lane(lane) else {
        return false;
    };
    if !l.kind.is_motorised() || l.kind == LaneKind::Internal || l.junction.is_some() {
        return false;
    }
    let fed = world.predecessors(lane).iter().any(|c| {
        c.direction != TurnDirection::UTurn
            && world
                .try_lane(c.from_lane)
                .is_some_and(|f| f.kind.is_motorised())
    });
    if fed {
        return false;
    }
    let p = l.start();
    let b = world.bbox;
    (p.x - b.min.x).abs() <= BOUNDARY_BAND_M
        || (b.max.x - p.x).abs() <= BOUNDARY_BAND_M
        || (p.y - b.min.y).abs() <= BOUNDARY_BAND_M
        || (b.max.y - p.y).abs() <= BOUNDARY_BAND_M
}

/// The engine's parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EngineParams {
    /// The mobility step, which must match the step the caller passes to
    /// [`Mobility::step`]; it is declared here because MOBIL's reconsideration probability
    /// is per step.
    pub step: Duration,
    /// Which intersection rule applies.
    pub intersections: IntersectionMode,
    /// Whether discretionary lane changes are enabled.
    pub lane_changes: bool,
    /// How far the neighbour query looks, metres.
    pub lookahead_m: f64,
    /// The classes the engine's vehicles may use.
    pub classes: ClassMask,
    /// The maximum lifetime of an actor, after which it despawns. `None` is unlimited.
    pub max_lifetime: Option<Duration>,
    /// The minimum net gap an insertion needs, metres: a trip whose origin is occupied is
    /// dropped rather than overlapped.
    pub insertion_gap_m: f64,
    /// Whether to re-plan a route when a closure or a travel-time update changes it.
    pub dynamic_rerouting: bool,
    /// How far before the end of a lane a discretionary lane change may no longer start,
    /// metres ([`DEFAULT_NO_CHANGE_ZONE_M`]). Not applied in the legacy parity mode.
    pub no_change_zone_m: f64,
    /// The average lateral acceleration a turn through a junction is driven at, m/s²:
    /// a connector of average radius `R` is taken at `sqrt(a·R)` at most. `0` disables it.
    /// Not applied in the legacy parity mode.
    ///
    /// 5.5 m/s² is SUMO `netconvert --junctions.limit-turn-speed`'s default ("limits speed
    /// on junctions to an average lateral acceleration of at most FLOAT m/s²").
    pub turn_lateral_accel_mps2: f64,
    /// The arc length over which a connector's radius is judged for its turn speed,
    /// metres. `0` (the default) judges the whole connector by its average radius, as
    /// SUMO's netconvert does; a positive window takes the tightest arc of that length
    /// (the connector's vertex angles summed over the window), so the lateral acceleration stays near `turn_lateral_accel_mps2` at
    /// the apex too, not only on average ([`DEFAULT_TURN_CURVE_WINDOW_M`] when enabled
    /// from the guide).
    pub turn_curve_window_m: f64,
    /// Whether a vehicle yields to vehicles already inside the junction on a conflicting
    /// movement, and does not enter a junction whose exit has no room for it. Not applied
    /// in the legacy parity mode. See [`crate::intersection::zones`].
    pub junction_clearance: bool,
    /// Whether vehicles yield to pedestrians on a crosswalk and keep out of a crosswalk
    /// they could not clear (`crate::vru::crosswalk`). On by default; off is the control
    /// run that proves the auditor's crosswalk checks can fail.
    pub crosswalk_yield: bool,
    /// Whether each driver gets their own [`DriverTraits`] — a free speed around the
    /// posted limit and a start-up reaction time. Not applied in the legacy parity mode.
    pub driver_heterogeneity: bool,
    /// Whether a right turn may be made against a steady red after a full stop — the
    /// jurisdiction's rule ([`SignalPlanParams::right_turn_on_red`]). Off by default, which
    /// is New York City's; [`crate::rules::TrafficRules`] maps a world's highway preset
    /// onto it.
    pub right_turn_on_red: bool,
    /// The median of a queued driver's start-up delay behind the car in front, seconds
    /// ([`FOLLOW_REACTION_MEDIAN_S`], a calibrated value).
    pub queue_start_delay_median_s: f64,
    /// The mean free speed as a multiple of the posted limit ([`TraitLaw`]); 1.0.
    pub speed_factor_mean: f64,
    /// A multiplier on each class's speed deviation ([`TraitLaw`]); 1.0.
    pub speed_spread: f64,
    /// The median start-up reaction to a signal, seconds ([`REACTION_MEDIAN_S`]).
    pub start_reaction_median_s: f64,
    /// The critical headway a permitted left turn accepts in the opposing flow, seconds
    /// ([`crate::intersection::signal_fixed_time::HCM_PERMITTED_LEFT_CRITICAL_HEADWAY_S`],
    /// HCM 6th ed. ch. 31: 4.5 s).
    pub permitted_left_critical_headway_s: f64,
    /// Whether heavy vehicles add the HCM 6th ed. Exhibit 20-11 heavy-vehicle terms to
    /// their critical gap and follow-up time, at unsignalised junctions and on a
    /// permitted left ([`crate::intersection::gap_acceptance::hcm_heavy_vehicle_terms_s`]).
    /// Off by default; `traffic.heavy_vehicle_gap_terms`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub heavy_vehicle_gap_terms: bool,
    /// Whether a driver closing on a standing (or, for a heavy vehicle, crawling) vehicle
    /// ahead brakes at no less than the constant deceleration that stops it at its
    /// standstill gap, `v²/(2·(s − s0))`, as the stop line already is
    /// ([`static_obstacle_accel`]; SUMO's `maximumSafeStopSpeed`). The IDM's braking
    /// scales with its `a` (1.3 m/s² for a delivery van), so without it such a driver
    /// reached its standstill gap still moving and the advance cap stopped it in one step:
    /// a delivery van 0.93 → 0.08 m/s in 0.1 s (−8.5 m/s², over its own 7 m/s² emergency
    /// limit), which an honest BSM then shows as 1.10 → 0.00 m/s and the US MaxAcceleration
    /// rule reports (docs/design/scms-real/testing/honest-density.md §2). Off by default;
    /// `traffic.standing_leader_stop`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub standing_leader_stop: bool,
    /// Whether a trip enters the road already moving, at the highest speed it may safely
    /// drive there (SUMO's `departSpeed="max"`): the lower of its desired speed on the
    /// lane, the speed it can still stop from before the lane's end, and the speed that
    /// keeps the IDM's desired gap to the vehicle ahead. Off, every trip departs at rest
    /// (SUMO's default `departSpeed="0"`), which on a freeway puts a standing car mid-lane
    /// on a 29 m/s road (live-scenarios finding 3). `traffic.depart_speed_max`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub depart_speed_max: bool,
    /// Whether the bends of a through lane's polyline are driven no faster than the
    /// turn-speed rule allows (`sqrt(turn_lateral_accel_mps2 · R)`, with R judged over the
    /// tightest [`EngineParams::turn_curve_window_m`] of the lane, 6 m when that is 0), as
    /// a junction connector already is. A lane polyline's vertex is a kink the vehicle
    /// follows exactly; with no limit a through car on a bent street (Midtown sender 194
    /// near Broadway, heading −2.06 → −3.11 rad and back at 10 m/s) turned at over
    /// 12 m/s², which receivers judge as RandomPosition jumps. Off by default;
    /// `traffic.lane_bend_speed`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub lane_bend_speed: bool,
    /// A multiplier on the HCM critical gaps and follow-up times at unsignalised
    /// junctions ([`GapAcceptanceParams::gap_factor`]); 1.0.
    pub critical_gap_factor: f64,
    /// Per-driver critical gaps and junction impatience, at unsignalised junctions, on a
    /// permitted left and on a right turn on red
    /// ([`crate::intersection::gap_acceptance::JunctionPatience`]). Off by default
    /// (`traffic.critical_gap_cv`, `traffic.time_to_impatience_s`,
    /// `traffic.impatience_gap_floor`, `traffic.impatience_foe_braking`).
    #[serde(skip_serializing_if = "crate::intersection::gap_acceptance::JunctionPatience::is_off")]
    pub junction_patience: crate::intersection::gap_acceptance::JunctionPatience,
    /// How many permitted left turners per lane may complete the turn in the change
    /// interval after a permissive green, the HCM's sneakers
    /// ([`crate::intersection::signal_fixed_time::FixedTimeSignals::sneaker_may_go`]); 0
    /// is none, the default (`traffic.permissive_sneakers`; the HCM credits 2).
    #[serde(skip_serializing_if = "is_zero_u8")]
    pub permissive_sneakers: u8,
    /// How motorcycles and mopeds ride ([`TwoWheelerParams`]). Not applied in the legacy
    /// parity mode.
    pub two_wheelers: TwoWheelerParams,
    /// How fast cyclists ride ([`CyclistSpeeds`]).
    pub cyclists: CyclistSpeeds,
    /// Whether a driver who committed to an amber and has since been stopped short of the
    /// line loses the commitment (on by default; off reproduces the earlier behaviour, for
    /// an ablation).
    pub amber_commit_lapse: bool,
    /// The share of drivers who run reds: inside their amber threshold they go without
    /// checking that they reach the line before the red, so some enter on red (the
    /// violations RLVW is built to warn of). 0 by default: every driver who goes clears
    /// on amber. Drawn per trip from a keyed stream of its own, so it moves no other draw.
    #[serde(skip_serializing_if = "is_zero_share")]
    pub red_runner_share: f64,
    /// Kerbside stops: buses at stops, taxis, double-parked deliveries
    /// ([`crate::kerbside`]). Not applied in the legacy parity mode, nor to a vehicle
    /// placed by a command.
    pub kerbside: crate::kerbside::KerbsideParams,
    /// Whether each driver follows in their own style ([`crate::style::DriverStyle`]):
    /// assertive drivers keep shorter gaps and accelerate harder, cautious ones the
    /// opposite. Needs `driver_heterogeneity`; not applied in the legacy parity mode.
    pub driver_styles: bool,
    /// Whether a heavy vehicle takes a turning connector at no more than the AASHTO design
    /// turning speed ([`crate::style::HEAVY_TURN_SPEED_MPS`]). Not applied in the legacy
    /// parity mode.
    pub heavy_turns: bool,
    /// Whether each class is routed on the lanes its own class may use (a car not on a
    /// bus-only lane), falling back to the engine-wide router only when its own has no
    /// route. Not applied in the legacy parity mode.
    pub class_routing: bool,
    /// Gridlock resolution: a vehicle that has stood still this long, seconds, is moved
    /// along its route to the next lane with room for it (SUMO's `--time-to-teleport`,
    /// whose default is 300 s), and counted ([`GridlockCounters`]). Standing at a kerbside
    /// stop or under a scripted stop does not count. `0` turns it off, which is the
    /// engine's default so a unit test sees the raw dynamics; a scenario sets it from
    /// `traffic.gridlock`.
    pub time_to_teleport_s: f64,
    /// Strategic lane choice: a route is planned from every lane of the road the vehicle
    /// is on, not only its own, and the vehicle changes lane ahead of the turn it needs
    /// (SUMO's strategic lane-change layer, Erdmann 2015, "SUMO's lane-changing model").
    /// Without it a driver in the wrong lane for their turn drove on and round the block.
    /// Needs `lane_changes`; off by default so a unit test sees the plain router, set by a
    /// scenario's `traffic.strategic_lane_choice`.
    pub strategic_lane_choice: bool,
    /// SUMO LC2013's lane-change motivations ([`crate::lanechange::lc2013`]) in place of
    /// MOBIL's discretionary decision, or `None` (the default: MOBIL). Set by any of
    /// `traffic.lc_strategic`, `lc_cooperative`, `lc_speed_gain`, `lc_keep_right`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lc2013: Option<crate::lanechange::lc2013::Lc2013Params>,
    /// Congestion-aware rerouting, every this many seconds per vehicle, or `0` (off, the
    /// default). When on, each lane's speed is sampled every second and smoothed over
    /// [`CONGESTION_ADAPTATION_STEPS`] samples, the router prices a lane at its length over
    /// that speed, and every vehicle re-plans its route from where it is once a period
    /// (SUMO's rerouting device, `--device.rerouting.period`, with its default adaptation
    /// interval of 1 s and 180 adaptation steps). Set by `traffic.reroute_period_s`.
    pub reroute_period_s: f64,
    /// The share of drivers who re-route on information (0-1), drawn once per driver
    /// ([`crate::routing::dynamic::draws_reroute`]): SUMO's
    /// `device.rerouting.probability`. 1 (the default) is every driver, as before; the
    /// rest keep the route they chose and detour only round a closure they can see.
    /// Set by `traffic.reroute_share`.
    #[serde(skip_serializing_if = "is_one")]
    pub reroute_share: f64,
    /// Where those drivers learn that the road has changed
    /// ([`crate::routing::dynamic::RerouteSource`]): a navigation app (the default) or
    /// the V2X warnings their own unit received. Set by `traffic.reroute_source`.
    #[serde(skip_serializing_if = "is_navigation")]
    pub reroute_source: crate::routing::dynamic::RerouteSource,
    /// Vehicles whose origin is a lane entering the map from outside it
    /// ([`is_boundary_entry`]) are put on at the lane's start at their desired speed,
    /// capped by the speed that is safe behind the vehicle ahead and that can still stop
    /// at the lane's end at the comfortable deceleration; origins inside the network
    /// stay at rest. SUMO `departSpeed="desired"` with `departPos="base"` (sumo.md P9):
    /// a car crossing the map's edge is already moving, and one dropped there at rest is
    /// a false halt and a false stopped-vehicle BSM. Off by default;
    /// `traffic.depart_at_speed`.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub depart_at_speed: bool,
    /// Human drivers: per-driver parameter spread, reaction time, perception errors and
    /// the heavy-vehicle left-lane restriction ([`crate::drivers`]). All off by default.
    /// Not applied in the legacy parity mode.
    pub human: crate::drivers::HumanDriverParams,
    /// Rule violations and crash states ([`crate::drivers::ViolationParams`]). Off by
    /// default. Not applied in the legacy parity mode.
    #[serde(skip_serializing_if = "crate::drivers::ViolationParams::is_default")]
    pub violations: crate::drivers::ViolationParams,
    /// **A test hook, not a model parameter.** Walks the decision pass in reverse actor
    /// order. Because the pass reads only the frozen snapshot, the published result must be
    /// bit-identical either way; that is the ADR 0004 Jacobi property, and this is how the
    /// crate tests it.
    pub reverse_order: bool,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_share(x: &f64) -> bool {
    *x == 0.0
}

fn is_zero_u8(x: &u8) -> bool {
    *x == 0
}

/// How far short of the first opposing-stream zone a permitted left turner waiting inside
/// the junction stops, metres. **This crate's choice.**
const BOX_WAIT_MARGIN_M: f64 = 1.0;

/// The least distance into the junction a wait point must lie for a turner to pull in to
/// it, metres: nearer the line, waiting there is waiting at the line.
const BOX_WAIT_MIN_M: f64 = 2.0;

impl Default for EngineParams {
    fn default() -> Self {
        Self {
            step: Duration::from_millis(100),
            intersections: IntersectionMode::default(),
            lane_changes: true,
            lookahead_m: crate::carfollowing::idm::LEGACY_LOOKAHEAD_M,
            classes: ClassMask::MOTOR_TRAFFIC,
            max_lifetime: None,
            insertion_gap_m: 2.0,
            dynamic_rerouting: true,
            no_change_zone_m: DEFAULT_NO_CHANGE_ZONE_M,
            turn_lateral_accel_mps2: DEFAULT_TURN_LATERAL_ACCEL_MPS2,
            turn_curve_window_m: 0.0,
            junction_clearance: true,
            crosswalk_yield: true,
            driver_heterogeneity: true,
            right_turn_on_red: false,
            queue_start_delay_median_s: FOLLOW_REACTION_MEDIAN_S,
            speed_factor_mean: 1.0,
            speed_spread: 1.0,
            start_reaction_median_s: REACTION_MEDIAN_S,
            heavy_vehicle_gap_terms: false,
            standing_leader_stop: false,
            depart_speed_max: false,
            lane_bend_speed: false,
            permitted_left_critical_headway_s:
                crate::intersection::signal_fixed_time::HCM_PERMITTED_LEFT_CRITICAL_HEADWAY_S,
            critical_gap_factor: 1.0,
            junction_patience: crate::intersection::gap_acceptance::JunctionPatience::OFF,
            permissive_sneakers: 0,
            two_wheelers: TwoWheelerParams::default(),
            cyclists: CyclistSpeeds::default(),
            amber_commit_lapse: true,
            red_runner_share: 0.0,
            kerbside: crate::kerbside::KerbsideParams::default(),
            driver_styles: true,
            heavy_turns: true,
            class_routing: true,
            time_to_teleport_s: 0.0,
            strategic_lane_choice: false,
            lc2013: None,
            reroute_period_s: 0.0,
            reroute_share: 1.0,
            reroute_source: crate::routing::dynamic::RerouteSource::Navigation,
            depart_at_speed: false,
            human: crate::drivers::HumanDriverParams::default(),
            violations: crate::drivers::ViolationParams::default(),
            reverse_order: false,
        }
    }
}

/// A vehicle's crash ([`crate::drivers::ViolationParams::crashes`]): it stands where it
/// collided until `clear_at`, then leaves with [`DespawnCause::Collision`].
#[derive(Debug, Clone, Copy, PartialEq)]
struct CrashState {
    /// When it is cleared from the road.
    clear_at: SimTime,
    /// Whether either vehicle was committing a tagged violation when they collided: a crash
    /// between two lawful vehicles is a model defect, and stays visible to the auditor.
    by_violation: bool,
}

/// A lane-change transition in progress.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Transition {
    /// The lane being left.
    from: LaneId,
    /// The lane being entered.
    to: LaneId,
    /// When it started.
    started: SimTime,
    /// How long it lasts.
    duration: Duration,
    /// The lateral offset it starts from, metres.
    from_offset_m: f64,
    /// The lateral offset it ends at, metres.
    to_offset_m: f64,
    /// Whether the lane change has already been applied to the actor's lane.
    switched: bool,
    /// `s` on the lane being left minus `s` on the lane being entered, at the switch:
    /// where the body still is on the lane it is leaving.
    from_s_delta: f64,
    /// A pull-out from behind a vehicle standing at the kerb, started at a crawl: its
    /// heading swing is taken at [`PULL_OUT_SPEED_MPS`] at least, so a car nosing out
    /// from a standstill angles out instead of swinging round on the spot.
    pull_out: bool,
    /// Begun while moving: such a change is held while the vehicle is later brought to a
    /// stand part-way across. One begun at a stand (a car nosing out from a queue) slides
    /// in as before; held, it stood a full lane width out of its new lane.
    began_moving: bool,
}

/// The road a strategic lane change needs, per lane crossed, metres: at 11 m/s a
/// 3 s manoeuvre (MOBIL's transition, [`crate::lanechange::mobil`]) covers 33 m, and a
/// slower urban approach less. **A choice**, not a cited figure.
const STRATEGIC_M_PER_CHANGE: f64 = 25.0;

/// How far before the no-change zone a vehicle's strategic change counts as urgent, so
/// that it drops back behind the target-lane leader that blocks it
/// ([`NativeMobility::strategic_fall_back`]), metres. **A choice**: about two car lengths
/// and a gap.
const URGENT_LOOK_M: f64 = 20.0;

/// What a strategic lane change costs in the route choice, seconds per lane crossed: the
/// driver prefers to stay in lane unless the other lane's route saves more than this.
/// **A choice**, of the order of the lane-change duration.
const STRATEGIC_CHANGE_COST_S: f64 = 4.0;

/// How long a vehicle stands at the end of its lane, unable to make the strategic change
/// its route needs, before it gives up and takes a route from the lane it is on, seconds.
/// **A choice**: about two cycles of a busy neighbour lane's gaps.
const STRATEGIC_GIVE_UP_S: f64 = 8.0;

/// How many one-second samples a lane's congestion speed is smoothed over: SUMO's
/// `--device.rerouting.adaptation-steps` default (180), as an exponential average of the
/// same memory.
const CONGESTION_ADAPTATION_STEPS: f64 = 180.0;

/// The slowest a congested lane is priced at, m/s, so a standing queue costs a long but
/// finite time and stays routable.
const CONGESTION_FLOOR_MPS: f64 = 0.5;

/// Below this speed a vehicle counts as standing for the gridlock timer, m/s: SUMO's
/// waiting-time threshold (`SUMO_const_haltingSpeed`, 0.1 m/s).
const STUCK_SPEED_MPS: f64 = 0.1;

/// What the gridlock resolution did over a run ([`EngineParams::time_to_teleport_s`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct GridlockCounters {
    /// Vehicles found standing for the whole timeout: each one is a jam the network did
    /// not resolve by itself.
    pub jammed: u64,
    /// Of those, moved on along their route (SUMO's teleport).
    pub teleported: u64,
    /// Of those, removed because no lane on the rest of the route had room.
    pub removed: u64,
    /// Strategic lane changes given up at the end of the lane for want of a gap
    /// ([`EngineParams::strategic_lane_choice`]).
    pub strategic_given_up: u64,
}

/// One actor the engine drives.
#[derive(Debug, Clone, PartialEq)]
struct Actor {
    id: ActorId,
    seq: u64,
    class: VehicleClass,
    driver: DriverProfile,
    route: Route,
    route_index: usize,
    destination: LaneId,
    lane: LaneId,
    /// Front-bumper arc length along `lane`, metres.
    s_m: f64,
    lateral_m: f64,
    speed_mps: f64,
    accel_mps2: f64,
    spawned: SimTime,
    planned_at: SimTime,
    planned_generation: u64,
    transition: Option<Transition>,
    cooldown_until: SimTime,
    speed_cap_mps: Option<f64>,
    stopped_until: Option<SimTime>,
    /// The lanes the vehicle drove to reach `lane`, most recent last, at most
    /// [`TRAIL_LANES`] of them: where its body still is while it straddles a boundary.
    /// Empty after a spawn or a lane change.
    trail: Vec<LaneId>,
    /// The junction the vehicle chose to clear on amber; see the decision pass.
    amber_commit: Option<JunctionId>,
    /// The junction whose amber the vehicle has already seen and decided on.
    amber_seen: Option<JunctionId>,
    /// The junction whose amber or red this driver has already judged for a violation
    /// ([`crate::drivers::ViolationParams`]): they decide once, as at the amber.
    signal_look: Option<JunctionId>,
    /// The violations this vehicle is committing on purpose ([`crate::audit::intent`]
    /// bits), kept while it is inside the junction they were committed at.
    violation: u8,
    /// Its crash, once it has collided ([`crate::drivers::ViolationParams::crashes`]).
    crash: Option<CrashState>,
    /// The driver's own speed factor and reaction time.
    traits: DriverTraits,
    /// While standing: when what held it first let it go (the start of its reaction).
    release_at: Option<SimTime>,
    /// A scripted emergency stop in progress ([`MobilityCommand::Brake`]): the
    /// deceleration, and when the vehicle may move off once it stands.
    forced_brake: Option<(f64, Option<SimTime>)>,
    /// A scripted cut-in pending ([`MobilityCommand::CutIn`]): the side, and until when.
    forced_cut_in: Option<(Side, SimTime)>,
    /// Its kerbside stops ([`crate::kerbside`]).
    kerb: crate::kerbside::KerbState,
    /// Since when it has stood still without a kerbside or scripted reason: what the
    /// gridlock resolution ([`EngineParams::time_to_teleport_s`]) reads.
    stuck_since: Option<SimTime>,
    /// The driver's reaction history and perception errors ([`crate::drivers`]).
    human: crate::drivers::HumanState,
    /// Its LC2013 accumulators ([`EngineParams::lc2013`]).
    lc: crate::lanechange::lc2013::Lc2013State,
}

impl Actor {
    /// Whether any of `lanes` is still ahead on this actor's route.
    fn reroutes_on(&self, lanes: &BTreeSet<LaneId>) -> bool {
        self.route
            .lanes
            .iter()
            .skip(self.route_index + 1)
            .any(|l| lanes.contains(l))
    }

    /// What the exterior-lamp rules need to know about this vehicle ([`crate::lamps`]):
    /// its speed and acceleration, the lane change under way, the turn it is making, and
    /// the next turn on its route with the distance to where it starts.
    fn lamp_input(&self, world: &World) -> crate::lamps::LampInput {
        let lane = world.lane(self.lane);
        let direction = |from: LaneId, to: LaneId| -> Option<TurnDirection> {
            world
                .successors(from)
                .iter()
                .find(|c| c.via == Some(to) || c.to_lane == to)
                .map(|c| c.direction)
        };
        let turning = if lane.kind == LaneKind::Internal {
            self.trail
                .last()
                .and_then(|from| direction(*from, self.lane))
        } else {
            None
        };
        let mut next_turn = None;
        if turning.is_none() {
            // Walk the route ahead a short way: the first movement that is not straight on
            // is the one a driver would be signalling for.
            let mut prev = self
                .route
                .lanes
                .get(self.route_index)
                .copied()
                .unwrap_or(self.lane);
            let mut dist = (lane.length_m - self.s_m).max(0.0);
            for &next in self.route.lanes.iter().skip(self.route_index + 1).take(8) {
                let Some(next_lane) = world.try_lane(next) else {
                    break;
                };
                if let Some(d) = direction(prev, next)
                    && d != TurnDirection::Straight
                {
                    next_turn = Some((d, dist));
                    break;
                }
                dist += next_lane.length_m;
                if dist > 200.0 {
                    break;
                }
                prev = next;
            }
        }
        let lane_change_left = self.transition.map(|t| {
            let from = world.lane(t.from).index;
            let to = world.lane(t.to).index;
            // Index 0 is the rightmost lane, so a higher index is further left.
            to > from
        });
        crate::lamps::LampInput {
            class: self.class,
            speed_mps: self.speed_mps,
            accel_mps2: self.accel_mps2,
            lane_change_left,
            next_turn,
            turning,
        }
    }

    /// Where the vehicle is, as the crosswalk rules read it.
    fn crosswalk_path(&self) -> VehiclePath<'_> {
        VehiclePath {
            route: &self.route.lanes,
            route_index: self.route_index,
            lane: self.lane,
            s_m: self.s_m,
            prev_lane: self.trail.last().copied(),
            length_m: self.class.spec().length_m,
            speed_mps: self.speed_mps,
        }
    }

    fn view(&self, world: &World) -> VehicleView {
        let lane = world.lane(self.lane);
        VehicleView {
            actor: self.id,
            class: self.class,
            lane: self.lane,
            lane_index: lane.index,
            s_m: self.s_m,
            lateral_m: self.lateral_m,
            speed_mps: self.speed_mps,
            accel_mps2: self.accel_mps2,
            heading_rad: lane.heading_at(self.s_m),
            dims: self.class.dims(),
            driver: self.driver,
        }
    }

    /// The published ground truth: the reference point is the rear-axle centre, taken at
    /// the rear bumper (see the module documentation).
    ///
    /// `along_path` selects how the rear is placed when the body straddles two lanes.
    /// `false` is the legacy placement, which clamps the rear to the start of the front
    /// bumper's lane: on every lane boundary the published point jumped forward by up to a
    /// body length and then stood still until the front had driven that far, and the
    /// heading was the lane's at that clamped point. `true` — every mode but the legacy
    /// parity one — puts the rear where it is, on the lane the vehicle came from, and takes
    /// the heading along the body from rear to front, so the pose moves continuously across
    /// every boundary and turns smoothly through a junction.
    fn kinematics(&self, world: &World, t: SimTime, along_path: bool) -> Kinematics {
        let lane = world.lane(self.lane);
        let length = self.class.spec().length_m;
        let lateral_rate = self.lateral_rate();
        if !along_path {
            let s_rear = (self.s_m - length).clamp(0.0, lane.length_m);
            let heading = lane.heading_at(s_rear);
            let pos = lane.offset_point(s_rear, self.lateral_m);
            let (sin_h, cos_h) = math::sin_cos(heading);
            return Kinematics {
                t,
                pos,
                vel: Vec3::new(
                    self.speed_mps * cos_h - lateral_rate * sin_h,
                    self.speed_mps * sin_h + lateral_rate * cos_h,
                    0.0,
                ),
                acc: Vec3::new(self.accel_mps2 * cos_h, self.accel_mps2 * sin_h, 0.0),
                heading_rad: heading + lateral_heading_offset(lateral_rate, self.speed_mps),
                yaw_rate_rad_s: 0.0,
                lane: Some(LanePos::new(self.lane, s_rear, self.lateral_m)),
                dims: self.class.dims(),
            };
        }
        // The lateral speed the smoothstep actually has now — zero at both ends — rather
        // than its peak: publishing the peak for the whole change left a 19° heading
        // offset standing at the last step, which vanished in one step when the change
        // ended.
        let lateral_rate = self.lateral_rate_at(t);
        let s_front = self.s_m.clamp(0.0, lane.length_m);
        let s_rear = self.s_m - length;
        // A rider holding a lane position (a constant offset, no lane change under way):
        // the body is placed on the centreline and moved sideways along its own normal, so
        // the pose is as continuous as the centreline is. Offsetting each point along its
        // own lane's normal jumped by `d·Δψ` wherever two lanes meet at an angle — 4.5 cm
        // at a moped's rear crossing a joint, which the auditor saw as a step the speed did
        // not explain.
        if self.transition.is_none() && self.lateral_m != 0.0 {
            let front0 = smooth_offset_point(lane, s_front, 0.0);
            let (rear0, rear_lane, rear_s) = self.rear_point_at(world, s_rear, 0.0);
            let chord = Vec3::new(front0.x - rear0.x, front0.y - rear0.y, 0.0);
            let body = if chord.norm_2d() > 0.25 * length.max(0.1) {
                math::atan2(chord.y, chord.x)
            } else {
                lane.heading_at(s_rear.max(0.0))
            };
            let (sin_h, cos_h) = math::sin_cos(body);
            let d = self.lateral_m;
            let pos = Vec3::new(rear0.x - d * sin_h, rear0.y + d * cos_h, rear0.z);
            return Kinematics {
                t,
                pos,
                vel: Vec3::new(self.speed_mps * cos_h, self.speed_mps * sin_h, 0.0),
                acc: Vec3::new(self.accel_mps2 * cos_h, self.accel_mps2 * sin_h, 0.0),
                heading_rad: v2xw_world::model::normalise_angle(body),
                yaw_rate_rad_s: 0.0,
                lane: Some(LanePos::new(rear_lane, rear_s, self.lateral_m)),
                dims: self.class.dims(),
            };
        }
        let front = smooth_offset_point(lane, s_front, self.lateral_m);
        let (pos, rear_lane, rear_s) = self.rear_point(world, s_rear);
        let chord = Vec3::new(front.x - pos.x, front.y - pos.y, 0.0);
        let body = if chord.norm_2d() > 0.25 * length.max(0.1) {
            math::atan2(chord.y, chord.x)
        } else {
            lane.heading_at(s_rear.max(0.0))
        };
        let (sin_h, cos_h) = math::sin_cos(body);
        Kinematics {
            t,
            pos,
            vel: Vec3::new(
                self.speed_mps * cos_h - lateral_rate * sin_h,
                self.speed_mps * sin_h + lateral_rate * cos_h,
                0.0,
            ),
            acc: Vec3::new(self.accel_mps2 * cos_h, self.accel_mps2 * sin_h, 0.0),
            heading_rad: v2xw_world::model::normalise_angle(
                body + lateral_heading_offset(
                    lateral_rate,
                    if self.transition.is_some_and(|t| t.pull_out) {
                        self.speed_mps.max(PULL_OUT_SPEED_MPS)
                    } else {
                        self.speed_mps
                    },
                ),
            ),
            yaw_rate_rad_s: 0.0,
            lane: Some(LanePos::new(rear_lane, rear_s, self.lateral_m)),
            dims: self.class.dims(),
        }
    }

    /// Where the rear of the body is when it is `s_rear` along the front's lane — behind
    /// the lane's start when negative, which is walked back along the trail of lanes the
    /// vehicle came by. Past the end of the trail (a spawn, or a lane change right at a
    /// lane start) the first lane's first segment is extended backwards.
    fn rear_point(&self, world: &World, s_rear: f64) -> (Vec3, LaneId, f64) {
        self.rear_point_at(world, s_rear, self.lateral_m)
    }

    /// [`Self::rear_point`] at a lateral offset `lateral_m` rather than the vehicle's own.
    fn rear_point_at(&self, world: &World, s_rear: f64, lateral_m: f64) -> (Vec3, LaneId, f64) {
        let lane = world.lane(self.lane);
        if s_rear >= 0.0 {
            return (
                smooth_offset_point(lane, s_rear, lateral_m),
                self.lane,
                s_rear,
            );
        }
        let mut behind = -s_rear;
        let mut earliest = lane;
        for id in self.trail.iter().rev() {
            let Some(prev) = world.try_lane(*id) else {
                break;
            };
            if behind <= prev.length_m {
                let s = prev.length_m - behind;
                return (smooth_offset_point(prev, s, lateral_m), prev.id, s);
            }
            behind -= prev.length_m;
            earliest = prev;
        }
        let start = earliest.offset_point(0.0, lateral_m);
        let (sin0, cos0) = math::sin_cos(earliest.heading_at(0.0));
        (
            Vec3::new(start.x - behind * cos0, start.y - behind * sin0, start.z),
            earliest.id,
            0.0,
        )
    }

    /// The stretches of junction connector the body covers: `(connector, rear, front)` in
    /// arc length along each — the front's own lane if it is one, and every connector on
    /// the trail the body still overhangs.
    fn connector_spans(&self, world: &World) -> Vec<(LaneId, f64, f64)> {
        let mut out = Vec::new();
        let length = self.class.spec().length_m;
        let lane = world.lane(self.lane);
        if lane.kind == LaneKind::Internal {
            out.push((self.lane, self.s_m - length, self.s_m));
        }
        // How far the body reaches back past the start of the lane walked so far.
        let mut behind = length - self.s_m;
        let mut ahead_of_start = self.s_m;
        for id in self.trail.iter().rev() {
            if behind <= 0.0 {
                break;
            }
            let Some(prev) = world.try_lane(*id) else {
                break;
            };
            if prev.kind == LaneKind::Internal {
                out.push((
                    prev.id,
                    prev.length_m - behind,
                    prev.length_m + ahead_of_start,
                ));
            }
            behind -= prev.length_m;
            ahead_of_start += prev.length_m;
        }
        out
    }

    /// The lateral velocity the smoothstep transition has at `t`, m/s:
    /// `Δ · 6p(1 − p) / T` with `p` the elapsed fraction.
    fn lateral_rate_at(&self, t: SimTime) -> f64 {
        self.transition.map_or(0.0, |tr| {
            let dur = tr.duration.as_secs_f64();
            // Held while the vehicle stands (the lateral pass of the step): no sideways
            // motion, so no heading swing either.
            if dur <= 0.0 || (tr.switched && tr.began_moving && self.speed_mps <= STANDSTILL_MPS) {
                return 0.0;
            }
            let p = (ns_to_secs(t.saturating_sub(tr.started)) / dur).clamp(0.0, 1.0);
            (tr.to_offset_m - tr.from_offset_m) * 6.0 * p * (1.0 - p) / dur
        })
    }

    /// The lateral velocity the transition is producing, m/s.
    fn lateral_rate(&self) -> f64 {
        self.transition.map_or(0.0, |t| {
            let dur = t.duration.as_secs_f64();
            if dur <= 0.0 {
                0.0
            } else {
                // The smoothstep's peak lateral velocity, which is what a heading deviation
                // of a few degrees comes from.
                1.5 * (t.to_offset_m - t.from_offset_m) / dur
            }
        })
    }
}

/// How far beyond a body's ends another body on the lane beside must be for the body to
/// finish a lane change into that lane while standing, metres. **A choice**: half the
/// 2 m standstill gap (IDM `s0`) on each side.
const BESIDE_CLEAR_M: f64 = 1.0;

/// Whether no body but `me`'s — a vehicle's or a ghost's (a vehicle part-way out of
/// `lane`) — is on `lane` within [`BESIDE_CLEAR_M`] of the stretch `[s_m - length_m, s_m]`.
fn lane_clear_beside(
    snapshot: &ActorSnapshot,
    me: ActorId,
    lane: LaneId,
    s_m: f64,
    length_m: f64,
) -> bool {
    snapshot.on_lane(lane).iter().all(|(front, other)| {
        if *other == me {
            return true;
        }
        let other_len = snapshot.view(*other).map_or(length_m, |v| v.dims.length_m);
        *front + BESIDE_CLEAR_M < s_m - length_m || *front - other_len - BESIDE_CLEAR_M > s_m
    })
}

/// The heading deviation a lateral velocity produces at a longitudinal speed.
fn lateral_heading_offset(lateral_rate: f64, speed_mps: f64) -> f64 {
    if lateral_rate == 0.0 {
        0.0
    } else {
        math::atan2(lateral_rate, speed_mps.max(0.5))
    }
}

/// The native medium-tier mobility engine.
pub struct NativeMobility {
    params: EngineParams,
    cf: Arc<dyn CarFollowing + Send + Sync>,
    /// Vehicles whose stated vehicle type names its own car-following model (an imported
    /// `.rou.xml` vType's `carFollowModel`, WP-15), with that model; every other vehicle
    /// drives `cf`.
    vtype_cf: BTreeMap<ActorId, Arc<dyn CarFollowing + Send + Sync>>,
    /// One instance per named model, shared by the vehicles of every type that names it.
    vtype_models: BTreeMap<&'static str, Arc<dyn CarFollowing + Send + Sync>>,
    lane_change: Option<Mobil>,
    gap: GapAcceptance,
    signals: FixedTimeSignals,
    two_coloring: Option<TwoColoring>,
    router: DynamicReroute,
    /// Routes riders over the lanes that admit bicycles.
    bike_router: DynamicReroute,
    /// One router per motor class's own lane-access bit ([`EngineParams::class_routing`]),
    /// keyed by the bit; empty when class routing is off.
    class_routers: BTreeMap<u16, DynamicReroute>,
    demand: Option<Box<dyn Demand>>,
    /// The VRU population to keep.
    vru_population: VruPopulation,
    /// The walkable lanes, and the lanes a bicycle may use, in id order (from `init`).
    walkable: Vec<LaneId>,
    bike_lanes: Vec<LaneId>,
    /// The world's crosswalks, for the yield rule and the pedestrians' kerb decision
    /// (`crate::vru::crosswalk`). Built in `init`.
    crosswalks: CrosswalkIndex,
    /// Each junction connector's siblings from the same approach lane, and where they
    /// part (`diverging_connectors`). Built in `init`.
    diverging: BTreeMap<LaneId, Vec<(LaneId, f64, f64)>>,
    vru: Option<SocialForce>,
    /// Whether each vehicle that met a pedestrian waiting mid-block yields to them, drawn
    /// once per pair: `(pedestrian, vehicle) → yields`.
    midblock_yields: BTreeMap<(ActorId, ActorId), bool>,
    clock: Option<Box<dyn ClockModel>>,
    actors: BTreeMap<ActorId, Actor>,
    published: BTreeMap<ActorId, Kinematics>,
    closed: BTreeSet<LaneId>,
    /// Lanes a coupled tier has no room on ([`NativeMobility::set_holds`]): stopped short
    /// of like a closure, never detoured round. Empty unless a tier coupling sets it.
    holds: BTreeSet<LaneId>,
    /// How many times a closure command has changed [`Self::closed`]: the router's cost
    /// generation ([`DynamicCost::with_generation`]). Counting the closed lanes instead
    /// missed a reopen and a close in the same step.
    closure_changes: u64,
    /// The blocked lanes each driver's own unit has told it about
    /// ([`MobilityCommand::HazardHeard`]), not yet acted on and acted on: its belief, kept
    /// for the rest of its trip. Read only under [`RerouteSource::V2x`].
    heard_hazards: BTreeMap<ActorId, (BTreeSet<LaneId>, bool)>,
    /// Every actor that has re-planned on information (a periodic congestion re-plan, a
    /// closure beyond its sight, a warning it received): what `reroute_share` and
    /// `reroute_source` gate ([`NativeMobility::rerouted_on_information`]).
    info_rerouted: BTreeSet<ActorId>,
    pending: Vec<MobilityCommand>,
    /// Trips whose origin had no room when they arrived, with the instant they first
    /// asked, oldest first (not in the legacy parity mode).
    waiting: Vec<(TripRequest, SimTime, Option<ActorId>)>,
    weather: WeatherState,
    /// Every junction's conflict zones, built at `init` (not in the legacy parity mode).
    zones: ConflictZones,
    /// The speed each junction connector may be taken at, from its curvature.
    turn_speed: BTreeMap<LaneId, f64>,
    /// The bends of through lanes with a turn speed under the lane's limit, as
    /// `(from_s_m, to_s_m, speed_mps)` along each lane ([`EngineParams::lane_bend_speed`]).
    bend_speed: BTreeMap<LaneId, Vec<(f64, f64, f64)>>,
    next_actor: u32,
    next_seq: u64,
    dropped_trips: u64,
    /// Of [`Self::dropped_trips`], the trips with no route from their origin or no room on it.
    unroutable_trips: u64,
    /// What the gridlock resolution did ([`EngineParams::time_to_teleport_s`]).
    gridlock: GridlockCounters,
    /// Each lane's smoothed speed, m/s, for congestion-aware routing
    /// ([`EngineParams::reroute_period_s`]); a lane at its free speed is not kept.
    lane_speed: BTreeMap<LaneId, f64>,
    /// When the lane speeds were last sampled.
    speed_sampled_at: Option<SimTime>,
    card: ModelCard,
    /// The exterior lamp model's parameters ([`crate::lamps`]).
    lamp_params: crate::lamps::LampParams,
    /// A researcher's passenger-car driver (`actors.vehicles.drivers`): each set field
    /// replaces the car-following model's own calibration for [`VehicleClass::Passenger`].
    driver_override: DriverOverride,
    /// Per-class car-following numbers set by hand ([`NativeMobility::with_class_override`]),
    /// applied over the class's own profile; empty for a scenario that sets none.
    class_overrides: Vec<(VehicleClass, DriverOverride)>,
    /// Per vehicle: the lamps shown last step (for the brake hysteresis) and the driver's
    /// signalling habit, drawn the first time the vehicle is asked about.
    lamp_memory: BTreeMap<ActorId, (u8, crate::lamps::SignalHabit)>,
    /// The sub-lane lateral model, when one is selected ([`crate::lateral`]): every
    /// vehicle then steers a continuous position across its lane and the leader search
    /// skips a same-lane vehicle it may lawfully pass. `None` keeps lane-based driving.
    lateral: Option<Arc<dyn crate::lateral::LateralModel>>,
}

impl core::fmt::Debug for NativeMobility {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NativeMobility")
            .field("params", &self.params)
            .field("car_following", &self.cf.card().id)
            .field("actors", &self.actors.len())
            .field("closed_lanes", &self.closed.len())
            .finish()
    }
}

impl NativeMobility {
    /// The engine with the default medium-tier models: IDM (Kesting 2010), MOBIL
    /// (Kesting 2007), HCM gap acceptance, fixed-time signals and Dijkstra.
    pub fn new(params: EngineParams) -> Self {
        // City drivers, calibrated to the HCM saturation flow (`IdmPreset::UrbanHcm`).
        let idm: Arc<dyn CarFollowing + Send + Sync> = Arc::new(Idm::new(IdmPreset::UrbanHcm));
        Self::with_models(params, idm, MobilPreset::Kesting2007)
    }

    /// The legacy parity configuration: the legacy IDM and MOBIL presets, the two-colouring
    /// signal abstraction and no dynamic rerouting.
    pub fn legacy(params: EngineParams) -> Self {
        let idm: Arc<dyn CarFollowing + Send + Sync> = Arc::new(Idm::new(IdmPreset::Legacy));
        Self::with_models(
            EngineParams {
                intersections: IntersectionMode::TwoColoringLegacy,
                dynamic_rerouting: false,
                ..params
            },
            idm,
            MobilPreset::Legacy,
        )
    }

    /// The engine with a car-following model and a lane-change preset of the caller's
    /// choosing.
    pub fn with_models(
        params: EngineParams,
        cf: Arc<dyn CarFollowing + Send + Sync>,
        lane_change: MobilPreset,
    ) -> Self {
        let mobil_params = lane_change.params();
        Self::with_lane_change_params(params, cf, lane_change, mobil_params)
    }

    /// The engine with a lane-change parameter set of the caller's own, labelled by the
    /// preset it derives from.
    ///
    /// `step_s` is overwritten from [`EngineParams::step`] whatever the caller passed, so
    /// the reconsideration probability and the engine's own step cannot disagree.
    pub fn with_lane_change_params(
        params: EngineParams,
        cf: Arc<dyn CarFollowing + Send + Sync>,
        lane_change: MobilPreset,
        mut mobil_params: MobilParams,
    ) -> Self {
        mobil_params.step_s = params.step.as_secs_f64();
        let mobil = Mobil::with_params(lane_change, mobil_params, Arc::clone(&cf));
        let router_params = DijkstraParams::for_classes(params.classes);
        let make_router = |p: DijkstraParams| {
            if params.dynamic_rerouting {
                DynamicReroute::on_change(p)
            } else {
                DynamicReroute::new(p, crate::views::ReroutePolicy::STATIC)
            }
        };
        let router = make_router(router_params);
        let class_routers: BTreeMap<u16, DynamicReroute> = if params.class_routing
            && params.intersections != IntersectionMode::TwoColoringLegacy
        {
            [
                ClassMask::CAR,
                ClassMask::TRUCK,
                ClassMask::BUS,
                ClassMask::MOTO,
                ClassMask::EMERGENCY,
            ]
            .into_iter()
            .filter(|m| params.classes.contains_any(*m))
            .map(|m| (m.bits(), make_router(DijkstraParams::for_classes(m))))
            .collect()
        } else {
            BTreeMap::new()
        };
        Self {
            card: card(&params, &cf.card().id),
            params,
            cf,
            vtype_cf: BTreeMap::new(),
            vtype_models: BTreeMap::new(),
            lane_change: params.lane_changes.then_some(mobil),
            gap: GapAcceptance::new(GapAcceptanceParams {
                gap_factor: params.critical_gap_factor,
                heavy_vehicle_terms: params.heavy_vehicle_gap_terms,
                patience: params.junction_patience,
                ..GapAcceptanceParams::default()
            }),
            signals: FixedTimeSignals::new(SignalPlanParams {
                right_turn_on_red: params.right_turn_on_red,
                gap_factor: params.critical_gap_factor,
                permitted_left_critical_headway_s: params.permitted_left_critical_headway_s,
                heavy_vehicle_terms: params.heavy_vehicle_gap_terms,
                patience: params.junction_patience,
                sneakers: params.permissive_sneakers,
                ..SignalPlanParams::default()
            }),
            two_coloring: None,
            router,
            bike_router: DynamicReroute::new(
                DijkstraParams::for_classes(ClassMask::BICYCLE),
                crate::views::ReroutePolicy::STATIC,
            ),
            class_routers,
            demand: None,
            vru_population: VruPopulation::default(),
            walkable: Vec::new(),
            bike_lanes: Vec::new(),
            crosswalks: CrosswalkIndex::default(),
            diverging: BTreeMap::new(),
            vru: None,
            midblock_yields: BTreeMap::new(),
            clock: None,
            actors: BTreeMap::new(),
            published: BTreeMap::new(),
            closed: BTreeSet::new(),
            holds: BTreeSet::new(),
            closure_changes: 0,
            heard_hazards: BTreeMap::new(),
            info_rerouted: BTreeSet::new(),
            pending: Vec::new(),
            waiting: Vec::new(),
            weather: WeatherState::CLEAR,
            zones: ConflictZones::default(),
            turn_speed: BTreeMap::new(),
            bend_speed: BTreeMap::new(),
            next_actor: 0,
            next_seq: 0,
            dropped_trips: 0,
            unroutable_trips: 0,
            gridlock: GridlockCounters::default(),
            lane_speed: BTreeMap::new(),
            speed_sampled_at: None,
            lamp_params: crate::lamps::LampParams::default(),
            driver_override: DriverOverride::default(),
            class_overrides: Vec::new(),
            lamp_memory: BTreeMap::new(),
            lateral: None,
        }
    }

    /// The same engine with a passenger-car driver of the caller's: `a`, `b`, `T` and
    /// `s0` replace the car-following model's calibration for cars, before the per-driver
    /// style and heterogeneity draws, so the population keeps its spread around them.
    #[must_use]
    pub fn with_class_override(mut self, class: VehicleClass, o: DriverOverride) -> Self {
        self.class_overrides.retain(|(c, _)| *c != class);
        if o != DriverOverride::default() {
            self.class_overrides.push((class, o));
        }
        self
    }

    /// Sets a passenger-car driver's car-following numbers by hand.
    pub fn with_driver_override(mut self, o: DriverOverride) -> Self {
        self.driver_override = o;
        self
    }

    /// The same engine with these exterior-lamp parameters ([`crate::lamps`]).
    #[must_use]
    pub fn with_lamp_params(mut self, params: crate::lamps::LampParams) -> Self {
        self.lamp_params = params;
        self
    }

    /// Every vehicle's exterior lamps after the last step, in actor-id order
    /// ([`crate::lamps`], vwp-v1 §3.3.5). Headlamps are the caller's.
    ///
    /// Read-only with respect to the traffic: the lamps are an output, nothing in a step
    /// reads them, so asking for them — or not — changes no position. The memory kept is
    /// the lamps shown last time (the brake hysteresis) and each driver's signalling
    /// habit, drawn on first sight from the driver's own keyed stream.
    pub fn exterior_lamps(&mut self, ctx: &mut dyn MobCtx) -> Vec<(ActorId, u8)> {
        let world = ctx.world();
        let p = self.lamp_params;
        let mut previous = core::mem::take(&mut self.lamp_memory);
        let mut out = Vec::with_capacity(self.actors.len());
        for a in self.actors.values() {
            let (last, habit) = previous.remove(&a.id).unwrap_or_else(|| {
                let key = EntityRef::custom(LAMP_HABIT_ID, a.seq);
                let mut rng = ctx.rng(RngDomain::ReactionTime, key);
                let turns = rng.uniform(0.0, 1.0) < p.turn_signal_use;
                let lane_changes = rng.uniform(0.0, 1.0) < p.lane_change_signal_use;
                (
                    0,
                    crate::lamps::SignalHabit {
                        turns,
                        lane_changes,
                    },
                )
            });
            let bits = crate::lamps::lamps(&a.lamp_input(world), last, habit, &p);
            self.lamp_memory.insert(a.id, (bits, habit));
            out.push((a.id, bits));
        }
        out
    }

    /// Adds a sub-lane lateral model ([`crate::lateral`]).
    #[must_use]
    pub fn with_lateral(mut self, lateral: Arc<dyn crate::lateral::LateralModel>) -> Self {
        self.lateral = Some(lateral);
        self
    }

    /// Adds a pedestrian model, which is stepped with the same frozen snapshot the
    /// vehicles read.
    #[must_use]
    pub fn with_vru(mut self, vru: SocialForce) -> Self {
        self.vru = Some(vru);
        self
    }

    /// Keeps `population` vulnerable road users in the world. Pedestrians need the
    /// social-force model, which is installed if it is not already.
    #[must_use]
    pub fn with_vru_population(mut self, population: VruPopulation) -> Self {
        if population.pedestrians > 0 && self.vru.is_none() {
            self.vru = Some(SocialForce::default());
        }
        self.vru_population = population;
        self
    }

    /// The router for a class: riders on the bicycle lane graph, every other class on the
    /// lanes its own class may use ([`EngineParams::class_routing`]), or the engine's own.
    fn router_for(&self, class: VehicleClass) -> &DynamicReroute {
        router_of(&self.router, &self.bike_router, &self.class_routers, class)
    }

    /// A route for a vehicle of `class` from `from` to `to`: on its own class's lanes, and
    /// on the engine-wide lanes only when its own class has no route (a trip that starts or
    /// ends on a lane its class is not signed for).
    fn plan_route(
        &self,
        class: VehicleClass,
        world: &World,
        from: LaneId,
        to: LaneId,
        t: SimTime,
        costs: &dyn crate::views::EdgeCost,
    ) -> Option<Route> {
        let own = self.router_for(class);
        own.replan(world, from, to, t, costs).or_else(|| {
            if class != VehicleClass::Bicycle && !core::ptr::eq(own, &self.router) {
                self.router.replan(world, from, to, t, costs)
            } else {
                None
            }
        })
    }

    /// [`Self::plan_route`] with strategic lane choice ([`EngineParams::strategic_lane_choice`]):
    /// the route from each other lane of `from`'s road that the vehicle still has room to
    /// reach from `s_m`, at [`STRATEGIC_CHANGE_COST_S`] per lane crossed, against its own
    /// lane's. The winner is returned as `[from, <the route from the chosen lane, after its
    /// first lane>]`: the next lane is then one the chosen lane reaches, which the decision
    /// pass reads as a lane change to make ([`Self::strategic_pending`]).
    #[allow(clippy::too_many_arguments)]
    fn plan_route_strategic(
        &self,
        class: VehicleClass,
        world: &World,
        from: LaneId,
        s_m: f64,
        to: LaneId,
        t: SimTime,
        costs: &dyn crate::views::EdgeCost,
    ) -> Option<Route> {
        let own = self.plan_route(class, world, from, to, t, costs);
        if !(self.params.strategic_lane_choice && self.params.lane_changes && self.along_path()) {
            return own;
        }
        let lane = world.try_lane(from)?;
        if lane.kind == LaneKind::Internal || from == to {
            return own;
        }
        let room = lane.length_m - s_m;
        let mut best: Option<(f64, Route)> = own.clone().map(|r| (r.cost_s, r));
        let mut siblings: Vec<LaneId> = world.edge(lane.edge).lanes.clone();
        siblings.sort_unstable();
        for other in siblings {
            if other == from {
                continue;
            }
            let o = world.lane(other);
            if !o.kind.is_motorised() || !o.admits(class.class_mask()) {
                continue;
            }
            let crossed = f64::from(o.index.abs_diff(lane.index));
            if room < self.params.no_change_zone_m + crossed * STRATEGIC_M_PER_CHANGE {
                continue;
            }
            let Some(r) = self.plan_route(class, world, other, to, t, costs) else {
                continue;
            };
            if r.lanes.len() < 2 {
                continue;
            }
            let cost = r.cost_s + crossed * STRATEGIC_CHANGE_COST_S;
            if best.as_ref().is_none_or(|(c, _)| cost < *c) {
                let mut lanes = vec![from];
                lanes.extend(r.lanes.iter().skip(1).copied());
                best = Some((cost, Self::route_of(world, lanes)));
            }
        }
        best.map(|(_, r)| r)
    }

    /// The distance to the end of its lane, metres, of a vehicle whose route's next lane
    /// leaves from another lane of its road: the strategic change it has still to make.
    /// `None` when the next lane follows from its own (or the change is under way).
    fn strategic_pending(&self, world: &World, actor: &Actor) -> Option<f64> {
        self.strategic_target(world, actor)?;
        let lane = world.lane(actor.lane);
        Some((lane.length_m - actor.s_m).max(0.0))
    }

    /// The lane of its road a vehicle has to change to for its route's next lane
    /// ([`Self::strategic_pending`]).
    fn strategic_target(&self, world: &World, actor: &Actor) -> Option<LaneId> {
        if !self.params.strategic_lane_choice || actor.transition.is_some() {
            return None;
        }
        let next = *actor.route.lanes.get(actor.route_index + 1)?;
        let reaches = |l: LaneId| {
            world
                .successors(l)
                .iter()
                .any(|c| c.permitted && c.via.unwrap_or(c.to_lane) == next)
        };
        if reaches(actor.lane) {
            return None;
        }
        let lane = world.try_lane(actor.lane)?;
        if lane.kind == LaneKind::Internal {
            return None;
        }
        world
            .edge(lane.edge)
            .lanes
            .iter()
            .copied()
            .filter(|l| *l != actor.lane)
            .find(|l| reaches(*l))
    }

    /// The lane change towards [`Self::strategic_target`], if the lane beside on that
    /// side has room now: the same acceptance as a move to the kerb
    /// ([`Self::move_to_kerb`]), on either side.
    fn strategic_change(
        &self,
        world: &World,
        mobil: &Mobil,
        actor: &Actor,
        ego: &VehicleView,
        nbrs: &LaneNeighbors,
    ) -> Option<LaneChangeDecision> {
        let target = self.strategic_target(world, actor)?;
        let side = if world.lane(target).index > world.lane(actor.lane).index {
            Side::Left
        } else {
            Side::Right
        };
        let beside = nbrs.side(side)?;
        let beside_lane = world.try_lane(beside.lane)?;
        if beside_lane.edge != world.lane(actor.lane).edge || !beside_lane.kind.is_motorised() {
            return None;
        }
        let lane_view = LaneView::of(beside_lane);
        if let Some(l) = beside.leader {
            if l.gap_m < ego.driver.min_gap_m {
                return None;
            }
            let mut moved = *ego;
            moved.lane = beside.lane;
            if VehicleCf(&self.cf, &self.vtype_cf).accel(&moved, Some(&l), &lane_view, &self.weather)
                < -PULL_OUT_FOLLOWER_DECEL_MPS2
            {
                return None;
            }
        }
        if let Some(f) = beside.follower
            && let Some(fv) = f.vehicle.as_ref()
        {
            if f.gap_m < fv.driver.min_gap_m {
                return None;
            }
            let a = VehicleCf(&self.cf, &self.vtype_cf).accel(
                fv,
                Some(&LeaderView::of(*ego, f.gap_m)),
                &lane_view,
                &self.weather,
            );
            if a < -PULL_OUT_FOLLOWER_DECEL_MPS2 {
                return None;
            }
        }
        let lateral = 0.5 * (world.lane(actor.lane).width_m + beside_lane.width_m);
        Some(LaneChangeDecision::Change {
            to: beside.lane,
            side,
            incentive_mps2: 0.0,
            duration: crate::lanechange::mobil::transition_duration(
                mobil.params(),
                lateral,
                ego.speed_mps.max(PULL_OUT_SPEED_MPS),
            ),
        })
    }

    /// How far a vehicle can stay on `lane` (one of its road's lanes) and still follow its
    /// route, metres: to the end of the lane when it cannot reach the route's next lane,
    /// else on along the route, up to SUMO's 3,000 m look-ahead (`MSVehicle::updateBestLanes`).
    ///
    /// As in SUMO's best-lanes, a lane continues the route when it connects to the route's
    /// next *road*, on whichever of its lanes: the route's next lane is only the one its
    /// present lane leads to, and a change re-plans the route from the new lane
    /// ([`Self::resolve_lane_changes`]). Matching the exact next lane made every side lane
    /// of a multi-lane street look like a dead end, so the 20 s remain-time rule refused
    /// almost every speed-gain change on a street network.
    fn lane_route_dist(&self, world: &World, actor: &Actor, lane: LaneId) -> (f64, bool) {
        let rem = (world.lane(lane).length_m - actor.s_m).max(0.0);
        let Some(next_edge) = actor.route.lanes
            [(actor.route_index + 1).min(actor.route.lanes.len())..]
            .iter()
            .filter_map(|l| world.try_lane(*l))
            .find(|l| l.kind != LaneKind::Internal)
            .map(|l| l.edge)
        else {
            return (rem, true);
        };
        let reaches = world
            .successors(lane)
            .iter()
            .any(|c| c.permitted && world.lane(c.to_lane).edge == next_edge);
        if !reaches {
            return (rem, false);
        }
        let mut d = rem;
        for l in &actor.route.lanes[actor.route_index + 1..] {
            d += world.lane(*l).length_m;
            if d >= 3000.0 {
                break;
            }
        }
        (d, true)
    }

    /// What each side offers an LC2013 driver ([`crate::lanechange::lc2013::SideOffer`]),
    /// with whether that lane keeps the route.
    fn lc2013_offer(
        &self,
        world: &World,
        actor: &Actor,
        ego: &VehicleView,
        nbrs: &LaneNeighbors,
        side: Side,
        v_max: f64,
    ) -> Option<(crate::lanechange::lc2013::SideOffer, bool)> {
        use crate::lanechange::lc2013::{DECEL_MPS2, SideOffer, lane_v_safe};
        let sn = nbrs.side(side)?;
        let l = world.try_lane(sn.lane)?;
        if l.edge != world.lane(actor.lane).edge
            || !l.kind.is_motorised()
            || !l.admits(actor.class.class_mask())
        {
            return None;
        }
        let (dist_m, on_route) = self.lane_route_dist(world, actor, sn.lane);
        Some((
            SideOffer {
                v_safe: lane_v_safe(
                    v_max.min(l.speed_limit_mps),
                    sn.leader.as_ref(),
                    DECEL_MPS2,
                    ego.driver.min_gap_m,
                ),
                dist_m,
                leader: sn.leader,
            },
            on_route,
        ))
    }

    /// Whether a strategic change is due under LC2013: within the look-ahead
    /// ([`crate::lanechange::lc2013::Lc2013Params::look_ahead_m`]), or further out when the
    /// target lane is no slower than the own (`changeToBest`).
    ///
    /// The distance left is shortened by the length of the vehicles already standing on
    /// the target lane (a queue there leaves less room to merge; Erdmann 2015 §3, strategic
    /// urgency, with SUMO's documented behaviour as the check, its code not used) and
    /// shared out over the lanes still to cross. Without the occupation a car slowing into the
    /// queue at a red light had its look-ahead shrink to two car lengths and never made the
    /// change before the no-change zone.
    #[allow(clippy::too_many_arguments)]
    fn lc2013_strategic_due(
        &self,
        world: &World,
        p: &crate::lanechange::lc2013::Lc2013Params,
        st: &crate::lanechange::lc2013::Lc2013State,
        actor: &Actor,
        ego: &VehicleView,
        nbrs: &LaneNeighbors,
        occupation: &BTreeMap<LaneId, f64>,
    ) -> bool {
        use crate::lanechange::lc2013::{DECEL_MPS2, lane_v_safe};
        let (Some(target), Some(pending)) = (
            self.strategic_target(world, actor),
            self.strategic_pending(world, actor),
        ) else {
            return true;
        };
        let side = if world.lane(target).index > world.lane(actor.lane).index {
            Side::Left
        } else {
            Side::Right
        };
        let la = p.look_ahead_m(st, ego.dims.length_m + ego.driver.min_gap_m, side);
        let crossed = f64::from(
            world
                .lane(target)
                .index
                .abs_diff(world.lane(actor.lane).index)
                .max(1),
        );
        let usable = pending - occupation.get(&target).copied().unwrap_or(0.0);
        if usable / crossed < la {
            return true;
        }
        let lane = world.lane(actor.lane);
        let v_max = ego.driver.desired_speed_mps.min(lane.speed_limit_mps);
        let this_v = lane_v_safe(
            v_max,
            nbrs.leader.as_ref(),
            DECEL_MPS2,
            ego.driver.min_gap_m,
        );
        self.lc2013_offer(world, actor, ego, nbrs, side, v_max)
            .is_some_and(|(o, _)| o.v_safe >= this_v)
    }

    /// LC2013's discretionary decision ([`crate::lanechange::lc2013`]): cooperation first,
    /// then keep right and speed gain, each through MOBIL's safety criterion.
    #[allow(clippy::too_many_arguments)]
    fn lc2013_decide(
        &self,
        world: &World,
        mobil: &Mobil,
        p: &crate::lanechange::lc2013::Lc2013Params,
        st: &mut crate::lanechange::lc2013::Lc2013State,
        actor: &Actor,
        ego: &VehicleView,
        nbrs: &LaneNeighbors,
        dt: f64,
    ) -> LaneChangeDecision {
        use crate::lanechange::lc2013::{DECEL_MPS2, TAU_S, TURN_LANE_DIST_M, lane_v_safe};
        let lane = world.lane(actor.lane);
        let v_max = ego.driver.desired_speed_mps.min(lane.speed_limit_mps);
        let s0 = ego.driver.min_gap_m;
        let this_v = lane_v_safe(v_max, nbrs.leader.as_ref(), DECEL_MPS2, s0);
        let right = self.lc2013_offer(world, actor, ego, nbrs, Side::Right, v_max);
        let left = self.lc2013_offer(world, actor, ego, nbrs, Side::Left, v_max);
        let change = |side: Side| -> LaneChangeDecision {
            let Some(sn) = nbrs.side(side) else {
                return LaneChangeDecision::Stay;
            };
            let w = world.lane(sn.lane).width_m;
            LaneChangeDecision::Change {
                to: sn.lane,
                side,
                incentive_mps2: 0.0,
                duration: crate::lanechange::mobil::transition_duration(
                    mobil.params(),
                    0.5 * (lane.width_m + w),
                    ego.speed_mps.max(PULL_OUT_SPEED_MPS),
                ),
            }
        };
        // Cooperation: make room for a neighbour whose urgent strategic change needs this
        // lane and which this vehicle blocks as its target-lane follower.
        if p.cooperative > 0.0 {
            for (blocked, out) in [(Side::Left, Side::Right), (Side::Right, Side::Left)] {
                let Some(l) = nbrs.side(blocked).and_then(|sn| sn.leader) else {
                    continue;
                };
                let Some(m) = l.vehicle.and_then(|v| self.actors.get(&v.actor)) else {
                    continue;
                };
                if self.strategic_target(world, m) != Some(actor.lane) {
                    continue;
                }
                let Some(pending) = self.strategic_pending(world, m) else {
                    continue;
                };
                let towards = match blocked {
                    Side::Left => Side::Right,
                    Side::Right => Side::Left,
                };
                let m_len = m.class.spec().length_m + m.driver.min_gap_m;
                if pending > p.look_ahead_m(&m.lc, m_len, towards) {
                    continue;
                }
                let v = ego.speed_mps;
                let secure = (v * TAU_S + (v * v - l.speed_mps * l.speed_mps) / (2.0 * DECEL_MPS2))
                    .max(0.0)
                    + s0;
                if l.gap_m >= secure {
                    continue;
                }
                let offer = match out {
                    Side::Left => left,
                    Side::Right => right,
                };
                let Some((o, true)) = offer else {
                    continue;
                };
                if p.inconvenience(st, out, this_v, o.v_safe) > p.cooperative {
                    continue;
                }
                if mobil
                    .incentive(world, ego, nbrs, out, &self.weather)
                    .is_none()
                {
                    continue;
                }
                return change(out);
            }
        }
        let wanted = p.step(
            st,
            dt,
            ego.speed_mps,
            v_max,
            lane.speed_limit_mps,
            DECEL_MPS2,
            this_v,
            right.map(|(o, _)| o),
            left.map(|(o, _)| o),
        );
        let Some((side, _)) = wanted else {
            return LaneChangeDecision::Stay;
        };
        // Strategic vetoes on a discretionary change (Erdmann 2015: a tactical change must
        // not defeat the route): away from a lane change the route still needs only when
        // the room left, shared over the lanes to cross plus two (out and back), still
        // exceeds the look-ahead; and off a lane that keeps the route onto one that does
        // not only when that lane runs on for `TURN_LANE_DIST` = 200 m and twice the
        // look-ahead, so that the driver has time to change back.
        let len_gap = ego.dims.length_m + ego.driver.min_gap_m;
        if let Some(target) = self.strategic_target(world, actor) {
            let to_target = if world.lane(target).index > lane.index {
                Side::Left
            } else {
                Side::Right
            };
            if side != to_target {
                let crossed = f64::from(world.lane(target).index.abs_diff(lane.index).max(1));
                let room = (lane.length_m - actor.s_m).max(0.0);
                if room / (crossed + 2.0) < p.look_ahead_m(st, len_gap, to_target) {
                    return LaneChangeDecision::Stay;
                }
            }
        } else {
            let offer = match side {
                Side::Left => left,
                Side::Right => right,
            };
            if let Some((o, false)) = offer
                && (o.dist_m < TURN_LANE_DIST_M
                    || 2.0 * o.dist_m < p.look_ahead_m(st, len_gap, side))
            {
                return LaneChangeDecision::Stay;
            }
        }
        if mobil
            .incentive(world, ego, nbrs, side, &self.weather)
            .is_some()
        {
            change(side)
        } else {
            LaneChangeDecision::Stay
        }
    }

    /// The acceleration cap of a vehicle whose strategic lane change is blocked by the
    /// vehicle ahead in the target lane: it drops back behind it, braking no harder than
    /// its comfortable deceleration. `None` when that lane has no such blocker.
    ///
    /// Without it two cars side by side at the same speed stayed side by side to the end
    /// of the lane, and the one that needed the other's lane missed its turn: on the SUMO
    /// cross-validation grid at 600 veh/h a car drove 130 m of an empty road level with
    /// the car 1.5 m ahead in its target lane and was removed with no route left. SUMO's
    /// LC2013 does the same: a vehicle that cannot overtake the target-lane leader in the
    /// space left slows down smoothly to follow it (Erdmann 2015, the blocked strategic
    /// change; SUMO's behaviour read only, its code not used).
    ///
    /// The other half of LC2013's courtesy, the target-lane follower braking to let an
    /// urgent change in, is not modelled: tried, it held the follower
    /// and the queue behind it back across the junction upstream, and on the Manhattan
    /// extract at 3600 veh/h spill-back locked junctions (trips completed 327 against
    /// SUMO Krauss's 368, 69 vehicles left against 32; without it 365 and 35), while on
    /// the grid it changed nothing the barrier at the lane end does not already do
    /// (648 trips completed either way, SUMO 648).
    fn strategic_fall_back(
        &self,
        world: &World,
        actor: &Actor,
        ego: &VehicleView,
        nbrs: &LaneNeighbors,
    ) -> Option<f64> {
        // Only once the change is urgent. Braking for every blocked change from the start of the
        // lane, and dropping behind a target-lane follower level with the car as well as
        // behind the leader, took the highway-congestion merge to a standstill
        // (`traffic_audit`: overlaps 1750 -> 2988 vehicle-steps against 500cbb0, 143 cars
        // standing 180 s); urgent and leader-only, 923 and none.
        if self.strategic_pending(world, actor)? > self.params.no_change_zone_m + URGENT_LOOK_M {
            return None;
        }
        let target = self.strategic_target(world, actor)?;
        let side = if world.lane(target).index > world.lane(actor.lane).index {
            Side::Left
        } else {
            Side::Right
        };
        let beside = nbrs.side(side)?;
        let beside_lane = world.try_lane(beside.lane)?;
        if beside_lane.edge != world.lane(actor.lane).edge {
            return None;
        }
        let l = beside.leader?;
        let mut moved = *ego;
        moved.lane = beside.lane;
        let a = self
            .cf
            .accel(&moved, Some(&l), &LaneView::of(beside_lane), &self.weather);
        let blocked = l.gap_m < ego.driver.min_gap_m || a < -PULL_OUT_FOLLOWER_DECEL_MPS2;
        blocked.then(|| a.max(-ego.driver.comfort_decel_mps2))
    }

    /// Adds a clock model. The engine does not read it — believed time belongs to the node
    /// runtime — but holding it here keeps a scenario's mobility-side model set in one
    /// place, and it is on the manifest through this model's card.
    #[must_use]
    pub fn with_clock(mut self, clock: Box<dyn ClockModel>) -> Self {
        self.clock = Some(clock);
        self
    }

    /// True unless the engine runs the legacy parity configuration, whose placement,
    /// integration and junction rules are kept exactly as the reference engine had them.
    fn along_path(&self) -> bool {
        self.params.intersections != IntersectionMode::TwoColoringLegacy
    }

    /// Where in its lane a vehicle of `class` rides when not changing lanes: metres left
    /// of the centreline ([`TwoWheelerParams::lane_position_m`] for a motorcycle or a
    /// moped, the centreline for everyone else and in the legacy parity mode).
    pub fn lane_position_m(&self, class: VehicleClass) -> f64 {
        lane_position_for(&self.params, class)
    }

    /// The factor on a junction connector's car turn speed for `class`: a two-wheeler's
    /// lean limit, `sqrt(g·tan φ / a_lat)`, when that is the tighter (never above 1).
    fn turn_factor(&self, class: VehicleClass) -> f64 {
        let tw = self.params.two_wheelers;
        if !(tw.enabled && matches!(class, VehicleClass::Motorcycle | VehicleClass::Moped))
            || self.params.turn_lateral_accel_mps2 <= 0.0
        {
            return 1.0;
        }
        let lean = tw.max_lean_deg.clamp(1.0, 60.0) * core::f64::consts::PI / 180.0;
        let a_lean = GRAVITY_MPS2 * math::tan(lean);
        math::sqrt(a_lean / self.params.turn_lateral_accel_mps2).min(1.0)
    }

    /// The speed a vehicle of `class` takes a curved junction connector at whose car turn
    /// speed is `car_turn_mps`: a two-wheeler's lean limit ([`Self::turn_factor`]) and a
    /// heavy vehicle's design turning speed ([`crate::style::turn_speed_cap`]).
    fn class_turn_speed(&self, class: VehicleClass, car_turn_mps: f64) -> f64 {
        let v = car_turn_mps * self.turn_factor(class);
        if self.params.heavy_turns {
            crate::style::turn_speed_cap(class, v)
        } else {
            v
        }
    }

    /// Sets the weather every vehicle drives in.
    pub fn set_weather(&mut self, weather: WeatherState) {
        self.weather = weather;
    }

    /// The weather in force.
    pub fn weather(&self) -> &WeatherState {
        &self.weather
    }

    /// The parameters in force.
    pub fn params(&self) -> &EngineParams {
        &self.params
    }

    /// How many actors are on the road.
    pub fn len(&self) -> usize {
        self.actors.len()
    }

    /// True if none are.
    pub fn is_empty(&self) -> bool {
        self.actors.is_empty()
    }

    /// Every actor that has re-planned its route on information so far: a periodic
    /// congestion re-plan, a closure beyond its sight, or a warning its own unit received
    /// ([`EngineParams::reroute_share`], [`EngineParams::reroute_source`]). A scripted
    /// destination change and a detour round a closure in sight are not counted.
    pub fn rerouted_on_information(&self) -> &BTreeSet<ActorId> {
        &self.info_rerouted
    }

    /// Whether `actor`'s driver re-routes on information ([`DriverTraits::reroutes`]), or
    /// `None` for an actor not on the road.
    pub fn reroutes_on_information(&self, actor: ActorId) -> Option<bool> {
        self.actors.get(&actor).map(|a| a.traits.reroutes)
    }

    /// What the gridlock resolution has done so far.
    pub fn gridlock_counters(&self) -> GridlockCounters {
        self.gridlock
    }

    /// Gridlock resolution, at the end of a step: every vehicle that has stood still for
    /// [`EngineParams::time_to_teleport_s`] is moved to the next lane on its route that
    /// has room at its start, as SUMO's teleport does (SUMO user documentation,
    /// "Why Vehicles are teleporting": a vehicle waiting `--time-to-teleport` s is moved
    /// to the next edge of its route with space). One with no such lane is removed, and
    /// returned so the caller announces it. Ids in ascending order, so the result does
    /// not depend on map iteration (the determinism contract).
    fn resolve_gridlock(&mut self, world: &World, t1: SimTime, gone: &[ActorId]) -> Vec<ActorId> {
        let mut removed = Vec::new();
        self.give_up_strategic(world, t1);
        self.bypass_breakdowns(world, t1);
        if self.params.time_to_teleport_s <= 0.0 {
            return removed;
        }
        let limit = secs_to_ns(self.params.time_to_teleport_s);
        // The nearest rear bumper to each lane's start, from the end-of-step positions.
        let mut first_rear: BTreeMap<LaneId, f64> = BTreeMap::new();
        for a in self.actors.values() {
            let rear = a.s_m - a.class.spec().length_m;
            let e = first_rear.entry(a.lane).or_insert(f64::INFINITY);
            if rear < *e {
                *e = rear;
            }
        }
        let ids: Vec<ActorId> = self.actors.keys().copied().collect();
        for id in ids {
            if gone.contains(&id) {
                continue;
            }
            let a = self.actors.get_mut(&id).expect("present");
            let excused = a.speed_mps >= STUCK_SPEED_MPS
                || a.kerb.at_kerb()
                || a.stopped_until.is_some()
                || a.forced_brake.is_some();
            if excused {
                a.stuck_since = None;
                continue;
            }
            let since = *a.stuck_since.get_or_insert(t1);
            if t1.saturating_sub(since) < limit {
                continue;
            }
            let length = a.class.spec().length_m;
            let need = length + self.params.insertion_gap_m;
            let target = (a.route_index + 1..a.route.lanes.len()).find(|k| {
                let l = a.route.lanes[*k];
                let lane = world.lane(l);
                lane.kind != LaneKind::Internal
                    && lane.length_m >= need
                    && !self.closed.contains(&l)
                    && first_rear.get(&l).is_none_or(|r| *r >= need)
            });
            self.gridlock.jammed += 1;
            match target {
                Some(k) => {
                    let l = a.route.lanes[k];
                    a.lane = l;
                    a.route_index = k;
                    a.s_m = length;
                    a.speed_mps = 0.0;
                    a.accel_mps2 = 0.0;
                    a.lateral_m = 0.0;
                    a.trail.clear();
                    a.transition = None;
                    a.amber_commit = None;
                    a.amber_seen = None;
                    a.signal_look = None;
                    a.violation = 0;
                    a.release_at = None;
                    a.stuck_since = None;
                    first_rear.insert(l, 0.0);
                    self.gridlock.teleported += 1;
                }
                None => {
                    self.gridlock.removed += 1;
                    removed.push(id);
                }
            }
        }
        removed
    }

    /// Samples every lane's speed once a second into [`Self::lane_speed`]: the mean speed
    /// of the vehicles on it, or its free speed when it is empty, smoothed exponentially
    /// over [`CONGESTION_ADAPTATION_STEPS`] samples. Lanes in id order.
    fn sample_congestion(&mut self, world: &World, t1: SimTime) {
        if self.params.reroute_period_s <= 0.0 {
            return;
        }
        if self
            .speed_sampled_at
            .is_some_and(|at| t1.saturating_sub(at) < secs_to_ns(1.0))
        {
            return;
        }
        self.speed_sampled_at = Some(t1);
        let mut sums: BTreeMap<LaneId, (f64, u32)> = BTreeMap::new();
        for a in self.actors.values() {
            let e = sums.entry(a.lane).or_insert((0.0, 0));
            e.0 += a.speed_mps;
            e.1 += 1;
        }
        let alpha = 1.0 / CONGESTION_ADAPTATION_STEPS;
        let lanes: BTreeSet<LaneId> = sums.keys().chain(self.lane_speed.keys()).copied().collect();
        for lane in lanes {
            let Some(l) = world.try_lane(lane) else {
                continue;
            };
            let free = l.speed_limit_mps.max(CONGESTION_FLOOR_MPS);
            let seen = sums
                .get(&lane)
                .map_or(free, |(sum, n)| (sum / f64::from(*n)).min(free));
            let old = self.lane_speed.get(&lane).copied().unwrap_or(free);
            let new = old + alpha * (seen - old);
            // An empty lane back within 1 % of its free speed is forgotten; an occupied one
            // is kept however close, or a slow first sample would be dropped at once.
            if !sums.contains_key(&lane) && new >= 0.99 * free {
                self.lane_speed.remove(&lane);
            } else {
                self.lane_speed.insert(lane, new);
            }
        }
    }

    /// A vehicle with a strategic change it has not made by the no-change zone before the
    /// end of its lane, or that has stood for [`STRATEGIC_GIVE_UP_S`] waiting to make it,
    /// takes a route from the lane it is on instead, as a driver who cannot get over goes
    /// straight on and finds another way.
    fn give_up_strategic(&mut self, world: &World, t1: SimTime) {
        if !self.params.strategic_lane_choice {
            return;
        }
        let costs = self.costs(world);
        let limit = secs_to_ns(STRATEGIC_GIVE_UP_S);
        let ids: Vec<ActorId> = self.actors.keys().copied().collect();
        // With the gridlock timer off nothing else keeps the standing clock, so it is kept
        // here (by speed alone); with it on, `resolve_gridlock` keeps it.
        let own_clock = self.params.time_to_teleport_s <= 0.0;
        for id in ids {
            let a = self.actors.get_mut(&id).expect("present");
            if own_clock {
                if a.speed_mps >= STUCK_SPEED_MPS {
                    a.stuck_since = None;
                } else {
                    a.stuck_since.get_or_insert(t1);
                }
            }
            let a = &self.actors[&id];
            let Some(left) = self.strategic_pending(world, a) else {
                continue;
            };
            let in_zone = left < self.params.no_change_zone_m;
            let stuck = a
                .stuck_since
                .is_some_and(|since| t1.saturating_sub(since) >= limit);
            if !(in_zone || stuck) {
                continue;
            }
            let (lane, destination, class) = (a.lane, a.destination, a.class);
            // To the destination lane, or failing that to another lane of the destination
            // road, or to the reachable lane nearest it, as a closure detour does
            // ([`Self::detour`]). Re-planning to the destination lane alone left a vehicle
            // whose lane cannot reach it owing its change in the no-change zone, where it
            // either changed lane or stood at the end of its lane (connected-intersections
            // and Midtown, 120 s: the auditor's gap and lane-change-near-junction classes).
            let route = self
                .plan_route(class, world, lane, destination, t1, &costs)
                .map(|r| (r, destination))
                .or_else(|| {
                    Self::detour(self.router_for(class), world, lane, destination, t1, &costs)
                });
            let a = self.actors.get_mut(&id).expect("present");
            if let Some((route, to)) = route {
                a.route = route;
                a.destination = to;
                a.route_index = 0;
                a.planned_at = t1;
                a.stuck_since = None;
                self.gridlock.strategic_given_up += 1;
            }
        }
    }

    /// How many trips were dropped because their origin was occupied or unroutable.
    pub fn dropped_trips(&self) -> u64 {
        self.dropped_trips
    }

    /// How many of the dropped trips had no route, or an origin lane too short to stand on
    /// (the rest waited [`MAX_DEPART_DELAY`] for room and gave up).
    pub fn unroutable_trips(&self) -> u64 {
        self.unroutable_trips
    }

    /// The pedestrian model, if one was given.
    pub fn vru(&self) -> Option<&SocialForce> {
        self.vru.as_ref()
    }

    /// The pedestrian model, mutably.
    pub fn vru_mut(&mut self) -> Option<&mut SocialForce> {
        self.vru.as_mut()
    }

    /// One vehicle's kerbside state ([`crate::kerbside`]).
    pub fn kerbside_state(&self, actor: ActorId) -> Option<crate::kerbside::KerbState> {
        self.actors.get(&actor).map(|a| a.kerb)
    }

    /// A census of the kerbside stops now ([`crate::kerbside`]): `(vehicles with a role,
    /// stops planned and not yet reached, vehicles standing at the kerb, stops completed by
    /// the vehicles still on the road)`.
    pub fn kerbside_census(&self) -> (usize, usize, usize, u32) {
        let mut out = (0, 0, 0, 0);
        for a in self.actors.values().filter(|a| a.kerb.role.is_some()) {
            out.0 += 1;
            match a.kerb.stop {
                Some(s) if s.since.is_some() => out.2 += 1,
                Some(_) => out.1 += 1,
                None => {}
            }
            out.3 += a.kerb.stops_made;
        }
        out
    }

    /// Every actor's `(lane, front arc length, speed)`, in actor-id order: what a
    /// validation run measures.
    pub fn longitudinal_states(&self) -> Vec<(ActorId, LaneId, f64, f64)> {
        self.actors
            .values()
            .map(|a| (a.id, a.lane, a.s_m, a.speed_mps))
            .collect()
    }

    /// Every vehicle's internal state, in actor-id order, for the traffic-invariant
    /// auditor ([`crate::audit`]). Read-only: nothing the auditor sees feeds back.
    pub fn audit_actors(&self, world: &World, t: SimTime) -> Vec<crate::audit::AuditActor> {
        self.actors
            .values()
            .map(|a| {
                let k = a.kinematics(world, t, self.along_path());
                let dims = a.class.dims();
                crate::audit::AuditActor {
                    actor: a.id,
                    class: a.class,
                    length_m: dims.length_m,
                    width_m: dims.width_m,
                    min_gap_m: a.driver.min_gap_m,
                    max_accel_mps2: a.driver.max_accel_mps2,
                    lane: a.lane,
                    prev_lane: a.trail.last().copied(),
                    s_m: a.s_m,
                    lateral_m: a.lateral_m,
                    speed_mps: a.speed_mps,
                    accel_mps2: a.accel_mps2,
                    changing: a.transition.map(|t| (t.from, t.to)),
                    route_next: a.route.lanes.get(a.route_index + 1).copied(),
                    pos: k.pos,
                    heading_rad: k.heading_rad,
                    min_path_radius_m: a.class.min_path_radius_m(),
                    lane_position_m: self.lane_position_m(a.class),
                    kerbside: a.kerb.at_kerb_as(),
                    intent: a.violation
                        | if a.crash.is_some_and(|c| c.by_violation) {
                            crate::audit::intent::CRASHED
                        } else {
                            0
                        },
                }
            })
            .collect()
    }

    /// Collisions at `t1` resolve to crash states: every pair of vehicles whose footprints
    /// intersect (the auditor's [`crate::audit::overlapping_pairs`], so a crash is exactly
    /// what the auditor would call an overlap) stops where it is and stands for
    /// [`crate::drivers::ViolationParams::crash_clearance_s`].
    fn resolve_collisions(&mut self, world: &World, t1: SimTime) {
        let actors = self.audit_actors(world, t1);
        let pairs = crate::audit::overlapping_pairs(
            &actors,
            crate::audit::AuditParams::default().footprint_shrink_m,
        );
        if pairs.is_empty() {
            return;
        }
        let clear_at = t1.saturating_add(secs_to_ns(
            self.params.violations.crash_clearance_s.max(0.0),
        ));
        for (i, j) in pairs {
            let (a, b) = (actors[i].actor, actors[j].actor);
            let tagged = |id: ActorId| {
                self.actors
                    .get(&id)
                    .is_some_and(|x| x.violation != 0 || x.crash.is_some_and(|c| c.by_violation))
            };
            let by_violation = tagged(a) || tagged(b);
            for id in [a, b] {
                if let Some(x) = self.actors.get_mut(&id) {
                    if x.crash.is_none() {
                        x.crash = Some(CrashState {
                            clear_at,
                            by_violation,
                        });
                        x.speed_mps = 0.0;
                        x.accel_mps2 = 0.0;
                        x.transition = None;
                        x.stopped_until = Some(clear_at);
                    } else if by_violation && let Some(c) = x.crash.as_mut() {
                        c.by_violation = true;
                    }
                }
            }
        }
    }

    /// Every pedestrian as the traffic auditor reads it, in actor-id order.
    pub fn audit_pedestrians(&self, world: &World) -> Vec<crate::audit::AuditPedestrian> {
        let Some(vru) = self.vru.as_ref() else {
            return Vec::new();
        };
        let dims = VehicleClass::Pedestrian.dims();
        vru.people()
            .filter(|p| !p.arrived)
            .map(|p| {
                let k = self.published.get(&p.actor);
                crate::audit::AuditPedestrian {
                    actor: p.actor,
                    lane: p.lane,
                    s_m: p.s_m,
                    pos: k.map_or_else(|| p.position(world), |k| k.pos),
                    heading_rad: k.map_or(0.0, |k| k.heading_rad),
                    length_m: dims.length_m,
                    width_m: dims.width_m,
                    activity: p.activity,
                    vel: p.vel,
                }
            })
            .collect()
    }

    /// Puts one vehicle on the road with a route the caller supplies.
    ///
    /// The router plans a *simple* path, which is the right thing for a trip and the wrong
    /// thing for a closed circuit: a fundamental-diagram run wants the ring driven many
    /// times over. This is the seam for it, and for any scenario that has its own idea of
    /// where a vehicle should go.
    ///
    /// # Errors
    ///
    /// [`MobError::NoSuchLane`] if the route's first lane is not in the world,
    /// [`MobError::LaneNotAdmitted`] if the class may not use it.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_with_route(
        &mut self,
        world: &World,
        t: SimTime,
        class: VehicleClass,
        driver: DriverProfile,
        route: Vec<LaneId>,
        s_m: f64,
    ) -> Result<ActorId> {
        let first = *route.first().ok_or(MobError::EmptyWorld {
            what: "lane in the supplied route",
        })?;
        let lane = world
            .try_lane(first)
            .ok_or(MobError::NoSuchLane { lane: first })?;
        if !lane.admits(class.class_mask()) || !lane.kind.is_motorised() {
            return Err(MobError::LaneNotAdmitted {
                lane: first,
                kind: lane.kind.wire_name(),
                classes: class.as_str().to_string(),
            });
        }
        let length_m = crate::worlds::cycle_length_m(world, &route);
        let cost_s = math::sum_ordered(
            route
                .iter()
                .map(|l| {
                    let l = world.lane(*l);
                    l.length_m / l.speed_limit_mps.max(1e-9)
                })
                .collect::<Vec<_>>(),
        );
        let destination = *route.last().expect("checked above");
        let id = ActorId::new(self.next_actor);
        self.next_actor += 1;
        let seq = self.next_seq;
        self.next_seq += 1;
        self.actors.insert(
            id,
            Actor {
                id,
                seq,
                class,
                driver,
                route: Route {
                    lanes: route,
                    length_m,
                    cost_s,
                },
                route_index: 0,
                destination,
                lane: first,
                s_m: s_m.clamp(0.0, lane.length_m),
                lateral_m: self.lane_position_m(class),
                speed_mps: 0.0,
                accel_mps2: 0.0,
                spawned: t,
                planned_at: t,
                planned_generation: 0,
                transition: None,
                cooldown_until: t,
                speed_cap_mps: None,
                stuck_since: None,
                stopped_until: None,
                trail: Vec::new(),
                amber_commit: None,
                amber_seen: None,
                signal_look: None,
                violation: 0,
                crash: None,
                traits: DriverTraits::NEUTRAL,
                release_at: None,
                forced_brake: None,
                forced_cut_in: None,
                kerb: crate::kerbside::KerbState::NONE,
                human: crate::drivers::HumanState::default(),
                lc: crate::lanechange::lc2013::Lc2013State::default(),
            },
        );
        Ok(id)
    }

    /// Sets one actor's speed directly, for a validation run that starts from a prescribed
    /// state rather than from rest.
    pub fn set_speed(&mut self, actor: ActorId, speed_mps: f64) -> Result<()> {
        let a = self
            .actors
            .get_mut(&actor)
            .ok_or(MobError::NoSuchActor { actor })?;
        a.speed_mps = speed_mps.max(0.0);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // The step, pass by pass
    // -----------------------------------------------------------------------

    /// Pass 1: the queued commands.
    ///
    /// Returns the actors a [`MobilityCommand::Spawn`] put on the road, in the order the
    /// commands were queued, so pass 2 can announce them on [`MobilityUpdate::spawned`]
    /// alongside the trips demand produced. A commanded trip carries a class, a route, a
    /// driver profile and a demand-stream `seq` that nothing in `states` records, so a
    /// consumer that provisions a node per spawn never hears about an injected vehicle if
    /// this list is thrown away.
    fn apply_commands(&mut self, ctx: &dyn MobCtx, now: SimTime) -> Vec<ActorId> {
        let world = ctx.world();
        let mut spawned = Vec::new();
        let commands = core::mem::take(&mut self.pending);
        for command in commands {
            match command {
                MobilityCommand::Reroute { actor, to } => {
                    if let Some(a) = self.actors.get_mut(&actor) {
                        a.destination = to;
                        a.planned_generation = u64::MAX; // force a re-plan this step
                    }
                }
                MobilityCommand::SpeedCap { actor, v_mps } => {
                    if let Some(a) = self.actors.get_mut(&actor) {
                        a.speed_cap_mps = v_mps;
                    }
                }
                MobilityCommand::Stop { actor, until } => {
                    if let Some(a) = self.actors.get_mut(&actor) {
                        a.stopped_until = Some(until.unwrap_or(SimTime::MAX));
                        // The stop supersedes a braking command still in force: left set,
                        // the brake turned into an open-ended hold on the step the vehicle
                        // stood (pass 7) and overwrote this stop's own release. A transit
                        // bus braked to its pole, was told to stand until its dwell ended,
                        // and stood there for the rest of the run.
                        a.forced_brake = None;
                    }
                }
                MobilityCommand::Despawn { actor } => {
                    self.actors.remove(&actor);
                }
                MobilityCommand::Closure { lane, closed } => {
                    let changed = if closed {
                        self.closed.insert(lane)
                    } else {
                        self.closed.remove(&lane)
                    };
                    if changed {
                        self.closure_changes += 1;
                    }
                }
                MobilityCommand::Brake {
                    actor,
                    decel_mps2,
                    hold_until,
                } => {
                    if let Some(a) = self.actors.get_mut(&actor)
                        && decel_mps2.is_finite()
                        && decel_mps2 > 0.0
                    {
                        a.forced_brake = Some((decel_mps2, hold_until));
                    }
                }
                MobilityCommand::CutIn { actor, side, until } => {
                    if let Some(a) = self.actors.get_mut(&actor) {
                        a.forced_cut_in = Some((side, until));
                    }
                }
                MobilityCommand::HazardHeard { actor, lane } => {
                    if self.actors.contains_key(&actor) && world.try_lane(lane).is_some() {
                        let e = self.heard_hazards.entry(actor).or_default();
                        if e.0.insert(lane) {
                            e.1 = true;
                        }
                    }
                }
                MobilityCommand::Spawn(trip) => {
                    let traits = self.traits_for(ctx, &trip);
                    if let Some(id) = self.insert_trip_with(world, &trip, now, traits) {
                        spawned.push(id);
                    }
                }
            }
        }
        spawned
    }

    /// The route a vehicle on `from` bound for `destination` takes when closures may have
    /// cut its way, and the destination it now heads for.
    ///
    /// In order: its own destination; another lane of the destination road; and, when the
    /// closures leave neither reachable, the reachable road lane nearest the destination,
    /// U-turns allowed ([`crate::routing::dijkstra::Dijkstra::search_nearest`]) — the
    /// driver turns back or goes round the block and ends the trip as near as they can.
    /// `None` only when nothing at all is reachable from `from`.
    fn detour(
        router: &DynamicReroute,
        world: &World,
        from: LaneId,
        destination: LaneId,
        t: SimTime,
        costs: &DynamicCost<'_>,
    ) -> Option<(Route, LaneId)> {
        if let Some(r) = router.replan(world, from, destination, t, costs) {
            return Some((r, destination));
        }
        let dest = world.try_lane(destination)?;
        for lane in &world.edge(dest.edge).lanes {
            if *lane != destination
                && let Some(r) = router.replan(world, from, *lane, t, costs)
            {
                return Some((r, *lane));
            }
        }
        let params = DijkstraParams {
            allow_uturn: true,
            ..*router.dijkstra().params()
        };
        let route = crate::routing::dijkstra::Dijkstra::new(params).search_nearest(
            world,
            from,
            dest.end(),
            t,
            costs,
        )?;
        let to = *route.lanes.last()?;
        Some((route, to))
    }

    /// Gives every vehicle whose route runs into a closed lane a way round it
    /// ([`Self::detour`]). With dynamic rerouting on, drivers know of a closure anywhere on
    /// their route (a navigation service); with it off, only once it is on one of the
    /// next [`CLOSURE_SIGHT_LANES`] lanes of their route — at the junction they are
    /// approaching, where the barrier is in sight.
    fn closure_detours(&mut self, world: &World, t: SimTime, costs: &DynamicCost<'_>) {
        // A driver with a navigation app hears of a closure anywhere on its route; one
        // without (outside `reroute_share`, or under the V2X source) sees it within
        // CLOSURE_SIGHT_LANES, or hears of it from a warning its own unit received.
        let navigation = self.params.dynamic_rerouting
            && self.params.reroute_source == RerouteSource::Navigation;
        let cut: Vec<(ActorId, bool)> = self
            .actors
            .values()
            .filter_map(|a| {
                let heard = self.heard_hazards.get(&a.id).map(|h| &h.0);
                let k = a
                    .route
                    .lanes
                    .iter()
                    .skip(a.route_index + 1)
                    .position(|l| self.closed.contains(l))?;
                let lane = a.route.lanes[a.route_index + 1 + k];
                let told = (navigation && a.traits.reroutes)
                    || (self.params.dynamic_rerouting
                        && a.traits.reroutes
                        && heard.is_some_and(|h| h.contains(&lane)));
                (k < CLOSURE_SIGHT_LANES || told).then_some((a.id, k >= CLOSURE_SIGHT_LANES))
            })
            .collect();
        let generation = costs.generation();
        for (id, informed) in cut {
            let (lane, destination, class) = {
                let a = &self.actors[&id];
                (a.lane, a.destination, a.class)
            };
            let own = self.costs_with_hazards(costs, id);
            let found = Self::detour(
                self.router_for(class),
                world,
                lane,
                destination,
                t,
                own.as_ref().unwrap_or(costs),
            );
            if informed {
                self.info_rerouted.insert(id);
            }
            if let Some((route, to)) = found {
                let a = self.actors.get_mut(&id).expect("present");
                a.route = route;
                a.route_index = 0;
                a.destination = to;
                a.planned_at = t;
                a.planned_generation = generation;
            }
        }
    }

    /// `costs` with the lanes `actor`'s own unit warned it of closed, under the V2X
    /// source; `None` when there are none to add.
    fn costs_with_hazards<'a>(
        &self,
        costs: &DynamicCost<'a>,
        actor: ActorId,
    ) -> Option<DynamicCost<'a>> {
        if self.params.reroute_source != RerouteSource::V2x {
            return None;
        }
        let heard = self.heard_hazards.get(&actor)?;
        if heard.0.is_empty() {
            return None;
        }
        let generation = costs.generation();
        let mut own = costs.clone();
        for lane in &heard.0 {
            own.set_closed(*lane, true);
        }
        Some(own.with_generation(generation))
    }

    /// Under the V2X source, every driver who re-routes on information and whose unit
    /// has just warned it of a blocked lane still ahead on its route re-plans round it
    /// (and round every lane it was warned of before). A warning for a lane it will not
    /// drive is kept and acted on no further; one heard on a junction connector waits for
    /// the next lane, as every re-plan does.
    fn hazard_detours(&mut self, world: &World, t: SimTime, costs: &DynamicCost<'_>) {
        if self.params.reroute_source != RerouteSource::V2x || !self.params.dynamic_rerouting {
            return;
        }
        let fresh: Vec<ActorId> = self
            .heard_hazards
            .iter()
            .filter(|(_, (_, new))| *new)
            .map(|(id, _)| *id)
            .collect();
        let generation = costs.generation();
        for id in fresh {
            let Some(a) = self.actors.get(&id) else {
                self.heard_hazards.remove(&id);
                continue;
            };
            if world.lane(a.lane).kind == LaneKind::Internal {
                continue;
            }
            let ahead = a.traits.reroutes && a.reroutes_on(&self.heard_hazards[&id].0);
            let (lane, s_m, destination, class) = (a.lane, a.s_m, a.destination, a.class);
            if ahead {
                let own = self.costs_with_hazards(costs, id);
                let replanned = self.plan_route_strategic(
                    class,
                    world,
                    lane,
                    s_m,
                    destination,
                    t,
                    own.as_ref().unwrap_or(costs),
                );
                self.info_rerouted.insert(id);
                let a = self.actors.get_mut(&id).expect("present");
                a.planned_at = t;
                a.planned_generation = generation;
                if let Some(route) = replanned {
                    a.route = route;
                    a.route_index = 0;
                }
            }
            if let Some(h) = self.heard_hazards.get_mut(&id) {
                h.1 = false;
            }
        }
    }

    /// Lanes a broken-down vehicle stands on: one under a scripted stop with no end
    /// ([`MobilityCommand::Brake`] with no `hold_until`), braking or already holding.
    fn broken_down_lanes(&self) -> BTreeSet<LaneId> {
        self.actors
            .values()
            .filter(|a| {
                a.speed_mps < STUCK_SPEED_MPS
                    && (matches!(a.forced_brake, Some((_, None)))
                        || a.stopped_until == Some(SimTime::MAX)
                        || a.crash.is_some())
            })
            .map(|a| a.lane)
            .collect()
    }

    /// A vehicle that has stood for [`STRATEGIC_GIVE_UP_S`] with a lane a broken-down
    /// vehicle blocks among the next [`CLOSURE_SIGHT_LANES`] of its route — typically held
    /// at the stop line because that car leaves no room on the exit (NY VTL §1175) — finds
    /// another way, as a driver who sees the hazard lights does. Without it the queue
    /// behind the exit waited out the whole breakdown: 160 s at an unsignalised junction on
    /// midtown-rush-hour (actors 152, 62, 181 behind the car on lane 3723).
    fn bypass_breakdowns(&mut self, world: &World, t1: SimTime) {
        let broken = self.broken_down_lanes();
        // With neither the gridlock timer nor strategic lane choice on, nothing else keeps
        // the standing clock; it is kept here (by speed alone) while a breakdown stands.
        if self.params.time_to_teleport_s <= 0.0 && !self.params.strategic_lane_choice {
            for a in self.actors.values_mut() {
                if broken.is_empty() || a.speed_mps >= STUCK_SPEED_MPS {
                    a.stuck_since = None;
                } else {
                    a.stuck_since.get_or_insert(t1);
                }
            }
        }
        if broken.is_empty() {
            return;
        }
        let limit = secs_to_ns(STRATEGIC_GIVE_UP_S);
        let blocked: Vec<ActorId> = self
            .actors
            .values()
            .filter(|a| {
                a.forced_brake.is_none()
                    && a.stopped_until.is_none()
                    && !broken.contains(&a.lane)
                    && a.stuck_since
                        .is_some_and(|since| t1.saturating_sub(since) >= limit)
                    // One that found no way round tries again a give-up time later.
                    && t1.saturating_sub(a.planned_at) >= limit
                    && a.route
                        .lanes
                        .iter()
                        .skip(a.route_index + 1)
                        .take(CLOSURE_SIGHT_LANES)
                        .any(|l| broken.contains(l))
            })
            .map(|a| a.id)
            .collect();
        if blocked.is_empty() {
            return;
        }
        let mut costs = self.costs(world);
        for lane in &broken {
            costs.set_closed(*lane, true);
        }
        for id in blocked {
            let (lane, destination, class) = {
                let a = &self.actors[&id];
                (a.lane, a.destination, a.class)
            };
            let found = Self::detour(self.router_for(class), world, lane, destination, t1, &costs);
            let a = self.actors.get_mut(&id).expect("present");
            a.planned_at = t1;
            if let Some((route, to)) = found {
                a.route = route;
                a.route_index = 0;
                a.destination = to;
                a.stuck_since = None;
            }
        }
    }

    /// The distance from the vehicle's front to the start of the first closed lane on its
    /// route within the look-ahead, metres, if there is one.
    fn closure_gap(&self, world: &World, actor: &Actor) -> Option<f64> {
        let mut dist = world.lane(actor.lane).length_m - actor.s_m;
        for lane in actor.route.lanes.iter().skip(actor.route_index + 1) {
            if dist > self.params.lookahead_m {
                return None;
            }
            if self.closed.contains(lane) || self.holds.contains(lane) {
                return Some(dist.max(0.0));
            }
            dist += world.lane(*lane).length_m;
        }
        None
    }

    /// The cost function in force, given the closures.
    fn costs<'a>(&self, world: &'a World) -> DynamicCost<'a> {
        let mut costs = DynamicCost::new(world);
        for lane in &self.closed {
            costs.set_closed(*lane, true);
        }
        for (lane, v) in &self.lane_speed {
            if let Some(l) = world.try_lane(*lane) {
                costs.observe(*lane, Some(l.length_m / v.max(CONGESTION_FLOOR_MPS)));
            }
        }
        costs.with_generation(self.closure_changes)
    }

    /// Pass 2: routes a trip and puts it on the road, or drops it.
    #[cfg(test)]
    fn insert_trip(&mut self, world: &World, trip: &TripRequest, now: SimTime) -> Option<ActorId> {
        self.insert_trip_with(world, trip, now, DriverTraits::NEUTRAL)
    }

    /// [`Self::insert_trip`] with the driver's own traits.
    fn insert_trip_with(
        &mut self,
        world: &World,
        trip: &TripRequest,
        now: SimTime,
        traits: DriverTraits,
    ) -> Option<ActorId> {
        match self.try_insert(world, trip, now, None, traits) {
            Insertion::Placed(id) => Some(id),
            Insertion::Occupied | Insertion::Unroutable => {
                self.dropped_trips += 1;
                if let Some(d) = self.demand.as_mut() {
                    d.refund();
                }
                None
            }
        }
    }

    /// Routes a trip and puts it on the road if its origin has room now, as actor
    /// `reserved` if the trip already holds an id, else as the next one.
    /// The traits the driver of `trip` gets: drawn from the trip's own streams, or
    /// [`DriverTraits::NEUTRAL`] in the legacy parity mode or with heterogeneity off.
    fn traits_for(&self, ctx: &dyn MobCtx, trip: &TripRequest) -> DriverTraits {
        let mut traits = if self.along_path() && self.params.driver_heterogeneity {
            let mut traits = DriverTraits::draw_law(
                ctx,
                trip.seq,
                trip.class,
                &TraitLaw {
                    speed_factor_mean: self.params.speed_factor_mean,
                    speed_spread: self.params.speed_spread,
                    reaction_median_s: self.params.start_reaction_median_s,
                    follow_median_s: self.params.queue_start_delay_median_s,
                },
            );
            let share = self.params.red_runner_share;
            if share > 0.0 {
                let u = ctx
                    .rng(
                        RngDomain::ReactionTime,
                        EntityRef::custom(AMBER_RUNNER_ID, trip.seq),
                    )
                    .uniform(0.0, 1.0);
                traits.amber_runner = u < share;
            }
            let v = self.params.violations;
            if v.enabled {
                // Each from a keyed stream of its own, so turning violations on moves no
                // other draw.
                let mut rng = ctx.rng(
                    RngDomain::ReactionTime,
                    EntityRef::custom(RED_RUN_ID, trip.seq),
                );
                let (u_share, u_tol) = (rng.uniform(0.0, 1.0), rng.uniform(0.0, 1.0));
                traits.red_tolerance_s = v.red_tolerance_s(u_share, u_tol);
                if v.amber_tail_max_s > AMBER_DILEMMA_TTI_S.1 {
                    traits.amber_go_tti_s = amber_threshold(
                        ctx.rng(
                            RngDomain::ReactionTime,
                            EntityRef::custom(AMBER_TRAIT_ID, trip.seq),
                        )
                        .uniform(0.0, 1.0),
                        v.amber_tail_max_s,
                    );
                }
            }
            traits
        } else {
            DriverTraits::NEUTRAL
        };
        // Who re-routes on information: its own stream, drawn only below a share of 1, so
        // the default draws nothing and every other stream is untouched.
        traits.reroutes =
            crate::routing::dynamic::draws_reroute(ctx, trip.seq, self.params.reroute_share);
        traits
    }

    fn try_insert(
        &mut self,
        world: &World,
        trip: &TripRequest,
        now: SimTime,
        reserved: Option<ActorId>,
        mut traits: DriverTraits,
    ) -> Insertion {
        let costs = self.costs(world);
        // A cyclist prefers a cycle lane where one exists ([`CyclistSpeeds::shared_lane_cost`]).
        let cyclist_costs = CyclistCost {
            inner: &costs,
            world,
            shared_factor: self.params.cyclists.shared_lane_cost,
        };
        let edge_costs: &dyn crate::views::EdgeCost =
            if trip.class == VehicleClass::Bicycle && self.params.cyclists.enabled {
                &cyclist_costs
            } else {
                &costs
            };
        // A demand model that planned the route itself (an assignment's saved route, a bus
        // line through its stops) supplies it; one that fails the drivability check, or
        // runs into a closed lane, is ignored and the router plans the trip instead
        // ([`crate::demand::hooks`]). With no such model this is the router alone, as
        // before.
        let planned = self
            .demand
            .as_ref()
            .and_then(|d| d.planned_route(world, trip))
            .filter(|lanes| {
                lanes.first() == Some(&trip.origin)
                    && lanes.last() == Some(&trip.destination)
                    && lanes.iter().all(|l| !self.closed.contains(l))
                    && crate::demand::hooks::planned_route_is_drivable(
                        world, trip.class, lanes, false,
                    )
            });
        let route = match planned {
            Some(lanes) => {
                let length_m = math::sum_ordered(
                    lanes
                        .iter()
                        .map(|l| world.lane(*l).length_m)
                        .collect::<Vec<_>>(),
                );
                let cost_s = math::sum_ordered(
                    lanes
                        .iter()
                        .map(|l| {
                            let l = world.lane(*l);
                            l.length_m / l.speed_limit_mps.max(1e-9)
                        })
                        .collect::<Vec<_>>(),
                );
                Route {
                    lanes,
                    length_m,
                    cost_s,
                }
            }
            None => {
                let Some(route) = self.plan_route_strategic(
                    trip.class,
                    world,
                    trip.origin,
                    trip.origin_s_m,
                    trip.destination,
                    trip.t,
                    edge_costs,
                ) else {
                    return Insertion::Unroutable;
                };
                route
            }
        };
        // An occupied origin: drop the trip rather than overlap two vehicles.
        //
        // `s_m` is a front bumper, so `s_m - length` is a rear bumper. For a vehicle
        // already on the lane, `ahead` is the clearance if it is in front of the insertion
        // point and `behind` is the clearance if it is behind. Exactly one of the two is
        // the real clearance and the other is negative — they are the same overlap
        // measured from opposite ends — so the larger is the one to test. Taking the
        // smaller instead tested a quantity that is negative for *every* non-overlapping
        // pair, which refused every insertion onto any lane holding any vehicle at all,
        // however far away.
        let length = trip.class.spec().length_m;
        // A vehicle crossing the map's edge comes in at the start of its lane, moving
        // (`depart_at_speed`, SUMO `departPos="base"`); everyone else is put down where the
        // demand model drew, at rest.
        let at_speed = self.along_path()
            && self.params.depart_at_speed
            && is_boundary_entry(world, trip.origin);
        let front = if at_speed {
            length
        } else {
            trip.origin_s_m.max(length)
        };
        if self.along_path() {
            // The body has to fit on the origin lane: inserted on a 1 m lane (Manhattan
            // has them between close junctions) a 5 m car was placed straight across the
            // next lanes, on top of whoever was there.
            let Some(origin) = world.try_lane(trip.origin) else {
                return Insertion::Unroutable;
            };
            if front > origin.length_m {
                return Insertion::Unroutable;
            }
            // Nor may it land on a vehicle whose body still overhangs the origin lane from
            // the lane after it.
            for c in world.successors(trip.origin) {
                let next = c.via.unwrap_or(c.to_lane);
                for a in self.actors.values().filter(|a| a.lane == next) {
                    let rear_on_origin = origin.length_m + a.s_m - a.class.spec().length_m;
                    if rear_on_origin - front < self.params.insertion_gap_m {
                        return Insertion::Occupied;
                    }
                }
            }
        }
        for a in self.actors.values() {
            // A vehicle changing out of the origin lane is already counted on the lane it
            // is entering, but its body is still partly on this one: placed beside it, a
            // car was dropped at rest across a car 0.6 m into its change, which braked at
            // 8.5 m/s² on top of it (`tests/traffic_invariants.rs`, the dense grid).
            let s_here = if a.lane == trip.origin {
                a.s_m
            } else if self.along_path()
                && let Some(tr) = a.transition
                && tr.switched
                && tr.from == trip.origin
            {
                a.s_m + tr.from_s_delta
            } else {
                continue;
            };
            let ahead = (s_here - a.class.spec().length_m) - front;
            let behind = (front - length) - s_here;
            if ahead.max(behind) < self.params.insertion_gap_m {
                return Insertion::Occupied;
            }
            if a.lane != trip.origin {
                continue;
            }
            // A vehicle behind the insertion point must be able to follow the newcomer
            // without an emergency stop: its standstill gap plus its time headway at its
            // current speed, the IDM's own desired gap for a standing leader. The fixed
            // 2 m gap alone dropped cars at rest in front of traffic doing 11 m/s, which
            // then braked at the IDM's -6 m/s² floor. Not applied in the legacy mode.
            if self.along_path() && behind >= 0.0 && !self.may_follow_newcomer(world, a, behind) {
                return Insertion::Occupied;
            }
        }
        // Nor on a pedestrian in the carriageway: a crosswalk someone is on, or the stretch of
        // the lane a pedestrian crossing mid-block is walking. A car was put down at rest
        // on top of two jaywalkers.
        if self.along_path()
            && let Some(vru) = self.vru.as_ref()
            && !vru.is_empty()
        {
            let rear = front - length;
            let clear = |enter: f64, exit: f64| exit < rear - 0.5 || enter > front + 0.5;
            let occupied = self
                .crosswalks
                .occupied(vru.people().filter(|p| !p.arrived).map(|p| p.lane));
            for c in self.crosswalks.conflicts_on(trip.origin) {
                if occupied.contains(&c.crosswalk) && !clear(c.enter_s(), c.exit_s()) {
                    return Insertion::Occupied;
                }
            }
            let midblock = MidblockBands::of(Some(vru));
            if let Some(list) = midblock.by_lane.get(&trip.origin) {
                for c in list {
                    let crossing = midblock.owner.get(&c.crosswalk).is_some_and(|(ped, _)| {
                        midblock.progress.get(ped).copied().flatten().is_some()
                    });
                    if crossing && !clear(c.enter_s(), c.exit_s()) {
                        return Insertion::Occupied;
                    }
                }
            }
        }
        if self.along_path() {
            // The same for traffic about to arrive from the lanes that feed this one.
            for c in world.predecessors(trip.origin) {
                let from = c.via.unwrap_or(c.from_lane);
                let Some(up) = world.try_lane(from) else {
                    continue;
                };
                for a in self.actors.values().filter(|a| a.lane == from) {
                    let gap = (up.length_m - a.s_m) + (front - length);
                    if !self.may_follow_newcomer(world, a, gap) {
                        return Insertion::Occupied;
                    }
                }
            }
        }
        let id = reserved.unwrap_or_else(|| {
            let id = ActorId::new(self.next_actor);
            self.next_actor += 1;
            id
        });
        let mut driver = DriverProfile {
            desired_speed_mps: trip.desired_speed_mps,
            ..self.driver_for(trip.class)
        };
        // How this driver follows: assertive or cautious drivers keep their own gaps
        // ([`crate::style`]). A rider keeps the bicycle vType.
        if self.along_path()
            && self.params.driver_styles
            && self.params.driver_heterogeneity
            && trip.class != VehicleClass::Bicycle
        {
            driver = traits.style.apply(driver);
        }
        // Each driver's own T, a, b and s0 around the class's ([`crate::drivers`]).
        if self.along_path()
            && self.params.driver_heterogeneity
            && self.params.human.parameter_spread > 0.0
            && trip.class != VehicleClass::Bicycle
        {
            driver = crate::drivers::spread_profile(
                driver,
                trip.class,
                traits.params_z,
                self.params.human.parameter_spread,
            );
        }
        // A vehicle type the demand states (an imported `.rou.xml` vType) is driven
        // exactly: SUMO's vClass default under the type's own numbers, its own drawn
        // speedFactor, and none of the engine's style or spread on top. No other demand
        // model states one, so every other run is as before.
        let vtype = self.demand.as_ref().and_then(|d| d.trip_driver(trip));
        if let Some(vt) = vtype {
            driver = vt.over(DriverProfile {
                desired_speed_mps: trip.desired_speed_mps,
                ..crate::carfollowing::krauss::Krauss::class_profile(trip.class)
            });
            if let Some((factor, mean, dev)) = vt.speed_factor {
                traits.speed_factor = factor;
                traits.style = crate::style::DriverStyle::of_speed_factor_around(factor, mean, dev);
            }
        }
        self.actors.insert(
            id,
            Actor {
                id,
                seq: trip.seq,
                class: trip.class,
                driver,
                route,
                route_index: 0,
                destination: trip.destination,
                lane: trip.origin,
                s_m: front,
                lateral_m: self.lane_position_m(trip.class),
                speed_mps: 0.0,
                accel_mps2: 0.0,
                spawned: trip.t.max(now),
                planned_at: trip.t,
                planned_generation: 0,
                transition: None,
                cooldown_until: trip.t,
                speed_cap_mps: None,
                stuck_since: None,
                stopped_until: None,
                trail: Vec::new(),
                amber_commit: None,
                amber_seen: None,
                signal_look: None,
                violation: 0,
                crash: None,
                traits,
                release_at: None,
                forced_brake: None,
                forced_cut_in: None,
                kerb: crate::kerbside::KerbState::NONE,
                human: crate::drivers::HumanState::default(),
                lc: crate::lanechange::lc2013::Lc2013State::default(),
            },
        );
        if self.along_path() && self.params.depart_speed_max {
            let v = self.depart_speed(world, id);
            if let Some(a) = self.actors.get_mut(&id) {
                a.speed_mps = v;
            }
        }
        self.next_seq = self.next_seq.max(trip.seq + 1);
        if at_speed {
            let v = self.insertion_speed(world, id);
            if let Some(a) = self.actors.get_mut(&id) {
                a.speed_mps = v;
            }
        }
        if let Some((name, seed)) =
            vtype.and_then(|vt| vt.car_following.map(|m| (m, vt.model_seed)))
        {
            let model = match self.vtype_models.get(name) {
                Some(m) => Some(Arc::clone(m)),
                None => {
                    let m = crate::carfollowing::library::build_one(name, seed);
                    if let Some(m) = &m {
                        self.vtype_models.insert(name, Arc::clone(m));
                    }
                    m
                }
            };
            if let Some(m) = model {
                self.vtype_cf.insert(id, m);
            }
        }
        if let Some(ds) = vtype.and_then(|vt| vt.depart_speed) {
            match self.stated_depart_speed(world, id, ds) {
                Some(v) => {
                    if let Some(a) = self.actors.get_mut(&id) {
                        a.speed_mps = v;
                    }
                }
                None => {
                    // SUMO delays a departure whose stated speed is unsafe (SUMO
                    // documentation, "Definition of Vehicles", departSpeed): the vehicle
                    // is taken off again and the trip waits for the next step, as an
                    // occupied origin does.
                    self.actors.remove(&id);
                    self.vtype_cf.remove(&id);
                    if reserved.is_none() && id.index() + 1 == self.next_actor {
                        self.next_actor -= 1;
                    }
                    return Insertion::Occupied;
                }
            }
        }
        Insertion::Placed(id)
    }

    /// The speed a vehicle entering the map at a boundary is put on the road at
    /// ([`EngineParams::depart_at_speed`]): its free speed — its desired speed, at most the
    /// lane's limit times its own speed factor — capped twice. Once by the stop it may
    /// have to make at the end of its lane, `sqrt(2·b·(d − s0))` at its comfortable
    /// deceleration `b`; and once by the vehicle ahead (on its lane or the next two of its
    /// route): the highest speed at which the car-following model asks for no harder
    /// braking than `b`, found by bisection. SUMO's `departSpeed="desired"` refuses an
    /// unsafe insertion; this is its `"max"` variant, which reduces the speed instead
    /// (SUMO user documentation, *Definition of Vehicles*, departSpeed).
    fn insertion_speed(&self, world: &World, id: ActorId) -> f64 {
        let Some(a) = self.actors.get(&id) else {
            return 0.0;
        };
        let lane = world.lane(a.lane);
        let mut view = LaneView::of(lane);
        view.speed_limit_mps *= a.traits.speed_factor;
        let b = a.driver.comfort_decel_mps2.max(0.1);
        let free = a
            .driver
            .desired_speed_mps
            .min(view.speed_limit_mps)
            .max(0.0);
        let to_end = (lane.length_m - a.s_m - a.driver.min_gap_m).max(0.0);
        let mut v = free.min(math::sqrt(2.0 * b * to_end));
        // The nearest vehicle ahead: on the lane, else on the next lanes of the route.
        let mut leader: Option<(f64, f64)> = None;
        let mut offset = 0.0;
        for (k, l) in a.route.lanes.iter().take(3).enumerate() {
            let rear = self
                .actors
                .values()
                .filter(|o| o.id != a.id && o.lane == *l && (k > 0 || o.s_m > a.s_m))
                .map(|o| (o.s_m - o.class.spec().length_m, o.speed_mps))
                .min_by(|x, y| x.0.total_cmp(&y.0));
            if let Some((rear_s, speed)) = rear {
                let gap = if k == 0 {
                    rear_s - a.s_m
                } else {
                    offset + rear_s
                };
                leader = Some((gap.max(0.0), speed));
                break;
            }
            offset += if k == 0 {
                world.lane(*l).length_m - a.s_m
            } else {
                world.lane(*l).length_m
            };
        }
        if let Some((gap, v_lead)) = leader {
            let obstacle = LeaderView::virtual_obstacle(gap, v_lead);
            let safe = |speed: f64| {
                let mut probe = a.clone();
                probe.speed_mps = speed;
                VehicleCf(&self.cf, &self.vtype_cf)
                    .accel(&probe.view(world), Some(&obstacle), &view, &self.weather)
                    >= -b
            };
            if !safe(v) {
                let (mut lo, mut hi) = (0.0, v);
                for _ in 0..30 {
                    let mid = 0.5 * (lo + hi);
                    if safe(mid) {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                v = lo;
            }
        }
        math::q3(v)
    }

    /// The speed a newly inserted vehicle departs at under `depart_at_speed`: its desired
    /// speed on the lane (its own limit, and the slower lane or turn it anticipates),
    /// capped by the speed it can stop from at its comfortable deceleration before the end
    /// of the lane (a red signal there is not yet known), and by the speed that keeps the
    /// IDM's desired gap `s0 + v T` to the vehicle ahead on the lane and lets it brake to
    /// that vehicle's speed within the gap.
    /// The speed a vehicle with a stated `departSpeed` enters at, as SUMO documents it:
    /// `max` and `random` are lowered to the safe [`Self::depart_speed`] (SUMO adapts
    /// them to the leader); a number, `desired` and `speedLimit` above the safe speed are
    /// `None`, a departure SUMO delays rather than slows.
    fn stated_depart_speed(
        &self,
        world: &World,
        id: ActorId,
        ds: crate::views::DepartSpeed,
    ) -> Option<f64> {
        use crate::views::DepartSpeed;
        let a = self.actors.get(&id)?;
        let lane = world.lane(a.lane);
        let safe = self.depart_speed(world, id);
        let desired = a
            .driver
            .desired_speed_mps
            .min(lane.speed_limit_mps * a.traits.speed_factor)
            .max(0.0);
        let wanted = match ds {
            DepartSpeed::Mps(v) => v.max(0.0),
            DepartSpeed::Max => safe,
            DepartSpeed::Desired => desired,
            DepartSpeed::SpeedLimit => lane.speed_limit_mps,
            DepartSpeed::Random(u) => u.clamp(0.0, 1.0) * desired,
        };
        match ds {
            DepartSpeed::Max | DepartSpeed::Random(_) => Some(wanted.min(safe)),
            // A millimetre per second of slack: the safe speed is rounded to q3.
            _ if wanted > safe + 1e-3 => None,
            _ => Some(wanted.min(safe)),
        }
    }

    fn depart_speed(&self, world: &World, id: ActorId) -> f64 {
        let Some(a) = self.actors.get(&id) else {
            return 0.0;
        };
        let lane = world.lane(a.lane);
        let b = a.driver.comfort_decel_mps2.max(0.1);
        let mut v = a
            .driver
            .desired_speed_mps
            .min(lane.speed_limit_mps * a.traits.speed_factor)
            .min(self.anticipated_speed(world, a));
        v = v.min((2.0 * b * (lane.length_m - a.s_m).max(0.0)).sqrt());
        let s0 = a.driver.min_gap_m;
        let t = a.driver.time_headway_s.max(0.1);
        for o in self.actors.values() {
            if o.id == id || o.lane != a.lane || o.s_m <= a.s_m {
                continue;
            }
            let room = (o.s_m - o.class.spec().length_m - a.s_m - s0).max(0.0);
            v = v
                .min(room / t)
                .min((o.speed_mps * o.speed_mps + 2.0 * b * room).sqrt());
        }
        v.max(0.0)
    }

    /// Whether `follower` can take a vehicle standing `gap_m` ahead of it without braking
    /// harder than its comfortable deceleration: its standstill gap plus its time headway
    /// at its speed, and the car-following model's own verdict on a standing leader at that
    /// gap.
    ///
    /// The headway alone let a car be placed at rest 16.7 m in front of one doing 8.5 m/s
    /// (2 + 8.5 × 1.5 = 14.8 m needed), which the IDM then braked for at −5.7 m/s² — a
    /// standing obstacle needs the braking distance, not the following distance.
    fn may_follow_newcomer(&self, world: &World, follower: &Actor, gap_m: f64) -> bool {
        let need = follower.driver.min_gap_m + follower.speed_mps * follower.driver.time_headway_s;
        if gap_m < need {
            return false;
        }
        if follower.speed_mps <= 0.0 {
            return true;
        }
        let mut lane = LaneView::of(world.lane(follower.lane));
        lane.speed_limit_mps *= follower.traits.speed_factor;
        let a = VehicleCf(&self.cf, &self.vtype_cf).accel(
            &follower.view(world),
            Some(&LeaderView::virtual_obstacle(gap_m, 0.0)),
            &lane,
            &self.weather,
        );
        a >= -follower.driver.comfort_decel_mps2
    }

    /// Tops the VRU population up to [`VruPopulation`], removing pedestrians who have
    /// finished their walk (they are replaced by new ones). Pedestrians start on a random
    /// walkable lane and walk a random chain of connected ones; cyclists ride from a random
    /// bicycle lane to another over the bicycle lane graph. Every draw comes from the new
    /// actor's own stream, so the placement does not depend on how many others were placed.
    fn keep_vru_population(
        &mut self,
        ctx: &mut dyn MobCtx,
        now: SimTime,
        spawned: &mut Vec<ActorSpawn>,
        gone: &mut Vec<(ActorId, DespawnCause)>,
    ) {
        let target = self.vru_population;
        // Pedestrians.
        if target.pedestrians > 0 && !self.walkable.is_empty() {
            if let Some(vru) = self.vru.as_mut() {
                let arrived: Vec<ActorId> = vru
                    .people()
                    .filter(|p| p.arrived)
                    .map(|p| p.actor)
                    .collect();
                for a in arrived {
                    vru.despawn(a);
                    gone.push((a, DespawnCause::TripComplete));
                }
            }
            let mut attempts = 0;
            while self.vru.as_ref().map_or(0, SocialForce::len) < target.pedestrians as usize
                && attempts < target.pedestrians as usize + 4
            {
                attempts += 1;
                let id = ActorId::new(self.next_actor);
                self.next_actor += 1;
                let (route, s_m) = {
                    let world = ctx.world();
                    let walkable = &self.walkable;
                    let mut rng =
                        ctx.rng(RngDomain::plugin(VRU_PLACEMENT_ID), EntityRef::Actor(id));
                    let first = walkable[rng.below(walkable.len() as u64) as usize];
                    let mut route = vec![first];
                    while route.len() < PEDESTRIAN_WALK_LANES {
                        let last = *route.last().expect("non-empty");
                        let next: Vec<LaneId> = SocialForce::walk_successors(world, last)
                            .into_iter()
                            .filter(|l| !route.contains(l))
                            .collect();
                        if next.is_empty() {
                            break;
                        }
                        route.push(next[rng.below(next.len() as u64) as usize]);
                    }
                    let length = world.lane(first).length_m;
                    (route, rng.uniform(0.0, length))
                };
                let Some(mut vru) = self.vru.take() else {
                    break;
                };
                // A group walks together (Moussaïd et al. 2010): drawn only when the
                // parameters ask for groups, so a run without them draws what it drew.
                let shares = vru.params().group_shares;
                let size = if shares[1..].iter().any(|x| *x > 0.0) {
                    let total: f64 = shares.iter().sum();
                    let mut u = ctx
                        .rng(
                            RngDomain::plugin(VRU_PLACEMENT_ID),
                            EntityRef::custom(VRU_GROUP_ID, u64::from(id.index())),
                        )
                        .uniform(0.0, total.max(1e-9));
                    let mut k = 1;
                    for (i, x) in shares.iter().enumerate() {
                        if u < *x {
                            k = i + 1;
                            break;
                        }
                        u -= x;
                    }
                    let room = (target.pedestrians as usize)
                        .saturating_sub(vru.len())
                        .max(1);
                    k.min(room)
                } else {
                    1
                };
                let mut members = vec![id];
                for _ in 1..size {
                    members.push(ActorId::new(self.next_actor));
                    self.next_actor += 1;
                }
                let placed = if size == 1 {
                    vru.spawn(ctx, id, route.clone(), s_m).is_ok()
                } else {
                    // The leader's compliance and gap judgement; the slowest member's pace.
                    let (traits, _) = vru.draw_traits(&*ctx, id);
                    let pace = members
                        .iter()
                        .map(|m| vru.draw_traits(&*ctx, *m).1)
                        .fold(f64::INFINITY, f64::min);
                    let mut ok = true;
                    for (i, m) in members.iter().enumerate() {
                        let lateral = (i as f64 - 0.5 * (size as f64 - 1.0)) * GROUP_SPACING_M;
                        ok &= vru
                            .spawn_with(
                                ctx,
                                *m,
                                route.clone(),
                                s_m,
                                lateral,
                                traits,
                                pace,
                                Some(id.index()),
                            )
                            .is_ok();
                    }
                    ok
                };
                for m in &members {
                    if !placed {
                        break;
                    }
                    let Some(p) = vru.get(*m) else { continue };
                    let world = ctx.world();
                    let mut k = Kinematics::at_rest(now, p.position(world));
                    k.dims = VehicleClass::Pedestrian.dims();
                    k.heading_rad = world.lane(p.lane).heading_at(p.s_m);
                    spawned.push(ActorSpawn {
                        actor: *m,
                        t: now,
                        class: VehicleClass::Pedestrian,
                        kinematics: k,
                        route: Self::route_of(world, route.clone()),
                        driver: DriverProfile {
                            desired_speed_mps: p.desired_speed_mps,
                            max_accel_mps2: 0.0,
                            comfort_decel_mps2: 0.0,
                            time_headway_s: 0.0,
                            min_gap_m: 0.0,
                            sigma: None,
                        },
                        seq: self.next_seq,
                        requested: None,
                    });
                    self.next_seq += 1;
                }
                self.vru = Some(vru);
            }
        }
        // Cyclists.
        if target.cyclists > 0 && self.bike_lanes.len() >= 2 {
            let riding = self
                .actors
                .values()
                .filter(|a| a.class == VehicleClass::Bicycle)
                .count();
            let mut missing = (target.cyclists as usize).saturating_sub(riding);
            let mut attempts = 0;
            while missing > 0 && attempts < 2 * target.cyclists as usize + 4 {
                attempts += 1;
                let key = self.next_seq;
                let (origin, destination, s_m) = {
                    let lanes = &self.bike_lanes;
                    let mut rng = ctx.rng(
                        RngDomain::plugin(VRU_PLACEMENT_ID),
                        EntityRef::custom(VRU_PLACEMENT_ID, key),
                    );
                    let o = lanes[rng.below(lanes.len() as u64) as usize];
                    let d = lanes[rng.below(lanes.len() as u64) as usize];
                    let length = ctx.world().lane(o).length_m;
                    (
                        o,
                        d,
                        rng.uniform(VehicleClass::Bicycle.spec().length_m, length),
                    )
                };
                if origin == destination {
                    continue;
                }
                let trip = TripRequest {
                    seq: key,
                    t: now,
                    origin,
                    origin_s_m: s_m,
                    destination,
                    class: VehicleClass::Bicycle,
                    desired_speed_mps: self.cyclist_speed(&*ctx, key),
                };
                self.next_seq += 1;
                let traits = self.traits_for(&*ctx, &trip);
                let world = ctx.world();
                if let Insertion::Placed(id) = self.try_insert(world, &trip, now, None, traits) {
                    spawned.push(self.actor_spawn(world, id, now).with_requested(trip.t));
                    missing -= 1;
                }
            }
        }
    }

    /// Which pedestrians waiting mid-block may step off now, judged on the vehicles as
    /// they are after this step's move (`crate::vru::midblock`): no vehicle on a band of
    /// the path, none too close to stop, and every approaching one either arriving after
    /// the pedestrian has cleared its lane, gone past before they reach it (the rolling
    /// gap), or a driver who yields and has slowed. Draws each new driver's yield.
    fn midblock_permits(
        &mut self,
        ctx: &dyn MobCtx,
        world: &World,
        vru: &SocialForce,
        midblock: &MidblockBands,
    ) -> BTreeMap<ActorId, bool> {
        let p = vru.params().midblock;
        let mut go: BTreeMap<ActorId, bool> = midblock.waiting.iter().map(|a| (*a, true)).collect();
        let mut seen: BTreeSet<(ActorId, ActorId)> = BTreeSet::new();
        for a in self.actors.values() {
            let path = a.crosswalk_path();
            let bands = crate::vru::crosswalk::bands_ahead(world, &path, &midblock.by_lane);
            let length = a.class.spec().length_m;
            for b in &bands {
                let Some(&(ped, band)) = midblock.owner.get(&b.crosswalk) else {
                    continue;
                };
                if !midblock.waiting.contains(&ped) {
                    continue;
                }
                let Some(person) = vru.get(ped) else { continue };
                seen.insert((ped, a.id));
                // A driver who has not yet met this pedestrian decides once whether to
                // yield, if they could stop comfortably.
                let comfortable = a.speed_mps * 1.0
                    + a.speed_mps * a.speed_mps / (2.0 * a.driver.comfort_decel_mps2.max(0.5))
                    + crate::vru::crosswalk::STOP_BEFORE_CROSSWALK_M;
                if b.enter_m > comfortable && !self.midblock_yields.contains_key(&(ped, a.id)) {
                    let key = (u64::from(ped.index()) << 32) | u64::from(a.id.index());
                    let u = ctx
                        .rng(
                            RngDomain::plugin(MIDBLOCK_YIELD_ID),
                            EntityRef::custom(MIDBLOCK_YIELD_ID, key),
                        )
                        .uniform(0.0, 1.0);
                    self.midblock_yields
                        .insert((ped, a.id), u < p.driver_yield_probability);
                }
                let yields = self.midblock_yields.get(&(ped, a.id)).copied() == Some(true);
                let on_it = b.enter_m <= 0.0 && b.exit_m + length > 0.0;
                let ok = if on_it {
                    false
                } else if b.enter_m <= 0.0 {
                    true
                } else if yields && a.speed_mps < 1.0 && b.enter_m > 0.5 {
                    // A driver who chose to yield has stopped (or all but) short of the
                    // path, where the yield rule holds them while the pedestrian crosses.
                    true
                } else if b.enter_m <= crate::vru::crosswalk::stopping_distance_m(a.speed_mps) {
                    false
                } else if a.speed_mps <= crate::vru::crosswalk::APPROACHING_MPS {
                    true
                } else {
                    let (d_near, d_far) = midblock.reach[&b.crosswalk];
                    let v = person.desired_speed_mps.max(0.1);
                    let t_s = person.traits.gap_margin_s;
                    let tta = b.enter_m / a.speed_mps;
                    let clears = tta > d_far / v + t_s;
                    let passes = (b.exit_m + length) / a.speed_mps + t_s.min(1.0)
                        < (d_near - MIDBLOCK_LOOKAHEAD_M).max(0.0) / v;
                    clears || passes
                };
                let _ = band;
                if !ok {
                    go.insert(ped, false);
                }
            }
        }
        // Forget the drivers who have passed, and the pedestrians who have crossed; a
        // driver who yielded is remembered until the pedestrian is across.
        self.midblock_yields
            .retain(|(ped, veh), yields| match midblock.progress.get(ped) {
                None => false,
                Some(None) => seen.contains(&(*ped, *veh)),
                Some(Some(_)) => *yields,
            });
        go
    }

    /// Where each pedestrian in the carriageway mid-block must stop short, metres along its
    /// path: just before the next lane whose band a vehicle's body is on, or that a moving
    /// vehicle is too close to stop before. Drivers stop for a pedestrian in or entering
    /// their lane; a pedestrian does not walk into one who could not, or into the side of
    /// a car still crossing the path (a car that let the pedestrian go first and then
    /// slowed in a queue) — they wait at the lane line, the rolling gap's other half.
    fn midblock_holds(&self, world: &World, midblock: &MidblockBands) -> BTreeMap<ActorId, f64> {
        let mut holds: BTreeMap<ActorId, f64> = BTreeMap::new();
        for a in self.actors.values() {
            let path = a.crosswalk_path();
            let bands = crate::vru::crosswalk::bands_ahead(world, &path, &midblock.by_lane);
            let length = a.class.spec().length_m;
            for b in &bands {
                let Some(&(ped, _)) = midblock.owner.get(&b.crosswalk) else {
                    continue;
                };
                let Some(Some(d)) = midblock.progress.get(&ped).copied() else {
                    continue;
                };
                let (d_near, _) = midblock.reach[&b.crosswalk];
                if d >= d_near {
                    // Already in the lane: the drivers stop for it.
                    continue;
                }
                // Short of the lane: wait at its line, or where it stands if it is already
                // closer than the line's margin (crossing the lane before, whose far edge
                // is this one's near edge).
                let edge = (d_near - MIDBLOCK_HOLD_MARGIN_M).max(d);
                let on_it = b.enter_m <= 0.0 && b.exit_m + length > 0.0;
                let closing = b.enter_m > 0.0
                    && a.speed_mps > crate::vru::crosswalk::APPROACHING_MPS
                    && b.enter_m <= crate::vru::crosswalk::stopping_distance_m(a.speed_mps);
                if on_it || closing {
                    let h = holds.entry(ped).or_insert(f64::INFINITY);
                    *h = h.min(edge);
                }
            }
        }
        holds
    }

    /// The pedestrians on a sidewalk beside traffic that stands still: a vehicle within
    /// a few metres, and every vehicle there below walking pace.
    fn beside_queue(
        world: &World,
        vru: &SocialForce,
        snapshot: &ActorSnapshot,
    ) -> BTreeSet<ActorId> {
        let mut out = BTreeSet::new();
        for p in vru.people() {
            if p.arrived || p.midblock.is_some() {
                continue;
            }
            let pos = p.position(world);
            let near = snapshot.actors_within(pos, 8.0);
            if near.is_empty() {
                continue;
            }
            if near.iter().all(|a| {
                snapshot
                    .kinematics(*a)
                    .is_some_and(|k| k.vel.norm_2d() < crate::vru::crosswalk::APPROACHING_MPS)
            }) {
                out.insert(p.actor);
            }
        }
        out
    }

    /// A cyclist's desired speed ([`CyclistSpeeds`]), from the trip's own stream.
    fn cyclist_speed(&self, ctx: &dyn MobCtx, key: u64) -> f64 {
        let c = self.params.cyclists;
        if !c.enabled {
            return bicycle_driver().desired_speed_mps;
        }
        let mut rng = ctx.rng(
            RngDomain::plugin(VRU_PLACEMENT_ID),
            EntityRef::custom(CYCLIST_SPEED_ID, key),
        );
        let mean = if rng.uniform(0.0, 1.0) < c.ebike_share {
            c.ebike_mps
        } else {
            c.conventional_mps
        };
        rng.normal(mean, mean * c.relative_sd)
            .clamp(0.4 * mean, CYCLIST_MAX_DESIRED_MPS)
    }

    /// The [`ActorSpawn`] announcing an actor this engine has just inserted.
    ///
    /// One builder for both spawn paths — commanded and demand-driven — so the two cannot
    /// announce different things about the same kind of event.
    /// Gives a demand trip's vehicle its kerbside role ([`crate::kerbside`]). Only demand
    /// trips stop at the kerb: a vehicle placed by a command (a scripted test, the
    /// saturation-flow field study's base conditions) does not.
    fn assign_kerbside(&mut self, ctx: &dyn MobCtx, id: ActorId) {
        if !self.along_path() || !self.params.kerbside.enabled {
            return;
        }
        let params = self.params.kerbside;
        if let Some(a) = self.actors.get_mut(&id) {
            a.kerb =
                crate::kerbside::KerbState::assign(ctx, &params, a.class, a.seq, a.route.length_m);
        }
    }

    fn actor_spawn(&self, world: &World, id: ActorId, t: SimTime) -> ActorSpawn {
        let actor = &self.actors[&id];
        ActorSpawn {
            actor: id,
            t,
            class: actor.class,
            kinematics: actor.kinematics(world, t, self.along_path()),
            route: actor.route.clone(),
            driver: actor.driver,
            seq: actor.seq,
            requested: None,
        }
    }

    /// The driver profile a class gets: the installed car-following model's own
    /// calibration.
    fn driver_for(&self, class: VehicleClass) -> DriverProfile {
        let base = self.class_profile(class);
        match self.class_overrides.iter().find(|(c, _)| *c == class) {
            Some((_, o)) => DriverProfile {
                max_accel_mps2: o.max_accel_mps2.unwrap_or(base.max_accel_mps2),
                comfort_decel_mps2: o.comfort_decel_mps2.unwrap_or(base.comfort_decel_mps2),
                time_headway_s: o.time_headway_s.unwrap_or(base.time_headway_s),
                min_gap_m: o.min_gap_m.unwrap_or(base.min_gap_m),
                ..base
            },
            None => base,
        }
    }

    /// The driver profile of `class` before any per-class override.
    fn class_profile(&self, class: VehicleClass) -> DriverProfile {
        if class == VehicleClass::Bicycle {
            return bicycle_driver();
        }
        let tw = self.params.two_wheelers;
        if tw.enabled
            && self.along_path()
            && matches!(class, VehicleClass::Motorcycle | VehicleClass::Moped)
        {
            let base = self.cf.profile(class);
            return DriverProfile {
                max_accel_mps2: if class == VehicleClass::Moped {
                    tw.moped_accel_mps2
                } else {
                    tw.motorcycle_accel_mps2
                },
                comfort_decel_mps2: tw.comfort_decel_mps2,
                time_headway_s: tw.time_headway_s,
                min_gap_m: tw.min_gap_m,
                ..base
            };
        }
        if class == VehicleClass::Passenger && self.driver_override != DriverOverride::default() {
            let o = self.driver_override;
            let base = self.cf.profile(class);
            return DriverProfile {
                max_accel_mps2: o.max_accel_mps2.unwrap_or(base.max_accel_mps2),
                comfort_decel_mps2: o.comfort_decel_mps2.unwrap_or(base.comfort_decel_mps2),
                time_headway_s: o.time_headway_s.unwrap_or(base.time_headway_s),
                min_gap_m: o.min_gap_m.unwrap_or(base.min_gap_m),
                ..base
            };
        }
        // [`CarFollowing::profile`] exists for this. Hard-coding a set here instead meant
        // `NativeMobility::legacy()` — the documented legacy-parity configuration — ran
        // the legacy equations with Kesting 2010 drivers: (1.4, 2.0, 1.5, 2.0) where the
        // legacy car is (1.8, 2.5, 1.3, 2.5), and a different set again for every class
        // the legacy table treats separately. A parity run could not have matched.
        self.cf.profile(class)
    }

    /// Pass 3: the frozen snapshot.
    fn snapshot(&self, world: &World, t: SimTime) -> ActorSnapshot {
        let mut snapshot = ActorSnapshot::build(
            t,
            self.params.lookahead_m.max(1.0),
            self.actors
                .values()
                .map(|a| (a.view(world), a.kinematics(world, t, self.along_path()))),
        );
        if self.along_path() {
            // A vehicle part-way through a lane change is in the target lane's frame, but
            // until its body has slid clear of the lane it is leaving it still occupies
            // that lane too. Registering it there keeps the follower it is leaving from
            // driving into the space its body is still in — which the frame switch alone
            // allowed, and which the auditor counted as overlaps.
            for a in self.actors.values() {
                let Some(tr) = a.transition else { continue };
                if !tr.switched {
                    continue;
                }
                let half_body = 0.5 * a.class.spec().width_m;
                let half_lane = 0.5 * world.lane(tr.to).width_m;
                if a.lateral_m.abs() + half_body > half_lane {
                    snapshot.push_ghost(tr.from, a.s_m + tr.from_s_delta, a.id);
                }
            }
            // A road user on a junction connector still occupies the start of every sibling
            // connector leaving the same approach lane, until the two paths are apart. The
            // driver behind, going another way, would otherwise not see it, because it is
            // not on their route: a car going straight on drove into the rear of a car
            // turning left from the same lane at the mouth of the junction, where the two
            // connectors share their first metres (pedestrian_invariants: 42 and 14
            // overlaps, every one between two motor vehicles on sibling connectors, on
            // 486c8cba). This was first done for bicycles only, which accelerate and
            // cruise slower than the car behind; a turning car is as slow. SUMO does the
            // same through the leaders it reports from a link's sibling links
            // (`MSLink::getLeaderInfo`, links that share their approach lane).
            for a in self.actors.values() {
                let Some(siblings) = self.diverging.get(&a.lane) else {
                    continue;
                };
                let spec = a.class.spec();
                let rear = a.s_m - spec.length_m;
                for (sibling, apart_s, apart_wide_s) in siblings {
                    let apart = if spec.width_m > 2.0 {
                        *apart_wide_s
                    } else {
                        *apart_s
                    };
                    if rear < apart {
                        snapshot.push_ghost(*sibling, a.s_m, a.id);
                    }
                }
            }
            snapshot.sort();
        }
        snapshot
    }

    /// For every junction connector, the other connectors that leave the same approach
    /// lane, each with the arc lengths beyond which the two centrelines are at least
    /// [`DIVERGED_M`] and [`DIVERGED_WIDE_M`] apart.
    fn diverging_connectors(world: &World) -> BTreeMap<LaneId, Vec<(LaneId, f64, f64)>> {
        let mut by_approach: BTreeMap<LaneId, Vec<LaneId>> = BTreeMap::new();
        for c in world.roads.connections() {
            if let Some(via) = c.via {
                let v = by_approach.entry(c.from_lane).or_default();
                if !v.contains(&via) {
                    v.push(via);
                }
            }
        }
        let mut out: BTreeMap<LaneId, Vec<(LaneId, f64, f64)>> = BTreeMap::new();
        for connectors in by_approach.values() {
            for a in connectors {
                for b in connectors {
                    if a == b {
                        continue;
                    }
                    let (la, lb) = (world.lane(*a), world.lane(*b));
                    let end = la.length_m.min(lb.length_m);
                    let apart = |m: f64| {
                        let mut s = 0.0;
                        while s < end && la.point_at(s).distance_2d(lb.point_at(s)) < m {
                            s += 0.5;
                        }
                        s.min(end)
                    };
                    out.entry(*a).or_default().push((
                        *b,
                        apart(DIVERGED_M),
                        apart(DIVERGED_WIDE_M),
                    ));
                }
            }
        }
        out
    }

    /// Pass 4: every signal plan's state at `t`.
    fn signal_states(&self, world: &World, t: SimTime) -> Vec<(SignalId, PhaseState)> {
        let t_s = ns_to_secs(t);
        let mut out = Vec::new();
        for plan in &world.signals {
            if let Some(state) = self.signals.phase_state(plan, t_s) {
                out.push((plan.id, state));
            }
        }
        out.sort_by_key(|(id, _)| *id);
        out
    }

    /// The junction an actor is approaching, if any is within the lookahead.
    fn junction_ahead(&self, world: &World, actor: &Actor, t: SimTime) -> Option<JunctionView> {
        let lane = world.try_lane(actor.lane)?;
        if lane.kind == LaneKind::Internal {
            return None; // already inside
        }
        let stop_line_gap_m = lane.length_m - actor.s_m;
        if stop_line_gap_m > self.params.lookahead_m {
            return None;
        }
        let next = actor.route.lanes.get(actor.route_index + 1).copied();
        self.junction_view(world, actor.lane, next, stop_line_gap_m, actor, t)
    }

    /// Every junction within the lookahead along the actor's route, nearest first: the
    /// one [`Self::junction_ahead`] finds, and — outside the legacy parity mode — the ones
    /// at the ends of the lanes after it.
    ///
    /// Looking only as far as the end of the lane the vehicle is on hid a signal whose
    /// approach lane was short — Manhattan has many a few metres long — until the
    /// vehicle was on that lane, by which time a red a few metres ahead could only be
    /// braked for at −6 m/s² and was run anyway.
    fn junctions_ahead(&self, world: &World, actor: &Actor, t: SimTime) -> Vec<JunctionView> {
        let mut out: Vec<JunctionView> = self.junction_ahead(world, actor, t).into_iter().collect();
        if !self.along_path() {
            return out;
        }
        let Some(lane) = world.try_lane(actor.lane) else {
            return out;
        };
        let mut dist = lane.length_m - actor.s_m;
        let mut k = actor.route_index + 1;
        while dist <= self.params.lookahead_m {
            let Some(id) = actor.route.lanes.get(k).copied() else {
                break;
            };
            let Some(l) = world.try_lane(id) else { break };
            let gap = dist + l.length_m;
            if l.kind != LaneKind::Internal && gap <= self.params.lookahead_m {
                let next = actor.route.lanes.get(k + 1).copied();
                if let Some(view) = self.junction_view(world, id, next, gap, actor, t) {
                    out.push(view);
                }
            }
            dist = gap;
            k += 1;
        }
        out
    }

    /// The junction at the end of `lane`, `gap` metres ahead of the actor, taken by the
    /// movement onto `next`.
    fn junction_view(
        &self,
        world: &World,
        lane_id: LaneId,
        next: Option<LaneId>,
        stop_line_gap_m: f64,
        actor: &Actor,
        t: SimTime,
    ) -> Option<JunctionView> {
        let lane = world.try_lane(lane_id)?;
        let junction_id = world.edge(lane.edge).to;
        let junction = world.roads.try_junction(junction_id)?;
        // The movement the route takes through it.
        let (movement, movement_lane) = match next {
            Some(next) => {
                let connection = world
                    .successors(lane_id)
                    .iter()
                    .find(|c| c.via == Some(next) || (c.via.is_none() && c.to_lane == next));
                match connection {
                    Some(c) => (c.direction, c.via.or(Some(next))),
                    None => self.fallback_movement(world, lane_id),
                }
            }
            None => (TurnDirection::Straight, None),
        };
        let major_lanes = u8::try_from(junction.incoming.len()).unwrap_or(u8::MAX);
        let signal = self.signal_for(world, junction_id, movement_lane, actor, t);
        Some(JunctionView {
            id: junction_id,
            position: junction.position,
            control: junction.control,
            stop_line_gap_m,
            movement,
            movement_lane,
            signal,
            major_lanes,
            waiting_s: actor
                .stuck_since
                .filter(|_| actor.speed_mps < STUCK_SPEED_MPS)
                .map_or(0.0, |since| ns_to_secs(t.saturating_sub(since))),
            gap_z: actor.traits.gap_z,
        })
    }

    /// The movement a vehicle whose route does not name one through this junction is
    /// judged by: the first permitted signal-controlled connector off its lane.
    ///
    /// A route that has lost its place — the defect that let a vehicle change lane round a
    /// queue and then drive through the junction — used to leave the movement unknown,
    /// and an unknown movement shows no signal, so the vehicle proceeded on red. It is now
    /// held to the lane's own movements. Not applied in the legacy parity mode.
    fn fallback_movement(&self, world: &World, lane: LaneId) -> (TurnDirection, Option<LaneId>) {
        if !self.along_path() {
            return (TurnDirection::Straight, None);
        }
        world
            .successors(lane)
            .iter()
            .find(|c| c.permitted && c.via.is_some())
            .map_or((TurnDirection::Straight, None), |c| (c.direction, c.via))
    }

    /// The state the actor's movement is being shown, if this junction shows one.
    fn signal_for(
        &self,
        world: &World,
        junction: JunctionId,
        movement_lane: Option<LaneId>,
        actor: &Actor,
        t: SimTime,
    ) -> Option<SignalState> {
        let t_s = ns_to_secs(t);
        match self.params.intersections {
            IntersectionMode::TwoColoringLegacy => {
                let colouring = self.two_coloring.as_ref()?;
                let heading = world.lane(actor.lane).heading_at(actor.s_m);
                Some(colouring.state_at(junction, heading, t_s))
            }
            IntersectionMode::None => None,
            _ => {
                let plan_id = match world.roads.try_junction(junction)?.control {
                    JunctionControl::Signalised { plan } => plan,
                    _ => return None,
                };
                let plan = world.signal_plan(plan_id)?;
                let lane = movement_lane?;
                self.signals.state_for(plan, lane, t_s)
            }
        }
    }

    /// Pass 5: every actor's claim on the junction it is approaching, by junction.
    fn claims(
        &self,
        world: &World,
        snapshot: &ActorSnapshot,
        t: SimTime,
    ) -> BTreeMap<JunctionId, Vec<ConflictView>> {
        let mut out: BTreeMap<JunctionId, Vec<ConflictView>> = BTreeMap::new();
        for actor in self.actors.values() {
            let Some(view) = self.junction_ahead(world, actor, t) else {
                continue;
            };
            let Some(state) = snapshot.view(actor.id) else {
                continue;
            };
            out.entry(view.id).or_default().push(ConflictView {
                actor: actor.id,
                stop_line_gap_m: view.stop_line_gap_m,
                speed_mps: state.speed_mps,
                heading_rad: state.heading_rad,
                movement: view.movement,
                movement_lane: view.movement_lane,
                // Filled in per ego below: whether a claim conflicts is a property of the
                // *pair* of movements, not of the claim.
                conflicts: false,
                ego_must_yield: false,
                // Its own tracked arrival: its launch at its own acceleration up to the
                // speed it wants on its lane (SUMO MSLink::setApproaching).
                arrival_s: Some(merge_arrival_s(
                    view.stop_line_gap_m,
                    state.speed_mps,
                    actor.driver.max_accel_mps2,
                    actor
                        .driver
                        .desired_speed_mps
                        .min(world.lane(actor.lane).speed_limit_mps),
                )),
            });
        }
        for list in out.values_mut() {
            list.sort_by_key(|c| c.actor);
        }
        out
    }

    /// The claims at `junction` as the ego sees them: conflict and priority filled in.
    fn conflicts_for(
        &self,
        world: &World,
        junction: &JunctionView,
        ego: &VehicleView,
        claims: &BTreeMap<JunctionId, Vec<ConflictView>>,
        t: SimTime,
    ) -> Vec<ConflictView> {
        let Some(all) = claims.get(&junction.id) else {
            return Vec::new();
        };
        let matrix = world
            .roads
            .try_junction(junction.id)
            .map(|j| (j.internal.clone(), j.conflicts.clone()));
        let ego_row = matrix.as_ref().and_then(|(internal, _)| {
            junction
                .movement_lane
                .and_then(|l| internal.iter().position(|i| *i == l))
        });
        all.iter()
            .filter(|c| c.actor != ego.actor)
            .map(|c| {
                let other_row = matrix.as_ref().and_then(|(internal, _)| {
                    c.movement_lane
                        .and_then(|l| internal.iter().position(|i| *i == l))
                });
                // The world's conflict matrix decides when both movements are in it; a
                // world without internal connectors (a ring, a legacy import) falls back on
                // the geometric rule of §2.3 and the legacy closest-first priority.
                let ego_closer = junction.stop_line_gap_m < c.stop_line_gap_m
                    || (junction.stop_line_gap_m == c.stop_line_gap_m && ego.actor < c.actor);
                // Two paths that cross are foes whatever the matrix says: the procedural
                // grid's matrix left a permitted left turn and the opposing through
                // movement unmarked, the turner never gave way, and the two collided inside
                // the junction (an overlap and a −6 m/s² stop on the dense grid). The
                // matrix still decides priority.
                let crosses = self.along_path()
                    && junction.movement_lane.is_some_and(|mine| {
                        self.zones
                            .of(mine)
                            .iter()
                            .any(|z| !z.merge && Some(z.other) == c.movement_lane)
                    });
                let (conflicts, ego_must_yield) = match (matrix.as_ref(), ego_row, other_row) {
                    (Some((_, m)), Some(a), Some(b)) => {
                        let foe = m.is_foe(a, b) || crosses;
                        let level = foe && !m.must_yield(a, b) && !m.must_yield(b, a);
                        // A conflicting pair the matrix leaves level — two opposing left
                        // turns, which no highway code ranks — used to be yielded by
                        // neither, so both entered together. The closest-first order
                        // settles it, as it settles a world with no matrix at all. Not
                        // applied in the legacy parity mode.
                        if level && self.along_path() {
                            (true, !ego_closer)
                        } else {
                            (foe, m.must_yield(a, b))
                        }
                    }
                    _ => {
                        let conflicts = headings_conflict(ego.heading_rad, c.heading_rad);
                        // The legacy rule: the closest claimant has priority, ties by the
                        // lower actor id — a strict total order, so the yield relation is
                        // acyclic and somebody always makes progress.
                        let ego_closer = junction.stop_line_gap_m < c.stop_line_gap_m
                            || (junction.stop_line_gap_m == c.stop_line_gap_m
                                && ego.actor < c.actor);
                        (conflicts, conflicts && !ego_closer)
                    }
                };
                // A claimant held by a red is not coming. Counting it had a left turner on
                // a permissive green wait for the cross street's car standing at *its* red
                // — which in turn waited for the left turner — for good.
                let held = self.along_path() && self.held_by_signal(world, junction.id, c, t);
                ConflictView {
                    conflicts: conflicts && !held,
                    ego_must_yield: ego_must_yield && !held,
                    ..*c
                }
            })
            .collect()
    }

    /// True if the claimant's own movement is showing red (or red-amber) at `t` and it can
    /// still stop for it. One that cannot — it went on the amber, and the red came up
    /// while it was a car length from the line — is coming whatever the light says, and
    /// counting it as held let the cross street's first car start into its path at the
    /// green (an overlap and a −6 m/s² stop inside the junction on the dense grid).
    fn held_by_signal(
        &self,
        world: &World,
        junction: JunctionId,
        c: &ConflictView,
        t: SimTime,
    ) -> bool {
        let Some(lane) = c.movement_lane else {
            return false;
        };
        let Some(JunctionControl::Signalised { plan }) =
            world.roads.try_junction(junction).map(|j| j.control)
        else {
            return false;
        };
        let Some(plan) = world.signal_plan(plan) else {
            return false;
        };
        let red = matches!(
            self.signals.state_for(plan, lane, ns_to_secs(t)),
            Some(SignalState::Red | SignalState::RedAmber)
        );
        let can_stop = c.speed_mps * c.speed_mps / (2.0 * PLANNED_STOP_MAX_DECEL_MPS2)
            <= c.stop_line_gap_m.max(0.0) + 0.5;
        red && can_stop
    }

    /// True if a conflicting claimant whose own movement is not green — the last car in on
    /// an amber — is about to enter the junction: within [`CLEARING_WINDOW_S`] of its stop
    /// line and moving. A driver starting on a fresh green gives way to it, as UVC
    /// §11-202(a)1 has every driver on a green give way to vehicles lawfully within the
    /// junction; the all-red interval exists for the same car, and a plan without one (a
    /// procedural grid) leaves only the driver to do it.
    fn clearing_foe(
        &self,
        world: &World,
        junction: JunctionId,
        conflicts: &[ConflictView],
        t: SimTime,
    ) -> bool {
        let Some(JunctionControl::Signalised { plan }) =
            world.roads.try_junction(junction).map(|j| j.control)
        else {
            return false;
        };
        let Some(plan) = world.signal_plan(plan) else {
            return false;
        };
        conflicts.iter().any(|c| {
            c.conflicts
                && c.speed_mps > 1.0
                && c.stop_line_gap_m > 0.0
                && c.stop_line_gap_m / c.speed_mps < CLEARING_WINDOW_S
                && c.movement_lane.is_some_and(|l| {
                    !matches!(
                        self.signals.state_for(plan, l, ns_to_secs(t)),
                        Some(SignalState::Green | SignalState::GreenYield) | None
                    )
                })
        })
    }

    /// The intersection decision for one actor.
    fn entry_decision(
        &self,
        world: &World,
        ego: &VehicleView,
        junction: &JunctionView,
        conflicts: &[ConflictView],
    ) -> EntryDecision {
        let signalised = matches!(junction.control, JunctionControl::Signalised { .. });
        match self.params.intersections {
            IntersectionMode::None => EntryDecision::Proceed,
            IntersectionMode::TwoColoringLegacy => match self.two_coloring.as_ref() {
                Some(c) => c.may_enter(ego, junction, conflicts, &self.weather),
                None => EntryDecision::Proceed,
            },
            IntersectionMode::SignalsOnly => {
                if signalised {
                    self.signals
                        .may_enter(ego, junction, conflicts, &self.weather)
                } else {
                    EntryDecision::Proceed
                }
            }
            IntersectionMode::GapAcceptanceOnly => {
                self.gap.may_enter(ego, junction, conflicts, &self.weather)
            }
            IntersectionMode::SignalsAndGapAcceptance => {
                if signalised {
                    self.signals
                        .may_enter(ego, junction, conflicts, &self.weather)
                } else {
                    let _ = world;
                    self.gap.may_enter(ego, junction, conflicts, &self.weather)
                }
            }
        }
    }

    /// Whether a vehicle may consider a discretionary lane change at all this step.
    ///
    /// Not on a junction connector, not while its body still overhangs the lane it came
    /// from, and not once it is inside the no-change zone before the end of its lane —
    /// MUTCD 2009 §3B.04's solid lane line on the approach to a junction, sized so the
    /// lateral transition finishes before the stop line. Changing lane inside that zone is
    /// what put vehicles round a queue standing at a red light.
    fn may_consider_lane_change(&self, world: &World, actor: &Actor, mobil: &Mobil) -> bool {
        let lane = world.lane(actor.lane);
        if lane.kind == LaneKind::Internal || actor.s_m < actor.class.spec().length_m {
            return false;
        }
        // The zone is the approach to a junction: a lane that simply continues into the
        // next one (a ring, a road split at a shape point) has no stop line to protect.
        if !world.successors(actor.lane).iter().any(|c| c.via.is_some()) {
            return true;
        }
        let zone = self
            .params
            .no_change_zone_m
            .max(actor.speed_mps * mobil.params().transition_s);
        lane.length_m - actor.s_m >= zone
    }

    /// Settles the lane changes pass 6 decided, in actor-id order, and returns the route
    /// each surviving change drives on.
    ///
    /// * **The route.** The vehicle's route is re-planned from the target lane; a change
    ///   whose target lane cannot continue to the same next road as the route it leaves
    ///   (or cannot reach the destination at all) is refused.
    /// * **Two into one gap.** Every decision was taken on the frozen snapshot, so two
    ///   vehicles from the lanes either side could each pick the same gap, and two
    ///   vehicles side by side could swap lanes through each other. Each change is checked
    ///   against every change already accepted into the same lane (or swapping with it),
    ///   and refused when the two would be closer than the follower's standstill gap plus
    ///   one second of its speed.
    ///
    /// Actor-id order makes it a function of the decisions, not of the walk order, so the
    /// Jacobi property holds.
    fn resolve_lane_changes(
        &self,
        world: &World,
        t: SimTime,
        decisions: &mut [Decision],
    ) -> BTreeMap<ActorId, (Route, LaneId)> {
        let costs = self.costs(world);
        let mut order: Vec<usize> = (0..decisions.len())
            .filter(|i| matches!(decisions[*i].lane_change, LaneChangeDecision::Change { .. }))
            .collect();
        order.sort_by_key(|i| decisions[*i].actor);
        let mut accepted: Vec<(ActorId, LaneId, LaneId)> = Vec::new();
        let mut out = BTreeMap::new();
        for i in order {
            let LaneChangeDecision::Change { to, .. } = decisions[i].lane_change else {
                continue;
            };
            let Some(actor) = self.actors.get(&decisions[i].actor) else {
                continue;
            };
            let from = actor.lane;
            let clash = accepted.iter().any(|(other, o_from, o_to)| {
                let same_gap = *o_to == to || (*o_from == to && *o_to == from);
                if !same_gap {
                    return false;
                }
                let Some(o) = self.actors.get(other) else {
                    return false;
                };
                let (lead, follow) = if o.s_m >= actor.s_m {
                    (o, actor)
                } else {
                    (actor, o)
                };
                let gap = lead.s_m - lead.class.spec().length_m - follow.s_m;
                gap < follow.driver.min_gap_m + follow.speed_mps * 1.0
            });
            // Nor into a gap the vehicle does not fit: the neighbour query floors a gap at
            // zero, and the car-following model judged a faster car cutting in with its rear
            // 1.6 m *behind* a cyclist's front harmless — the cyclist need hardly brake for
            // a leader pulling away — so the change began with the two side by side and the
            // cyclist inside the car's standstill gap. The car must be clear of everything
            // on the target lane by each follower's standstill gap.
            let alongside = self.along_path()
                && self.actors.values().any(|o| {
                    if o.id == actor.id || o.lane != to {
                        return false;
                    }
                    if o.s_m <= actor.s_m {
                        actor.s_m - actor.class.spec().length_m - o.s_m < o.driver.min_gap_m
                    } else {
                        o.s_m - o.class.spec().length_m - actor.s_m < actor.driver.min_gap_m
                    }
                });
            // Nor between lanes that are not side by side where the slide happens: near a
            // junction an imported lane can bend round a corner its neighbour does not, and
            // the lateral transition then cut the corner through the building on it.
            let skewed = self.along_path() && !lanes_parallel_ahead(world, actor, from, to);
            let route = if clash || alongside || skewed {
                None
            } else {
                self.route_after_change(world, actor, to, t, &costs)
                    .or_else(|| {
                        if decisions[i].detour_ok {
                            self.replan_from(world, actor, to, t, &costs)
                        } else {
                            None
                        }
                    })
            };
            match route {
                Some(r) => {
                    accepted.push((actor.id, from, to));
                    out.insert(actor.id, r);
                }
                None => decisions[i].lane_change = LaneChangeDecision::Stay,
            }
        }
        out
    }

    /// The route (and destination) a vehicle drives after changing onto `to`, or `None` if
    /// that lane cannot make the route's next movement.
    ///
    /// The remaining route is *shifted* onto the new lane: movement by movement, the
    /// connection off the current lane that leaves for the same road as the old route's —
    /// the same connector when there is one — so the trip keeps its roads, a closed
    /// circuit stays a circuit, and no router is run on the common path. Where a later
    /// movement cannot be made from the shifted lane, the rest is re-planned from there to
    /// the destination road. A target lane that cannot make even the *next* movement is
    /// refused: this model has no mandatory lane change to get back.
    fn route_after_change(
        &self,
        world: &World,
        actor: &Actor,
        to: LaneId,
        t: SimTime,
        costs: &DynamicCost<'_>,
    ) -> Option<(Route, LaneId)> {
        let target = world.try_lane(to)?;
        let old = &actor.route.lanes;
        let dest_edge = world.try_lane(actor.destination).map(|l| l.edge);
        // A strategic change keeps the route it was made for: the lanes after this one
        // stay, and the next of them is reached from the target lane or from one further
        // across ([`Self::strategic_target`]).
        if target.edge == world.lane(actor.lane).edge
            && self.strategic_target(world, actor).is_some()
        {
            let mut lanes = vec![to];
            lanes.extend(old.iter().skip(actor.route_index + 1).copied());
            return Some((Self::route_of(world, lanes), actor.destination));
        }
        if old.len() <= actor.route_index + 1 && dest_edge == Some(target.edge) {
            // Changing lane on the destination road: the trip ends at the end of this lane.
            return Some((Self::route_of(world, vec![to]), to));
        }
        let mut lanes = vec![to];
        let mut cur = to;
        let mut k = actor.route_index + 1;
        let mut stuck = false;
        while k < old.len() {
            let o = world.lane(old[k]);
            let (via, exit_edge, want_exit) = if o.kind == LaneKind::Internal {
                let Some(exit) = old.get(k + 1).copied() else {
                    break;
                };
                (true, world.lane(exit).edge, exit)
            } else {
                (false, o.edge, old[k])
            };
            let pick = world
                .successors(cur)
                .iter()
                .filter(|c| {
                    c.permitted && c.via.is_some() == via && world.lane(c.to_lane).edge == exit_edge
                })
                .min_by_key(|c| (c.to_lane != want_exit, c.via != Some(old[k]), c.to_lane));
            let Some(c) = pick.copied() else {
                stuck = true;
                break;
            };
            if let Some(i) = c.via {
                lanes.push(i);
                k += 2;
            } else {
                k += 1;
            }
            lanes.push(c.to_lane);
            cur = c.to_lane;
        }
        if !stuck {
            let destination = *lanes.last().expect("starts with the target lane");
            return Some((Self::route_of(world, lanes), destination));
        }
        if lanes.len() == 1 {
            return None; // the target lane cannot make the next movement
        }
        // The shift stopped on a later road whose lane cannot make the old route's next
        // movement. A driver who changed lane for speed changes back on that road when
        // another of its lanes makes the turn and the road is long enough for it, as the
        // strategic change of a planned route does ([`Self::strategic_target`]); the old
        // route is kept from that movement on. Re-planning from the shifted lane alone
        // took a detour of minutes round the block for want of one lane change (grid at
        // 600 veh/h: 296 s against SUMO's 109 s on one trip).
        if self.params.strategic_lane_choice && k < old.len() {
            let here = world.lane(cur);
            let next = old[k];
            let reaches = |l: LaneId| {
                world
                    .successors(l)
                    .iter()
                    .any(|c| c.permitted && c.via.unwrap_or(c.to_lane) == next)
            };
            let feasible = here.kind != LaneKind::Internal
                && world.edge(here.edge).lanes.iter().any(|l| {
                    let o = world.lane(*l);
                    *l != cur
                        && o.admits(actor.class.class_mask())
                        && reaches(*l)
                        && here.length_m
                            >= self.params.no_change_zone_m
                                + f64::from(o.index.abs_diff(here.index)) * STRATEGIC_M_PER_CHANGE
                });
            if feasible {
                lanes.extend(old.iter().skip(k).copied());
                return Some((Self::route_of(world, lanes), actor.destination));
            }
            // No room to change back on that road: the change would cost the turn, so it
            // is refused, as LC2013 refuses a speed-gain change off the lanes its route
            // needs ahead (its best-lanes look-ahead over the next roads). Allowing it and
            // re-planning from the stuck lane sent cars round the block (Manhattan extract
            // at 2400 veh/h: 223 trips completed with it, 225 without; SUMO 232).
            return None;
        }
        // Re-plan the rest from where the shift stopped, to any lane of the destination
        // road.
        let dest_lanes: Vec<LaneId> = dest_edge
            .map(|e| world.edge(e).lanes.clone())
            .unwrap_or_default();
        for d in std::iter::once(actor.destination).chain(dest_lanes) {
            if let Some(rest) = self.router_for(actor.class).replan(world, cur, d, t, costs) {
                lanes.extend(rest.lanes.into_iter().skip(1));
                return Some((Self::route_of(world, lanes), d));
            }
        }
        None
    }

    /// A route from `from` to the vehicle's destination road, whatever its old route was:
    /// what a driver who had to leave their lane to get round an obstruction does when the
    /// lane they are now in cannot make the turn they meant to take.
    fn replan_from(
        &self,
        world: &World,
        actor: &Actor,
        from: LaneId,
        t: SimTime,
        costs: &DynamicCost<'_>,
    ) -> Option<(Route, LaneId)> {
        let dest_lanes: Vec<LaneId> = world
            .try_lane(actor.destination)
            .map(|l| world.edge(l.edge).lanes.clone())
            .unwrap_or_default();
        for d in std::iter::once(actor.destination).chain(dest_lanes) {
            if let Some(r) = self.plan_route(actor.class, world, from, d, t, costs) {
                return Some((r, d));
            }
        }
        None
    }

    /// The lane change that takes a vehicle round the vehicle standing at the kerb ahead
    /// of it, if there is room for it now.
    ///
    /// The leader must be standing at the kerb ([`crate::kerbside`]) and within
    /// [`KERB_PASS_LOOK_M`]. A side qualifies when the lane beside has no standing vehicle
    /// closer than the far end of the one being passed, the ego fits ahead of that lane's
    /// follower, and that follower need brake no harder than
    /// [`PULL_OUT_FOLLOWER_DECEL_MPS2`] for it. Left before right. MOBIL's minimum speed
    /// and reconsideration rate do not apply: a driver stuck behind a double-parked van
    /// pulls out the moment a gap comes, and a moving one changes lanes well before
    /// reaching it.
    fn pass_kerbside(
        &self,
        world: &World,
        mobil: &Mobil,
        at_kerb: &BTreeSet<ActorId>,
        ego: &VehicleView,
        nbrs: &LaneNeighbors,
    ) -> Option<LaneChangeDecision> {
        let lead = nbrs.leader?;
        let blocker = lead.vehicle?;
        if !at_kerb.contains(&blocker.actor) || lead.gap_m > KERB_PASS_LOOK_M {
            return None;
        }
        for side in [Side::Left, Side::Right] {
            let Some(target) = nbrs.side(side) else {
                continue;
            };
            let Some(target_lane) = world.try_lane(target.lane) else {
                continue;
            };
            if let Some(t_lead) = target.leader {
                if t_lead.gap_m < ego.driver.min_gap_m + 1.0 {
                    continue;
                }
                if t_lead.vehicle.is_some_and(|v| at_kerb.contains(&v.actor)) {
                    continue;
                }
                if t_lead.speed_mps < 1.0 && t_lead.gap_m < lead.gap_m + blocker.dims.length_m + 2.0
                {
                    continue;
                }
            }
            if let Some(f) = target.follower
                && let Some(fv) = f.vehicle.as_ref()
            {
                if f.gap_m < fv.driver.min_gap_m {
                    continue;
                }
                let a = VehicleCf(&self.cf, &self.vtype_cf).accel(
                    fv,
                    Some(&LeaderView::of(*ego, f.gap_m)),
                    &LaneView::of(target_lane),
                    &self.weather,
                );
                if a < -PULL_OUT_FOLLOWER_DECEL_MPS2 {
                    continue;
                }
            }
            let lateral = 0.5 * (world.lane(ego.lane).width_m + target_lane.width_m);
            return Some(LaneChangeDecision::Change {
                to: target.lane,
                side,
                incentive_mps2: 0.0,
                duration: crate::lanechange::mobil::transition_duration(
                    mobil.params(),
                    lateral,
                    ego.speed_mps.max(PULL_OUT_SPEED_MPS),
                ),
            });
        }
        None
    }

    /// The lane change to the right that takes a vehicle due a kerbside stop towards the
    /// kerb, if the lane to its right has room for it now: the ego fits behind that lane's
    /// leader without braking harder than [`PULL_OUT_FOLLOWER_DECEL_MPS2`], and that lane's
    /// follower need brake no harder than that for it. The route must still make its next
    /// movement from the new lane (the usual lane-change rule): a driver about to turn left
    /// does not pull over to the right first.
    fn move_to_kerb(
        &self,
        world: &World,
        mobil: &Mobil,
        ego: &VehicleView,
        nbrs: &LaneNeighbors,
    ) -> Option<LaneChangeDecision> {
        let target = nbrs.right?;
        let target_lane = world.try_lane(target.lane)?;
        if !matches!(target_lane.kind, LaneKind::Driving | LaneKind::Bus) {
            return None;
        }
        let lane_view = LaneView::of(target_lane);
        if let Some(l) = target.leader {
            if l.gap_m < ego.driver.min_gap_m + 1.0 {
                return None;
            }
            let mut moved = *ego;
            moved.lane = target.lane;
            if VehicleCf(&self.cf, &self.vtype_cf).accel(&moved, Some(&l), &lane_view, &self.weather)
                < -PULL_OUT_FOLLOWER_DECEL_MPS2
            {
                return None;
            }
        }
        if let Some(f) = target.follower
            && let Some(fv) = f.vehicle.as_ref()
        {
            if f.gap_m < fv.driver.min_gap_m {
                return None;
            }
            let a = VehicleCf(&self.cf, &self.vtype_cf).accel(
                fv,
                Some(&LeaderView::of(*ego, f.gap_m)),
                &lane_view,
                &self.weather,
            );
            if a < -PULL_OUT_FOLLOWER_DECEL_MPS2 {
                return None;
            }
        }
        let lateral = 0.5 * (world.lane(ego.lane).width_m + target_lane.width_m);
        Some(LaneChangeDecision::Change {
            to: target.lane,
            side: Side::Right,
            incentive_mps2: 0.0,
            duration: crate::lanechange::mobil::transition_duration(
                mobil.params(),
                lateral,
                ego.speed_mps.max(PULL_OUT_SPEED_MPS),
            ),
        })
    }

    /// A route over `lanes`, with its length and free-flow cost.
    fn route_of(world: &World, lanes: Vec<LaneId>) -> Route {
        let length_m = math::sum_ordered(
            lanes
                .iter()
                .map(|l| world.lane(*l).length_m)
                .collect::<Vec<_>>(),
        );
        let cost_s = math::sum_ordered(
            lanes
                .iter()
                .map(|l| {
                    let l = world.lane(*l);
                    l.length_m / l.speed_limit_mps.max(1e-9)
                })
                .collect::<Vec<_>>(),
        );
        Route {
            lanes,
            length_m,
            cost_s,
        }
    }

    /// Who is inside which junction connector at the start of the step: `(actor, rear,
    /// front)` in arc length along the connector, for every vehicle whose body is on one —
    /// including one whose front has already left it and whose rear has not.
    fn occupancy(&self, world: &World) -> BTreeMap<LaneId, Vec<(ActorId, f64, f64)>> {
        let mut out: BTreeMap<LaneId, Vec<(ActorId, f64, f64)>> = BTreeMap::new();
        if !self.along_path() || !self.params.junction_clearance {
            return out;
        }
        for a in self.actors.values() {
            for (lane, rear, front) in a.connector_spans(world) {
                out.entry(lane).or_default().push((a.id, rear, front));
            }
        }
        out
    }

    /// Whether `actor` must wait at the stop line before taking `movement`, although the
    /// junction's own control would let it go:
    ///
    /// * a vehicle already inside the junction on a conflicting movement has not yet
    ///   cleared the zone the two paths share — UVC §11-202(a)1 / NY VTL §1111(a)1: a
    ///   driver facing a green "shall yield the right of way to other vehicles … lawfully
    ///   within the intersection";
    /// * the movement's exit lane has no room for the vehicle behind a queue standing on
    ///   it — NY VTL §1175, "no driver shall enter an intersection … unless there is
    ///   sufficient space on the opposite side … to accommodate the vehicle", the rule
    ///   that keeps a queue from spilling back across the junction and locking the grid.
    fn junction_blocked(
        &self,
        world: &World,
        snapshot: &ActorSnapshot,
        occupancy: &BTreeMap<LaneId, Vec<(ActorId, f64, f64)>>,
        actor: &Actor,
        movement: LaneId,
    ) -> bool {
        for z in self.zones.of(movement) {
            if z.merge {
                continue; // a merge is followed through, not waited for: `junction_leader`
            }
            if let Some(list) = occupancy.get(&z.other)
                && list.iter().any(|(other, rear, front)| {
                    *other != actor.id
                        && z.other_not_clear(*rear)
                        // A permitted left turner standing in the junction short of this
                        // zone is waiting for this vehicle, not crossing it (box entry,
                        // `permissive_sneakers`).
                        && !(self.params.permissive_sneakers > 0
                            && *front < z.s_other - z.half_other
                            && self.opposing_approaches(world, movement, z.other)
                            && snapshot
                                .view(*other)
                                .is_some_and(|v| v.speed_mps < STUCK_SPEED_MPS))
                })
            {
                return true;
            }
        }
        let Some(exit) = self.zones.exit_of(movement) else {
            return false;
        };
        let Some(&(tail_front, tail)) = snapshot.on_lane(exit).first() else {
            return false;
        };
        let Some(tail_view) = snapshot.view(tail) else {
            return false;
        };
        if tail_view.speed_mps > EXIT_QUEUE_MOVING_MPS {
            return false; // the queue is moving: the room is opening
        }
        let need = actor.class.spec().length_m + actor.driver.min_gap_m;
        let mut room = tail_front - tail_view.dims.length_m;
        // Whoever is already inside heading for the same exit takes room first.
        for (lane, list) in occupancy {
            if self.zones.exit_of(*lane) != Some(exit) {
                continue;
            }
            for (other, _, _) in list {
                if *other == actor.id {
                    continue;
                }
                if let Some(v) = snapshot.view(*other) {
                    room -= v.dims.length_m + v.driver.min_gap_m;
                }
            }
        }
        let _ = world;
        room < need
    }

    /// The constraint other vehicles inside a junction put on `actor`, as a virtual leader.
    ///
    /// * **A merge** (two connectors ending on one exit lane): whichever of the two is
    ///   nearer the merge point goes first and the other follows it, at the gap their
    ///   distances to the merge point imply — a zip, not a stop. Ties go to the lower id.
    /// * **A crossing, both inside**: a vehicle already inside the junction that will
    ///   reach (or is in) a zone the actor's path crosses before the actor does, and has
    ///   not cleared it, is waited for at the zone's edge. The actor waits only if it has
    ///   not yet entered the zone itself; ties go to the lower id. (A vehicle still
    ///   *approaching* is held at the stop line instead, by [`Self::junction_blocked`].)
    fn junction_leader(
        &self,
        world: &World,
        snapshot: &ActorSnapshot,
        occupancy: &BTreeMap<LaneId, Vec<(ActorId, f64, f64)>>,
        claims: &BTreeMap<JunctionId, Vec<ConflictView>>,
        actor: &Actor,
        t: SimTime,
    ) -> Option<JunctionConstraint> {
        let lane = world.lane(actor.lane);
        // The connector the actor is on or about to take, and its front's arc length
        // along it (negative before the stop line).
        let (movement, pos) = if lane.kind == LaneKind::Internal {
            (actor.lane, actor.s_m)
        } else {
            let gap = lane.length_m - actor.s_m;
            if gap > self.params.lookahead_m {
                return None;
            }
            let next = actor.route.lanes.get(actor.route_index + 1).copied()?;
            if world.try_lane(next)?.kind != LaneKind::Internal {
                return None;
            }
            (next, -gap)
        };
        let inside = pos >= 0.0;
        let len_m = world.lane(movement).length_m;
        let mut best: Option<LeaderView> = None;
        let mut merge: Option<MergePartner> = None;
        let mut take_merge = |m: MergePartner| {
            if merge.as_ref().is_none_or(|b| m.gap_m < b.gap_m) {
                merge = Some(m);
            }
        };
        // Vehicles still approaching on a movement that merges with ours are in the
        // order too: two that entered a short merge in the same step — each judging the
        // other not yet inside — could not sort themselves out in a 4 m connector.
        let approaching: &[ConflictView] = world
            .lane(movement)
            .junction
            .and_then(|j| claims.get(&j))
            .map_or(&[], Vec::as_slice);
        // Who reaches the merge point first is decided by *time*, from each vehicle's own
        // start-of-step speed ([`merge_arrival_s`]), not by distance: a car standing 40 m
        // from the merge behind its own queue is not ahead of one arriving at 11 m/s from
        // 45 m, and ordering the two by distance projected the standing one onto the
        // moving one's path at a zero gap — a phantom obstacle braked for at −6 m/s² (the
        // auditor's jerk class). The rule is the same from both sides, so the two agree.
        let self_view = snapshot.view(actor.id);
        let self_speed = self_view.map_or(actor.speed_mps, |v| v.speed_mps);
        let t_self = merge_arrival_s(
            len_m - pos,
            self_speed,
            actor.driver.max_accel_mps2,
            self.connector_speed(world, movement),
        );
        for z in self.zones.of(movement).iter().filter(|z| z.merge) {
            let len_o = world.lane(z.other).length_m;
            let cap_o = self.connector_speed(world, z.other);
            for c in approaching
                .iter()
                .filter(|c| c.movement_lane == Some(z.other))
            {
                if c.actor == actor.id {
                    continue;
                }
                // One held at a red is not merging yet.
                if let Some(j) = world.lane(movement).junction
                    && self.held_by_signal(world, j, c, t)
                {
                    continue;
                }
                let Some(view) = snapshot.view(c.actor) else {
                    continue;
                };
                let d_self = len_m - pos;
                let d_other = len_o + c.stop_line_gap_m;
                let t_other =
                    merge_arrival_s(d_other, view.speed_mps, view.driver.max_accel_mps2, cap_o);
                // One still before its stop line is never ahead of us once we are in the
                // stretch where the paths converge (see the rule for vehicles inside).
                let self_in = pos >= z.s_self - z.half_self;
                if !self_in && (t_other < t_self || (t_other == t_self && c.actor < actor.id)) {
                    take_merge(MergePartner {
                        view: *view,
                        gap_m: d_self - d_other - view.dims.length_m,
                        d_self_m: d_self,
                    });
                }
            }
        }
        for z in self.zones.of(movement) {
            let Some(list) = occupancy.get(&z.other) else {
                continue;
            };
            let len_o = world.lane(z.other).length_m;
            for &(other, rear, front) in list {
                if other == actor.id {
                    continue;
                }
                let Some(view) = snapshot.view(other) else {
                    continue;
                };
                if z.merge {
                    let d_self = len_m - pos;
                    let d_other = len_o - front;
                    let t_other = merge_arrival_s(
                        d_other,
                        view.speed_mps,
                        view.driver.max_accel_mps2,
                        self.connector_speed(world, z.other),
                    );
                    // A car already in the stretch where the two paths converge is ahead of
                    // one that is not, whatever their arrival times: the time rule let a car
                    // creeping at 0.1 m/s count itself first against one standing half in
                    // the merge, and crawl into it (333 overlap steps on dense Midtown).
                    // Both in it: the one nearer the merge point. The rule reads the same
                    // from both sides, so the two agree.
                    let other_in = front >= z.s_other - z.half_other;
                    let self_in = pos >= z.s_self - z.half_self;
                    let ahead = match (other_in, self_in) {
                        (true, false) => true,
                        (false, true) => false,
                        (true, true) => d_other < d_self || (d_other == d_self && other < actor.id),
                        (false, false) => {
                            t_other < t_self || (t_other == t_self && other < actor.id)
                        }
                    };
                    if !ahead || rear > len_o {
                        continue; // behind us, or already off the connector
                    }
                    take_merge(MergePartner {
                        view: *view,
                        gap_m: d_self - d_other - view.dims.length_m,
                        d_self_m: d_self,
                    });
                } else if inside {
                    let edge = z.s_self - z.half_self;
                    if pos > edge || !z.other_not_clear(rear) {
                        continue; // we are in it already, or they have cleared it
                    }
                    let d_self = edge - pos;
                    let d_other = (z.s_other - z.half_other) - front;
                    let first = d_other <= 0.0
                        || d_other < d_self
                        || (d_other == d_self && other < actor.id);
                    if first {
                        let l = LeaderView::virtual_obstacle(d_self.max(0.0), 0.0);
                        best = Self::closest(best, Some(l));
                    }
                }
            }
        }
        Some(JunctionConstraint {
            crossing: best,
            merge,
        })
    }

    /// How long the amber shown to the movement on `movement_lane` at `junction` has left
    /// at `t`, seconds: `Some(0)` if it shows no amber, `None` if nothing is signalled.
    /// Whether connectors `a` and `b` come from opposite approaches (headings at the
    /// approach lanes' ends more than 150° apart).
    fn opposing_approaches(&self, world: &World, a: LaneId, b: LaneId) -> bool {
        let (Some(la), Some(lb)) = (self.zones.approach_of(a), self.zones.approach_of(b)) else {
            return false;
        };
        let (la, lb) = (world.lane(la), world.lane(lb));
        v2xw_world::model::normalise_angle(la.heading_at(la.length_m) - lb.heading_at(lb.length_m))
            .abs()
            > core::f64::consts::PI * (150.0 / 180.0)
    }

    /// Where on permitted-left connector `movement` a turner waiting inside the junction
    /// stops, metres of arc length: [`BOX_WAIT_MARGIN_M`] short of the first zone it
    /// shares with a movement from the opposing approach, the stream it waits for. `None`
    /// for a movement that does not cross the opposing stream, or whose first such zone is
    /// too near the line to wait short of.
    fn box_wait_s(&self, world: &World, movement: LaneId, turn: TurnDirection) -> Option<f64> {
        if !crate::intersection::signal_fixed_time::crosses_opposing(turn) {
            return None;
        }
        self.zones.approach_of(movement)?;
        let first = self
            .zones
            .of(movement)
            .iter()
            .filter(|z| !z.merge)
            .filter(|z| self.opposing_approaches(world, movement, z.other))
            .map(|z| z.s_self - z.half_self)
            .fold(f64::INFINITY, f64::min);
        let w = first - BOX_WAIT_MARGIN_M;
        (w.is_finite() && w >= BOX_WAIT_MIN_M).then_some(w)
    }

    /// The junction a permitted left turner waiting inside it is judged at, with the gap to
    /// its wait point ([`Self::box_wait_s`]), metres; `None` unless the actor is on a
    /// signalised permitted-left connector and not yet past the wait point.
    fn box_wait_view(
        &self,
        world: &World,
        actor: &Actor,
        t: SimTime,
    ) -> Option<(JunctionView, f64)> {
        let lane = world.try_lane(actor.lane)?;
        if lane.kind != LaneKind::Internal {
            return None;
        }
        let junction_id = lane.junction?;
        let junction = world.roads.try_junction(junction_id)?;
        let JunctionControl::Signalised { plan } = junction.control else {
            return None;
        };
        let from = self.zones.approach_of(actor.lane)?;
        let turn = world
            .successors(from)
            .iter()
            .find(|c| c.via == Some(actor.lane))?
            .direction;
        let w = self.box_wait_s(world, actor.lane, turn)?;
        let gap = w - actor.s_m;
        if gap < -BOX_WAIT_MARGIN_M {
            return None; // past its wait point: it has gone
        }
        let plan = world.signal_plan(plan)?;
        let signal = self.signals.state_for(plan, actor.lane, ns_to_secs(t));
        Some((
            JunctionView {
                id: junction_id,
                position: junction.position,
                control: junction.control,
                stop_line_gap_m: gap.max(0.01),
                movement: turn,
                movement_lane: Some(actor.lane),
                signal,
                major_lanes: u8::try_from(junction.incoming.len()).unwrap_or(u8::MAX),
                waiting_s: actor
                    .stuck_since
                    .filter(|_| actor.speed_mps < STUCK_SPEED_MPS)
                    .map_or(0.0, |since| ns_to_secs(t.saturating_sub(since))),
                gap_z: actor.traits.gap_z,
            },
            gap,
        ))
    }

    /// Whether the amber `movement_lane` shows at `t` ends a permissive green
    /// ([`SignalState::GreenYield`]): the state of the phase before its run of amber
    /// phases.
    fn amber_follows_permissive(
        &self,
        world: &World,
        junction: JunctionId,
        movement_lane: Option<LaneId>,
        t: SimTime,
    ) -> bool {
        let Some(JunctionControl::Signalised { plan }) =
            world.roads.try_junction(junction).map(|j| j.control)
        else {
            return false;
        };
        let Some(plan) = world.signal_plan(plan) else {
            return false;
        };
        let Some(k) = plan
            .controlled
            .iter()
            .position(|l| Some(*l) == movement_lane)
        else {
            return false;
        };
        let Some((i, _)) = plan.phase_at(ns_to_secs(t)) else {
            return false;
        };
        let n = plan.phases.len();
        let state = |p: usize| plan.phases[p].states.get(k).copied();
        let mut j = i;
        for _ in 0..n {
            match state(j) {
                Some(SignalState::Amber) => j = (j + n - 1) % n,
                s => return s == Some(SignalState::GreenYield),
            }
        }
        false
    }

    fn amber_left_s(
        &self,
        world: &World,
        junction: JunctionId,
        movement_lane: Option<LaneId>,
        t: SimTime,
    ) -> Option<f64> {
        let JunctionControl::Signalised { plan } = world.roads.try_junction(junction)?.control
        else {
            return None;
        };
        let plan = world.signal_plan(plan)?;
        let k = plan
            .controlled
            .iter()
            .position(|l| Some(*l) == movement_lane)?;
        let (i, into) = plan.phase_at(ns_to_secs(t))?;
        let amber = |p: usize| plan.phases[p].states.get(k) == Some(&SignalState::Amber);
        if !amber(i) {
            return Some(0.0);
        }
        let mut left = plan.phases[i].duration_s - into;
        let mut j = i;
        for _ in 1..plan.phases.len() {
            j = (j + 1) % plan.phases.len();
            if !amber(j) {
                break;
            }
            left += plan.phases[j].duration_s;
        }
        Some(left.max(0.0))
    }

    /// How long the red shown to the movement on `movement_lane` at `junction` has been
    /// showing at `t`, seconds: `None` if it shows no red or nothing is signalled. Read
    /// from the plan, as [`Self::amber_left_s`] is.
    fn red_elapsed_s(
        &self,
        world: &World,
        junction: JunctionId,
        movement_lane: Option<LaneId>,
        t: SimTime,
    ) -> Option<f64> {
        let JunctionControl::Signalised { plan } = world.roads.try_junction(junction)?.control
        else {
            return None;
        };
        let plan = world.signal_plan(plan)?;
        let k = plan
            .controlled
            .iter()
            .position(|l| Some(*l) == movement_lane)?;
        let (i, into) = plan.phase_at(ns_to_secs(t))?;
        let red = |p: usize| plan.phases[p].states.get(k) == Some(&SignalState::Red);
        if !red(i) {
            return None;
        }
        let n = plan.phases.len();
        let mut elapsed = into;
        let mut j = i;
        for _ in 1..n {
            j = (j + n - 1) % n;
            if !red(j) {
                break;
            }
            elapsed += plan.phases[j].duration_s;
        }
        Some(elapsed.max(0.0))
    }

    /// The speed a junction connector is driven at: its turn speed, or its limit.
    fn connector_speed(&self, world: &World, connector: LaneId) -> f64 {
        let limit = world.lane(connector).speed_limit_mps;
        self.turn_speed
            .get(&connector)
            .map_or(limit, |v| v.min(limit))
    }

    /// The highest speed the vehicle should be doing now so that, braking at its
    /// comfortable deceleration, it reaches each of the next lanes of its route at no more
    /// than that lane's limit (or its turn speed): `sqrt(v_next² + 2·b·d)`.
    fn anticipated_speed(&self, world: &World, actor: &Actor) -> f64 {
        let b = actor.driver.comfort_decel_mps2.max(0.1);
        let mut dist = world.lane(actor.lane).length_m - actor.s_m;
        let mut best = f64::INFINITY;
        // The bends of this lane still ahead, or under the vehicle (`lane_bend_speed`).
        if let Some(zones) = self.bend_speed.get(&actor.lane) {
            for &(s0, s1, v) in zones {
                let d = s0 - actor.s_m;
                if s1 >= actor.s_m && d <= self.params.lookahead_m {
                    let cap = self.class_turn_speed(actor.class, v);
                    best = best.min(math::sqrt(cap * cap + 2.0 * b * d.max(0.0)));
                }
            }
        }
        for k in 1..=4 {
            if dist > self.params.lookahead_m {
                break;
            }
            let Some(next) = actor.route.lanes.get(actor.route_index + k).copied() else {
                break;
            };
            let Some(lane) = world.try_lane(next) else {
                break;
            };
            let mut cap = lane.speed_limit_mps * actor.traits.speed_factor;
            if let Some(v) = self.turn_speed.get(&next) {
                cap = cap.min(self.class_turn_speed(actor.class, *v));
            }
            best = best.min(math::sqrt(cap * cap + 2.0 * b * dist.max(0.0)));
            if let Some(zones) = self.bend_speed.get(&next) {
                for &(s0, _, v) in zones {
                    let d = dist + s0;
                    if d <= self.params.lookahead_m {
                        let cap = self.class_turn_speed(actor.class, v);
                        best = best.min(math::sqrt(cap * cap + 2.0 * b * d.max(0.0)));
                    }
                }
            }
            dist += lane.length_m;
        }
        best
    }

    /// The braking a bend of a through lane ahead (or under the vehicle) asks for
    /// ([`EngineParams::lane_bend_speed`]): `−(v² − v_b²) / 2d` to reach the bend's speed
    /// `v_b` at its start `d` ahead, or `(v_b − v) / dt` inside it, binding only from
    /// [`STOP_FLOOR_BIND_MPS2`] and never harder than a planned stop
    /// ([`PLANNED_STOP_MAX_DECEL_MPS2`]). `None` when no bend binds.
    fn bend_floor(&self, world: &World, actor: &Actor, dt_s: f64) -> Option<f64> {
        if self.bend_speed.is_empty() || dt_s <= 0.0 {
            return None;
        }
        let v = actor.speed_mps;
        let mut best: Option<f64> = None;
        let mut consider = |d: f64, v_b: f64| {
            if v <= v_b {
                return;
            }
            let need = if d <= 0.0 {
                (v_b - v) / dt_s
            } else {
                -(v * v - v_b * v_b) / (2.0 * d)
            };
            if -need >= STOP_FLOOR_BIND_MPS2 {
                let need = need.max(-PLANNED_STOP_MAX_DECEL_MPS2);
                best = Some(best.map_or(need, |b: f64| b.min(need)));
            }
        };
        if let Some(zones) = self.bend_speed.get(&actor.lane) {
            for &(s0, s1, v_b) in zones {
                let d = s0 - actor.s_m;
                if s1 >= actor.s_m && d <= self.params.lookahead_m {
                    consider(d, self.class_turn_speed(actor.class, v_b));
                }
            }
        }
        let mut dist = world.lane(actor.lane).length_m - actor.s_m;
        for k in 1..=4 {
            if dist > self.params.lookahead_m {
                break;
            }
            let Some(next) = actor.route.lanes.get(actor.route_index + k).copied() else {
                break;
            };
            let Some(lane) = world.try_lane(next) else {
                break;
            };
            if let Some(zones) = self.bend_speed.get(&next) {
                for &(s0, _, v_b) in zones {
                    if dist + s0 <= self.params.lookahead_m {
                        consider(dist + s0, self.class_turn_speed(actor.class, v_b));
                    }
                }
            }
            dist += lane.length_m;
        }
        best
    }

    /// The closer of two constraints, as a virtual leader.
    fn closest(a: Option<LeaderView>, b: Option<LeaderView>) -> Option<LeaderView> {
        match (a, b) {
            (None, x) | (x, None) => x,
            (Some(x), Some(y)) => Some(if x.gap_m <= y.gap_m { x } else { y }),
        }
    }
}

/// What happened to a trip offered for insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Insertion {
    /// On the road, as this actor.
    Placed(ActorId),
    /// Its origin has no room for it now.
    Occupied,
    /// No route, or an origin it cannot stand on at all.
    Unroutable,
}

/// How long a trip may wait for room at its origin before it is dropped: 120 s.
///
/// **This crate's choice.** SUMO's `--max-depart-delay` defaults to unlimited; a bound
/// keeps a gridlocked origin from accumulating demand without end, and two minutes is
/// longer than any signal cycle a queue could be waiting on.
const MAX_DEPART_DELAY: Duration = Duration::from_secs(120);

/// One actor's buffered decision, the output of pass 6.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Decision {
    actor: ActorId,
    accel_mps2: f64,
    v0_mps: f64,
    lane_change: LaneChangeDecision,
    /// The junction this vehicle is committed to clearing because it chose to go on
    /// amber, carried to the next step.
    amber_commit: Option<JunctionId>,
    /// The junction whose amber this vehicle has decided on, carried to the next step.
    amber_seen: Option<JunctionId>,
    /// The junction whose amber or red the driver has judged for a violation.
    signal_look: Option<JunctionId>,
    /// The violations decided this step ([`crate::audit::intent`] bits).
    violation: u8,
    /// When the standing vehicle's reaction to being let go began, carried to the next
    /// step.
    release_at: Option<SimTime>,
    /// How far the front may advance this step without closing inside the standstill gap
    /// of a vehicle standing ahead, metres; `None` when the car ahead is moving or there is
    /// none.
    advance_cap_m: Option<f64>,
    /// A lane change round a vehicle at the kerb: when the lane beside cannot make the
    /// route's next movement, the trip is re-planned from it rather than the change
    /// refused.
    detour_ok: bool,
    /// Where across its lane the sub-lane model steers the vehicle this step.
    lateral: Option<crate::lateral::LateralDecision>,
    /// The driver's next perception errors and this step's car-following decision, when
    /// the drivers are imperfect ([`crate::drivers`]).
    human: Option<(f64, f64, f64)>,
}

impl v2xw_core::model::Model for NativeMobility {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl Mobility for NativeMobility {
    fn tier(&self) -> Tier {
        match self.params.intersections {
            IntersectionMode::TwoColoringLegacy | IntersectionMode::None => Tier::Abstract,
            _ => Tier::Medium,
        }
    }

    fn init(&mut self, ctx: &mut dyn MobCtx, demand: Box<dyn Demand>) -> Result<()> {
        let world = ctx.world();
        if world
            .roads
            .lanes()
            .iter()
            .all(|l| !l.kind.is_motorised() || !l.admits(self.params.classes))
        {
            return Err(MobError::EmptyWorld {
                what: "drivable lane the configured classes may use",
            });
        }
        if self.params.intersections == IntersectionMode::TwoColoringLegacy {
            self.two_coloring = Some(TwoColoring::new(world, TwoColoringParams::default()));
        } else {
            if self.params.junction_clearance {
                self.zones = ConflictZones::build(world);
            }
            if self.params.turn_lateral_accel_mps2 > 0.0 {
                self.turn_speed = turn_speeds(
                    world,
                    self.params.turn_lateral_accel_mps2,
                    self.params.turn_curve_window_m,
                );
                if self.params.lane_bend_speed {
                    let window = if self.params.turn_curve_window_m > 0.0 {
                        self.params.turn_curve_window_m
                    } else {
                        DEFAULT_TURN_CURVE_WINDOW_M
                    };
                    self.bend_speed =
                        bend_speeds(world, self.params.turn_lateral_accel_mps2, window);
                }
            }
        }
        // Where a pedestrian may be placed: a pavement, never a crosswalk — putting someone
        // down in the carriageway in front of a car is not a spawn, it is a collision.
        self.walkable = world
            .roads
            .lanes()
            .iter()
            .filter(|l| SocialForce::is_walkable(world, l.id) && l.kind == LaneKind::Sidewalk)
            .map(|l| l.id)
            .collect();
        self.crosswalks = CrosswalkIndex::build(world);
        if let Some(vru) = self.vru.as_mut()
            && vru.params().midblock.rate_per_100m > 0.0
        {
            vru.set_midblock_index(Arc::new(crate::vru::MidblockIndex::build(
                world,
                &vru.params().midblock,
            )));
        }
        self.diverging = Self::diverging_connectors(world);
        self.bike_lanes = world
            .roads
            .lanes()
            .iter()
            .filter(|l| {
                l.kind != LaneKind::Internal
                    && l.kind != LaneKind::Sidewalk
                    && l.admits(ClassMask::BICYCLE)
                    && l.length_m >= VehicleClass::Bicycle.spec().length_m + 1.0
            })
            .map(|l| l.id)
            .collect();
        self.demand = Some(demand);
        Ok(())
    }

    fn step(&mut self, ctx: &mut dyn MobCtx, dt: Duration) -> MobilityUpdate {
        let t0 = ctx.now();
        let t1 = dt.after(t0);
        let dt_s = dt.as_secs_f64();
        // The car-following model's step clock (`CarFollowing::begin_step`): before any
        // `accel` of this step, so every evaluation in the step sees the same instant.
        self.cf.begin_step(t0, dt);
        for m in self.vtype_models.values() {
            m.begin_step(t0, dt);
        }

        // --- pass 1: commands ---------------------------------------------
        let commanded: Vec<ActorId> = self.apply_commands(&*ctx, t0);

        // --- pass 2: spawn -------------------------------------------------
        let mut spawned: Vec<ActorSpawn> = Vec::new();
        {
            let world = ctx.world();
            for id in commanded {
                spawned.push(self.actor_spawn(world, id, t0));
            }
        }
        if let Some(mut demand) = self.demand.take() {
            let trips = demand.spawns_in(ctx, t0, t1);
            self.demand = Some(demand);
            if self.along_path() {
                // Trips whose origin has no room *now* wait for it, oldest first, as
                // SUMO's insertion queue does (`--max-depart-delay`), rather than being
                // lost: the safe-insertion gap the follower needs refuses more trips than
                // the old 2 m test did, and a refusal must delay demand, not delete it.
                // One that has waited `MAX_DEPART_DELAY` is dropped and counted.
                //
                // A trip that has to wait reserves its actor id at once, so ids stay in
                // demand-stream order (invariant I-M2) however long it waits.
                let mut queue = core::mem::take(&mut self.waiting);
                queue.extend(trips.into_iter().map(|trip| (trip, t0, None)));
                // A held fleet ([`Demand::fleet_target`]): top it up to its size, counting
                // the trips already waiting to enter. A finished trip is replaced in the
                // step after it ends; a refused one is replaced in the same step. The
                // attempts are bounded so a network with nowhere to put a vehicle cannot
                // spin: at most twice the shortfall per step, and 64 at most.
                let fleet = self.demand.as_ref().and_then(|d| d.fleet_target());
                if let Some(target) = fleet {
                    let have = (self.actors.len() + queue.len()) as u64;
                    let short = target.saturating_sub(have);
                    for _ in 0..short.min(64) {
                        if let Some(trip) = self.demand.as_mut().and_then(|d| d.top_up(ctx, t0)) {
                            queue.push((trip, t0, None));
                        }
                    }
                }
                let world = ctx.world();
                let mut unroutable_now = 0u64;
                for (trip, since, reserved) in queue {
                    let traits = self.traits_for(&*ctx, &trip);
                    match self.try_insert(world, &trip, t0, reserved, traits) {
                        Insertion::Placed(id) => {
                            self.assign_kerbside(&*ctx, id);
                            spawned.push(self.actor_spawn(world, id, t0).with_requested(trip.t));
                        }
                        Insertion::Occupied
                            if t0.saturating_sub(since) < MAX_DEPART_DELAY.as_nanos() =>
                        {
                            let id = reserved.unwrap_or_else(|| {
                                let id = ActorId::new(self.next_actor);
                                self.next_actor += 1;
                                id
                            });
                            self.waiting.push((trip, since, Some(id)));
                        }
                        Insertion::Occupied => {
                            self.dropped_trips += 1;
                            if let Some(d) = self.demand.as_mut() {
                                d.refund();
                            }
                        }
                        Insertion::Unroutable => {
                            unroutable_now += 1;
                            if let Some(d) = self.demand.as_mut() {
                                d.refund();
                            }
                        }
                    }
                }
                // A trip with no route is redrawn at once with a new origin and destination,
                // up to `REDRAW_TRIES` times, rather than lost. SUMO's randomTrips
                // `--validate` (tools/randomTrips.py) discards unroutable trips, which thins
                // the stated flow; on a downtown box whose one-way streets only leave it, or
                // a highway with destinations behind the origin, that lost 31 % to 80 % of
                // the stated arrivals (highway-congestion: 909 of 1,131 in 90 s, measured
                // 2026-10-08). A held fleet tops up the same way. Bounded per step.
                const REDRAW_TRIES: u64 = 4;
                let mut tries = (unroutable_now * REDRAW_TRIES).min(256);
                let mut pending = unroutable_now;
                let mut failed_in_row = 0u64;
                while pending > 0 && tries > 0 {
                    tries -= 1;
                    let Some(trip) = self.demand.as_mut().and_then(|d| d.redraw(ctx, t0)) else {
                        break;
                    };
                    let world = ctx.world();
                    let traits = self.traits_for(&*ctx, &trip);
                    match self.try_insert(world, &trip, t0, None, traits) {
                        Insertion::Placed(id) => {
                            self.assign_kerbside(&*ctx, id);
                            spawned.push(self.actor_spawn(world, id, t0).with_requested(trip.t));
                            pending -= 1;
                            failed_in_row = 0;
                        }
                        Insertion::Occupied => {
                            let id = ActorId::new(self.next_actor);
                            self.next_actor += 1;
                            self.waiting.push((trip, t0, Some(id)));
                            pending -= 1;
                            failed_in_row = 0;
                        }
                        Insertion::Unroutable => {
                            if let Some(d) = self.demand.as_mut() {
                                d.refund();
                            }
                            failed_in_row += 1;
                            // This arrival has had its tries; it is lost and counted.
                            if failed_in_row >= REDRAW_TRIES {
                                self.dropped_trips += 1;
                                self.unroutable_trips += 1;
                                pending -= 1;
                                failed_in_row = 0;
                            }
                        }
                    }
                }
                // What the bound or the model left unredrawn is lost and counted.
                self.dropped_trips += pending;
                self.unroutable_trips += pending;
            } else {
                let world = ctx.world();
                for trip in trips {
                    let traits = self.traits_for(&*ctx, &trip);
                    if let Some(id) = self.insert_trip_with(world, &trip, t0, traits) {
                        spawned.push(self.actor_spawn(world, id, t0).with_requested(trip.t));
                    }
                }
            }
        }

        // --- pass 2b: the vulnerable road users ------------------------------
        let mut vru_gone: Vec<(ActorId, DespawnCause)> = Vec::new();
        self.keep_vru_population(ctx, t0, &mut spawned, &mut vru_gone);

        // --- pass 3: freeze ------------------------------------------------
        let world = ctx.world();
        let snapshot = self.snapshot(world, t0);
        // --- pass 4: signals -----------------------------------------------
        let signal_states = self.signal_states(world, t0);
        // --- pass 5: claims ------------------------------------------------
        let claims = self.claims(world, &snapshot, t0);
        let occupancy = self.occupancy(world);
        // The crosswalks someone is walking on, from the start-of-step positions.
        // Not counted: someone at the far end of one crossing about to go onto the next (a
        // refuge island, where `SocialForce` holds them while a vehicle is too close to
        // the second crossing). They are out of this crossing's traffic, as
        // in SUMO, where a standing pedestrian outside a vehicle's path does not block it
        // (`MSPModel_Interacting::blockedAtDist`).
        let crosswalks_in_use = self.crosswalks.occupied(self.vru.iter().flat_map(|v| {
            v.people()
                .filter(|p| !p.arrived && !v.at_refuge(p, world))
                .map(|p| p.lane)
        }));
        // The mid-block paths in use, from the start-of-step positions: each band, the
        // pedestrian it belongs to, and whether that pedestrian is in (or about to step
        // into) the lane it cuts.
        let midblock = MidblockBands::of(self.vru.as_ref());

        // --- pass 6: decide -------------------------------------------------
        // Every input is the frozen snapshot; every output is buffered. The iteration order
        // is therefore irrelevant, which `reverse_order` exists to prove.
        let mut ids: Vec<ActorId> = self.actors.keys().copied().collect();
        if self.params.reverse_order {
            ids.reverse();
        }
        let mut decisions: Vec<Decision> = Vec::with_capacity(ids.len());
        // The view is stored beside the neighbours so the lane-change walk scores the SAME
        // ego the car-following call did — the speed-capped one. Rebuilding it there from
        // `actor.view(world)` handed MOBIL a driver without the externally commanded cap,
        // so a capped vehicle could decide to change lane for a speed gain it would never
        // be allowed to take.
        let mut neighbours: BTreeMap<ActorId, (VehicleView, LaneNeighbors)> = BTreeMap::new();
        // One actor's decision: a pure function of the frozen state, so the actors are
        // decided in parallel and merged in id order (`crate::par`, ADR 0004 decision 5).
        let decide_one = |id: &ActorId| -> Option<(Decision, VehicleView, LaneNeighbors)> {
            let Some(actor) = self.actors.get(id) else {
                return None;
            };
            let ego = actor.view(world);
            let lane = world.lane(actor.lane);
            let along = self.along_path();
            let mut lane_view = LaneView::of(lane);
            // The driver's own free speed on this lane: the posted limit times their
            // speed factor ([`DriverTraits`]).
            lane_view.speed_limit_mps *= actor.traits.speed_factor;
            // A junction connector is driven no faster than its curvature allows.
            if along && let Some(v) = self.turn_speed.get(&actor.lane) {
                lane_view.speed_limit_mps = lane_view
                    .speed_limit_mps
                    .min(self.class_turn_speed(actor.class, *v));
            }
            let options = NeighborOptions {
                lookahead_m: self.params.lookahead_m,
                max_lane_hops: 8,
                // The ego's own classes: a car does not change onto a bus lane, and a
                // rider follows the bicycle lane graph.
                classes: actor.class.class_mask(),
                sides: self.params.lane_changes,
                sublane: self.lateral.as_ref().map(|l| l.filter()),
            };
            // **One** neighbour query, shared by the car-following and lane-change models.
            let nbrs =
                snapshot.neighbors(world, &ego, &actor.route.lanes, actor.route_index, options);
            let mut leader = nbrs.leader;
            // The nearest real vehicle among the constraints, kept apart from the binding
            // one: a virtual obstacle's relaxed stop (below) must never let the vehicle
            // brake less than the car-following model asks for behind a real car.
            let mut nearest_vehicle = nbrs.leader.filter(LeaderView::is_vehicle);
            let mut merge_partner: Option<MergePartner> = None;
            // The junction ahead becomes a virtual leader when it says stop or slow.
            let mut amber_commit: Option<JunctionId> = None;
            let mut amber_seen: Option<JunctionId> = None;
            let mut signal_look: Option<JunctionId> = None;
            let mut violation: u8 = 0;
            let viol = self.params.violations;
            let violating = along && viol.enabled;
            let mut commit_floor: Option<f64> = None;
            // The stop line behind the binding virtual obstacle, when a junction's "stop"
            // is what binds: how much room there is up to the line itself.
            let mut binding_stop_line: Option<f64> = None;
            // A signal's "stop" hidden behind a car ahead that is going on (see below).
            let mut signal_stop: Option<(LeaderView, f64)> = None;
            for junction in self.junctions_ahead(world, actor, t0) {
                let conflicts = self.conflicts_for(world, &junction, &ego, &claims, t0);
                let mut decision = self.entry_decision(world, &ego, &junction, &conflicts);
                // Failure to yield ([`crate::drivers::ViolationParams::fail_to_yield_share`]):
                // where this driver must give way — no signal, or a permissive green — a
                // keyed draw per driver and junction decides whether they ignore the
                // conflicting foes slow enough to look as if they are stopping.
                if violating
                    && viol.fail_to_yield_share > 0.0
                    && junction.stop_line_gap_m > 0.0
                    && matches!(decision, EntryDecision::Stop { .. })
                    && matches!(junction.signal, None | Some(SignalState::GreenYield))
                {
                    // A hash, not a stream: the decision pass runs in parallel on the
                    // frozen snapshot and draws nothing from the context.
                    let u = fail_to_yield_uniform(actor.seq, u64::from(junction.id.index()));
                    if u < viol.fail_to_yield_share {
                        let kept: Vec<ConflictView> = conflicts
                            .iter()
                            .filter(|c| !(c.conflicts && c.speed_mps <= viol.ignore_foe_speed_mps))
                            .cloned()
                            .collect();
                        if kept.len() < conflicts.len()
                            && self.entry_decision(world, &ego, &junction, &kept)
                                == EntryDecision::Proceed
                        {
                            decision = EntryDecision::Proceed;
                            violation |= crate::audit::intent::FOE_IGNORED;
                        }
                    }
                }
                // Red-light running after the red onset: a driver who sees the red before
                // they have judged this junction's amber goes if the red began no longer
                // ago than their tolerance less the time they need to the line.
                if violating
                    && junction.signal == Some(SignalState::Red)
                    && junction.stop_line_gap_m > 0.0
                    && actor.signal_look != Some(junction.id)
                    && actor.amber_seen != Some(junction.id)
                    && signal_look.is_none()
                {
                    if actor.traits.red_tolerance_s > 0.0
                        && matches!(decision, EntryDecision::Stop { .. })
                        && ego.speed_mps > RED_RUN_MIN_SPEED_MPS
                        && let Some(into) =
                            self.red_elapsed_s(world, junction.id, junction.movement_lane, t0)
                        && into + junction.stop_line_gap_m / ego.speed_mps
                            <= actor.traits.red_tolerance_s
                    {
                        decision = EntryDecision::Proceed;
                        amber_commit = Some(junction.id);
                        commit_floor = Some(f64::INFINITY);
                        violation |= crate::audit::intent::RED_RUN;
                    }
                    signal_look = Some(junction.id);
                }
                if violating
                    && matches!(junction.signal, Some(SignalState::Amber | SignalState::Red))
                    && signal_look.is_none()
                    && (actor.signal_look == Some(junction.id)
                        || actor.amber_seen == Some(junction.id)
                        || actor.amber_commit == Some(junction.id))
                {
                    signal_look = Some(junction.id);
                }
                // The amber decision, taken once, at the onset: a driver who could stop still
                // goes if they are nearer the line than their own threshold
                // ([`DriverTraits::amber_go_tti_s`]) and reach it before the red. Taken
                // later, a driver braking for the line would see their time to it shrink
                // and change their mind.
                if along
                    && junction.signal == Some(SignalState::Amber)
                    && junction.stop_line_gap_m > 0.0
                    && amber_seen.is_none()
                {
                    let first_look = actor.amber_seen != Some(junction.id)
                        && actor.amber_commit != Some(junction.id);
                    if first_look
                        && matches!(decision, EntryDecision::Stop { .. })
                        && ego.speed_mps > 1.0
                    {
                        let tti = junction.stop_line_gap_m / ego.speed_mps;
                        let left = self
                            .amber_left_s(world, junction.id, junction.movement_lane, t0)
                            .unwrap_or(0.0);
                        if tti < actor.traits.amber_go_tti_s
                            && (actor.traits.amber_runner || tti + AMBER_GO_MARGIN_S <= left)
                        {
                            decision = EntryDecision::Proceed;
                            if violating {
                                if tti >= AMBER_DILEMMA_TTI_S.1 {
                                    violation |= crate::audit::intent::AMBER_RUN;
                                }
                                if tti + AMBER_GO_MARGIN_S > left {
                                    violation |= crate::audit::intent::RED_RUN;
                                }
                            }
                        } else if violating
                            && actor.traits.red_tolerance_s > 0.0
                            && tti - left <= actor.traits.red_tolerance_s
                        {
                            // A red-light runner (`ViolationParams`: "at the amber onset
                            // they go if, at their present speed, they reach the line no
                            // later than their tolerance after the red"), whatever their
                            // amber threshold. Gated on `tti < amber_go_tti_s` as it was,
                            // the arm only ever took drivers who reach the line before the
                            // red: one red entry in an hour of a 600 veh/h approach, against
                            // the field's 3-6 per 1,000 (TTI 0-4196-P1, Table 1). Tagged a red run
                            // when they would not clear with the stop-or-go margin, as the
                            // arm above tags its own late goers.
                            decision = EntryDecision::Proceed;
                            if tti + AMBER_GO_MARGIN_S > left {
                                violation |= crate::audit::intent::RED_RUN;
                            }
                            if tti >= AMBER_DILEMMA_TTI_S.1 {
                                violation |= crate::audit::intent::AMBER_RUN;
                            }
                        }
                        if violating {
                            signal_look = Some(junction.id);
                        }
                    }
                    amber_seen = Some(junction.id);
                }
                // A permitted left turner held through its permissive green may complete
                // the turn in the change interval, as the HCM's sneakers do, when it can
                // cross the line before the red and no opposing car still able to reach
                // its own line is inside its gap
                // ([`FixedTimeSignals::sneaker_may_go`]). Off unless
                // `permissive_sneakers` is set.
                if along
                    && self.params.permissive_sneakers > 0
                    && junction.signal == Some(SignalState::Amber)
                    && matches!(decision, EntryDecision::Stop { .. })
                    && self.amber_follows_permissive(world, junction.id, junction.movement_lane, t0)
                    && let Some(left) =
                        self.amber_left_s(world, junction.id, junction.movement_lane, t0)
                    && self
                        .signals
                        .sneaker_may_go(&ego, &junction, &conflicts, left)
                {
                    decision = EntryDecision::Proceed;
                }
                // A permitted left turner refused a gap on its permissive green pulls into
                // the junction and waits there, short of the opposing stream's paths, so
                // that it is within the junction when the green ends and clears it then:
                // the HCM's sneakers, and UVC §11-402's "within the intersection". Only one
                // waits on a connector at a time, never with the exit full or a conflicting
                // vehicle still inside ([`Self::junction_blocked`]). Off unless
                // `permissive_sneakers` is set.
                if along
                    && self.params.permissive_sneakers > 0
                    && junction.signal == Some(SignalState::GreenYield)
                    && matches!(decision, EntryDecision::Stop { .. })
                    && junction.stop_line_gap_m > 0.0
                    && let Some(movement) = junction
                        .movement_lane
                        .filter(|l| world.lane(*l).kind == LaneKind::Internal)
                    && let Some(w) = self.box_wait_s(world, movement, junction.movement)
                    && occupancy
                        .get(&movement)
                        .is_none_or(|l| l.iter().all(|(o, _, _)| *o == actor.id))
                    && !self.junction_blocked(world, &snapshot, &occupancy, actor, movement)
                {
                    decision = EntryDecision::Stop {
                        gap_m: junction.stop_line_gap_m + w,
                    };
                }
                // A vehicle that chose to go on amber — it was inside the distance it could
                // stop in — is committed: it does not change its mind a step later because
                // it has slowed for the turn ahead, which is what left drivers braking into
                // the junction on red. It keeps its speed and clears.
                if along && matches!(junction.signal, Some(SignalState::Amber | SignalState::Red)) {
                    // A committed driver who has since been brought (almost) to a stop short
                    // of the line — by someone stepping onto a crosswalk or into the road in
                    // front — is no longer clearing the junction: the commitment lapses and
                    // the signal holds them like anyone else. Without this a car stopped for
                    // a pedestrian on amber drove on into the red once they had passed.
                    let lapsed = self.params.amber_commit_lapse
                        && ego.speed_mps < AMBER_COMMIT_LAPSE_MPS
                        && junction.stop_line_gap_m > STOP_LINE_MARGIN_M;
                    let committed = if actor.amber_commit == Some(junction.id) && !lapsed {
                        decision = EntryDecision::Proceed;
                        true
                    } else {
                        junction.signal == Some(SignalState::Amber)
                            && decision == EntryDecision::Proceed
                            && junction.stop_line_gap_m > 0.0
                            && amber_commit.is_none()
                    };
                    if committed {
                        amber_commit = Some(junction.id);
                        // The slowest average speed that still reaches the line before
                        // the amber ends: a committed driver still slows for the turn
                        // beyond the line, but never below this.
                        let left =
                            self.amber_left_s(world, junction.id, junction.movement_lane, t0);
                        commit_floor = Some(match left {
                            Some(r) if r > 0.05 => junction.stop_line_gap_m.max(0.0) / r,
                            _ => f64::INFINITY,
                        });
                    }
                }
                // On a green, the last car in on the cross street's amber goes first.
                if along
                    && decision == EntryDecision::Proceed
                    && junction.stop_line_gap_m > 0.0
                    && matches!(
                        junction.signal,
                        Some(SignalState::Green | SignalState::GreenYield)
                    )
                    && self.clearing_foe(world, junction.id, &conflicts, t0)
                {
                    decision = EntryDecision::Stop {
                        gap_m: (junction.stop_line_gap_m - STOP_LINE_OFFSET_M).max(0.0),
                    };
                }
                if along
                    && self.params.junction_clearance
                    && decision == EntryDecision::Proceed
                    && junction.stop_line_gap_m > 0.0
                    && let Some(movement) = junction
                        .movement_lane
                        .filter(|l| world.lane(*l).kind == LaneKind::Internal)
                    && self.junction_blocked(world, &snapshot, &occupancy, actor, movement)
                {
                    decision = EntryDecision::Stop {
                        gap_m: (junction.stop_line_gap_m - STOP_LINE_OFFSET_M).max(0.0),
                    };
                    // A driver who committed to an amber and is now held at the line
                    // because the junction's exit is full is no longer clearing it: the
                    // commitment ends, and the signal holds them once the exit frees. Kept,
                    // it sent buses and trucks — whose length needs the most room on the
                    // exit — creeping into the junction on the red that had come while
                    // they waited (75 red entries in five minutes of a mixed fleet on the
                    // dense grid, `tests/realism.rs`).
                    if along && amber_commit == Some(junction.id) {
                        amber_commit = None;
                        commit_floor = None;
                    }
                }
                // Committed: a give-way decision that turns to "stop" when the vehicle is
                // already too close to stop without an emergency brake is not taken. A
                // left turner on a permissive green was 2 m from the line at 6.8 m/s when
                // an opposing car came inside the critical gap; it braked at −6 m/s²,
                // could not stop, and rolled in anyway. It now clears the junction, and
                // the traffic it crosses holds for a vehicle already within it (the entry
                // rule of `junction_blocked`) and, inside, `junction_leader` settles the
                // crossing. Signals keep their own rules: red has no commitment, and amber
                // has its own (above).
                //
                // The room it has is to where a stop now ends, `STOP_LINE_MARGIN_M` short
                // of the line — not the old 4 m short of it (the decision's 2 m offset and
                // the standstill gap): judged by that, a turner 6.3 m out at 4.7 m/s
                // "could not stop" at 1.9 m/s², took a 1.2 s lag in front of a car at
                // 11 m/s, and the two collided inside the junction.
                if along
                    && matches!(decision, EntryDecision::Stop { .. })
                    && !matches!(
                        junction.signal,
                        Some(SignalState::Red | SignalState::RedAmber | SignalState::Amber)
                    )
                    && ego.speed_mps > 0.0
                    && ego.speed_mps * ego.speed_mps
                        / (2.0 * (junction.stop_line_gap_m - STOP_LINE_MARGIN_M).max(0.1))
                        > PLANNED_STOP_MAX_DECEL_MPS2
                {
                    decision = EntryDecision::Proceed;
                }
                let virtual_leader = decision.as_leader().map(|mut l| {
                    // The stop line is `STOP_LINE_OFFSET_M` before the junction, and the
                    // decision already accounts for it; nothing more to do but keep the
                    // sign honest.
                    l.gap_m = l.gap_m.max(0.0);
                    l
                });
                let before = leader;
                leader = Self::closest(leader, virtual_leader);
                if leader != before {
                    binding_stop_line = Some(junction.stop_line_gap_m);
                } else if let Some(v) = virtual_leader
                    && matches!(
                        junction.signal,
                        Some(SignalState::Red | SignalState::RedAmber | SignalState::Amber)
                    )
                    && signal_stop.is_none()
                {
                    signal_stop = Some((v, junction.stop_line_gap_m));
                }
            }
            // A permitted left turner waiting inside the junction (box entry, above): it
            // goes on when the gap test passes on the permissive green, and clears in the
            // change interval once the opposing stream has stopped
            // ([`FixedTimeSignals::box_may_clear`]); until then it holds at its wait point.
            if along
                && self.params.permissive_sneakers > 0
                && let Some((jv, w_gap)) = self.box_wait_view(world, actor, t0)
            {
                let conflicts = self.conflicts_for(world, &jv, &ego, &claims, t0);
                let go = match jv.signal {
                    Some(SignalState::GreenYield) => {
                        self.signals.may_enter(&ego, &jv, &conflicts, &self.weather)
                            == EntryDecision::Proceed
                    }
                    Some(SignalState::Amber | SignalState::Red | SignalState::RedAmber) => {
                        let left = self
                            .amber_left_s(world, jv.id, jv.movement_lane, t0)
                            .unwrap_or(0.0);
                        self.signals.box_may_clear(&ego, &jv, &conflicts, left)
                    }
                    _ => true,
                };
                if !go {
                    leader = Self::closest(
                        leader,
                        EntryDecision::Stop {
                            gap_m: w_gap.max(0.0),
                        }
                        .as_leader(),
                    );
                }
            }
            // While a lane change is under way the body is still partly in the lane being
            // left, so the vehicle ahead *there* is followed too. Following only the
            // target lane's leader let a slow change — 8 s at 3 m/s — drive its body into
            // the car standing ahead in the lane it was leaving.
            if along
                && let Some(tr) = actor.transition
                && tr.switched
                && actor.lateral_m.abs() + 0.5 * actor.class.spec().width_m
                    > 0.5 * world.lane(tr.to).width_m
            {
                let in_from = VehicleView {
                    lane: tr.from,
                    s_m: actor.s_m + tr.from_s_delta,
                    ..ego
                };
                let before = leader;
                let in_from_leader =
                    snapshot.leader_on_route(world, &in_from, &[tr.from], 0, options);
                nearest_vehicle = Self::closest(nearest_vehicle, in_from_leader);
                leader = Self::closest(leader, in_from_leader);
                if leader != before {
                    binding_stop_line = None;
                }
            }
            // Other vehicles inside the junction: follow one merging ahead onto the same
            // exit, and give way at a crossing to one that reaches it first.
            if along && self.params.junction_clearance {
                let before = leader;
                let inside = self.junction_leader(world, &snapshot, &occupancy, &claims, actor, t0);
                leader = Self::closest(leader, inside.as_ref().and_then(|c| c.crossing));
                if leader != before {
                    binding_stop_line = None;
                }
                merge_partner = inside.and_then(|c| c.merge);
            }
            // Crosswalks: yield to anyone on one, and do not stop inside one
            // (`crate::vru::crosswalk`, UVC §11-502(a), §11-202(a)1, §11-1003).
            if along && self.params.crosswalk_yield && !self.crosswalks.is_empty() {
                let path = actor.crosswalk_path();
                let bands = self.crosswalks.ahead(world, &path);
                if let Some(gap) = self.crosswalks.stop_gap(
                    &bands,
                    &crosswalks_in_use,
                    &path,
                    leader.map(|l| (l.gap_m, l.speed_mps)),
                    actor.driver.min_gap_m,
                ) {
                    let before = leader;
                    leader = Self::closest(leader, Some(LeaderView::virtual_obstacle(gap, 0.0)));
                    if leader != before {
                        binding_stop_line = None;
                    }
                }
            }
            // A pedestrian crossing mid-block: every driver stops for one in, or about to
            // enter, their lane (UVC §11-504, due care), and one who chose to yield stops
            // for one waiting at the kerb (`crate::vru::midblock`).
            if along && self.params.crosswalk_yield && !midblock.is_empty() {
                let path = actor.crosswalk_path();
                let bands = crate::vru::crosswalk::bands_ahead(world, &path, &midblock.by_lane);
                if let Some(gap) = midblock.stop_gap(&bands, actor.id, &self.midblock_yields) {
                    let before = leader;
                    leader = Self::closest(leader, Some(LeaderView::virtual_obstacle(gap, 0.0)));
                    if leader != before {
                        binding_stop_line = None;
                    }
                }
            }
            // A closed lane ahead on the route is a barrier, stopped short of like a stop
            // line. The end-of-step detour ([`Self::closure_detours`]) has already looked
            // for a way round it, so a vehicle still routed into one has none: it waits
            // for the lane to reopen instead of driving up to the barrier and vanishing.
            if along
                && (!self.closed.is_empty() || !self.holds.is_empty())
                && let Some(gap) = self.closure_gap(world, actor)
            {
                let before = leader;
                leader = Self::closest(leader, Some(LeaderView::virtual_obstacle(gap, 0.0)));
                if leader != before {
                    binding_stop_line = None;
                }
            }
            // A strategic change not yet made: the lane's end is a barrier, as in SUMO,
            // where a vehicle that cannot make its change waits at the end of its lane.
            // A route the vehicle's own lane can make replaces the change in the
            // no-change zone ([`Self::give_up_strategic`]), so a vehicle still owing it
            // there has no other way on: it drove past the end of the lane and was removed with
            // no route (SUMO cross-validation, Manhattan extract at 2400 veh/h: four trips
            // traced, at links of about 62 m; trips completed 220 -> 223, SUMO 232).
            // The barrier stands from the start of the lane, so the driver approaches it
            // as it would a red light; raised only on entering the zone it was 15-30 m
            // away at speed and the driver braked at the 6 m/s² bound, closing up the car
            // behind (connected-intersections, 10 m/s at 25 m from the lane end).
            if along && let Some(left) = self.strategic_pending(world, actor) {
                let d = (left - STOP_LINE_MARGIN_M).max(0.0);
                let before = leader;
                leader = Self::closest(
                    leader,
                    Some(LeaderView::virtual_obstacle(
                        d + actor.driver.min_gap_m,
                        0.0,
                    )),
                );
                if leader != before {
                    binding_stop_line = Some(d + STOP_LINE_MARGIN_M);
                }
            }
            // An external stop command is a virtual leader at zero gap.
            if actor.stopped_until.is_some_and(|until| t0 < until) {
                leader = Self::closest(leader, Some(LeaderView::virtual_obstacle(0.0, 0.0)));
                binding_stop_line = None;
            }
            // A kerbside stop ([`crate::kerbside`]) is braked into like a stop line: an
            // obstacle the standstill gap beyond the point the front stops at, held until
            // the dwell is over.
            if along
                && actor.kerb.holds(t0)
                && let Some(d) = actor.kerb.stop_distance(
                    world,
                    actor.lane,
                    actor.s_m,
                    &actor.route.lanes,
                    actor.route_index,
                )
            {
                let gap = (d + actor.driver.min_gap_m).max(0.0);
                let before = leader;
                leader = Self::closest(leader, Some(LeaderView::virtual_obstacle(gap, 0.0)));
                if leader != before {
                    // Stopped at like a stop line, so the relaxed stop below may use no
                    // room beyond the stop point: as a plain obstacle it let the vehicle
                    // creep on into its own standstill gap, past the stop, at half a metre
                    // a second.
                    binding_stop_line = Some(d + STOP_LINE_MARGIN_M);
                }
            }
            let v0 = match actor.speed_cap_mps {
                Some(cap) => actor.driver.desired_speed_mps.min(cap),
                None => actor.driver.desired_speed_mps,
            };
            // Slow down *before* a slower lane or a turn rather than at its boundary. A
            // vehicle committed to clearing a junction on amber slows too, but only as far
            // as still reaches the stop line before the amber ends. Holding its speed
            // instead carried it into a 6 m/s turn at 11 m/s, and the car-following model
            // braked it at −6 m/s² on the connector (the auditor's jerk class).
            let v0 = if along {
                let anticipated = self.anticipated_speed(world, actor);
                match commit_floor {
                    None => v0.min(anticipated),
                    Some(floor) => v0.min(anticipated.max(floor)),
                }
            } else {
                v0
            };
            let ego_capped = VehicleView {
                driver: DriverProfile {
                    desired_speed_mps: v0,
                    ..actor.driver
                },
                ..ego
            };
            // A stop line is stopped *at*. The junction decisions place their virtual
            // obstacle `STOP_LINE_OFFSET_M` before the line (the legacy engine's 2 m) and the
            // car-following model then stops its standstill gap `s0` behind that, so the first
            // car of every queue stood 4 m back from its own stop line: its rear had 9 m to
            // cover before the queue discharge began to count, which alone put a second on
            // the HCM's first headway. The obstacle is moved so the front stops
            // [`STOP_LINE_MARGIN_M`] short of the line.
            let mut line_slack = 0.0;
            if along
                && let Some(line) = binding_stop_line
                && let Some(l) = leader.as_mut()
                && !l.is_vehicle()
                && l.speed_mps == 0.0
            {
                l.gap_m = l
                    .gap_m
                    .max(line - STOP_LINE_MARGIN_M + actor.driver.min_gap_m);
                // The relaxed stop may use the room up to just short of the line, never
                // past it — which, the obstacle now standing beyond the line, is a
                // negative slack.
                line_slack = (line - 0.2) - l.gap_m;
            } else if let Some(line) = binding_stop_line
                && let Some(l) = leader.as_ref()
            {
                line_slack = (line - l.gap_m - 0.5).max(0.0);
            }
            // A human driver sees the car ahead through their estimation errors and acts on
            // what they decided their reaction time ago ([`crate::drivers`]).
            let hp = &self.params.human;
            let mut human_next = None;
            let mut accel = if along && hp.imperfect() && actor.class != VehicleClass::Pedestrian {
                let step_ns = (dt_s * 1e9).round().max(1.0) as u64;
                let k = t0 / step_ns;
                let key = actor.traits.noise_key
                    ^ crate::drivers::splitmix64(u64::from(actor.id.index()));
                let mut seen = leader;
                let (mut wg, mut wr) = (actor.human.w_gap, actor.human.w_rate);
                if let Some(l) = seen.as_mut()
                    && l.is_vehicle()
                    && (hp.distance_error > 0.0 || hp.rate_error_per_s > 0.0)
                {
                    let p = crate::drivers::perceive(
                        hp,
                        &actor.human,
                        key,
                        k,
                        dt_s,
                        l.gap_m,
                        ego.speed_mps,
                        l.speed_mps,
                    );
                    l.gap_m = p.gap_m;
                    l.speed_mps = p.leader_speed_mps;
                    wg = p.w_gap;
                    wr = p.w_rate;
                }
                let now = self
                    .cf
                    .accel(&ego_capped, seen.as_ref(), &lane_view, &self.weather);
                human_next = Some((wg, wr, now));
                crate::drivers::delayed(
                    &actor.human,
                    hp.reaction_time_s,
                    dt_s,
                    now,
                    actor.driver.comfort_decel_mps2,
                )
            } else {
                VehicleCf(&self.cf, &self.vtype_cf)
                    .accel(&ego_capped, leader.as_ref(), &lane_view, &self.weather)
            };
            // A merge partner that reaches the merge point first is fallen in behind by the
            // time the ego gets there ([`merge_accel`]), not braked for as if it stood at
            // the ego's bumper now.
            let merge_limit = merge_partner.as_ref().map(|m| {
                let tau = merge_arrival_s(
                    m.d_self_m,
                    ego.speed_mps,
                    actor.driver.max_accel_mps2,
                    self.connector_speed(world, world.lane(actor.lane).id),
                );
                merge_accel(&ego_capped, m, tau)
            });
            if let Some(limit) = merge_limit {
                accel = accel.min(limit);
            }
            if along
                && let Some(l) = leader.as_ref()
                && !l.is_vehicle()
                && l.speed_mps == 0.0
            {
                accel = static_obstacle_accel(
                    accel,
                    ego.speed_mps,
                    l.gap_m,
                    actor.driver.min_gap_m,
                    line_slack,
                );
                // A planned stop is braked into, not stamped on: the deceleration builds
                // at no more than `PLANNED_BRAKE_JERK_MPS3` while the stop needs no more
                // than a firm service brake. Harder than that is an emergency and is not
                // limited.
                if accel < actor.accel_mps2 && -accel <= PLANNED_STOP_MAX_DECEL_MPS2 {
                    accel = accel.max(actor.accel_mps2 - PLANNED_BRAKE_JERK_MPS3 * dt_s);
                }
                // Relaxing the stop relaxes the obstacle, not the driver's own speed: a
                // vehicle above the speed it has to slow to for the turn beyond the line
                // still slows for it. Without this, the relaxed stop for a line 60 m away
                // (−0.8 m/s²) overrode the anticipation of a 6 m/s turn, the car reached
                // the connector at 9.7 m/s, and the model braked it there at −5.6 m/s².
                accel = accel.min(VehicleCf(&self.cf, &self.vtype_cf).accel(&ego_capped, None, &lane_view, &self.weather));
                // Whatever the stop line allows, the car ahead still binds. The relaxed
                // stop lets a vehicle creep up to a line (or a crosswalk's edge) it is
                // already inside its standstill gap of; with a car standing just beyond
                // that obstacle, it crept into the car's standstill gap at 0.1-0.2 m/s
                // for seconds on end (the auditor's gap-below-s0 class, 308 vehicle-steps
                // on the shipped Midtown run).
                if let Some(v) = nearest_vehicle {
                    accel =
                        accel.min(
                            VehicleCf(&self.cf, &self.vtype_cf)
                                .accel(&ego_capped, Some(&v), &lane_view, &self.weather),
                        );
                }
                // And so does a merge partner ahead: relaxed, two cars converging on a
                // lane drop crept side by side towards the same crosswalk at 0.1 m/s,
                // each overriding the other, until their bodies met (333 overlap steps on
                // dense Midtown).
                if let Some(limit) = merge_limit {
                    accel = accel.min(limit);
                }
            }
            // A red or amber the driver decided to stop for binds even while a car ahead,
            // nearer than the line, is going on through it: following that car alone, a
            // driver 37 m from the line at 10 m/s on the amber's onset held its speed 12 m
            // behind a leader clearing the junction, and once the leader had gone was too
            // close to stop and entered on red (peak-density grid, 20,000 veh/h). Drivers
            // stop for the signal, not for the car in front (UVC §11-202(c)); the stop is
            // braked into as any stop line is ([`static_obstacle_accel`]).
            if along
                && let Some((stop, line)) = signal_stop
                && leader.is_some_and(|l| l.is_vehicle() && l.gap_m < stop.gap_m)
            {
                let mut stop = stop;
                stop.gap_m = stop
                    .gap_m
                    .max(line - STOP_LINE_MARGIN_M + actor.driver.min_gap_m);
                let slack = (line - 0.2) - stop.gap_m;
                let a_stop = static_obstacle_accel(
                    VehicleCf(&self.cf, &self.vtype_cf)
                        .accel(&ego_capped, Some(&stop), &lane_view, &self.weather),
                    ego.speed_mps,
                    stop.gap_m,
                    actor.driver.min_gap_m,
                    slack,
                );
                accel = accel.min(a_stop);
            }
            // Any braking short of an emergency builds at a service-brake rate, whatever
            // asked for it — a car ahead braking, a merge partner, a pedestrian stepping
            // out — not only a stop line: the car-following model has no jerk term, and a
            // new constraint stepped the deceleration from +1 to −3 m/s² in one 0.1 s step
            // (40 m/s³, the auditor's jerk class on the dense grid). Past
            // `PLANNED_STOP_MAX_DECEL_MPS2` it is an emergency, and nothing is limited.
            // Not below 2 m/s, where the car-following model's last few centimetres into
            // a queue are what keep a car out of its standstill gap: limited there, cars
            // stopped 5 cm inside it (2,530 vehicle-steps on the dense grid).
            if along
                && ego.speed_mps > 2.0
                && accel < actor.accel_mps2
                && -accel <= PLANNED_STOP_MAX_DECEL_MPS2
            {
                accel = accel.max(actor.accel_mps2 - PLANNED_BRAKE_JERK_MPS3 * dt_s);
            }
            // A scripted emergency stop: the driver stands on the brake. The deceleration
            // builds at the brake system's onset rate and then holds, capped by the road's
            // grip; whatever else binds harder (a car ahead stopping shorter) still binds.
            if let Some((decel, _)) = actor.forced_brake
                && ego.speed_mps > STANDSTILL_MPS
            {
                let grip = crate::weather::surface_friction(self.weather.surface)
                    .map_or(f64::INFINITY, |mu| mu * 9.806_65);
                let target = -decel.min(grip.max(0.5));
                let onset = actor.accel_mps2.min(0.0) - EMERGENCY_BRAKE_ONSET_JERK_MPS3 * dt_s;
                accel = accel.min(onset.max(target));
            }
            // Moving off takes the driver their reaction time: a vehicle standing still
            // goes only once what held it has let it go for `reaction_s` ([`DriverTraits`]).
            // This is the start-up wave of a queue at green — the source of the HCM's
            // start-up lost time — which the IDM, reacting instantly, does not have.
            // Behind a car in a queue, the driver's own start-up delay applies instead: they
            // have watched the queue start and respond to the car in front moving.
            let mut release_at = None;
            if along
                && ego.speed_mps < STANDSTILL_MPS
                && actor.traits.reaction_s > 0.0
                && accel > GO_ACCEL_MPS2
            {
                let since = actor.release_at.unwrap_or(t0);
                release_at = Some(since);
                let queued = nearest_vehicle.is_some_and(|v| v.gap_m < QUEUE_FOLLOW_GAP_M);
                let delay = if queued {
                    actor.traits.follow_reaction_s
                } else {
                    actor.traits.reaction_s
                };
                if ns_to_secs(t0.saturating_sub(since)) < delay {
                    accel = accel.min(0.0);
                }
            }
            // Coming off the brake happens at no more than `RELEASE_JERK_MPS3`, until the
            // brake is off; from there the car-following model's acceleration applies as
            // it is. Limiting an *increase* in acceleration can only leave a vehicle
            // further back than the model asked, never closer.
            if along && actor.accel_mps2 < 0.0 {
                accel = accel.min(actor.accel_mps2 + RELEASE_JERK_MPS3 * dt_s);
            }
            let v0_effective = v0.min(lane_view.speed_limit_mps);
            // A standing (or, for a heavy vehicle, crawling) vehicle ahead is stopped behind
            // at its standstill gap with the least constant deceleration that does, as a stop
            // line is (`standing_leader_stop`): the IDM's braking scales with its `a`, and a
            // delivery van creeping up on a stopped car reached its gap at 0.93 m/s and was
            // stopped in one step by the advance cap below.
            let crawl_floor = if actor.driver.max_accel_mps2 < 1.0 {
                HEAVY_CRAWL_MPS
            } else {
                STANDSTILL_MPS
            };
            // A bend of the lane polyline ahead is braked for at the constant deceleration
            // that reaches its turn speed at its start (`lane_bend_speed`), as a stop line
            // is: the IDM's free term tracks a lowered desired speed only from above, by
            // about `b / 4a` (a 6.5 m/s kink was taken at 9 m/s).
            if along && let Some(need) = self.bend_floor(world, actor, dt_s) {
                accel = accel.min(need);
            }
            if along
                && self.params.standing_leader_stop
                && actor.speed_mps > 0.0
                && let Some(v) = nearest_vehicle.filter(|v| {
                    v.speed_mps.abs() < 1e-6
                        || (v.speed_mps.abs() < crawl_floor && actor.speed_mps > v.speed_mps)
                })
            {
                let room = v.gap_m + v.speed_mps.max(0.0) * dt_s - actor.driver.min_gap_m;
                if room > 1e-3 {
                    let need = -(actor.speed_mps * actor.speed_mps) / (2.0 * room);
                    if -need >= STOP_FLOOR_BIND_MPS2 {
                        accel = accel.min(need.max(-actor.class.spec().emergency_decel_mps2));
                    }
                }
            }
            // A car stopping behind one that stands still stops its standstill gap short of
            // it. The car-following model only approaches that gap asymptotically, and the
            // 0.1 s integration of the calibrated city drivers overshot it by up to 5 cm and
            // stood there (2,530 vehicle-steps under s0 on the dense grid).
            //
            // "Standing" includes a leader all but stopped that the follower is closing on —
            // a cyclist closing its own 0.5 m gap at 0.4 mm/s let a car behind it, uncapped
            // because its leader was not at exactly zero, stop 5 cm inside its 2 m gap. Such
            // a leader's own advance this step is added to the room. A follower moving off
            // behind a leader that is moving off is not capped.
            // A truck or a bus (the IDM's braking scales with `a`, 0.7 m/s² for them) closing
            // on a leader that is creeping to a stop crept on into its own 4 m standstill
            // gap: for those drivers a leader crawling below `HEAVY_CRAWL_MPS` counts as
            // standing too.
            let crawl = if actor.driver.max_accel_mps2 < 1.0 {
                HEAVY_CRAWL_MPS
            } else {
                STANDSTILL_MPS
            };
            let advance_cap_m = if along {
                nearest_vehicle
                    .filter(|v| {
                        v.speed_mps.abs() < 1e-6
                            || (v.speed_mps.abs() < crawl && actor.speed_mps > v.speed_mps)
                    })
                    .map(|v| {
                        (v.gap_m + v.speed_mps.max(0.0) * dt_s - actor.driver.min_gap_m).max(0.0)
                    })
            } else {
                None
            };
            Some((
                Decision {
                    actor: *id,
                    accel_mps2: accel,
                    v0_mps: v0_effective,
                    lane_change: LaneChangeDecision::Stay,
                    amber_commit,
                    amber_seen,
                    signal_look,
                    violation,
                    release_at,
                    advance_cap_m,
                    detour_ok: false,
                    human: human_next,
                    lateral: self
                        .lateral
                        .as_ref()
                        .filter(|_| actor.transition.is_none())
                        .map(|m| {
                            crate::lateral::decide_in_lane(
                                &**m,
                                &snapshot,
                                &ego,
                                v0_effective,
                                &lane_view,
                                lane_position_for(&self.params, actor.class),
                            )
                        }),
                },
                ego_capped,
                nbrs,
            ))
        };
        let decided = crate::par::map_ordered(&ids, decide_one);
        for (decision, ego_capped, nbrs) in decided.into_iter().flatten() {
            neighbours.insert(decision.actor, (ego_capped, nbrs));
            decisions.push(decision);
        }
        // Lane-change decisions: a second walk, because MOBIL needs the context for its
        // reconsideration draw and the borrow of the world above is shared.
        if let Some(mobil) = self.lane_change.take() {
            // The vehicles standing at the kerb, from the start-of-step state.
            let at_kerb: BTreeSet<ActorId> = self
                .actors
                .values()
                .filter(|a| a.kerb.at_kerb())
                .map(|a| a.id)
                .collect();
            // LC2013's per-vehicle memory, written back after the walk.
            let lc_params = self.params.lc2013;
            let lc_dt = self.params.step.as_secs_f64();
            let mut lc_updates: Vec<(ActorId, crate::lanechange::lc2013::Lc2013State)> = Vec::new();
            // Each lane's brutto occupation (length plus minimum gap of the vehicles on it),
            // for LC2013's strategic urgency.
            let mut lc_occupation: BTreeMap<LaneId, f64> = BTreeMap::new();
            if lc_params.is_some() {
                for a in self.actors.values() {
                    *lc_occupation.entry(a.lane).or_default() +=
                        a.class.spec().length_m + a.driver.min_gap_m;
                }
            }
            for decision in &mut decisions {
                let Some(actor) = self.actors.get(&decision.actor) else {
                    continue;
                };
                if lc_params.is_some() {
                    let mut st = actor.lc;
                    if actor.transition.is_some() {
                        st.changed();
                    }
                    st.track_speed(actor.speed_mps, lc_dt);
                    lc_updates.push((actor.id, st));
                }
                if actor.transition.is_some() || t0 < actor.cooldown_until {
                    continue;
                }
                // A strategic change still owed by the no-change zone is the one change the
                // zone allows: the vehicle waits at the end of its lane for it (the barrier
                // in the decision pass) instead of driving on with no way out.
                let owed = self.along_path()
                    && self.strategic_target(ctx.world(), actor).is_some()
                    && actor.s_m >= actor.class.spec().length_m;
                if self.along_path()
                    && !owed
                    && !self.may_consider_lane_change(ctx.world(), actor, &mobil)
                {
                    continue;
                }
                // A strategic change the route needs comes before anything discretionary.
                // It too keeps out of the no-change zone (MUTCD 2009 §3B.04's solid line):
                // a driver who reaches the zone without a gap gives up and finds another
                // way ([`Self::give_up_strategic`]).
                // LC2013: further out than the look-ahead the change waits unless the
                // target lane is no slower (`changeToBest`), and meanwhile the driver
                // weighs discretionary changes as SUMO's `_wantsChange` goes on to do.
                if self.along_path() && self.strategic_target(ctx.world(), actor).is_some() {
                    if let Some((ego, nbrs)) = neighbours.get(&decision.actor) {
                        if let Some(p) = lc_params
                            && let Some(st) = lc_updates.last()
                            && !self.lc2013_strategic_due(
                                ctx.world(),
                                &p,
                                &st.1,
                                actor,
                                ego,
                                nbrs,
                                &lc_occupation,
                            )
                        {
                        } else {
                            if let Some(change) =
                                self.strategic_change(ctx.world(), &mobil, actor, ego, nbrs)
                            {
                                decision.lane_change = change;
                            } else if let Some(cap) =
                                self.strategic_fall_back(ctx.world(), actor, ego, nbrs)
                            {
                                decision.accel_mps2 = decision.accel_mps2.min(cap);
                            }
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                // A vehicle making for its kerbside stop keeps to the kerb lane.
                if actor.kerb.stop.is_some() {
                    continue;
                }
                let Some((ego, nbrs)) = neighbours.get(&decision.actor) else {
                    continue;
                };
                // A scripted cut-in takes the side it was told to, into the gap ahead of
                // that lane's follower, whatever MOBIL's incentive or its safety
                // criterion say — that is what makes it a cut-in. The resolution below
                // still refuses a gap the body does not fit.
                if let Some((side, until)) = actor.forced_cut_in
                    && t0 < until
                    && let Some(target) = nbrs.side(side)
                    && let Some(target_lane) = ctx.world().try_lane(target.lane)
                {
                    let from_w = ctx.world().lane(actor.lane).width_m;
                    let lateral = 0.5 * (from_w + target_lane.width_m);
                    decision.lane_change = LaneChangeDecision::Change {
                        to: target.lane,
                        side,
                        incentive_mps2: 0.0,
                        duration: crate::lanechange::mobil::transition_duration(
                            mobil.params(),
                            lateral,
                            ego.speed_mps,
                        ),
                    };
                    continue;
                }
                // Round a vehicle standing at the kerb: the driver behind goes past it when
                // the lane beside has room, however slowly they are moving, and detours if
                // that lane cannot make their next turn ([`Self::pass_kerbside`]).
                if self.along_path()
                    && !at_kerb.is_empty()
                    && let Some(change) =
                        self.pass_kerbside(ctx.world(), &mobil, &at_kerb, ego, nbrs)
                {
                    decision.lane_change = change;
                    decision.detour_ok = true;
                    continue;
                }
                // A vehicle due a kerbside stop moves right, towards the kerb, when the lane
                // to its right has room ([`Self::move_to_kerb`]); while it looks for the kerb
                // it makes no discretionary change away from it.
                if self.along_path() && actor.kerb.due() {
                    if let Some(change) = self.move_to_kerb(ctx.world(), &mobil, ego, nbrs) {
                        decision.lane_change = change;
                    }
                    continue;
                }
                decision.lane_change = match (lc_params, lc_updates.last_mut()) {
                    (Some(p), Some((_, st))) => {
                        self.lc2013_decide(ctx.world(), &mobil, &p, st, actor, ego, nbrs, lc_dt)
                    }
                    _ => mobil.decide(ctx, ego, nbrs, &self.weather),
                };
                // Heavy goods vehicles keep out of a motorway's leftmost lane where the
                // carriageway has three or more ([`crate::drivers`], StVO §18(11)).
                if self.params.human.truck_lane_restriction
                    && matches!(actor.class, VehicleClass::Truck | VehicleClass::Trailer)
                    && let LaneChangeDecision::Change {
                        to,
                        side: Side::Left,
                        ..
                    } = decision.lane_change
                    && crate::drivers::is_restricted_left_lane(ctx.world(), to)
                {
                    decision.lane_change = LaneChangeDecision::Stay;
                }
            }
            for (id, st) in lc_updates {
                if let Some(a) = self.actors.get_mut(&id) {
                    a.lc = st;
                }
            }
            self.lane_change = Some(mobil);
        }
        let reroutes = if self.along_path() {
            self.resolve_lane_changes(ctx.world(), t0, &mut decisions)
        } else {
            BTreeMap::new()
        };

        // --- pass 7: integrate and publish ----------------------------------
        let world = ctx.world();
        let costs = self.costs(world);
        let generation = costs.generation();
        let mut despawned: Vec<(ActorId, DespawnCause)> = Vec::new();
        let along = self.along_path();
        let kerb_params = self.params.kerbside;
        let no_change_zone_m = self.params.no_change_zone_m;
        for decision in &decisions {
            let Some(actor) = self.actors.get_mut(&decision.actor) else {
                continue;
            };
            // A crashed vehicle stands where it collided, blocking its lane, until it is
            // cleared.
            if let Some(crash) = actor.crash {
                actor.speed_mps = 0.0;
                actor.accel_mps2 = 0.0;
                if t1 >= crash.clear_at {
                    despawned.push((decision.actor, DespawnCause::Collision));
                }
                continue;
            }
            actor.signal_look = decision.signal_look;
            if decision.violation != 0 {
                actor.violation |= decision.violation;
            } else if decision.amber_commit.is_none()
                && world.lane(actor.lane).kind != LaneKind::Internal
            {
                actor.violation = 0;
            }
            // Longitudinal: the legacy integration order — the new speed carries the step,
            // which is what keeps a vehicle from rolling through a stop line.
            //
            // The ceiling is the desired speed, as in the reference engine — except that
            // outside the legacy parity mode it never *cuts* a speed the vehicle already
            // has. Entering a slower lane used to chop the speed to the new limit in one
            // step (11.2 -> 4.5 m/s in 0.1 s on Manhattan's 10 mph lanes, a 67 m/s²
            // deceleration); the car-following model now brakes for it instead, and the
            // anticipation in the decision pass has it slowing before the boundary.
            actor.accel_mps2 = decision.accel_mps2;
            actor.amber_commit = decision.amber_commit;
            actor.amber_seen = decision.amber_seen;
            actor.release_at = decision.release_at;
            if let Some((wg, wr, decided)) = decision.human {
                actor.human.w_gap = wg;
                actor.human.w_rate = wr;
                crate::drivers::remember(
                    &mut actor.human,
                    self.params.human.reaction_time_s,
                    dt_s,
                    decided,
                );
            }
            let ceiling = if along {
                decision.v0_mps.max(actor.speed_mps)
            } else {
                decision.v0_mps
            };
            let before = actor.speed_mps;
            actor.speed_mps = (actor.speed_mps + decision.accel_mps2 * dt_s).clamp(0.0, ceiling);
            // The cap is a braking decision like any other, so it cannot take more than a
            // tyre gives: cutting 1.05 m/s to 0 in one step read as -10.5 m/s² (phase1-grid
            // lane 119, t 88.4 s). Held to `CAP_MAX_DECEL_MPS2` the vehicle overruns the
            // cap by under 2 cm at a crawl and stops on the next step.
            if let Some(cap) = decision.advance_cap_m
                && dt_s > 0.0
                && actor.speed_mps * dt_s > cap
            {
                actor.speed_mps = (cap / dt_s)
                    .max(before - CAP_MAX_DECEL_MPS2 * dt_s)
                    .max(0.0);
            }
            // What is published is the acceleration the vehicle *had*, not the one the
            // model asked for: a car standing at a red line has a car-following
            // acceleration of up to −6 m/s² with its speed clamped at zero, and it was
            // publishing that — into its pose, and into every BSM it signed. Not in the
            // legacy parity mode, which publishes the command as the reference did.
            if along && dt_s > 0.0 {
                actor.accel_mps2 = (actor.speed_mps - before) / dt_s;
            }
            // The speed is the vehicle's own, along the path it drives; off the centreline
            // on a bend (in a lane change) that path is shorter or longer than the
            // centreline, so the centreline arc advances by what moves the published
            // point `v·dt` along the offset path — one secant step on the same offset
            // curve the pose is drawn from. Advancing it by `v·dt` had a car 3.3 m inside a
            // 9 m bend moving 37 % less than its reported speed (the auditor's step-speed
            // class on Midtown); the curvature formula `1/(1 − d·κ)`, tried first, spiked
            // at polyline vertices into 1.4-1.7 m steps (teleports).
            let base = actor.speed_mps * dt_s;
            let advance = if along && actor.lateral_m != 0.0 && base > 0.0 {
                let lane = world.lane(actor.lane);
                let rear = actor.s_m - actor.class.spec().length_m;
                if rear >= 0.0 && rear + base <= lane.length_m {
                    let p0 = smooth_offset_point(lane, rear, actor.lateral_m);
                    let p1 = smooth_offset_point(lane, rear + base, actor.lateral_m);
                    let moved = p0.distance(p1);
                    if moved > 1e-6 {
                        base * (base / moved).clamp(0.67, 1.5)
                    } else {
                        base
                    }
                } else {
                    base
                }
            } else {
                base
            };
            actor.s_m += advance;
            if let Some((_, hold_until)) = actor.forced_brake
                && actor.speed_mps <= STANDSTILL_MPS
            {
                // Standing: the emergency stop becomes a hold, released when the script
                // says (the driver moves off with their own reaction time after that).
                actor.forced_brake = None;
                actor.stopped_until = Some(hold_until.unwrap_or(SimTime::MAX));
            }
            if actor.stopped_until.is_some_and(|until| t0 >= until) {
                actor.stopped_until = None;
            }

            // Start a lane change, if one was decided. The vehicle enters the target
            // lane's frame immediately, offset sideways by the distance between the two
            // centrelines, and the smoothstep slides that offset to zero over the
            // transition — which is the legacy engine's lateral model [run.py L2361-2401].
            if let LaneChangeDecision::Change {
                to, side, duration, ..
            } = decision.lane_change
            {
                let separation = 0.5 * (world.lane(actor.lane).width_m + world.lane(to).width_m);
                let home = lane_position_for(&self.params, actor.class);
                let nominal = actor.lateral_m
                    + match side {
                        // Moving left puts the vehicle to the *right* of the target lane's
                        // centreline, and the offset closes from there.
                        Side::Left => -separation,
                        Side::Right => separation,
                    };
                // Where the vehicle actually is, across the target lane. Two lanes' half
                // widths are their centrelines' separation only where they abut; a bicycle
                // lane past a buffer or a parking lane, or two imported lanes that overlap,
                // do not, and the half-width offset put a cyclist leaving the bike lane on
                // Midtown 1 m sideways in one step (`vru_motion`: 1.06 m, 92 m/s²). The
                // nominal offset stays the answer where the projection is not a sensible
                // one (a target lane that does not run beside this one here).
                let from_offset_m = {
                    let here_lane = world.lane(actor.lane);
                    let here =
                        here_lane.offset_point(actor.s_m.min(here_lane.length_m), actor.lateral_m);
                    let measured = world.lane(to).project_point(here).d_m;
                    if (measured - nominal).abs() <= separation {
                        measured
                    } else {
                        nominal
                    }
                };
                actor.forced_cut_in = None;
                actor.transition = Some(Transition {
                    from: actor.lane,
                    to,
                    started: t0,
                    duration,
                    from_offset_m,
                    to_offset_m: home,
                    switched: false,
                    from_s_delta: 0.0,
                    // Only the kerbside rules change lanes this slowly (MOBIL's own minimum
                    // is 3 m/s): a pull-out round a vehicle at the kerb, or a move to it.
                    pull_out: actor.speed_mps < PULL_OUT_SPEED_MPS,
                    began_moving: actor.speed_mps > STANDSTILL_MPS,
                });
                // The route is re-planned from the lane being entered, so the vehicle's
                // next movement is one that lane actually has. Keeping the old route
                // left `route_index` pointing into the lane it had left: at the junction
                // it crossed onto the *other* lane's connector — a sideways jump through
                // the junction that ignored the signal for its own lane.
                if let Some((route, destination)) = reroutes.get(&decision.actor) {
                    actor.route = route.clone();
                    actor.route_index = 0;
                    actor.destination = *destination;
                    actor.planned_at = t0;
                }
                // The rule belongs to the lane-change model, so it is asked for rather
                // than reconstructed: `max(cooldown_factor·transition_s,
                // cooldown_factor·duration)`. Rebuilding it here as `max(2·duration, 5 s)`
                // was arithmetically identical only because no constructor lets a scenario
                // pass custom `MobilParams` — the moment one does, the engine and the card
                // would disagree silently, and the card is what a reader trusts.
                actor.cooldown_until = self
                    .lane_change
                    .as_ref()
                    .map_or_else(
                        || duration.saturating_mul(2).max(Duration::from_secs_f64(5.0)),
                        |mobil| mobil.cooldown(duration),
                    )
                    .after(t0);
            }

            // Advance the lateral transition.
            if let Some(mut transition) = actor.transition {
                // A vehicle brought to a stand part-way across does not go on sliding
                // sideways: a car cannot move across the road without moving along it, and
                // the transition is a function of time, so it is held while the vehicle
                // stands. Left running, a 7 m truck stopped 9 m into a pull-out round a
                // taxi swung its heading from 94 to 133 degrees on the spot and swept its
                // body into the queue in the next lane (`tests/realism.rs`, kerbside on:
                // 206 overlap steps).
                //
                // The hold ends when the stretch of the target lane beside the body is
                // clear: the vehicle then finishes the move into it (its heading is held,
                // `lateral_rate_at`). Held for good, a car that changed out of a queue and
                // stopped behind that queue's next car — its body still over the line, so
                // it follows that car too — stood part-way across for the rest of the run
                // and blocked both lanes: every `phantom_stop` of the signalised grid at
                // 9,000 veh/h (`check-traffic`: 3, 1, 3, 5 and 7 over five same-seed
                // variants, 25 to 110 s each, lateral offsets 1.2 to 2.2 m) and the
                // kerbside test's standstill (actor 62, 180 s beside a delivery van). A
                // driver in that spot steers into the free lane; SUMO lets a standing
                // vehicle finish a lane change by default (`lcMaxSpeedLatStanding` =
                // `maxSpeedLat`). The 7 m truck's next lane was not clear, so it still
                // holds.
                if transition.switched
                    && transition.began_moving
                    && actor.speed_mps <= STANDSTILL_MPS
                    && !lane_clear_beside(
                        &snapshot,
                        actor.id,
                        transition.to,
                        actor.s_m,
                        actor.class.spec().length_m,
                    )
                {
                    transition.started = transition.started.saturating_add(t1.saturating_sub(t0));
                }
                let elapsed = ns_to_secs(t1.saturating_sub(transition.started));
                let total = transition.duration.as_secs_f64().max(1e-9);
                if !transition.switched {
                    // The lane itself changes at the start of the transition: the vehicle is
                    // in the target lane's frame, offset sideways, and slides in.
                    let target = transition.to;
                    let projection = {
                        let from_lane = world.lane(transition.from);
                        let point = from_lane.offset_point(actor.s_m.min(from_lane.length_m), 0.0);
                        world.lane(target).project_point(point)
                    };
                    transition.from_s_delta = actor.s_m - projection.s_m;
                    actor.lane = target;
                    actor.trail.clear();
                    actor.s_m = projection.s_m;
                    actor.route_index = actor
                        .route
                        .lanes
                        .iter()
                        .position(|l| *l == target)
                        .unwrap_or(actor.route_index);
                    transition.switched = true;
                }
                let p = (elapsed / total).clamp(0.0, 1.0);
                actor.lateral_m = transition.from_offset_m
                    + (transition.to_offset_m - transition.from_offset_m) * smoothstep(p);
                if p >= 1.0 {
                    actor.lateral_m = transition.to_offset_m;
                    actor.transition = None;
                } else {
                    actor.transition = Some(transition);
                }
            }

            // Steer across the lane, when a sub-lane model decided where to.
            if actor.transition.is_none()
                && let Some(lat) = decision.lateral
            {
                let max = lat.speed_mps * dt_s;
                actor.lateral_m += (lat.target_m - actor.lateral_m).clamp(-max, max);
            }

            // Cross lane boundaries.
            let mut cause: Option<DespawnCause> = None;
            loop {
                let lane_length = world.lane(actor.lane).length_m;
                if actor.s_m <= lane_length {
                    break;
                }
                let mut next = actor.route.lanes.get(actor.route_index + 1).copied();
                // The next lane must be one the lane graph reaches from this one. A route
                // that has lost its place is re-planned here rather than followed onto a
                // lane somewhere else. Not applied in the legacy parity mode.
                if along
                    && let Some(n) = next
                    && !world.successor_lanes(actor.lane).contains(&n)
                {
                    let router = router_of(
                        &self.router,
                        &self.bike_router,
                        &self.class_routers,
                        actor.class,
                    );
                    next = router
                        .replan(world, actor.lane, actor.destination, t1, &costs)
                        .or_else(|| {
                            (actor.class != VehicleClass::Bicycle)
                                .then(|| {
                                    self.router.replan(
                                        world,
                                        actor.lane,
                                        actor.destination,
                                        t1,
                                        &costs,
                                    )
                                })
                                .flatten()
                        })
                        .and_then(|route| {
                            let n = route.lanes.get(1).copied();
                            actor.route = route;
                            actor.route_index = 0;
                            actor.planned_at = t1;
                            n
                        });
                    if next.is_none() {
                        actor.s_m = lane_length;
                        actor.speed_mps = 0.0;
                        cause = Some(DespawnCause::RouteBlocked);
                        break;
                    }
                }
                // A closure met at the line: the driver takes another way from here if
                // there is one ([`Self::detour`]) and otherwise waits at the line for the
                // lane to reopen. Only the legacy parity mode still removes the vehicle.
                if along
                    && let Some(n) = next
                    && self.closed.contains(&n)
                {
                    let router = if actor.class == VehicleClass::Bicycle {
                        &self.bike_router
                    } else {
                        &self.router
                    };
                    match Self::detour(router, world, actor.lane, actor.destination, t1, &costs) {
                        Some((route, to)) => {
                            next = route.lanes.get(1).copied();
                            actor.route = route;
                            actor.route_index = 0;
                            actor.destination = to;
                            actor.planned_at = t1;
                            actor.planned_generation = generation;
                        }
                        None => {
                            actor.s_m = lane_length;
                            actor.speed_mps = 0.0;
                            actor.accel_mps2 = 0.0;
                            break;
                        }
                    }
                }
                match next {
                    Some(next) => {
                        if self.closed.contains(&next) {
                            actor.s_m = lane_length;
                            actor.speed_mps = 0.0;
                            cause = Some(DespawnCause::RouteBlocked);
                            break;
                        }
                        actor.s_m -= lane_length;
                        actor.trail.push(actor.lane);
                        if actor.trail.len() > TRAIL_LANES {
                            actor.trail.remove(0);
                        }
                        actor.lane = next;
                        actor.route_index += 1;
                    }
                    None => {
                        actor.s_m = lane_length;
                        cause = Some(DespawnCause::TripComplete);
                        break;
                    }
                }
            }
            // Kerbside stops: arrive, dwell, leave, and plan the next one
            // ([`crate::kerbside`]) — from this vehicle's own state at the step's end.
            if along && cause.is_none() && actor.kerb.role.is_some() {
                let changing = actor.transition.is_some();
                let length_m = actor.class.spec().length_m;
                actor.kerb.advance(
                    &*ctx,
                    &kerb_params,
                    world,
                    actor.seq,
                    length_m,
                    t1,
                    actor.lane,
                    actor.s_m,
                    actor.speed_mps,
                    advance,
                    &actor.route.lanes,
                    actor.route_index,
                    changing,
                    no_change_zone_m,
                );
            }
            if cause.is_none()
                && let Some(limit) = self.params.max_lifetime
                && t1.saturating_sub(actor.spawned) >= limit.as_nanos()
            {
                cause = Some(DespawnCause::LifetimeExpired);
            }
            if let Some(cause) = cause {
                despawned.push((decision.actor, cause));
            }
        }
        // Re-plan whoever is due: on a closure, and with congestion-aware rerouting on,
        // once a period on the lane speeds just sampled.
        self.sample_congestion(world, t1);
        let costs = if self.params.reroute_period_s > 0.0 {
            self.costs(world)
        } else {
            costs
        };
        let period = secs_to_ns(self.params.reroute_period_s.max(0.0));
        if self.params.dynamic_rerouting {
            // Only a driver with a navigation app re-plans on what the network shows; a
            // scripted destination change (`Reroute`) re-plans whoever it is sent to.
            let navigation = self.params.reroute_source == RerouteSource::Navigation;
            let due: Vec<(ActorId, bool)> = self
                .actors
                .values()
                .filter_map(|a| {
                    if a.planned_generation == u64::MAX {
                        return self
                            .router
                            .due(t1, a.planned_at, a.planned_generation, generation)
                            .then_some((a.id, false));
                    }
                    let due = navigation
                        && a.traits.reroutes
                        && (self
                            .router
                            .due(t1, a.planned_at, a.planned_generation, generation)
                            || (period > 0
                                && t1.saturating_sub(a.planned_at) >= period
                                && world.lane(a.lane).kind != LaneKind::Internal));
                    due.then_some((a.id, true))
                })
                .collect();
            for (id, informed) in due {
                let (lane, s_m, destination, class) = {
                    let a = &self.actors[&id];
                    (a.lane, a.s_m, a.destination, a.class)
                };
                let own = self.costs_with_hazards(&costs, id);
                let replanned = self.plan_route_strategic(
                    class,
                    world,
                    lane,
                    s_m,
                    destination,
                    t1,
                    own.as_ref().unwrap_or(&costs),
                );
                if informed {
                    self.info_rerouted.insert(id);
                }
                let a = self.actors.get_mut(&id).expect("present");
                a.planned_at = t1;
                a.planned_generation = generation;
                if let Some(route) = replanned {
                    a.route = route;
                    a.route_index = 0;
                }
            }
        }
        if along {
            let gone: Vec<ActorId> = despawned.iter().map(|(id, _)| *id).collect();
            for id in self.resolve_gridlock(world, t1, &gone) {
                despawned.push((id, DespawnCause::RouteBlocked));
            }
        }
        for (id, _) in &despawned {
            self.actors.remove(id);
        }
        if along && !self.closed.is_empty() {
            self.closure_detours(world, t1, &costs);
        }
        if !self.heard_hazards.is_empty() {
            self.heard_hazards
                .retain(|id, _| !despawned.iter().any(|(d, _)| d == id));
            self.hazard_detours(world, t1, &costs);
        }
        despawned.extend(vru_gone);
        if along && self.params.violations.enabled && self.params.violations.crashes {
            self.resolve_collisions(world, t1);
        }

        // Publish, in actor-id order.
        let mut states: Vec<(ActorId, Kinematics)> = self
            .actors
            .values()
            .map(|a| (a.id, a.kinematics(world, t1, along)))
            .collect();

        // VRUs read the same frozen snapshot the vehicles did, and each crossing's permit:
        // its pedestrian signal at the step's start, and whether a vehicle — where it is
        // now, having moved this step — is on the crosswalk or too close to yield.
        if let Some(mut vru) = self.vru.take() {
            if !self.crosswalks.is_empty() {
                let mut exposure = BTreeMap::new();
                for a in self.actors.values() {
                    let bands = self.crosswalks.ahead(world, &a.crosswalk_path());
                    crate::vru::crosswalk::expose(
                        &bands,
                        a.speed_mps,
                        a.class.spec().length_m,
                        &mut exposure,
                    );
                }
                vru.set_crossing_permits(
                    self.crosswalks.permits_exposed(&signal_states, &exposure),
                );
                if !vru.knows_crosswalks() {
                    vru.set_crosswalk_of(self.crosswalks.walk_map().clone());
                }
            }
            if !midblock.is_empty() {
                let go = self.midblock_permits(&*ctx, world, &vru, &midblock);
                vru.set_midblock_permits(go);
                vru.set_midblock_holds(self.midblock_holds(world, &midblock));
            } else {
                self.midblock_yields.clear();
            }
            if vru.params().midblock.rate_per_100m > 0.0 {
                vru.set_beside_queue(Self::beside_queue(world, &vru, &snapshot));
            }
            let mut people = vru.step(ctx, dt, &snapshot);
            states.append(&mut people);
            self.vru = Some(vru);
        }
        states.sort_by_key(|(a, _)| *a);
        self.published = states.iter().copied().collect();
        spawned.sort_by_key(|s| s.actor);
        despawned.sort_by_key(|(a, _)| *a);
        let activities = self
            .vru
            .as_ref()
            .map(|v| {
                v.people()
                    .filter(|p| !p.arrived)
                    .map(|p| (p.actor, p.activity.code()))
                    .collect()
            })
            .unwrap_or_default();
        // A demand model that follows its own vehicles (a bus that stops at its stops) is
        // shown where every vehicle is now; its commands apply at the next step's pass 1
        // ([`crate::demand::hooks`]). Nothing is built for a model that does not ask.
        if self.demand.as_ref().is_some_and(|d| d.wants_progress()) {
            let progress: Vec<crate::demand::hooks::ActorProgress> = self
                .actors
                .values()
                .map(|a| crate::demand::hooks::ActorProgress {
                    actor: a.id,
                    seq: a.seq,
                    class: a.class,
                    lane: a.lane,
                    s_m: a.s_m,
                    speed_mps: a.speed_mps,
                    route_index: a.route_index,
                })
                .collect();
            if let Some(d) = self.demand.as_mut() {
                let commands = d.observe(t1, &progress);
                self.pending.extend(commands);
            }
        }
        MobilityUpdate {
            t: t1,
            states,
            spawned,
            despawned,
            signal_states,
            activities,
        }
    }

    fn command(&mut self, _ctx: &mut dyn MobCtx, cmd: MobilityCommand) {
        self.pending.push(cmd);
    }

    fn resume_after_pause(&mut self, paused: Duration) {
        let by = paused.as_nanos();
        if by == 0 {
            return;
        }
        let later = |t: &mut SimTime| *t = t.saturating_add(by);
        for a in self.actors.values_mut() {
            later(&mut a.spawned);
            later(&mut a.planned_at);
            later(&mut a.cooldown_until);
            if let Some(tr) = a.transition.as_mut() {
                later(&mut tr.started);
            }
            if let Some(t) = a.stopped_until.as_mut() {
                later(t);
            }
            if let Some(t) = a.release_at.as_mut() {
                later(t);
            }
            if let Some((_, Some(t))) = a.forced_brake.as_mut() {
                later(t);
            }
            if let Some((_, t)) = a.forced_cut_in.as_mut() {
                later(t);
            }
            if let Some(t) = a.kerb.stop.as_mut().and_then(|s| s.since.as_mut()) {
                later(t);
            }
            if let Some(t) = a.stuck_since.as_mut() {
                later(t);
            }
        }
        for (_, since, _) in &mut self.waiting {
            later(since);
        }
        if let Some(t) = self.speed_sampled_at.as_mut() {
            later(t);
        }
        if let Some(d) = self.demand.as_mut() {
            d.resume_after_pause(paused);
        }
        if let Some(vru) = self.vru.as_mut() {
            vru.resume_after_pause(paused);
        }
    }

    fn kinematics(&self, a: ActorId) -> Option<&Kinematics> {
        self.published.get(&a)
    }

    fn set_weather(&mut self, weather: WeatherState) {
        self.weather = weather;
    }

    fn gridlock(&self) -> Option<GridlockCounters> {
        (self.params.time_to_teleport_s > 0.0).then_some(self.gridlock)
    }

    fn trip_losses(&self) -> Option<(u64, u64)> {
        Some((self.dropped_trips, self.unroutable_trips))
    }

    fn set_demand_multiplier(&mut self, m: f64) -> bool {
        self.demand.as_mut().is_some_and(|d| d.set_multiplier(m))
    }

    fn cooperative(&self) -> Option<Arc<crate::coop::CoopLink>> {
        self.cf.cooperative()
    }

    fn exterior_lamps(&mut self, ctx: &mut dyn MobCtx) -> Vec<(ActorId, u8)> {
        NativeMobility::exterior_lamps(self, ctx)
    }

    fn intent(&self, world: &World, a: ActorId) -> Option<crate::views::Intent> {
        let actor = self.actors.get(&a)?;
        let here = world.try_lane(actor.lane)?;
        let movement = |approach: LaneId, connector: LaneId, distance_m: f64| {
            let lane = world.try_lane(connector)?;
            let c = world
                .successors(approach)
                .iter()
                .find(|c| c.via == Some(connector))?;
            Some(crate::views::Intent {
                junction: lane.junction?,
                turn: c.direction,
                distance_m,
                approach,
                connector,
            })
        };
        if here.kind == LaneKind::Internal {
            let approach = actor.trail.last().copied().or_else(|| {
                actor
                    .route_index
                    .checked_sub(1)
                    .and_then(|i| actor.route.lanes.get(i).copied())
            })?;
            return movement(approach, actor.lane, -actor.s_m);
        }
        let mut distance = (here.length_m - actor.s_m).max(0.0);
        let mut prev = actor.lane;
        for lane in actor.route.lanes.iter().skip(actor.route_index + 1) {
            let l = world.try_lane(*lane)?;
            if l.kind == LaneKind::Internal {
                return movement(prev, *lane, distance);
            }
            distance += l.length_m;
            prev = *lane;
            if distance > crate::views::INTENT_HORIZON_M {
                return None;
            }
        }
        None
    }
}

/// The stream a driver's decision to yield to a pedestrian waiting mid-block is drawn from.
const MIDBLOCK_YIELD_ID: &str = "vru/midblock/yield";

/// The mid-block paths in use at a step's start, as the vehicles read them.
#[derive(Debug, Default)]
struct MidblockBands {
    /// Each band on each driven lane; its `crosswalk` number is unique to the band.
    by_lane: BTreeMap<LaneId, Vec<v2xw_world::walk::CrosswalkConflict>>,
    /// Band number → (pedestrian, index of the band on its path).
    owner: BTreeMap<usize, (ActorId, usize)>,
    /// Band number → where along the path the lane's near and far edges are, metres.
    reach: BTreeMap<usize, (f64, f64)>,
    /// The bands a pedestrian is in, or about to step into: every driver stops for them.
    occupied: BTreeSet<usize>,
    /// The pedestrians waiting at the kerb for a gap.
    waiting: Vec<ActorId>,
    /// Every pedestrian with a path in use, and how far along it they are (`None` while
    /// waiting at the kerb).
    progress: BTreeMap<ActorId, Option<f64>>,
}

/// How far short of a lane's edge a pedestrian crossing mid-block waits when that lane is
/// not clear, metres.
const MIDBLOCK_HOLD_MARGIN_M: f64 = 0.3;

/// How far ahead of a crossing pedestrian a lane counts as occupied, metres: about a
/// second and a half of walking, so a driver is stopping before the pedestrian reaches the
/// lane. The rolling-gap test lets a vehicle go first only if its rear is past before the
/// pedestrian is this close, so no driver it let through meets the lane occupied.
const MIDBLOCK_LOOKAHEAD_M: f64 = 2.0;

impl MidblockBands {
    fn of(vru: Option<&SocialForce>) -> Self {
        let mut out = MidblockBands::default();
        let Some(vru) = vru else { return out };
        for (ped, m) in vru.midblock_paths() {
            let progress = match m.stage {
                crate::vru::social_force::MidblockStage::Crossing(d) => Some(d),
                crate::vru::social_force::MidblockStage::Waiting(_) => {
                    out.waiting.push(ped);
                    None
                }
                crate::vru::social_force::MidblockStage::Approaching => continue,
            };
            out.progress.insert(ped, progress);
            for (i, b) in m.bands.iter().enumerate() {
                let id = crate::vru::midblock::DYNAMIC_BASE + ped.as_usize() * 16 + i.min(15);
                let mut c = b.conflict;
                c.crosswalk = id;
                out.by_lane.entry(c.lane).or_default().push(c);
                out.owner.insert(id, (ped, i));
                out.reach.insert(id, (b.d_near_m, b.d_far_m));
                if let Some(d) = progress
                    && d + MIDBLOCK_LOOKAHEAD_M >= b.d_near_m
                    && d <= b.d_far_m + 0.5
                {
                    out.occupied.insert(id);
                }
            }
        }
        for v in out.by_lane.values_mut() {
            v.sort_by(|a, b| a.s_m.total_cmp(&b.s_m).then(a.crosswalk.cmp(&b.crosswalk)));
        }
        out
    }

    fn is_empty(&self) -> bool {
        self.by_lane.is_empty()
    }

    /// The gap to the nearest band ahead that this vehicle must stop for: one a pedestrian
    /// is in or about to enter, or one whose waiting pedestrian this driver yields to.
    fn stop_gap(
        &self,
        bands: &[crate::vru::crosswalk::BandAhead],
        vehicle: ActorId,
        yields: &BTreeMap<(ActorId, ActorId), bool>,
    ) -> Option<f64> {
        bands
            .iter()
            .find(|b| {
                b.enter_m > 0.0
                    && (self.occupied.contains(&b.crosswalk)
                        || self.owner.get(&b.crosswalk).is_some_and(|(ped, _)| {
                            // A driver who yielded holds until the pedestrian is past
                            // their lane.
                            let (_, d_far) = self.reach[&b.crosswalk];
                            yields.get(&(*ped, vehicle)).copied() == Some(true)
                                && self
                                    .progress
                                    .get(ped)
                                    .copied()
                                    .flatten()
                                    .is_none_or(|d| d <= d_far + 0.5)
                        }))
            })
            .map(|b| (b.enter_m - crate::vru::crosswalk::STOP_BEFORE_CROSSWALK_M).max(0.0))
    }
}

/// A point `d_m` to the left of `lane`'s centreline at arc length `s_m`, with the normal
/// blended across each vertex instead of switching at it.
///
/// [`v2xw_world::Lane::offset_point`] takes the normal of the segment `s_m` falls on, so
/// an offset point jumps by about `d·Δψ` as it passes a vertex where the polyline turns
/// by `Δψ` — 0.4 m for a lane change 3 m out on a curved Manhattan lane, which the
/// auditor saw as a teleport. Within `r` of a vertex (a metre, or half the shorter of
/// its two segments) the heading the normal is taken from is interpolated linearly from
/// one segment's to the next, so the offset curve is continuous. On the centreline
/// (`d = 0`) it is exactly `point_at`.
pub fn smooth_offset_point(lane: &v2xw_world::Lane, s_m: f64, d_m: f64) -> Vec3 {
    let base = lane.point_at(s_m);
    if d_m == 0.0 {
        return base;
    }
    let n = lane.centreline.len();
    let seg_heading = |i: usize| {
        let (a, b) = lane.segment(i.min(n.saturating_sub(2)));
        math::atan2(b.y - a.y, b.x - a.x)
    };
    let i = lane.segment_at(s_m);
    let mut heading = seg_heading(i);
    let seg_len = |k: usize| lane.cumulative[k + 1] - lane.cumulative[k];
    // The vertex at the start of segment i, and the one at its end.
    if i > 0 {
        let r = (0.5 * seg_len(i).min(seg_len(i - 1))).min(1.0);
        let from_vertex = s_m - lane.cumulative[i];
        if r > 0.0 && from_vertex < r {
            let prev = seg_heading(i - 1);
            let turn = v2xw_world::model::normalise_angle(heading - prev);
            heading = prev + turn * (0.5 + 0.5 * from_vertex / r);
        }
    }
    if i + 2 < n {
        let r = (0.5 * seg_len(i).min(seg_len(i + 1))).min(1.0);
        let to_vertex = lane.cumulative[i + 1] - s_m;
        if r > 0.0 && to_vertex < r {
            let next = seg_heading(i + 1);
            let turn = v2xw_world::model::normalise_angle(next - heading);
            heading += turn * (0.5 - 0.5 * to_vertex / r);
        }
    }
    let (sin_h, cos_h) = math::sin_cos(heading);
    Vec3::new(base.x - sin_h * d_m, base.y + cos_h * d_m, base.z)
}

/// The speed each junction connector may be driven at: `sqrt(a_lat · R)` with `R` the
/// connector's average radius (length over total heading change), which is how SUMO's
/// `netconvert --junctions.limit-turn-speed` limits a turn. A connector that turns less
/// than 0.1 rad is not limited.
/// The router a vehicle of `class` plans with (see [`NativeMobility::router_for`]); a free
/// function so the integration pass can call it while it holds an actor mutably.
fn router_of<'r>(
    general: &'r DynamicReroute,
    bike: &'r DynamicReroute,
    by_class: &'r BTreeMap<u16, DynamicReroute>,
    class: VehicleClass,
) -> &'r DynamicReroute {
    if class == VehicleClass::Bicycle {
        bike
    } else {
        by_class.get(&class.class_mask().bits()).unwrap_or(general)
    }
}

/// How far apart in heading two lanes may be for a lateral lane change between them,
/// radians (10°). **This crate's choice**: SUMO's netconvert builds lanes of one edge
/// parallel by construction, and a change is only offered between them; an imported
/// lane that bends round a corner its neighbour does not is not a lane to slide into.
const LANE_CHANGE_PARALLEL_RAD: f64 = 10.0 * std::f64::consts::PI / 180.0;

/// How far ahead of the vehicle the two lanes must stay parallel, metres: the length a
/// lateral transition of a few seconds covers at city speed. **This crate's choice.**
const LANE_CHANGE_PARALLEL_LOOK_M: f64 = 15.0;

/// Whether `from` and `to` run side by side from the vehicle's position for
/// [`LANE_CHANGE_PARALLEL_LOOK_M`]: heading within [`LANE_CHANGE_PARALLEL_RAD`] at both
/// ends, with `to` sampled where the point on `from` projects onto it.
fn lanes_parallel_ahead(world: &World, actor: &Actor, from: LaneId, to: LaneId) -> bool {
    let (Some(a), Some(b)) = (world.try_lane(from), world.try_lane(to)) else {
        return true;
    };
    let s0 = actor.s_m.clamp(0.0, a.length_m);
    let s1 = (actor.s_m + LANE_CHANGE_PARALLEL_LOOK_M).clamp(0.0, a.length_m);
    [s0, s1].into_iter().all(|s| {
        let p = b.project_point(a.point_at(s));
        let d = v2xw_world::model::normalise_angle(a.heading_at(s) - b.heading_at(p.s_m));
        d.abs() <= LANE_CHANGE_PARALLEL_RAD
    })
}

/// The bends of every motorised through lane whose turn speed is under the lane's limit:
/// for each interior vertex, the heading change summed over the `window_m` of polyline
/// from it gives the radius `R = window / Σθ` of the tightest arc there, driven at
/// `sqrt(a_lat · R)` at most ([`EngineParams::lane_bend_speed`]). Each bend holds from its
/// first vertex to the last vertex inside the window, so a vehicle anywhere on the kink
/// is held to the bend's speed.
fn bend_speeds(
    world: &World,
    a_lat_mps2: f64,
    window_m: f64,
) -> BTreeMap<LaneId, Vec<(f64, f64, f64)>> {
    let mut out = BTreeMap::new();
    if a_lat_mps2 <= 0.0 || window_m <= 0.0 {
        return out;
    }
    for lane in world.roads.lanes() {
        if lane.kind == LaneKind::Internal || !lane.kind.is_motorised() || lane.length_m <= 0.0 {
            continue;
        }
        let mut bends: Vec<(f64, f64)> = Vec::new();
        for i in 0..lane.centreline.len().saturating_sub(2) {
            let (a, b) = lane.segment(i);
            let (_, c) = lane.segment(i + 1);
            let h0 = math::atan2(b.y - a.y, b.x - a.x);
            let h1 = math::atan2(c.y - b.y, c.x - b.x);
            let at = lane.cumulative.get(i + 1).copied().unwrap_or(0.0);
            bends.push((at, v2xw_world::model::normalise_angle(h1 - h0).abs()));
        }
        let mut zones = Vec::new();
        for (k, &(s0, _)) in bends.iter().enumerate() {
            let inside: Vec<&(f64, f64)> = bends[k..]
                .iter()
                .take_while(|b| b.0 - s0 <= window_m + 1e-9)
                .collect();
            let sum: f64 = inside.iter().map(|b| b.1).sum();
            if sum <= 1e-6 {
                continue;
            }
            let v = math::sqrt(a_lat_mps2 * window_m / sum);
            if v < lane.speed_limit_mps {
                let s1 = inside.last().map_or(s0, |b| b.0);
                zones.push((s0, s1, v));
            }
        }
        if !zones.is_empty() {
            out.insert(lane.id, zones);
        }
    }
    out
}

fn turn_speeds(world: &World, a_lat_mps2: f64, window_m: f64) -> BTreeMap<LaneId, f64> {
    let mut out = BTreeMap::new();
    for lane in world.roads.lanes() {
        if lane.kind != LaneKind::Internal || lane.length_m <= 0.0 {
            continue;
        }
        // The heading change at each interior vertex, and where along the lane it is.
        let mut bends: Vec<(f64, f64)> = Vec::new();
        for i in 0..lane.centreline.len().saturating_sub(2) {
            let (a, b) = lane.segment(i);
            let (_, c) = lane.segment(i + 1);
            let h0 = math::atan2(b.y - a.y, b.x - a.x);
            let h1 = math::atan2(c.y - b.y, c.x - b.x);
            let at = lane.cumulative.get(i + 1).copied().unwrap_or(0.0);
            bends.push((at, v2xw_world::model::normalise_angle(h1 - h0).abs()));
        }
        let turned: f64 = bends.iter().map(|b| b.1).sum();
        if turned < 0.1 {
            continue;
        }
        let mut radius = lane.length_m / turned;
        if window_m > 0.0 {
            // The tightest arc: the most heading change within any window of `window_m`
            // starting at a vertex (the whole lane when it is shorter than the window).
            let w = window_m.min(lane.length_m);
            for (k, &(s0, _)) in bends.iter().enumerate() {
                let sum: f64 = bends[k..]
                    .iter()
                    .take_while(|b| b.0 - s0 <= w + 1e-9)
                    .map(|b| b.1)
                    .sum();
                if sum > 1e-6 {
                    radius = radius.min(w / sum);
                }
            }
        }
        out.insert(
            lane.id,
            math::sqrt(a_lat_mps2 * radius).min(lane.speed_limit_mps),
        );
    }
    out
}

/// What other vehicles at a junction impose on one approaching or inside it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct JunctionConstraint {
    /// A crossing to wait at, as a virtual obstacle.
    crossing: Option<LeaderView>,
    /// The partner ahead in a merge onto the same exit, the tightest one.
    merge: Option<MergePartner>,
}

/// A vehicle on another connector that reaches the shared merge point first.
#[derive(Debug, Clone, Copy, PartialEq)]
struct MergePartner {
    /// The partner.
    view: VehicleView,
    /// Its rear's distance ahead of the ego's front once both are projected onto the
    /// merge point, metres — negative while the two are side by side.
    gap_m: f64,
    /// The ego's front's distance to the merge point, metres.
    d_self_m: f64,
}

/// The acceleration that has the ego fall in behind a merge partner by the time it
/// reaches the merge point.
///
/// The partner's projected gap is where it will be relative to the ego *at the merge
/// point*, `τ` seconds ahead (the ego's own [`merge_arrival_s`]). Holding both speeds, the
/// gap there would be `g + (v_p − v)·τ`; it has to be at least `s_req`, the gap at which
/// the car-following model, at equal speeds, brakes at no more than the driver's
/// comfortable deceleration `b`: from the IDM, `(s0 + v_p·T) / sqrt(1 + b/a)`. What is
/// missing is made up at the constant deceleration that loses it over `τ`:
/// `a = 2·(g + (v_p − v)·τ − s_req) / τ²` — and nothing is imposed when nothing is
/// missing. Feeding the projected gap to the car-following model instead, as this engine
/// did, braked a car at −6 m/s² for a partner exactly beside it at the same speed 40 m
/// before the merge, because the model reads a gap as a body at the bumper now (the
/// auditor's jerk class: 23 of the 30 classified onsets on dense Midtown). **This crate's
/// rule**, from the IDM's own parameters; near the merge point `τ` is small and it becomes
/// the emergency brake it should be.
fn merge_accel(ego: &VehicleView, m: &MergePartner, tau_s: f64) -> f64 {
    let d = &ego.driver;
    let v = ego.speed_mps;
    let v_p = m.view.speed_mps;
    let s_req = (d.min_gap_m + v_p * d.time_headway_s)
        / math::sqrt(1.0 + d.comfort_decel_mps2 / d.max_accel_mps2.max(0.1));
    let tau = tau_s.max(0.3);
    // A partner that is braking keeps braking; one accelerating is not counted on.
    let a_p = m.view.accel_mps2.min(0.0);
    let slack = m.gap_m + (v_p - v) * tau + 0.5 * a_p * tau * tau - s_req;
    if slack >= 0.0 {
        return f64::INFINITY;
    }
    (2.0 * slack / (tau * tau)).max(crate::carfollowing::idm::LEGACY_A_MIN_MPS2)
}

/// When a vehicle `d_m` from a merge point, doing `v_mps`, reaches it if it accelerates
/// at `a_mps2` up to `cap_mps` and holds that: the arrival order a merge is settled by.
/// A vehicle at the merge point already arrives at 0; one standing arrives when its launch
/// would bring it there.
pub fn merge_arrival_s(d_m: f64, v_mps: f64, a_mps2: f64, cap_mps: f64) -> f64 {
    if d_m <= 0.0 {
        return 0.0;
    }
    let a = a_mps2.max(0.1);
    let v = v_mps.max(0.0);
    let cap = cap_mps.max(v).max(0.1);
    let t_acc = (cap - v) / a;
    let d_acc = 0.5 * (v + cap) * t_acc;
    if d_acc >= d_m {
        (-v + math::sqrt(v * v + 2.0 * a * d_m)) / a
    } else {
        t_acc + (d_m - d_acc) / cap
    }
}

/// The acceleration towards a *standing* virtual obstacle (a stop line, a give-way line)
/// `gap_m` ahead: the car-following model's, unless that brakes harder than stopping
/// `s0` short of the obstacle needs, in which case the kinematic deceleration
/// `v² / (2·(gap − s0))` — the constant-acceleration heuristic of Kesting, Treiber and
/// Helbing 2010 for a standing obstacle.
///
/// The IDM's interaction term reacts to an obstacle that *appears* — a light turning amber
/// ahead of a vehicle that can still stop — with its full −6 m/s² floor in one step, which
/// is neither what a driver does nor smooth. A driver who has decided to stop brakes just
/// hard enough to stop at the line; that is what this returns. It never brakes *less*
/// than stopping needs, so it cannot carry a vehicle over the line.
///
/// `slack_m` is how far past the obstacle the vehicle may still stop without crossing
/// anything: for a stop line's virtual obstacle, the distance from it to just short of the
/// line — negative, since that obstacle stands beyond the line ([`STOP_LINE_MARGIN_M`]).
/// When stopping
/// `s0` short of the obstacle would take more than a planned stop's deceleration
/// ([`PLANNED_STOP_MAX_DECEL_MPS2`]), the driver uses the room up to the line itself —
/// the amber rule decided "stop" by the distance to the *line* (ITE), so the stop it
/// decided on is one the vehicle can make there.
pub fn static_obstacle_accel(
    a_model: f64,
    speed_mps: f64,
    gap_m: f64,
    s0_m: f64,
    slack_m: f64,
) -> f64 {
    if speed_mps <= 0.0 {
        return a_model;
    }
    let need = |room: f64| -(speed_mps * speed_mps) / (2.0 * room);
    let room = gap_m - s0_m;
    let to_line = gap_m + slack_m;
    // Whatever the car-following model asks, never less braking than stops the vehicle
    // short of the line itself. The IDM's braking scales with its `a`, and a heavy
    // vehicle's (0.7 m/s²) creeping up to the line asked for −0.34 m/s² at 0.6 m/s with
    // 0.35 m left: buses and trucks rolled over the stop line on red at walking pace, and
    // the cross traffic's green then met them in the junction.
    //
    // The floor binds only once stopping at the line needs real braking
    // (`STOP_FLOOR_BIND_MPS2`): it is a bound on speed, as SUMO's stop speed is
    // (`MSCFModel::maximumSafeStopSpeed`, src/microsim/cfmodels/MSCFModel.cpp: the speed
    // from which the vehicle can still stop in the gap), not an order to brake. Bound
    // always, it held every vehicle that moved off with a red line far ahead at the speed
    // of its first step: −v²/2d is below zero for any v, so nothing could accelerate,
    // and cars crawled at 0.17 m/s for the whole red, 65 m short of the line (Midtown,
    // check-traffic's phantom stop, actor 9 after a kerb stop). Binding from a small
    // deceleration on is as safe: the floor then tracks `need`, a constant deceleration
    // that ends at the line.
    let at_least = |a: f64| {
        if to_line > 0.05 && -need(to_line) >= STOP_FLOOR_BIND_MPS2 {
            a.min(need(to_line))
        } else {
            a
        }
    };
    if room > 0.05 && -need(room) <= PLANNED_STOP_MAX_DECEL_MPS2 {
        return at_least(a_model.max(need(room)));
    }
    if to_line <= 0.05 {
        return a_model;
    }
    // Already inside its standstill gap of the obstacle and slow: the driver stops, at no
    // more than a planned stop's deceleration. Easing in at exactly `v²/2d` instead never
    // quite arrived: at 0.2 m/s, 2.5 m out, it took 25 s, the car creeping at a few
    // centimetres a second on a green behind a car that had moved on (Midtown, actor 41).
    if room <= 0.05 && -need(to_line) <= PLANNED_STOP_MAX_DECEL_MPS2 {
        return at_least(
            a_model
                .max(-PLANNED_STOP_MAX_DECEL_MPS2)
                .max(need(to_line).min(-0.5)),
        );
    }
    let tight = if room > 0.05 {
        need(room)
    } else {
        f64::NEG_INFINITY
    };
    at_least(a_model.max(tight.max(need(to_line))))
}

/// The model card.
pub fn card(params: &EngineParams, car_following: &str) -> ModelCard {
    let src = Source {
        kind: SourceKind::Code,
        reference: "04-models.md §2 (the native tiers) and 03-interfaces.md §3 (the \
                    `Mobility` trait and its invariants I-M1, I-M2, I-M4)"
            .to_string(),
        accessed: Some("2026-09-18".to_string()),
        note: None,
    };
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        MODEL_VERSION,
        "The native mobility engine: it freezes every actor's state at the start of each \
         step, asks the car-following, lane-change and intersection models for one decision \
         per actor from that frozen state, integrates them all together, and publishes \
         `Kinematics` for every actor with the spawn, despawn and signal-state lists. The \
         freeze is what makes the update a Jacobi update and therefore independent of the \
         order the actors are visited in (ADR 0004).",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium];
    card.equations = vec![
        v2xw_core::card::Equation {
            name: "integration".to_string(),
            latex_or_text: "v ← clamp(v + a·Δt, 0, v0);  s ← s + v·Δt".to_string(),
            notes: Some(
                "the *new* speed carries the step, which is the legacy engine's order \
                 [run.py L2493-2495] and what keeps a decelerating vehicle from rolling \
                 through a stop line"
                    .to_string(),
            ),
        },
        v2xw_core::card::Equation {
            name: "published position".to_string(),
            latex_or_text: "pos = point on the driven path at s − length (walked back over the \
                            lanes behind);  heading = atan2(front − rear)"
                .to_string(),
            notes: Some(
                "`s` is the front bumper and `Kinematics::pos` is the rear-axle centre; the \
                 class table gives no wheelbase, so the rear bumper stands in for the rear \
                 axle. The heading is the body's, from rear to front, so it turns \
                 continuously through a junction. The legacy parity mode keeps the \
                 reference engine's placement, clamped to the front's lane"
                    .to_string(),
            ),
        },
        v2xw_core::card::Equation {
            name: "stop-line braking".to_string(),
            latex_or_text: "a = max(a_IDM, −v²/(2·(s − s0)))  for a standing virtual obstacle"
                .to_string(),
            notes: Some(STOP_LINE_ONSET_NOTE.to_string()),
        },
        v2xw_core::card::Equation {
            name: "turn speed".to_string(),
            latex_or_text: "v_turn = min(v_limit, sqrt(a_lat · L / |Δψ|))".to_string(),
            notes: Some(
                "a junction connector of length L turning through Δψ, driven at an average \
                 lateral acceleration a_lat = 5.5 m/s² (SUMO netconvert \
                 --junctions.limit-turn-speed default)"
                    .to_string(),
            ),
        },
        v2xw_core::card::Equation {
            name: "anticipation".to_string(),
            latex_or_text: "v0 ← min(v0, sqrt(v_next² + 2·b·d))".to_string(),
            notes: Some(
                "for each lane ahead on the route within the lookahead, d metres away with \
                 limit (or turn speed) v_next: the vehicle slows at its comfortable \
                 deceleration b before the boundary instead of having its speed cut at it"
                    .to_string(),
            ),
        },
    ];
    card.parameters = vec![
        Parameter::new(
            "step",
            "s",
            serde_json::json!(params.step.as_secs_f64()),
            Source::new(
                SourceKind::Standard,
                "ADR 0004 decision 2: mobility step default 100 ms, range 10-100 ms",
            ),
        ),
        Parameter::new(
            "car_following",
            "-",
            serde_json::json!(car_following),
            src.clone(),
        ),
        Parameter::new(
            "intersections",
            "-",
            serde_json::json!(format!("{:?}", params.intersections)),
            src.clone(),
        ),
        Parameter::new(
            "lane_changes",
            "-",
            serde_json::json!(params.lane_changes),
            src.clone(),
        ),
        Parameter::new(
            "lookahead",
            "m",
            serde_json::json!(params.lookahead_m),
            Source::new(
                SourceKind::Code,
                "legacy/scms_sim_ref/mock_pipeline/run.py: `idm_lookahead_m` 70 m",
            ),
        ),
        Parameter::new(
            "classes",
            "-",
            serde_json::json!(params.classes.names()),
            src.clone(),
        ),
        Parameter::new(
            "insertion_gap",
            "m",
            serde_json::json!(params.insertion_gap_m),
            Source::new(
                SourceKind::Code,
                "this crate: a trip whose origin is occupied is dropped rather than \
                 overlapped, and the drop is counted (`dropped_trips`)",
            ),
        ),
        Parameter::new(
            "stop_line_offset",
            "m",
            serde_json::json!(STOP_LINE_OFFSET_M),
            Source::new(
                SourceKind::Code,
                "legacy/scms_sim_ref/mock_pipeline/run.py L2467",
            ),
        ),
        Parameter::new(
            "dynamic_rerouting",
            "-",
            serde_json::json!(params.dynamic_rerouting),
            src.clone(),
        ),
        Parameter::new(
            "no_change_zone",
            "m",
            serde_json::json!(params.no_change_zone_m),
            Source {
                kind: SourceKind::Standard,
                reference: "MUTCD 2009 §3B.04: a solid lane line where crossing it is \
                            discouraged, as on the approach to a junction"
                    .to_string(),
                accessed: Some("2026-09-23".to_string()),
                note: Some(
                    "the manual does not fix the length; 30 m is this crate's choice — the \
                     2.5 s transition at 25 mph (28 m) plus margin — and the zone is \
                     stretched to the vehicle's own speed × transition time when longer"
                        .to_string(),
                ),
            },
        ),
        Parameter::new(
            "turn_lateral_accel",
            "m/s²",
            serde_json::json!(params.turn_lateral_accel_mps2),
            Source::new(
                SourceKind::Code,
                "SUMO netconvert --junctions.limit-turn-speed, default 5.5 (\"limits speed \
                 on junctions to an average lateral acceleration of at most FLOAT m/s^2\")",
            ),
        ),
        Parameter::new(
            "junction_clearance",
            "-",
            serde_json::json!(params.junction_clearance),
            Source::new(
                SourceKind::Standard,
                "UVC §11-202(a)1 / NY VTL §1111(a)1: a driver facing a green shall yield to \
                 vehicles lawfully within the intersection; NY VTL §1175: no driver shall \
                 enter an intersection unless there is space on the opposite side",
            ),
        ),
        Parameter::new(
            "kerbside",
            "s, m, 1",
            serde_json::json!(params.kerbside),
            Source::new(
                SourceKind::Standard,
                "crate::kerbside: bus dwell TCRP Report 100 (TCQSM 2nd ed.) Part 4, 60 s \
                 downtown default, c_v 0.6 (secondary); stop spacing, taxi share and dwell, \
                 delivery share and dwell are choices recorded on the module",
            ),
        ),
        Parameter::new(
            "driver_styles",
            "-",
            serde_json::json!(params.driver_styles),
            Source::new(
                SourceKind::Paper,
                "crate::style: cautious / normal / assertive below the 15th, between, and \
                 above the 85th percentile of the free-speed draw; faster drivers follow \
                 closer (Evans & Wasielewski 1982, 1983; secondary); multipliers a choice \
                 that keeps the fleet means",
            ),
        ),
        Parameter::new(
            "heavy_turns",
            "-",
            serde_json::json!(params.heavy_turns),
            Source::new(
                SourceKind::Standard,
                "AASHTO Green Book 2018 §2.1.3: minimum turning paths drawn for 10 mph or \
                 less (secondary)",
            ),
        ),
        Parameter::new(
            "class_routing",
            "-",
            serde_json::json!(params.class_routing),
            Source::new(
                SourceKind::Standard,
                "the world's per-lane class masks (a bus lane admits buses and emergency \
                 vehicles, NYC Traffic Rules §4-12(m)); each class is routed on its own",
            ),
        ),
        Parameter::new(
            "reverse_order",
            "-",
            serde_json::json!(params.reverse_order),
            Source::new(
                SourceKind::Code,
                "a test hook, not a model parameter: it walks the decision pass backwards \
                 so the Jacobi property can be asserted (ADR 0004)",
            ),
        ),
    ];
    card.assumptions = vec![
        "Every decision in a step is a function of the start-of-step state of every actor \
         (the Jacobi update, ADR 0004)."
            .to_string(),
        "Actor ids are assigned in demand-stream order (invariant I-M2), and every output \
         list is ordered by actor id (I-M1)."
            .to_string(),
        "Between steps, consumers extrapolate at constant velocity (02-architecture.md \
         §5.2, invariant I-M4)."
            .to_string(),
        "A red signal, a junction to give way at and a curve-speed cap all reach the \
         vehicle as a virtual leader, so one equation produces every deceleration."
            .to_string(),
        "The published reference point is the rear bumper, standing in for the rear-axle \
         centre."
            .to_string(),
        "Outside the legacy parity mode: a lane change is refused inside the no-change zone \
         before a junction, while the body still overhangs the previous lane, when the \
         target lane cannot make the route's next movement, and when another vehicle has \
         already taken the same gap this step (actor-id order); the route is shifted onto \
         the new lane movement by movement."
            .to_string(),
        "Outside the legacy parity mode: a vehicle holds at the stop line while a vehicle \
         already inside the junction on a crossing movement has not cleared the shared \
         conflict zone, or while its exit has no room behind a standing queue; inside the \
         junction it gives way at a crossing to a vehicle that reaches it first, and \
         follows one merging ahead onto the same exit."
            .to_string(),
        "A vehicle that chose to go on amber is committed to clearing the junction and does \
         not slow for the turn ahead until it has entered."
            .to_string(),
        "Vulnerable road users (VruPopulation): pedestrians are placed on a random walkable \
         lane and walk a random chain of connected walkable lanes with the social-force \
         model; cyclists ride between random bicycle-admitting lanes on the bicycle lane \
         graph with the car-following model and the SUMO bicycle vType (20 km/h, 1.2 m/s², \
         3.0 m/s², 0.5 m, tau 1 s). Each one that finishes is replaced."
            .to_string(),
        "A trip whose origin has no room waits, oldest first, up to 120 s (SUMO's insertion \
         queue), holding the actor id it was given when it first asked."
            .to_string(),
    ];
    card.limitations = vec![
        "A lane change moves the vehicle into the target lane's frame at the *start* of \
         the transition and slides the lateral offset to zero over it; the vehicle is \
         therefore in the target lane for the whole transition, which is the legacy \
         engine's behaviour and not a sublane model."
            .to_string(),
        "A trip whose origin is occupied is dropped rather than queued.".to_string(),
    ];
    card.ignores = vec![
        "Everything the SUMO tier does that the native models do not: driver imperfection, \
         action step length, cooperative lane changes, the junction `request` matrices, \
         pedestrian striping (04-models.md §2.8)."
            .to_string(),
    ];
    card.sources = vec![src];
    card.determinism = Determinism {
        uses_rng: true,
        rng_domains: vec![
            v2xw_core::rng::RngDomain::Spawn.as_str().to_string(),
            v2xw_core::rng::RngDomain::LaneChange.as_str().to_string(),
            v2xw_core::rng::RngDomain::DesiredSpeed.as_str().to_string(),
        ],
    };
    // Listed only when set, so a card of the default engine is unchanged.
    if params.depart_at_speed {
        card.parameters.push(Parameter::new(
            "depart_at_speed",
            "-",
            serde_json::json!(true),
            Source::new(
                SourceKind::Code,
                "Re-implemented from the SUMO user documentation (Definition of Vehicles: \
                 departSpeed=\"desired\"/\"max\", departPos=\"base\"); SUMO was read for \
                 behaviour only and no code was copied. Boundary entries only; the \
                 boundary band (30 m) is this crate's choice",
            ),
        ));
    }
    if params.reroute_share < 1.0 || params.reroute_source != RerouteSource::Navigation {
        card.parameters.push(Parameter::new(
            "reroute_share",
            "1",
            serde_json::json!(params.reroute_share),
            Source::new(
                SourceKind::Code,
                "Re-implemented from the SUMO user documentation (Rerouting device: \
                 device.rerouting.probability); SUMO was read for behaviour only and no \
                 code was copied",
            ),
        ));
        card.parameters.push(Parameter::new(
            "reroute_source",
            "-",
            serde_json::json!(params.reroute_source.as_str()),
            Source::new(
                SourceKind::Code,
                "this crate's choice (sumo.md P14): a navigation app sees the whole \
                 network on 3-minute smoothed speeds; the V2X source re-routes only on a \
                 hazard warning the driver's own unit received and verified",
            ),
        ));
    }
    if params.turn_curve_window_m > 0.0 {
        card.parameters.push(Parameter::new(
            "turn_curve_window",
            "m",
            serde_json::json!(params.turn_curve_window_m),
            Source::new(
                SourceKind::Code,
                "this crate's choice: the turn speed is set by the tightest arc of this \
                 length, where SUMO netconvert averages the radius over the connector",
            ),
        ));
    }
    if let Some(lc) = params.lc2013 {
        for (name, v) in [
            ("lc_strategic", lc.strategic),
            ("lc_cooperative", lc.cooperative),
            ("lc_speed_gain", lc.speed_gain),
            ("lc_keep_right", lc.keep_right),
        ] {
            card.parameters.push(Parameter::new(
                name,
                "1",
                serde_json::json!(v),
                Source::new(
                    SourceKind::Paper,
                    "Erdmann 2015, SUMO's Lane-Changing Model, doi:10.1007/978-3-319-15024-6_7; \
                     eagerness factors lcStrategic / lcCooperative / lcSpeedGain / \
                     lcKeepRight, default 1 per SUMO's vType documentation",
                ),
            ));
        }
    }
    card.validation = Validation {
        status: ValidationStatus::LiteratureChecked,
        references: vec![Source::new(
            SourceKind::Paper,
            "04-models.md §2.9 fundamental-diagram targets [R10 §B15]",
        )],
        tests: vec![
            "engine::tests::the_jacobi_update_is_order_independent".to_string(),
            "engine::tests::a_vehicle_stops_at_red_and_goes_on_green".to_string(),
            "fd::tests::the_ring_reproduces_the_fundamental_diagram_targets".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::MobilityCtx;
    use crate::demand::poisson::{PoissonDemand, PoissonParams};
    use crate::demand::{NoDemand, OdParams};
    use crate::views::TripRequest;
    use crate::worlds::{RingParams, cycle_length_m, ring, ring_cycle};
    use v2xw_core::model::Model;
    use v2xw_core::rng::RngRegistry;
    use v2xw_core::time::{NS_PER_MS, NS_PER_S};
    use v2xw_world::{ImportOptions, procedural::GridParams};

    fn grid(signalised: bool) -> World {
        v2xw_world::procedural::grid(
            &GridParams::legacy().with_signals(signalised),
            &ImportOptions::default(),
        )
        .expect("grid")
    }

    fn engine_on(world: &World, params: EngineParams, rng: &RngRegistry) -> NativeMobility {
        let mut engine = NativeMobility::new(params);
        let mut ctx = MobilityCtx::new(0, world, rng);
        engine
            .init(&mut ctx, Box::new(NoDemand::new()))
            .expect("init");
        engine
    }

    /// An approach lane whose junction is signalised, with the lane beyond it.
    fn signalised_approach(world: &World) -> (LaneId, LaneId) {
        for lane in world.roads.lanes() {
            if lane.kind != LaneKind::Driving {
                continue;
            }
            let junction = world.edge(lane.edge).to;
            let Some(j) = world.roads.try_junction(junction) else {
                continue;
            };
            if !matches!(j.control, JunctionControl::Signalised { .. }) {
                continue;
            }
            // A straight-on movement through it.
            if let Some(c) = world
                .successors(lane.id)
                .iter()
                .find(|c| c.permitted && c.via.is_some() && c.direction == TurnDirection::Straight)
            {
                return (lane.id, c.to_lane);
            }
        }
        panic!("the signalised grid has no signalised straight-on movement");
    }

    #[test]
    fn a_vehicle_stops_at_red_and_goes_on_green() {
        let world = grid(true);
        let (approach, beyond) = signalised_approach(&world);
        let rng = RngRegistry::new(4);
        let params = EngineParams {
            intersections: IntersectionMode::SignalsOnly,
            lane_changes: false,
            ..EngineParams::default()
        };
        let signals = FixedTimeSignals::default();
        let junction = world.edge(world.lane(approach).edge).to;
        let plan_id = match world.roads.junction(junction).control {
            JunctionControl::Signalised { plan } => plan,
            _ => unreachable!("chosen above"),
        };
        let plan = world.signal_plan(plan_id).expect("a plan");
        let movement = world
            .successors(approach)
            .iter()
            .find(|c| c.to_lane == beyond)
            .and_then(|c| c.via)
            .expect("an internal connector");

        let mut ever_stopped_at_red = false;
        // Try a spawn every five seconds of the cycle: at least one arrival must meet a red
        // light, and no arrival may ever cross one.
        for offset_s in (0u64..60).step_by(5) {
            let start = offset_s * NS_PER_S;
            let mut engine = NativeMobility::new(params);
            {
                let mut ctx = MobilityCtx::new(start, &world, &rng);
                engine
                    .init(&mut ctx, Box::new(NoDemand::new()))
                    .expect("init");
                engine.command(
                    &mut ctx,
                    MobilityCommand::Spawn(TripRequest {
                        seq: 0,
                        t: start,
                        origin: approach,
                        origin_s_m: 5.0,
                        destination: beyond,
                        class: VehicleClass::Passenger,
                        desired_speed_mps: 13.89,
                    }),
                );
            }
            let approach_length = world.lane(approach).length_m;
            let mut previous: Option<(LaneId, SignalState)> = None;
            let mut stopped_here = false;
            let mut crossed = false;
            let mut t = start;
            while t < start + 90 * NS_PER_S {
                let mut ctx = MobilityCtx::new(t, &world, &rng);
                let update = engine.step(&mut ctx, params.step);
                let state = signals
                    .state_for(plan, movement, ns_to_secs(t))
                    .expect("the movement is controlled");
                if let Some((_, lane, s_m, speed)) = engine.longitudinal_states().first().copied() {
                    if lane == approach {
                        // Never past the stop line while red.
                        if !state.permits_entry() {
                            assert!(
                                s_m <= approach_length + 1e-6,
                                "crossed the stop line on {state:?}"
                            );
                            if s_m > approach_length - 6.0 && speed < 0.3 {
                                stopped_here = true;
                            }
                        }
                    } else if !crossed {
                        crossed = true;
                        // It left the approach lane: the light it left on must have
                        // permitted entry.
                        let (_, entering) = previous.expect("a previous step");
                        // Green, or an amber it was too close to stop for — the dilemma
                        // zone the yellow interval is sized for. Never a red.
                        assert!(
                            entering.permits_entry() || entering == SignalState::Amber,
                            "entered the junction on {entering:?}"
                        );
                        if stopped_here {
                            ever_stopped_at_red = true;
                            // And it is moving again: it stood half a metre short of the
                            // line (`STOP_LINE_MARGIN_M`), so it crosses a few tenths of a
                            // metre after moving off — slower than when stops ended 4 m back.
                            assert!(speed > 0.5, "it is going again: {speed} m/s");
                        }
                    }
                    previous = Some((lane, state));
                }
                if update
                    .despawned
                    .iter()
                    .any(|(_, c)| *c == DespawnCause::TripComplete)
                {
                    break;
                }
                t += params.step.as_nanos();
            }
            assert!(crossed, "the vehicle never crossed at offset {offset_s} s");
        }
        assert!(
            ever_stopped_at_red,
            "no arrival in a whole cycle ever met a red light"
        );
    }

    /// Drivers differ at the amber: across a cycle of arrivals 0.1 s apart, some who could still have
    /// stopped at 3 m/s² go on — inside the dilemma zone, where the field says drivers
    /// differ — and none of them ever meets the red. Without heterogeneity, nobody who could
    /// stop goes.
    #[test]
    fn drivers_differ_at_the_amber_and_none_meets_the_red() {
        let world = grid(true);
        let (approach, beyond) = signalised_approach(&world);
        let rng = RngRegistry::new(4);
        let signals = FixedTimeSignals::default();
        let junction = world.edge(world.lane(approach).edge).to;
        let plan_id = match world.roads.junction(junction).control {
            JunctionControl::Signalised { plan } => plan,
            _ => unreachable!("chosen above"),
        };
        let plan = world.signal_plan(plan_id).expect("a plan");
        let movement = world
            .successors(approach)
            .iter()
            .find(|c| c.to_lane == beyond)
            .and_then(|c| c.via)
            .expect("an internal connector");
        let len = world.lane(approach).length_m;
        let went_though_could_stop = |heterogeneity: bool| -> Vec<(u64, f64, f64)> {
            let params = EngineParams {
                intersections: IntersectionMode::SignalsOnly,
                lane_changes: false,
                driver_heterogeneity: heterogeneity,
                ..EngineParams::default()
            };
            let mut went = Vec::new();
            for k in 0u64..600 {
                let start = k * NS_PER_S / 10;
                let mut engine = NativeMobility::new(params);
                {
                    let mut ctx = MobilityCtx::new(start, &world, &rng);
                    engine
                        .init(&mut ctx, Box::new(NoDemand::new()))
                        .expect("init");
                    engine.command(
                        &mut ctx,
                        MobilityCommand::Spawn(TripRequest {
                            seq: k,
                            t: start,
                            origin: approach,
                            origin_s_m: 5.0,
                            destination: beyond,
                            class: VehicleClass::Passenger,
                            desired_speed_mps: 13.89,
                        }),
                    );
                }
                let mut onset: Option<(f64, f64)> = None;
                let mut prev: Option<(LaneId, f64, f64, SignalState)> = None;
                let mut t = start;
                while t < start + 60 * NS_PER_S {
                    let state = signals
                        .state_for(plan, movement, ns_to_secs(t))
                        .expect("controlled");
                    let mut ctx = MobilityCtx::new(t, &world, &rng);
                    engine.step(&mut ctx, params.step);
                    let Some((_, lane, s, v)) = engine.longitudinal_states().first().copied()
                    else {
                        break;
                    };
                    if let Some((plane, ps, pv, _)) = prev {
                        if plane == approach && state == SignalState::Amber && onset.is_none() {
                            onset = Some((len - ps, pv));
                        }
                        if plane == approach && lane != approach {
                            assert_ne!(
                                state,
                                SignalState::Red,
                                "arrival {k} entered on red (heterogeneity {heterogeneity})"
                            );
                            if state == SignalState::Amber
                                && let Some((gap, speed)) = onset
                                && speed * speed / 6.0 + 3.0 < gap
                            {
                                went.push((k, gap, speed));
                            }
                            break;
                        }
                        if state != SignalState::Amber {
                            onset = None;
                        }
                    }
                    prev = Some((lane, s, v, state));
                    t += params.step.as_nanos();
                }
            }
            went
        };
        let off = went_though_could_stop(false);
        assert!(off.is_empty(), "without heterogeneity: {off:?}");
        let on = went_though_could_stop(true);
        assert!(!on.is_empty());
        eprintln!("went on amber though they could stop: {on:?}");
    }

    #[test]
    fn the_jacobi_update_is_order_independent() {
        // The property ADR 0004 requires: the published states must not depend on the order
        // the actors are visited in. `reverse_order` walks the decision pass backwards; the
        // two runs must agree bit for bit, every step, for every actor.
        let world = ring(&RingParams {
            circumference_m: 600.0,
            segments: 6,
            ..RingParams::default()
        })
        .expect("a ring");
        let cycle = ring_cycle(&world, 0);
        let length = cycle_length_m(&world, &cycle);
        let run = |reverse: bool| {
            let rng = RngRegistry::new(2024);
            let params = EngineParams {
                intersections: IntersectionMode::None,
                lane_changes: false,
                reverse_order: reverse,
                dynamic_rerouting: false,
                ..EngineParams::default()
            };
            let mut engine = engine_on(&world, params, &rng);
            let route: Vec<LaneId> = cycle
                .iter()
                .cycle()
                .take(cycle.len() * 6)
                .copied()
                .collect();
            let driver = IdmPreset::Treiber2000.profile(VehicleClass::Passenger);
            let count = 24;
            for i in 0..count {
                let s = length * f64::from(i) / f64::from(count);
                let mut remaining = s;
                let mut index = 0usize;
                for (k, lane) in cycle.iter().enumerate() {
                    let l = world.lane(*lane).length_m;
                    if remaining <= l || k == cycle.len() - 1 {
                        index = k;
                        break;
                    }
                    remaining -= l;
                }
                let id = engine
                    .spawn_with_route(
                        &world,
                        0,
                        VehicleClass::Passenger,
                        driver,
                        route[index..].to_vec(),
                        remaining,
                    )
                    .expect("spawned");
                // A different starting speed per vehicle, so the run is not a symmetric
                // fixed point that would hide an order dependence.
                engine
                    .set_speed(id, 10.0 + f64::from(i % 7))
                    .expect("speed set");
            }
            let mut trace: Vec<Vec<(ActorId, Kinematics)>> = Vec::new();
            let mut t = 0u64;
            for _ in 0..300 {
                let mut ctx = MobilityCtx::new(t, &world, &rng);
                let update = engine.step(&mut ctx, EngineParams::default().step);
                trace.push(update.states);
                t += 100 * NS_PER_MS;
            }
            trace
        };
        let forward = run(false);
        let reverse = run(true);
        assert_eq!(forward.len(), reverse.len());
        for (step, (a, b)) in forward.iter().zip(&reverse).enumerate() {
            assert_eq!(
                a, b,
                "step {step}: the reverse-order pass produced different states"
            );
        }
        // And the run actually did something: the vehicles moved and interacted.
        let first = &forward[0];
        let last = forward.last().expect("steps");
        assert_eq!(first.len(), 24);
        assert!(
            last.iter().any(|(_, k)| k.speed_mps() > 1.0),
            "the run is not frozen"
        );
        assert!(
            last.iter().any(|(_, k)| k.speed_mps() < 25.0),
            "somebody is interacting"
        );
    }

    #[test]
    fn a_vehicle_yields_to_a_conflicting_claimant() {
        // Two vehicles on crossing arms of an unsignalised junction. The world's conflict
        // matrix gives one of them priority; the other must be the one that stops.
        let world = grid(false);
        let rng = RngRegistry::new(9);
        let params = EngineParams {
            intersections: IntersectionMode::GapAcceptanceOnly,
            lane_changes: false,
            ..EngineParams::default()
        };
        // Find a junction with two crossing approaches.
        let junction = world
            .roads
            .junctions()
            .iter()
            .find(|j| j.incoming.len() >= 4 && !j.internal.is_empty())
            .expect("a four-arm junction");
        let mut approaches: Vec<(LaneId, LaneId, LaneId)> = Vec::new();
        for lane in &junction.incoming {
            if let Some(c) = world
                .successors(*lane)
                .iter()
                .find(|c| c.permitted && c.via.is_some() && c.direction == TurnDirection::Straight)
            {
                approaches.push((*lane, c.via.expect("via"), c.to_lane));
            }
        }
        assert!(approaches.len() >= 2, "two straight movements");
        // Two whose movements actually conflict.
        let internal_index = |lane: LaneId| {
            junction
                .internal
                .iter()
                .position(|i| *i == lane)
                .expect("row")
        };
        let pair = approaches
            .iter()
            .enumerate()
            .flat_map(|(i, a)| approaches.iter().skip(i + 1).map(move |b| (a, b)))
            .find(|(a, b)| {
                junction
                    .conflicts
                    .is_foe(internal_index(a.1), internal_index(b.1))
            })
            .map(|(a, b)| (*a, *b))
            .expect("two conflicting movements");
        let (a, b) = pair;
        let a_yields = junction
            .conflicts
            .must_yield(internal_index(a.1), internal_index(b.1));
        let (yielding, priority) = if a_yields { (a, b) } else { (b, a) };

        let mut engine = engine_on(&world, params, &rng);
        let driver = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
        // Both start the same distance from the line, moving at the same speed, so the only
        // thing that can separate them is the right of way.
        let start_gap = 40.0;
        let ids: Vec<ActorId> = [yielding, priority]
            .iter()
            .map(|(approach, internal, beyond)| {
                let lane = world.lane(*approach);
                let id = engine
                    .spawn_with_route(
                        &world,
                        0,
                        VehicleClass::Passenger,
                        driver,
                        vec![*approach, *internal, *beyond],
                        lane.length_m - start_gap,
                    )
                    .expect("spawned");
                engine.set_speed(id, 10.0).expect("speed");
                id
            })
            .collect();
        let mut min_speed = [f64::INFINITY; 2];
        let mut t = 0u64;
        for _ in 0..80 {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            engine.step(&mut ctx, params.step);
            for (k, id) in ids.iter().enumerate() {
                if let Some(state) = engine
                    .longitudinal_states()
                    .iter()
                    .find(|(a, _, _, _)| a == id)
                {
                    min_speed[k] = min_speed[k].min(state.3);
                }
            }
            t += 100 * NS_PER_MS;
        }
        assert!(
            min_speed[0] < 0.7 * min_speed[1],
            "the yielding vehicle slowed to {} m/s while the priority one kept {} m/s",
            min_speed[0],
            min_speed[1]
        );
        assert!(
            min_speed[1] > 8.0,
            "the priority vehicle was barely slowed: {} m/s from 10 m/s",
            min_speed[1]
        );
        assert!(
            min_speed[0] < 6.0,
            "the yielding vehicle gave way from 10 m/s: {} m/s",
            min_speed[0]
        );
    }

    #[test]
    fn a_leader_is_followed_and_never_run_into() {
        let world = ring(&RingParams {
            circumference_m: 400.0,
            segments: 4,
            ..RingParams::default()
        })
        .expect("a ring");
        let cycle = ring_cycle(&world, 0);
        let rng = RngRegistry::new(3);
        let params = EngineParams {
            intersections: IntersectionMode::None,
            lane_changes: false,
            dynamic_rerouting: false,
            ..EngineParams::default()
        };
        let mut engine = engine_on(&world, params, &rng);
        let route: Vec<LaneId> = cycle
            .iter()
            .cycle()
            .take(cycle.len() * 20)
            .copied()
            .collect();
        let driver = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
        let leader = engine
            .spawn_with_route(
                &world,
                0,
                VehicleClass::Passenger,
                driver,
                route.clone(),
                60.0,
            )
            .expect("leader");
        let follower = engine
            .spawn_with_route(&world, 0, VehicleClass::Passenger, driver, route, 20.0)
            .expect("follower");
        engine.set_speed(leader, 5.0).expect("speed");
        engine.set_speed(follower, 20.0).expect("speed");
        let mut t = 0u64;
        let mut minimum_gap = f64::INFINITY;
        for _ in 0..600 {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            engine.step(&mut ctx, params.step);
            let states = engine.longitudinal_states();
            let l = states.iter().find(|(a, _, _, _)| *a == leader).copied();
            let f = states.iter().find(|(a, _, _, _)| *a == follower).copied();
            if let (Some(l), Some(f)) = (l, f)
                && l.1 == f.1
            {
                let gap = (l.2 - VehicleClass::Passenger.spec().length_m) - f.2;
                if gap > 0.0 {
                    minimum_gap = minimum_gap.min(gap);
                }
            }
            t += 100 * NS_PER_MS;
        }
        assert!(
            minimum_gap > 0.4,
            "the follower closed to {minimum_gap} m — the gap floor is 0.5 m"
        );
    }

    #[test]
    fn demand_spawns_and_trips_complete() {
        let world = grid(false);
        let rng = RngRegistry::new(11);
        let params = EngineParams {
            intersections: IntersectionMode::SignalsAndGapAcceptance,
            ..EngineParams::default()
        };
        let mut engine = NativeMobility::new(params);
        let demand = PoissonDemand::new(
            &world,
            PoissonParams {
                arrival_rate_per_s: 1.0,
                duration: Duration::from_secs(120),
                ..PoissonParams::default()
            },
            OdParams::default(),
        )
        .expect("demand");
        {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine.init(&mut ctx, Box::new(demand)).expect("init");
        }
        let mut spawned = 0usize;
        let mut completed = 0usize;
        let mut t = 0u64;
        while t < 120 * NS_PER_S {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            let update = engine.step(&mut ctx, params.step);
            spawned += update.spawned.len();
            completed += update
                .despawned
                .iter()
                .filter(|(_, c)| *c == DespawnCause::TripComplete)
                .count();
            // Invariant I-M1: every published list is ordered by actor id.
            let ids: Vec<ActorId> = update.states.iter().map(|(a, _)| *a).collect();
            let mut sorted = ids.clone();
            sorted.sort_unstable();
            assert_eq!(ids, sorted);
            t += 100 * NS_PER_MS;
        }
        assert!(
            spawned > 30,
            "{spawned} trips spawned in two minutes at 1/s"
        );
        assert!(completed > 0, "no trip ever finished");
        // And every published state is answerable through the trait.
        for (id, _, _, _) in engine.longitudinal_states() {
            assert!(Mobility::kinematics(&engine, id).is_some());
        }
    }

    /// A small grid with a fleet of `n` held vehicles: how many are on the road at each
    /// whole second over `secs`, and how many trips finished.
    fn fleet_run(params: PoissonParams, secs: u64) -> (Vec<usize>, usize) {
        let world = v2xw_world::procedural::grid(
            &GridParams::legacy().with_size(4, 8).with_signals(true),
            &ImportOptions::default(),
        )
        .expect("grid");
        let rng = RngRegistry::new(7);
        let ep = EngineParams::default();
        let mut engine = NativeMobility::new(ep);
        let demand = PoissonDemand::new(&world, params, OdParams::default()).expect("demand");
        {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine.init(&mut ctx, Box::new(demand)).expect("init");
        }
        let (mut on_road, mut done) = (Vec::new(), 0usize);
        let mut t = 0u64;
        while t < secs * NS_PER_S {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            let u = engine.step(&mut ctx, ep.step);
            done += u
                .despawned
                .iter()
                .filter(|(_, c)| *c == DespawnCause::TripComplete)
                .count();
            t += ep.step.as_nanos();
            if t % NS_PER_S == 0 {
                on_road.push(engine.len());
            }
        }
        (on_road, done)
    }

    #[test]
    fn a_fleet_of_two_is_two_vehicles_on_the_road() {
        // The owner's report: a cap of 2 on a 4 x 8 grid gave 0 vehicles. A held fleet
        // of 2 is 2 vehicles from the first seconds to the end, finished trips replaced.
        let (on_road, done) = fleet_run(
            PoissonParams {
                fleet_size: 2,
                duration: Duration::from_secs(300),
                ..PoissonParams::default()
            },
            300,
        );
        let full = on_road.iter().skip(2).filter(|n| **n == 2).count();
        assert!(
            on_road.iter().all(|n| *n <= 2),
            "more than the fleet on the road: {on_road:?}"
        );
        assert!(
            full * 100 >= (on_road.len() - 2) * 95,
            "the fleet of 2 was on the road {full} of {} s: {on_road:?}",
            on_road.len() - 2
        );
        assert!(
            done > 0,
            "no trip finished in five minutes, so none was replaced"
        );
    }

    /// A driving lane long enough to hold several vehicles, and somewhere routable to go.
    fn long_lane_and_destination(world: &World) -> (LaneId, LaneId) {
        let mut driving: Vec<LaneId> = world
            .roads
            .lanes()
            .iter()
            .filter(|l| l.kind == LaneKind::Driving && l.length_m > 100.0)
            .map(|l| l.id)
            .collect();
        driving.sort_unstable();
        let origin = driving[0];
        let costs = DynamicCost::new(world);
        let router = DynamicReroute::new(
            DijkstraParams::for_classes(ClassMask::CAR),
            crate::views::ReroutePolicy::STATIC,
        );
        for candidate in driving.iter().skip(1) {
            if router
                .replan(world, origin, *candidate, 0, &costs)
                .is_some_and(|r| r.lanes.len() >= 2)
            {
                return (origin, *candidate);
            }
        }
        panic!("the grid has no routable pair of long driving lanes");
    }

    /// `depart_speed_max` (SUMO `departSpeed="max"`): a trip enters at its desired speed
    /// when the lane ahead is clear, slower behind a vehicle it would otherwise crowd
    /// (keeping the IDM's `s0 + v T`), never faster than it could stop from before the
    /// lane's end; off, every trip departs at rest (live-scenarios finding 3).
    #[test]
    fn a_trip_departs_at_the_highest_safe_speed_when_asked() {
        let world = grid(false);
        let rng = RngRegistry::new(29);
        let (origin, destination) = long_lane_and_destination(&world);
        let lane = world.lane(origin);
        let trip_at = |s_m: f64, seq: u64| TripRequest {
            seq,
            t: 0,
            origin,
            origin_s_m: s_m,
            destination,
            class: VehicleClass::Passenger,
            desired_speed_mps: 13.89,
        };
        let speeds = |depart_speed_max: bool| {
            let params = EngineParams {
                depart_speed_max,
                ..EngineParams::default()
            };
            let mut engine = engine_on(&world, params, &rng);
            let lead = engine
                .insert_trip(&world, &trip_at(60.0, 0), 0)
                .expect("lead");
            let body = VehicleClass::Passenger.spec().length_m;
            let back = engine
                .insert_trip(&world, &trip_at(60.0 - body - 8.0, 1), 0)
                .expect("behind");
            let st = engine.longitudinal_states();
            let v = |id| st.iter().find(|x| x.0 == id).map(|x| x.3).unwrap();
            let d = engine.actors[&back].driver;
            (
                v(lead),
                v(back),
                d,
                engine.actors[&lead].traits.speed_factor,
            )
        };
        let (lead0, back0, _, _) = speeds(false);
        assert_eq!(
            (lead0, back0),
            (0.0, 0.0),
            "off: every trip departs at rest"
        );
        let (lead, back, d, factor) = speeds(true);
        let cap = 13.89_f64.min(lane.speed_limit_mps * factor);
        let stop = (2.0 * d.comfort_decel_mps2 * (lane.length_m - 60.0)).sqrt();
        assert!(
            lead > 0.5 * cap.min(stop),
            "the clear lane's trip departs moving: {lead}"
        );
        assert!(
            lead <= cap.min(stop) + 1e-9,
            "{lead} over min({cap}, {stop})"
        );
        let room = 8.0 - d.min_gap_m;
        assert!(
            back <= room / d.time_headway_s + 1e-9,
            "{back} crowds the leader"
        );
        assert!(
            back < lead,
            "the trip 8 m behind departs slower: {back} vs {lead}"
        );
    }

    #[test]
    fn a_vehicle_on_the_origin_lane_does_not_block_an_insertion_that_has_room() {
        // The defect this pins: the occupancy test took `min(clearance ahead, clearance
        // behind)`. For any pair that does not overlap exactly one of the two is negative,
        // so a single vehicle anywhere on the origin lane refused every further insertion
        // on it — at any distance. Every spawn test in this crate starts from an empty
        // lane, which is why nothing caught it.
        let world = grid(false);
        let rng = RngRegistry::new(29);
        let params = EngineParams::default();
        let mut engine = engine_on(&world, params, &rng);
        let (origin, destination) = long_lane_and_destination(&world);
        let length_m = world.lane(origin).length_m;
        let body = VehicleClass::Passenger.spec().length_m;

        let trip_at = |s_m: f64, seq: u64| TripRequest {
            seq,
            t: 0,
            origin,
            origin_s_m: s_m,
            destination,
            class: VehicleClass::Passenger,
            desired_speed_mps: 13.89,
        };

        // One vehicle parked near the start of the lane.
        assert!(
            engine.insert_trip(&world, &trip_at(10.0, 0), 0).is_some(),
            "the first insertion, into an empty lane"
        );

        // Six more, every one of them with metres of clearance in front of or behind it.
        let offsets = [20.0, 30.0, 45.0, 60.0, 80.0, length_m - 5.0];
        for (i, s_m) in offsets.iter().enumerate() {
            assert!(
                engine
                    .insert_trip(&world, &trip_at(*s_m, i as u64 + 1), 0)
                    .is_some(),
                "a trip at s = {s_m} m on a {length_m:.1} m lane was refused, with the \
                 nearest vehicle metres away"
            );
        }
        assert_eq!(engine.len(), 1 + offsets.len(), "every trip is on the road");
        assert_eq!(engine.dropped_trips(), 0, "nothing was dropped");

        // And the rule it exists to enforce still holds: a trip that would overlap a
        // vehicle, or sit inside the insertion gap of one, is still refused.
        let occupied = engine.len();
        for s_m in [10.0, 10.0 + body, 10.0 + body + 1.9, 20.0 - 1.0] {
            assert!(
                engine.insert_trip(&world, &trip_at(s_m, 99), 0).is_none(),
                "a trip at s = {s_m} m overlaps or crowds the vehicle at 10 m"
            );
        }
        assert_eq!(engine.len(), occupied, "no refused trip reached the road");
        assert_eq!(engine.dropped_trips(), 4, "every refusal was counted");
    }

    #[test]
    fn a_leader_pulling_away_does_not_slow_the_follower_in_the_engine() {
        // The equation-level statement is
        // `carfollowing::idm::tests::a_leader_pulling_away_never_makes_the_ego_brake`;
        // this is the same defect seen through the whole engine, because an equation-level
        // fix that the integrate pass undid would still be a defect. Ego at 5 m/s, a real
        // leader 25 m ahead at 30 m/s: the shipped default took it to 4.966 m/s after one
        // 100 ms step, where the same vehicle alone on the ring reaches 5.140.
        let world = ring(&RingParams {
            circumference_m: 400.0,
            segments: 4,
            ..RingParams::default()
        })
        .expect("a ring");
        let cycle = ring_cycle(&world, 0);
        let rng = RngRegistry::new(31);
        let params = EngineParams {
            intersections: IntersectionMode::None,
            lane_changes: false,
            dynamic_rerouting: false,
            ..EngineParams::default()
        };
        let route: Vec<LaneId> = cycle
            .iter()
            .cycle()
            .take(cycle.len() * 4)
            .copied()
            .collect();
        let driver = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);

        // Alone on the ring: the free-road answer, which is what the follower should very
        // nearly get, because its leader is leaving.
        let alone = {
            let mut engine = engine_on(&world, params, &rng);
            let id = engine
                .spawn_with_route(
                    &world,
                    0,
                    VehicleClass::Passenger,
                    driver,
                    route.clone(),
                    20.0,
                )
                .expect("alone");
            engine.set_speed(id, 5.0).expect("speed");
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine.step(&mut ctx, params.step);
            engine.longitudinal_states()[0].3
        };
        assert!(
            (alone - 5.140).abs() < 0.01,
            "the free-road step: {alone} m/s"
        );

        let mut engine = engine_on(&world, params, &rng);
        let body = VehicleClass::Passenger.spec().length_m;
        // The leader's front bumper 25 m of clear gap ahead of the follower's.
        let leader = engine
            .spawn_with_route(
                &world,
                0,
                VehicleClass::Passenger,
                driver,
                route.clone(),
                20.0 + 25.0 + body,
            )
            .expect("leader");
        let follower = engine
            .spawn_with_route(&world, 0, VehicleClass::Passenger, driver, route, 20.0)
            .expect("follower");
        engine.set_speed(leader, 30.0).expect("speed");
        engine.set_speed(follower, 5.0).expect("speed");
        let mut ctx = MobilityCtx::new(0, &world, &rng);
        engine.step(&mut ctx, params.step);
        let after = engine
            .longitudinal_states()
            .iter()
            .find(|(a, _, _, _)| *a == follower)
            .expect("the follower")
            .3;
        assert!(
            after > 5.0,
            "the follower slowed to {after} m/s because its leader was pulling away"
        );
        // And it is within a whisker of the free road: s* is pinned at s0 = 2 m, so the
        // only difference is a_max·(2/25)².
        assert!(
            (alone - after).abs() < 0.002,
            "free road {alone} m/s against {after} m/s behind a departing leader"
        );
    }

    #[test]
    fn the_legacy_configuration_drives_legacy_drivers() {
        // `driver_for` hard-coded the Kesting 2010 set whatever car-following model was
        // installed, so `NativeMobility::legacy()` — the documented legacy-parity
        // configuration — put Kesting 2010 drivers behind the legacy equations. A parity
        // run against the frozen corpus could not have matched, and nothing said so.
        let world = grid(false);
        let rng = RngRegistry::new(37);
        let (origin, destination) = long_lane_and_destination(&world);
        for (label, mut engine, preset) in [
            (
                "legacy",
                NativeMobility::legacy(EngineParams::default()),
                IdmPreset::Legacy,
            ),
            (
                "default",
                NativeMobility::new(EngineParams::default()),
                IdmPreset::UrbanHcm,
            ),
            (
                "treiber-2000",
                NativeMobility::with_models(
                    EngineParams::default(),
                    Arc::new(Idm::new(IdmPreset::Treiber2000)),
                    MobilPreset::Kesting2007,
                ),
                IdmPreset::Treiber2000,
            ),
        ] {
            {
                let mut ctx = MobilityCtx::new(0, &world, &rng);
                engine
                    .init(&mut ctx, Box::new(NoDemand::new()))
                    .expect("init");
            }
            for class in [
                VehicleClass::Passenger,
                VehicleClass::Truck,
                VehicleClass::Bus,
                VehicleClass::Motorcycle,
            ] {
                let id = engine
                    .insert_trip(
                        &world,
                        &TripRequest {
                            seq: 0,
                            t: 0,
                            origin,
                            origin_s_m: 10.0,
                            destination,
                            class,
                            desired_speed_mps: 13.89,
                        },
                        0,
                    )
                    .expect("a trip");
                let got = engine.actors[&id].driver;
                let mut want = preset.profile(class);
                // Outside the legacy parity mode a motorcycle rides as one
                // ([`TwoWheelerParams`]): its own acceleration, braking, headway and gap
                // on the installed model's equations. The legacy mode keeps the preset's.
                if label != "legacy" && class == VehicleClass::Motorcycle {
                    let tw = TwoWheelerParams::default();
                    want.max_accel_mps2 = tw.motorcycle_accel_mps2;
                    want.comfort_decel_mps2 = tw.comfort_decel_mps2;
                    want.time_headway_s = tw.time_headway_s;
                    want.min_gap_m = tw.min_gap_m;
                }
                assert_eq!(
                    (
                        got.max_accel_mps2,
                        got.comfort_decel_mps2,
                        got.time_headway_s,
                        got.min_gap_m
                    ),
                    (
                        want.max_accel_mps2,
                        want.comfort_decel_mps2,
                        want.time_headway_s,
                        want.min_gap_m
                    ),
                    "{label}/{class:?}: the driver does not follow the installed model"
                );
                // The desired speed is the trip's, as it always was.
                assert_eq!(got.desired_speed_mps, 13.89);
                engine.actors.remove(&id);
            }
        }
        // And the two sets really are different, so the assertion above has teeth.
        let legacy = IdmPreset::Legacy.profile(VehicleClass::Passenger);
        let kesting = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
        assert_eq!(
            (
                legacy.max_accel_mps2,
                legacy.comfort_decel_mps2,
                legacy.time_headway_s,
                legacy.min_gap_m
            ),
            (1.8, 2.5, 1.3, 2.5)
        );
        assert_eq!(
            (
                kesting.max_accel_mps2,
                kesting.comfort_decel_mps2,
                kesting.time_headway_s,
                kesting.min_gap_m
            ),
            (1.4, 2.0, 1.5, 2.0)
        );
    }

    #[test]
    fn a_commanded_spawn_is_announced_on_the_update() {
        // `apply_commands` called `insert_trip` and threw the `ActorId` away, so
        // `MobilityUpdate::spawned` was built from the demand pass alone and a commanded
        // trip never reached a consumer. `states` carries none of what an `ActorSpawn`
        // carries — class, route, driver profile, demand-stream `seq` — so a recorder or a
        // node runtime driven off `spawned` never provisioned the node behind an injected
        // vehicle.
        let world = grid(false);
        let rng = RngRegistry::new(41);
        let params = EngineParams::default();
        let mut engine = engine_on(&world, params, &rng);
        let (origin, destination) = long_lane_and_destination(&world);
        let trip = TripRequest {
            seq: 7,
            t: 0,
            origin,
            origin_s_m: 10.0,
            destination,
            class: VehicleClass::Delivery,
            desired_speed_mps: 11.0,
        };
        {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine.command(&mut ctx, MobilityCommand::Spawn(trip.clone()));
        }
        let update = {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine.step(&mut ctx, params.step)
        };
        assert_eq!(engine.len(), 1, "the vehicle is on the road");
        assert_eq!(update.states.len(), 1);
        assert_eq!(
            update.spawned.len(),
            1,
            "a commanded spawn is missing from `spawned`"
        );
        let announced = &update.spawned[0];
        assert_eq!(announced.actor, update.states[0].0);
        assert_eq!(announced.class, VehicleClass::Delivery);
        assert_eq!(announced.seq, 7, "invariant I-M2's seq accounting");
        assert_eq!(announced.driver.desired_speed_mps, 11.0);
        assert_eq!(announced.route.lanes.first().copied(), Some(origin));
        assert_eq!(announced.route.lanes.last().copied(), Some(destination));

        // Two commanded spawns and a demand pass in the same step: every one is announced,
        // once, and the list is still ordered by actor id (invariant I-M1).
        let mut engine = engine_on(&world, params, &rng);
        {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            for (i, s_m) in [10.0f64, 60.0].iter().enumerate() {
                engine.command(
                    &mut ctx,
                    MobilityCommand::Spawn(TripRequest {
                        seq: i as u64,
                        origin_s_m: *s_m,
                        ..trip.clone()
                    }),
                );
            }
        }
        let update = {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine.step(&mut ctx, params.step)
        };
        assert_eq!(update.spawned.len(), 2);
        let ids: Vec<ActorId> = update.spawned.iter().map(|s| s.actor).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted, "invariant I-M1");
    }

    #[test]
    fn demand_is_not_suppressed_by_the_vehicles_already_on_the_road() {
        // The scenario-scale consequence of the `min`/`max` defect above: with the
        // occupancy test refusing every insertion onto any occupied lane, 600 s of default
        // Poisson demand on the legacy grid lost 520 of 1,218 routed trips — 42.7 % of the
        // demand, silently. The number below is the measured placement rate of the same
        // run; it is asserted rather than printed so a regression cannot pass unnoticed.
        let world = grid(false);
        let rng = RngRegistry::new(11);
        let params = EngineParams::default();
        let mut engine = NativeMobility::new(params);
        let demand = PoissonDemand::new(&world, PoissonParams::default(), OdParams::default())
            .expect("demand");
        {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine.init(&mut ctx, Box::new(demand)).expect("init");
        }
        let mut placed = 0usize;
        let mut t = 0u64;
        while t < 600 * NS_PER_S {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            placed += engine.step(&mut ctx, params.step).spawned.len();
            t += params.step.as_nanos();
        }
        let dropped = engine.dropped_trips() as usize;
        let routed = placed + dropped;
        let rate = 100.0 * placed as f64 / routed as f64;
        println!(
            "{placed} of {routed} routed trips placed ({rate:.1} %), {dropped} dropped at \
             insertion, {} on the road at the end",
            engine.len()
        );
        assert!(routed > 1000, "only {routed} trips were routed");
        // Measured 81.0 % with the fix and 59.1 % with `min` in place of `max`. The
        // residual drops are real: a trip's origin offset is drawn uniformly along the
        // lane, and each vehicle already on a 113 m grid lane makes a 14 m band of that
        // draw (its own 5 m plus the 2 m insertion gap at each end) a genuine refusal,
        // which is about 12 % of the lane per vehicle.
        assert!(
            rate > 75.0,
            "only {rate:.1} % of {routed} routed trips were placed ({dropped} dropped at \
             insertion): demand is being suppressed"
        );
    }

    #[test]
    fn a_closure_is_not_driven_into() {
        let world = grid(false);
        let rng = RngRegistry::new(13);
        let params = EngineParams::default();
        let mut engine = engine_on(&world, params, &rng);
        let lanes: Vec<LaneId> = world
            .roads
            .lanes()
            .iter()
            .filter(|l| l.kind == LaneKind::Driving)
            .map(|l| l.id)
            .collect();
        let driver = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
        // A route of three lanes, with the third closed after the vehicle sets off.
        let route = {
            let costs = DynamicCost::new(&world);
            engine
                .router
                .replan(&world, lanes[0], lanes[lanes.len() / 2], 0, &costs)
                .expect("a route")
        };
        assert!(route.lanes.len() >= 3);
        let id = engine
            .spawn_with_route(
                &world,
                0,
                VehicleClass::Passenger,
                driver,
                route.lanes.clone(),
                1.0,
            )
            .expect("spawned");
        let closed = route.lanes[2];
        {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine.command(
                &mut ctx,
                MobilityCommand::Closure {
                    lane: closed,
                    closed: true,
                },
            );
        }
        let mut t = 0u64;
        let mut ever_on_closed = false;
        for _ in 0..600 {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            engine.step(&mut ctx, params.step);
            if engine
                .longitudinal_states()
                .iter()
                .any(|(a, lane, _, _)| *a == id && *lane == closed)
            {
                ever_on_closed = true;
            }
            t += 100 * NS_PER_MS;
        }
        assert!(!ever_on_closed, "the vehicle drove onto a closed lane");
    }

    /// A reopen and a close in the same step change the router's cost generation, so every
    /// vehicle that planned before them knows to plan again. Counting the closed lanes, as
    /// the engine did, left the generation where it was.
    #[test]
    fn a_reopen_and_a_close_in_one_step_change_the_cost_generation() {
        let world = grid(false);
        let rng = RngRegistry::new(13);
        let params = EngineParams::default();
        let mut engine = engine_on(&world, params, &rng);
        let lanes: Vec<LaneId> = world
            .roads
            .lanes()
            .iter()
            .filter(|l| l.kind == LaneKind::Driving)
            .map(|l| l.id)
            .collect();
        let close = |engine: &mut NativeMobility, t: u64, cmds: &[(LaneId, bool)]| {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            for (lane, closed) in cmds {
                engine.command(
                    &mut ctx,
                    MobilityCommand::Closure {
                        lane: *lane,
                        closed: *closed,
                    },
                );
            }
            engine.step(&mut ctx, params.step);
            engine.costs(&world).generation()
        };
        let first = close(&mut engine, 0, &[(lanes[1], true)]);
        let second = close(
            &mut engine,
            100 * NS_PER_MS,
            &[(lanes[1], false), (lanes[2], true)],
        );
        assert_ne!(
            first, second,
            "the closure set changed and the generation did not"
        );
        // A command that changes nothing is not a change.
        let third = close(&mut engine, 200 * NS_PER_MS, &[(lanes[2], true)]);
        assert_eq!(second, third);
    }

    /// A vehicle whose destination a closure cuts off does not vanish at the barrier as
    /// `RouteBlocked`: it goes round, and ends its trip as near its destination as it can.
    #[test]
    fn a_vehicle_cut_off_by_a_closure_detours_instead_of_vanishing() {
        let world = grid(false);
        let rng = RngRegistry::new(13);
        for dynamic_rerouting in [true, false] {
            let params = EngineParams {
                dynamic_rerouting,
                ..EngineParams::default()
            };
            let mut engine = engine_on(&world, params, &rng);
            let lanes: Vec<LaneId> = world
                .roads
                .lanes()
                .iter()
                .filter(|l| l.kind == LaneKind::Driving)
                .map(|l| l.id)
                .collect();
            let driver = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
            let route = {
                let costs = DynamicCost::new(&world);
                engine
                    .router
                    .replan(&world, lanes[0], lanes[lanes.len() / 2], 0, &costs)
                    .expect("a route")
            };
            let destination = *route.lanes.last().expect("a lane");
            let id = engine
                .spawn_with_route(
                    &world,
                    0,
                    VehicleClass::Passenger,
                    driver,
                    route.lanes.clone(),
                    1.0,
                )
                .expect("spawned");
            {
                // The destination itself closes: nothing can reach it any more.
                let mut ctx = MobilityCtx::new(0, &world, &rng);
                engine.command(
                    &mut ctx,
                    MobilityCommand::Closure {
                        lane: destination,
                        closed: true,
                    },
                );
            }
            let mut t = 0u64;
            let mut left = None;
            for _ in 0..3000 {
                let mut ctx = MobilityCtx::new(t, &world, &rng);
                let update = engine.step(&mut ctx, params.step);
                if let Some((_, cause)) = update.despawned.iter().find(|(a, _)| *a == id) {
                    left = Some(*cause);
                    break;
                }
                assert!(
                    !engine
                        .longitudinal_states()
                        .iter()
                        .any(|(a, lane, _, _)| *a == id && *lane == destination),
                    "the vehicle drove onto its closed destination"
                );
                t += 100 * NS_PER_MS;
            }
            assert_eq!(
                left,
                Some(DespawnCause::TripComplete),
                "dynamic rerouting {dynamic_rerouting}: the cut-off vehicle left as {left:?}"
            );
        }
    }

    #[test]
    fn an_external_stop_holds_a_vehicle() {
        let world = ring(&RingParams::default()).expect("a ring");
        let cycle = ring_cycle(&world, 0);
        let rng = RngRegistry::new(5);
        let params = EngineParams {
            intersections: IntersectionMode::None,
            lane_changes: false,
            dynamic_rerouting: false,
            ..EngineParams::default()
        };
        let mut engine = engine_on(&world, params, &rng);
        let route: Vec<LaneId> = cycle
            .iter()
            .cycle()
            .take(cycle.len() * 4)
            .copied()
            .collect();
        let driver = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
        let id = engine
            .spawn_with_route(&world, 0, VehicleClass::Passenger, driver, route, 10.0)
            .expect("spawned");
        engine.set_speed(id, 20.0).expect("speed");
        {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine.command(
                &mut ctx,
                MobilityCommand::Stop {
                    actor: id,
                    until: Some(10 * NS_PER_S),
                },
            );
        }
        let mut t = 0u64;
        let mut speed_at_5s = f64::INFINITY;
        let mut speed_at_20s = 0.0;
        for _ in 0..300 {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            engine.step(&mut ctx, params.step);
            let v = engine
                .longitudinal_states()
                .first()
                .map(|(_, _, _, v)| *v)
                .unwrap_or(0.0);
            if t == 5 * NS_PER_S {
                speed_at_5s = v;
            }
            if t == 20 * NS_PER_S {
                speed_at_20s = v;
            }
            t += 100 * NS_PER_MS;
        }
        assert!(speed_at_5s < 1.0, "held at {speed_at_5s} m/s");
        assert!(speed_at_20s > 5.0, "released to {speed_at_20s} m/s");
    }

    /// A delivery van creeping up on a stopped car: the hardest step-to-step braking and
    /// the gap it stands at, with and without `standing_leader_stop`.
    fn van_behind_a_standing_car(standing_leader_stop: bool) -> (f64, f64) {
        let world = ring(&RingParams {
            circumference_m: 400.0,
            segments: 4,
            ..RingParams::default()
        })
        .expect("a ring");
        let cycle = ring_cycle(&world, 0);
        let rng = RngRegistry::new(11);
        let params = EngineParams {
            intersections: IntersectionMode::None,
            lane_changes: false,
            dynamic_rerouting: false,
            standing_leader_stop,
            ..EngineParams::default()
        };
        let mut engine = engine_on(&world, params, &rng);
        let route: Vec<LaneId> = cycle
            .iter()
            .cycle()
            .take(cycle.len() * 4)
            .copied()
            .collect();
        let car = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
        let van = IdmPreset::Kesting2010.profile(VehicleClass::Delivery);
        let leader = engine
            .spawn_with_route(&world, 0, VehicleClass::Passenger, car, route.clone(), 60.0)
            .expect("leader");
        let rear = 60.0 - VehicleClass::Passenger.spec().length_m;
        let follower = engine
            .spawn_with_route(
                &world,
                0,
                VehicleClass::Delivery,
                van,
                route,
                rear - van.min_gap_m - 1.5,
            )
            .expect("follower");
        engine.set_speed(leader, 0.0).expect("speed");
        engine.set_speed(follower, 1.5).expect("speed");
        {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine.command(
                &mut ctx,
                MobilityCommand::Stop {
                    actor: leader,
                    until: None,
                },
            );
        }
        let mut t = 0u64;
        let mut prev = 1.5;
        let mut hardest = 0.0f64;
        let mut gap = f64::INFINITY;
        for _ in 0..150 {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            engine.step(&mut ctx, params.step);
            let states = engine.longitudinal_states();
            let l = states
                .iter()
                .find(|(a, _, _, _)| *a == leader)
                .copied()
                .expect("leader");
            let f = states
                .iter()
                .find(|(a, _, _, _)| *a == follower)
                .copied()
                .expect("van");
            hardest = hardest.max((prev - f.3) / 0.1);
            prev = f.3;
            if l.1 == f.1 {
                gap = (l.2 - VehicleClass::Passenger.spec().length_m) - f.2;
            }
            t += 100 * NS_PER_MS;
        }
        (hardest, gap)
    }

    #[test]
    fn a_van_behind_a_standing_car_stops_at_its_gap_without_a_one_step_stop() {
        // Without the rule the van reaches its standstill gap still moving and the advance
        // cap stops it in one step (the Midtown delivery van at -8.5 m/s²).
        let (hardest_off, _) = van_behind_a_standing_car(false);
        assert!(
            hardest_off > 3.0,
            "the IDM's late braking this rule replaces: {hardest_off} m/s²"
        );
        let (hardest, gap) = van_behind_a_standing_car(true);
        let s0 = IdmPreset::Kesting2010
            .profile(VehicleClass::Delivery)
            .min_gap_m;
        assert!(
            hardest < 2.5,
            "braked at {hardest} m/s² for a stop from 1.5 m/s in 1.5 m"
        );
        assert!(
            gap > s0 - 0.05 && gap < s0 + 0.5,
            "stood {gap} m behind (s0 {s0} m)"
        );
    }

    #[test]
    fn a_kink_in_a_through_lane_is_driven_at_its_turn_speed() {
        // A square ring with one 45-degree vertex in the middle of every edge's lane.
        let world = ring(&RingParams {
            circumference_m: 800.0,
            segments: 4,
            points_per_segment: 2,
            speed_limit_mps: 15.0,
            ..RingParams::default()
        })
        .expect("a ring");
        let cycle = ring_cycle(&world, 0);
        let run = |lane_bend_speed: bool| -> (f64, f64) {
            let rng = RngRegistry::new(13);
            let params = EngineParams {
                intersections: IntersectionMode::None,
                lane_changes: false,
                dynamic_rerouting: false,
                turn_curve_window_m: 6.0,
                lane_bend_speed,
                ..EngineParams::default()
            };
            let mut engine = engine_on(&world, params, &rng);
            let route: Vec<LaneId> = cycle
                .iter()
                .cycle()
                .take(cycle.len() * 4)
                .copied()
                .collect();
            let driver = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
            let id = engine
                .spawn_with_route(&world, 0, VehicleClass::Passenger, driver, route, 5.0)
                .expect("spawned");
            engine.set_speed(id, 15.0).expect("speed");
            let mut t = 0u64;
            let mut at_kink = 0.0f64;
            let mut slowest = f64::INFINITY;
            for _ in 0..600 {
                let mut ctx = MobilityCtx::new(t, &world, &rng);
                engine.step(&mut ctx, params.step);
                let (_, lane, s, v) = engine.longitudinal_states()[0];
                let l = world.lane(lane);
                if l.centreline.len() == 3 && (s - l.cumulative[1]).abs() < 1.0 {
                    at_kink = at_kink.max(v);
                }
                if t > 20 * NS_PER_S {
                    slowest = slowest.min(v);
                }
                t += 100 * NS_PER_MS;
            }
            (at_kink, slowest)
        };
        // 0.785 rad within the 6 m window: R = 7.64 m, sqrt(5.5 · 7.64) = 6.5 m/s.
        let cap = (DEFAULT_TURN_LATERAL_ACCEL_MPS2 * 6.0 / core::f64::consts::FRAC_PI_4).sqrt();
        let (off, _) = run(false);
        assert!(
            off > 12.0,
            "without the rule the kink is taken at {off} m/s"
        );
        let (on, slowest) = run(true);
        assert!(
            on > 0.0 && on <= cap + 0.3,
            "the kink was taken at {on} m/s (cap {cap:.2})"
        );
        assert!(
            slowest > 0.5 * cap,
            "slowed to {slowest} m/s for a {cap:.2} m/s bend"
        );
    }

    #[test]
    fn a_vehicle_jammed_past_the_timeout_is_moved_on_and_counted() {
        // A leader held for good (a scripted stop, which is excused) and a follower stuck
        // behind it: with a 20 s timeout the follower is moved to the next lane of its
        // route with room, once, and counted; with the timeout off it waits for ever.
        let run = |timeout_s: f64| {
            let world = ring(&RingParams::default()).expect("a ring");
            let cycle = ring_cycle(&world, 0);
            let rng = RngRegistry::new(5);
            let params = EngineParams {
                intersections: IntersectionMode::None,
                lane_changes: false,
                dynamic_rerouting: false,
                time_to_teleport_s: timeout_s,
                ..EngineParams::default()
            };
            let mut engine = engine_on(&world, params, &rng);
            let route: Vec<LaneId> = cycle
                .iter()
                .cycle()
                .take(cycle.len() * 4)
                .copied()
                .collect();
            let driver = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
            let leader = engine
                .spawn_with_route(
                    &world,
                    0,
                    VehicleClass::Passenger,
                    driver,
                    route.clone(),
                    40.0,
                )
                .expect("leader");
            let follower = engine
                .spawn_with_route(&world, 0, VehicleClass::Passenger, driver, route, 10.0)
                .expect("follower");
            {
                let mut ctx = MobilityCtx::new(0, &world, &rng);
                engine.command(
                    &mut ctx,
                    MobilityCommand::Stop {
                        actor: leader,
                        until: None,
                    },
                );
            }
            let mut t = 0u64;
            for _ in 0..600 {
                let mut ctx = MobilityCtx::new(t, &world, &rng);
                engine.step(&mut ctx, params.step);
                t += 100 * NS_PER_MS;
            }
            let lane_of = |id: ActorId| {
                engine
                    .longitudinal_states()
                    .iter()
                    .find(|(a, _, _, _)| *a == id)
                    .map(|(_, l, s, _)| (*l, *s))
            };
            (
                engine.gridlock_counters(),
                lane_of(leader),
                lane_of(follower),
            )
        };
        let (on, leader, follower) = run(20.0);
        assert!(
            on.teleported >= 1,
            "the stuck follower was not moved on: {on:?}"
        );
        assert_eq!(on.removed, 0, "{on:?}");
        assert_ne!(
            leader.map(|(l, _)| l),
            follower.map(|(l, _)| l),
            "the follower is still behind the held leader"
        );
        let (off, _, _) = run(0.0);
        assert_eq!(
            off,
            GridlockCounters::default(),
            "the timeout off still acted"
        );
    }

    #[test]
    fn a_driver_in_the_wrong_lane_changes_lane_for_the_turn_instead_of_going_round() {
        // Two lanes each way, left turns from the left lane only. A trip that starts in the
        // right lane and turns left at the first junction: with strategic lane choice the
        // driver moves over and turns; without it the router can only send them round.
        let world = v2xw_world::procedural::grid(
            &GridParams {
                lanes_per_direction: 2,
                ..GridParams::legacy().with_size(3, 3).with_signals(false)
            },
            &ImportOptions::default(),
        )
        .expect("grid");
        let pick = world.roads.lanes().iter().find_map(|l| {
            if l.kind != LaneKind::Driving || l.index != 0 || l.length_m < 80.0 {
                return None;
            }
            let left_of = world
                .edge(l.edge)
                .lanes
                .iter()
                .copied()
                .find(|x| world.lane(*x).index == 1)?;
            let turn = world
                .successors(left_of)
                .iter()
                .find(|c| c.permitted && c.direction == TurnDirection::Left)?;
            let own_turns_left = world
                .successors(l.id)
                .iter()
                .any(|c| c.direction == TurnDirection::Left);
            (!own_turns_left).then_some((l.id, left_of, turn.to_lane))
        });
        let (origin, left_lane, exit) =
            pick.expect("a right lane whose left turn is from the lane beside");
        let run = |strategic: bool| {
            let rng = RngRegistry::new(3);
            let params = EngineParams {
                strategic_lane_choice: strategic,
                ..EngineParams::default()
            };
            let mut engine = engine_on(&world, params, &rng);
            let trip = TripRequest {
                seq: 0,
                t: 0,
                origin,
                origin_s_m: 10.0,
                destination: exit,
                class: VehicleClass::Passenger,
                desired_speed_mps: 11.0,
            };
            // Without strategic choice the trip may have no route at all from this lane.
            let Some(id) = engine.insert_trip(&world, &trip, 0) else {
                return (f64::INFINITY, false, None);
            };
            let route_m = engine.actors[&id].route.length_m;
            let (mut on_left, mut done_at) = (false, None);
            let mut t = 0u64;
            while t < 120 * NS_PER_S && done_at.is_none() {
                let mut ctx = MobilityCtx::new(t, &world, &rng);
                let u = engine.step(&mut ctx, params.step);
                if engine.actors.get(&id).is_some_and(|a| a.lane == left_lane) {
                    on_left = true;
                }
                if u.despawned
                    .iter()
                    .any(|(a, c)| *a == id && *c == DespawnCause::TripComplete)
                {
                    done_at = Some(t);
                }
                t += params.step.as_nanos();
            }
            (route_m, on_left, done_at)
        };
        let (plain_m, _, _) = run(false);
        let (strategic_m, on_left, done) = run(true);
        assert!(on_left, "the driver never moved into the left-turn lane");
        assert!(
            done.is_some(),
            "the strategic trip did not finish in two minutes"
        );
        assert!(
            strategic_m + 50.0 < plain_m,
            "the strategic route ({strategic_m:.0} m) is not shorter than going round ({plain_m:.0} m)"
        );
    }

    #[test]
    fn a_driver_level_with_a_car_in_the_turn_lane_drops_behind_it_and_turns() {
        // As above, but a car starts level with the driver in the left-turn lane, at the
        // same speed, going straight on. Without falling behind it the two drove side by
        // side to the end of the lane and the driver missed the turn.
        let world = v2xw_world::procedural::grid(
            &GridParams {
                lanes_per_direction: 2,
                ..GridParams::legacy().with_size(3, 3).with_signals(false)
            },
            &ImportOptions::default(),
        )
        .expect("grid");
        let pick = world.roads.lanes().iter().find_map(|l| {
            if l.kind != LaneKind::Driving || l.index != 0 || l.length_m < 80.0 {
                return None;
            }
            let left_of = world
                .edge(l.edge)
                .lanes
                .iter()
                .copied()
                .find(|x| world.lane(*x).index == 1)?;
            let turn = world
                .successors(left_of)
                .iter()
                .find(|c| c.permitted && c.direction == TurnDirection::Left)?;
            let straight = world
                .successors(left_of)
                .iter()
                .find(|c| c.permitted && c.direction == TurnDirection::Straight)?;
            let own_turns_left = world
                .successors(l.id)
                .iter()
                .any(|c| c.direction == TurnDirection::Left);
            (!own_turns_left).then_some((l.id, left_of, turn.to_lane, straight.to_lane))
        });
        let (origin, left_lane, exit, ahead) =
            pick.expect("a right lane whose left turn is from the lane beside");
        let rng = RngRegistry::new(3);
        let params = EngineParams {
            strategic_lane_choice: true,
            driver_heterogeneity: false,
            driver_styles: false,
            ..EngineParams::default()
        };
        let mut engine = engine_on(&world, params, &rng);
        let trip = |seq: u64, origin: LaneId, origin_s_m: f64, destination: LaneId| TripRequest {
            seq,
            t: 0,
            origin,
            origin_s_m,
            destination,
            class: VehicleClass::Passenger,
            desired_speed_mps: 11.0,
        };
        let id = engine
            .insert_trip(&world, &trip(0, origin, 10.0, exit), 0)
            .expect("the turning car");
        engine
            .insert_trip(&world, &trip(1, left_lane, 11.0, ahead), 0)
            .expect("the car beside it");
        let (mut done, mut on_left) = (false, false);
        let mut t = 0u64;
        while t < 120 * NS_PER_S && !done {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            let u = engine.step(&mut ctx, params.step);
            on_left |= engine.actors.get(&id).is_some_and(|a| a.lane == left_lane);
            if let Some((_, c)) = u.despawned.iter().find(|(a, _)| *a == id) {
                assert_eq!(
                    *c,
                    DespawnCause::TripComplete,
                    "the turning car was removed"
                );
                done = true;
            }
            t += params.step.as_nanos();
        }
        assert!(on_left, "the turning car never got into the left-turn lane");
        assert!(done, "the turning car did not finish in two minutes");
    }

    #[test]
    fn a_standing_queue_prices_its_lane_up_for_the_router() {
        let world = grid(false);
        let (origin, destination) = long_lane_and_destination(&world);
        let run = |period_s: f64| {
            let rng = RngRegistry::new(4);
            let params = EngineParams {
                reroute_period_s: period_s,
                ..EngineParams::default()
            };
            let mut engine = engine_on(&world, params, &rng);
            let trip = TripRequest {
                seq: 0,
                t: 0,
                origin,
                origin_s_m: 30.0,
                destination,
                class: VehicleClass::Passenger,
                desired_speed_mps: 11.0,
            };
            let id = engine.insert_trip(&world, &trip, 0).expect("placed");
            {
                let mut ctx = MobilityCtx::new(0, &world, &rng);
                engine.command(
                    &mut ctx,
                    MobilityCommand::Stop {
                        actor: id,
                        until: None,
                    },
                );
            }
            let mut t = 0u64;
            for _ in 0..1200 {
                let mut ctx = MobilityCtx::new(t, &world, &rng);
                engine.step(&mut ctx, params.step);
                t += params.step.as_nanos();
            }
            let free = world.lane(origin).length_m / world.lane(origin).speed_limit_mps;
            let priced = crate::views::EdgeCost::lane_cost_s(&engine.costs(&world), origin, t)
                .expect("open");
            (free, priced)
        };
        let (free, on) = run(60.0);
        assert!(
            on > 1.8 * free,
            "a lane with a car standing on it for 2 min costs {on:.1} s against {free:.1} s free"
        );
        let (_, off) = run(0.0);
        assert!(
            (off - free).abs() < 1e-9,
            "with rerouting off the lane costs {off} s, not its free {free} s"
        );
    }

    /// A car brought to a stand part-way through a lane change, behind a car standing in
    /// the lane it is leaving, finishes the change when the lane it is entering is clear
    /// beside it — and stays held while a car stands there. Held for good, it stood
    /// across both lanes for the rest of the run (`check-traffic` `phantom_stop` on the
    /// signalised grid, every case a car 1.2 to 2.2 m off its lane's centre).
    #[test]
    fn a_car_stopped_part_way_across_finishes_the_change_when_the_lane_beside_is_clear() {
        let world = v2xw_world::procedural::grid(
            &GridParams {
                lanes_per_direction: 2,
                ..GridParams::legacy().with_size(3, 3).with_signals(false)
            },
            &ImportOptions::default(),
        )
        .expect("grid");
        let (from, to) = world
            .roads
            .lanes()
            .iter()
            .find_map(|l| {
                if l.kind != LaneKind::Driving || l.index != 0 || l.length_m < 80.0 {
                    return None;
                }
                let beside = world
                    .edge(l.edge)
                    .lanes
                    .iter()
                    .copied()
                    .find(|x| world.lane(*x).index == 1)?;
                Some((l.id, beside))
            })
            .expect("a two-lane edge");
        let run = |blocker_beside: bool| {
            let rng = RngRegistry::new(5);
            let params = EngineParams::default();
            let mut engine = engine_on(&world, params, &rng);
            let trip = |seq: u64, lane: LaneId, s: f64| TripRequest {
                seq,
                t: 0,
                origin: lane,
                origin_s_m: s,
                destination: world.successors(lane)[0].to_lane,
                class: VehicleClass::Passenger,
                desired_speed_mps: 11.0,
            };
            let lead = engine
                .insert_trip(&world, &trip(0, from, 60.0), 0)
                .expect("lead");
            let ego = engine
                .insert_trip(&world, &trip(1, to, 30.0), 0)
                .expect("ego");
            let side = blocker_beside.then(|| {
                engine
                    .insert_trip(&world, &trip(2, to, 62.0), 0)
                    .expect("side")
            });
            let t_start = 10 * NS_PER_S;
            let len = VehicleClass::Passenger.spec().length_m;
            // The lead and the car beside stand for good.
            for id in [Some(lead), side].into_iter().flatten() {
                let a = engine.actors.get_mut(&id).expect("actor");
                a.speed_mps = 0.0;
                a.stopped_until = Some(SimTime::MAX);
            }
            if let Some(id) = side {
                // Beside the ego's body: [51.5, 56] against its [49.5, 54].
                engine.actors.get_mut(&id).expect("side").s_m = 56.0;
            }
            // The ego: in the target lane's frame, half-way across from `from`, its front
            // 1.5 m behind the lead's rear (inside the 2 m standstill gap), standing.
            let home = engine.lane_position_m(VehicleClass::Passenger);
            let from_offset = {
                let p = world.lane(from).offset_point(60.0 - len - 1.5, 0.0);
                world.lane(to).project_point(p).d_m
            };
            {
                let a = engine.actors.get_mut(&ego).expect("ego");
                a.s_m = 60.0 - len - 1.5;
                a.speed_mps = 0.0;
                a.lateral_m = 0.5 * (from_offset + home);
                a.transition = Some(Transition {
                    from,
                    to,
                    started: t_start - 1_500_000_000,
                    duration: Duration::from_secs(3),
                    from_offset_m: from_offset,
                    to_offset_m: home,
                    switched: true,
                    from_s_delta: 0.0,
                    pull_out: false,
                    began_moving: true,
                });
            }
            let mut t = t_start;
            while t < t_start + 5 * NS_PER_S {
                let mut ctx = MobilityCtx::new(t, &world, &rng);
                engine.step(&mut ctx, params.step);
                t += params.step.as_nanos();
            }
            engine.actors.get(&ego).map_or((true, 0.0), |a| {
                (a.transition.is_none(), (a.lateral_m - home).abs())
            })
        };
        let (done, off) = run(false);
        assert!(
            done && off < 0.05,
            "with the lane beside clear the change never finished: {off:.2} m off the centre"
        );
        let (done, off) = run(true);
        assert!(
            !done && off > 1.0,
            "a car slid sideways into the lane beside it while one stood there ({off:.2} m off)"
        );
    }

    #[test]
    fn the_lane_change_cooldown_is_the_models_own_rule() {
        // The engine reconstructed the cooldown inline as `max(2·duration, 5 s)` instead of
        // asking `Mobil::cooldown`, which is
        // `max(cooldown_factor·transition_s, cooldown_factor·duration)`. The two agreed
        // only because no constructor let a scenario pass custom `MobilParams`; the
        // literal 5.0 was a hard-coded copy of `2 · transition_s` for the shipped presets.
        // `with_lane_change_params` is that seam, so the disagreement is now reachable —
        // and pinned.
        let world = ring(&RingParams {
            circumference_m: 800.0,
            segments: 4,
            lanes: 2,
            ..RingParams::default()
        })
        .expect("a ring");
        let right = crate::worlds::ring_cycle(&world, 0);
        let params = EngineParams {
            intersections: IntersectionMode::None,
            lane_changes: true,
            dynamic_rerouting: false,
            ..EngineParams::default()
        };
        // A parameter set whose cooldown rule is nothing like `max(2·duration, 5 s)`.
        let mobil_params = MobilParams {
            cooldown_factor: 4.0,
            transition_s: 3.0,
            reconsider_rate_per_s: 1e9,
            ..MobilPreset::Kesting2007.params()
        };
        let mobil = Mobil::with_params(
            MobilPreset::Kesting2007,
            MobilParams {
                step_s: params.step.as_secs_f64(),
                ..mobil_params
            },
            Arc::new(Idm::new(IdmPreset::Kesting2010)),
        );
        let rng = RngRegistry::new(19);
        let mut engine = NativeMobility::with_lane_change_params(
            params,
            Arc::new(Idm::new(IdmPreset::Kesting2010)),
            MobilPreset::Kesting2007,
            mobil_params,
        );
        {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine
                .init(&mut ctx, Box::new(NoDemand::new()))
                .expect("init");
        }
        let route: Vec<LaneId> = right
            .iter()
            .cycle()
            .take(right.len() * 8)
            .copied()
            .collect();
        let driver = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
        let leader = engine
            .spawn_with_route(
                &world,
                0,
                VehicleClass::Passenger,
                driver,
                route.clone(),
                60.0,
            )
            .expect("leader");
        let follower = engine
            .spawn_with_route(&world, 0, VehicleClass::Passenger, driver, route, 20.0)
            .expect("follower");
        engine.set_speed(leader, 4.0).expect("speed");
        engine.set_speed(follower, 25.0).expect("speed");

        let mut t = 0u64;
        let mut armed: Option<(SimTime, Duration)> = None;
        for _ in 0..600 {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            engine.step(&mut ctx, params.step);
            let actor = &engine.actors[&follower];
            if let Some(transition) = actor.transition {
                armed = Some((actor.cooldown_until, transition.duration));
                break;
            }
            t += 100 * NS_PER_MS;
        }
        let (cooldown_until, duration) = armed.expect("the follower never changed lane");
        let want = mobil.cooldown(duration).after(t);
        assert_eq!(
            cooldown_until,
            want,
            "the engine's cooldown ({} s after the change) is not the model's \
             ({} s), for a {} s transition",
            ns_to_secs(cooldown_until.saturating_sub(t)),
            ns_to_secs(want.saturating_sub(t)),
            duration.as_secs_f64()
        );
        // And the rule it replaced would have given a different answer here, so the
        // assertion above has teeth.
        let inline = duration
            .saturating_mul(2)
            .max(Duration::from_secs_f64(5.0))
            .after(t);
        assert_ne!(
            inline, want,
            "the two rules agree for these parameters, so nothing is being tested"
        );
    }

    #[test]
    fn a_speed_capped_vehicle_does_not_change_lane_for_a_gain_it_cannot_take() {
        // MOBIL was handed `actor.view(world)` — the driver WITHOUT the externally
        // commanded speed cap — while the car-following call used the capped view. So a
        // vehicle told to slow to 4 m/s went on scoring the overtaking lane as if it could
        // still do 33.3, and pulled out for a speed gain it would never be allowed to take.
        let world = ring(&RingParams {
            circumference_m: 800.0,
            segments: 4,
            lanes: 2,
            ..RingParams::default()
        })
        .expect("a ring");
        let right = crate::worlds::ring_cycle(&world, 0);
        let params = EngineParams {
            intersections: IntersectionMode::None,
            lane_changes: true,
            dynamic_rerouting: false,
            ..EngineParams::default()
        };
        // The same scenario as `a_blocked_vehicle_changes_lane_and_settles_in_it`, run
        // twice: once with a speed cap on the follower and once without.
        // Reconsideration every step, so the answer is MOBIL's incentive and not the
        // timing of its Bernoulli draw; everything else is the shipped Kesting 2007 set.
        let mobil_params = MobilParams {
            reconsider_rate_per_s: 1e9,
            ..MobilPreset::Kesting2007.params()
        };
        let first_change = |cap: Option<f64>| -> Option<usize> {
            let rng = RngRegistry::new(19);
            let mut engine = NativeMobility::with_lane_change_params(
                params,
                Arc::new(Idm::new(IdmPreset::Kesting2010)),
                MobilPreset::Kesting2007,
                mobil_params,
            );
            {
                let mut ctx = MobilityCtx::new(0, &world, &rng);
                engine
                    .init(&mut ctx, Box::new(NoDemand::new()))
                    .expect("init");
            }
            let route: Vec<LaneId> = right
                .iter()
                .cycle()
                .take(right.len() * 8)
                .copied()
                .collect();
            let driver = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
            let leader = engine
                .spawn_with_route(
                    &world,
                    0,
                    VehicleClass::Passenger,
                    driver,
                    route.clone(),
                    60.0,
                )
                .expect("leader");
            let follower = engine
                .spawn_with_route(&world, 0, VehicleClass::Passenger, driver, route, 20.0)
                .expect("follower");
            engine.set_speed(leader, 4.0).expect("speed");
            engine.set_speed(follower, 25.0).expect("speed");
            if let Some(v) = cap {
                let mut ctx = MobilityCtx::new(0, &world, &rng);
                engine.command(
                    &mut ctx,
                    MobilityCommand::SpeedCap {
                        actor: follower,
                        v_mps: Some(v),
                    },
                );
            }
            let mut t = 0u64;
            for step in 0..600usize {
                let mut ctx = MobilityCtx::new(t, &world, &rng);
                engine.step(&mut ctx, params.step);
                if let Some((_, lane, _, _)) = engine
                    .longitudinal_states()
                    .iter()
                    .find(|(a, _, _, _)| *a == follower)
                    .copied()
                    && !right.contains(&lane)
                {
                    return Some(step);
                }
                t += 100 * NS_PER_MS;
            }
            None
        };
        let uncapped = first_change(None);
        let capped = first_change(Some(4.0));
        println!("uncapped changed lane at step {uncapped:?}, capped at {capped:?}");
        // The control pulls out at once: at 25 m/s behind a leader doing 4 m/s the
        // incentive is the whole of a_max.
        assert_eq!(
            uncapped,
            Some(0),
            "the control did not pull out, so the test proves nothing"
        );
        // The capped vehicle must not, while it is braking to the cap: it is about to be
        // held at 4 m/s in either lane, so there is no gain to take. It may change later —
        // once it is settled at 4 m/s a hand's breadth behind a leader doing 4 m/s, an
        // empty lane is a real gain — and it does, at step 331.
        assert!(
            capped.is_none_or(|step| step > 100),
            "a vehicle capped to 4 m/s pulled out at step {capped:?} to overtake a leader \
             doing 4 m/s"
        );
    }

    #[test]
    fn a_blocked_vehicle_changes_lane_and_settles_in_it() {
        // A two-lane ring, a slow leader ahead and an empty lane beside: MOBIL's incentive
        // fires, the engine moves the vehicle into the target lane's frame, and the
        // smoothstep closes the lateral offset to zero.
        let world = ring(&RingParams {
            circumference_m: 800.0,
            segments: 4,
            lanes: 2,
            ..RingParams::default()
        })
        .expect("a ring");
        let right = crate::worlds::ring_cycle(&world, 0);
        let rng = RngRegistry::new(19);
        let params = EngineParams {
            intersections: IntersectionMode::None,
            lane_changes: true,
            dynamic_rerouting: false,
            ..EngineParams::default()
        };
        // Reconsideration every step, as in the two tests above, so the follower decides
        // as soon as it is blocked. With the shipped 0.25/s rate the *leader* drew first
        // (at 4.2 s, MOBIL's politeness term: moving over lets the follower pass), and the
        // follower was only ever blocked again because of the defect this engine no longer
        // has: the leader's route still named the lanes it had left, so at the next
        // segment boundary it jumped straight back into the follower's lane, 3.5 m
        // sideways in one step. That the test passed by that jump is why it is set up so.
        let mut engine = NativeMobility::with_lane_change_params(
            params,
            Arc::new(Idm::new(IdmPreset::Kesting2010)),
            MobilPreset::Kesting2007,
            MobilParams {
                reconsider_rate_per_s: 1e9,
                ..MobilPreset::Kesting2007.params()
            },
        );
        {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine
                .init(&mut ctx, Box::new(NoDemand::new()))
                .expect("init");
        }
        let route: Vec<LaneId> = right
            .iter()
            .cycle()
            .take(right.len() * 8)
            .copied()
            .collect();
        let driver = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
        let leader = engine
            .spawn_with_route(
                &world,
                0,
                VehicleClass::Passenger,
                driver,
                route.clone(),
                60.0,
            )
            .expect("leader");
        let follower = engine
            .spawn_with_route(&world, 0, VehicleClass::Passenger, driver, route, 20.0)
            .expect("follower");
        engine.set_speed(leader, 4.0).expect("speed");
        engine.set_speed(follower, 25.0).expect("speed");

        let mut changed_to: Option<LaneId> = None;
        let mut t = 0u64;
        for _ in 0..600 {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            engine.step(&mut ctx, params.step);
            if let Some((_, lane, _, _)) = engine
                .longitudinal_states()
                .iter()
                .find(|(a, _, _, _)| *a == follower)
                .copied()
                && !right.contains(&lane)
            {
                changed_to = Some(lane);
                break;
            }
            t += 100 * NS_PER_MS;
        }
        let target = changed_to.expect("the follower never changed lane");
        assert_eq!(world.lane(target).index, 1, "it moved to the left lane");
        // Let the transition finish, then the lateral offset must be zero and the vehicle
        // must be free to accelerate again.
        for _ in 0..120 {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            engine.step(&mut ctx, params.step);
            t += 100 * NS_PER_MS;
        }
        let state = Mobility::kinematics(&engine, follower).expect("published");
        let lane_pos = state.lane.expect("on a lane");
        assert!(
            lane_pos.d_m.abs() < 1e-9,
            "the transition settled: lateral {} m",
            lane_pos.d_m
        );
        assert!(
            state.speed_mps() > 10.0,
            "and it is moving freely again: {} m/s",
            state.speed_mps()
        );
    }

    #[test]
    fn lc2013_overtakes_a_slow_leader() {
        // The same ring, with SUMO LC2013's motivations in place of MOBIL: the speed-gain
        // accumulator takes the follower left past the slow leader.
        let world = ring(&RingParams {
            circumference_m: 800.0,
            segments: 4,
            lanes: 2,
            ..RingParams::default()
        })
        .expect("a ring");
        let right = crate::worlds::ring_cycle(&world, 0);
        let rng = RngRegistry::new(19);
        let params = EngineParams {
            intersections: IntersectionMode::None,
            lane_changes: true,
            dynamic_rerouting: false,
            lc2013: Some(crate::lanechange::lc2013::Lc2013Params::default()),
            ..EngineParams::default()
        };
        let mut engine = NativeMobility::new(params);
        {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine
                .init(&mut ctx, Box::new(NoDemand::new()))
                .expect("init");
        }
        let route: Vec<LaneId> = right
            .iter()
            .cycle()
            .take(right.len() * 8)
            .copied()
            .collect();
        let driver = IdmPreset::Kesting2010.profile(VehicleClass::Passenger);
        let leader = engine
            .spawn_with_route(
                &world,
                0,
                VehicleClass::Passenger,
                driver,
                route.clone(),
                60.0,
            )
            .expect("leader");
        let follower = engine
            .spawn_with_route(&world, 0, VehicleClass::Passenger, driver, route, 20.0)
            .expect("follower");
        engine.set_speed(leader, 4.0).expect("speed");
        engine.set_speed(follower, 25.0).expect("speed");
        let mut went_left = false;
        let mut t = 0u64;
        for _ in 0..1200 {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            engine.step(&mut ctx, params.step);
            t += 100 * NS_PER_MS;
            if engine
                .longitudinal_states()
                .iter()
                .any(|s| s.0 == follower && !right.contains(&s.1))
            {
                went_left = true;
                break;
            }
        }
        assert!(went_left, "LC2013 never overtook the slow leader");
    }

    #[test]
    fn the_card_validates_and_declares_its_domains() {
        let engine = NativeMobility::new(EngineParams::default());
        engine.card().validate().expect("validates");
        assert!(engine.card().determinism.uses_rng);
        assert!(
            engine
                .card()
                .determinism
                .rng_domains
                .contains(&"lane-change".to_string())
        );
        assert_eq!(engine.tier(), Tier::Medium);
        let legacy = NativeMobility::legacy(EngineParams::default());
        assert_eq!(legacy.tier(), Tier::Abstract);
    }

    // ---- WP-08: rule violations and crash states ------------------------------------

    /// A fixed list of trips, released at their times: the arrivals of [`approach_run`].
    struct ArrivalList {
        inner: NoDemand,
        trips: Vec<TripRequest>,
        next: usize,
    }

    impl ArrivalList {
        fn new(trips: Vec<TripRequest>) -> Self {
            Self {
                inner: NoDemand::new(),
                trips,
                next: 0,
            }
        }
    }

    impl Model for ArrivalList {
        fn card(&self) -> &v2xw_core::card::ModelCard {
            self.inner.card()
        }
    }

    impl crate::traits::Demand for ArrivalList {
        fn spawns_in(
            &mut self,
            _ctx: &mut dyn MobCtx,
            _from: SimTime,
            to: SimTime,
        ) -> Vec<TripRequest> {
            let mut out = Vec::new();
            while self.next < self.trips.len() && self.trips[self.next].t < to {
                out.push(self.trips[self.next].clone());
                self.next += 1;
            }
            out
        }
    }

    /// What one run of Poisson arrivals on a signalised approach produced.
    struct ApproachRun {
        /// Vehicles that entered the junction from the approach.
        entries: u64,
        /// Of those, entries on red, with how long the red had shown, seconds.
        into_red_s: Vec<f64>,
        /// The auditor fed the engine's own tags.
        tagged: crate::audit::AuditReport,
        /// The auditor fed the same states with every tag stripped: the fault "an untagged
        /// red-light crossing", which must be caught as a defect.
        untagged: crate::audit::AuditReport,
        /// Every published state, for the identity checks.
        states: Vec<Vec<(ActorId, Kinematics)>>,
        /// The signal cycle, seconds.
        cycle_s: f64,
    }

    fn approach_run(
        params: EngineParams,
        rate_veh_per_h: f64,
        duration_s: f64,
        seed: u64,
    ) -> ApproachRun {
        approach_run_on(
            GridParams::legacy().with_signals(true),
            params,
            rate_veh_per_h,
            duration_s,
            seed,
            true,
        )
    }

    /// [`approach_run`] on a grid of `grid` (its signal cycle, say), keeping every
    /// published state only when `keep_states` is set: a day of steps is too many to hold.
    fn approach_run_on(
        grid: GridParams,
        params: EngineParams,
        rate_veh_per_h: f64,
        duration_s: f64,
        seed: u64,
        keep_states: bool,
    ) -> ApproachRun {
        let world =
            v2xw_world::procedural::grid(&grid, &ImportOptions::default()).expect("grid");
        let (approach, beyond) = signalised_approach(&world);
        let rng = RngRegistry::new(seed);
        let junction = world.edge(world.lane(approach).edge).to;
        let plan_id = match world.roads.junction(junction).control {
            JunctionControl::Signalised { plan } => plan,
            _ => unreachable!("chosen above"),
        };
        let plan = world.signal_plan(plan_id).expect("a plan");
        let cycle_s: f64 = plan.phases.iter().map(|p| p.duration_s).sum();
        let movement = world
            .successors(approach)
            .iter()
            .find(|c| c.to_lane == beyond)
            .and_then(|c| c.via)
            .expect("an internal connector");
        let signals = FixedTimeSignals::default();
        // Poisson arrivals from a fixed hash, so the arrival stream is the same whatever
        // the engine draws.
        let mut arrivals = Vec::new();
        let mut t_s = 0.0;
        let mut k = 0u64;
        while t_s < duration_s {
            let h = crate::drivers::splitmix64(seed ^ k.wrapping_mul(0x9e37_79b9));
            let u = (h >> 11) as f64 / (1u64 << 53) as f64;
            t_s += -math::ln(1.0 - u.min(1.0 - 1e-12)) * 3600.0 / rate_veh_per_h;
            arrivals.push(secs_to_ns(t_s));
            k += 1;
        }
        // The arrivals are offered as demand, so a trip whose origin has no room waits in
        // the engine's insertion queue (SUMO's `--max-depart-delay`) as every scenario's
        // trips do. Offered as `MobilityCommand::Spawn`, a refused trip was simply lost:
        // with every car entering at rest 5 m into the lane, the Poisson clusters lost about
        // 40 % of the arrivals and the hour put only 355 vehicles through the junction.
        let trips: Vec<TripRequest> = arrivals
            .iter()
            .enumerate()
            .map(|(i, &at)| TripRequest {
                seq: i as u64,
                t: at,
                origin: approach,
                origin_s_m: 5.0,
                destination: beyond,
                class: VehicleClass::Passenger,
                desired_speed_mps: 13.89,
            })
            .collect();
        let mut engine = NativeMobility::new(params);
        {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine
                .init(&mut ctx, Box::new(ArrivalList::new(trips)))
                .expect("init");
        }
        let audit = crate::audit::AuditParams::default();
        let mut tagged = crate::audit::TrafficAuditor::new(&world, audit);
        let mut untagged = crate::audit::TrafficAuditor::new(&world, audit);
        let mut run = ApproachRun {
            entries: 0,
            into_red_s: Vec::new(),
            tagged: crate::audit::AuditReport::default(),
            untagged: crate::audit::AuditReport::default(),
            states: Vec::new(),
            cycle_s,
        };
        let mut red_since: Option<SimTime> = None;
        let mut on_approach: BTreeSet<ActorId> = BTreeSet::new();
        let mut t = 0;
        let end = secs_to_ns(duration_s);
        while t < end {
            let mut ctx = MobilityCtx::new(t, &world, &rng);
            let state = signals
                .state_for(plan, movement, ns_to_secs(t))
                .expect("controlled");
            if state == SignalState::Red {
                red_since.get_or_insert(t);
            } else {
                red_since = None;
            }
            let update = engine.step(&mut ctx, params.step);
            let t1 = t + params.step.as_nanos();
            let actors = engine.audit_actors(&world, t1);
            for a in &actors {
                if a.lane == approach {
                    on_approach.insert(a.actor);
                } else if on_approach.remove(&a.actor) {
                    run.entries += 1;
                    if let Some(since) = red_since {
                        run.into_red_s.push(ns_to_secs(t - since));
                    }
                }
            }
            tagged.observe(&world, t, t1, &actors, &update.despawned);
            let stripped: Vec<crate::audit::AuditActor> = actors
                .iter()
                .map(|a| crate::audit::AuditActor { intent: 0, ..*a })
                .collect();
            untagged.observe(&world, t, t1, &stripped, &update.despawned);
            if keep_states {
                run.states.push(update.states);
            }
            t = t1;
        }
        run.tagged = tagged.report();
        run.untagged = untagged.report();
        run
    }

    fn violations_on() -> EngineParams {
        EngineParams {
            intersections: IntersectionMode::SignalsOnly,
            lane_changes: false,
            violations: crate::drivers::ViolationParams {
                enabled: true,
                ..crate::drivers::ViolationParams::default()
            },
            ..EngineParams::default()
        }
    }

    /// With violations on, red-light running on a signalised approach sits in the field
    /// band (Bonneson, Zimmerman & Quiroga, TTI 0-4196-P1, Table 1: 3.0-5.3 violations per
    /// 1,000 vehicles, 1.0 per 10,000 vehicle-cycles), entries come early in the red
    /// (median under 0.5 s, Figure 4; none beyond 4.0 s, Table 3), every one of them is
    /// tagged, and the same entries untagged are caught as defects.
    ///
    /// The approach runs a 90 s cycle for two days. The guide's 1.0 per 10,000
    /// vehicle-cycles "equates to 3 to 6 violations per 1000 vehicles for cycle lengths in
    /// the range of 60 to 120 s" — 360/C per 1,000, so 6.0 at 60 s, the band's upper edge,
    /// and 4.0 at 90 s, its middle. The test ran the grid's 60 s cycle for an hour: the
    /// model's own expectation sat on the edge, and four entries (6.72 per 1,000, median
    /// of four 0.60 s) could not judge a rate or a median.
    #[test]
    fn red_light_running_sits_in_the_field_band_and_untagged_it_is_a_defect() {
        let duration_s = 48.0 * 3600.0;
        let grid = GridParams {
            cycle_s: 90.0,
            ..GridParams::legacy().with_signals(true)
        };
        let run = approach_run_on(grid, violations_on(), 600.0, duration_s, 21, false);
        let v = run.into_red_s.len() as f64;
        let n = run.entries as f64;
        let per_1000_veh = 1000.0 * v / n;
        let cycles = duration_s / run.cycle_s;
        let per_1000_cycles = 1000.0 * v / cycles;
        let per_10k_veh_cycles = per_1000_veh * run.cycle_s / 360.0;
        let mut into = run.into_red_s.clone();
        into.sort_by(f64::total_cmp);
        let median = into.get(into.len() / 2).copied().unwrap_or(f64::NAN);
        eprintln!(
            "cycle {:.0} s, {n} entries, {v} on red: {per_1000_veh:.2} per 1,000 veh, \
             {per_1000_cycles:.1} per 1,000 cycles, {per_10k_veh_cycles:.2} per 10,000 \
             veh-cycles; into red median {median:.2} s, max {:.2} s; tagged {:?}",
            run.cycle_s,
            into.last().copied().unwrap_or(0.0),
            (
                run.tagged.stats.intended_red_entries,
                run.tagged.count(crate::audit::Check::RedEntry)
            )
        );
        let (lo, hi) = crate::drivers::RED_RUN_PER_1000_VEH_BAND;
        assert!(n > 10_000.0, "too few entries to judge a rate: {n}");
        assert!(v >= 30.0, "too few red entries to judge a median: {v}");
        assert!(
            (lo..=hi).contains(&per_1000_veh),
            "{per_1000_veh:.2} red entries per 1,000 vehicles, outside the field band {lo}-{hi}"
        );
        assert!(
            (0.5..=2.0).contains(&per_10k_veh_cycles),
            "{per_10k_veh_cycles:.2} per 10,000 veh-cycles, against Texas's 1.0"
        );
        assert!(
            median < 0.5,
            "median entry {median:.2} s into red (field: under 0.5 s)"
        );
        assert!(
            into.iter()
                .all(|x| *x <= crate::drivers::RED_RUN_TOLERANCE_MAX_S),
            "an entry later than 4.0 s into red"
        );
        // Every one tagged: behaviour, not a defect.
        assert_eq!(run.tagged.count(crate::audit::Check::RedEntry), 0);
        assert_eq!(run.tagged.stats.intended_red_entries, v as u64);
        // The fault: the same entries without their tag are red-entry defects.
        assert_eq!(run.untagged.count(crate::audit::Check::RedEntry), v as u64);
        assert!(v > 0.0);
    }

    /// With violations off — the default, or `enabled: false` whatever else the block
    /// says — the published states are byte-identical to a run without the block; and a
    /// run with violations on is reproducible.
    #[test]
    fn violations_off_change_nothing_and_on_is_deterministic() {
        let base = EngineParams {
            intersections: IntersectionMode::SignalsOnly,
            lane_changes: false,
            ..EngineParams::default()
        };
        let off = EngineParams {
            violations: crate::drivers::ViolationParams {
                enabled: false,
                red_run_share: 1.0,
                fail_to_yield_share: 1.0,
                amber_tail_max_s: 12.0,
                ..crate::drivers::ViolationParams::default()
            },
            ..base
        };
        let inert = EngineParams {
            violations: crate::drivers::ViolationParams {
                enabled: true,
                red_run_share: 0.0,
                amber_tail_max_s: AMBER_DILEMMA_TTI_S.1,
                ..crate::drivers::ViolationParams::default()
            },
            ..base
        };
        let a = approach_run(base, 600.0, 3600.0, 5);
        let b = approach_run(off, 600.0, 3600.0, 5);
        let c = approach_run(inert, 600.0, 3600.0, 5);
        assert!(a.states == b.states, "enabled: false moved a state");
        assert!(
            a.states == c.states,
            "violations on with every violation zero moved a state"
        );
        let on1 = approach_run(violations_on(), 600.0, 3600.0, 5);
        let on2 = approach_run(violations_on(), 600.0, 3600.0, 5);
        assert!(on1.states == on2.states, "same seed, different states");
        assert!(on1.states != a.states, "violations on changed nothing");
    }

    /// A collision resolves to a crash state: both vehicles stop where they are, the lane
    /// is blocked (the car behind stops short of them), and after the clearance time both
    /// leave with `Collision`. A crash a tagged violation caused is behaviour to the
    /// auditor; the same crash between untagged vehicles is still an overlap defect.
    ///
    /// The tagged vehicle is a red runner as the engine leaves one — committed to the
    /// junction ahead while it shows red, tag set — so the decision pass keeps its tag. A
    /// bare tag on a vehicle committed to nothing is cleared by that pass, as it must be: a
    /// driver who is not violating anything is untagged before the collision check reads it.
    #[test]
    fn a_collision_becomes_a_crash_that_blocks_the_lane_until_cleared() {
        let world = grid(true);
        let (approach, beyond) = signalised_approach(&world);
        let route = vec![approach, beyond];
        let rng = RngRegistry::new(3);
        // The first step at which the approach shows red, and the junction it leads to.
        let junction = world.edge(world.lane(approach).edge).to;
        let plan = match world.roads.junction(junction).control {
            JunctionControl::Signalised { plan } => world.signal_plan(plan).expect("a plan"),
            _ => unreachable!("chosen signalised"),
        };
        let movement = world
            .successors(approach)
            .iter()
            .find(|c| c.to_lane == beyond)
            .and_then(|c| c.via)
            .expect("an internal connector");
        let step_ns = EngineParams::default().step.as_nanos();
        let t0 = (0..)
            .map(|k| k * step_ns)
            .find(|t| {
                FixedTimeSignals::default().state_for(plan, movement, ns_to_secs(*t))
                    == Some(SignalState::Red)
            })
            .expect("a red");
        // Within the junction look-ahead of the line, so the commitment is read.
        let line = world.lane(approach).length_m;
        let params = EngineParams {
            intersections: IntersectionMode::SignalsOnly,
            lane_changes: false,
            violations: crate::drivers::ViolationParams {
                enabled: true,
                crash_clearance_s: 5.0,
                ..crate::drivers::ViolationParams::default()
            },
            ..EngineParams::default()
        };
        for tag in [false, true] {
            let mut engine = engine_on(&world, params, &rng);
            let driver = engine.driver_for(VehicleClass::Passenger);
            let car = VehicleClass::Passenger;
            let a = engine
                .spawn_with_route(&world, t0, car, driver, route.clone(), line - 28.0)
                .expect("spawn");
            let b = engine
                .spawn_with_route(&world, t0, car, driver, route.clone(), line - 30.0)
                .expect("spawn");
            let c = engine
                .spawn_with_route(&world, t0, car, driver, route.clone(), line - 60.0)
                .expect("spawn");
            engine.set_speed(b, 8.0).expect("speed");
            engine.set_speed(c, 8.0).expect("speed");
            if tag {
                let x = engine.actors.get_mut(&b).expect("b");
                x.violation = crate::audit::intent::RED_RUN;
                x.amber_commit = Some(junction);
            }
            let mut auditor =
                crate::audit::TrafficAuditor::new(&world, crate::audit::AuditParams::default());
            let mut gone = Vec::new();
            let mut t = t0;
            while t < t0 + secs_to_ns(12.0) {
                let mut ctx = MobilityCtx::new(t, &world, &rng);
                let update = engine.step(&mut ctx, params.step);
                let t1 = t + params.step.as_nanos();
                let actors = engine.audit_actors(&world, t1);
                auditor.observe(&world, t, t1, &actors, &update.despawned);
                if t == t0 {
                    for id in [a, b] {
                        let x = &engine.actors[&id];
                        assert!(x.crash.is_some(), "{id:?} did not crash");
                        assert_eq!(x.speed_mps, 0.0);
                        assert_eq!(x.crash.map(|k| k.by_violation), Some(tag));
                    }
                    assert!(
                        engine.broken_down_lanes().contains(&approach),
                        "lane not blocked"
                    );
                }
                gone.extend(update.despawned.iter().copied());
                t = t1;
            }
            assert!(gone.contains(&(a, DespawnCause::Collision)), "{gone:?}");
            assert!(gone.contains(&(b, DespawnCause::Collision)), "{gone:?}");
            assert!(
                !gone.iter().any(|(id, _)| *id == c),
                "the car behind left: {gone:?}"
            );
            // The car behind stopped short of the crash and never touched it.
            assert!(
                engine.actors[&c].crash.is_none(),
                "the car behind crashed into it"
            );
            let r = auditor.report();
            if tag {
                assert_eq!(r.count(crate::audit::Check::Overlap), 0, "{:?}", r.examples);
                assert!(r.stats.crash_overlap_steps > 0);
                assert_eq!(r.stats.crashes_cleared, 2);
                assert_eq!(r.count(crate::audit::Check::MidRoadDespawn), 0);
            } else {
                assert!(
                    r.count(crate::audit::Check::Overlap) > 0,
                    "an untagged crash went unflagged"
                );
                assert_eq!(r.stats.crashes_cleared, 0);
                assert_eq!(r.count(crate::audit::Check::MidRoadDespawn), 2);
            }
        }
    }
}

/// The model a vehicle drives: its type's own when it has one, else the engine's.
struct VehicleCf<'a>(
    &'a Arc<dyn CarFollowing + Send + Sync>,
    &'a BTreeMap<ActorId, Arc<dyn CarFollowing + Send + Sync>>,
);

impl VehicleCf<'_> {
    fn accel(
        &self,
        ego: &crate::views::VehicleView,
        leader: Option<&LeaderView>,
        lane: &LaneView,
        w: &WeatherState,
    ) -> f64 {
        self.1.get(&ego.actor).unwrap_or(self.0).accel(ego, leader, lane, w)
    }
}
