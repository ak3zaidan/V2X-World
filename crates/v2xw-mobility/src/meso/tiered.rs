//! `mobility/tiered/meso-micro` — microscopic inside a focus region, mesoscopic outside.
//!
//! # The coupling
//!
//! A city is driven by the queue network ([`super::MesoCore`]) everywhere except a focus
//! region, where the microscopic engine ([`NativeMobility`]) runs with every model it has.
//! A lane belongs to the region when its midpoint is inside it; a junction's internal
//! lane belongs to it when the junction is. One id space covers both tiers
//! ([`NativeMobility::reserve_actor_id`]), so a vehicle crossing the boundary is the same
//! actor to the engine — the same node, the same certificates, the same records — and is
//! never announced as a despawn and a spawn.
//!
//! Crossing *in* (queue → micro): a vehicle whose next lane is microscopic leaves the head
//! of its last queue only if the microscopic engine admits it on that lane at its speed
//! ([`NativeMobility::admit`]: room to follow whoever is ahead, and room for whoever is
//! behind to follow it). Refused, it stays at the head of its queue and blocks it, as a
//! full segment would; that is how a queue inside the region spills back out of it.
//!
//! Crossing *out* (micro → queue): a microscopic vehicle whose lane, after a step, is
//! mesoscopic is released into the segment at its position. Before every step, each
//! mesoscopic lane fed directly from the region whose first segment has no room for a
//! vehicle is *held* ([`NativeMobility::set_holds`]): microscopic drivers stop short of it
//! as they would of a closed lane, without detouring round it — spill-back crossing the
//! boundary the other way. A vehicle already too close to stop is let in over capacity,
//! which the queue absorbs by blocking its own inflow until it drains.
//!
//! Demand: every trip is taken in stream order and given its id at once; a trip starting
//! on a microscopic lane is placed by the microscopic engine's insertion rules, one
//! starting outside by the queue network's. Either waits for room, up to two minutes.
//!
//! # Why this is not SUMO
//!
//! SUMO runs a network either microscopically or mesoscopically. A hybrid in one run is
//! what a V2X study wants: car-following, lane changes, signals and pedestrians where the
//! radio is evaluated at full fidelity, and a whole city of queues around it that still
//! produces the arrivals, spill-back and channel load the region sees. The engine matches
//! the radio's focus region (02-architecture.md §7.3) to this region.
//!
//! # How to add another tier pair
//!
//! A coupling is a [`Mobility`] that owns two providers and an id space. The seams on the
//! microscopic side are in `crate::engine::tiering`; the queue network's are
//! [`super::MesoBoundary`]. A new outside tier (a link-transmission model) implements the
//! same boundary and is selected by `scale.traffic.model` in the engine's table.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Determinism, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::geom::Vec3;
use v2xw_core::ids::{ActorId, LaneId};
use v2xw_core::kinematics::Kinematics;
use v2xw_core::time::{Duration, SimTime};
use v2xw_core::weather::WeatherState;
use v2xw_world::{ClassMask, LaneKind, World};

use super::{
    MAX_DEPART_DELAY_S, MesoBoundary, MesoCore, MesoExit, MesoParams, MesoVehicle,
    REFERENCE_VEHICLE_M, route_batch,
};
use crate::ctx::MobCtx;
use crate::engine::{DriverTraits, Handoff, NativeMobility};
use crate::error::Result;
use crate::routing::dijkstra::{Dijkstra, DijkstraParams};
use crate::traits::{Demand, Mobility};
use crate::views::{
    ActorSpawn, DespawnCause, DriverProfile, MobilityCommand, MobilityUpdate, TripRequest,
};

/// The model id.
pub const MODEL_ID: &str = "mobility/tiered/meso-micro";

/// The model version.
pub const MODEL_VERSION: &str = "1.0.0";

/// Where the microscopic tier runs, in world-local metres (horizontal).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TierRegion {
    /// A disc.
    Circle {
        /// Its centre, `[x, y]` metres.
        centre_m: [f64; 2],
        /// Its radius, metres.
        radius_m: f64,
    },
    /// An axis-aligned box.
    Box {
        /// The south-west corner, `[x, y]` metres.
        min_m: [f64; 2],
        /// The north-east corner, `[x, y]` metres.
        max_m: [f64; 2],
    },
}

impl TierRegion {
    /// Whether `p` is inside, horizontally.
    #[must_use]
    pub fn contains(&self, p: Vec3) -> bool {
        match *self {
            TierRegion::Circle { centre_m, radius_m } => {
                let (dx, dy) = (p.x - centre_m[0], p.y - centre_m[1]);
                dx * dx + dy * dy <= radius_m * radius_m
            }
            TierRegion::Box { min_m, max_m } => {
                p.x >= min_m[0] && p.x <= max_m[0] && p.y >= min_m[1] && p.y <= max_m[1]
            }
        }
    }

    /// How far outside `p` is, metres; zero inside.
    #[must_use]
    pub fn distance_outside(&self, p: Vec3) -> f64 {
        match *self {
            TierRegion::Circle { centre_m, radius_m } => {
                let (dx, dy) = (p.x - centre_m[0], p.y - centre_m[1]);
                ((dx * dx + dy * dy).sqrt() - radius_m).max(0.0)
            }
            TierRegion::Box { min_m, max_m } => {
                let dx = (min_m[0] - p.x).max(p.x - max_m[0]).max(0.0);
                let dy = (min_m[1] - p.y).max(p.y - max_m[1]).max(0.0);
                (dx * dx + dy * dy).sqrt()
            }
        }
    }
}

/// The coupling's parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TieredParams {
    /// Where the microscopic tier runs.
    pub region: TierRegion,
    /// The queue network outside it.
    #[serde(default)]
    pub meso: MesoParams,
}

/// Running totals of what crossed the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TierCounters {
    /// Vehicles handed from the queue network into the region.
    pub handed_in: u64,
    /// Vehicles handed from the region out to the queue network.
    pub handed_out: u64,
    /// Hand-in attempts refused for want of room: a refused vehicle is offered again at
    /// every step until it fits, so one long wait counts many times.
    pub refused_in: u64,
    /// Hand-outs let into a full segment because the vehicle could not stop.
    pub over_capacity_out: u64,
    /// Trips dropped for want of a route or of room within two minutes.
    pub dropped_trips: u64,
}

/// The tiered provider.
pub struct TieredMobility {
    params: TieredParams,
    micro: NativeMobility,
    meso: Option<MesoCore>,
    /// Per lane (dense index): whether the microscopic tier owns it.
    micro_lane: Vec<bool>,
    /// Every way out of the region: the lane a vehicle is held short of (the first lane
    /// past a microscopic one — a junction connector, or the road itself) and the
    /// mesoscopic road whose first segment decides, in order.
    exits: Vec<(LaneId, LaneId)>,
    router: Dijkstra,
    demand: Option<Box<dyn Demand>>,
    /// Trips whose origin is microscopic and had no room, routed, with their front
    /// position and the instant they first asked.
    micro_waiting: Vec<(MesoVehicle, f64, SimTime)>,
    /// Trips whose origin is mesoscopic and had no room.
    meso_waiting: Vec<(MesoVehicle, f64, SimTime)>,
    published: BTreeMap<ActorId, Kinematics>,
    pending: Vec<MobilityCommand>,
    counters: TierCounters,
    card: ModelCard,
}

impl core::fmt::Debug for TieredMobility {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TieredMobility")
            .field("params", &self.params)
            .field("micro", &self.micro.len())
            .field("meso", &self.meso.as_ref().map_or(0, MesoCore::len))
            .field("counters", &self.counters)
            .finish_non_exhaustive()
    }
}

impl TieredMobility {
    /// The coupling of `micro` inside `params.region` and a queue network outside it.
    #[must_use]
    pub fn new(params: TieredParams, micro: NativeMobility) -> Self {
        Self {
            card: card(&params),
            params,
            micro,
            meso: None,
            micro_lane: Vec::new(),
            exits: Vec::new(),
            router: Dijkstra::new(DijkstraParams::for_classes(ClassMask::MOTOR_TRAFFIC)),
            demand: None,
            micro_waiting: Vec::new(),
            meso_waiting: Vec::new(),
            published: BTreeMap::new(),
            pending: Vec::new(),
            counters: TierCounters::default(),
        }
    }

    /// The microscopic engine.
    #[must_use]
    pub fn micro(&self) -> &NativeMobility {
        &self.micro
    }

    /// The queue network, once initialised.
    #[must_use]
    pub fn meso(&self) -> Option<&MesoCore> {
        self.meso.as_ref()
    }

    /// What has crossed the boundary so far.
    #[must_use]
    pub fn counters(&self) -> TierCounters {
        self.counters
    }

    /// The parameters in force.
    #[must_use]
    pub fn params(&self) -> &TieredParams {
        &self.params
    }

    /// Whether the microscopic tier owns `lane`.
    #[must_use]
    pub fn is_micro_lane(&self, lane: LaneId) -> bool {
        self.micro_lane
            .get(lane.as_usize())
            .copied()
            .unwrap_or(false)
    }

    /// Which lanes are microscopic: a lane by its midpoint, an internal lane by its
    /// junction.
    fn classify(world: &World, region: &TierRegion) -> Vec<bool> {
        world
            .roads
            .lanes()
            .iter()
            .map(|l| match l.junction {
                Some(j) if l.kind == LaneKind::Internal => {
                    region.contains(world.junction(j).position)
                }
                _ => region.contains(l.point_at(0.5 * l.length_m)),
            })
            .collect()
    }

    /// The meso lane and front position a released vehicle enters the queue network at:
    /// its own lane, or — on a junction's internal lane outside the region — the next
    /// ordinary lane of its route, at its start.
    fn meso_entry(world: &World, h: &Handoff) -> Option<(usize, f64)> {
        let lane = world.lane(*h.route.lanes.get(h.route_index)?);
        if lane.kind != LaneKind::Internal {
            return Some((h.route_index, h.s_m.min(lane.length_m)));
        }
        let k = (h.route_index + 1..h.route.lanes.len())
            .find(|k| world.lane(h.route.lanes[*k]).kind != LaneKind::Internal)?;
        Some((k, 0.0))
    }

    /// Holds every way out whose queued road's first segment has no room for a reference
    /// vehicle.
    fn update_holds(&mut self, world: &World) {
        let Some(meso) = self.meso.as_ref() else {
            return;
        };
        let holds: BTreeSet<LaneId> = self
            .exits
            .iter()
            .filter(|(_, road)| {
                meso.segment_at(world, *road, 0.0)
                    .is_some_and(|seg| !meso.can_enter(seg, REFERENCE_VEHICLE_M))
            })
            .map(|(hold, _)| *hold)
            .collect();
        self.micro.set_holds(holds);
    }
}

/// The microscopic side of the boundary, as the queue network sees it.
struct MicroSide<'a> {
    micro: &'a mut NativeMobility,
    micro_lane: &'a [bool],
    counters: &'a mut TierCounters,
}

impl MesoBoundary for MicroSide<'_> {
    fn is_meso(&self, lane: LaneId) -> bool {
        !self
            .micro_lane
            .get(lane.as_usize())
            .copied()
            .unwrap_or(false)
    }

    fn offer(&mut self, world: &World, v: &MesoVehicle, next_index: usize, at: SimTime) -> bool {
        let lane = world.lane(v.route.lanes[next_index]);
        let speed = v
            .driver
            .desired_speed_mps
            .min(lane.speed_limit_mps * v.traits.speed_factor);
        let h = Handoff {
            id: v.id,
            seq: v.seq,
            class: v.class,
            driver: v.driver,
            traits: v.traits,
            route: v.route.clone(),
            route_index: next_index,
            s_m: v.class.spec().length_m,
            speed_mps: speed,
            spawned: v.spawned,
        };
        match self.micro.admit(world, h, at) {
            Ok(()) => {
                self.counters.handed_in += 1;
                true
            }
            Err(_) => {
                self.counters.refused_in += 1;
                false
            }
        }
    }
}

impl v2xw_core::model::Model for TieredMobility {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl Mobility for TieredMobility {
    fn gridlock(&self) -> Option<crate::engine::GridlockCounters> {
        Mobility::gridlock(&self.micro)
    }

    fn tier(&self) -> Tier {
        Tier::Medium
    }

    fn init(&mut self, ctx: &mut dyn MobCtx, demand: Box<dyn Demand>) -> Result<()> {
        // The microscopic engine places nothing on its own: every trip comes through here.
        self.micro
            .init(ctx, Box::new(crate::demand::NoDemand::new()))?;
        let world = ctx.world();
        self.meso = Some(MesoCore::new(
            world,
            self.params.meso,
            ClassMask::MOTOR_TRAFFIC,
        ));
        self.micro_lane = Self::classify(world, &self.params.region);
        let mut exits = BTreeSet::new();
        for lane in world.roads.lanes() {
            if !self.micro_lane[lane.id.as_usize()] {
                continue;
            }
            for c in world.successors(lane.id) {
                // Held short of the junction when the road beyond it is queued: a vehicle
                // waits at the stop line, not inside the junction.
                if !self.micro_lane[c.to_lane.as_usize()]
                    && world.lane(c.to_lane).kind != LaneKind::Internal
                {
                    exits.insert((c.via.unwrap_or(c.to_lane), c.to_lane));
                }
            }
        }
        self.exits = exits.into_iter().collect();
        self.demand = Some(demand);
        Ok(())
    }

    fn step(&mut self, ctx: &mut dyn MobCtx, dt: Duration) -> MobilityUpdate {
        let t0 = ctx.now();
        let t1 = dt.after(t0);
        let Some(mut meso) = self.meso.take() else {
            return self.micro.step(ctx, dt);
        };
        let mut spawned: Vec<ActorSpawn> = Vec::new();
        let mut despawned: Vec<(ActorId, DespawnCause)> = Vec::new();

        // --- commands: each to the tier holding its actor ---------------------------
        let mut trips: Vec<TripRequest> = Vec::new();
        for command in core::mem::take(&mut self.pending) {
            match command {
                MobilityCommand::Spawn(trip) => trips.push(trip),
                MobilityCommand::Despawn { actor } if meso.vehicle(actor).is_some() => {
                    meso.remove(actor);
                    despawned.push((actor, DespawnCause::Commanded));
                }
                // Speed caps, stops, brakes, cut-ins and reroutes of a vehicle in the queue
                // network mean nothing to a queue; closures reach the microscopic engine,
                // which is where they are modelled.
                other => self.micro.command(ctx, other),
            }
        }
        if let Some(mut demand) = self.demand.take() {
            trips.extend(demand.spawns_in(ctx, t0, t1));
            self.demand = Some(demand);
        }

        // --- demand: old waiters first, then the new trips in stream order ------------
        let world = ctx.world();
        let max_wait = Duration::from_secs_f64(MAX_DEPART_DELAY_S).as_nanos();
        let mut micro_queue = core::mem::take(&mut self.micro_waiting);
        let mut meso_queue = core::mem::take(&mut self.meso_waiting);
        // Every trip gets its id in stream order and the microscopic engine's own driver
        // (so a vehicle drives the same in both tiers), and is routed once, on the pool;
        // a trip that waits for room keeps its route rather than searching again.
        let mut to_route: Vec<(TripRequest, ActorId, DriverProfile, DriverTraits)> = Vec::new();
        for trip in trips {
            let id = self.micro.reserve_actor_id();
            let (driver, traits) = self.micro.trip_driver(&*ctx, &trip);
            to_route.push((trip, id, driver, traits));
        }
        for (trip, routed) in route_batch(&self.router, world, to_route) {
            let s_m = trip.origin_s_m.max(trip.class.spec().length_m);
            match routed {
                Some(v) if self.is_micro_lane(trip.origin) => micro_queue.push((v, s_m, t0)),
                Some(v) => meso_queue.push((v, s_m, t0)),
                None => self.counters.dropped_trips += 1,
            }
        }
        for (v, s_m, since) in micro_queue {
            let h = Handoff {
                id: v.id,
                seq: v.seq,
                class: v.class,
                driver: v.driver,
                traits: v.traits,
                route: v.route.clone(),
                route_index: 0,
                s_m,
                speed_mps: 0.0,
                spawned: t0,
            };
            match self.micro.admit(world, h, t0) {
                Ok(()) => spawned.extend(self.micro.spawn_record(world, v.id, t0)),
                Err(_) if t0.saturating_sub(since) < max_wait => {
                    self.micro_waiting.push((v, s_m, since));
                }
                Err(_) => self.counters.dropped_trips += 1,
            }
        }
        for (mut v, s_m, since) in meso_queue {
            let id = v.id;
            v.spawned = since;
            match meso.insert(world, v, s_m, t0, false) {
                Ok(()) => {
                    if let (Some(k), Some(v)) = (meso.state_of(world, id, t0), meso.vehicle(id)) {
                        spawned.push(ActorSpawn {
                            actor: id,
                            t: t0,
                            class: v.class,
                            kinematics: k,
                            route: v.route.clone(),
                            driver: v.driver,
                            seq: v.seq,
                        });
                    }
                }
                Err(v) if t0.saturating_sub(since) < max_wait => {
                    self.meso_waiting.push((*v, s_m, since))
                }
                Err(_) => self.counters.dropped_trips += 1,
            }
        }

        // --- the queue network, handing vehicles into the region as it goes -----------
        {
            let mut side = MicroSide {
                micro: &mut self.micro,
                micro_lane: &self.micro_lane,
                counters: &mut self.counters,
            };
            for exit in meso.advance(world, t0, t1, &mut side) {
                if let MesoExit::Completed(id, _) = exit {
                    despawned.push((id, DespawnCause::TripComplete));
                }
            }
        }

        // --- the region --------------------------------------------------------------
        self.meso = Some(meso);
        self.update_holds(ctx.world());
        let mut micro_update = self.micro.step(ctx, dt);
        let mut meso = self.meso.take().expect("put back above");
        let world = ctx.world();

        // --- out of the region ----------------------------------------------------------
        let mut released: BTreeSet<ActorId> = BTreeSet::new();
        for (id, lane, class) in self.micro.vehicle_lanes() {
            if self.is_micro_lane(lane)
                || !ClassMask::MOTOR_TRAFFIC.contains_any(class.class_mask())
            {
                continue;
            }
            let Some(h) = self.micro.release(id) else {
                continue;
            };
            let Some((k, s_m)) = Self::meso_entry(world, &h) else {
                // Nowhere in the queue network to go (the route ends on the connector):
                // the trip is over.
                despawned.push((id, DespawnCause::TripComplete));
                released.insert(id);
                continue;
            };
            let v = MesoVehicle {
                id,
                seq: h.seq,
                class: h.class,
                driver: h.driver,
                traits: h.traits,
                route: h.route.clone(),
                route_index: k,
                segment: 0,
                entered: t1,
                earliest_exit: t1,
                entry_frac: 0.0,
                spawned: h.spawned,
            };
            let full = meso
                .segment_at(world, v.lane(), s_m)
                .is_some_and(|seg| !meso.can_enter(seg, v.footprint_m()));
            match meso.insert(world, v, s_m, t1, true) {
                Ok(()) => {
                    self.counters.handed_out += 1;
                    if full {
                        self.counters.over_capacity_out += 1;
                    }
                    released.insert(id);
                }
                Err(_) => {
                    // The lane is not in the queue network (a class-restricted lane): the
                    // vehicle stays microscopic. Should the engine have no room to take it
                    // back, it leaves as a blocked route rather than vanishing unannounced.
                    if self.micro.admit(world, h, t1).is_err() {
                        despawned.push((id, DespawnCause::RouteBlocked));
                        released.insert(id);
                    }
                }
            }
        }
        micro_update.states.retain(|(a, _)| !released.contains(a));

        // --- publish -------------------------------------------------------------------
        let mut states = micro_update.states;
        states.extend(meso.states(world, t1));
        states.sort_by_key(|(a, _)| *a);
        self.published = states.iter().copied().collect();
        spawned.extend(micro_update.spawned);
        spawned.sort_by_key(|s| s.actor);
        despawned.extend(micro_update.despawned);
        despawned.sort_by_key(|(a, _)| *a);
        despawned.dedup_by_key(|(a, _)| *a);
        self.meso = Some(meso);
        MobilityUpdate {
            t: t1,
            states,
            spawned,
            despawned,
            signal_states: micro_update.signal_states,
            activities: micro_update.activities,
        }
    }

    fn command(&mut self, _ctx: &mut dyn MobCtx, cmd: MobilityCommand) {
        self.pending.push(cmd);
    }

    fn resume_after_pause(&mut self, paused: Duration) {
        self.micro.resume_after_pause(paused);
        if let Some(core) = self.meso.as_mut() {
            core.resume_after_pause(paused);
        }
        for (_, _, since) in self.micro_waiting.iter_mut().chain(self.meso_waiting.iter_mut()) {
            *since = since.saturating_add(paused.as_nanos());
        }
        if let Some(d) = self.demand.as_mut() {
            d.resume_after_pause(paused);
        }
    }

    fn kinematics(&self, a: ActorId) -> Option<&Kinematics> {
        self.published.get(&a)
    }

    fn set_weather(&mut self, weather: WeatherState) {
        self.micro.set_weather(weather);
    }

    fn set_demand_multiplier(&mut self, m: f64) -> bool {
        self.demand.as_mut().is_some_and(|d| d.set_multiplier(m))
    }

    fn exterior_lamps(&mut self, ctx: &mut dyn MobCtx) -> Vec<(ActorId, u8)> {
        self.micro.exterior_lamps(ctx)
    }

    fn intent(&self, world: &World, a: ActorId) -> Option<crate::views::Intent> {
        self.micro.intent(world, a)
    }

    fn tier_of(&self, a: ActorId) -> Option<Tier> {
        if self.meso.as_ref().is_some_and(|m| m.vehicle(a).is_some()) {
            Some(Tier::Abstract)
        } else {
            Some(Tier::Medium)
        }
    }
}

/// The model card.
#[must_use]
pub fn card(params: &TieredParams) -> ModelCard {
    let src = Source::new(
        SourceKind::Code,
        "this crate: the coupling rules of crate::meso::tiered (hand-in on admission, \
         hand-out with holds), with 02-architecture.md §7.3's focus region as the region",
    );
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        MODEL_VERSION,
        "Microscopic traffic (the native engine) inside a focus region and the mesoscopic \
         queue network everywhere else, with vehicles handed across the boundary under one \
         id, route and driver, and spill-back crossing it both ways.",
    );
    card.tier = vec![Tier::Medium, Tier::Abstract];
    card.parameters = vec![
        Parameter::new("region", "m", serde_json::json!(params.region), src.clone()),
        Parameter::new(
            "meso",
            "-",
            serde_json::to_value(params.meso).unwrap_or_default(),
            Source::new(SourceKind::Code, "see mobility/meso/queue's card"),
        ),
    ];
    card.assumptions = vec![
        "A lane is microscopic when its midpoint is in the region; a junction's internal lane \
         when the junction is."
            .to_string(),
        "A vehicle handed in enters at the start of its next lane at the smaller of its \
         desired speed and the lane's limit, within the step it left its queue."
            .to_string(),
        "A vehicle handed out keeps its position; the queue gives it a free-flow time over \
         the rest of the segment from the end of the step."
            .to_string(),
    ];
    card.limitations = vec![
        "A hand-in is timed to the step it happens in, not to its instant within it.".to_string(),
        "Pedestrians and cyclists stay in the microscopic tier wherever they are.".to_string(),
    ];
    card.sources = vec![src];
    card.determinism = Determinism {
        uses_rng: true,
        rng_domains: vec!["DesiredSpeed".to_string(), "ReactionTime".to_string()],
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec![
            "meso::tests::a_vehicle_crosses_into_the_region_and_out_again_as_one_actor".to_string(),
            "meso::tests::the_tiered_run_is_deterministic".to_string(),
        ],
    };
    card
}
