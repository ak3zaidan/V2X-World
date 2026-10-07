//! Network-level measures, set beside what is published for real cities: the macroscopic
//! fundamental diagram, stops per kilometre and travel-time rate, kerbside dwell, how
//! evenly traffic uses the lanes of an approach, cyclist conflicts, and pedestrian speed
//! by crowding.
//!
//! [`crate::calibration::CalibrationObserver`] measures the kerb-side figures of a single
//! stop line or a single vehicle. These are the figures of the whole network, the ones the
//! owner's "congestion that looks real" is about. The calibration harness feeds this
//! observer on every step and appends its comparisons to its own table.
//!
//! | Figure | Procedure | Reference |
//! |---|---|---|
//! | macroscopic fundamental diagram | Edie's generalised definitions over every motorised road lane (junction connectors excluded) and every interval of [`NetworkObserver::interval_s`]: density `k = Σ tᵢ / (L·T)`, flow `q = Σ dᵢ / (L·T)`, speed `v = q / k` | Daganzo & Geroliminis 2008 (*Transp. Res. B* 42:771), the method of cuts: on a signalised network the lane flow cannot exceed the saturation flow times the green ratio, `q ≤ s·g/C`, here the HCM CBD saturation flow (1,710 veh/h/ln) times the mean through green ratio of the world's own plans. Edie 1963 for the definitions. Geroliminis & Daganzo 2008 (*Transp. Res. B* 42:759) measured that a real network (Yokohama) has such a curve with little scatter; their values are not transcribed here |
//! | stops per kilometre | a stop is a fall from above 2 m/s to standing; kerbside stops on purpose are not counted | reported: no measured Midtown figure was found to cite |
//! | travel-time rate | minutes per kilometre, from the network speed | NYC DOT *Mobility Report* 2019: Midtown core ≈ 5 mph, i.e. about 7.5 min/km (secondary, as [`crate::calibration`] cites it) |
//! | kerbside dwell | each continuous spell a vehicle stands at the kerb, by role | bus: TCQSM 2nd ed. downtown default 60 s (see [`crate::kerbside`]); HCM's bus-blockage factor assumes each bus blocks its lane for 14.4 s, which a downtown dwell far exceeds |
//! | lane utilisation | HCM `f_LU = v / (N·v_max)` on every approach with two or three general lanes, from the vehicles crossing each lane's stop line | HCM 2000 Exhibit 10-23 defaults: 0.952 for two lanes, 0.908 for three (**secondary**: as the HCM default table is widely reproduced) |
//! | right-hook conflicts | a motor vehicle entering a right-turn connector with a cyclist going straight on within 12 m of its front, on the same approach | reported (NACTO and NYC DOT treat the right hook as the leading turning conflict with cyclists; no rate is cited) |
//! | dooring exposures | a cyclist passing within a door's reach (1 m) of a taxi standing at the kerb | reported |
//! | blocking the box | vehicle-seconds standing inside a signalised junction on a movement showing red | NY VTL §1175 and NYC's "Don't Block the Box": no driver enters a junction without room to leave it; the expected figure is zero |
//! | queue spillback | lane-seconds during which a standing queue on a signalised approach reaches back to the lane's start — the upstream junction | reported: how often a queue blocks the junction behind it |
//! | pedestrian speed by crowding | walkway cells 10 m long (clear of the kerb ends) over 5 s intervals: density `k = Σ tᵢ / (A·T)` and space-mean speed `Σ dᵢ / Σ tᵢ` of the walkers in them (Edie), binned by Fruin's walkway levels of service — space per person ≥ 3.25 m² (A), 2.32 (B), 1.39 (C), 0.93 (D), 0.46 (E), less (F), i.e. 35/25/15/10/5 ft² (Fruin 1971, *Pedestrian Planning and Design*) | Weidmann 1993's speed-density relation `v/v₀ = 1 − exp(−1.913·(1/ρ − 1/5.4))` (**secondary**: the parameters as the pedestrian literature quotes them) |
//!
//! # Determinism
//!
//! Everything is a `BTreeMap` or a `Vec` filled in actor-id order.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use v2xw_core::ids::{ActorId, EdgeId, LaneId};
use v2xw_core::math;
use v2xw_core::time::{SimTime, ns_to_secs};
use v2xw_world::{LaneKind, SignalState, TurnDirection, World};

use crate::audit::{AuditActor, AuditPedestrian};
use crate::calibration::{Comparison, HCM_BASE_SATURATION_FLOW, HCM_CBD_AREA_FACTOR, Summary};
use crate::kerbside::KerbRole;
use crate::views::DespawnCause;

/// Weidmann's free walking speed, m/s.
pub const WEIDMANN_V0_MPS: f64 = 1.34;
/// Weidmann's shape parameter γ, ped/m².
pub const WEIDMANN_GAMMA: f64 = 1.913;
/// Weidmann's jam density, ped/m².
pub const WEIDMANN_JAM_DENSITY: f64 = 5.4;

/// Weidmann's speed at density `rho` (ped/m²) as a fraction of the free speed.
pub fn weidmann_factor(rho: f64) -> f64 {
    if rho <= 1e-6 {
        return 1.0;
    }
    (1.0 - math::exp(-WEIDMANN_GAMMA * (1.0 / rho - 1.0 / WEIDMANN_JAM_DENSITY))).clamp(0.0, 1.0)
}

/// Fruin's walkway level-of-service lower bounds on space per person, m² (A..E; F below
/// the last): 35, 25, 15, 10 and 5 ft².
pub const FRUIN_SPACE_M2: [f64; 5] = [3.25, 2.32, 1.39, 0.93, 0.46];

/// The Fruin level of service (0 = A … 5 = F) of a space per person, m².
pub fn fruin_level(space_m2: f64) -> usize {
    FRUIN_SPACE_M2
        .iter()
        .position(|lo| space_m2 >= *lo)
        .unwrap_or(FRUIN_SPACE_M2.len())
}

/// The radius a pedestrian's local density is counted over, metres.
pub const CROWD_RADIUS_M: f64 = 2.0;

/// A pedestrian's local density, ped/m²: itself and the `others` within
/// [`CROWD_RADIUS_M`], over the part of that circle a walkway `width_m` wide covers.
pub fn local_density(others: usize, width_m: f64) -> f64 {
    let r = CROWD_RADIUS_M;
    let circle = core::f64::consts::PI * r * r;
    let area = circle.min(2.0 * r * width_m.max(0.5));
    (others as f64 + 1.0) / area
}

/// A stop counts once the speed has been above this, m/s.
const MOVING_MPS: f64 = 2.0;
/// And has since fallen below this, m/s.
const STOPPED_MPS: f64 = 0.1;
/// A cyclist this near a right turner's front is in a right-hook conflict, metres.
const RIGHT_HOOK_M: f64 = 12.0;
/// How far a car door reaches into the lane beside, metres.
const DOOR_ZONE_M: f64 = 1.0;
/// The fewest stop-line crossings an approach needs before its lane utilisation counts.
const LANE_UTIL_MIN: usize = 20;
/// How far from either end of a pavement a walkway cell must be for the speed-density
/// measure, metres: beyond the two metres a walker slows over into a kerb, and the first
/// steps off one.
const STEADY_MARGIN_M: f64 = 4.0;
/// The length of a walkway cell the pedestrian speed-density relation is measured over,
/// metres.
const PED_CELL_M: f64 = 10.0;
/// The interval it is measured over, seconds.
const PED_INTERVAL_S: f64 = 5.0;

/// One interval's point on the macroscopic fundamental diagram.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MfdSample {
    /// The interval's end, seconds.
    pub t_s: f64,
    /// Density, vehicles per lane-kilometre.
    pub density_veh_per_km: f64,
    /// Flow, vehicles per hour per lane.
    pub flow_veh_per_h: f64,
    /// Space-mean speed, m/s.
    pub speed_mps: f64,
}

/// The observer. Feed it every step, then read [`NetworkObserver::report`].
#[derive(Debug, Clone)]
pub struct NetworkObserver {
    /// The MFD aggregation interval, seconds.
    pub interval_s: f64,
    lane_km: f64,
    measured: BTreeSet<LaneId>,
    movement_of: BTreeMap<LaneId, (LaneId, TurnDirection)>,
    green_ratio: f64,
    interval_start_s: Option<f64>,
    acc_dist_m: f64,
    acc_time_s: f64,
    samples: Vec<MfdSample>,
    moving: BTreeMap<ActorId, bool>,
    prev_lane: BTreeMap<ActorId, LaneId>,
    stops: usize,
    vehicle_m: f64,
    vehicle_s: f64,
    kerb_since: BTreeMap<ActorId, (KerbRole, f64)>,
    dwell: BTreeMap<KerbRole, Vec<f64>>,
    lane_counts: BTreeMap<EdgeId, BTreeMap<LaneId, usize>>,
    /// How many general (driving) lanes each road has.
    general_lanes: BTreeMap<EdgeId, usize>,
    right_turns: usize,
    right_hooks: usize,
    door_pairs: BTreeSet<(ActorId, ActorId)>,
    /// Per Fruin level: Σ person-seconds × speed, Σ person-seconds, cell-intervals.
    ped_by_level: [(f64, f64, usize); 6],
    ped_density: Vec<f64>,
    /// The current pedestrian interval's walkway cells: person-seconds and metres walked.
    ped_cells: BTreeMap<(LaneId, u32), (f64, f64)>,
    ped_interval_start_s: Option<f64>,
    /// The signalised approach lanes (those feeding a signal-controlled connector).
    signal_approaches: BTreeSet<LaneId>,
    box_blocking_s: f64,
    spillback_lane_s: f64,
    observed_s: f64,
}

impl NetworkObserver {
    /// An observer for runs on `world`, aggregating the MFD over 60 s.
    pub fn new(world: &World) -> Self {
        let mut movement_of = BTreeMap::new();
        for c in world.roads.connections() {
            if let Some(via) = c.via {
                movement_of.insert(via, (c.from_lane, c.direction));
            }
        }
        let measured: BTreeSet<LaneId> = world
            .roads
            .lanes()
            .iter()
            .filter(|l| matches!(l.kind, LaneKind::Driving | LaneKind::Bus))
            .map(|l| l.id)
            .collect();
        let mut general_lanes: BTreeMap<EdgeId, usize> = BTreeMap::new();
        for l in world.roads.lanes() {
            if l.kind == LaneKind::Driving {
                *general_lanes.entry(l.edge).or_insert(0) += 1;
            }
        }
        let lane_km = measured
            .iter()
            .map(|l| world.lane(*l).length_m)
            .sum::<f64>()
            / 1000.0;
        // The mean green ratio of every signalised through movement.
        let mut ratios = Vec::new();
        for plan in &world.signals {
            if !(plan.cycle_s > 0.0) {
                continue;
            }
            for (k, l) in plan.controlled.iter().enumerate() {
                let through = movement_of
                    .get(l)
                    .is_some_and(|(_, t)| *t == TurnDirection::Straight);
                if !through || !world.try_lane(*l).is_some_and(|x| x.kind.is_motorised()) {
                    continue;
                }
                let green: f64 = plan
                    .phases
                    .iter()
                    .filter(|p| {
                        matches!(
                            p.states.get(k),
                            Some(SignalState::Green | SignalState::GreenYield)
                        )
                    })
                    .map(|p| p.duration_s)
                    .sum();
                ratios.push((green / plan.cycle_s).clamp(0.0, 1.0));
            }
        }
        let mut signal_approaches = BTreeSet::new();
        for plan in &world.signals {
            for l in &plan.controlled {
                if let Some((approach, _)) = movement_of.get(l)
                    && world.try_lane(*approach).is_some_and(|x| x.kind.is_motorised())
                {
                    signal_approaches.insert(*approach);
                }
            }
        }
        let green_ratio = if ratios.is_empty() {
            f64::NAN
        } else {
            ratios.iter().sum::<f64>() / ratios.len() as f64
        };
        Self {
            interval_s: 60.0,
            lane_km,
            measured,
            movement_of,
            green_ratio,
            interval_start_s: None,
            acc_dist_m: 0.0,
            acc_time_s: 0.0,
            samples: Vec::new(),
            moving: BTreeMap::new(),
            prev_lane: BTreeMap::new(),
            stops: 0,
            vehicle_m: 0.0,
            vehicle_s: 0.0,
            kerb_since: BTreeMap::new(),
            dwell: BTreeMap::new(),
            lane_counts: BTreeMap::new(),
            general_lanes,
            right_turns: 0,
            right_hooks: 0,
            door_pairs: BTreeSet::new(),
            ped_by_level: Default::default(),
            ped_density: Vec::new(),
            ped_cells: BTreeMap::new(),
            ped_interval_start_s: None,
            signal_approaches,
            box_blocking_s: 0.0,
            spillback_lane_s: 0.0,
            observed_s: 0.0,
        }
    }

    /// One step from `t0` to `t1`.
    pub fn observe(
        &mut self,
        world: &World,
        t0: SimTime,
        t1: SimTime,
        actors: &[AuditActor],
        despawned: &[(ActorId, DespawnCause)],
        pedestrians: &[AuditPedestrian],
    ) {
        let dt = ns_to_secs(t1.saturating_sub(t0)).max(1e-9);
        let t1_s = ns_to_secs(t1);
        let start = *self.interval_start_s.get_or_insert(ns_to_secs(t0));
        self.observed_s += dt;
        let motor = |a: &&AuditActor| a.class != crate::VehicleClass::Bicycle;
        // Blocking the box, and queues reaching back to the junction behind.
        let mut spilled: BTreeSet<LaneId> = BTreeSet::new();
        for a in actors.iter().filter(motor) {
            if a.speed_mps >= STOPPED_MPS {
                continue;
            }
            if crate::audit::movement_state(world, a.lane, t1)
                .is_some_and(|st| matches!(st, SignalState::Red | SignalState::RedAmber))
            {
                self.box_blocking_s += dt;
            }
            if self.signal_approaches.contains(&a.lane) && a.s_m - a.length_m < 2.0 {
                spilled.insert(a.lane);
            }
        }
        self.spillback_lane_s += dt * spilled.len() as f64;
        for a in actors.iter().filter(motor) {
            if self.measured.contains(&a.lane) {
                self.acc_dist_m += a.speed_mps * dt;
                self.acc_time_s += dt;
            }
            self.vehicle_m += a.speed_mps * dt;
            self.vehicle_s += dt;
            // Stops.
            let was = self.moving.get(&a.actor).copied().unwrap_or(false);
            let now = if a.speed_mps > MOVING_MPS {
                true
            } else if a.speed_mps < STOPPED_MPS {
                if was && a.kerbside.is_none() {
                    self.stops += 1;
                }
                false
            } else {
                was
            };
            self.moving.insert(a.actor, now);
            // Kerbside dwell.
            match (a.kerbside, self.kerb_since.get(&a.actor).copied()) {
                (Some(role), None) => {
                    self.kerb_since.insert(a.actor, (role, t1_s));
                }
                (None, Some((role, since))) => {
                    self.dwell.entry(role).or_default().push(t1_s - since);
                    self.kerb_since.remove(&a.actor);
                }
                _ => {}
            }
        }
        for (id, _) in despawned {
            if let Some((role, since)) = self.kerb_since.remove(id) {
                self.dwell.entry(role).or_default().push(t1_s - since);
            }
            self.moving.remove(id);
        }
        // Stop-line crossings by lane, and right turns past cyclists.
        let cyclists: Vec<&AuditActor> = actors
            .iter()
            .filter(|a| a.class == crate::VehicleClass::Bicycle && a.speed_mps > 1.0)
            .collect();
        let mut lanes_now = BTreeMap::new();
        for a in actors {
            lanes_now.insert(a.actor, a.lane);
            let Some(prev) = self.prev_lane.get(&a.actor).copied() else {
                continue;
            };
            if prev == a.lane || a.class == crate::VehicleClass::Bicycle {
                continue;
            }
            let Some((approach, turn)) = self.movement_of.get(&a.lane).copied() else {
                continue;
            };
            if approach != prev {
                continue;
            }
            let edge = world.lane(prev).edge;
            if world.lane(prev).kind == LaneKind::Driving {
                *self
                    .lane_counts
                    .entry(edge)
                    .or_default()
                    .entry(prev)
                    .or_insert(0) += 1;
            }
            if turn == TurnDirection::Right {
                self.right_turns += 1;
                let front = front_of(a);
                let hooked = cyclists.iter().any(|c| {
                    let on_approach = world.lane(c.lane).edge == edge
                        && c.route_next
                            .and_then(|n| self.movement_of.get(&n))
                            .is_none_or(|(_, t)| *t != TurnDirection::Right);
                    let straight_through = self
                        .movement_of
                        .get(&c.lane)
                        .is_some_and(|(ap, t)| {
                            world.lane(*ap).edge == edge && *t == TurnDirection::Straight
                        });
                    (on_approach || straight_through)
                        && front_of(c).distance_2d(front) < RIGHT_HOOK_M
                });
                if hooked {
                    self.right_hooks += 1;
                }
            }
        }
        self.prev_lane = lanes_now;
        // Dooring exposures: cyclists passing a taxi at the kerb within a door's reach.
        for taxi in actors
            .iter()
            .filter(|a| a.kerbside == Some(KerbRole::Taxi))
        {
            let (tc, th) = (centre_of(taxi), taxi.heading_rad);
            let (sin_h, cos_h) = math::sin_cos(th);
            for c in &cyclists {
                let d = centre_of(c);
                let (dx, dy) = (d.x - tc.x, d.y - tc.y);
                let along = dx * cos_h + dy * sin_h;
                let across = -dx * sin_h + dy * cos_h;
                if along.abs() <= 0.5 * taxi.length_m
                    && across.abs() <= 0.5 * taxi.width_m + DOOR_ZONE_M + 0.5 * c.width_m
                {
                    self.door_pairs.insert((taxi.actor, c.actor));
                }
            }
        }
        // Pedestrians: each walker's local density (a summary), and the walkway cells'
        // density and speed by Edie's definitions over short intervals — the
        // speed-density relation, binned by Fruin's levels of service.
        if !pedestrians.is_empty() {
            let cell = CROWD_RADIUS_M;
            let key = |x: f64, y: f64| ((x / cell).floor() as i64, (y / cell).floor() as i64);
            let mut grid: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
            for (i, p) in pedestrians.iter().enumerate() {
                grid.entry(key(p.pos.x, p.pos.y)).or_default().push(i);
            }
            for (i, p) in pedestrians.iter().enumerate() {
                let v = p.vel.norm_2d();
                // Walking along a pavement, clear of its ends, where a walker slows into
                // a kerb or picks up speed off one.
                let Some(lane) = world.try_lane(p.lane) else {
                    continue;
                };
                if p.activity != crate::vru::PedActivity::Walking
                    || lane.kind != LaneKind::Sidewalk
                {
                    continue;
                }
                let k = (p.s_m / PED_CELL_M).floor();
                let (lo, hi) = (k * PED_CELL_M, (k + 1.0) * PED_CELL_M);
                if lo >= STEADY_MARGIN_M && hi <= lane.length_m - STEADY_MARGIN_M && k >= 0.0 {
                    let e = self
                        .ped_cells
                        .entry((p.lane, k as u32))
                        .or_insert((0.0, 0.0));
                    e.0 += dt;
                    e.1 += v.min(4.0) * dt;
                }
                if !(0.3..4.0).contains(&v) {
                    continue;
                }
                let (cx, cy) = key(p.pos.x, p.pos.y);
                let mut others = 0usize;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        if let Some(list) = grid.get(&(cx + dx, cy + dy)) {
                            others += list
                                .iter()
                                .filter(|j| {
                                    **j != i
                                        && pedestrians[**j].pos.distance_2d(p.pos) <= CROWD_RADIUS_M
                                })
                                .count();
                        }
                    }
                }
                self.ped_density.push(local_density(others, lane.width_m));
            }
        }
        let ped_start = *self.ped_interval_start_s.get_or_insert(ns_to_secs(t0));
        if t1_s - ped_start >= PED_INTERVAL_S - 1e-9 {
            let span = t1_s - ped_start;
            for ((lane, _), (person_s, metres)) in core::mem::take(&mut self.ped_cells) {
                let width = world.try_lane(lane).map_or(2.0, |l| l.width_m);
                let k = person_s / (PED_CELL_M * width * span);
                if person_s > 0.0 && k > 0.0 {
                    let level = fruin_level(1.0 / k);
                    let v = metres / person_s;
                    let acc = &mut self.ped_by_level[level];
                    acc.0 += person_s * v;
                    acc.1 += person_s;
                    acc.2 += 1;
                }
            }
            self.ped_interval_start_s = Some(t1_s);
        }
        // Close the MFD interval.
        if t1_s - start >= self.interval_s - 1e-9 {
            let span = t1_s - start;
            if self.lane_km > 0.0 && span > 0.0 {
                let k = self.acc_time_s / (self.lane_km * span);
                let q = (self.acc_dist_m / 1000.0) / (self.lane_km * span / 3600.0);
                self.samples.push(MfdSample {
                    t_s: t1_s,
                    density_veh_per_km: k,
                    flow_veh_per_h: q,
                    speed_mps: if k > 0.0 { q / k / 3.6 } else { f64::NAN },
                });
            }
            self.interval_start_s = Some(t1_s);
            self.acc_dist_m = 0.0;
            self.acc_time_s = 0.0;
        }
    }

    /// The figures so far.
    pub fn report(&self) -> NetworkReport {
        let speed = if self.vehicle_s > 0.0 {
            self.vehicle_m / self.vehicle_s
        } else {
            f64::NAN
        };
        let util = |n: usize| -> (f64, usize) {
            let mut weighted = 0.0;
            let mut total = 0usize;
            for (edge, counts) in &self.lane_counts {
                if self.general_lanes.get(edge).copied() != Some(n) {
                    continue;
                }
                let sum: usize = counts.values().sum();
                let max = counts.values().copied().max().unwrap_or(0);
                if sum < LANE_UTIL_MIN || max == 0 {
                    continue;
                }
                weighted += sum as f64 / (n as f64 * max as f64) * sum as f64;
                total += sum;
            }
            if total == 0 {
                (f64::NAN, 0)
            } else {
                (weighted / total as f64, total)
            }
        };
        let (lu2, lu2_n) = util(2);
        let (lu3, lu3_n) = util(3);
        let mut dwell = BTreeMap::new();
        for (role, v) in &self.dwell {
            dwell.insert(role.label().to_string(), Summary::of(v));
        }
        let level_speed: Vec<Summary> = self
            .ped_by_level
            .iter()
            .map(|(sum, weight, n)| Summary {
                n: *n,
                mean: if *weight > 0.0 { sum / weight } else { f64::NAN },
                sd: f64::NAN,
                p15: f64::NAN,
                p50: f64::NAN,
                p85: f64::NAN,
                max: f64::NAN,
            })
            .collect();
        let max_flow = self
            .samples
            .iter()
            .map(|s| s.flow_veh_per_h)
            .fold(f64::NAN, f64::max);
        let vkm = self.vehicle_m / 1000.0;
        NetworkReport {
            lane_km: self.lane_km,
            mfd: self.samples.clone(),
            mfd_max_flow_veh_per_h: max_flow,
            mean_through_green_ratio: self.green_ratio,
            capacity_bound_veh_per_h: HCM_BASE_SATURATION_FLOW
                * HCM_CBD_AREA_FACTOR
                * self.green_ratio,
            network_speed_mps: speed,
            travel_time_min_per_km: if speed > 0.0 {
                1000.0 / speed / 60.0
            } else {
                f64::NAN
            },
            stops: self.stops,
            vehicle_km: vkm,
            stops_per_km: if vkm > 0.0 {
                self.stops as f64 / vkm
            } else {
                f64::NAN
            },
            kerbside_dwell_s: dwell,
            kerbside_now: {
                let mut now: BTreeMap<String, usize> = BTreeMap::new();
                for (role, _) in self.kerb_since.values() {
                    *now.entry(role.label().to_string()).or_insert(0) += 1;
                }
                now
            },
            lane_utilisation_2: lu2,
            lane_utilisation_2_vehicles: lu2_n,
            lane_utilisation_3: lu3,
            lane_utilisation_3_vehicles: lu3_n,
            right_turns: self.right_turns,
            right_hook_conflicts: self.right_hooks,
            dooring_exposures: self.door_pairs.len(),
            pedestrian_speed_by_fruin_level: level_speed,
            pedestrian_density: Summary::of(&self.ped_density),
            box_blocking_s: self.box_blocking_s,
            spillback_lane_s_per_h: if self.observed_s > 0.0 {
                self.spillback_lane_s * 3600.0 / self.observed_s
            } else {
                f64::NAN
            },
        }
    }
}

/// A vehicle's front-bumper point.
fn front_of(a: &AuditActor) -> v2xw_core::geom::Vec3 {
    let (sin_h, cos_h) = math::sin_cos(a.heading_rad);
    v2xw_core::geom::Vec3::new_2d(a.pos.x + a.length_m * cos_h, a.pos.y + a.length_m * sin_h)
}

/// A vehicle's body centre.
fn centre_of(a: &AuditActor) -> v2xw_core::geom::Vec3 {
    let (sin_h, cos_h) = math::sin_cos(a.heading_rad);
    v2xw_core::geom::Vec3::new_2d(
        a.pos.x + 0.5 * a.length_m * cos_h,
        a.pos.y + 0.5 * a.length_m * sin_h,
    )
}

/// What the network observer measured.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NetworkReport {
    /// Motorised road lane length measured, lane-km.
    pub lane_km: f64,
    /// The MFD, one point per interval.
    pub mfd: Vec<MfdSample>,
    /// The highest interval flow, veh/h/ln.
    pub mfd_max_flow_veh_per_h: f64,
    /// The mean green ratio of the world's signalised through movements.
    pub mean_through_green_ratio: f64,
    /// The method-of-cuts bound `s·g/C`, veh/h/ln.
    pub capacity_bound_veh_per_h: f64,
    /// Total distance over total vehicle-time, m/s.
    pub network_speed_mps: f64,
    /// Its inverse, minutes per kilometre.
    pub travel_time_min_per_km: f64,
    /// Stops counted.
    pub stops: usize,
    /// Vehicle-kilometres driven.
    pub vehicle_km: f64,
    /// Stops per vehicle-kilometre.
    pub stops_per_km: f64,
    /// Each completed kerbside spell's length, by role, seconds.
    pub kerbside_dwell_s: BTreeMap<String, Summary>,
    /// Vehicles at the kerb when the report was taken, by role (spells not yet over, so
    /// not in `kerbside_dwell_s`).
    pub kerbside_now: BTreeMap<String, usize>,
    /// HCM lane utilisation factor on two-lane approaches.
    pub lane_utilisation_2: f64,
    /// Stop-line crossings behind it.
    pub lane_utilisation_2_vehicles: usize,
    /// The same on three-lane approaches.
    pub lane_utilisation_3: f64,
    /// Stop-line crossings behind it.
    pub lane_utilisation_3_vehicles: usize,
    /// Right turns made.
    pub right_turns: usize,
    /// Right turns made with a cyclist going straight on within 12 m.
    pub right_hook_conflicts: usize,
    /// Distinct (taxi, cyclist) pairs where the cyclist passed within a door's reach of a
    /// taxi at the kerb.
    pub dooring_exposures: usize,
    /// Space-mean walking speed by Fruin level of service, A..F, over walkway cells 10 m
    /// long and 5 s intervals (Edie): `n` is the cell-intervals, `mean` the speed weighted
    /// by person-time.
    pub pedestrian_speed_by_fruin_level: Vec<Summary>,
    /// Pedestrians' local density, ped/m².
    pub pedestrian_density: Summary,
    /// Vehicle-seconds stood inside a signalised junction on a red movement.
    pub box_blocking_s: f64,
    /// Lane-seconds per hour a standing queue reached back to a signalised approach's
    /// start.
    pub spillback_lane_s_per_h: f64,
}

impl NetworkReport {
    /// The network figures beside their references.
    pub fn comparisons(&self) -> Vec<Comparison> {
        let a = self.pedestrian_speed_by_fruin_level[0];
        let ratio = |level: usize| -> (f64, usize) {
            let s = self.pedestrian_speed_by_fruin_level[level];
            if a.n == 0 || s.n == 0 {
                (f64::NAN, 0)
            } else {
                (s.mean / a.mean, s.n.min(a.n))
            }
        };
        // Weidmann at the middle of the Fruin band (space per person, m²).
        let mid = |level: usize| -> f64 {
            let hi = if level == 0 { 5.0 } else { FRUIN_SPACE_M2[level - 1] };
            let lo = FRUIN_SPACE_M2.get(level).copied().unwrap_or(0.3);
            weidmann_factor(2.0 / (hi + lo)) / weidmann_factor(1.0 / 5.0)
        };
        let (c_ratio, c_n) = ratio(2);
        let (d_ratio, d_n) = ratio(3);
        let bus = self.kerbside_dwell_s.get("bus").copied().unwrap_or_default();
        vec![
            Comparison {
                metric: "network max flow / s·g/C cut bound",
                measured: self.mfd_max_flow_veh_per_h / self.capacity_bound_veh_per_h,
                samples: self.mfd.len(),
                reference: 1.0,
                band: (0.0, 1.0),
                source: "Daganzo & Geroliminis 2008 (Transp. Res. B 42:771), method of cuts: \
                         lane flow on a signalised network cannot exceed saturation flow × \
                         green ratio; HCM CBD saturation flow 1,710 veh/h/ln × the world's \
                         mean through green ratio",
                tested: false,
            },
            Comparison {
                metric: "travel-time rate, min/km",
                measured: self.travel_time_min_per_km,
                samples: self.vehicle_km.round() as usize,
                reference: 1000.0 / 2.2 / 60.0,
                band: (1000.0 / 4.0 / 60.0, 1000.0 / 1.8 / 60.0),
                source: "NYC DOT Mobility Report 2019: Midtown core ≈ 5 mph (2017 taxi GPS, \
                         all-day; secondary); depends on demand, reported only",
                tested: false,
            },
            Comparison {
                metric: "blocking the box, vehicle-seconds",
                measured: self.box_blocking_s,
                samples: self.mfd.len(),
                reference: 0.0,
                band: (0.0, 0.0),
                source: "NY VTL §1175 (\"Don't Block the Box\"): no driver enters an \
                         intersection without room to leave it",
                tested: false,
            },
            Comparison {
                metric: "stops per vehicle-km",
                measured: self.stops_per_km,
                samples: self.stops,
                reference: f64::NAN,
                band: (0.0, f64::INFINITY),
                source: "reported: no measured Midtown stops-per-km figure was found to cite",
                tested: false,
            },
            Comparison {
                metric: "bus dwell at kerb, mean, s",
                measured: bus.mean,
                samples: bus.n,
                reference: 60.0,
                band: (30.0, 90.0),
                source: "TCRP Report 100 (TCQSM 2nd ed.) Part 4: 60 s default at a downtown \
                         stop when no field data (secondary); band a choice",
                tested: false,
            },
            Comparison {
                metric: "lane utilisation f_LU, 2-lane approaches",
                measured: self.lane_utilisation_2,
                samples: self.lane_utilisation_2_vehicles,
                reference: 0.952,
                band: (0.85, 1.0),
                source: "HCM 2000 Exhibit 10-23 default lane utilisation factor, two lanes \
                         (secondary); band a choice",
                tested: false,
            },
            Comparison {
                metric: "lane utilisation f_LU, 3-lane approaches",
                measured: self.lane_utilisation_3,
                samples: self.lane_utilisation_3_vehicles,
                reference: 0.908,
                band: (0.8, 1.0),
                source: "HCM 2000 Exhibit 10-23 default lane utilisation factor, three lanes \
                         (secondary); band a choice",
                tested: false,
            },
            Comparison {
                metric: "pedestrian speed, Fruin LOS C / LOS A",
                measured: c_ratio,
                samples: c_n,
                reference: mid(2),
                band: (mid(2) - 0.1, mid(2) + 0.1),
                source: "Weidmann 1993 speed-density relation at the middle of Fruin's LOS C \
                         (15-25 ft²/person), relative to free walking (secondary)",
                tested: false,
            },
            Comparison {
                metric: "pedestrian speed, Fruin LOS D / LOS A",
                measured: d_ratio,
                samples: d_n,
                reference: mid(3),
                band: (mid(3) - 0.1, mid(3) + 0.1),
                source: "Weidmann 1993 speed-density relation at the middle of Fruin's LOS D \
                         (10-15 ft²/person), relative to free walking (secondary)",
                tested: false,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weidmann_slows_a_crowd_and_fruin_bins_space() {
        assert!((weidmann_factor(0.01) - 1.0).abs() < 1e-6);
        // About 0.79 of the free speed at one person per square metre.
        let one = weidmann_factor(1.0);
        assert!((0.75..0.83).contains(&one), "{one}");
        assert!(weidmann_factor(2.0) < one);
        assert_eq!(weidmann_factor(WEIDMANN_JAM_DENSITY), 0.0);
        assert_eq!(fruin_level(10.0), 0);
        assert_eq!(fruin_level(3.0), 1);
        assert_eq!(fruin_level(2.0), 2);
        assert_eq!(fruin_level(1.0), 3);
        assert_eq!(fruin_level(0.5), 4);
        assert_eq!(fruin_level(0.2), 5);
        // A walker alone on a 2.5 m pavement has 10 m² to themself.
        assert!((local_density(0, 2.5) - 0.1).abs() < 1e-12);
    }
}
