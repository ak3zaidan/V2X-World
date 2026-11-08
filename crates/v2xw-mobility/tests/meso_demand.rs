//! The queue tier takes the same demand as the microscopic engine: on a signalised grid
//! (the shape of scenarios/tests/traffic-grid.yaml) the meso run must put within 15 % of
//! the microscopic run's vehicles on the road, from the same Poisson demand and seed.

use v2xw_core::rng::RngRegistry;
use v2xw_core::time::{Duration, NS_PER_S};
use v2xw_mobility::demand::{OdParams, PoissonDemand, PoissonParams};
use v2xw_mobility::meso::{MesoMobility, MesoParams};
use v2xw_mobility::{EngineParams, Mobility, MobilityCtx, NativeMobility};
use v2xw_world::procedural::GridParams;
use v2xw_world::{ImportOptions, World};

fn world() -> World {
    v2xw_world::procedural::grid(
        &GridParams {
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
            crossings: true,
            block_buildings: false,
            corner_radius_m: 4.5,
            ..GridParams::legacy()
        },
        &ImportOptions::default(),
    )
    .expect("grid")
}

fn spawned(engine: &mut dyn Mobility, world: &World, seconds: u64) -> usize {
    let rng = RngRegistry::new(0x00C0_FFEE_5EED);
    let demand = PoissonDemand::new(
        world,
        PoissonParams {
            arrival_rate_per_s: 1500.0 / 3600.0,
            duration: Duration::from_secs(seconds),
            ..PoissonParams::default()
        },
        OdParams::default(),
    )
    .expect("demand");
    {
        let mut ctx = MobilityCtx::new(0, world, &rng);
        engine.init(&mut ctx, Box::new(demand)).expect("init");
    }
    let step = EngineParams::default().step;
    let mut n = 0;
    let mut t = 0u64;
    while t < seconds * NS_PER_S {
        let mut ctx = MobilityCtx::new(t, world, &rng);
        let update = engine.step(&mut ctx, step);
        n += update.spawned.len();
        t = update.t;
    }
    n
}

#[test]
fn the_queue_tier_takes_the_microscopic_demand() {
    let w = world();
    let micro = spawned(&mut NativeMobility::new(EngineParams::default()), &w, 180);
    let mut m = MesoMobility::new(MesoParams::default());
    let meso = spawned(&mut m, &w, 180);
    eprintln!(
        "spawned: micro {micro}, meso {meso}; meso dropped {}",
        m.dropped_trips()
    );
    assert!(micro >= 50, "micro {micro}");
    let r = meso as f64 / micro as f64;
    assert!(
        (0.85..=1.15).contains(&r),
        "meso {meso} against micro {micro}"
    );
}
