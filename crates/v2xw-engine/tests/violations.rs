//! WP-08: the safety applications react to drivers who break the rules on their own.
//!
//! The connected-intersections family (six signalised junctions with SPaT/MAP roadside
//! units, every application on) is run twice on the same seed: once with the drivers'
//! violations off (the default, under which every driver clears on amber and RLVW has
//! nothing true to warn of) and once with `actors.vehicles.drivers.violations` on. No
//! event is scripted: the red-light runners and the drivers who fail to yield are drawn
//! from the population. The off run is the negative control: a check that passed on it
//! would not be measuring the violators.

use std::path::{Path, PathBuf};

use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

fn run(violations: bool) -> RunReport {
    let mut s = Scenario::load(scenarios().join("connected-intersections.yaml"))
        .expect("the shipped scenario loads");
    s.time.duration_s = 60.0;
    s.metrics = vec![];
    if violations {
        s.actors.vehicles.drivers.violations = Some(
            serde_json::from_value(serde_json::json!({
                "enabled": true,
                "fail_to_yield_share": 0.2,
            }))
            .expect("the violations block"),
        );
    }
    let mut engine = Engine::build(s, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    engine.run(&mut recorder).expect("runs")
}

#[test]
fn rlvw_and_ima_fire_on_drivers_who_break_the_rules_unscripted() {
    let off = run(false);
    let on = run(true);
    let tally = |r: &RunReport, app: &str| r.apps.get(app).copied().unwrap_or_default();
    let (rlvw_off, rlvw_on) = (tally(&off, "rlvw"), tally(&on, "rlvw"));
    let (ima_off, ima_on) = (tally(&off, "ima"), tally(&on, "ima"));
    eprintln!(
        "rlvw issued {} -> {} (true {} -> {}); ima issued {} -> {} (true {} -> {})",
        rlvw_off.issued,
        rlvw_on.issued,
        rlvw_off.true_warnings,
        rlvw_on.true_warnings,
        ima_off.issued,
        ima_on.issued,
        ima_off.true_warnings,
        ima_on.true_warnings
    );
    // RLVW warns a driver who will enter on red: with violations on, some do, and the
    // labeller (which reads the truth) confirms some warnings.
    assert!(
        rlvw_on.issued > rlvw_off.issued,
        "RLVW did not react to the red-light runners: {} -> {}",
        rlvw_off.issued,
        rlvw_on.issued
    );
    assert!(
        rlvw_on.true_warnings > rlvw_off.true_warnings,
        "no more true RLVW warnings with red-light runners"
    );
    // IMA warns of crossing paths with close arrival times: a runner entering as the cross
    // street moves off, a driver who does not yield.
    assert!(
        ima_on.issued > ima_off.issued,
        "IMA did not react to the violators: {} -> {}",
        ima_off.issued,
        ima_on.issued
    );
}
