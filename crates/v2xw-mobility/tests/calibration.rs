//! The traffic model against the figures a traffic engineer measures at the kerb.
//!
//! `v2xw_mobility::calibration` measures the model with the literature's own procedures and
//! sets each figure beside its published reference with a stated band
//! ([`CalibrationReport::comparisons`]). These tests hold the model to those bands:
//!
//! * **The saturation-flow field study** ([`SaturationExperiment`]): a signalised crossroads
//!   whose approaches never run dry, measured the HCM way — saturation headway from the
//!   fourth queued vehicle on, start-up lost time over the first four, queue spacing.
//! * **A Midtown-shaped grid under demand, with pedestrians**: free-flow speed against the
//!   posted limit, launch acceleration, stopping deceleration, walking speed.
//!
//! Each test prints its whole table, so a failure shows every figure, not only the one that
//! left its band. The full-size measurements (Midtown itself) are made with
//! `cargo run -p v2xw-engine --example traffic_calibration`.

use v2xw_core::rng::RngRegistry;
use v2xw_core::time::{Duration, NS_PER_S};
use v2xw_mobility::calibration::{
    CalibrationObserver, CalibrationReport, Comparison, SaturationExperiment,
};
use v2xw_mobility::demand::{OdParams, PoissonDemand, PoissonParams};
use v2xw_mobility::engine::VruPopulation;
use v2xw_mobility::{EngineParams, Mobility, MobilityCtx, NativeMobility};
use v2xw_world::procedural::GridParams;
use v2xw_world::{ImportOptions, World};

fn print(report: &CalibrationReport) {
    for c in report.comparisons() {
        eprintln!(
            "{:<46} {:>8.3} [{:>6}] ref {:>6.3} band {:.2}-{:.2} {}",
            c.metric,
            c.measured,
            c.samples,
            c.reference,
            c.band.0,
            c.band.1,
            if c.passes() { "ok" } else { "OUT" }
        );
    }
    let f = &report.free_flow_speed_ratio;
    let (acc, rej) = (
        &report.permitted_left_lag_accepted_s,
        &report.permitted_left_lag_rejected_s,
    );
    eprintln!(
        "free-flow ratio p15/p50/p85 {:.2}/{:.2}/{:.2}; permitted-left lags accepted mean {:.2} \
         p15 {:.2} [{}], rejected mean {:.2} p85 {:.2} [{}]; stop decel p50 {:.2}",
        f.p15, f.p50, f.p85, acc.mean, acc.p15, acc.n, rej.mean, rej.p85, rej.n,
        report.stop_decel_mps2.p50
    );
    eprintln!(
        "headways by position: {:?}",
        report
            .headway_by_position_s
            .iter()
            .map(|h| format!("{:.2}[{}]", h.mean, h.n))
            .collect::<Vec<_>>()
    );
}

fn metric<'a>(report: &'a [Comparison], name: &str) -> &'a Comparison {
    report
        .iter()
        .find(|c| c.metric == name)
        .unwrap_or_else(|| panic!("no metric {name}"))
}

fn hold(comparisons: &[Comparison], names: &[&str], min_samples: usize) {
    let failing: Vec<String> = names
        .iter()
        .map(|n| metric(comparisons, n))
        .filter(|c| !c.passes() || c.samples < min_samples)
        .map(|c| {
            format!(
                "{} = {:.3} over {} samples, band {:.2}-{:.2} ({})",
                c.metric, c.measured, c.samples, c.band.0, c.band.1, c.source
            )
        })
        .collect();
    assert!(failing.is_empty(), "outside the published band: {failing:#?}");
}

#[test]
fn a_saturated_stop_line_discharges_at_the_published_rate() {
    let report = SaturationExperiment::default()
        .run(NativeMobility::new(EngineParams::default()))
        .expect("experiment");
    print(&report);
    let c = report.comparisons();
    hold(
        &c,
        &[
            "saturation headway, through, s",
            "start-up lost time, s",
            "queue spacing, front to front, m",
        ],
        10,
    );
    // The queue stabilises after the first few: the fifth-and-later headways are shorter
    // than the first (FHWA Signal Timing Manual 2008 §3.3.1, Fig. 3-2).
    let h = &report.headway_by_position_s;
    assert!(h[0].mean > h[5].mean, "{:?}", h);
}

/// A Midtown-shaped grid: avenues 274 m apart, streets 80 m, 25 mph, signals, crosswalks
/// and pedestrians.
fn midtown_grid() -> World {
    let params = GridParams {
        cols: 5,
        rows: 8,
        block_x_m: 274.0,
        block_y_m: 80.0,
        lanes_per_direction: 2,
        lane_width_m: 3.35,
        sidewalk_m: 2.5,
        speed_limit_mps: 11.176,
        signalised: true,
        cycle_s: 90.0,
        amber_s: 3.0,
        crossings: true,
        block_buildings: true,
        corner_radius_m: 4.5,
        ..GridParams::legacy()
    };
    v2xw_world::procedural::grid(&params, &ImportOptions::default()).expect("grid")
}

#[test]
fn drivers_and_pedestrians_on_a_midtown_grid_move_as_published() {
    let world = midtown_grid();
    let rng = RngRegistry::new(0xCA11_B8A7);
    let seconds = 300;
    let params = EngineParams::default();
    let mut engine = NativeMobility::new(params).with_vru_population(VruPopulation {
        pedestrians: 120,
        cyclists: 0,
    });
    let demand = PoissonDemand::new(
        &world,
        PoissonParams {
            arrival_rate_per_s: 0.8,
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
    let mut harness = CalibrationObserver::new(&world);
    let mut t = 0u64;
    while t < seconds * NS_PER_S {
        let mut ctx = MobilityCtx::new(t, &world, &rng);
        let update = engine.step(&mut ctx, params.step);
        let actors = engine.audit_actors(&world, update.t);
        let people = engine.audit_pedestrians(&world);
        harness.observe(&world, t, update.t, &actors, &update.despawned, &people);
        t = update.t;
    }
    let report = harness.report();
    print(&report);
    hold(
        &report.comparisons(),
        &[
            "free-flow speed / limit, mean",
            "free-flow speed / limit, spread",
            "launch acceleration 0-8 m/s, mean, m/s²",
            "stop deceleration, 85th percentile, m/s²",
            "pedestrian walking speed, mean, m/s",
            "pedestrian walking speed, 15th percentile, m/s",
        ],
        30,
    );
}
