//! The `traffic` section in a run: actuated signals move their plans and log it on
//! `traffic.control`, and fixed-time control does neither.

use std::path::{Path, PathBuf};

use v2xw_engine::scenario::schema::{SignalControl, SignalControlKind, Traffic};
use v2xw_engine::{Engine, MemoryRecorder, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

fn records(s: Scenario, channel: &str) -> Vec<serde_json::Value> {
    let mut engine = Engine::build(s, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    engine.run(&mut recorder).expect("runs");
    recorder
        .records()
        .iter()
        .filter(|(_, r)| r.channel == channel)
        .map(|(_, r)| serde_json::from_slice(&r.json).expect("json"))
        .collect()
}

fn scenario(control: SignalControlKind) -> Scenario {
    let mut s = Scenario::load(scenarios().join("connected-intersections.yaml"))
        .expect("the shipped scenario loads");
    s.time.duration_s = 90.0;
    s.metrics = vec![];
    s.actors.vehicles.equipped_fraction = 0.0;
    s.traffic = Some(Traffic {
        signals: SignalControl {
            control,
            ..SignalControl::default()
        },
        ..Traffic::default()
    });
    s
}

#[test]
fn actuated_signals_act_and_say_so_and_fixed_time_ones_do_not() {
    let actuated = records(scenario(SignalControlKind::Actuated), "traffic.control");
    let last = actuated
        .last()
        .expect("an actuated run logs its controllers");
    let acted = last["gap_outs"].as_u64().unwrap_or(0) + last["max_outs"].as_u64().unwrap_or(0);
    assert!(
        acted > 0 || last["extended_s"].as_f64().unwrap_or(0.0) > 0.0,
        "the actuated controllers did nothing in 90 s: {last}"
    );
    // The metric provider reports the same counts (it is on whenever a traffic section is).
    let metrics = records(scenario(SignalControlKind::Actuated), "metric.sample");
    assert!(
        metrics.iter().any(|m| m["metric"] == "signal_gap_outs"),
        "no signal_gap_outs sample among {} metric samples",
        metrics.len()
    );
    let fixed = records(scenario(SignalControlKind::FixedTime), "traffic.control");
    assert!(fixed.is_empty(), "fixed-time control logged {fixed:?}");
}
