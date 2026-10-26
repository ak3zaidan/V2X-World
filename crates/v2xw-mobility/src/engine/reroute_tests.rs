//! WP-05 (frameworks/00-adoption-plan.md): the re-routing share and source (sumo.md P14)
//! and boundary insertion at speed (P9).

use super::*;
use crate::ctx::MobilityCtx;
use crate::demand::NoDemand;
use crate::views::TripRequest;
use v2xw_core::rng::RngRegistry;
use v2xw_core::time::NS_PER_S;
use v2xw_world::{ImportOptions, procedural::GridParams};

fn grid() -> World {
    v2xw_world::procedural::grid(
        &GridParams::legacy().with_signals(false),
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

fn trip(seq: u64, origin: LaneId, s_m: f64, destination: LaneId) -> TripRequest {
    TripRequest {
        seq,
        t: 0,
        origin,
        origin_s_m: s_m,
        destination,
        class: VehicleClass::Passenger,
        desired_speed_mps: 11.0,
    }
}

/// A driving lane over 100 m long and a destination at least eight lanes of route away.
fn far_trip(world: &World) -> (LaneId, LaneId) {
    let mut driving: Vec<LaneId> = world
        .roads
        .lanes()
        .iter()
        .filter(|l| l.kind == LaneKind::Driving && l.length_m > 100.0)
        .map(|l| l.id)
        .collect();
    driving.sort_unstable();
    let costs = DynamicCost::new(world);
    let router = DynamicReroute::new(
        DijkstraParams::for_classes(ClassMask::CAR),
        crate::views::ReroutePolicy::STATIC,
    );
    let origin = driving[0];
    for candidate in driving.iter().rev() {
        if router
            .replan(world, origin, *candidate, 0, &costs)
            .is_some_and(|r| r.lanes.len() >= 8)
        {
            return (origin, *candidate);
        }
    }
    panic!("no far destination on the grid");
}

fn step_for(engine: &mut NativeMobility, world: &World, rng: &RngRegistry, from: u64, s: u64) {
    let step = engine.params.step.as_nanos();
    let mut t = from;
    while t < from + s * NS_PER_S {
        let mut ctx = MobilityCtx::new(t, world, rng);
        engine.step(&mut ctx, engine.params.step);
        t += step;
    }
}

/// `reroute_share` 0 equips nobody, 1 everybody (with no draw), and 0.3 three drivers in
/// ten within the binomial's 3σ; the draw is the driver's own, so it repeats exactly.
#[test]
fn the_reroute_share_equips_its_share_of_drivers() {
    let world = grid();
    let (origin, destination) = far_trip(&world);
    let n = 4_000u64;
    let draws = |share: f64| -> Vec<bool> {
        let rng = RngRegistry::new(11);
        let engine = engine_on(
            &world,
            EngineParams {
                reroute_share: share,
                ..EngineParams::default()
            },
            &rng,
        );
        let ctx = MobilityCtx::new(0, &world, &rng);
        (0..n)
            .map(|seq| {
                engine
                    .traits_for(&ctx, &trip(seq, origin, 10.0, destination))
                    .reroutes
            })
            .collect()
    };
    let count = |v: &[bool]| v.iter().filter(|b| **b).count() as f64;
    assert_eq!(count(&draws(0.0)), 0.0, "share 0 equips nobody");
    assert_eq!(count(&draws(1.0)), n as f64, "share 1 equips everybody");
    let third = draws(0.3);
    let p = count(&third) / n as f64;
    let sigma = (0.3_f64 * 0.7 / n as f64).sqrt();
    assert!(
        (p - 0.3).abs() <= 3.0 * sigma,
        "share 0.3 equipped {p:.4} of {n} drivers (3σ = {:.4})",
        3.0 * sigma
    );
    assert_eq!(third, draws(0.3), "the same seed draws the same drivers");
}

/// With congestion re-routing every 10 s, only a driver who re-routes takes the periodic
/// re-plan; with the share at 0 nobody does, and at 1 both do.
#[test]
fn only_drivers_who_reroute_take_the_periodic_replan() {
    let world = grid();
    let (origin, destination) = far_trip(&world);
    let run = |share: f64, forced: Option<[bool; 2]>| -> BTreeSet<ActorId> {
        let rng = RngRegistry::new(5);
        let params = EngineParams {
            reroute_period_s: 10.0,
            reroute_share: share,
            ..EngineParams::default()
        };
        let mut engine = engine_on(&world, params, &rng);
        let ctx = MobilityCtx::new(0, &world, &rng);
        for (k, s) in [10.0, 60.0].into_iter().enumerate() {
            let tr = trip(k as u64, origin, s, destination);
            let mut traits = engine.traits_for(&ctx, &tr);
            if let Some(f) = forced {
                traits.reroutes = f[k];
            }
            engine
                .insert_trip_with(&world, &tr, 0, traits)
                .expect("placed");
        }
        step_for(&mut engine, &world, &rng, 0, 25);
        engine.rerouted_on_information().clone()
    };
    let first = ActorId::new(0);
    let second = ActorId::new(1);
    assert_eq!(
        run(0.5, Some([true, false])),
        BTreeSet::from([first]),
        "only the equipped driver re-planned"
    );
    assert!(run(0.0, None).is_empty(), "share 0: somebody re-planned");
    assert_eq!(
        run(1.0, None),
        BTreeSet::from([first, second]),
        "share 1: both drivers re-plan"
    );
}

/// Under the V2X source a warning re-routes the driver whose unit received it round the
/// blocked lane, and nobody else; under the navigation source the same warning moves
/// nobody.
#[test]
fn under_the_v2x_source_only_the_warned_driver_reroutes() {
    let world = grid();
    let (origin, destination) = far_trip(&world);
    let run = |source: RerouteSource| {
        let rng = RngRegistry::new(9);
        let params = EngineParams {
            reroute_source: source,
            ..EngineParams::default()
        };
        let mut engine = engine_on(&world, params, &rng);
        let a = engine
            .insert_trip(&world, &trip(0, origin, 60.0, destination), 0)
            .expect("placed");
        let b = engine
            .insert_trip(&world, &trip(1, origin, 10.0, destination), 0)
            .expect("placed");
        // A plain driving lane well ahead on the shared route.
        let route = engine.actors[&a].route.lanes.clone();
        assert_eq!(route, engine.actors[&b].route.lanes, "same trip, same route");
        let blocked = *route
            .iter()
            .skip(3)
            .find(|l| world.lane(**l).kind == LaneKind::Driving)
            .expect("a driving lane ahead");
        {
            let mut ctx = MobilityCtx::new(0, &world, &rng);
            engine.command(
                &mut ctx,
                MobilityCommand::HazardHeard {
                    actor: a,
                    lane: blocked,
                },
            );
        }
        step_for(&mut engine, &world, &rng, 0, 1);
        let ahead = |id: ActorId| {
            let x = &engine.actors[&id];
            x.route.lanes[x.route_index..].contains(&blocked)
        };
        (ahead(a), ahead(b), engine.rerouted_on_information().clone(), a)
    };
    let (a_v2x, b_v2x, who, a) = run(RerouteSource::V2x);
    assert!(!a_v2x, "the warned driver still drives the blocked lane");
    assert!(b_v2x, "a driver who heard nothing re-routed");
    assert_eq!(who, BTreeSet::from([a]), "re-planned on information");
    let (a_nav, _, who_nav, _) = run(RerouteSource::Navigation);
    assert!(a_nav, "a warning moved a navigation-app driver");
    assert!(who_nav.is_empty(), "navigation source: {who_nav:?} re-planned");
}

/// A procedural grid has no boundary entries (every lane is fed by the junction it
/// leaves), so its origins stay at rest; and the insertion speed is the free speed
/// capped by the lane end, and by a standing car ahead to the highest speed the
/// car-following model takes without braking harder than b.
#[test]
fn the_insertion_speed_is_the_free_speed_capped_by_the_road_ahead() {
    let world = grid();
    assert!(
        world.roads.lanes().iter().all(|l| !is_boundary_entry(&world, l.id)),
        "a grid lane counted as a map-edge entry"
    );
    let (origin, destination) = far_trip(&world);
    let rng = RngRegistry::new(3);
    let mut engine = engine_on(
        &world,
        EngineParams {
            depart_at_speed: true,
            ..EngineParams::default()
        },
        &rng,
    );
    let lone = engine
        .insert_trip(&world, &trip(0, origin, 20.0, destination), 0)
        .expect("placed");
    assert_eq!(engine.actors[&lone].speed_mps, 0.0, "an interior origin starts at rest");
    let a = engine.actors[&lone].clone();
    let lane = world.lane(origin);
    let free = 11.0_f64.min(lane.speed_limit_mps);
    let b = a.driver.comfort_decel_mps2;
    let stop = (2.0 * b * (lane.length_m - a.s_m - a.driver.min_gap_m)).sqrt();
    let v = engine.insertion_speed(&world, lone);
    assert!(
        (v - math::q3(free.min(stop))).abs() < 1e-9,
        "free road: {v} m/s, expected min({free}, {stop})"
    );
    assert!(v > 5.0, "a free road entry at {v} m/s");
    // A car standing 12 m ahead of a newcomer.
    let mut engine = engine_on(&world, EngineParams::default(), &rng);
    let leader = engine
        .insert_trip(&world, &trip(0, origin, 60.0, destination), 0)
        .expect("leader");
    let rear = engine.actors[&leader].s_m - engine.actors[&leader].class.spec().length_m;
    let follower = engine
        .insert_trip(&world, &trip(1, origin, rear - 12.0, destination), 0)
        .expect("follower");
    let v = engine.insertion_speed(&world, follower);
    assert!(v < free.min(stop) - 1.0, "behind a standing car: {v} m/s");
    let f = engine.actors[&follower].clone();
    let view = LaneView::of(lane);
    let accel = |speed: f64| {
        let mut p = f.clone();
        p.speed_mps = speed;
        engine.cf.accel(
            &p.view(&world),
            Some(&LeaderView::virtual_obstacle(12.0, 0.0)),
            &view,
            &engine.weather,
        )
    };
    assert!(accel(v) >= -b - 1e-2, "at {v} m/s it brakes at {}", accel(v));
    assert!(accel(v + 0.5) < -b, "0.5 m/s faster would still be comfortable");
}
