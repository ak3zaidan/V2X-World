//! `mobility/meso/queue` — the mesoscopic tier: a queue per road segment (city scale).
//!
//! # What it is
//!
//! The vehicle-based queue model of Eissfeldt (2003), the model SUMO's `--mesosim` runs
//! (Krajzewicz et al. 2012, §3.2): every edge is cut into segments of about
//! [`MesoParams::segment_length_m`]; each segment is a FIFO queue with a storage capacity in
//! metres of lane; a vehicle may leave a segment no earlier than its free-flow time over it,
//! only from the head of the queue, only a *headway* after the vehicle before it left, and
//! only into a segment with room. The headway depends on whether the segment being left
//! and the one being entered are *jammed*: the four constants `τ_ff`, `τ_fj`, `τ_jf`, `τ_jj`
//! are what reproduce free flow, the queue discharge and spill-back without a single
//! car-following evaluation. Signals hold the head of the last segment before the stop
//! line while its movement shows anything but green (SUMO `--meso-junction-control`).
//!
//! The cost of a step is a few heap operations per vehicle that *changes segment*, not a
//! neighbour search per vehicle, which is why a mesoscopic tier carries a whole city: a
//! segment change happens every 5-10 s per vehicle, against ten car-following decisions a
//! second in the microscopic tier.
//!
//! # Where it sits
//!
//! [`MesoCore`] is the queue network and nothing else; it is driven two ways:
//!
//! * [`MesoMobility`] runs a whole world mesoscopically — SUMO's `--mesosim` — and is a
//!   [`Mobility`] like every other tier.
//! * [`tiered::TieredMobility`] runs the microscopic engine inside a focus region and this
//!   core everywhere else, handing vehicles across the boundary with their identity, route
//!   and driver (`mobility/tiered/meso-micro`). SUMO has no such coupling within one run;
//!   it is what lets the radio's own focus region (02-architecture.md §7.3) see microscopic
//!   traffic while the rest of the city is still driven.
//!
//! # Positions
//!
//! A queue model has no positions. A radio and a viewer need one, so a vehicle is placed
//! where its queue puts it: moving at its free-flow speed from where it entered until its
//! earliest exit, and from then on standing in the queue at the segment's downstream end,
//! `rank × (length + gap)` back from it, never ahead of a vehicle in front of it in the
//! queue. That is a statement of where the model believes the vehicle is, and the card
//! says it is not a car-following trajectory.
//!
//! # How to add another queue rule
//!
//! The headway law is [`MesoParams::headway_s`], the entry rule is
//! [`MesoCore::can_enter`], and the junction rule is [`MesoCore::junction_allows`]. A
//! variant (a per-lane queue, a link-transmission model) is a new parameter set or a new
//! `Mobility` beside [`MesoMobility`] built on the same [`MesoCore`] — register its card in
//! [`crate::model_cards`] and name it in the engine's `scale.traffic.model` table.
//!
//! # Sources
//!
//! * Eissfeldt, N. G. (2003). *Vehicle-based modelling of traffic: theory and application
//!   to environmental impact modelling.* PhD thesis, Universität zu Köln. — the queue model.
//! * Krajzewicz, D., Erdmann, J., Behrisch, M., Bieker, L. (2012). Recent Development and
//!   Applications of SUMO — Simulation of Urban MObility. *Int. J. Advances in Systems and
//!   Measurements* 5(3&4), 128-138. — MESO in SUMO.
//! * SUMO documentation, "Simulation/Meso" (accessed 2026-10-07): the defaults
//!   `--meso-edgelength 98`, `--meso-tauff 1.13`, `--meso-taufj 1.13`, `--meso-taujf 1.73`,
//!   `--meso-taujj 1.4`, `--meso-jam-threshold -1` and `--meso-junction-control`.

pub mod tiered;

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, VecDeque};

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::geom::{LanePos, Vec3};
use v2xw_core::ids::{ActorId, EdgeId, LaneId};
use v2xw_core::kinematics::Kinematics;
use v2xw_core::math;
use v2xw_core::time::{Duration, SimTime, ns_to_secs};
use v2xw_world::{ClassMask, JunctionControl, LaneKind, SignalState, World};

use crate::classes::VehicleClass;
use crate::ctx::MobCtx;
use crate::engine::DriverTraits;
use crate::error::{MobError, Result};
use crate::routing::dijkstra::{Dijkstra, DijkstraParams, FreeFlowCost};
use crate::traits::{Demand, Mobility};
use crate::views::{
    ActorSpawn, DespawnCause, DriverProfile, MobilityCommand, MobilityUpdate, PhaseState, Route,
    TripRequest,
};

/// The model id.
pub const MODEL_ID: &str = "mobility/meso/queue";

/// The model version.
pub const MODEL_VERSION: &str = "1.0.0";

/// SUMO's default vehicle length plus minimum gap, metres (5 m + 2.5 m): the reference
/// vehicle the jam threshold is stated for (SUMO "Simulation/Meso", `--meso-jam-threshold`).
pub const REFERENCE_VEHICLE_M: f64 = 7.5;

/// How long a trip waits for room at its origin before it is dropped, seconds — the same
/// two minutes the microscopic engine's insertion queue allows (SUMO `--max-depart-delay`).
pub const MAX_DEPART_DELAY_S: f64 = 120.0;

/// The model's parameters, SUMO MESO's own defaults unless stated.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MesoParams {
    /// The longest a segment may be, metres (`--meso-edgelength`, default 98).
    pub segment_length_m: f64,
    /// Headway leaving a free segment into a free one, s (`--meso-tauff`, 1.13).
    pub tau_ff_s: f64,
    /// Headway leaving a free segment into a jammed one, s (`--meso-taufj`, 1.13).
    pub tau_fj_s: f64,
    /// Headway leaving a jammed segment into a free one, s (`--meso-taujf`, 1.73): the queue
    /// discharge headway, 2,080 veh/h/lane.
    pub tau_jf_s: f64,
    /// Headway leaving a jammed segment into a jammed one, s (`--meso-taujj`, 1.4).
    pub tau_jj_s: f64,
    /// The occupancy share above which a segment is jammed. `None` is SUMO's `-1`: jammed
    /// when its vehicles could no longer all drive at the segment's limit with headway
    /// `τ_ff`, i.e. above `L / (v_max·τ_ff + L)` with `L` = [`REFERENCE_VEHICLE_M`].
    pub jam_threshold: Option<f64>,
    /// Whether a signal holds the head of the queue while its movement is not green
    /// (`--meso-junction-control`). Off, a signalised junction is only its headways.
    pub junction_control: bool,
}

impl Default for MesoParams {
    fn default() -> Self {
        Self {
            segment_length_m: 98.0,
            tau_ff_s: 1.13,
            tau_fj_s: 1.13,
            tau_jf_s: 1.73,
            tau_jj_s: 1.4,
            jam_threshold: None,
            junction_control: true,
        }
    }
}

impl MesoParams {
    /// The headway between two departures from a segment, seconds: the four-way table of
    /// Eissfeldt (2003) and SUMO MESO, shared between the segment's lanes.
    #[must_use]
    pub fn headway_s(&self, from_jammed: bool, to_jammed: bool, lanes: u8) -> f64 {
        let tau = match (from_jammed, to_jammed) {
            (false, false) => self.tau_ff_s,
            (false, true) => self.tau_fj_s,
            (true, false) => self.tau_jf_s,
            (true, true) => self.tau_jj_s,
        };
        tau / f64::from(lanes.max(1))
    }

    /// The jam occupancy share of a segment whose limit is `v_max_mps`.
    #[must_use]
    pub fn jam_share(&self, v_max_mps: f64) -> f64 {
        match self.jam_threshold {
            Some(share) => share.clamp(0.0, 1.0),
            None => {
                REFERENCE_VEHICLE_M / (v_max_mps.max(0.1) * self.tau_ff_s + REFERENCE_VEHICLE_M)
            }
        }
    }
}

/// One queue: a stretch of one edge.
#[derive(Debug, Clone)]
pub struct Segment {
    /// The edge it belongs to.
    pub edge: EdgeId,
    /// Its place along the edge, `0` = first.
    pub index: u32,
    /// How many segments the edge has.
    pub of: u32,
    /// The stretch of the edge it covers, as fractions of each lane's length.
    pub frac: (f64, f64),
    /// Its length, metres (of the edge's longest motorised lane).
    pub length_m: f64,
    /// Its motorised lanes.
    pub lanes: u8,
    /// The highest speed limit among them, m/s.
    pub v_max_mps: f64,
    /// Storage: length times lanes, metres.
    pub capacity_m: f64,
    /// The occupancy above which it is jammed, metres.
    pub jam_m: f64,
    /// What its vehicles take up (length plus gap), metres.
    pub occupancy_m: f64,
    /// Its vehicles, head first.
    pub queue: VecDeque<ActorId>,
    /// The earliest the next vehicle may leave: the last departure plus its headway.
    pub free_at: SimTime,
}

impl Segment {
    /// Whether its occupancy is over the jam threshold.
    #[must_use]
    pub fn jammed(&self) -> bool {
        self.occupancy_m > self.jam_m
    }
}

/// One vehicle in the queue network.
#[derive(Debug, Clone, PartialEq)]
pub struct MesoVehicle {
    /// Its id.
    pub id: ActorId,
    /// Its demand-stream position.
    pub seq: u64,
    /// Its class.
    pub class: VehicleClass,
    /// The driver behind the wheel.
    pub driver: DriverProfile,
    /// The driver's traits, carried for a hand-back to the microscopic tier.
    pub traits: DriverTraits,
    /// Its route.
    pub route: Route,
    /// Index into the route of the lane it is on (never an internal lane).
    pub route_index: usize,
    /// The segment it is queued in.
    pub segment: u32,
    /// When it entered the segment.
    pub entered: SimTime,
    /// When it reaches the segment's end at its free-flow speed.
    pub earliest_exit: SimTime,
    /// Where on the segment it entered, as a fraction of the segment's own length.
    pub entry_frac: f64,
    /// When it first appeared in the simulation.
    pub spawned: SimTime,
}

impl MesoVehicle {
    /// Length plus standstill gap: what it occupies in a queue, metres.
    #[must_use]
    pub fn footprint_m(&self) -> f64 {
        self.class.spec().length_m + self.driver.min_gap_m.max(0.0)
    }

    /// The lane it is on.
    #[must_use]
    pub fn lane(&self) -> LaneId {
        self.route.lanes[self.route_index]
    }
}

/// What happened to a vehicle during [`MesoCore::advance`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MesoExit {
    /// It reached the end of its route.
    Completed(ActorId, SimTime),
    /// It left the queue network for a lane the network does not own, and the boundary
    /// took it ([`MesoBoundary::offer`]).
    Handed(ActorId, SimTime),
}

/// The other side of the queue network's edge, for a vehicle about to leave it.
pub trait MesoBoundary {
    /// Whether `lane` is driven by the queue network.
    fn is_meso(&self, lane: LaneId) -> bool;

    /// Offers `vehicle`, about to enter route lane `next_index`, to the tier that owns that
    /// lane at `at`. `true` takes it: the queue network forgets it. `false` leaves it at
    /// the head of its queue, blocking it, as a full segment would.
    fn offer(
        &mut self,
        world: &World,
        vehicle: &MesoVehicle,
        next_index: usize,
        at: SimTime,
    ) -> bool;
}

/// A whole world in the queue network: nothing is ever handed anywhere.
#[derive(Debug, Clone, Copy, Default)]
pub struct Closed;

impl MesoBoundary for Closed {
    fn is_meso(&self, _lane: LaneId) -> bool {
        true
    }

    fn offer(&mut self, _: &World, _: &MesoVehicle, _: usize, _: SimTime) -> bool {
        false
    }
}

/// The queue network: segments, the vehicles in them, and the step that moves them.
#[derive(Debug, Clone)]
pub struct MesoCore {
    params: MesoParams,
    segments: Vec<Segment>,
    /// Per edge (dense index): its first segment and how many it has.
    edge_segments: Vec<(u32, u32)>,
    vehicles: BTreeMap<ActorId, MesoVehicle>,
}

impl MesoCore {
    /// The network for `world`: every edge with a lane `classes` may drive becomes one or
    /// more segments.
    #[must_use]
    pub fn new(world: &World, params: MesoParams, classes: ClassMask) -> Self {
        let mut segments = Vec::new();
        let mut edge_segments = Vec::with_capacity(world.roads.edges().len());
        for edge in world.roads.edges() {
            let lanes: Vec<&v2xw_world::Lane> = edge
                .lanes
                .iter()
                .map(|l| world.lane(*l))
                .filter(|l| {
                    l.kind.is_motorised() && l.kind != LaneKind::Internal && l.admits(classes)
                })
                .collect();
            if lanes.is_empty() || edge.is_internal() {
                edge_segments.push((segments.len() as u32, 0));
                continue;
            }
            let length = lanes.iter().map(|l| l.length_m).fold(0.0_f64, f64::max);
            let v_max = lanes
                .iter()
                .map(|l| l.speed_limit_mps)
                .fold(0.0_f64, f64::max);
            let n = (length / params.segment_length_m.max(1.0)).ceil().max(1.0) as u32;
            let first = segments.len() as u32;
            let seg_len = length / f64::from(n);
            let lane_count = u8::try_from(lanes.len()).unwrap_or(u8::MAX);
            let capacity = seg_len * f64::from(lane_count);
            for i in 0..n {
                segments.push(Segment {
                    edge: edge.id,
                    index: i,
                    of: n,
                    frac: (f64::from(i) / f64::from(n), f64::from(i + 1) / f64::from(n)),
                    length_m: seg_len,
                    lanes: lane_count,
                    v_max_mps: v_max,
                    capacity_m: capacity,
                    jam_m: capacity * params.jam_share(v_max),
                    occupancy_m: 0.0,
                    queue: VecDeque::new(),
                    free_at: 0,
                });
            }
            edge_segments.push((first, n));
        }
        Self {
            params,
            segments,
            edge_segments,
            vehicles: BTreeMap::new(),
        }
    }

    /// The parameters in force.
    #[must_use]
    pub fn params(&self) -> &MesoParams {
        &self.params
    }

    /// Every segment, in network order.
    #[must_use]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Every vehicle, in id order.
    pub fn vehicles(&self) -> impl Iterator<Item = &MesoVehicle> {
        self.vehicles.values()
    }

    /// One vehicle.
    #[must_use]
    pub fn vehicle(&self, id: ActorId) -> Option<&MesoVehicle> {
        self.vehicles.get(&id)
    }

    /// How many vehicles are in the network.
    #[must_use]
    pub fn len(&self) -> usize {
        self.vehicles.len()
    }

    /// True if none are.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.vehicles.is_empty()
    }

    /// The segment of `lane` that holds the point `s_m` along it, if the lane is in the
    /// network.
    #[must_use]
    pub fn segment_at(&self, world: &World, lane: LaneId, s_m: f64) -> Option<u32> {
        let l = world.try_lane(lane)?;
        let (first, n) = *self.edge_segments.get(l.edge.as_usize())?;
        if n == 0 {
            return None;
        }
        let frac = (s_m / l.length_m.max(1e-9)).clamp(0.0, 1.0);
        let i = ((frac * f64::from(n)) as u32).min(n - 1);
        Some(first + i)
    }

    /// Whether a vehicle occupying `footprint_m` may enter `segment` now: it has room, or
    /// it is empty (so a vehicle longer than a short segment is never locked out of it).
    #[must_use]
    pub fn can_enter(&self, segment: u32, footprint_m: f64) -> bool {
        let s = &self.segments[segment as usize];
        s.queue.is_empty() || s.occupancy_m + footprint_m <= s.capacity_m
    }

    /// The free-flow speed of `v` on segment `seg`, m/s.
    fn free_speed(&self, v: &MesoVehicle, seg: u32) -> f64 {
        let s = &self.segments[seg as usize];
        let limit = s.v_max_mps * v.traits.speed_factor;
        v.driver.desired_speed_mps.min(limit).max(0.5)
    }

    /// Puts `vehicle` into the segment of its current route lane that holds `s_m`, at `t`.
    /// With `force`, a full segment takes it anyway (a hand-over from a tier that could not
    /// be stopped); otherwise a full segment refuses and the vehicle comes back.
    ///
    /// # Errors
    /// The vehicle itself, when its lane is not in the network or the segment is full.
    pub fn insert(
        &mut self,
        world: &World,
        mut vehicle: MesoVehicle,
        s_m: f64,
        t: SimTime,
        force: bool,
    ) -> core::result::Result<(), Box<MesoVehicle>> {
        let lane = vehicle.lane();
        let Some(seg) = self.segment_at(world, lane, s_m) else {
            return Err(Box::new(vehicle));
        };
        let footprint = vehicle.footprint_m();
        if !force && !self.can_enter(seg, footprint) {
            return Err(Box::new(vehicle));
        }
        let lane_len = world.lane(lane).length_m.max(1e-9);
        let s = &self.segments[seg as usize];
        let seg_frac =
            ((s_m / lane_len - s.frac.0) / (s.frac.1 - s.frac.0).max(1e-12)).clamp(0.0, 1.0);
        vehicle.segment = seg;
        vehicle.entered = t;
        vehicle.entry_frac = seg_frac;
        let v = self.free_speed(&vehicle, seg);
        let remaining = (1.0 - seg_frac) * self.segments[seg as usize].length_m;
        vehicle.earliest_exit = Duration::from_secs_f64(remaining / v).after(t);
        let s = &mut self.segments[seg as usize];
        s.occupancy_m += footprint;
        s.queue.push_back(vehicle.id);
        self.vehicles.insert(vehicle.id, vehicle);
        Ok(())
    }

    /// Takes a vehicle out of the network wherever it is.
    pub fn remove(&mut self, id: ActorId) -> Option<MesoVehicle> {
        let v = self.vehicles.remove(&id)?;
        let s = &mut self.segments[v.segment as usize];
        if let Some(pos) = s.queue.iter().position(|q| *q == id) {
            s.queue.remove(pos);
        }
        s.occupancy_m = (s.occupancy_m - v.footprint_m()).max(0.0);
        if s.queue.is_empty() {
            s.occupancy_m = 0.0;
        }
        Some(v)
    }

    /// Whether the junction at the end of route lane `index` lets `v` through at `at`:
    /// no signal, or its movement shows green.
    #[must_use]
    pub fn junction_allows(
        &self,
        world: &World,
        v: &MesoVehicle,
        index: usize,
        at: SimTime,
    ) -> bool {
        if !self.params.junction_control {
            return true;
        }
        let lane = world.lane(v.route.lanes[index]);
        let junction = world.edge(lane.edge).to;
        let Some(j) = world.roads.try_junction(junction) else {
            return true;
        };
        let JunctionControl::Signalised { plan } = j.control else {
            return true;
        };
        let Some(plan) = world.signal_plan(plan) else {
            return true;
        };
        // The movement: the connector the route takes, or the first one off this lane
        // to the next route lane.
        let next = v.route.lanes.get(index + 1).copied();
        let movement = next.and_then(|n| {
            if world.lane(n).kind == LaneKind::Internal {
                Some(n)
            } else {
                world
                    .successors(lane.id)
                    .iter()
                    .find(|c| c.to_lane == n)
                    .map(|c| c.via.unwrap_or(n))
            }
        });
        let Some(movement) = movement else {
            return true;
        };
        let Some(i) = plan.controlled.iter().position(|l| *l == movement) else {
            return true;
        };
        match plan
            .states_at(ns_to_secs(at))
            .and_then(|s| s.get(i).copied())
        {
            Some(SignalState::Green | SignalState::GreenYield) | None => true,
            Some(_) => false,
        }
    }

    /// The next non-internal route index after `index`, and the length and speed limit of
    /// the connectors driven to reach it.
    fn next_lane(world: &World, route: &Route, index: usize) -> Option<(usize, f64, f64)> {
        let mut k = index + 1;
        let mut conn_len = 0.0;
        let mut conn_v = f64::INFINITY;
        while let Some(l) = route.lanes.get(k) {
            let lane = world.lane(*l);
            if lane.kind != LaneKind::Internal {
                return Some((
                    k,
                    conn_len,
                    if conn_v.is_finite() {
                        conn_v
                    } else {
                        lane.speed_limit_mps
                    },
                ));
            }
            conn_len += lane.length_m;
            conn_v = conn_v.min(lane.speed_limit_mps);
            k += 1;
        }
        None
    }

    /// When the head of `seg` may next leave, if it has one.
    fn candidate(&self, seg: u32) -> Option<SimTime> {
        let s = &self.segments[seg as usize];
        let head = s.queue.front()?;
        let v = &self.vehicles[head];
        Some(v.earliest_exit.max(s.free_at))
    }

    /// Moves every vehicle whose time to leave its segment falls in `[t0, t1)`, in time
    /// order (ties by segment index), and returns what left the network.
    ///
    /// A head that cannot leave — red signal, full next segment, a boundary that refuses —
    /// is retried at the next step. That quantises a *blocked* departure to the step; a
    /// free one keeps its exact instant.
    pub fn advance(
        &mut self,
        world: &World,
        t0: SimTime,
        t1: SimTime,
        boundary: &mut dyn MesoBoundary,
    ) -> Vec<MesoExit> {
        let mut heap: BinaryHeap<Reverse<(SimTime, u32)>> = BinaryHeap::new();
        for seg in 0..self.segments.len() as u32 {
            if let Some(at) = self.candidate(seg)
                && at < t1
            {
                heap.push(Reverse((at.max(t0), seg)));
            }
        }
        let mut exits = Vec::new();
        // Segments whose head was refused this step: not retried until the next one.
        let mut held = std::collections::BTreeSet::new();
        while let Some(Reverse((at, seg))) = heap.pop() {
            if held.contains(&seg) {
                continue;
            }
            // The entry may be stale: the head or its gate changed since it was pushed.
            let Some(now) = self.candidate(seg).map(|c| c.max(t0)) else {
                continue;
            };
            if now != at {
                if now < t1 {
                    heap.push(Reverse((now, seg)));
                }
                continue;
            }
            let head = *self.segments[seg as usize]
                .queue
                .front()
                .expect("candidate has a head");
            let (edge_last, from_jammed, lanes) = {
                let s = &self.segments[seg as usize];
                (s.index + 1 == s.of, s.jammed(), s.lanes)
            };
            // Where it goes.
            enum To {
                Segment(u32, Option<(usize, f64, f64)>),
                Done,
                Boundary(usize),
            }
            let to = {
                let v = &self.vehicles[&head];
                if !edge_last {
                    To::Segment(seg + 1, None)
                } else {
                    match Self::next_lane(world, &v.route, v.route_index) {
                        None => To::Done,
                        Some((k, len, vc)) => {
                            if !self.junction_allows(world, v, v.route_index, at) {
                                held.insert(seg);
                                continue;
                            }
                            let lane = v.route.lanes[k];
                            // The first lane past this one that the other tier owns: the
                            // junction's connector when the junction is the other tier's,
                            // so the vehicle crosses it there rather than appearing past it.
                            let handed = (v.route_index + 1..=k)
                                .find(|i| !boundary.is_meso(v.route.lanes[*i]));
                            if let Some(i) = handed {
                                To::Boundary(i)
                            } else {
                                match self.segment_at(world, lane, 0.0) {
                                    Some(next) => To::Segment(next, Some((k, len, vc))),
                                    // A lane the network does not hold (a class-only lane):
                                    // the trip ends where the network does.
                                    None => To::Done,
                                }
                            }
                        }
                    }
                }
            };
            match to {
                To::Done => {
                    self.depart(seg, head, at, from_jammed, false, lanes);
                    self.vehicles.remove(&head);
                    exits.push(MesoExit::Completed(head, at));
                }
                To::Boundary(k) => {
                    let v = self.vehicles[&head].clone();
                    if boundary.offer(world, &v, k, at) {
                        self.depart(seg, head, at, from_jammed, false, lanes);
                        self.vehicles.remove(&head);
                        exits.push(MesoExit::Handed(head, at));
                    } else {
                        held.insert(seg);
                        continue;
                    }
                }
                To::Segment(next, via) => {
                    let footprint = self.vehicles[&head].footprint_m();
                    if !self.can_enter(next, footprint) {
                        held.insert(seg);
                        continue;
                    }
                    let to_jammed = self.segments[next as usize].jammed();
                    self.depart(seg, head, at, from_jammed, to_jammed, lanes);
                    let mut entry = at;
                    {
                        let v = self.vehicles.get_mut(&head).expect("present");
                        if let Some((k, conn_len, conn_v)) = via {
                            v.route_index = k;
                            let speed = v.driver.desired_speed_mps.min(conn_v).max(0.5);
                            entry = Duration::from_secs_f64(conn_len / speed).after(at);
                        }
                        v.segment = next;
                        v.entered = entry;
                        v.entry_frac = 0.0;
                    }
                    let speed = self.free_speed(&self.vehicles[&head], next);
                    let len = self.segments[next as usize].length_m;
                    let v = self.vehicles.get_mut(&head).expect("present");
                    v.earliest_exit = Duration::from_secs_f64(len / speed).after(entry);
                    let s = &mut self.segments[next as usize];
                    s.occupancy_m += footprint;
                    s.queue.push_back(head);
                    if s.queue.len() == 1
                        && let Some(c) = self.candidate(next)
                        && c < t1
                    {
                        heap.push(Reverse((c.max(t0), next)));
                    }
                }
            }
            if let Some(c) = self.candidate(seg)
                && c < t1
            {
                heap.push(Reverse((c.max(t0), seg)));
            }
        }
        exits
    }

    /// The head of `seg` leaves at `at`: the queue shortens and the gate closes for one
    /// headway.
    fn depart(
        &mut self,
        seg: u32,
        head: ActorId,
        at: SimTime,
        from_jammed: bool,
        to_jammed: bool,
        lanes: u8,
    ) {
        let footprint = self.vehicles[&head].footprint_m();
        let headway = self.params.headway_s(from_jammed, to_jammed, lanes);
        let s = &mut self.segments[seg as usize];
        s.queue.pop_front();
        s.occupancy_m = (s.occupancy_m - footprint).max(0.0);
        if s.queue.is_empty() {
            s.occupancy_m = 0.0;
        }
        s.free_at = Duration::from_secs_f64(headway).after(at);
    }

    /// Where every vehicle is at `t`, in id order (see the module documentation for the
    /// placement rule).
    #[must_use]
    pub fn states(&self, world: &World, t: SimTime) -> Vec<(ActorId, Kinematics)> {
        let mut out: Vec<(ActorId, Kinematics)> = Vec::with_capacity(self.vehicles.len());
        for seg in 0..self.segments.len() {
            self.segment_states(world, seg, t, &mut out);
        }
        out.sort_by_key(|(a, _)| *a);
        out
    }

    /// Where one vehicle is at `t`.
    #[must_use]
    pub fn state_of(&self, world: &World, id: ActorId, t: SimTime) -> Option<Kinematics> {
        let v = self.vehicles.get(&id)?;
        let mut out = Vec::new();
        self.segment_states(world, v.segment as usize, t, &mut out);
        out.into_iter().find(|(a, _)| *a == id).map(|(_, k)| k)
    }

    /// The states of one segment's vehicles, queue order.
    fn segment_states(
        &self,
        world: &World,
        seg: usize,
        t: SimTime,
        out: &mut Vec<(ActorId, Kinematics)>,
    ) {
        let seg = &self.segments[seg];
        if seg.queue.is_empty() {
            return;
        }
        // Front-bumper distance back from the segment's end the next vehicle in the queue
        // may stand at, per lane column: it grows by each vehicle's footprint.
        let mut back_m = vec![0.0_f64; usize::from(seg.lanes.max(1))];
        for (rank, id) in seg.queue.iter().enumerate() {
            let v = &self.vehicles[id];
            let column = rank % back_m.len();
            let travel = ns_to_secs(v.earliest_exit.saturating_sub(v.entered));
            let since = ns_to_secs(t.saturating_sub(v.entered));
            let p = if travel > 0.0 {
                (since / travel).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let along = v.entry_frac + (1.0 - v.entry_frac) * p;
            let free_s = along * seg.length_m;
            let queued_s = (seg.length_m - back_m[column]).max(0.0);
            let s_seg = free_s.min(queued_s);
            let moving = p < 1.0 && free_s <= queued_s;
            back_m[column] += v.footprint_m();
            let speed = if moving && travel > 0.0 {
                (1.0 - v.entry_frac) * seg.length_m / travel
            } else {
                0.0
            };
            out.push((v.id, Self::kinematics(world, v, seg, s_seg, speed, t)));
        }
    }

    /// One vehicle's state at `s_seg` metres into its segment.
    fn kinematics(
        world: &World,
        v: &MesoVehicle,
        seg: &Segment,
        s_seg: f64,
        speed: f64,
        t: SimTime,
    ) -> Kinematics {
        let lane = world.lane(v.lane());
        let frac = seg.frac.0 + (seg.frac.1 - seg.frac.0) * (s_seg / seg.length_m.max(1e-9));
        let front = frac * lane.length_m;
        let length = v.class.spec().length_m;
        let s_rear = (front - length).clamp(0.0, lane.length_m);
        let (pos, heading) = lane.pose_at(s_rear);
        let (sin_h, cos_h) = math::sin_cos(heading);
        Kinematics {
            t,
            pos,
            vel: Vec3::new(speed * cos_h, speed * sin_h, 0.0),
            acc: Vec3::ZERO,
            heading_rad: heading,
            yaw_rate_rad_s: 0.0,
            lane: Some(LanePos::centred(lane.id, s_rear)),
            dims: v.class.dims(),
        }
    }
}

// =========================================================================================
// The whole-world mesoscopic tier
// =========================================================================================

/// A whole world in the queue network: SUMO's `--mesosim` as a [`Mobility`].
pub struct MesoMobility {
    core: Option<MesoCore>,
    params: MesoParams,
    router: Dijkstra,
    demand: Option<Box<dyn Demand>>,
    /// Trips with no room at their origin yet, with the instant they first asked.
    waiting: Vec<(MesoVehicle, f64, SimTime)>,
    published: BTreeMap<ActorId, Kinematics>,
    pending: Vec<MobilityCommand>,
    next_actor: u32,
    dropped_trips: u64,
    card: ModelCard,
}

impl core::fmt::Debug for MesoMobility {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MesoMobility")
            .field("params", &self.params)
            .field("vehicles", &self.core.as_ref().map_or(0, MesoCore::len))
            .finish_non_exhaustive()
    }
}

impl MesoMobility {
    /// The tier with the given parameters.
    #[must_use]
    pub fn new(params: MesoParams) -> Self {
        Self {
            core: None,
            card: card(&params),
            params,
            router: Dijkstra::new(DijkstraParams::for_classes(ClassMask::MOTOR_TRAFFIC)),
            demand: None,
            waiting: Vec::new(),
            published: BTreeMap::new(),
            pending: Vec::new(),
            next_actor: 0,
            dropped_trips: 0,
        }
    }

    /// The queue network, once [`Mobility::init`] has built it.
    #[must_use]
    pub fn core(&self) -> Option<&MesoCore> {
        self.core.as_ref()
    }

    /// Trips dropped for want of room or of a route.
    #[must_use]
    pub fn dropped_trips(&self) -> u64 {
        self.dropped_trips
    }
}

/// A trip as a queue-network vehicle: routed on free-flow cost, not yet in a segment.
pub(crate) fn vehicle_for_trip(
    router: &Dijkstra,
    world: &World,
    trip: &TripRequest,
    id: ActorId,
    driver: DriverProfile,
    traits: DriverTraits,
) -> Option<MesoVehicle> {
    let route = router.search(
        world,
        trip.origin,
        trip.destination,
        trip.t,
        &FreeFlowCost::new(world),
    )?;
    Some(MesoVehicle {
        id,
        seq: trip.seq,
        class: trip.class,
        driver: DriverProfile {
            desired_speed_mps: trip.desired_speed_mps,
            ..driver
        },
        traits,
        route,
        route_index: 0,
        segment: 0,
        entered: trip.t,
        earliest_exit: trip.t,
        entry_frac: 0.0,
        spawned: trip.t,
    })
}

/// Routes a step's trips on the thread pool and returns them in trip order: a route is
/// a pure function of the world and the trip, so a city's start-up of thousands of
/// Dijkstra searches uses every core and the result does not depend on how many there
/// are (`crate::par`).
pub(crate) fn route_batch(
    router: &Dijkstra,
    world: &World,
    batch: Vec<(TripRequest, ActorId, DriverProfile, DriverTraits)>,
) -> Vec<(TripRequest, Option<MesoVehicle>)> {
    let routed = crate::par::map_ordered_from(&batch, 2, |(trip, id, driver, traits)| {
        vehicle_for_trip(router, world, trip, *id, *driver, *traits)
    });
    batch.into_iter().map(|(t, ..)| t).zip(routed).collect()
}

/// The driver profile a queue-network vehicle of `class` carries: the class table's own
/// values, which is all a queue model reads (its desired speed and standstill gap).
pub(crate) fn class_driver(class: VehicleClass) -> DriverProfile {
    let spec = class.spec();
    DriverProfile {
        desired_speed_mps: spec.desired_max_speed_mps.unwrap_or(spec.max_speed_mps),
        max_accel_mps2: spec.accel_mps2,
        comfort_decel_mps2: spec.decel_mps2,
        time_headway_s: 1.0,
        min_gap_m: spec.min_gap_m,
    }
}

impl v2xw_core::model::Model for MesoMobility {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl Mobility for MesoMobility {
    fn tier(&self) -> Tier {
        Tier::Abstract
    }

    fn init(&mut self, ctx: &mut dyn MobCtx, demand: Box<dyn Demand>) -> Result<()> {
        let core = MesoCore::new(ctx.world(), self.params, ClassMask::MOTOR_TRAFFIC);
        if core.segments().is_empty() {
            return Err(MobError::EmptyWorld {
                what: "drivable edge for the queue network",
            });
        }
        self.core = Some(core);
        self.demand = Some(demand);
        Ok(())
    }

    fn step(&mut self, ctx: &mut dyn MobCtx, dt: Duration) -> MobilityUpdate {
        let t0 = ctx.now();
        let t1 = dt.after(t0);
        let Some(mut core) = self.core.take() else {
            return MobilityUpdate::empty(t1);
        };
        let mut despawned: Vec<(ActorId, DespawnCause)> = Vec::new();
        let mut trips: Vec<TripRequest> = Vec::new();
        for command in core::mem::take(&mut self.pending) {
            match command {
                MobilityCommand::Despawn { actor } => {
                    if core.remove(actor).is_some() {
                        despawned.push((actor, DespawnCause::Commanded));
                    }
                }
                MobilityCommand::Spawn(trip) => trips.push(trip),
                // Speed caps, stops, reroutes and closures are interaction a queue model
                // does not represent; the card says so.
                _ => {}
            }
        }
        if let Some(mut demand) = self.demand.take() {
            trips.extend(demand.spawns_in(ctx, t0, t1));
            self.demand = Some(demand);
        }
        let world = ctx.world();
        let mut queue = core::mem::take(&mut self.waiting);
        let mut to_route = Vec::with_capacity(trips.len());
        for trip in trips {
            let id = ActorId::new(self.next_actor);
            self.next_actor += 1;
            let traits = DriverTraits::draw(&*ctx, trip.seq, trip.class);
            let driver = class_driver(trip.class);
            to_route.push((trip, id, driver, traits));
        }
        for (trip, routed) in route_batch(&self.router, world, to_route) {
            match routed {
                Some(v) => queue.push((v, trip.origin_s_m.max(trip.class.spec().length_m), t0)),
                None => self.dropped_trips += 1,
            }
        }
        let mut new_ids = Vec::new();
        for (mut v, s_m, since) in queue {
            v.spawned = t0;
            let id = v.id;
            match core.insert(world, v, s_m, t0, false) {
                Ok(()) => new_ids.push(id),
                Err(v) => {
                    if ns_to_secs(t0.saturating_sub(since)) < MAX_DEPART_DELAY_S {
                        self.waiting.push((*v, s_m, since));
                    } else {
                        self.dropped_trips += 1;
                    }
                }
            }
        }
        let at_t0: BTreeMap<ActorId, Kinematics> = core.states(world, t0).into_iter().collect();
        let mut spawned: Vec<ActorSpawn> = new_ids
            .iter()
            .filter_map(|id| {
                let v = core.vehicle(*id)?;
                Some(ActorSpawn {
                    actor: *id,
                    t: t0,
                    class: v.class,
                    kinematics: at_t0.get(id).copied()?,
                    route: v.route.clone(),
                    driver: v.driver,
                    seq: v.seq,
                })
            })
            .collect();
        for exit in core.advance(world, t0, t1, &mut Closed) {
            if let MesoExit::Completed(id, _) = exit {
                despawned.push((id, DespawnCause::TripComplete));
            }
        }
        let states = core.states(world, t1);
        self.published = states.iter().copied().collect();
        self.core = Some(core);
        spawned.sort_by_key(|s| s.actor);
        despawned.sort_by_key(|(a, _)| *a);
        MobilityUpdate {
            t: t1,
            states,
            spawned,
            despawned,
            signal_states: signal_states(world, t1),
            activities: Vec::new(),
        }
    }

    fn command(&mut self, _ctx: &mut dyn MobCtx, cmd: MobilityCommand) {
        self.pending.push(cmd);
    }

    fn kinematics(&self, a: ActorId) -> Option<&Kinematics> {
        self.published.get(&a)
    }
}

/// Every signal plan's state at `t`, in signal order — what a viewer draws.
pub(crate) fn signal_states(
    world: &World,
    t: SimTime,
) -> Vec<(v2xw_core::ids::SignalId, PhaseState)> {
    let t_s = ns_to_secs(t);
    let mut out: Vec<_> = world
        .signals
        .iter()
        .filter_map(|plan| {
            let (phase, elapsed) = plan.phase_at(t_s)?;
            let p = &plan.phases[phase];
            Some((
                plan.id,
                PhaseState {
                    phase,
                    elapsed_s: elapsed,
                    remaining_s: (p.duration_s - elapsed).max(0.0),
                    states: p.states.clone(),
                },
            ))
        })
        .collect();
    out.sort_by_key(|(id, _)| *id);
    out
}

/// The model card.
#[must_use]
pub fn card(params: &MesoParams) -> ModelCard {
    let sumo = Source {
        kind: SourceKind::Code,
        reference: "SUMO documentation, Simulation/Meso: --meso-edgelength 98, --meso-tauff \
                    1.13, --meso-taufj 1.13, --meso-taujf 1.73, --meso-taujj 1.4, \
                    --meso-jam-threshold -1, --meso-junction-control"
            .to_string(),
        accessed: Some("2026-10-07".to_string()),
        note: None,
    };
    let eissfeldt = Source {
        kind: SourceKind::Paper,
        reference: "Eissfeldt, N. G. (2003). Vehicle-based modelling of traffic: theory and \
                    application to environmental impact modelling. PhD thesis, Universität zu \
                    Köln."
            .to_string(),
        accessed: None,
        note: Some(
            "the queue model: segment capacity, free-flow exit time, jam-dependent headways"
                .to_string(),
        ),
    };
    let krajzewicz = Source {
        kind: SourceKind::Paper,
        reference: "Krajzewicz, D., Erdmann, J., Behrisch, M., Bieker, L. (2012). Recent \
                    Development and Applications of SUMO. Int. J. Advances in Systems and \
                    Measurements 5(3&4), 128-138."
            .to_string(),
        accessed: None,
        note: Some("MESO, SUMO's mesoscopic mode".to_string()),
    };
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        MODEL_VERSION,
        "The mesoscopic tier: each edge is a chain of FIFO queues; a vehicle leaves a segment \
         no earlier than its free-flow time, from the head only, a jam-dependent headway \
         after the one before, into a segment with room, and through a signal only on green.",
    );
    card.tier = vec![Tier::Abstract];
    card.equations = vec![
        Equation {
            name: "exit time".to_string(),
            latex_or_text: "t_exit = max(t_entry + ℓ / min(v_desired, v_max·f_speed), t_last + τ_xy / lanes, t_green, t_room)".to_string(),
            notes: Some("x, y ∈ {free, jammed} for this segment and the next".to_string()),
        },
        Equation {
            name: "jam".to_string(),
            latex_or_text: "jammed ⇔ occupancy > capacity · L / (v_max·τ_ff + L), L = 7.5 m".to_string(),
            notes: Some("SUMO's --meso-jam-threshold -1".to_string()),
        },
    ];
    let p = |name: &str, unit: &str, value: serde_json::Value| {
        Parameter::new(name, unit, value, sumo.clone())
    };
    card.parameters = vec![
        p(
            "segment_length_m",
            "m",
            serde_json::json!(params.segment_length_m),
        ),
        p("tau_ff_s", "s", serde_json::json!(params.tau_ff_s)),
        p("tau_fj_s", "s", serde_json::json!(params.tau_fj_s)),
        p("tau_jf_s", "s", serde_json::json!(params.tau_jf_s)),
        p("tau_jj_s", "s", serde_json::json!(params.tau_jj_s)),
        p(
            "jam_threshold",
            "-",
            serde_json::json!(params.jam_threshold),
        ),
        p(
            "junction_control",
            "-",
            serde_json::json!(params.junction_control),
        ),
    ];
    card.assumptions = vec![
        "One FIFO queue per segment shared by its lanes (Eissfeldt's original), not SUMO's \
         later per-lane queues; the headway is divided by the lane count."
            .to_string(),
        "A blocked departure (red, full segment, refusing boundary) is retried at the next \
         mobility step; a free one keeps its exact instant."
            .to_string(),
        "Positions are the queue's placement, not a trajectory: free-flow motion to the \
         earliest exit, then standing in the queue at the segment's end."
            .to_string(),
    ];
    card.limitations = vec![
        "No lane changes, overtaking within a segment, or acceleration: speed is free-flow \
         or zero."
            .to_string(),
        "Unsignalised junctions are only their headways: no priority between approaches."
            .to_string(),
    ];
    card.ignores = vec![
        "Speed caps, stops, reroutes and closures sent as commands.".to_string(),
        "Weather.".to_string(),
    ];
    card.sources = vec![eissfeldt, krajzewicz, sumo];
    card.determinism = Determinism {
        uses_rng: true,
        rng_domains: vec!["DesiredSpeed".to_string(), "ReactionTime".to_string()],
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec![
            "meso::tests::a_free_segment_is_crossed_at_free_flow".to_string(),
            "meso::tests::a_queue_discharges_at_the_jam_headway".to_string(),
            "meso::tests::a_full_segment_holds_back_the_one_upstream".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests;
