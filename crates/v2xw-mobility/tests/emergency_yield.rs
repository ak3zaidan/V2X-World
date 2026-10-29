//! WP-20 P20.5: traffic yields to an emergency vehicle, measured on a congested arterial.
//!
//! An ambulance crosses a 1 km signalised arterial (five 200 m blocks, two lanes each way,
//! 60 s cycle) through Poisson traffic, under three seeds, with
//! `traffic.emergency_yield` off and on. With yielding on, drivers ahead of it on its
//! carriageway move right and stop (NY VTL §1144(a)) and it passes reds after stopping
//! once the junction is clear (UVC §11-106(b)2); off, it is a vehicle like any other.
//! The engine's own auditor watches every run: no civilian may leave its lane
//! (`lateral-offset`), and no vehicles may touch.

use v2xw_core::ids::{ActorId, LaneId};
use v2xw_core::rng::RngRegistry;
use v2xw_core::time::{Duration, NS_PER_S};
use v2xw_mobility::audit::{AuditParams, AuditReport, Check, TrafficAuditor};
use v2xw_mobility::demand::{FleetMix, OdParams, PoissonDemand, PoissonParams};
use v2xw_mobility::{
    EmergencyYieldParams, EngineParams, Mobility, MobilityCtx, NativeMobility, VehicleClass,
};
use v2xw_world::procedural::GridParams;
use v2xw_world::{ImportOptions, LaneKind, TurnDirection, World};

/// Six avenues 200 m apart and three streets: the middle street is a 1 km arterial.
fn arterial() -> World {
    let params = GridParams {
        cols: 6,
        rows: 3,
        block_x_m: 200.0,
        block_y_m: 120.0,
        lanes_per_direction: 2,
        lane_width_m: 3.5,
        sidewalk_m: 2.0,
        speed_limit_mps: 13.4,
        signalised: true,
        cycle_s: 60.0,
        amber_s: 3.0,
        ..GridParams::legacy()
    };
    v2xw_world::procedural::grid(&params, &ImportOptions::default()).expect("grid")
}

/// The eastbound left lane of the middle street, straight on from its west end to its
/// east end: the ambulance's route (approach and departure lanes; the engine takes the
/// connectors between them).
fn corridor(world: &World) -> Vec<LaneId> {
    let ys: Vec<f64> = world
        .roads
        .lanes()
        .iter()
        .filter(|l| l.kind == LaneKind::Driving)
        .map(|l| l.start().y)
        .collect();
    let mid = 0.5 * (ys.iter().cloned().fold(f64::INFINITY, f64::min)
        + ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    let first = world
        .roads
        .lanes()
        .iter()
        .filter(|l| {
            l.kind == LaneKind::Driving && l.heading_at(0.0).abs() < 0.1 && l.index == 1
        })
        .min_by(|a, b| {
            let key = |l: &v2xw_world::Lane| (l.start().y - mid).abs() * 1e6 + l.start().x;
            key(a).total_cmp(&key(b))
        })
        .expect("an eastbound left lane");
    let mut route = vec![first.id];
    let mut lane = first.id;
    // Straight on, keeping to the left lane where the junction allows it: an emergency
    // vehicle keeps left of the traffic pulling right.
    while let Some(c) = {
        let straight: Vec<_> = world
            .successors(lane)
            .iter()
            .filter(|c| c.permitted && c.direction == TurnDirection::Straight && c.via.is_some())
            .collect();
        straight
            .iter()
            .find(|c| world.lane(c.to_lane).index == 1)
            .or(straight.first())
            .copied()
    } {
        route.push(c.to_lane);
        lane = c.to_lane;
    }
    route
}

/// One run: Poisson traffic for 120 s, the ambulance put on the corridor at 60 s. Its
/// travel time from then to the corridor's end (or `None` if it did not arrive in 240 s),
/// and the audit of the whole run.
fn run(seed: u64, yielding: bool) -> (Option<f64>, AuditReport, f64) {
    let world = arterial();
    let route = corridor(&world);
    let length_m: f64 = route.iter().map(|l| world.lane(*l).length_m).sum();
    assert!(length_m > 900.0, "the corridor is {length_m:.0} m");
    let rng = RngRegistry::new(seed);
    let params = EngineParams {
        emergency_yield: EmergencyYieldParams {
            enabled: yielding,
            ..EmergencyYieldParams::default()
        },
        ..EngineParams::default()
    };
    let mut engine = NativeMobility::new(params);
    let demand = PoissonDemand::new(
        &world,
        PoissonParams {
            arrival_rate_per_s: 1.5,
            duration: Duration::from_secs(120),
            fleet: FleetMix::from_shares(&[
                (VehicleClass::Passenger, 0.85),
                (VehicleClass::Delivery, 0.10),
                (VehicleClass::Bus, 0.05),
            ]),
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
    let spawn_t = 60 * NS_PER_S;
    let end_t = spawn_t + 240 * NS_PER_S;
    let last = *route.last().expect("a route");
    let mut ev: Option<ActorId> = None;
    let mut arrived: Option<f64> = None;
    let mut t = 0u64;
    while t < end_t && arrived.is_none() {
        if t == spawn_t {
            let driver = v2xw_mobility::carfollowing::idm::IdmPreset::UrbanHcm
                .profile(VehicleClass::Emergency);
            let id = engine
                .spawn_with_route(&world, t, VehicleClass::Emergency, driver, route.clone(), 5.0)
                .expect("the ambulance");
            ev = Some(id);
        }
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = engine.step(&mut ctx, params.step);
        let actors = engine.audit_actors(&world, update.t);
        auditor.observe(&world, t, update.t, &actors, &update.despawned);
        if let Some(id) = ev
            && std::env::var("V2XW_EVDBG").is_ok()
            && update.t % NS_PER_S == 0
            && let Some(e) = actors.iter().find(|a| a.actor == id)
        {
            let ahead = actors
                .iter()
                .filter(|a| a.lane == e.lane && a.s_m > e.s_m)
                .min_by(|a, b| a.s_m.total_cmp(&b.s_m));
            eprintln!(
                "DBG y{} t{} ev lane {} idx {} s {:.1}/{:.0} v {:.1} chg {:?} | ahead {:?}",
                yielding as u8,
                (update.t - spawn_t) / NS_PER_S,
                e.lane.index(),
                world.lane(e.lane).index,
                e.s_m,
                world.lane(e.lane).length_m,
                e.speed_mps,
                e.changing.is_some(),
                ahead.map(|a| (a.actor.index(), a.class, (a.s_m - e.s_m) as i32, (a.speed_mps * 10.0).round() / 10.0, a.changing.is_some()))
            );
        }
        if let Some(id) = ev {
            let done = update.despawned.iter().any(|(a, _)| *a == id)
                || actors.iter().any(|a| {
                    a.actor == id && a.lane == last && a.s_m >= world.lane(last).length_m - 1.0
                });
            if done {
                arrived = Some((update.t - spawn_t) as f64 / 1e9);
            }
        }
        t = update.t;
    }
    (arrived, auditor.report(), length_m)
}

#[test]
fn corridor_time_falls_with_yielding() {
    let mut off_sum = 0.0;
    let mut on_sum = 0.0;
    let seeds: Vec<u64> = std::env::var("V2XW_SEEDS").ok().map_or(vec![11, 12, 13], |v| v.split(',').filter_map(|x| x.parse().ok()).collect());
    for seed in seeds {
        let (off, off_audit, len) = run(seed, false);
        let (on, on_audit, _) = run(seed, true);
        let off = off.unwrap_or(240.0);
        let on = on.expect("with yielding the ambulance never arrived");
        eprintln!(
            "seed {seed}: {len:.0} m in {off:.1} s off, {on:.1} s on; on audit {:?}",
            on_audit.counts
        );
        assert!(on < off, "seed {seed}: {on:.1} s on, {off:.1} s off");
        // No civilian leaves the carriageway, nobody touches, no untagged red entry.
        for c in [
            Check::LateralOffset,
            Check::Overlap,
            Check::RedEntry,
            Check::ConflictZone,
        ] {
            assert!(
                on_audit.count(c) <= off_audit.count(c),
                "seed {seed}: {} {} on, {} off",
                c.label(),
                on_audit.count(c),
                off_audit.count(c)
            );
        }
        assert_eq!(on_audit.count(Check::LateralOffset), 0);
        assert_eq!(on_audit.count(Check::RedEntry), 0, "{:?}", on_audit.examples);
        off_sum += off;
        on_sum += on;
    }
    let change = on_sum / off_sum - 1.0;
    eprintln!("mean change {:.1} %", 100.0 * change);
    assert!(change <= -0.15, "the mean fell only {:.1} %", 100.0 * change);
}

#[test]
fn the_corridor_run_replays_exactly() {
    let (a, ra, _) = run(12, true);
    let (b, rb, _) = run(12, true);
    assert_eq!(a.map(f64::to_bits), b.map(f64::to_bits));
    assert_eq!(ra.counts, rb.counts);
}
