//! The SPaT's timing bounds against what its controller then does (live F-traffic track): under
//! actuated control and a priority service `maxEndTime` is an upper bound and `minEndTime` a
//! lower bound on every group's next change, whatever the detectors and the requests do. Session
//! 2's probe found 42 % of changes
//! after `maxEndTime` under actuated control and 16 % before `minEndTime` under priority service.

use std::collections::{BTreeMap, BTreeSet};

use v2xw_core::ids::LaneId;
use v2xw_core::time::WallClock;
use v2xw_engine::actuated::ActuatedControllers;
use v2xw_engine::infra::IntersectionFeed;
use v2xw_engine::priority::{PriorityControllers, PriorityParams};
use v2xw_engine::scenario::schema::{SignalControl, SignalControlKind};
use v2xw_msg::j2735::spat::seconds_until;
use v2xw_world::ImportOptions;
use v2xw_world::World;
use v2xw_world::procedural::GridParams;

/// The resolution of a `TimeMark`, a tenth of a second, and of the step.
const TOL_S: f64 = 0.25;

fn grid() -> World {
    v2xw_world::procedural::grid(
        &GridParams::legacy().with_size(3, 3).with_signals(true),
        &ImportOptions::default(),
    )
    .expect("grid")
}

/// A small reproducible random stream.
struct Draw(u64);
impl Draw {
    fn chance(&mut self, p: f64) -> bool {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) as f64 / (1u64 << 31) as f64) < p
    }
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n
    }
}

/// One stated window: (at, min, max).
type Window = (f64, f64, Option<f64>);

/// Every stated window of every group, judged at the group's next change.
#[derive(Default)]
#[allow(
    clippy::type_complexity,
    reason = "a one-off tuple read where it is built; an alias would hide its fields"
)]
struct Judge {
    /// group -> (state shown, the windows stated since it began: (at, min, max)).
    open: BTreeMap<u8, (u8, Vec<Window>)>,
    changes: u64,
    windows: u64,
    early: u64,
    late: u64,
    unbounded: u64,
    worst_early_s: f64,
    worst_late_s: f64,
}

impl Judge {
    fn see(&mut self, t: f64, group: u8, state: u8, min: f64, max: Option<f64>) {
        let e = self.open.entry(group).or_insert((state, Vec::new()));
        if e.0 != state {
            self.changes += 1;
            for (_, lo, hi) in e.1.drain(..) {
                self.windows += 1;
                if t < lo - TOL_S {
                    self.early += 1;
                    self.worst_early_s = self.worst_early_s.max(lo - t);
                }
                match hi {
                    Some(hi) if t > hi + TOL_S => {
                        self.late += 1;
                        self.worst_late_s = self.worst_late_s.max(t - hi);
                    }
                    None => self.unbounded += 1,
                    _ => {}
                }
            }
            e.0 = state;
        }
        e.1.push((t, min, max));
    }
}

/// What a SPaT at `t` states of each group, handed to `judge`.
fn observe(judge: &mut Judge, spat: &v2xw_msg::j2735::spat::Spat, t: f64, hour_s: f64) {
    for s in &spat.intersections[0].states {
        let e = &s.events[0];
        let timing = e.timing.expect("timed");
        let min = t + seconds_until(timing.min_end_time, hour_s).expect("minEndTime");
        let max = timing
            .max_end_time
            .and_then(|m| seconds_until(m, hour_s))
            .map(|d| t + d);
        judge.see(t, s.signal_group, e.event_state.index() as u8, min, max);
    }
}

fn wall() -> WallClock {
    WallClock::parse_rfc3339("2027-03-04T17:00:00Z").expect("t0")
}

fn hour_s(wall: WallClock, t: f64) -> f64 {
    v2xw_node::apps::seconds_into_hour(wall, (t * 1e9) as u64)
}

/// The actuated controller with vehicles arriving at random on every approach: calls stand on a
/// red until served, as a locking detector holds them, and thin out on the green.
#[test]
fn the_actuated_window_bounds_every_change() {
    let mut world = grid();
    let control = SignalControl {
        control: SignalControlKind::Actuated,
        ..SignalControl::default()
    };
    let mut ctl = ActuatedControllers::new(&control, &world);
    let origin: v2xw_core::geo::GeoOrigin = world.origin.into();
    let mut feed = IntersectionFeed::build(&world, 0, origin).expect("a feed");
    let approach = world.movement_approaches();
    let lanes: BTreeSet<LaneId> = world.signals[0]
        .controlled
        .iter()
        .filter_map(|l| approach.get(l).copied())
        .collect();
    assert!(lanes.len() >= 2, "an approach each way");
    let wall = wall();
    let mut draw = Draw(0x5eed);
    let mut calls: BTreeSet<LaneId> = BTreeSet::new();
    let mut judge = Judge::default();
    let dt = 0.1;
    let mut gap_outs = 0;
    for step in 0..36_000u64 {
        let t = step as f64 * dt;
        let plan = &world.signals[0];
        let shown: Vec<bool> = plan
            .phase_at(t)
            .map(|(i, _)| {
                plan.controlled
                    .iter()
                    .zip(&plan.phases[i].states)
                    .map(|(l, s)| {
                        lanes.contains(approach.get(l).unwrap_or(l))
                            && matches!(
                                s,
                                v2xw_world::SignalState::Green
                                    | v2xw_world::SignalState::GreenYield
                            )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let green: BTreeSet<LaneId> = plan
            .controlled
            .iter()
            .zip(&shown)
            .filter(|(_, g)| **g)
            .filter_map(|(l, _)| approach.get(l).copied())
            .collect();
        // Quiet spells let a green rest, busy ones hold it to its maximum.
        let busy = (step / 3_000) % 2 == 0;
        for l in &lanes {
            if green.contains(l) {
                if draw.chance(if busy { 0.02 } else { 0.2 }) {
                    calls.remove(l);
                } else if draw.chance(if busy { 0.2 } else { 0.005 }) {
                    calls.insert(*l);
                }
            } else if draw.chance(if busy { 0.01 } else { 0.001 }) {
                calls.insert(*l);
            }
        }
        let before = ctl.counters().gap_outs;
        ctl.step_with(&mut world, &calls, &calls, t, dt);
        gap_outs += ctl.counters().gap_outs - before;
        let plan = &world.signals[0];
        feed.sync(plan);
        feed.set_actuation(ctl.outlook(&world, 0, t));
        let spat = feed.spat_at((t * 1e9) as u64, wall);
        observe(&mut judge, &spat, t, hour_s(wall, t));
    }
    assert!(gap_outs > 3, "gap-outs {gap_outs}");
    assert!(judge.changes > 100, "changes {}", judge.changes);
    assert!(judge.unbounded > 0, "a green rested in the quiet spells");
    assert_eq!(
        (judge.early, judge.late),
        (0, 0),
        "of {} windows {} early (by up to {:.1} s) and {} late (by up to {:.1} s)",
        judge.windows,
        judge.early,
        judge.worst_early_s,
        judge.late,
        judge.worst_late_s
    );
}

/// A fixed-time plan under a priority service, requests from every group coming and going: the
/// stated bounds hold whatever comes, and a service never holds a green past them.
#[test]
fn the_priority_window_bounds_every_change() {
    let mut world = grid();
    let mut service = PriorityControllers::new(PriorityParams::default());
    service.set_enabled(true);
    let origin: v2xw_core::geo::GeoOrigin = world.origin.into();
    let mut feed = IntersectionFeed::build(&world, 0, origin).expect("a feed");
    let mut groups: Vec<u16> = world.signals[0].heads.iter().map(|h| h.group).collect();
    groups.sort_unstable();
    groups.dedup();
    let wall = wall();
    let mut draw = Draw(0xabc);
    let mut active: Vec<(u16, [u8; 8], f64)> = Vec::new();
    let mut judge = Judge::default();
    let (mut extends, mut cuts) = (0, 0);
    let dt = 0.1;
    for step in 0..36_000u64 {
        let t = step as f64 * dt;
        if draw.chance(0.004) {
            let g = groups[draw.below(groups.len() as u64) as usize];
            let mut id = [0u8; 8];
            id[0] = draw.below(255) as u8;
            id[1] = draw.below(255) as u8;
            active.push((g, id, t + draw.below(25) as f64 + 1.0));
        }
        active.retain(|a| a.2 > t);
        for a in &active {
            service.request(&world, 0, a.1, a.0, a.2 - t, t);
        }
        for r in service.step(&mut world, t, dt) {
            match r.action {
                "extend" => extends += 1,
                "early-green" => cuts += 1,
                _ => {}
            }
        }
        feed.sync(&world.signals[0]);
        feed.set_priority_window(service.window(&world, 0, t));
        let spat = feed.spat_with((t * 1e9) as u64, wall, service.outlook(&world, 0, t));
        observe(&mut judge, &spat, t, hour_s(wall, t));
    }
    assert!(extends > 3, "extensions {extends}");
    assert!(cuts > 3, "early greens {cuts}");
    assert!(judge.changes > 100, "changes {}", judge.changes);
    assert_eq!(
        (judge.early, judge.late),
        (0, 0),
        "of {} windows {} early (by up to {:.1} s) and {} late (by up to {:.1} s)",
        judge.windows,
        judge.early,
        judge.worst_early_s,
        judge.late,
        judge.worst_late_s
    );
}
