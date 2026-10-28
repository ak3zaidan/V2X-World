//! WP-17 P17.5: `traffic.signals.control: nema` in a run. The dual ring drives every
//! signal of a generated grid, the drivers move and stop by it, the run is byte-identical
//! on a rerun, and it behaves unlike both fixed-time and single-ring actuated control.

use std::path::{Path, PathBuf};

use v2xw_engine::scenario::schema::{SignalControl, SignalControlKind, Traffic};
use v2xw_engine::{Engine, MemoryRecorder, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// A signalised 4x6 procedural grid, traffic only (no radios), with a steady demand.
fn grid(control: SignalControlKind, secs: f64) -> Scenario {
    let mut s = Scenario::load(scenarios().join("phase1-grid.yaml")).expect("loads");
    s.time.duration_s = secs;
    s.metrics = vec![];
    s.actors.vehicles.equipped_fraction = 0.0;
    s.actors.vehicles.demand.rate_veh_per_h = Some(1500.0);
    s.world.buildings.enabled = false;
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = serde_json::json!(4);
        params["rows"] = serde_json::json!(6);
    }
    s.traffic = Some(Traffic {
        signals: SignalControl {
            control,
            ..SignalControl::default()
        },
        ..Traffic::default()
    });
    s
}

fn run(s: Scenario) -> MemoryRecorder {
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "the scenario should load: {errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    engine.run(&mut rec).expect("runs");
    rec
}

fn channel(rec: &MemoryRecorder, name: &str) -> Vec<serde_json::Value> {
    rec.records()
        .iter()
        .filter(|(_, r)| r.channel == name)
        .map(|(_, r)| serde_json::from_slice(&r.json).expect("json"))
        .collect()
}

/// `(mean speed, share of samples standing)` over every ground-truth actor sample.
fn motion(rec: &MemoryRecorder) -> (f64, f64) {
    let v: Vec<f64> = channel(rec, "gt.kinematics")
        .iter()
        .filter_map(|k| k.get("speed_mps")?.as_f64())
        .collect();
    assert!(!v.is_empty(), "no kinematics");
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    let standing = v.iter().filter(|x| **x < 0.1).count() as f64 / v.len() as f64;
    (mean, standing)
}

/// The controllers' own log at the end of the run: (gap-outs, max-outs).
fn ends(rec: &MemoryRecorder) -> (u64, u64) {
    let last = channel(rec, "traffic.control")
        .pop()
        .expect("a signal controller logs traffic.control");
    (
        last["gap_outs"].as_u64().unwrap_or(0),
        last["max_outs"].as_u64().unwrap_or(0),
    )
}

/// The dual ring runs every grid signal: it gaps out and maxes out, vehicles move and
/// stand at its reds, and its log and motion differ from single-ring actuated control
/// on the same seed. A rerun is byte-identical, record for record.
#[test]
fn the_dual_ring_drives_a_grid_and_reruns_byte_identical() {
    let a = run(grid(SignalControlKind::Nema, 180.0));
    let (gaps, maxes) = ends(&a);
    assert!(
        gaps + maxes > 0,
        "the dual ring did nothing: {gaps} {maxes}"
    );
    let (v, standing) = motion(&a);
    assert!(v > 1.0, "vehicles barely move under the dual ring: {v} m/s");
    assert!(standing > 0.02, "nobody stands at a red: {standing}");
    eprintln!("nema: {gaps} gap-outs, {maxes} max-outs, {v:.3} m/s, {standing:.3} standing");

    let b = run(grid(SignalControlKind::Nema, 180.0));
    let bytes = |r: &MemoryRecorder| -> Vec<(String, Vec<u8>)> {
        r.records()
            .iter()
            .map(|(_, x)| (x.channel.to_string(), x.json.to_vec()))
            .collect()
    };
    assert!(bytes(&a) == bytes(&b), "a rerun of the same seed differs");

    let single = run(grid(SignalControlKind::Actuated, 180.0));
    let (sv, _) = motion(&single);
    assert!(
        ends(&single) != (gaps, maxes) || (sv - v).abs() > 1e-9,
        "the dual ring ran exactly as the single ring"
    );
    eprintln!("single ring: {:?}, {sv:.3} m/s", ends(&single));
}
