//! The calibration harness: what a traffic engineer would measure at the kerb, measured on
//! a simulated run and set against the published figure it should reproduce.
//!
//! # Why it exists
//!
//! [`crate::audit`] proves the traffic breaks no rule. It cannot say whether the traffic is
//! *real*: a queue that discharges at one car every four seconds breaks no rule and is
//! nothing like a New York stop line. This module measures what the literature measures,
//! with the literature's own procedure where it has one, and [`CalibrationReport::
//! comparisons`] lays each figure beside its reference with a stated tolerance, so a test
//! can hold the model to it and a reader can see where it stands.
//!
//! # What is measured, and how
//!
//! | Figure | Procedure | Reference |
//! |---|---|---|
//! | saturation headway `h_s` | HCM field method: at the onset of green, every vehicle standing in the queue on an approach lane is recorded; each one's **rear** crossing of the stop line is timed; `h_s` is the mean headway from the 4th queued vehicle on (positions ≥ 5), through movements only, excluding a vehicle that stopped again before the line | HCM base 1,900 pc/h/ln × CBD area factor 0.90 = 1,710 veh/h/ln, `h_s` = 2.11 s; practical range 1,500–2,000 (FHWA Signal Timing Manual 2008 §3.3.1) |
//! | start-up lost time `l1` | `Σ_{i=1..4} (h_i − h_s)`, `h_1` measured from the green onset | HCM default 2.0 s (FHWA STM §3.3.1: "commonly assumed to be approximately 2 seconds") |
//! | discharge headway by queue position | mean `h_i` for i = 1..10 | "after approximately the fourth vehicle in the queue, the flow rate tends to stabilize" (FHWA STM §3.3.1, Fig. 3-2) |
//! | queue spacing | front-to-front distance between consecutive vehicles standing in a queue at green onset | HCM average queue storage 25 ft (7.6 m) per vehicle |
//! | free-flow speed / posted limit | mid-block, cruising (`|a| < 0.3 m/s²`), no vehicle within `max(60 m, 5 s)` ahead on the lane, ≥ 40 m from the stop line | SUMO passenger `speedFactor` N(1.0, 0.1) (SUMO vType defaults) — a model default, not a New York measurement |
//! | launch acceleration | mean acceleration from standstill to 8 m/s, launches that were never held back (acceleration stayed positive) | Wang, Dixon, Li & Ogle 2004 (TRR 1883): mean 0.127 g (1.25 m/s²) over the first 15 s from rest, straight movements |
//! | stopping deceleration | peak deceleration of each stop from above 5 m/s to standstill | AASHTO *Green Book* 2018 §3.2.2: 3.4 m/s² is comfortable for most drivers |
//! | turning speed | lowest speed through each junction connector, by turn, traversals that never stopped | reported, not tested: no single published figure fits every corner radius |
//! | travel speed | total distance over total vehicle-time, all vehicles | NYC DOT *Mobility Report* 2019: Midtown core ≈ 5 mph (2.2 m/s), CBD ≈ 7 mph (3.1 m/s), 2017 taxi GPS — a whole-day average over real demand, so a comparison, not a test |
//! | pedestrian walking speed | speed of every walking pedestrian step (≥ 0.3 m/s) | Knoblauch, Pietrucha & Nitzburg 1996 (TRR 1538): younger pedestrians mean 1.51 m/s, 15th percentile 1.25 m/s; MUTCD 2009 §4E.06 design speed 3.5 ft/s (1.07 m/s) |
//!
//! Every reference that could not be re-read from its primary document is labelled
//! **secondary** on the comparison, with where it was read.
//!
//! # Determinism
//!
//! The harness reads the engine's state and the world and writes nothing back; every
//! collection is a `BTreeMap` or a `Vec` filled in actor-id order, so a report is a pure
//! function of the run.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use v2xw_core::ids::{ActorId, LaneId};
use v2xw_core::time::{SimTime, ns_to_secs};
use v2xw_world::{LaneKind, SignalState, TurnDirection, World};

use crate::audit::{AuditActor, AuditPedestrian};
use crate::views::DespawnCause;

/// Standing, for the queue and launch procedures, m/s.
const STANDING_MPS: f64 = 0.1;
/// A queued vehicle, at green onset, is moving slower than this, m/s (a creeping queue
/// still counts).
const QUEUED_MPS: f64 = 1.0;
/// The first queued vehicle's front is within this of the stop line, metres: the 2 m
/// stop-line offset, the standstill gap and a car length of creep.
const QUEUE_HEAD_M: f64 = 10.0;
/// Two queued vehicles are in one queue when the gap between them is under this, metres.
const QUEUE_LINK_M: f64 = 8.0;
/// A launch is timed from standstill to this speed, m/s.
const LAUNCH_TO_MPS: f64 = 8.0;
/// A stop is counted from above this speed, m/s.
const STOP_FROM_MPS: f64 = 5.0;
/// How many queue positions the per-position headway table reports.
const POSITIONS: usize = 10;

/// One vehicle of a queue recorded at the onset of green.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Member {
    actor: ActorId,
    /// When its front crossed the stop line.
    front_cross: Option<f64>,
    /// Distance driven since its front crossed, metres.
    past_line_m: f64,
    /// When its rear crossed the stop line.
    rear_cross: Option<f64>,
    /// The turn it took.
    turn: Option<TurnDirection>,
    /// Highest speed reached since green.
    peak_mps: f64,
    /// True if it stood again after having got going, before its front crossed.
    restopped: bool,
    length_m: f64,
}

/// A queue recorded at a green onset, waiting to discharge.
#[derive(Debug, Clone, PartialEq)]
struct OpenQueue {
    /// The approach lane whose stop line the queue discharges over.
    lane: LaneId,
    /// The lanes behind it the queue reached back over.
    upstream: Vec<LaneId>,
    /// The movements of `lane` that turned green together.
    movements: Vec<LaneId>,
    /// The green onset, seconds.
    onset_s: f64,
    members: Vec<Member>,
}

/// Per-actor state carried between steps.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Prev {
    lane: LaneId,
    speed_mps: f64,
    pos: v2xw_core::geom::Vec3,
    /// Launch in progress: when it left standstill, and whether it was ever held back.
    launch: Option<(f64, bool)>,
    /// Stop in progress: the peak deceleration since the speed was last above
    /// [`STOP_FROM_MPS`].
    stop_peak: Option<f64>,
    /// The connector being traversed and the lowest speed on it so far, and whether it
    /// stood.
    connector: Option<(LaneId, f64)>,
    /// Distance and time driven since spawn.
    driven_m: f64,
    driven_s: f64,
}

/// The harness. Feed it every step with [`CalibrationObserver::observe`], then read
/// [`CalibrationObserver::report`].
#[derive(Debug, Clone)]
pub struct CalibrationObserver {
    prev: BTreeMap<ActorId, Prev>,
    open: Vec<OpenQueue>,
    /// The (approach lane, onset) pairs already captured.
    captured: BTreeSet<(LaneId, u64)>,
    /// Each junction connector's approach lane and turn.
    movement_of: BTreeMap<LaneId, (LaneId, TurnDirection)>,
    /// Lanes that continue straight into each lane without a junction.
    continues_into: BTreeMap<LaneId, Vec<LaneId>>,
    closed: Vec<OpenQueue>,
    queue_sizes: Vec<usize>,
    spacing_m: Vec<f64>,
    free_ratio: Vec<f64>,
    launch_mps2: Vec<f64>,
    stop_decel_mps2: Vec<f64>,
    turn_speeds: BTreeMap<String, Vec<f64>>,
    trip_speed_mps: Vec<f64>,
    total_m: f64,
    total_s: f64,
    ped_prev: BTreeMap<ActorId, v2xw_core::geom::Vec3>,
    ped_speed_mps: Vec<f64>,
    steps: u64,
}

impl CalibrationObserver {
    /// A harness for runs on `world`.
    pub fn new(world: &World) -> Self {
        let mut movement_of = BTreeMap::new();
        let mut continues_into: BTreeMap<LaneId, Vec<LaneId>> = BTreeMap::new();
        for c in world.roads.connections() {
            match c.via {
                Some(via) => {
                    movement_of.insert(via, (c.from_lane, c.direction));
                }
                None => {
                    let from = world.lane(c.from_lane);
                    if from.kind != LaneKind::Internal
                        && world.lane(c.to_lane).kind != LaneKind::Internal
                    {
                        continues_into.entry(c.to_lane).or_default().push(c.from_lane);
                    }
                }
            }
        }
        Self {
            prev: BTreeMap::new(),
            open: Vec::new(),
            captured: BTreeSet::new(),
            movement_of,
            continues_into,
            closed: Vec::new(),
            queue_sizes: Vec::new(),
            spacing_m: Vec::new(),
            free_ratio: Vec::new(),
            launch_mps2: Vec::new(),
            stop_decel_mps2: Vec::new(),
            turn_speeds: BTreeMap::new(),
            trip_speed_mps: Vec::new(),
            total_m: 0.0,
            total_s: 0.0,
            ped_prev: BTreeMap::new(),
            ped_speed_mps: Vec::new(),
            steps: 0,
        }
    }

    /// One mobility step from `t0` to `t1`: `actors` are the vehicles at `t1`,
    /// `despawned` those that left in the step, `pedestrians` the people at `t1`.
    pub fn observe(
        &mut self,
        world: &World,
        t0: SimTime,
        t1: SimTime,
        actors: &[AuditActor],
        despawned: &[(ActorId, DespawnCause)],
        pedestrians: &[AuditPedestrian],
    ) {
        self.steps += 1;
        let dt = ns_to_secs(t1.saturating_sub(t0)).max(1e-9);
        let t1_s = ns_to_secs(t1);
        let t0_s = ns_to_secs(t0);
        self.capture_queues(world, t0_s, t1_s, actors);
        self.track_queues(world, t1, dt, actors, despawned);
        self.track_vehicles(world, t1_s, dt, actors, despawned);
        self.track_pedestrians(dt, pedestrians);
    }

    /// Records the queue on every approach lane whose movements turned green between
    /// `t0` and `t1`, from the states at `t1` — the states the first green decision reads.
    fn capture_queues(&mut self, world: &World, t0_s: f64, t1_s: f64, actors: &[AuditActor]) {
        let mut by_lane: BTreeMap<LaneId, Vec<&AuditActor>> = BTreeMap::new();
        for a in actors {
            if a.changing.is_none() {
                by_lane.entry(a.lane).or_default().push(a);
            }
        }
        for plan in &world.signals {
            let (Some(before), Some(after)) = (plan.states_at(t0_s), plan.states_at(t1_s)) else {
                continue;
            };
            let mut turned: BTreeMap<LaneId, Vec<LaneId>> = BTreeMap::new();
            for (k, m) in plan.controlled.iter().enumerate() {
                let (Some(b), Some(a)) = (before.get(k), after.get(k)) else {
                    continue;
                };
                if is_green(*a) && !is_green(*b) {
                    if let Some((approach, _)) = self.movement_of.get(m) {
                        turned.entry(*approach).or_default().push(*m);
                    }
                }
            }
            if turned.is_empty() {
                continue;
            }
            // The onset is the phase boundary, not the step that saw it.
            let onset_s = plan
                .phase_at(t1_s)
                .map_or(t1_s, |(_, into)| (t1_s - into).max(t0_s));
            for (approach, movements) in turned {
                let key = (approach, (onset_s * 1e3).round() as u64);
                if !self.captured.insert(key) {
                    continue;
                }
                let lane = world.lane(approach);
                // Walk the queue back from the stop line, over the lanes that continue
                // straight into this one.
                let mut members: Vec<Member> = Vec::new();
                let mut spacing = Vec::new();
                let mut upstream = Vec::new();
                let mut cur = approach;
                let mut end_at = lane.length_m; // the stop line, in `cur`'s arc length
                let mut last_rear: Option<f64> = None; // distance of the last rear behind the line
                let mut last_front: Option<f64> = None;
                let mut offset = 0.0; // distance from `cur`'s end to the stop line
                'walk: loop {
                    let mut here: Vec<&AuditActor> =
                        by_lane.get(&cur).cloned().unwrap_or_default();
                    here.sort_by(|a, b| b.s_m.total_cmp(&a.s_m).then(a.actor.cmp(&b.actor)));
                    for a in here {
                        let front_behind = offset + (end_at - a.s_m);
                        let ok_head = match last_rear {
                            None => front_behind <= QUEUE_HEAD_M,
                            Some(r) => front_behind - r <= QUEUE_LINK_M,
                        };
                        // A rider is not a passenger car: the HCM procedure measures
                        // cars, so a queue is counted up to the first bicycle in it.
                        if !ok_head
                            || a.speed_mps >= QUEUED_MPS
                            || a.class == crate::VehicleClass::Bicycle
                        {
                            break 'walk;
                        }
                        if let Some(f) = last_front {
                            if a.speed_mps < STANDING_MPS {
                                spacing.push(front_behind - f);
                            }
                        }
                        last_front = Some(front_behind);
                        last_rear = Some(front_behind + a.length_m);
                        members.push(Member {
                            actor: a.actor,
                            front_cross: None,
                            past_line_m: 0.0,
                            rear_cross: None,
                            turn: None,
                            peak_mps: a.speed_mps,
                            restopped: false,
                            length_m: a.length_m,
                        });
                    }
                    // Continue onto a single lane that feeds this one straight on.
                    let Some(feeders) = self.continues_into.get(&cur) else {
                        break;
                    };
                    if feeders.len() != 1 || upstream.len() >= 3 {
                        break;
                    }
                    offset += end_at;
                    cur = feeders[0];
                    end_at = world.lane(cur).length_m;
                    upstream.push(cur);
                }
                if members.is_empty() {
                    continue;
                }
                self.queue_sizes.push(members.len());
                self.spacing_m.extend(spacing);
                self.open.push(OpenQueue {
                    lane: approach,
                    upstream,
                    movements,
                    onset_s,
                    members,
                });
            }
        }
    }

    /// Times each queued vehicle's stop-line crossing, and closes a queue once it has
    /// discharged or its green has ended.
    fn track_queues(
        &mut self,
        world: &World,
        t1: SimTime,
        dt: f64,
        actors: &[AuditActor],
        despawned: &[(ActorId, DespawnCause)],
    ) {
        let t1_s = ns_to_secs(t1);
        let index: BTreeMap<ActorId, &AuditActor> = actors.iter().map(|a| (a.actor, a)).collect();
        let gone: BTreeSet<ActorId> = despawned.iter().map(|(a, _)| *a).collect();
        let mut still_open = Vec::new();
        for mut q in core::mem::take(&mut self.open) {
            let mut done = true;
            for m in &mut q.members {
                if m.rear_cross.is_some() {
                    continue;
                }
                let Some(a) = index.get(&m.actor) else {
                    // Left the run in the step: whatever it had not crossed is lost.
                    let _ = gone.contains(&m.actor);
                    continue;
                };
                m.peak_mps = m.peak_mps.max(a.speed_mps);
                if m.front_cross.is_none() {
                    let on_queue_lane = a.lane == q.lane || q.upstream.contains(&a.lane);
                    if on_queue_lane {
                        if m.peak_mps > 3.0 && a.speed_mps < STANDING_MPS {
                            m.restopped = true;
                        }
                        done = false;
                        continue;
                    }
                    // The front has left the approach: onto a connector of this stop line.
                    let Some((approach, turn)) = self.movement_of.get(&a.lane).copied() else {
                        // Somewhere else (a lane change): not a discharge of this queue.
                        m.rear_cross = Some(f64::NAN);
                        continue;
                    };
                    if approach != q.lane {
                        m.rear_cross = Some(f64::NAN);
                        continue;
                    }
                    let v = a.speed_mps.max(0.1);
                    m.front_cross = Some(t1_s - (a.s_m / v).min(dt));
                    m.turn = Some(turn);
                    m.past_line_m = a.s_m;
                } else {
                    m.past_line_m += a.speed_mps * dt;
                }
                if m.past_line_m >= m.length_m {
                    let v = a.speed_mps.max(0.1);
                    let over = ((m.past_line_m - m.length_m) / v).min(dt);
                    m.rear_cross = Some(t1_s - over);
                } else {
                    done = false;
                }
            }
            // The green ended with vehicles still queued: they did not discharge on it.
            let green = q.movements.iter().any(|mv| {
                crate::audit::movement_state(world, *mv, t1).is_some_and(is_green)
            });
            let waited_too_long = t1_s - q.onset_s > 120.0;
            if done || waited_too_long || (!green && q.members.iter().all(|m| m.front_cross.is_some() || m.rear_cross.is_some() || !index.contains_key(&m.actor))) {
                self.closed.push(q);
            } else if !green {
                // Anything that has not reached the line by the end of green is out.
                for m in &mut q.members {
                    if m.front_cross.is_none() && m.rear_cross.is_none() {
                        m.rear_cross = Some(f64::NAN);
                    }
                }
                still_open.push(q);
            } else {
                still_open.push(q);
            }
        }
        self.open = still_open;
    }

    /// Speeds, launches, stops, turns, trips.
    fn track_vehicles(
        &mut self,
        world: &World,
        t1_s: f64,
        dt: f64,
        actors: &[AuditActor],
        despawned: &[(ActorId, DespawnCause)],
    ) {
        // Who is ahead of whom on each lane, for the free-flow test.
        let mut by_lane: BTreeMap<LaneId, Vec<(f64, f64)>> = BTreeMap::new();
        for a in actors {
            by_lane
                .entry(a.lane)
                .or_default()
                .push((a.s_m - a.length_m, a.s_m));
        }
        for v in by_lane.values_mut() {
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        for a in actors.iter().filter(|a| a.class != crate::VehicleClass::Bicycle) {
            self.total_m += a.speed_mps * dt;
            self.total_s += dt;
            let lane = world.lane(a.lane);
            let mut p = self.prev.get(&a.actor).copied().unwrap_or(Prev {
                lane: a.lane,
                speed_mps: a.speed_mps,
                pos: a.pos,
                launch: None,
                stop_peak: None,
                connector: None,
                driven_m: 0.0,
                driven_s: 0.0,
            });
            p.driven_m += a.speed_mps * dt;
            p.driven_s += dt;
            // Free-flow speed, mid-block.
            if lane.kind.is_motorised()
                && lane.kind != LaneKind::Internal
                && a.changing.is_none()
                && lane.length_m >= 80.0
                && a.s_m >= 15.0
                && lane.length_m - a.s_m >= 40.0
                && a.accel_mps2.abs() < 0.3
                && lane.speed_limit_mps > 0.0
            {
                let clear = (60.0f64).max(5.0 * a.speed_mps);
                let blocked = by_lane.get(&a.lane).is_some_and(|v| {
                    v.iter()
                        .any(|(rear, _)| *rear > a.s_m && *rear - a.s_m < clear)
                });
                if !blocked {
                    self.free_ratio.push(a.speed_mps / lane.speed_limit_mps);
                }
            }
            // Launches.
            if p.speed_mps < STANDING_MPS && a.speed_mps >= STANDING_MPS {
                p.launch = Some((t1_s - dt, false));
            }
            if let Some((start, held)) = p.launch {
                let held = held || a.accel_mps2 <= 0.0;
                if a.speed_mps >= LAUNCH_TO_MPS {
                    if !held {
                        self.launch_mps2.push(a.speed_mps / (t1_s - start).max(dt));
                    }
                    p.launch = None;
                } else if a.speed_mps < STANDING_MPS {
                    p.launch = None;
                } else {
                    p.launch = Some((start, held));
                }
            }
            // Stops.
            if a.speed_mps > STOP_FROM_MPS {
                p.stop_peak = Some(0.0);
            } else if let Some(peak) = p.stop_peak {
                let peak = peak.max(-a.accel_mps2);
                if a.speed_mps < STANDING_MPS {
                    self.stop_decel_mps2.push(peak);
                    p.stop_peak = None;
                } else if a.accel_mps2 > 0.5 {
                    p.stop_peak = None; // not a stop after all
                } else {
                    p.stop_peak = Some(peak);
                }
            }
            if let Some(peak) = p.stop_peak
                && a.speed_mps > STOP_FROM_MPS
            {
                p.stop_peak = Some(peak.max(-a.accel_mps2));
            }
            // Turns.
            if lane.kind == LaneKind::Internal {
                match p.connector {
                    Some((c, low)) if c == a.lane => p.connector = Some((c, low.min(a.speed_mps))),
                    _ => {
                        self.close_connector(&p);
                        p.connector = Some((a.lane, a.speed_mps));
                    }
                }
            } else if p.connector.is_some() {
                self.close_connector(&p);
                p.connector = None;
            }
            p.lane = a.lane;
            p.speed_mps = a.speed_mps;
            p.pos = a.pos;
            self.prev.insert(a.actor, p);
        }
        for (id, cause) in despawned {
            if let Some(p) = self.prev.remove(id)
                && *cause == DespawnCause::TripComplete
                && p.driven_s > 30.0
            {
                self.trip_speed_mps.push(p.driven_m / p.driven_s);
            }
        }
    }

    fn close_connector(&mut self, p: &Prev) {
        let Some((c, low)) = p.connector else { return };
        if low < 1.0 {
            return; // it stood on the connector: held, not turning at will
        }
        if let Some((_, turn)) = self.movement_of.get(&c) {
            let key = match turn {
                TurnDirection::Left => "left",
                TurnDirection::Right => "right",
                TurnDirection::Straight => "straight",
                TurnDirection::SlightLeft | TurnDirection::SlightRight => "slight",
                TurnDirection::UTurn => "u-turn",
            };
            self.turn_speeds.entry(key.to_string()).or_default().push(low);
        }
    }

    fn track_pedestrians(&mut self, dt: f64, pedestrians: &[AuditPedestrian]) {
        let mut next = BTreeMap::new();
        for p in pedestrians {
            if let Some(q) = self.ped_prev.get(&p.actor) {
                let v = q.distance_2d(p.pos) / dt;
                if (0.3..4.0).contains(&v) {
                    self.ped_speed_mps.push(v);
                }
            }
            next.insert(p.actor, p.pos);
        }
        self.ped_prev = next;
    }

    /// The figures, from everything observed so far.
    pub fn report(&self) -> CalibrationReport {
        let mut queues: Vec<&OpenQueue> = self.closed.iter().collect();
        queues.extend(self.open.iter());
        // Per-position headways, and the saturation headway over positions ≥ 5.
        let mut by_pos: Vec<Vec<f64>> = vec![Vec::new(); POSITIONS];
        let mut sat: Vec<f64> = Vec::new();
        let mut sat_all: Vec<f64> = Vec::new();
        let mut first_four: Vec<[f64; 4]> = Vec::new();
        let mut discharged = 0usize;
        for q in &queues {
            let mut prev_t = q.onset_s;
            let mut prev_straight = true;
            let mut prev_ok = true;
            let mut heads: Vec<f64> = Vec::new();
            for (i, m) in q.members.iter().enumerate() {
                let Some(t) = m.rear_cross.filter(|t| t.is_finite()) else {
                    break; // the rest of the queue did not discharge in order
                };
                discharged += 1;
                let h = t - prev_t;
                let straight = m.turn == Some(TurnDirection::Straight);
                let ok = !m.restopped;
                if i < POSITIONS {
                    by_pos[i].push(h);
                }
                heads.push(h);
                if i >= 4 && ok && prev_ok {
                    sat_all.push(h);
                    if straight && prev_straight {
                        sat.push(h);
                    }
                }
                prev_t = t;
                prev_straight = straight;
                prev_ok = ok;
            }
            if heads.len() >= 5 && q.members.iter().take(4).all(|m| !m.restopped) {
                first_four.push([heads[0], heads[1], heads[2], heads[3]]);
            }
        }
        let h_s = mean(&sat);
        let lost = if h_s.is_finite() && !first_four.is_empty() {
            let per: Vec<f64> = first_four
                .iter()
                .map(|h| h.iter().map(|x| x - h_s).sum::<f64>())
                .collect();
            mean(&per)
        } else {
            f64::NAN
        };
        let mut turns = BTreeMap::new();
        for (k, v) in &self.turn_speeds {
            turns.insert(k.clone(), Summary::of(v));
        }
        CalibrationReport {
            steps: self.steps,
            queues: queues.len(),
            queued_vehicles_discharged: discharged,
            saturation_headway_s: Summary::of(&sat),
            saturation_headway_all_movements_s: Summary::of(&sat_all),
            startup_lost_time_s: lost,
            startup_lost_time_queues: first_four.len(),
            headway_by_position_s: by_pos.iter().map(|v| Summary::of(v)).collect(),
            queue_size: Summary::of(
                &self.queue_sizes.iter().map(|n| *n as f64).collect::<Vec<_>>(),
            ),
            queue_spacing_m: Summary::of(&self.spacing_m),
            free_flow_speed_ratio: Summary::of(&self.free_ratio),
            launch_accel_mps2: Summary::of(&self.launch_mps2),
            stop_decel_mps2: Summary::of(&self.stop_decel_mps2),
            turn_speed_mps: turns,
            network_speed_mps: if self.total_s > 0.0 {
                self.total_m / self.total_s
            } else {
                f64::NAN
            },
            trip_speed_mps: Summary::of(&self.trip_speed_mps),
            pedestrian_speed_mps: Summary::of(&self.ped_speed_mps),
        }
    }
}

fn is_green(s: SignalState) -> bool {
    matches!(s, SignalState::Green | SignalState::GreenYield)
}

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    // Summed in the order observed, which is the run's own deterministic order.
    v.iter().sum::<f64>() / v.len() as f64
}

/// A sample's size, mean, standard deviation and percentiles.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Summary {
    /// Samples.
    pub n: usize,
    /// Mean.
    pub mean: f64,
    /// Standard deviation (population).
    pub sd: f64,
    /// 15th percentile.
    pub p15: f64,
    /// Median.
    pub p50: f64,
    /// 85th percentile.
    pub p85: f64,
    /// Largest.
    pub max: f64,
}

impl Summary {
    /// The summary of `v` (NaN everywhere when empty).
    pub fn of(v: &[f64]) -> Summary {
        if v.is_empty() {
            return Summary {
                n: 0,
                mean: f64::NAN,
                sd: f64::NAN,
                p15: f64::NAN,
                p50: f64::NAN,
                p85: f64::NAN,
                max: f64::NAN,
            };
        }
        let m = mean(v);
        let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / v.len() as f64;
        let mut s = v.to_vec();
        s.sort_by(f64::total_cmp);
        let pct = |p: f64| s[((p * (s.len() - 1) as f64).round() as usize).min(s.len() - 1)];
        Summary {
            n: v.len(),
            mean: m,
            sd: v2xw_core::math::sqrt(var),
            p15: pct(0.15),
            p50: pct(0.5),
            p85: pct(0.85),
            max: s[s.len() - 1],
        }
    }
}

/// What the harness measured.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CalibrationReport {
    /// Steps observed.
    pub steps: u64,
    /// Queues recorded at a green onset.
    pub queues: usize,
    /// Queued vehicles timed across the stop line.
    pub queued_vehicles_discharged: usize,
    /// Saturation headway, through movements, positions ≥ 5, seconds.
    pub saturation_headway_s: Summary,
    /// The same over every movement.
    pub saturation_headway_all_movements_s: Summary,
    /// Start-up lost time, seconds.
    pub startup_lost_time_s: f64,
    /// Queues the lost time is averaged over.
    pub startup_lost_time_queues: usize,
    /// Mean discharge headway by queue position 1..10, seconds.
    pub headway_by_position_s: Vec<Summary>,
    /// Vehicles standing in a queue at the green onset, per approach lane per cycle.
    pub queue_size: Summary,
    /// Front-to-front spacing in a standing queue, metres.
    pub queue_spacing_m: Summary,
    /// Free-flow mid-block speed over the posted limit.
    pub free_flow_speed_ratio: Summary,
    /// Mean acceleration from standstill to 8 m/s, m/s².
    pub launch_accel_mps2: Summary,
    /// Peak deceleration of each stop, m/s².
    pub stop_decel_mps2: Summary,
    /// Lowest speed through a connector, by turn, m/s.
    pub turn_speed_mps: BTreeMap<String, Summary>,
    /// Total distance over total vehicle-time, m/s.
    pub network_speed_mps: f64,
    /// Completed trips' mean speed, m/s.
    pub trip_speed_mps: Summary,
    /// Walking pedestrians' speed, m/s.
    pub pedestrian_speed_mps: Summary,
}

/// One figure against its reference.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Comparison {
    /// What is compared.
    pub metric: &'static str,
    /// The measured value.
    pub measured: f64,
    /// Samples behind it.
    pub samples: usize,
    /// The reference value.
    pub reference: f64,
    /// The band the measured value must fall in to pass.
    pub band: (f64, f64),
    /// Where the reference comes from.
    pub source: &'static str,
    /// Whether the test holds the model to it (`false`: reported for comparison only).
    pub tested: bool,
}

impl Comparison {
    /// True if the measured value is in the band (and there is one to judge).
    pub fn passes(&self) -> bool {
        self.samples > 0 && self.measured >= self.band.0 && self.measured <= self.band.1
    }
}

/// HCM base saturation flow, pc/h/ln (HCM 2000 Ch. 16 / HCM 6th Ch. 19 default for a
/// metropolitan area of 250,000 or more; secondary: read in FHWA STM §3.3.1 and the
/// McTrans HCS calibration guide, not the HCM itself).
pub const HCM_BASE_SATURATION_FLOW: f64 = 1900.0;
/// HCM area-type adjustment factor in a central business district.
pub const HCM_CBD_AREA_FACTOR: f64 = 0.90;
/// HCM default start-up lost time, s.
pub const HCM_STARTUP_LOST_TIME_S: f64 = 2.0;
/// HCM average queue storage length per vehicle, m (25 ft).
pub const HCM_QUEUE_SPACING_M: f64 = 7.62;

impl CalibrationReport {
    /// Every figure beside its reference. `tested` marks the ones a calibration test holds
    /// the model to; the rest depend on demand the reference does not share.
    pub fn comparisons(&self) -> Vec<Comparison> {
        let h_s_ref = 3600.0 / (HCM_BASE_SATURATION_FLOW * HCM_CBD_AREA_FACTOR);
        vec![
            Comparison {
                metric: "saturation headway, through, s",
                measured: self.saturation_headway_s.mean,
                samples: self.saturation_headway_s.n,
                reference: h_s_ref,
                // FHWA STM: saturation flow "commonly range[s] from 1,500 to 2,000".
                band: (3600.0 / 2000.0, 3600.0 / 1500.0),
                source: "HCM base 1,900 pc/h/ln × CBD factor 0.90 (secondary, via FHWA Signal \
                         Timing Manual 2008 §3.3.1); band: the manual's 1,500–2,000 range",
                tested: true,
            },
            Comparison {
                metric: "start-up lost time, s",
                measured: self.startup_lost_time_s,
                samples: self.startup_lost_time_queues,
                reference: HCM_STARTUP_LOST_TIME_S,
                band: (1.0, 3.0),
                source: "HCM default 2.0 s (FHWA STM 2008 §3.3.1: \"commonly assumed to be \
                         approximately 2 seconds\"); band ±1 s",
                tested: true,
            },
            Comparison {
                metric: "queue spacing, front to front, m",
                measured: self.queue_spacing_m.mean,
                samples: self.queue_spacing_m.n,
                reference: HCM_QUEUE_SPACING_M,
                band: (6.5, 8.5),
                source: "HCM average queue storage per vehicle, 25 ft (secondary)",
                tested: true,
            },
            Comparison {
                metric: "free-flow speed / limit, mean",
                measured: self.free_flow_speed_ratio.mean,
                samples: self.free_flow_speed_ratio.n,
                reference: 1.0,
                band: (0.93, 1.07),
                source: "SUMO passenger vType speedFactor N(1.0, 0.1): a model default, no \
                         New York field distribution was available",
                tested: true,
            },
            Comparison {
                metric: "free-flow speed / limit, sd",
                measured: self.free_flow_speed_ratio.sd,
                samples: self.free_flow_speed_ratio.n,
                reference: 0.1,
                band: (0.05, 0.15),
                source: "SUMO passenger vType speedDev 0.1",
                tested: true,
            },
            Comparison {
                metric: "launch acceleration 0-8 m/s, mean, m/s²",
                measured: self.launch_accel_mps2.mean,
                samples: self.launch_accel_mps2.n,
                reference: 1.25,
                band: (0.9, 2.0),
                source: "Wang, Dixon, Li & Ogle 2004, TRR 1883: 0.127 g mean over 15 s from \
                         rest (secondary: abstract); band a choice",
                tested: true,
            },
            Comparison {
                metric: "stop deceleration, 85th percentile, m/s²",
                measured: self.stop_decel_mps2.p85,
                samples: self.stop_decel_mps2.n,
                reference: 3.4,
                band: (1.5, 3.4),
                source: "AASHTO Green Book 2018 §3.2.2: 3.4 m/s² comfortable for most drivers",
                tested: true,
            },
            Comparison {
                metric: "pedestrian walking speed, mean, m/s",
                measured: self.pedestrian_speed_mps.mean,
                samples: self.pedestrian_speed_mps.n,
                reference: 1.51,
                band: (1.2, 1.6),
                source: "Knoblauch, Pietrucha & Nitzburg 1996, TRR 1538: younger pedestrians \
                         1.51 m/s mean, older 1.25 (secondary)",
                tested: true,
            },
            Comparison {
                metric: "pedestrian walking speed, 15th percentile, m/s",
                measured: self.pedestrian_speed_mps.p15,
                samples: self.pedestrian_speed_mps.n,
                reference: 1.25,
                band: (0.97, 1.4),
                source: "Knoblauch 1996: 15th percentile 1.25 m/s younger, 0.97 m/s older; \
                         MUTCD 2009 §4E.06 design 3.5 ft/s (1.07 m/s)",
                tested: true,
            },
            Comparison {
                metric: "network travel speed, m/s",
                measured: self.network_speed_mps,
                samples: self.steps as usize,
                reference: 2.2,
                band: (1.8, 4.0),
                source: "NYC DOT Mobility Report 2019: Midtown core ≈ 5 mph, CBD ≈ 7 mph \
                         (2017 taxi GPS, all-day); depends on demand, reported only",
                tested: false,
            },
        ]
    }
}

/// The HCM saturation-flow field study, run on the model: one signalised crossroads whose
/// four approaches are kept loaded, so every green starts with a long standing queue.
///
/// The world is a 3 × 3 procedural grid of 274 m blocks (Midtown's avenue spacing), 25 mph,
/// `lanes_per_direction` lanes each way, a fixed-time plan of `cycle_s`; no crosswalks, so
/// nothing but the queue itself governs its discharge — the HCM's base conditions. Each
/// step, every approach lane of the centre junction gets a new car at its upstream end
/// when there is room (a `MobilityCommand::Spawn`, so it is routed and given a driver like
/// any demand trip), bound straight on through the centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SaturationExperiment {
    /// Simulated seconds.
    pub seconds: u64,
    /// Signal cycle, seconds.
    pub cycle_s: f64,
    /// Lanes each way.
    pub lanes_per_direction: u32,
    /// The master seed.
    pub seed: u64,
}

impl Default for SaturationExperiment {
    fn default() -> Self {
        Self {
            seconds: 600,
            // A common Midtown cycle; the queue discharge does not depend on it.
            cycle_s: 90.0,
            lanes_per_direction: 2,
            seed: 0x5A7_F10E,
        }
    }
}

impl SaturationExperiment {
    /// The experiment's world.
    ///
    /// # Errors
    /// The procedural generator's.
    pub fn world(&self) -> core::result::Result<World, Box<dyn std::error::Error>> {
        let params = v2xw_world::procedural::GridParams {
            cols: 3,
            rows: 3,
            block_x_m: 274.0,
            block_y_m: 274.0,
            lanes_per_direction: self.lanes_per_direction,
            lane_width_m: 3.35,
            sidewalk_m: 2.0,
            speed_limit_mps: 11.176,
            signalised: true,
            cycle_s: self.cycle_s,
            amber_s: 3.0,
            crossings: false,
            block_buildings: false,
            corner_radius_m: 4.5,
            ..v2xw_world::procedural::GridParams::legacy()
        };
        Ok(v2xw_world::procedural::grid(
            &params,
            &v2xw_world::ImportOptions::default(),
        )?)
    }

    /// Runs `engine` (constructed, not yet initialised) through the experiment and
    /// returns what the harness measured.
    ///
    /// # Errors
    /// The world generator's, or the engine's `init`.
    pub fn run(
        &self,
        mut engine: crate::NativeMobility,
    ) -> core::result::Result<CalibrationReport, Box<dyn std::error::Error>> {
        use crate::traits::Mobility;
        let world = self.world()?;
        let rng = v2xw_core::rng::RngRegistry::new(self.seed);
        {
            let mut ctx = crate::MobilityCtx::new(0, &world, &rng);
            engine.init(&mut ctx, Box::new(crate::demand::NoDemand::new()))?;
        }
        // The centre: the signalised junction with the most approach lanes.
        let centre = world
            .roads
            .junctions()
            .iter()
            .filter(|j| matches!(j.control, v2xw_world::JunctionControl::Signalised { .. }))
            .max_by_key(|j| (j.incoming.len(), core::cmp::Reverse(j.id)))
            .ok_or("no signalised junction")?
            .id;
        // Each approach lane of the centre, and the lane straight on beyond it.
        let mut feeds: Vec<(LaneId, LaneId)> = Vec::new();
        for lane in world.roads.lanes() {
            if lane.kind != LaneKind::Driving || world.edge(lane.edge).to != centre {
                continue;
            }
            if let Some(c) = world
                .successors(lane.id)
                .iter()
                .find(|c| c.via.is_some() && c.direction == TurnDirection::Straight)
            {
                feeds.push((lane.id, c.to_lane));
            }
        }
        let step = v2xw_core::time::Duration::from_millis(100);
        let mut harness = CalibrationObserver::new(&world);
        let horizon = self.seconds * v2xw_core::time::NS_PER_S;
        let mut seq = 1_000_000u64;
        let mut t = 0u64;
        while t < horizon {
            let mut nearest: BTreeMap<LaneId, f64> = BTreeMap::new();
            for (_, lane, s, _) in engine.longitudinal_states() {
                let e = nearest.entry(lane).or_insert(f64::INFINITY);
                *e = e.min(s);
            }
            {
                let mut ctx = crate::MobilityCtx::new(t, &world, &rng);
                for (origin, exit) in &feeds {
                    // Room for a car and its standstill gap behind the last one in.
                    if nearest.get(origin).is_some_and(|front| *front < 14.0) {
                        continue;
                    }
                    seq += 1;
                    let class = crate::VehicleClass::Passenger;
                    engine.command(
                        &mut ctx,
                        crate::MobilityCommand::Spawn(crate::TripRequest {
                            seq,
                            t,
                            origin: *origin,
                            origin_s_m: class.spec().length_m,
                            destination: *exit,
                            class,
                            desired_speed_mps: class.spec().desired_speed_mps(),
                        }),
                    );
                }
            }
            let mut ctx = crate::MobilityCtx::new(t, &world, &rng);
            let update = engine.step(&mut ctx, step);
            let actors = engine.audit_actors(&world, update.t);
            harness.observe(&world, t, update.t, &actors, &update.despawned, &[]);
            t = update.t;
        }
        Ok(harness.report())
    }
}
