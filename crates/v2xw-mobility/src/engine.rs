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
use v2xw_core::time::{Duration, SimTime, ns_to_secs};
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
use crate::routing::dynamic::{DynamicCost, DynamicReroute};
use crate::snapshot::{ActorSnapshot, NeighborOptions};
use crate::traits::{
    CarFollowing, ClockModel, Demand, IntersectionControl, LaneChange, Mobility, VruMobility,
};
use crate::views::{
    ActorSpawn, ConflictView, DespawnCause, DriverProfile, EntryDecision, JunctionView,
    LaneChangeDecision, LaneNeighbors, LaneView, LeaderView, MobilityCommand, MobilityUpdate,
    PhaseState, Route, Side, TripRequest, VehicleView,
};
use crate::vru::crosswalk::{CrosswalkIndex, VehiclePath};
use crate::vru::social_force::SocialForce;
use v2xw_core::rng::{EntityRef, RngDomain};

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

/// The model id a driver's own traits are drawn under ([`DriverTraits`]).
const DRIVER_TRAITS_ID: &str = "mobility/native/driver";

/// The median of a driver's reaction time, seconds.
///
/// Taoka 1989 (TRR 1213, "An analytical model for driver response") fits a lognormal to
/// published measurements of drivers responding to a signal change — the amber onset —
/// and reports a median of 1.1-1.2 s, a mean of 1.3-1.4 s and a standard deviation of
/// 0.55-0.75 s (**secondary**: read in the paper's abstract and a search excerpt, not the
/// full text). 1.15 s is the middle of the median range.
pub const REACTION_MEDIAN_S: f64 = 1.15;

/// The lognormal shape of a driver's reaction time: σ = 0.5 gives a mean of 1.30 s and a
/// standard deviation of 0.69 s with the median above, inside Taoka's ranges.
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

/// What makes one driver unlike another, beyond the car-following parameters.
///
/// * `speed_factor` — the speed the driver chooses on a free road as a multiple of the
///   posted limit: SUMO's `speedFactor`, drawn per vehicle from `normc(1, speedDev, 0.2,
///   2)` with the class's `speedDev` (0.1 for a passenger car) — the SUMO vType default
///   (R10 §B4), and the same law SUMO applies to every lane limit. No New York field
///   distribution of free speeds against the 25 mph limit was available to calibrate it.
/// * `reaction_s` — how long the driver takes to move off once a signal or a crosswalk
///   that held them lets them go: lognormal with median [`REACTION_MEDIAN_S`] and shape
///   [`REACTION_SIGMA`] (Taoka 1989), clamped to 0.3-3 s.
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
}

impl DriverTraits {
    /// The limit exactly and no reaction time: the model without heterogeneity.
    pub const NEUTRAL: DriverTraits = DriverTraits {
        speed_factor: 1.0,
        reaction_s: 0.0,
        follow_reaction_s: 0.0,
        amber_go_tti_s: 0.0,
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
        let key = EntityRef::custom(DRIVER_TRAITS_ID, seq);
        let speed_factor = {
            let mut rng = ctx.rng(RngDomain::DesiredSpeed, key);
            rng.normal(1.0, class.spec().speed_dev).clamp(0.2, 2.0)
        };
        // One standard-normal draw sets how quick this driver is, for both responses.
        let z = ctx.rng(RngDomain::ReactionTime, key).normal(0.0, 1.0);
        let reaction_s =
            math::exp(math::ln(REACTION_MEDIAN_S) + REACTION_SIGMA * z).clamp(0.3, 3.0);
        let follow_reaction_s =
            math::exp(math::ln(follow_median_s.max(0.05)) + REACTION_SIGMA * z).clamp(0.2, 3.0);
        // The amber threshold: logistic across the dilemma zone, 10 % at its near edge
        // and 90 % at its far edge (median 4.0 s, scale 1.5 s / ln 9), clamped to it.
        let amber_go_tti_s = {
            let (near, far) = AMBER_DILEMMA_TTI_S;
            let u = ctx
                .rng(RngDomain::ReactionTime, EntityRef::custom(AMBER_TRAIT_ID, seq))
                .uniform(0.0, 1.0)
                .clamp(1e-9, 1.0 - 1e-9);
            let median = 0.5 * (near + far);
            let scale = 0.5 * (far - near) / math::ln(9.0);
            (median + scale * math::ln(u / (1.0 - u))).clamp(near, far)
        };
        DriverTraits {
            speed_factor,
            reaction_s,
            follow_reaction_s,
            amber_go_tti_s,
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

/// How far apart two connectors from one approach lane must be before a cyclist on one no
/// longer occupies the other, metres: half a truck (2.6 m) plus half a bicycle (0.65 m)
/// plus a margin, rounded up.
const DIVERGED_M: f64 = 2.5;

/// How many walkable lanes a pedestrian's walk strings together at most.
const PEDESTRIAN_WALK_LANES: usize = 12;

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
    }
}

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
    /// **A test hook, not a model parameter.** Walks the decision pass in reverse actor
    /// order. Because the pass reads only the frozen snapshot, the published result must be
    /// bit-identical either way; that is the ADR 0004 Jacobi property, and this is how the
    /// crate tests it.
    pub reverse_order: bool,
}

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
            junction_clearance: true,
            crosswalk_yield: true,
            driver_heterogeneity: true,
            right_turn_on_red: false,
            queue_start_delay_median_s: FOLLOW_REACTION_MEDIAN_S,
            reverse_order: false,
        }
    }
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
    /// The driver's own speed factor and reaction time.
    traits: DriverTraits,
    /// While standing: when what held it first let it go (the start of its reaction).
    release_at: Option<SimTime>,
}

impl Actor {
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
        let front = smooth_offset_point(lane, s_front, self.lateral_m);
        let s_rear = self.s_m - length;
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
                body + lateral_heading_offset(lateral_rate, self.speed_mps),
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
        let lane = world.lane(self.lane);
        if s_rear >= 0.0 {
            return (
                smooth_offset_point(lane, s_rear, self.lateral_m),
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
                return (smooth_offset_point(prev, s, self.lateral_m), prev.id, s);
            }
            behind -= prev.length_m;
            earliest = prev;
        }
        let start = earliest.offset_point(0.0, self.lateral_m);
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
            if dur <= 0.0 {
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
    lane_change: Option<Mobil>,
    gap: GapAcceptance,
    signals: FixedTimeSignals,
    two_coloring: Option<TwoColoring>,
    router: DynamicReroute,
    /// Routes riders over the lanes that admit bicycles.
    bike_router: DynamicReroute,
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
    diverging: BTreeMap<LaneId, Vec<(LaneId, f64)>>,
    vru: Option<SocialForce>,
    clock: Option<Box<dyn ClockModel>>,
    actors: BTreeMap<ActorId, Actor>,
    published: BTreeMap<ActorId, Kinematics>,
    closed: BTreeSet<LaneId>,
    /// How many times a closure command has changed [`Self::closed`]: the router's cost
    /// generation ([`DynamicCost::with_generation`]). Counting the closed lanes instead
    /// missed a reopen and a close in the same step.
    closure_changes: u64,
    pending: Vec<MobilityCommand>,
    /// Trips whose origin had no room when they arrived, with the instant they first
    /// asked, oldest first (not in the legacy parity mode).
    waiting: Vec<(TripRequest, SimTime, Option<ActorId>)>,
    weather: WeatherState,
    /// Every junction's conflict zones, built at `init` (not in the legacy parity mode).
    zones: ConflictZones,
    /// The speed each junction connector may be taken at, from its curvature.
    turn_speed: BTreeMap<LaneId, f64>,
    next_actor: u32,
    next_seq: u64,
    dropped_trips: u64,
    card: ModelCard,
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
        let router = if params.dynamic_rerouting {
            DynamicReroute::on_change(router_params)
        } else {
            DynamicReroute::new(router_params, crate::views::ReroutePolicy::STATIC)
        };
        Self {
            card: card(&params, &cf.card().id),
            params,
            cf,
            lane_change: params.lane_changes.then_some(mobil),
            gap: GapAcceptance::new(GapAcceptanceParams::default()),
            signals: FixedTimeSignals::new(SignalPlanParams {
                right_turn_on_red: params.right_turn_on_red,
                ..SignalPlanParams::default()
            }),
            two_coloring: None,
            router,
            bike_router: DynamicReroute::new(
                DijkstraParams::for_classes(ClassMask::BICYCLE),
                crate::views::ReroutePolicy::STATIC,
            ),
            demand: None,
            vru_population: VruPopulation::default(),
            walkable: Vec::new(),
            bike_lanes: Vec::new(),
            crosswalks: CrosswalkIndex::default(),
            diverging: BTreeMap::new(),
            vru: None,
            clock: None,
            actors: BTreeMap::new(),
            published: BTreeMap::new(),
            closed: BTreeSet::new(),
            closure_changes: 0,
            pending: Vec::new(),
            waiting: Vec::new(),
            weather: WeatherState::CLEAR,
            zones: ConflictZones::default(),
            turn_speed: BTreeMap::new(),
            next_actor: 0,
            next_seq: 0,
            dropped_trips: 0,
        }
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

    /// The router for a class: riders on the bicycle lane graph, everyone else on the
    /// engine's own.
    fn router_for(&self, class: VehicleClass) -> &DynamicReroute {
        if class == VehicleClass::Bicycle {
            &self.bike_router
        } else {
            &self.router
        }
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

    /// How many trips were dropped because their origin was occupied or unroutable.
    pub fn dropped_trips(&self) -> u64 {
        self.dropped_trips
    }

    /// The pedestrian model, if one was given.
    pub fn vru(&self) -> Option<&SocialForce> {
        self.vru.as_ref()
    }

    /// The pedestrian model, mutably.
    pub fn vru_mut(&mut self) -> Option<&mut SocialForce> {
        self.vru.as_mut()
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
                }
            })
            .collect()
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
                lateral_m: 0.0,
                speed_mps: 0.0,
                accel_mps2: 0.0,
                spawned: t,
                planned_at: t,
                planned_generation: 0,
                transition: None,
                cooldown_until: t,
                speed_cap_mps: None,
                stopped_until: None,
                trail: Vec::new(),
                amber_commit: None,
                amber_seen: None,
                traits: DriverTraits::NEUTRAL,
                release_at: None,
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
        let route = crate::routing::dijkstra::Dijkstra::new(params)
            .search_nearest(world, from, dest.end(), t, costs)?;
        let to = *route.lanes.last()?;
        Some((route, to))
    }

    /// Gives every vehicle whose route runs into a closed lane a way round it
    /// ([`Self::detour`]). With dynamic rerouting on, drivers know of a closure anywhere on
    /// their route (a navigation service); with it off, only once it is on one of the
    /// next [`CLOSURE_SIGHT_LANES`] lanes of their route — at the junction they are
    /// approaching, where the barrier is in sight.
    fn closure_detours(&mut self, world: &World, t: SimTime, costs: &DynamicCost<'_>) {
        let sight = if self.params.dynamic_rerouting {
            usize::MAX
        } else {
            CLOSURE_SIGHT_LANES
        };
        let cut: Vec<ActorId> = self
            .actors
            .values()
            .filter(|a| {
                a.route
                    .lanes
                    .iter()
                    .skip(a.route_index + 1)
                    .take(sight)
                    .any(|l| self.closed.contains(l))
            })
            .map(|a| a.id)
            .collect();
        let generation = costs.generation();
        for id in cut {
            let (lane, destination, class) = {
                let a = &self.actors[&id];
                (a.lane, a.destination, a.class)
            };
            let found = Self::detour(self.router_for(class), world, lane, destination, t, costs);
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

    /// The distance from the vehicle's front to the start of the first closed lane on its
    /// route within the look-ahead, metres, if there is one.
    fn closure_gap(&self, world: &World, actor: &Actor) -> Option<f64> {
        let mut dist = world.lane(actor.lane).length_m - actor.s_m;
        for lane in actor.route.lanes.iter().skip(actor.route_index + 1) {
            if dist > self.params.lookahead_m {
                return None;
            }
            if self.closed.contains(lane) {
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
                None
            }
        }
    }

    /// Routes a trip and puts it on the road if its origin has room now, as actor
    /// `reserved` if the trip already holds an id, else as the next one.
    /// The traits the driver of `trip` gets: drawn from the trip's own streams, or
    /// [`DriverTraits::NEUTRAL`] in the legacy parity mode or with heterogeneity off.
    fn traits_for(&self, ctx: &dyn MobCtx, trip: &TripRequest) -> DriverTraits {
        if self.along_path() && self.params.driver_heterogeneity {
            DriverTraits::draw_with(
                ctx,
                trip.seq,
                trip.class,
                self.params.queue_start_delay_median_s,
            )
        } else {
            DriverTraits::NEUTRAL
        }
    }

    fn try_insert(
        &mut self,
        world: &World,
        trip: &TripRequest,
        now: SimTime,
        reserved: Option<ActorId>,
        traits: DriverTraits,
    ) -> Insertion {
        let costs = self.costs(world);
        let Some(route) = self.router_for(trip.class).replan(
            world,
            trip.origin,
            trip.destination,
            trip.t,
            &costs,
        ) else {
            return Insertion::Unroutable;
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
        let front = trip.origin_s_m.max(length);
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
            if a.lane != trip.origin {
                continue;
            }
            let ahead = (a.s_m - a.class.spec().length_m) - front;
            let behind = (front - length) - a.s_m;
            if ahead.max(behind) < self.params.insertion_gap_m {
                return Insertion::Occupied;
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
        let driver = DriverProfile {
            desired_speed_mps: trip.desired_speed_mps,
            ..self.driver_for(trip.class)
        };
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
                lateral_m: 0.0,
                speed_mps: 0.0,
                accel_mps2: 0.0,
                spawned: trip.t.max(now),
                planned_at: trip.t,
                planned_generation: 0,
                transition: None,
                cooldown_until: trip.t,
                speed_cap_mps: None,
                stopped_until: None,
                trail: Vec::new(),
                amber_commit: None,
                amber_seen: None,
                traits,
                release_at: None,
            },
        );
        self.next_seq = self.next_seq.max(trip.seq + 1);
        Insertion::Placed(id)
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
        let a = self.cf.accel(
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
                        let next: Vec<LaneId> = world
                            .successor_lanes(last)
                            .into_iter()
                            .filter(|l| SocialForce::is_walkable(world, *l) && !route.contains(l))
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
                let placed = vru.spawn(ctx, id, route.clone(), s_m).is_ok();
                if placed && let Some(p) = vru.get(id) {
                    let world = ctx.world();
                    let mut k = Kinematics::at_rest(now, p.position(world));
                    k.dims = VehicleClass::Pedestrian.dims();
                    k.heading_rad = world.lane(p.lane).heading_at(p.s_m);
                    spawned.push(ActorSpawn {
                        actor: id,
                        t: now,
                        class: VehicleClass::Pedestrian,
                        kinematics: k,
                        route: Self::route_of(world, route),
                        driver: DriverProfile {
                            desired_speed_mps: p.desired_speed_mps,
                            max_accel_mps2: 0.0,
                            comfort_decel_mps2: 0.0,
                            time_headway_s: 0.0,
                            min_gap_m: 0.0,
                        },
                        seq: self.next_seq,
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
                    desired_speed_mps: bicycle_driver().desired_speed_mps,
                };
                self.next_seq += 1;
                let traits = self.traits_for(&*ctx, &trip);
                let world = ctx.world();
                if let Insertion::Placed(id) = self.try_insert(world, &trip, now, None, traits) {
                    spawned.push(self.actor_spawn(world, id, now));
                    missing -= 1;
                }
            }
        }
    }

    /// The [`ActorSpawn`] announcing an actor this engine has just inserted.
    ///
    /// One builder for both spawn paths — commanded and demand-driven — so the two cannot
    /// announce different things about the same kind of event.
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
        }
    }

    /// The driver profile a class gets: the installed car-following model's own
    /// calibration.
    fn driver_for(&self, class: VehicleClass) -> DriverProfile {
        if class == VehicleClass::Bicycle {
            return bicycle_driver();
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
            // A cyclist on a junction connector still occupies the start of every sibling
            // connector leaving the same approach lane, until the two paths are apart: a
            // car turning right from behind a cyclist going straight on would otherwise
            // not see it, because the cyclist is not on the car's route — and a bicycle
            // accelerates and cruises slower than the car behind, so the car drove into
            // it where the connectors diverge (the pedestrian_invariants gate measured
            // it). Registering every class this way is the general fix; it is limited to
            // bicycles so that motor traffic, which the auditor holds at zero overlaps
            // without it, keeps its calibrated behaviour and its golden records.
            for a in self
                .actors
                .values()
                .filter(|a| a.class == VehicleClass::Bicycle)
            {
                let Some(siblings) = self.diverging.get(&a.lane) else {
                    continue;
                };
                let rear = a.s_m - a.class.spec().length_m;
                for (sibling, apart_s) in siblings {
                    if rear < *apart_s {
                        snapshot.push_ghost(*sibling, a.s_m, a.id);
                    }
                }
            }
            snapshot.sort();
        }
        snapshot
    }

    /// For every junction connector, the other connectors that leave the same approach
    /// lane, each with the arc length beyond which the two centrelines are at least
    /// [`DIVERGED_M`] apart.
    fn diverging_connectors(world: &World) -> BTreeMap<LaneId, Vec<(LaneId, f64)>> {
        let mut by_approach: BTreeMap<LaneId, Vec<LaneId>> = BTreeMap::new();
        for c in world.roads.connections() {
            if let Some(via) = c.via {
                let v = by_approach.entry(c.from_lane).or_default();
                if !v.contains(&via) {
                    v.push(via);
                }
            }
        }
        let mut out: BTreeMap<LaneId, Vec<(LaneId, f64)>> = BTreeMap::new();
        for connectors in by_approach.values() {
            for a in connectors {
                for b in connectors {
                    if a == b {
                        continue;
                    }
                    let (la, lb) = (world.lane(*a), world.lane(*b));
                    let end = la.length_m.min(lb.length_m);
                    let mut s = 0.0;
                    while s < end && la.point_at(s).distance_2d(lb.point_at(s)) < DIVERGED_M {
                        s += 0.5;
                    }
                    out.entry(*a).or_default().push((*b, s.min(end)));
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
            let route = if clash || alongside {
                None
            } else {
                self.route_after_change(world, actor, to, t, &costs)
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
                && list
                    .iter()
                    .any(|(other, rear, _)| *other != actor.id && z.other_not_clear(*rear))
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
                        (true, true) => {
                            d_other < d_self || (d_other == d_self && other < actor.id)
                        }
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
        let k = plan.controlled.iter().position(|l| Some(*l) == movement_lane)?;
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

    /// The speed a junction connector is driven at: its turn speed, or its limit.
    fn connector_speed(&self, world: &World, connector: LaneId) -> f64 {
        let limit = world.lane(connector).speed_limit_mps;
        self.turn_speed.get(&connector).map_or(limit, |v| v.min(limit))
    }

    /// The highest speed the vehicle should be doing now so that, braking at its
    /// comfortable deceleration, it reaches each of the next lanes of its route at no more
    /// than that lane's limit (or its turn speed): `sqrt(v_next² + 2·b·d)`.
    fn anticipated_speed(&self, world: &World, actor: &Actor) -> f64 {
        let b = actor.driver.comfort_decel_mps2.max(0.1);
        let mut dist = world.lane(actor.lane).length_m - actor.s_m;
        let mut best = f64::INFINITY;
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
                cap = cap.min(*v);
            }
            best = best.min(math::sqrt(cap * cap + 2.0 * b * dist.max(0.0)));
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
    /// When the standing vehicle's reaction to being let go began, carried to the next
    /// step.
    release_at: Option<SimTime>,
    /// How far the front may advance this step without closing inside the standstill gap
    /// of a vehicle standing ahead, metres; `None` when the car ahead is moving or there is
    /// none.
    advance_cap_m: Option<f64>,
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
                self.turn_speed = turn_speeds(world, self.params.turn_lateral_accel_mps2);
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
            let world = ctx.world();
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
                for (trip, since, reserved) in queue {
                    let traits = self.traits_for(&*ctx, &trip);
                    match self.try_insert(world, &trip, t0, reserved, traits) {
                        Insertion::Placed(id) => spawned.push(self.actor_spawn(world, id, t0)),
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
                        Insertion::Occupied | Insertion::Unroutable => self.dropped_trips += 1,
                    }
                }
            } else {
                for trip in trips {
                    let traits = self.traits_for(&*ctx, &trip);
                    if let Some(id) = self.insert_trip_with(world, &trip, t0, traits) {
                        spawned.push(self.actor_spawn(world, id, t0));
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
        let crosswalks_in_use = self.crosswalks.occupied(
            self.vru
                .iter()
                .flat_map(|v| v.people().filter(|p| !p.arrived).map(|p| p.lane)),
        );

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
        for id in &ids {
            let Some(actor) = self.actors.get(id) else {
                continue;
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
                lane_view.speed_limit_mps = lane_view.speed_limit_mps.min(*v);
            }
            let options = NeighborOptions {
                lookahead_m: self.params.lookahead_m,
                max_lane_hops: 8,
                // The ego's own classes: a car does not change onto a bus lane, and a
                // rider follows the bicycle lane graph.
                classes: actor.class.class_mask(),
                sides: self.params.lane_changes,
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
            let mut commit_floor: Option<f64> = None;
            // The stop line behind the binding virtual obstacle, when a junction's "stop"
            // is what binds: how much room there is up to the line itself.
            let mut binding_stop_line: Option<f64> = None;
            for junction in self.junctions_ahead(world, actor, t0) {
                let conflicts = self.conflicts_for(world, &junction, &ego, &claims, t0);
                let mut decision = self.entry_decision(world, &ego, &junction, &conflicts);
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
                        if tti < actor.traits.amber_go_tti_s && tti + AMBER_GO_MARGIN_S <= left {
                            decision = EntryDecision::Proceed;
                        }
                    }
                    amber_seen = Some(junction.id);
                }
                // A vehicle that chose to go on amber — it was inside the distance it could
                // stop in — is committed: it does not change its mind a step later because
                // it has slowed for the turn ahead, which is what left drivers braking into
                // the junction on red. It keeps its speed and clears.
                if along && matches!(junction.signal, Some(SignalState::Amber | SignalState::Red)) {
                    let committed = if actor.amber_commit == Some(junction.id) {
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
                        let left = self.amber_left_s(world, junction.id, junction.movement_lane, t0);
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
            // A closed lane ahead on the route is a barrier, stopped short of like a stop
            // line. The end-of-step detour ([`Self::closure_detours`]) has already looked
            // for a way round it, so a vehicle still routed into one has none: it waits
            // for the lane to reopen instead of driving up to the barrier and vanishing.
            if along
                && !self.closed.is_empty()
                && let Some(gap) = self.closure_gap(world, actor)
            {
                let before = leader;
                leader = Self::closest(leader, Some(LeaderView::virtual_obstacle(gap, 0.0)));
                if leader != before {
                    binding_stop_line = None;
                }
            }
            // An external stop command is a virtual leader at zero gap.
            if actor.stopped_until.is_some_and(|until| t0 < until) {
                leader = Self::closest(leader, Some(LeaderView::virtual_obstacle(0.0, 0.0)));
                binding_stop_line = None;
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
                l.gap_m = l.gap_m.max(line - STOP_LINE_MARGIN_M + actor.driver.min_gap_m);
                // The relaxed stop may use the room up to just short of the line, never
                // past it — which, the obstacle now standing beyond the line, is a
                // negative slack.
                line_slack = (line - 0.2) - l.gap_m;
            } else if let Some(line) = binding_stop_line
                && let Some(l) = leader.as_ref()
            {
                line_slack = (line - l.gap_m - 0.5).max(0.0);
            }
            let mut accel = self
                .cf
                .accel(&ego_capped, leader.as_ref(), &lane_view, &self.weather);
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
                accel = accel.min(self.cf.accel(&ego_capped, None, &lane_view, &self.weather));
                // Whatever the stop line allows, the car ahead still binds. The relaxed
                // stop lets a vehicle creep up to a line (or a crosswalk's edge) it is
                // already inside its standstill gap of; with a car standing just beyond
                // that obstacle, it crept into the car's standstill gap at 0.1-0.2 m/s
                // for seconds on end (the auditor's gap-below-s0 class, 308 vehicle-steps
                // on the shipped Midtown run).
                if let Some(v) = nearest_vehicle {
                    accel = accel.min(self.cf.accel(&ego_capped, Some(&v), &lane_view, &self.weather));
                }
                // And so does a merge partner ahead: relaxed, two cars converging on a
                // lane drop crept side by side towards the same crosswalk at 0.1 m/s,
                // each overriding the other, until their bodies met (333 overlap steps on
                // dense Midtown).
                if let Some(limit) = merge_limit {
                    accel = accel.min(limit);
                }
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
            // A car stopping behind one that stands still stops its standstill gap short of
            // it. The car-following model only approaches that gap asymptotically, and the
            // 0.1 s integration of the calibrated city drivers overshot it by up to 5 cm and
            // stood there (2,530 vehicle-steps under s0 on the dense grid).
            let advance_cap_m = if along {
                nearest_vehicle
                    .filter(|v| v.speed_mps.abs() < 1e-6)
                    .map(|v| (v.gap_m - actor.driver.min_gap_m).max(0.0))
            } else {
                None
            };
            decisions.push(Decision {
                actor: *id,
                accel_mps2: accel,
                v0_mps: v0_effective,
                lane_change: LaneChangeDecision::Stay,
                amber_commit,
                amber_seen,
                release_at,
                advance_cap_m,
            });
            neighbours.insert(*id, (ego_capped, nbrs));
        }
        // Lane-change decisions: a second walk, because MOBIL needs the context for its
        // reconsideration draw and the borrow of the world above is shared.
        if let Some(mobil) = self.lane_change.take() {
            for decision in &mut decisions {
                let Some(actor) = self.actors.get(&decision.actor) else {
                    continue;
                };
                if actor.transition.is_some() || t0 < actor.cooldown_until {
                    continue;
                }
                if self.along_path() && !self.may_consider_lane_change(ctx.world(), actor, &mobil) {
                    continue;
                }
                let Some((ego, nbrs)) = neighbours.get(&decision.actor) else {
                    continue;
                };
                decision.lane_change = mobil.decide(ctx, ego, nbrs, &self.weather);
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
        for decision in &decisions {
            let Some(actor) = self.actors.get_mut(&decision.actor) else {
                continue;
            };
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
            let ceiling = if along {
                decision.v0_mps.max(actor.speed_mps)
            } else {
                decision.v0_mps
            };
            let before = actor.speed_mps;
            actor.speed_mps = (actor.speed_mps + decision.accel_mps2 * dt_s).clamp(0.0, ceiling);
            if let Some(cap) = decision.advance_cap_m
                && dt_s > 0.0
                && actor.speed_mps * dt_s > cap
            {
                actor.speed_mps = cap / dt_s;
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
                let from_offset_m = match side {
                    // Moving left puts the vehicle to the *right* of the target lane's
                    // centreline, and the offset closes from there.
                    Side::Left => -separation,
                    Side::Right => separation,
                };
                actor.transition = Some(Transition {
                    from: actor.lane,
                    to,
                    started: t0,
                    duration,
                    from_offset_m,
                    to_offset_m: 0.0,
                    switched: false,
                    from_s_delta: 0.0,
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
                    let router = if actor.class == VehicleClass::Bicycle {
                        &self.bike_router
                    } else {
                        &self.router
                    };
                    next = router
                        .replan(world, actor.lane, actor.destination, t1, &costs)
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
        // Re-plan whoever is due.
        if self.params.dynamic_rerouting {
            let due: Vec<ActorId> = self
                .actors
                .values()
                .filter(|a| {
                    self.router
                        .due(t1, a.planned_at, a.planned_generation, generation)
                })
                .map(|a| a.id)
                .collect();
            for id in due {
                let (lane, destination, class) = {
                    let a = &self.actors[&id];
                    (a.lane, a.destination, a.class)
                };
                let replanned = self
                    .router_for(class)
                    .replan(world, lane, destination, t1, &costs);
                let a = self.actors.get_mut(&id).expect("present");
                a.planned_at = t1;
                a.planned_generation = generation;
                if let Some(route) = replanned {
                    a.route = route;
                    a.route_index = 0;
                }
            }
        }
        for (id, _) in &despawned {
            self.actors.remove(id);
        }
        if along && !self.closed.is_empty() {
            self.closure_detours(world, t1, &costs);
        }
        despawned.extend(vru_gone);

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
                let mut hazards = std::collections::BTreeSet::new();
                for a in self.actors.values() {
                    let bands = self.crosswalks.ahead(world, &a.crosswalk_path());
                    self.crosswalks
                        .hazards_from(&bands, a.speed_mps, &mut hazards);
                }
                vru.set_crossing_permits(self.crosswalks.permits(&signal_states, &hazards));
            }
            let mut people = vru.step(ctx, dt, &snapshot);
            states.append(&mut people);
            self.vru = Some(vru);
        }
        states.sort_by_key(|(a, _)| *a);
        self.published = states.iter().copied().collect();
        spawned.sort_by_key(|s| s.actor);
        despawned.sort_by_key(|(a, _)| *a);
        MobilityUpdate {
            t: t1,
            states,
            spawned,
            despawned,
            signal_states,
        }
    }

    fn command(&mut self, _ctx: &mut dyn MobCtx, cmd: MobilityCommand) {
        self.pending.push(cmd);
    }

    fn kinematics(&self, a: ActorId) -> Option<&Kinematics> {
        self.published.get(&a)
    }

    fn set_weather(&mut self, weather: WeatherState) {
        self.weather = weather;
    }

    fn set_demand_multiplier(&mut self, m: f64) -> bool {
        self.demand.as_mut().is_some_and(|d| d.set_multiplier(m))
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
fn turn_speeds(world: &World, a_lat_mps2: f64) -> BTreeMap<LaneId, f64> {
    let mut out = BTreeMap::new();
    for lane in world.roads.lanes() {
        if lane.kind != LaneKind::Internal || lane.length_m <= 0.0 {
            continue;
        }
        let mut turned = 0.0;
        for i in 0..lane.centreline.len().saturating_sub(2) {
            let (a, b) = lane.segment(i);
            let (_, c) = lane.segment(i + 1);
            let h0 = math::atan2(b.y - a.y, b.x - a.x);
            let h1 = math::atan2(c.y - b.y, c.x - b.x);
            turned += v2xw_world::model::normalise_angle(h1 - h0).abs();
        }
        if turned < 0.1 {
            continue;
        }
        let radius = lane.length_m / turned;
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
    if room > 0.05 && -need(room) <= PLANNED_STOP_MAX_DECEL_MPS2 {
        return a_model.max(need(room));
    }
    let to_line = gap_m + slack_m;
    if to_line <= 0.05 {
        return a_model;
    }
    let tight = if room > 0.05 {
        need(room)
    } else {
        f64::NEG_INFINITY
    };
    a_model.max(tight.max(need(to_line)))
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
                let want = preset.profile(class);
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
        let second = close(&mut engine, 100 * NS_PER_MS, &[(lanes[1], false), (lanes[2], true)]);
        assert_ne!(first, second, "the closure set changed and the generation did not");
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
}
