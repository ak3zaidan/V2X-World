//! The plug-in traits of 03-interfaces.md §3.
//!
//! Every one extends [`Model`], so a registered mobility model always carries a card
//! (03-interfaces.md §12) and the card is reachable from the live object.
//!
//! # Deviations from the document, and why
//!
//! Three signatures differ from the ones 03-interfaces.md prints, and each difference is a
//! compile error in the document's version:
//!
//! 1. `&mut dyn Ctx` becomes `&mut dyn MobCtx`. [`v2xw_core::ctx::Ctx`] has an associated
//!    `Payload` type that only the kernel crate can name; [`crate::ctx`] explains the
//!    adapter and the one-line migration.
//! 2. `Mobility::init` takes `Box<dyn Demand>`, not `&dyn Demand`. Two things make the
//!    document's version impossible: [`Demand::spawns_in`] takes `&mut self`, because a
//!    demand model advances a stream, so a shared reference cannot call it at all; and
//!    `step` — which is where trips actually arrive — is given no demand argument, so the
//!    mobility model must *hold* the demand model rather than borrow it for the length of
//!    one call. Taking ownership at `init` is the only reading of the two signatures
//!    together that compiles. The separate `world: &World` parameter is dropped as well:
//!    the world is [`MobCtx::world`], and passing it twice would let a caller pass two
//!    different worlds.
//! 3. `Mobility::step` takes `dt: Duration`, not `dt: SimTime`. [`SimTime`] is an instant
//!    and [`Duration`] is a length of time; the step length is the second thing.
//!
//! One method is *added* rather than changed: [`CarFollowing::profile`]. 03-interfaces.md
//! §3 gives the trait `accel` alone, which leaves the engine with no way to ask a
//! car-following model for the per-class driver parameters that model was calibrated with;
//! the engine used a hard-coded set instead, so `NativeMobility::legacy()` ran the legacy
//! equations with Kesting 2010 drivers. It is defaulted, so no implementation outside this
//! crate has to change.

use v2xw_core::card::Tier;
use v2xw_core::ids::{ActorId, LaneId, NodeId};
use v2xw_core::kinematics::Kinematics;
use v2xw_core::model::Model;
use v2xw_core::time::{Duration, SimTime};
use v2xw_core::weather::WeatherState;
use v2xw_core::{FixQuality, PositionEstimate};

use crate::classes::VehicleClass;
use crate::ctx::MobCtx;
use crate::error::Result;
use crate::snapshot::ActorSnapshot;
use crate::views::{
    ConflictView, DriverProfile, EdgeCost, EntryDecision, GnssEnv, JunctionView,
    LaneChangeDecision, LaneNeighbors, LaneView, LeaderView, MobilityCommand, MobilityUpdate,
    ReroutePolicy, Route, TripRequest, VehicleView,
};

/// A source of ground-truth kinematics for every actor (03-interfaces.md §3).
pub trait Mobility: Model {
    /// Which fidelity tier this implementation serves.
    fn tier(&self) -> Tier;

    /// Prepares the model: reads the world from `ctx`, takes the demand model's first
    /// trips, and places nothing on the road yet.
    ///
    /// # Errors
    ///
    /// Whatever the world or the parameters make impossible — see [`crate::error::MobError`].
    fn init(&mut self, ctx: &mut dyn MobCtx, demand: Box<dyn Demand>) -> Result<()>;

    /// Advances every actor by `dt`.
    ///
    /// Must be a pure function of `(state, dt, RNG streams)`: no wall clock, no
    /// thread-local, no dependence on the order actors happen to be stored in. The
    /// returned [`MobilityUpdate`] is ordered by [`ActorId`] (invariant I-M1).
    fn step(&mut self, ctx: &mut dyn MobCtx, dt: Duration) -> MobilityUpdate;

    /// Applies an external control at the next step.
    fn command(&mut self, ctx: &mut dyn MobCtx, cmd: MobilityCommand);

    /// The traffic was frozen for `paused` (a `time.fast_forward` window: the calendar ran
    /// on for the credential backend, the road did not), and the next [`Mobility::step`]
    /// continues it. Every instant the model keeps that measures elapsed *traffic* time —
    /// when a vehicle came to a standstill, when a lane change began, when a trip started
    /// waiting to enter, when a stop ends, the demand's next arrival — moves on by
    /// `paused`, so the step after the window continues the traffic exactly as a run with
    /// no window would have from the instant the window began.
    ///
    /// Without it every such clock read the window as time spent: each vehicle standing
    /// when the window began had "stood" for hours and the gridlock resolution teleported
    /// or removed it on the first step (the full-scale benchmark's 1,833 → 1,281 nodes and
    /// its burst of honest position-jump reports, R-FFBURST), every lane change finished
    /// in one step, and every trip queued to enter was dropped. A provided no-op, for a
    /// tier with no such clock.
    fn resume_after_pause(&mut self, _paused: Duration) {}

    /// The last published state of one actor, for perception and safety metrics.
    fn kinematics(&self, a: ActorId) -> Option<&Kinematics>;

    /// The weather every vehicle drives in from the next step on.
    ///
    /// A provided method, so a tier that models no weather response keeps compiling; the
    /// native engine overrides it. Until the kernel called it, `weather.*` reached the
    /// radio and the GNSS models and no driver: a snowstorm changed nothing on the road.
    fn set_weather(&mut self, _weather: v2xw_core::weather::WeatherState) {}

    /// What the gridlock resolution has done so far ([`crate::EngineParams::time_to_teleport_s`]),
    /// or `None` for a model without one or with it off.
    fn gridlock(&self) -> Option<crate::engine::GridlockCounters> {
        None
    }

    /// Trips the demand produced that never reached the road, as `(dropped, unroutable)`:
    /// every dropped trip, and of those the ones with no route from their origin. `None`
    /// for a model that does not count them. A run whose demand is mostly dropped looks
    /// like an empty map; this is how the report says why.
    fn trip_losses(&self) -> Option<(u64, u64)> {
        None
    }

    /// Passes a demand multiplier to the demand model (see [`Demand::set_multiplier`]).
    /// Returns whether it was honoured; a tier with no demand model returns `false`.
    fn set_demand_multiplier(&mut self, _m: f64) -> bool {
        false
    }

    /// What `a` means to do at the next junction on its route (see
    /// [`crate::views::Intent`]), or `None` when it is further than
    /// [`crate::views::INTENT_HORIZON_M`] or this tier plans no routes.
    ///
    /// The vehicle's own intention — what its navigation knows and its turn signal shows —
    /// which its own applications (left-turn assist, intersection movement assist) and
    /// its BSM's exterior lights read. A provided method, so a tier without routes keeps
    /// compiling.
    fn intent(&self, _world: &v2xw_world::World, _a: ActorId) -> Option<crate::views::Intent> {
        None
    }

    /// Every vehicle's exterior lamps after the last step (vwp-v1 §3.3.5,
    /// [`crate::lamps`]), in actor-id order. Headlamps are not decided here.
    ///
    /// A provided method: a tier that models no lamps returns nothing and every vehicle
    /// is streamed dark, which is what the stream carried before the byte existed.
    fn exterior_lamps(&mut self, _ctx: &mut dyn MobCtx) -> Vec<(ActorId, u8)> {
        Vec::new()
    }

    /// The cooperative-awareness link the tier's car-following models read
    /// ([`crate::coop`]), which the kernel fills from what each node delivered. A provided
    /// method: a tier without cooperative driving returns `None` and the kernel feeds
    /// nothing.
    fn cooperative(&self) -> Option<std::sync::Arc<crate::coop::CoopLink>> {
        None
    }

    /// Which tier is driving `a` now, for a provider that runs more than one
    /// (`crate::meso::tiered`): the engine matches the radio's focus region to it and
    /// keeps the full node stack where the traffic is microscopic. `None` — the default —
    /// means the provider is one tier throughout, [`Mobility::tier`].
    fn tier_of(&self, _a: ActorId) -> Option<Tier> {
        None
    }
}

/// Longitudinal acceleration from the gap and the speed difference to the leader
/// (03-interfaces.md §3, 04-models.md §2.1).
///
/// `accel` must be pure: same inputs, same output, on every platform. It draws no random
/// numbers — driver heterogeneity is a per-vehicle draw that reaches the model through
/// [`crate::views::DriverProfile`] — so it takes no context.
pub trait CarFollowing: Model {
    /// The acceleration, m/s², for `ego` behind `leader` on `lane` in weather `w`.
    ///
    /// `leader` is `None` on a free road; a stop line, a signal or a junction to yield at
    /// arrives as a *virtual* leader ([`LeaderView::virtual_obstacle`]), so one equation
    /// produces every deceleration the vehicle applies.
    fn accel(
        &self,
        ego: &VehicleView,
        leader: Option<&LeaderView>,
        lane: &LaneView,
        w: &WeatherState,
    ) -> f64;

    /// The driver parameters this model's own calibration gives a vehicle of `class`.
    ///
    /// `a_max`, `b`, `T` and `s0` are *this model's* parameters — they are what the
    /// calibration the model was published with consists of — but they travel per vehicle,
    /// in a [`DriverProfile`], because they are drawn per driver. Without this method the
    /// engine has nowhere to get them from and has to guess a set, which is how a
    /// legacy-parity run came to be driven by Kesting 2010 drivers.
    ///
    /// Defaulted to the Kesting 2010 set, §2.1's medium-tier default, so a model with no
    /// per-class calibration of its own is explicit about which one it borrows.
    fn profile(&self, class: VehicleClass) -> DriverProfile {
        crate::carfollowing::idm::IdmPreset::Kesting2010.profile(class)
    }

    /// Called by the engine once per mobility step, before any `accel` of that step, with
    /// the instant the step starts at and its length.
    ///
    /// Added for the driving-model library (`crate::carfollowing::library`): a
    /// discrete-time model (Krauss, Gipps, the PATH and Ploeg controllers) needs the step
    /// length, and a model with per-step noise needs the step index for its keyed stream.
    /// Defaulted to nothing, so the IDM and every model outside this crate are unaffected.
    fn begin_step(&self, _now: SimTime, _dt: Duration) {}

    /// The cooperative-awareness link this model drives on, when it is a cooperative model
    /// ([`crate::coop`]): the kernel writes the BSMs and CAMs each follower's node
    /// delivered into it. Defaulted to `None`.
    fn cooperative(&self) -> Option<std::sync::Arc<crate::coop::CoopLink>> {
        None
    }
}

/// Discretionary and mandatory lane changes (03-interfaces.md §3, 04-models.md §2.2).
pub trait LaneChange: Model {
    /// Whether `ego` changes lane, given the **one** neighbour query `nbrs` that the
    /// car-following leader search also used.
    fn decide(
        &self,
        ctx: &mut dyn MobCtx,
        ego: &VehicleView,
        nbrs: &LaneNeighbors,
        w: &WeatherState,
    ) -> LaneChangeDecision;
}

/// Right of way at a junction (03-interfaces.md §3, 04-models.md §2.3).
pub trait IntersectionControl: Model {
    /// Whether `ego` may enter `j` given the other claimants, and if not, where to stop.
    ///
    /// `conflicts` is ordered by [`ActorId`], so a model that breaks a tie by id sees the
    /// same order on every run and every thread count.
    fn may_enter(
        &self,
        ego: &VehicleView,
        j: &JunctionView,
        conflicts: &[ConflictView],
        w: &WeatherState,
    ) -> EntryDecision;
}

/// Shortest paths on the lane graph (03-interfaces.md §3, 04-models.md §2.4).
pub trait Router: Model {
    /// A route from `from` to `to` for a vehicle that may use the lanes `costs` prices,
    /// or `None` when none exists.
    fn route(
        &self,
        ctx: &mut dyn MobCtx,
        from: LaneId,
        to: LaneId,
        at: SimTime,
        costs: &dyn EdgeCost,
    ) -> Option<Route>;

    /// When this router re-plans.
    fn reroute_policy(&self) -> ReroutePolicy;
}

/// A source of trips (03-interfaces.md §3, 04-models.md §2.4).
pub trait Demand: Model + Send + Sync {
    /// Every trip that starts in `[from, to)`, in stream order (invariant I-M2).
    ///
    /// Called once per mobility step with consecutive, non-overlapping windows. A model
    /// that draws candidate arrivals faster than it keeps them (the thinned Poisson
    /// process of §2.4) must keep its own cursor so the draw sequence does not depend on
    /// how the caller chopped time up.
    fn spawns_in(&mut self, ctx: &mut dyn MobCtx, from: SimTime, to: SimTime) -> Vec<TripRequest>;

    /// Multiplies the offered demand by `m` from the next window on, for a scenario
    /// timeline's `demand.multiplier` (03-interfaces.md §13). Returns whether this model
    /// honours it: one with no arrival process to scale — a one-shot drop — returns `false`
    /// and the caller says so rather than pretending.
    fn set_multiplier(&mut self, _m: f64) -> bool {
        false
    }

    /// The traffic was frozen for `paused` ([`Mobility::resume_after_pause`]): an arrival
    /// process keeps its cursor in traffic time, so its next arrival moves on by `paused`
    /// and the window neither spawns the trips of the frozen hours nor burns through
    /// their draws. A provided no-op for a model with no cursor.
    fn resume_after_pause(&mut self, _paused: Duration) {}

    /// The route this model planned for `trip`, when it plans routes itself — an
    /// assignment's saved route, a bus line through its stops ([`crate::demand::hooks`]).
    ///
    /// `None` (the default) lets the engine's router plan the trip. A returned route must
    /// start on `trip.origin`; the engine checks it with
    /// [`crate::demand::hooks::planned_route_is_drivable`] and routes the trip itself when
    /// the check fails.
    fn planned_route(
        &self,
        _world: &v2xw_world::World,
        _trip: &TripRequest,
    ) -> Option<Vec<LaneId>> {
        None
    }

    /// True if this model wants [`Demand::observe`] called after every step. `false` by
    /// default, and then the engine builds nothing for it.
    fn wants_progress(&self) -> bool {
        false
    }

    /// Every vehicle on the road at `t`, the end of a step, in actor-id order; the
    /// commands returned are applied at the start of the next step. Called only when
    /// [`Demand::wants_progress`] is true.
    fn observe(
        &mut self,
        _t: SimTime,
        _progress: &[crate::demand::hooks::ActorProgress],
    ) -> Vec<MobilityCommand> {
        Vec::new()
    }

    /// Tells the model that one trip it produced never reached the road: its origin had
    /// no room until the departure deadline, or no route existed. A model with a cap on
    /// its trips gives the slot back, so a cap of N means N vehicles placed rather than N
    /// trips drawn (a cap of 2 on a small grid used to put 0 vehicles on the road when both
    /// draws were refused). The default does nothing.
    fn refund(&mut self) {}

    /// The number of vehicles this model holds on the road at all times, when it holds a
    /// fleet rather than running an arrival process. The engine then asks
    /// [`Demand::top_up`] for a trip whenever fewer are on the road (or waiting to enter),
    /// so a finished trip is replaced by a new one. `None` (the default) keeps the arrival
    /// process.
    fn fleet_target(&self) -> Option<u64> {
        None
    }

    /// One trip starting at `t`, outside the arrival process: what holds a fleet at
    /// [`Demand::fleet_target`]. `None` (the default) when the model cannot make one.
    fn top_up(&mut self, _ctx: &mut dyn MobCtx, _t: SimTime) -> Option<TripRequest> {
        None
    }

    /// A fresh trip at `t` in place of one that had no route (its slot already given back
    /// with [`Demand::refund`]), so a network with streets that only leave the map does
    /// not silently thin the arrival process. `None` when the model cannot draw one; the
    /// default is [`Demand::top_up`], which only a held fleet answers.
    fn redraw(&mut self, ctx: &mut dyn MobCtx, t: SimTime) -> Option<TripRequest> {
        self.top_up(ctx, t)
    }
}

/// Vulnerable road users (03-interfaces.md §3, 04-models.md §2.5).
pub trait VruMobility: Model {
    /// Advances every VRU by `dt` and returns their states, ordered by [`ActorId`].
    ///
    /// `vehicles` is the start-of-step snapshot: VRUs see vehicles where they were at the
    /// start of the step, exactly as vehicles see each other, so the whole mobility phase
    /// is one Jacobi update.
    fn step(
        &mut self,
        ctx: &mut dyn MobCtx,
        dt: Duration,
        vehicles: &ActorSnapshot,
    ) -> Vec<(ActorId, Kinematics)>;
}

/// A node's belief about its own position and time (03-interfaces.md §3, 04-models.md §3.8).
pub trait GnssModel: Model {
    /// The belief of `node`, given its ground truth and its environment.
    ///
    /// Stateful per node: the error process is correlated in time, so the model keeps one
    /// state per node and advances it here. The state must be advanced from the RNG stream
    /// of that node only, so one node's outage cannot shift another node's noise.
    fn estimate(
        &mut self,
        ctx: &mut dyn MobCtx,
        node: NodeId,
        gt: &Kinematics,
        env: &GnssEnv,
    ) -> PositionEstimate;

    /// The road was frozen for `paused` ([`Mobility::resume_after_pause`], a
    /// `time.fast_forward` window): every node's receiver resumes as if no time had passed,
    /// so the instant of its last epoch and the ends of its current outage, burst or
    /// jamming move on by `paused`.
    ///
    /// Without it the first epoch after a 12 h window read the window as elapsed receiver
    /// time: the per-second hazards of an outlier and of a multipath burst over 43,000 s
    /// are 1, so every vehicle's first position after the resume carried an outlier and
    /// entered a burst, and its next BSM, 0.1 s later and without the outlier, was a
    /// [SM-MBR] RandomPosition jump that every honest receiver reported (R-FFBURST). A
    /// provided no-op, for a model with no clock.
    fn resume_after_pause(&mut self, _paused: Duration) {}
}

/// A node's clock (03-interfaces.md §3, 04-models.md §3.8).
pub trait ClockModel: Model + Send + Sync {
    /// The time `node` believes it is, including drift during holdover.
    fn read(&mut self, ctx: &mut dyn MobCtx, node: NodeId, gnss: &FixQuality) -> SimTime;
}
