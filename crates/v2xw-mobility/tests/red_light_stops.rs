//! Red-light entries by the car-following models of the driving library: on a grid (4 x 6 blocks of 274 x 80 m, two lanes each way, 25 mph, 60 s
//! fixed-time cycle) every model must stop for a red. No driver here is a violator.

use std::sync::Arc;

use v2xw_core::rng::RngRegistry;
use v2xw_core::time::{Duration, NS_PER_S};
use v2xw_mobility::audit::{AuditParams, AuditReport, Check, TrafficAuditor};
use v2xw_mobility::carfollowing::w99::{W99, W99Params};
use v2xw_mobility::demand::{OdParams, PoissonDemand, PoissonParams};
use v2xw_mobility::traits::CarFollowing;
use v2xw_mobility::{EngineParams, MobilPreset, Mobility, MobilityCtx, NativeMobility};
use v2xw_world::procedural::GridParams;
use v2xw_world::{ImportOptions, World};

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
        block_buildings: false,
        corner_radius_m: 4.5,
        ..GridParams::legacy()
    };
    v2xw_world::procedural::grid(&params, &ImportOptions::default()).expect("grid")
}

fn run(cf: Option<Arc<dyn CarFollowing + Send + Sync>>, seconds: u64) -> AuditReport {
    let world = world();
    let rng = RngRegistry::new(0x00C0_FFEE_5EED);
    let params = EngineParams::default();
    let mut engine = match cf {
        Some(cf) => NativeMobility::with_models(params, cf, MobilPreset::Kesting2007),
        None => NativeMobility::new(params),
    };
    let demand = PoissonDemand::new(
        &world,
        PoissonParams {
            arrival_rate_per_s: 1500.0 / 3600.0,
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
    let mut t = 0u64;
    while t < seconds * NS_PER_S {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = engine.step(&mut ctx, params.step);
        let actors = engine.audit_actors(&world, update.t);
        auditor.observe(&world, t, update.t, &actors, &update.despawned);
        t = update.t;
    }
    auditor.report()
}

fn red_entries(r: &AuditReport) -> (u64, Vec<String>) {
    let ex = r
        .examples
        .iter()
        .filter(|e| e.check == Check::RedEntry)
        .take(4)
        .map(|e| format!("{e:?}"))
        .collect();
    (r.count(Check::RedEntry), ex)
}

#[test]
fn w99_drivers_stop_for_a_red() {
    let w99: Arc<dyn CarFollowing + Send + Sync> = Arc::new(W99::new(W99Params::default(), 7));
    let r = run(Some(w99), 180);
    let (n, ex) = red_entries(&r);
    eprintln!("w99: red entries {n}; {:?}", r.stats);
    assert!(r.stats.signalised_entries >= 50, "{:?}", r.stats);
    assert_eq!(n, 0, "W99 drivers entered on red: {ex:#?}");
}

#[test]
fn krauss_drivers_stop_for_a_red() {
    // Krauss dawdles (sigma 0.5): a committed driver slowed by its own random braking
    // arrived 0.6-0.9 s into the red with no car ahead.
    use v2xw_mobility::carfollowing::krauss::{Krauss, KraussParams};
    let k: Arc<dyn CarFollowing + Send + Sync> = Arc::new(Krauss::new(KraussParams::default(), 7));
    let r = run(Some(k), 180);
    let (n, ex) = red_entries(&r);
    eprintln!("krauss: red entries {n}; {:?}", r.stats);
    assert!(r.stats.signalised_entries >= 50, "{:?}", r.stats);
    assert_eq!(n, 0, "Krauss drivers entered on red: {ex:#?}");
}

#[test]
fn idm_drivers_stop_for_a_red() {
    let r = run(None, 180);
    let (n, ex) = red_entries(&r);
    eprintln!("idm: red entries {n}; {:?}", r.stats);
    assert!(r.stats.signalised_entries >= 50, "{:?}", r.stats);
    assert_eq!(n, 0, "IDM drivers entered on red: {ex:#?}");
    // Kept out of the red without braking inside the junction: a committed driver held
    // at its speed into a 6 m/s turn braked there at -6 m/s² (the IDM floor).
    assert!(r.stats.max_decel_mps2 <= 4.5, "{:?}", r.stats);
}

#[test]
fn the_amber_arrival_time_counts_the_slowing_for_a_turn() {
    use v2xw_mobility::engine::arrival_time_s;
    // No turn, or a turn faster than the present speed: distance over speed.
    assert!((arrival_time_s(30.0, 10.0, None, 2.0) - 3.0).abs() < 1e-12);
    assert!((arrival_time_s(30.0, 10.0, Some(12.0), 2.0) - 3.0).abs() < 1e-12);
    // 10 -> 6 m/s at 2 m/s² takes 16 m and 2 s; the other 14 m at 10 m/s, 1.4 s.
    assert!((arrival_time_s(30.0, 10.0, Some(6.0), 2.0) - 3.4).abs() < 1e-9);
    // Too close to slow all the way: 10 m at 2 m/s² from 10 m/s ends at sqrt(60).
    let t = arrival_time_s(10.0, 10.0, Some(6.0), 2.0);
    assert!((t - (10.0 - 60f64.sqrt()) / 2.0).abs() < 1e-9, "{t}");
}
