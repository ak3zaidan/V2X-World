//! Pedestrians and cyclists in dense signalised traffic: the traffic invariants still hold,
//! and no vehicle touches a pedestrian or drives into an occupied crosswalk.
//!
//! The world is the grid of `traffic_invariants.rs` with its crossings switched on, so it
//! carries the pedestrian network (a sidewalk each way along every block face, crosswalks
//! across every arm, and MUTCD pedestrian intervals in every signal plan), and bicycles
//! may ride the carriageway. Pedestrians walk random chains of sidewalks and crosswalks;
//! cyclists ride between random bicycle-admitting lanes.
//!
//! The control runs the same world with `crosswalk_yield` off — drivers ignore people on
//! crosswalks — and must go red on the crosswalk checks. If it ever stops doing so the
//! auditor has gone blind and the zero above means nothing.

use v2xw_core::rng::RngRegistry;
use v2xw_core::time::{Duration, NS_PER_S};
use v2xw_mobility::VehicleClass;
use v2xw_mobility::audit::{AuditParams, AuditReport, Check, TrafficAuditor};
use v2xw_mobility::demand::FleetMix;
use v2xw_mobility::demand::{OdParams, PoissonDemand, PoissonParams};
use v2xw_mobility::engine::VruPopulation;
use v2xw_mobility::vru::{PedActivity, SocialForce, SocialForceParams};
use v2xw_mobility::{EngineParams, Mobility, MobilityCtx, NativeMobility};
use v2xw_world::procedural::GridParams;
use v2xw_world::{ClassMask, ImportOptions, World};

fn world() -> World {
    let params = GridParams {
        cols: 5,
        rows: 6,
        block_x_m: 160.0,
        block_y_m: 80.0,
        lanes_per_direction: 2,
        lane_width_m: 3.5,
        sidewalk_m: 2.0,
        speed_limit_mps: 11.176,
        signalised: true,
        cycle_s: 60.0,
        amber_s: 3.0,
        crossings: true,
        block_buildings: true,
        corner_radius_m: 4.5,
        bicycles_on_roads: true,
        ..GridParams::legacy()
    };
    v2xw_world::procedural::grid(&params, &ImportOptions::default()).expect("grid")
}

fn run(params: EngineParams, seconds: u64) -> AuditReport {
    let world = world();
    let rng = RngRegistry::new(0x5EED_7AFF);
    let mut engine = NativeMobility::new(params).with_vru_population(VruPopulation {
        pedestrians: 160,
        cyclists: 12,
    });
    let demand = PoissonDemand::new(
        &world,
        PoissonParams {
            arrival_rate_per_s: 1.2,
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
    auditor.audit_world(&world);
    let mut t = 0u64;
    while t < seconds * NS_PER_S {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = engine.step(&mut ctx, params.step);
        let actors = engine.audit_actors(&world, update.t);
        let people = engine.audit_pedestrians(&world);
        trace_watched(update.t, &actors);
        auditor.observe_with_pedestrians(&world, t, update.t, &actors, &update.despawned, &people);
        t = update.t;
    }
    auditor.report()
}

/// A diagnostic, off unless `V2XW_TRACE_ACTORS` names actors: each watched vehicle at
/// every step between `V2XW_TRACE_T0` and `V2XW_TRACE_T1` seconds.
fn trace_watched(t: u64, actors: &[v2xw_mobility::audit::AuditActor]) {
    let env = |k: &str| std::env::var(k).ok();
    let Some(watch) = env("V2XW_TRACE_ACTORS") else {
        return;
    };
    let watch: Vec<u32> = watch
        .split(',')
        .filter_map(|x| x.trim().parse().ok())
        .collect();
    let t0: f64 = env("V2XW_TRACE_T0")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    let t1: f64 = env("V2XW_TRACE_T1")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);
    let ts = t as f64 / 1e9;
    if ts < t0 || ts > t1 {
        return;
    }
    for a in actors.iter().filter(|a| watch.contains(&a.actor.index())) {
        eprintln!(
            "t={ts:.1} watch {} {:?} lane {} s {:.3} v {:.3} acc {:.2} lat {:.2} changing {:?} pos ({:.2},{:.2}) hdg {:.3} next {:?}",
            a.actor.index(),
            a.class,
            a.lane.index(),
            a.s_m,
            a.speed_mps,
            a.accel_mps2,
            a.lateral_m,
            a.changing.map(|(f, to)| (f.index(), to.index())),
            a.pos.x,
            a.pos.y,
            a.heading_rad,
            a.route_next.map(|l| l.index())
        );
    }
}

/// Every class that is a collision or a rule broken.
const SAFETY: [Check; 19] = [
    Check::Overlap,
    Check::GapBelowMinimum,
    Check::LateralOffset,
    Check::InBuilding,
    Check::OutsideJunction,
    Check::RedEntry,
    Check::AmberEntry,
    Check::ConflictZone,
    Check::LaneChangeNearJunction,
    Check::QueueJump,
    Check::IllegalTransition,
    Check::Teleport,
    Check::HeadingFlip,
    Check::SpeedJump,
    Check::AccelBound,
    Check::MidRoadDespawn,
    Check::PedestrianOverlap,
    Check::OccupiedCrosswalkEntry,
    Check::PedestrianDontWalkEntry,
];

#[test]
fn pedestrians_and_cyclists_in_signalised_traffic_hold_every_invariant() {
    let report = run(EngineParams::default(), 180);
    eprintln!("{:?}\n{:?}", report.stats, report.counts);
    let failing: Vec<String> = SAFETY
        .iter()
        .filter(|c| report.count(**c) > 0)
        .map(|c| format!("{} = {}", c.label(), report.count(*c)))
        .collect();
    let examples: Vec<&v2xw_mobility::audit::Example> = report
        .examples
        .iter()
        .filter(|e| SAFETY.contains(&e.check))
        .take(12)
        .collect();
    assert!(
        failing.is_empty(),
        "invariants violated: {failing:?}\nexamples: {examples:#?}"
    );
    // A real run: people walked, and a real share of that walking was on crosswalks.
    assert!(
        report.stats.pedestrian_steps > 100_000,
        "{:?}",
        report.stats
    );
    assert!(
        report.stats.pedestrian_crossing_steps > 2_000,
        "hardly anyone crossed a street: {:?}",
        report.stats
    );
    assert!(report.stats.signalised_entries >= 100, "{:?}", report.stats);
    assert_eq!(report.count(Check::Standstill), 0, "{:?}", report.stats);
}

#[test]
fn the_auditor_sees_drivers_who_ignore_people_on_crosswalks() {
    let params = EngineParams {
        crosswalk_yield: false,
        ..EngineParams::default()
    };
    let report = run(params, 120);
    eprintln!("control: {:?}", report.counts);
    assert!(
        report.count(Check::OccupiedCrosswalkEntry) > 0,
        "no occupied-crosswalk entries when drivers ignore pedestrians: {:?}",
        report.counts
    );
}

/// Pedestrians who cross against the signal are what the don't-walk check exists for:
/// switched fully on, the check must see them.
#[test]
fn the_auditor_sees_pedestrians_who_cross_on_dont_walk() {
    let world = world();
    let rng = RngRegistry::new(0x5EED_7AFF);
    let reckless = v2xw_mobility::vru::SocialForceParams {
        jaywalk_probability: 1.0,
        ..v2xw_mobility::vru::SocialForceParams::default()
    };
    let mut engine = NativeMobility::new(EngineParams::default())
        .with_vru(v2xw_mobility::vru::SocialForce::new(reckless))
        .with_vru_population(VruPopulation {
            pedestrians: 160,
            cyclists: 0,
        });
    let demand = PoissonDemand::new(
        &world,
        PoissonParams {
            arrival_rate_per_s: 0.2,
            duration: Duration::from_secs(60),
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
    let mut t = 0u64;
    while t < 60 * NS_PER_S {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = engine.step(&mut ctx, Duration::from_millis(100));
        let actors = engine.audit_actors(&world, update.t);
        let people = engine.audit_pedestrians(&world);
        auditor.observe_with_pedestrians(&world, t, update.t, &actors, &update.despawned, &people);
        t = update.t;
    }
    let report = auditor.report();
    // The model chose those crossings (`PedActivity::CrossingAgainstSignal`): observed
    // behaviour, counted, not a defect.
    assert!(
        report.stats.pedestrian_red_crossings > 0,
        "{:?}",
        report.stats
    );
    assert_eq!(
        report.count(Check::PedestrianDontWalkEntry),
        0,
        "{:?}",
        report.counts
    );
    // Crossing against the signal never means walking into a car.
    assert_eq!(report.count(Check::PedestrianOverlap), 0);
}

/// The fault the don't-walk check exists for: a pedestrian on a crosswalk on don't-walk
/// that the model did not decide to put there. The same run as above, with every
/// against-the-signal label stripped before the auditor sees it — as a model that walked
/// people out on red by mistake would report them — must go red.
#[test]
fn the_auditor_sees_a_pedestrian_on_dont_walk_the_model_did_not_choose() {
    let world = world();
    let rng = RngRegistry::new(0x5EED_7AFF);
    let reckless = v2xw_mobility::vru::SocialForceParams {
        jaywalk_probability: 1.0,
        ..v2xw_mobility::vru::SocialForceParams::default()
    };
    let mut engine = NativeMobility::new(EngineParams::default())
        .with_vru(v2xw_mobility::vru::SocialForce::new(reckless))
        .with_vru_population(VruPopulation {
            pedestrians: 160,
            cyclists: 0,
        });
    let demand = PoissonDemand::new(
        &world,
        PoissonParams {
            arrival_rate_per_s: 0.2,
            duration: Duration::from_secs(60),
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
    let mut t = 0u64;
    while t < 60 * NS_PER_S {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = engine.step(&mut ctx, Duration::from_millis(100));
        let actors = engine.audit_actors(&world, update.t);
        let mut people = engine.audit_pedestrians(&world);
        for p in &mut people {
            if p.activity == PedActivity::CrossingAgainstSignal {
                p.activity = PedActivity::CrossingOnWalk;
            }
        }
        auditor.observe_with_pedestrians(&world, t, update.t, &actors, &update.despawned, &people);
        t = update.t;
    }
    let report = auditor.report();
    assert!(
        report.count(Check::PedestrianDontWalkEntry) > 0,
        "{:?}",
        report.counts
    );
}

/// Runs the grid with pedestrians as observed — speeds by age, start-up, signal
/// violators who wait for a gap, groups, mid-block crossing at a high rate so a short run
/// sees plenty — and a fleet with motorcycles and mopeds among the cars.
fn observed_run(params: EngineParams, seconds: u64) -> AuditReport {
    let world = world();
    let rng = RngRegistry::new(0x0B5E_57ED);
    let mut people = SocialForceParams::observed();
    people.midblock.rate_per_100m = 3.0;
    let mut engine = NativeMobility::new(params)
        .with_vru(SocialForce::new(people))
        .with_vru_population(VruPopulation {
            pedestrians: 220,
            cyclists: 16,
        });
    let demand = PoissonDemand::new(
        &world,
        PoissonParams {
            arrival_rate_per_s: 1.2,
            duration: Duration::from_secs(seconds),
            fleet: FleetMix::from_shares(&[
                (VehicleClass::Passenger, 0.8),
                (VehicleClass::Motorcycle, 0.1),
                (VehicleClass::Moped, 0.1),
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
    auditor.audit_world(&world);
    let mut t = 0u64;
    let mut moto_steps = 0u64;
    while t < seconds * NS_PER_S {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = engine.step(&mut ctx, params.step);
        let actors = engine.audit_actors(&world, update.t);
        moto_steps += actors
            .iter()
            .filter(|a| matches!(a.class, VehicleClass::Motorcycle | VehicleClass::Moped))
            .count() as u64;
        let people = engine.audit_pedestrians(&world);
        trace_watched(update.t, &actors);
        auditor.observe_with_pedestrians(&world, t, update.t, &actors, &update.despawned, &people);
        t = update.t;
    }
    let stats = engine.vru().map(|v| v.stats()).unwrap_or_default();
    eprintln!("pedestrian stats: {stats:?}; two-wheeler steps {moto_steps}");
    assert!(moto_steps > 1_000, "hardly any two-wheelers: {moto_steps}");
    auditor.report()
}

/// Jaywalkers, signal violators, groups, motorcycles and mopeds: every safety invariant
/// still holds — no vehicle touches a pedestrian whatever the pedestrian is doing — and
/// the behaviours really happened.
#[test]
fn observed_pedestrians_and_two_wheelers_hold_every_invariant() {
    let report = observed_run(EngineParams::default(), 180);
    eprintln!("{:?}\n{:?}", report.stats, report.counts);
    let failing: Vec<String> = SAFETY
        .iter()
        .filter(|c| report.count(**c) > 0)
        .map(|c| format!("{} = {}", c.label(), report.count(*c)))
        .collect();
    let examples: Vec<&v2xw_mobility::audit::Example> = report
        .examples
        .iter()
        .filter(|e| SAFETY.contains(&e.check))
        .take(12)
        .collect();
    assert!(
        failing.is_empty(),
        "invariants violated: {failing:?}\nexamples: {examples:#?}"
    );
    assert!(
        report.stats.pedestrian_midblock_steps > 500,
        "hardly anyone crossed mid-block: {:?}",
        report.stats
    );
    assert!(
        report.stats.pedestrian_red_crossings > 0,
        "nobody crossed against the signal: {:?}",
        report.stats
    );
    assert_eq!(report.count(Check::Standstill), 0, "{:?}", report.stats);
}

/// The kinematic limits ([`EngineParams::lateral_speed_bound`],
/// [`EngineParams::braking_tyre_limit`]): a lane change moves sideways no faster than its
/// speed along the road lets it steer, and no stop brakes harder than the tyres can. Both
/// runs above, with the two on, hold every safety invariant. Both are on by default since
/// fix round 5 (2026-10-28), so these two now repeat the default tests above by name; off,
/// the runs were red on `in-building` (a car that braked mid-change swung 62° off its lane
/// into a block, actor 301; one released from a standstill hold slid 2 m sideways in 2 s,
/// actor 253) and on `speed-jump`/`accel-bound` (−12.7 m/s² for a pedestrian stepping
/// out, actor 255).
fn safety_failures(report: &AuditReport) -> Vec<String> {
    SAFETY
        .iter()
        .filter(|c| report.count(**c) > 0)
        .map(|c| format!("{} = {}", c.label(), report.count(*c)))
        .collect()
}

fn kinematic_limits() -> EngineParams {
    EngineParams {
        lateral_speed_bound: true,
        braking_tyre_limit: true,
        ..EngineParams::default()
    }
}

#[test]
fn with_the_kinematic_limits_signalised_traffic_holds_every_invariant() {
    let report = run(kinematic_limits(), 180);
    eprintln!("{:?}\n{:?}", report.stats, report.counts);
    assert!(
        safety_failures(&report).is_empty(),
        "invariants violated: {:?}",
        safety_failures(&report)
    );
    assert!(
        report.stats.pedestrian_steps > 100_000,
        "{:?}",
        report.stats
    );
    assert!(report.stats.signalised_entries >= 100, "{:?}", report.stats);
    assert_eq!(report.count(Check::Standstill), 0, "{:?}", report.stats);
}

#[test]
fn with_the_kinematic_limits_the_observed_run_holds_every_invariant() {
    let report = observed_run(kinematic_limits(), 180);
    eprintln!("{:?}\n{:?}", report.stats, report.counts);
    assert!(
        safety_failures(&report).is_empty(),
        "invariants violated: {:?}",
        safety_failures(&report)
    );
    assert!(
        report.stats.pedestrian_midblock_steps > 500,
        "{:?}",
        report.stats
    );
    assert!(
        report.stats.pedestrian_red_crossings > 0,
        "{:?}",
        report.stats
    );
    assert_eq!(report.count(Check::Standstill), 0, "{:?}", report.stats);
}

/// A diagnostic, ignored: the observed run with the amber-commitment lapse off, its counts
/// printed for comparison.
#[test]
#[ignore = "a diagnostic comparison, not a gate"]
fn observed_run_without_the_amber_lapse() {
    let report = observed_run(
        EngineParams {
            amber_commit_lapse: false,
            ..EngineParams::default()
        },
        180,
    );
    eprintln!("no lapse: {:?}", report.counts);
    for e in report.examples.iter().take(8) {
        eprintln!(
            "  {:?} t={} {:?}->{:?}: {}",
            e.check, e.t_s, e.actor, e.other, e.detail
        );
    }
}

/// The control: drivers who ignore pedestrians — on crosswalks and in the carriageway
/// mid-block — must make the auditor count collisions with them.
#[test]
fn the_auditor_sees_drivers_who_ignore_jaywalkers() {
    let params = EngineParams {
        crosswalk_yield: false,
        ..EngineParams::default()
    };
    let report = observed_run(params, 120);
    eprintln!("control: {:?}", report.counts);
    assert!(
        report.count(Check::PedestrianOverlap) > 0,
        "no pedestrian overlaps when drivers ignore people in the road: {:?}",
        report.counts
    );
}

/// The grid used above really is the one with the pedestrian network, and bicycles may
/// use its carriageway.
#[test]
fn the_world_has_a_walk_network_and_bicycle_lanes() {
    let w = world();
    let lanes = w.roads.lanes();
    assert!(
        lanes
            .iter()
            .any(|l| l.kind == v2xw_world::LaneKind::Crossing)
    );
    assert!(
        lanes
            .iter()
            .any(|l| l.kind == v2xw_world::LaneKind::Sidewalk)
    );
    assert!(
        lanes
            .iter()
            .filter(|l| l.kind == v2xw_world::LaneKind::Driving)
            .all(|l| l.admits(ClassMask::BICYCLE))
    );
}

/// A diagnostic, ignored: the observed run's state around one recorded overlap — each
/// pedestrian in the carriageway within 8 m of a vehicle, and that vehicle, between
/// `V2XW_TRACE_T0` and `V2XW_TRACE_T1` seconds (environment; defaults 41.0 and 42.1).
#[test]
#[ignore = "a diagnostic trace, not a gate"]
fn trace_the_observed_run() {
    let t0: f64 = std::env::var("V2XW_TRACE_T0")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(41.0);
    let t1: f64 = std::env::var("V2XW_TRACE_T1")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(42.1);
    let watch: Vec<u32> = std::env::var("V2XW_TRACE_ACTORS")
        .ok()
        .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_default();
    let world = world();
    let rng = RngRegistry::new(0x0B5E_57ED);
    let mut people = SocialForceParams::observed();
    people.midblock.rate_per_100m = 3.0;
    let params = EngineParams::default();
    let mut engine = NativeMobility::new(params)
        .with_vru(SocialForce::new(people))
        .with_vru_population(VruPopulation {
            pedestrians: 220,
            cyclists: 16,
        });
    let demand = PoissonDemand::new(
        &world,
        PoissonParams {
            arrival_rate_per_s: 1.2,
            duration: Duration::from_secs(180),
            fleet: FleetMix::from_shares(&[
                (VehicleClass::Passenger, 0.8),
                (VehicleClass::Motorcycle, 0.1),
                (VehicleClass::Moped, 0.1),
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
    let mut t = 0u64;
    while (t as f64) / 1e9 < t1 {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = engine.step(&mut ctx, params.step);
        let ts = update.t as f64 / 1e9;
        if ts >= t0 {
            let actors = engine.audit_actors(&world, update.t);
            for a in actors.iter().filter(|a| watch.contains(&a.actor.index())) {
                eprintln!(
                    "t={ts:.1} watch {} {:?} lane {} s {:.3} v {:.3} acc {:.2} changing {:?} pos ({:.2},{:.2}) hdg {:.3} prev {:?} next {:?}",
                    a.actor.index(),
                    a.class,
                    a.lane.index(),
                    a.s_m,
                    a.speed_mps,
                    a.accel_mps2,
                    a.changing.map(|(f, to)| (f.index(), to.index())),
                    a.pos.x,
                    a.pos.y,
                    a.heading_rad,
                    a.prev_lane.map(|l| l.index()),
                    a.route_next.map(|l| l.index())
                );
            }
            for sp in &update.spawned {
                if watch.contains(&sp.actor.index()) {
                    eprintln!("t={ts:.1} spawned {} {:?}", sp.actor.index(), sp.class);
                }
            }
            let vru = engine.vru().expect("pedestrians");
            for p in vru.people() {
                let Some(m) = p.midblock.as_ref() else {
                    continue;
                };
                let pos = p.position(&world);
                let near: Vec<&v2xw_mobility::audit::AuditActor> = actors
                    .iter()
                    .filter(|a| a.pos.distance_2d(pos) < 8.0)
                    .collect();
                if near.is_empty() {
                    continue;
                }
                eprintln!(
                    "t={ts:.1} ped {} {:?} stage {:?} pos ({:.2},{:.2}) bands {:?}",
                    p.actor.index(),
                    p.activity,
                    m.stage,
                    pos.x,
                    pos.y,
                    m.bands
                        .iter()
                        .map(|b| (
                            b.conflict.lane.index(),
                            (b.conflict.s_m * 100.0).round() / 100.0,
                            (b.conflict.half_extent_m * 100.0).round() / 100.0,
                            (b.d_near_m * 100.0).round() / 100.0,
                            (b.d_far_m * 100.0).round() / 100.0
                        ))
                        .collect::<Vec<_>>()
                );
                for a in near {
                    eprintln!(
                        "      veh {} {:?} lane {} s {:.2} v {:.2} pos ({:.2},{:.2}) len {:.1} changing {:?} next {:?}",
                        a.actor.index(),
                        a.class,
                        a.lane.index(),
                        a.s_m,
                        a.speed_mps,
                        a.pos.x,
                        a.pos.y,
                        a.length_m,
                        a.changing.map(|(f, to)| (f.index(), to.index())),
                        a.route_next.map(|l| l.index())
                    );
                }
            }
        }
        t = update.t;
    }
}

/// A diagnostic, ignored: the lanes named in `V2XW_LANES` (comma-separated ids) — kind,
/// junction, ends, length, the connection each lane carries — and each internal lane's
/// foes in its junction's conflict matrix.
#[test]
#[ignore = "a diagnostic, not a gate"]
fn describe_lanes() {
    let world = world();
    let ids: Vec<u32> = std::env::var("V2XW_LANES")
        .ok()
        .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_default();
    let roads = &world.roads;
    for id in &ids {
        let lane = roads
            .lanes()
            .iter()
            .find(|l| l.id.index() == *id)
            .expect("lane");
        let (a, b) = (lane.centreline[0], *lane.centreline.last().expect("pts"));
        let conns: Vec<_> = roads
            .connections()
            .iter()
            .filter(|c| c.via == Some(lane.id) || c.from_lane == lane.id)
            .map(|c| {
                (
                    c.from_lane.index(),
                    c.to_lane.index(),
                    c.via.map(|v| v.index()),
                    c.direction,
                )
            })
            .collect();
        eprintln!(
            "lane {id} {:?} junction {:?} index {} from ({:.2},{:.2}) to ({:.2},{:.2}) len {:.2} conns {:?}",
            lane.kind,
            lane.junction.map(|j| j.index()),
            lane.index,
            a.x,
            a.y,
            b.x,
            b.y,
            lane.length_m,
            conns
        );
        if let Some(j) = lane.junction {
            let junction = roads.junction(j);
            if let Some(me) = junction.internal.iter().position(|l| *l == lane.id) {
                let foes: Vec<u32> = junction
                    .conflicts
                    .foes_of(me)
                    .iter()
                    .map(|i| junction.internal[*i].index())
                    .collect();
                eprintln!("    foes {foes:?}");
            }
        }
    }
}
