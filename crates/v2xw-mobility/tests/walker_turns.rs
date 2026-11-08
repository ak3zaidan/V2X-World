//! Walkers rounding a corner do not dither on the spot: on a signalised grid with 300
//! pedestrians, under 0.4 % of pedestrian steps may change speed faster than 3 m/s²
//! (check-traffic's `ped_speed_jump`), as a walker turning back and forth at a lane end
//! whose next lane starts beside it would. Speeds here are from the positions, step to step, as the recording's are.

use std::collections::BTreeMap;

use v2xw_core::rng::RngRegistry;
use v2xw_core::time::{Duration, NS_PER_S};
use v2xw_mobility::demand::{OdParams, PoissonDemand, PoissonParams};
use v2xw_mobility::engine::VruPopulation;
use v2xw_mobility::{EngineParams, Mobility, MobilityCtx, NativeMobility};
use v2xw_world::ImportOptions;
use v2xw_world::procedural::GridParams;

#[test]
fn walkers_rounding_a_corner_do_not_dither() {
    let world = v2xw_world::procedural::grid(
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
    .expect("grid");
    let rng = RngRegistry::new(0x00C0_FFEE_5EED);
    let params = EngineParams::default();
    let mut engine = NativeMobility::new(params).with_vru_population(VruPopulation {
        pedestrians: 300,
        cyclists: 0,
    });
    let seconds = 120;
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
    let dt = params.step.as_secs_f64();
    let mut last: BTreeMap<_, (f64, f64, Option<f64>)> = BTreeMap::new();
    let (mut steps, mut jumps) = (0u64, 0u64);
    let mut per_walker: BTreeMap<_, u64> = BTreeMap::new();
    let mut t = 0u64;
    while t < seconds * NS_PER_S {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = engine.step(&mut ctx, params.step);
        for p in engine.audit_pedestrians(&world) {
            let e = last.entry(p.actor).or_insert((p.pos.x, p.pos.y, None));
            let v = ((p.pos.x - e.0).powi(2) + (p.pos.y - e.1).powi(2)).sqrt() / dt;
            if let Some(v0) = e.2 {
                steps += 1;
                if ((v - v0) / dt).abs() > 3.0 {
                    jumps += 1;
                    *per_walker.entry(p.actor).or_default() += 1;
                }
            }
            *e = (p.pos.x, p.pos.y, Some(v));
        }
        t = update.t;
    }
    let worst = per_walker.values().copied().max().unwrap_or(0);
    let share = jumps as f64 / steps.max(1) as f64;
    eprintln!(
        "walker steps {steps}, over 3 m/s² {jumps} ({:.3} %), worst walker {worst}",
        100.0 * share
    );
    assert!(steps > 20_000, "too few walker steps: {steps}");
    assert!(share < 0.004, "{jumps} of {steps} walker steps over 3 m/s²");
    assert!(
        worst < 100,
        "one walker changed speed past 3 m/s² on {worst} steps"
    );
}

/// No walker changes speed faster than 3 m/s² (check-traffic's `ped_speed_jump`) with the
/// observed behaviour on 1 m pavements: crowds waiting at the crossings, groups, mid-block
/// crossings. On the quarter benchmark's first live window that row counted 1,812 steps in
/// 60 s, from walkers pushed sideways in the crowds at the crossings and stopped dead on a
/// pavement's edge; once a walker's velocity was bounded, 58 remained, every one a
/// mid-block crossing started from where the wait began rather than from where the walker
/// stood (0.9 m/s to 0 and back, or put back 0.25 m in a step).
#[test]
fn walkers_on_narrow_pavements_change_speed_as_walkers_do() {
    let world = v2xw_world::procedural::grid(
        &GridParams {
            cols: 4,
            rows: 6,
            block_x_m: 274.0,
            block_y_m: 80.0,
            lanes_per_direction: 2,
            lane_width_m: 3.5,
            sidewalk_m: 1.0,
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
    .expect("grid");
    let rng = RngRegistry::new(0x00C0_FFEE_5EED);
    let params = EngineParams::default();
    let mut engine = NativeMobility::new(params)
        .with_vru(v2xw_mobility::SocialForce::new(
            v2xw_mobility::SocialForceParams::observed(),
        ))
        .with_vru_population(VruPopulation {
            pedestrians: 300,
            cyclists: 0,
        });
    let seconds = 120;
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
    let dt = params.step.as_secs_f64();
    let mut last: BTreeMap<_, (f64, f64, Option<f64>)> = BTreeMap::new();
    let (mut steps, mut jumps, mut midblock_steps) = (0u64, 0u64, 0u64);
    let mut examples = Vec::new();
    let mut t = 0u64;
    while t < seconds * NS_PER_S {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = engine.step(&mut ctx, params.step);
        for p in engine.audit_pedestrians(&world) {
            if p.activity == v2xw_mobility::vru::PedActivity::CrossingMidblock {
                midblock_steps += 1;
            }
            // A walker's first frame has no step behind it, so no speed.
            let Some(e) = last.get_mut(&p.actor) else {
                last.insert(p.actor, (p.pos.x, p.pos.y, None));
                continue;
            };
            let v = ((p.pos.x - e.0).powi(2) + (p.pos.y - e.1).powi(2)).sqrt() / dt;
            if let Some(v0) = e.2 {
                steps += 1;
                if ((v - v0) / dt).abs() > 3.0 {
                    jumps += 1;
                    if examples.len() < 8 {
                        examples.push(format!("{:?} at {} ns: {v0:.2}->{v:.2}", p.actor, t));
                    }
                }
            }
            *e = (p.pos.x, p.pos.y, Some(v));
        }
        t = update.t;
    }
    eprintln!(
        "walker steps {steps}, over 3 m/s² {jumps}, mid-block crossing steps {midblock_steps}"
    );
    assert!(steps > 20_000, "too few walker steps: {steps}");
    assert!(
        midblock_steps > 0,
        "no walker crossed mid-block, so the start of a crossing is not exercised"
    );
    assert_eq!(
        jumps, 0,
        "{jumps} of {steps} walker steps over 3 m/s²: {examples:#?}"
    );
}
