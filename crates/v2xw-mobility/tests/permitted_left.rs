//! Permitted left turns on a saturated signalised grid.
//!
//! The traffic validation of 2026-10-24 (`docs/validation/traffic-2026-10-24.md`, open
//! item 1) found queue heads on a permissive green standing at the stop line for 216-258 s
//! with the junction and their exit empty. Two causes, each with a test here:
//!
//! * the turner counted opposing cars that were not coming — one standing at its line
//!   behind a full exit, and every car queued behind a standing one — as arriving within
//!   the critical headway, so it never found a gap;
//! * nobody turned at the end of the green. HCM 6th ed. Ch. 31 credits a permitted left
//!   lane with the turners that clear at the change interval (the "sneakers", about two a
//!   cycle); facing a continuous opposing stream they are its only capacity.
//!
//! The grid is the traffic validation's `phase1-grid` S family: 4 x 6 junctions, blocks
//! 274 m x 80 m, two lanes each way, 25 mph, fixed time 60 s with a 3 s amber, 9,000
//! veh/h of demand.

use std::collections::BTreeMap;

use v2xw_core::rng::RngRegistry;
use v2xw_core::time::{Duration, NS_PER_S};
use v2xw_mobility::audit::{AuditActor, AuditParams, Check, TrafficAuditor, movement_state};
use v2xw_mobility::demand::{OdParams, PoissonDemand, PoissonParams};
use v2xw_mobility::{EngineParams, Mobility, MobilityCtx, NativeMobility};
use v2xw_world::procedural::GridParams;
use v2xw_world::{ImportOptions, LaneKind, SignalState, TurnDirection, World};

/// The S-family grid of the traffic validation.
fn world() -> World {
    let params = GridParams {
        cols: 4,
        rows: 6,
        block_x_m: 274.0,
        block_y_m: 80.0,
        lanes_per_direction: 2,
        lane_width_m: 3.5,
        sidewalk_m: 2.0,
        speed_limit_mps: 11.176,
        signalised: true,
        cycle_s: 60.0,
        amber_s: 3.0,
        block_buildings: true,
        corner_radius_m: 4.5,
        ..GridParams::legacy()
    };
    v2xw_world::procedural::grid(&params, &ImportOptions::default()).expect("grid")
}

/// What the run showed about permitted left turners.
#[derive(Debug, Default)]
struct Lefts {
    /// The longest a left turner at the head of its lane stood on its permissive green, in
    /// one wait at the line, while its exit had room for it, s.
    longest_green_wait_s: f64,
    /// The longest a left turner stood still on its connector inside the junction, s.
    longest_inside_wait_s: f64,
    /// The left turners that left their connector while it showed amber or red: the
    /// sneakers, who waited inside the junction and cleared at the change.
    sneakers: u32,
    /// Every left turn completed off a permitted-left connector.
    turns: u32,
    /// Red entries and conflict-zone violations the auditor saw.
    unsafe_entries: u64,
}

fn left_connector(world: &World, via: v2xw_core::ids::LaneId) -> Option<v2xw_core::ids::LaneId> {
    world
        .roads
        .connections()
        .iter()
        .find(|c| {
            c.via == Some(via)
                && matches!(
                    c.direction,
                    TurnDirection::Left | TurnDirection::SlightLeft | TurnDirection::UTurn
                )
        })
        .map(|c| c.to_lane)
}

/// True if the exit lane has room for `a` behind the rearmost vehicle on it.
fn exit_has_room(world: &World, actors: &[AuditActor], exit: v2xw_core::ids::LaneId, a: &AuditActor) -> bool {
    let _ = world;
    actors
        .iter()
        .filter(|o| o.lane == exit)
        .map(|o| o.s_m - o.length_m)
        .fold(f64::INFINITY, f64::min)
        >= a.length_m + 2.0 * a.min_gap_m
}

fn run(seconds: u64) -> Lefts {
    let world = world();
    let rng = RngRegistry::new(0xC0FF_EE5E);
    let params = EngineParams::default();
    let mut engine = NativeMobility::new(params);
    let demand = PoissonDemand::new(
        &world,
        PoissonParams {
            arrival_rate_per_s: 2.5,
            duration: Duration::from_secs(seconds),
            ..PoissonParams::default()
        },
        OdParams::default(),
    )
    .expect("demand");
    {
        let mut ctx = MobilityCtx::new(0, &world, &rng);
        engine.init(&mut ctx, Box::new(demand)).expect("init");
    }
    let mut auditor = TrafficAuditor::new(&world, AuditParams::default());
    let mut out = Lefts::default();
    // Per actor: the lane it waits on and how long it has stood there on a permissive
    // green with room on its exit.
    let mut waiting: BTreeMap<u32, (u32, f64)> = BTreeMap::new();
    let mut prev: BTreeMap<u32, v2xw_core::ids::LaneId> = BTreeMap::new();
    let mut t = 0u64;
    while t < seconds * NS_PER_S {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = engine.step(&mut ctx, params.step);
        let actors = engine.audit_actors(&world, update.t);
        auditor.observe(&world, t, update.t, &actors, &update.despawned);
        let dt = (update.t - t) as f64 / NS_PER_S as f64;
        // The front-most vehicle on each approach lane.
        let mut head: BTreeMap<u32, (f64, u32)> = BTreeMap::new();
        for a in &actors {
            if world.lane(a.lane).kind == LaneKind::Internal {
                continue;
            }
            let e = head.entry(a.lane.index()).or_insert((a.s_m, a.actor.index()));
            if a.s_m > e.0 {
                *e = (a.s_m, a.actor.index());
            }
        }
        let mut still: Vec<u32> = Vec::new();
        for a in &actors {
            let id = a.actor.index();
            let lane = world.lane(a.lane);
            // Turns completed off a left-turn connector, by the state it showed.
            if lane.kind != LaneKind::Internal
                && let Some(p) = prev.get(&id).copied()
                && world.lane(p).kind == LaneKind::Internal
                && left_connector(&world, p).is_some()
            {
                match movement_state(&world, p, t) {
                    Some(SignalState::Amber | SignalState::Red) => {
                        out.sneakers += 1;
                        out.turns += 1;
                    }
                    Some(SignalState::GreenYield) => out.turns += 1,
                    _ => {}
                }
            }
            // Standing inside the junction on a left-turn connector.
            if lane.kind == LaneKind::Internal
                && a.speed_mps <= 0.1
                && left_connector(&world, a.lane).is_some()
            {
                still.push(id);
                let e = waiting.entry(id).or_insert((a.lane.index(), 0.0));
                if e.0 != a.lane.index() {
                    *e = (a.lane.index(), 0.0);
                }
                e.1 += dt;
                out.longest_inside_wait_s = out.longest_inside_wait_s.max(e.1);
                continue;
            }
            let Some(next) = a.route_next else { continue };
            let Some(exit) = left_connector(&world, next) else {
                continue;
            };
            let at_head = head.get(&a.lane.index()).is_some_and(|h| h.1 == id);
            if lane.kind == LaneKind::Internal
                || !at_head
                || a.speed_mps > 0.1
                || lane.length_m - a.s_m > 3.0
            {
                continue;
            }
            still.push(id);
            let e = waiting.entry(id).or_insert((a.lane.index(), 0.0));
            if e.0 != a.lane.index() {
                *e = (a.lane.index(), 0.0);
            }
            if movement_state(&world, next, update.t) == Some(SignalState::GreenYield)
                && exit_has_room(&world, &actors, exit, a)
            {
                e.1 += dt;
                out.longest_green_wait_s = out.longest_green_wait_s.max(e.1);
            }
        }
        waiting.retain(|id, _| still.contains(id));
        prev = actors.iter().map(|a| (a.actor.index(), a.lane)).collect();
        t = update.t;
    }
    let report = auditor.report();
    out.unsafe_entries = report.count(Check::RedEntry) + report.count(Check::ConflictZone);
    out
}

#[test]
fn permitted_left_turners_get_through_on_a_saturated_grid() {
    let lefts = run(300);
    eprintln!("{lefts:?}");
    // A queue head with room on its exit is never held through more than two permissive
    // greens (27 s each): whatever the opposing stream, it clears at the latest at the end
    // of its second green. On 38ad00cf the longest such wait was well over a minute.
    assert!(
        lefts.longest_green_wait_s <= 54.0,
        "a permitted left turner stood {:.1} s of permissive green at the head of its lane \
         with room on its exit",
        lefts.longest_green_wait_s
    );
    // A turner waiting inside the junction clears at the latest at the end of the green
    // it went in on: never a whole cycle (60 s) standing inside.
    assert!(
        lefts.longest_inside_wait_s <= 60.0,
        "a left turner stood {:.1} s inside the junction",
        lefts.longest_inside_wait_s
    );
    // Sneakers happen: turners who waited inside and cleared on the change interval. With
    // 24 junctions, two permitted-left approaches each and five cycles there are 240
    // approach-cycles; HCM's two a cycle is the ceiling, not the expectation.
    assert!(lefts.sneakers > 0, "no left turner cleared at the change: {lefts:?}");
    assert!(lefts.sneakers <= 2 * 240, "{lefts:?}");
    // And none of it is bought with a red entry or a conflict-zone violation.
    assert_eq!(lefts.unsafe_entries, 0, "{lefts:?}");
}
