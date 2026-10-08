//! What the microscopic engine offers a tier coupled to it (`crate::meso::tiered`).
//!
//! Four seams, and nothing in the step changes for a run that does not use them:
//!
//! * [`NativeMobility::reserve_actor_id`] — one id space for both tiers, so a vehicle keeps
//!   its id (and the engine its node, certificates and records) across the boundary;
//! * [`NativeMobility::admit_trip`] — a demand trip that starts inside the region, placed
//!   by the engine's own insertion rules under an id the caller reserved;
//! * [`NativeMobility::admit`] / [`NativeMobility::release`] — a vehicle handed in from,
//!   or out to, the other tier, with its route, driver and speed;
//! * [`NativeMobility::set_holds`] — lanes a vehicle may not enter because the other tier
//!   has no room for it: each is stopped short of like a closed lane, **without** the
//!   detour a closure triggers, so spill-back crosses the boundary the way it crosses a
//!   junction.

use super::*;

/// A vehicle crossing between the microscopic engine and another tier.
#[derive(Debug, Clone, PartialEq)]
pub struct Handoff {
    /// Its id, kept across the crossing.
    pub id: ActorId,
    /// Its demand-stream position.
    pub seq: u64,
    /// Its class.
    pub class: VehicleClass,
    /// The driver behind the wheel.
    pub driver: DriverProfile,
    /// The driver's traits.
    pub traits: DriverTraits,
    /// Its route.
    pub route: Route,
    /// The route lane it is on (or, handed in, enters).
    pub route_index: usize,
    /// Its front bumper along that lane, metres.
    pub s_m: f64,
    /// Its speed, m/s.
    pub speed_mps: f64,
    /// When it first appeared in the simulation.
    pub spawned: SimTime,
}

/// What [`NativeMobility::admit_trip`] did with a trip.
#[derive(Debug, Clone, PartialEq)]
pub enum TripAdmission {
    /// On the road.
    Placed(ActorSpawn),
    /// Its origin has no room now; the caller may retry.
    Occupied,
    /// No route.
    Unroutable,
}

impl NativeMobility {
    /// Takes the next actor id, so a tier coupled to this engine shares its id space.
    pub fn reserve_actor_id(&mut self) -> ActorId {
        let id = ActorId::new(self.next_actor);
        self.next_actor += 1;
        id
    }

    /// Replaces the set of lanes no vehicle may enter (see the module documentation).
    pub fn set_holds(&mut self, holds: BTreeSet<LaneId>) {
        self.holds = holds;
    }

    /// The lanes currently held.
    #[must_use]
    pub fn holds(&self) -> &BTreeSet<LaneId> {
        &self.holds
    }

    /// Every vehicle this engine drives (not the pedestrians) with its lane and class, in
    /// id order.
    #[must_use]
    pub fn vehicle_lanes(&self) -> Vec<(ActorId, LaneId, VehicleClass)> {
        self.actors
            .values()
            .map(|a| (a.id, a.lane, a.class))
            .collect()
    }

    /// The driver this engine puts behind the wheel for `trip`: its car-following model's
    /// profile for the class at the trip's desired speed, and the driver's traits drawn
    /// from the trip's own streams — exactly what an insertion here would give it.
    pub fn trip_driver(
        &self,
        ctx: &dyn MobCtx,
        trip: &TripRequest,
    ) -> (DriverProfile, DriverTraits) {
        let driver = DriverProfile {
            desired_speed_mps: trip.desired_speed_mps,
            ..self.driver_for(trip.class)
        };
        (driver, self.traits_for(ctx, trip))
    }

    /// The spawn announcement of actor `id`, at `t`, if this engine drives it.
    #[must_use]
    pub fn spawn_record(&self, world: &World, id: ActorId, t: SimTime) -> Option<ActorSpawn> {
        self.actors
            .contains_key(&id)
            .then(|| self.actor_spawn(world, id, t))
    }

    /// Places a demand trip as actor `id` by the engine's own insertion rules.
    pub fn admit_trip(
        &mut self,
        ctx: &dyn MobCtx,
        trip: &TripRequest,
        id: ActorId,
        t: SimTime,
    ) -> TripAdmission {
        let traits = self.traits_for(ctx, trip);
        let world = ctx.world();
        match self.try_insert(world, trip, t, Some(id), traits) {
            Insertion::Placed(id) => TripAdmission::Placed(self.actor_spawn(world, id, t)),
            Insertion::Occupied => TripAdmission::Occupied,
            Insertion::Unroutable => TripAdmission::Unroutable,
        }
    }

    /// Takes a vehicle out of this engine, for the other tier.
    pub fn release(&mut self, id: ActorId) -> Option<Handoff> {
        let a = self.actors.remove(&id)?;
        self.published.remove(&id);
        self.lamp_memory.remove(&id);
        self.midblock_yields.retain(|(_, veh), _| *veh != id);
        Some(Handoff {
            id,
            seq: a.seq,
            class: a.class,
            driver: a.driver,
            traits: a.traits,
            route_index: a.route_index.min(a.route.lanes.len().saturating_sub(1)),
            route: a.route,
            s_m: a.s_m,
            speed_mps: a.speed_mps,
            spawned: a.spawned,
        })
    }

    /// Puts a vehicle handed in from the other tier on its route lane, at its speed, if
    /// there is room: it can follow whoever is ahead at its desired gap, and whoever is
    /// behind can follow it ([`Self::may_follow_newcomer`]). `Err` gives it back.
    ///
    /// # Errors
    /// The hand-off itself, when there is no room.
    pub fn admit(
        &mut self,
        world: &World,
        h: Handoff,
        t: SimTime,
    ) -> core::result::Result<(), Box<Handoff>> {
        let Some(&lane_id) = h.route.lanes.get(h.route_index) else {
            return Err(Box::new(h));
        };
        let Some(lane) = world.try_lane(lane_id) else {
            return Err(Box::new(h));
        };
        let length = h.class.spec().length_m;
        let front = h.s_m.clamp(length.min(lane.length_m), lane.length_m);
        let rear = front - length;
        let speed = h
            .speed_mps
            .min(lane.speed_limit_mps * h.traits.speed_factor)
            .max(0.0);
        // The newcomer's own following gap to whoever is ahead.
        let need_ahead = h.driver.min_gap_m + speed * h.driver.time_headway_s;
        for a in self.actors.values() {
            if a.lane == lane_id {
                let a_rear = a.s_m - a.class.spec().length_m;
                if a.s_m >= front {
                    if a_rear - front < need_ahead {
                        return Err(Box::new(h));
                    }
                } else {
                    let gap = rear - a.s_m;
                    if gap < self.params.insertion_gap_m || !self.may_follow_newcomer(world, a, gap)
                    {
                        return Err(Box::new(h));
                    }
                }
            }
        }
        // A body still overhanging this lane from the lane after it.
        for c in world.successors(lane_id) {
            let next = c.via.unwrap_or(c.to_lane);
            for a in self.actors.values().filter(|a| a.lane == next) {
                let rear_on = lane.length_m + a.s_m - a.class.spec().length_m;
                if rear_on - front
                    < need_ahead.min(self.params.insertion_gap_m.max(h.driver.min_gap_m))
                {
                    return Err(Box::new(h));
                }
            }
        }
        // Traffic about to arrive from the lanes that feed this one.
        for c in world.predecessors(lane_id) {
            let from = c.via.unwrap_or(c.from_lane);
            let Some(up) = world.try_lane(from) else {
                continue;
            };
            for a in self.actors.values().filter(|a| a.lane == from) {
                let gap = (up.length_m - a.s_m) + rear;
                if !self.may_follow_newcomer(world, a, gap) {
                    return Err(Box::new(h));
                }
            }
        }
        let destination = *h
            .route
            .lanes
            .last()
            .expect("route has the lane checked above");
        self.next_seq = self.next_seq.max(h.seq + 1);
        self.actors.insert(
            h.id,
            Actor {
                id: h.id,
                seq: h.seq,
                class: h.class,
                driver: h.driver,
                route: h.route,
                route_index: h.route_index,
                destination,
                lane: lane_id,
                s_m: front,
                lateral_m: self.lane_position_m(h.class),
                speed_mps: speed,
                accel_mps2: 0.0,
                spawned: h.spawned,
                planned_at: t,
                planned_generation: 0,
                transition: None,
                cooldown_until: t,
                speed_cap_mps: None,
                stopped_until: None,
                trail: Vec::new(),
                amber_commit: None,
                amber_seen: None,
                traits: h.traits,
                release_at: None,
                forced_brake: None,
                forced_cut_in: None,
                kerb: crate::kerbside::KerbState::NONE,
                stuck_since: None,
            },
        );
        Ok(())
    }
}
