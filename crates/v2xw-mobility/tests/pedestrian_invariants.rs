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
use v2xw_mobility::audit::{AuditParams, AuditReport, Check, TrafficAuditor};
use v2xw_mobility::demand::{OdParams, PoissonDemand, PoissonParams};
use v2xw_mobility::demand::FleetMix;
use v2xw_mobility::engine::VruPopulation;
use v2xw_mobility::vru::{PedActivity, SocialForce, SocialForceParams};
use v2xw_mobility::VehicleClass;
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
        auditor.observe_with_pedestrians(&world, t, update.t, &actors, &update.despawned, &people);
        t = update.t;
    }
    auditor.report()
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
    assert_eq!(report.count(Check::PedestrianDontWalkEntry), 0, "{:?}", report.counts);
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
        eprintln!("  {:?} t={} {:?}->{:?}: {}", e.check, e.t_s, e.actor, e.other, e.detail);
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
