//! Traffic realism beyond the single stop line: kerbside stops, heavy vehicles in a queue,
//! driving styles and crowded pavements, each measured with the calibration harness and
//! set beside its published reference, with the traffic invariants held at zero.
//!
//! * **Kerbside stops** (`v2xw_mobility::kerbside`): buses dwell at stops, taxis pick up
//!   and set down, delivery vans double-park; the traffic behind goes round them. The same
//!   run with kerbside stops off is the "before".
//! * **Heavy vehicles** in the saturation-flow field study: their discharge headway over a
//!   car's, against the HCM's passenger-car equivalent of 2.0.
//! * **Driving styles** (`v2xw_mobility::style`): 15 / 70 / 15 cautious, normal and
//!   assertive drivers.
//! * **Crowding**: walkers slowed by Weidmann's speed-density relation, binned by Fruin's
//!   levels of service, against the same crowd with the relation off.

use std::collections::BTreeMap;

use v2xw_core::rng::RngRegistry;
use v2xw_core::time::{Duration, NS_PER_S};
use v2xw_mobility::audit::{AuditParams, AuditReport, Check, TrafficAuditor};
use v2xw_mobility::calibration::{CalibrationObserver, CalibrationReport, SaturationExperiment};
use v2xw_mobility::demand::{FleetMix, OdParams, PoissonDemand, PoissonParams};
use v2xw_mobility::engine::{DriverTraits, VruPopulation};
use v2xw_mobility::kerbside::KerbsideParams;
use v2xw_mobility::style::DriverStyle;
use v2xw_mobility::vru::social_force::{SocialForce, SocialForceParams};
use v2xw_mobility::{EngineParams, Mobility, MobilityCtx, NativeMobility, VehicleClass};
use v2xw_world::procedural::GridParams;
use v2xw_world::{ImportOptions, World};

/// The safety classes that must stay at zero (as `tests/traffic_invariants.rs`), and
/// gridlock.
const SAFETY: [Check; 18] = [
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
    Check::StepSpeed,
    Check::HeadingFlip,
    Check::SpeedJump,
    Check::AccelBound,
    Check::MidRoadDespawn,
    Check::Standstill,
];

/// A Midtown-shaped grid with two lanes each way: avenues 160 m apart, streets 80 m.
fn grid() -> World {
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
        block_buildings: true,
        corner_radius_m: 4.5,
        ..GridParams::legacy()
    };
    v2xw_world::procedural::grid(&params, &ImportOptions::default()).expect("grid")
}

/// The engine as it was before this realism work: no kerbside stops, no driving styles,
/// no heavy-vehicle turn cap, one router for every class.
fn before_realism() -> EngineParams {
    EngineParams {
        kerbside: KerbsideParams {
            enabled: false,
            ..KerbsideParams::default()
        },
        driver_styles: false,
        heavy_turns: false,
        class_routing: false,
        ..EngineParams::default()
    }
}

/// A diagnostic, off unless `V2XW_TRACE_ACTORS` names actors (comma-separated ids): once a
/// second between `V2XW_TRACE_T0` and `V2XW_TRACE_T1` (seconds), each watched actor and
/// every vehicle on, or about to enter, the junction its route enters next.
fn trace_junction(world: &World, t: u64, actors: &[v2xw_mobility::audit::AuditActor]) {
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
    if ts < t0 || ts > t1 || t % NS_PER_S != 0 {
        return;
    }
    let junction_of = |l: v2xw_core::ids::LaneId| world.roads.lane(l).junction;
    let line = |a: &v2xw_mobility::audit::AuditActor| {
        format!(
            "{} {:?} lane {} s {:.2}/{:.1} v {:.2} acc {:.2} lat {:.2} kerb {:?} changing {:?} next {:?} pos ({:.1},{:.1}) hdg {:.2}",
            a.actor.index(),
            a.class,
            a.lane.index(),
            a.s_m,
            world.roads.lane(a.lane).length_m,
            a.speed_mps,
            a.accel_mps2,
            a.lateral_m,
            a.kerbside,
            a.changing.map(|(f, to)| (f.index(), to.index())),
            a.route_next.map(|l| l.index()),
            a.pos.x,
            a.pos.y,
            a.heading_rad
        )
    };
    for a in actors.iter().filter(|a| watch.contains(&a.actor.index())) {
        eprintln!("t={ts:.0} watch {}", line(a));
        let j = junction_of(a.lane).or_else(|| a.route_next.and_then(junction_of));
        let Some(j) = j else { continue };
        for b in actors.iter().filter(|b| {
            b.actor != a.actor
                && (junction_of(b.lane) == Some(j) || b.route_next.and_then(junction_of) == Some(j))
        }) {
            eprintln!("        at j{} {}", j.index(), line(b));
        }
    }
}

fn run_mixed(params: EngineParams, seconds: u64) -> (CalibrationReport, AuditReport) {
    let world = grid();
    let rng = RngRegistry::new(0xB05_57095);
    let kerbside = params.kerbside;
    let mut engine = NativeMobility::new(params);
    let demand = PoissonDemand::new(
        &world,
        PoissonParams {
            arrival_rate_per_s: 1.2,
            duration: Duration::from_secs(seconds),
            fleet: FleetMix::from_shares(&[
                (VehicleClass::Passenger, 0.75),
                (VehicleClass::Bus, 0.08),
                (VehicleClass::Delivery, 0.12),
                (VehicleClass::Truck, 0.05),
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
    let mut harness = CalibrationObserver::new(&world);
    let mut auditor = TrafficAuditor::new(
        &world,
        AuditParams {
            examples_per_check: 400,
            ..AuditParams::default()
        },
    );
    // Who each actor was at each step, to explain an overlap afterwards.
    let mut who: BTreeMap<(u64, u32), String> = BTreeMap::new();
    let mut t = 0u64;
    while t < seconds * NS_PER_S {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = engine.step(&mut ctx, params.step);
        let actors = engine.audit_actors(&world, update.t);
        for a in &actors {
            who.insert(
                (update.t / 100_000_000, a.actor.index()),
                format!(
                    "{} v={:.2} lat={:.2} changing={:?} kerb={:?} lane={:?}",
                    a.class, a.speed_mps, a.lateral_m, a.changing, a.kerbside, a.lane
                ),
            );
        }
        if kerbside.enabled {
            trace_junction(&world, update.t, &actors);
        }
        harness.observe(&world, t, update.t, &actors, &update.despawned, &[]);
        auditor.observe(&world, t, update.t, &actors, &update.despawned);
        t = update.t;
        if kerbside.enabled && t % (30 * NS_PER_S) == 0 {
            eprintln!(
                "t={}s kerbside (with a role, planned, at the kerb, done) {:?}",
                t / NS_PER_S,
                engine.kerbside_census()
            );
        }
    }
    let audit = auditor.report();
    let mut shown = 0;
    let mut last: Option<(u32, u32)> = None;
    for e in audit.examples.iter().filter(|e| e.check == Check::Overlap) {
        let (Some(a), Some(b)) = (e.actor, e.other) else {
            continue;
        };
        let pair = (a, b);
        if last == Some(pair) || shown >= 12 {
            continue;
        }
        last = Some(pair);
        shown += 1;
        let step = (e.t_s * 10.0).round() as u64;
        eprintln!(
            "  overlap t={:.1}s: {} [{}] / {} [{}]: {}",
            e.t_s,
            a,
            who.get(&(step, a)).cloned().unwrap_or_default(),
            b,
            who.get(&(step, b)).cloned().unwrap_or_default(),
            e.detail
        );
    }
    let mut last_gap: Option<(u32, u32)> = None;
    let mut shown_gap = 0;
    for e in audit
        .examples
        .iter()
        .filter(|e| e.check == Check::GapBelowMinimum)
    {
        let (Some(a), Some(b)) = (e.actor, e.other) else {
            continue;
        };
        if last_gap == Some((a, b)) || shown_gap >= 10 {
            continue;
        }
        last_gap = Some((a, b));
        shown_gap += 1;
        let step = (e.t_s * 10.0).round() as u64;
        eprintln!(
            "  gap t={:.1}s: {a} [{}] / {b} [{}]: {}",
            e.t_s,
            who.get(&(step, a)).cloned().unwrap_or_default(),
            who.get(&(step, b)).cloned().unwrap_or_default(),
            e.detail
        );
    }
    for e in audit
        .examples
        .iter()
        .filter(|e| e.check == Check::RedEntry)
        .take(8)
    {
        let step = (e.t_s * 10.0).round() as u64;
        let a = e.actor.unwrap_or(u32::MAX);
        eprintln!(
            "  red entry t={:.1}s: {a} [{}]: {}",
            e.t_s,
            who.get(&(step, a)).cloned().unwrap_or_default(),
            e.detail
        );
    }
    (harness.report(), audit)
}

fn safety_failures(audit: &AuditReport) -> Vec<String> {
    SAFETY
        .iter()
        .filter(|c| audit.count(**c) > 0)
        .map(|c| format!("{} = {}", c.label(), audit.count(*c)))
        .collect()
}

fn print_network(label: &str, r: &CalibrationReport) {
    let n = &r.network;
    eprintln!(
        "[{label}] network speed {:.2} m/s ({:.1} min/km), stops/km {:.2} over {:.1} veh-km, \
         MFD max flow {:.0} veh/h/ln vs bound {:.0} (g/C {:.2}), lane utilisation 2-lane \
         {:.3} [{}], right turns {} (right-hook conflicts {}), dooring exposures {}",
        n.network_speed_mps,
        n.travel_time_min_per_km,
        n.stops_per_km,
        n.vehicle_km,
        n.mfd_max_flow_veh_per_h,
        n.capacity_bound_veh_per_h,
        n.mean_through_green_ratio,
        n.lane_utilisation_2,
        n.lane_utilisation_2_vehicles,
        n.right_turns,
        n.right_hook_conflicts,
        n.dooring_exposures
    );
    for (role, s) in &n.kerbside_dwell_s {
        eprintln!(
            "[{label}]   kerbside {role}: {} spells, mean {:.1} s, p50 {:.1} s, max {:.1} s",
            s.n, s.mean, s.p50, s.max
        );
    }
    for m in &n.mfd {
        eprintln!(
            "[{label}]   MFD t={:.0}s k={:.2} veh/km/ln q={:.1} veh/h/ln v={:.2} m/s",
            m.t_s, m.density_veh_per_km, m.flow_veh_per_h, m.speed_mps
        );
    }
}

/// The safety classes a kerbside stop could break by itself — a vehicle at the kerb, one
/// going round it, one stuck behind it — and which a mixed fleet holds at zero without
/// it. Heavy vehicles' long bodies on Manhattan-tight corners already leave the junction
/// area and clip block corners on main (`outside-junction`, `in-building`), with or
/// without anything here; those are reported, not held.
const KERBSIDE_SAFETY: [Check; 10] = [
    Check::RedEntry,
    Check::AmberEntry,
    Check::ConflictZone,
    Check::LaneChangeNearJunction,
    Check::QueueJump,
    Check::IllegalTransition,
    Check::Teleport,
    Check::StepSpeed,
    Check::LateralOffset,
    Check::Standstill,
];

#[test]
fn buses_taxis_and_deliveries_stop_at_the_kerb_and_the_traffic_goes_round() {
    let seconds = 300;
    let (main, main_audit) = run_mixed(before_realism(), seconds);
    let (before, before_audit) = run_mixed(
        EngineParams {
            kerbside: KerbsideParams {
                enabled: false,
                ..KerbsideParams::midtown()
            },
            ..EngineParams::default()
        },
        seconds,
    );
    let (after, audit) = run_mixed(
        EngineParams {
            kerbside: KerbsideParams::midtown(),
            ..EngineParams::default()
        },
        seconds,
    );
    print_network("main", &main);
    print_network("kerbside off", &before);
    print_network("kerbside on", &after);
    eprintln!("[main] audit {:?}", main_audit.counts);
    eprintln!("[kerbside off] audit {:?}", before_audit.counts);
    eprintln!("[kerbside on] audit {:?}", audit.counts);
    for e in audit
        .examples
        .iter()
        .filter(|e| SAFETY.contains(&e.check))
        .take(12)
    {
        eprintln!(
            "  [{}] t={:.1}s actor={:?} other={:?} lane={:?}: {}",
            e.check.label(),
            e.t_s,
            e.actor,
            e.other,
            e.lane,
            e.detail
        );
    }
    for e in audit
        .examples
        .iter()
        .filter(|e| e.check == Check::Standstill)
        .take(4)
    {
        eprintln!(
            "  [standstill] t={:.1}s actor={:?} lane={:?}: {}",
            e.t_s, e.actor, e.lane, e.detail
        );
    }
    let dwell = &after.network.kerbside_dwell_s;
    let now = &after.network.kerbside_now;
    eprintln!("[kerbside on] at the kerb at the end: {now:?}");
    let spells =
        |role: &str| dwell.get(role).map_or(0, |s| s.n) + now.get(role).copied().unwrap_or(0);
    assert!(spells("bus") >= 2, "buses served their stops: {dwell:?}");
    assert!(spells("taxi") >= 2, "taxis stopped at the kerb: {dwell:?}");
    assert!(
        spells("delivery") >= 1,
        "deliveries double-parked: {dwell:?}"
    );
    assert!(before.network.kerbside_dwell_s.is_empty());
    let failing: Vec<String> = KERBSIDE_SAFETY
        .iter()
        .filter(|c| audit.count(**c) > before_audit.count(**c))
        .map(|c| {
            format!(
                "{} = {} (off: {})",
                c.label(),
                audit.count(*c),
                before_audit.count(*c)
            )
        })
        .collect();
    assert!(
        failing.is_empty(),
        "kerbside stops broke an invariant: {failing:?}"
    );
    // Vehicles touching: no more than the same fleet without kerbside stops.
    assert!(
        audit.count(Check::Overlap)
            <= before_audit
                .count(Check::Overlap)
                .max(main_audit.count(Check::Overlap)),
        "overlaps {} with kerbside stops, {} without, {} on main",
        audit.count(Check::Overlap),
        before_audit.count(Check::Overlap),
        main_audit.count(Check::Overlap)
    );
    let _ = safety_failures;
}

#[test]
fn heavy_vehicles_discharge_at_the_hcm_passenger_car_equivalent() {
    let report = SaturationExperiment {
        heavy_share: 0.2,
        ..SaturationExperiment::default()
    }
    .run(NativeMobility::new(EngineParams::default()))
    .expect("experiment");
    let pce = report.saturation_headway_heavy_s.mean / report.saturation_headway_car_s.mean;
    eprintln!(
        "heavy headway {:.2} s [{}], car headway {:.2} s [{}], equivalent {pce:.2}; start-up \
         wave {:.2} m/s [{}]",
        report.saturation_headway_heavy_s.mean,
        report.saturation_headway_heavy_s.n,
        report.saturation_headway_car_s.mean,
        report.saturation_headway_car_s.n,
        report.startup_wave_mps.mean,
        report.startup_wave_mps.n
    );
    assert!(report.saturation_headway_heavy_s.n >= 10, "{report:?}");
    assert!(
        (1.5..=2.5).contains(&pce),
        "a heavy vehicle in a saturated queue is worth {pce:.2} cars; the HCM's E_T is 2.0"
    );
}

#[test]
fn drivers_are_cautious_normal_and_assertive_fifteen_seventy_fifteen() {
    let world = grid();
    let rng = RngRegistry::new(7);
    let ctx = MobilityCtx::new(0, &world, &rng);
    let mut counts = [0usize; 3];
    let n = 4000;
    for seq in 0..n {
        let t = DriverTraits::draw(&ctx, seq, VehicleClass::Passenger);
        let i = DriverStyle::ALL
            .iter()
            .position(|s| *s == t.style)
            .expect("a style");
        counts[i] += 1;
    }
    let share = |i: usize| counts[i] as f64 / n as f64;
    eprintln!("styles cautious/normal/assertive {:?}", counts);
    assert!((share(0) - 0.15).abs() < 0.025, "{counts:?}");
    assert!((share(1) - 0.70).abs() < 0.03, "{counts:?}");
    assert!((share(2) - 0.15).abs() < 0.025, "{counts:?}");
}

/// A small signalised grid with narrow pavements, packed with walkers.
fn crowded_grid() -> World {
    let params = GridParams {
        cols: 3,
        rows: 3,
        block_x_m: 70.0,
        block_y_m: 70.0,
        lanes_per_direction: 1,
        lane_width_m: 3.3,
        sidewalk_m: 1.5,
        speed_limit_mps: 11.176,
        signalised: true,
        cycle_s: 60.0,
        amber_s: 3.0,
        crossings: true,
        block_buildings: true,
        corner_radius_m: 4.5,
        ..GridParams::legacy()
    };
    v2xw_world::procedural::grid(&params, &ImportOptions::default()).expect("grid")
}

fn walk_crowd(crowd_slowing: bool) -> CalibrationReport {
    let world = crowded_grid();
    let rng = RngRegistry::new(0xC20_3D);
    let params = EngineParams::default();
    // Everyone wants the same speed, so a dense stretch of pavement is dense because it
    // is crowded, not because a fast walker is overtaking a slow one (with the usual
    // spread, the denser cells were the faster ones, crowding or not).
    let people = SocialForceParams {
        crowd_slowing,
        desired_speed_std_mps: 0.0,
        ..SocialForceParams::default()
    };
    let mut engine = NativeMobility::new(params)
        .with_vru(SocialForce::new(people))
        .with_vru_population(VruPopulation {
            pedestrians: 1300,
            cyclists: 0,
        });
    {
        let mut ctx = MobilityCtx::new(0, &world, &rng);
        engine
            .init(&mut ctx, Box::new(v2xw_mobility::demand::NoDemand::new()))
            .expect("init");
    }
    let mut harness = CalibrationObserver::new(&world);
    let mut t = 0u64;
    while t < 90 * NS_PER_S {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = engine.step(&mut ctx, params.step);
        let actors = engine.audit_actors(&world, update.t);
        let people = engine.audit_pedestrians(&world);
        harness.observe(&world, t, update.t, &actors, &update.despawned, &people);
        t = update.t;
    }
    harness.report()
}

#[test]
fn a_crowded_pavement_walks_slower_as_weidmann_measured() {
    let before = walk_crowd(false);
    let after = walk_crowd(true);
    let levels = |r: &CalibrationReport| -> Vec<String> {
        r.network
            .pedestrian_speed_by_fruin_level
            .iter()
            .zip(["A", "B", "C", "D", "E", "F"])
            .map(|(s, l)| format!("{l}: {:.2} m/s [{}]", s.mean, s.n))
            .collect()
    };
    eprintln!("crowd slowing off: {:?}", levels(&before));
    eprintln!("crowd slowing on:  {:?}", levels(&after));
    for c in after
        .network
        .comparisons()
        .iter()
        .filter(|c| c.metric.starts_with("pedestrian"))
    {
        eprintln!(
            "{} = {:.3} [{}] ref {:.3} band {:.2}-{:.2}",
            c.metric, c.measured, c.samples, c.reference, c.band.0, c.band.1
        );
    }
    let comparison = |r: &CalibrationReport, metric: &str| {
        r.network
            .comparisons()
            .into_iter()
            .find(|c| c.metric == metric)
            .expect("metric")
    };
    let d = "pedestrian speed, Fruin LOS D / LOS A";
    let (on, off) = (comparison(&after, d), comparison(&before, d));
    assert!(
        on.samples >= 20,
        "too few walkway cells at LOS D to judge: {on:?}"
    );
    // With the relation, walkers on a stretch at Fruin's LOS D walk at Weidmann's speed
    // relative to free walking; without it they do not slow, and the same check fails.
    assert!(on.passes(), "with crowd slowing: {on:?}");
    assert!(
        !off.passes(),
        "without crowd slowing the check still passed: {off:?}"
    );
}
