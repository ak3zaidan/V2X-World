//! WP-08: the safety applications react to drivers who break the rules on their own.
//!
//! Each check runs one scenario twice on the same seed: once with the drivers' violations
//! off (the default: the drivers yield where they must, and only the population's own few
//! late amber goers come near a red) and once with
//! `actors.vehicles.drivers.violations` on. No event is scripted: the red-light runners
//! and the drivers who fail to yield are drawn from the population. The off run is the
//! negative control: a check that passed on it would not be measuring the violators.
//!
//! # Why two scenarios, and not the field rates
//!
//! The first form of this test (2026-10-26) ran `connected-intersections` for 60 s at the
//! field rates with its four SPaT units and asserted RLVW and IMA both rise. It could not
//! pass, for two reasons in its premise, not in the engine (traffic-fix4.yaml
//! `F4.viol.*`, traffic-r5.yaml `R5.viol.*`):
//!
//! * **Too few runners where RLVW can see them.** At the field rate (TTI 0-4196-P1
//!   Table 1, 1.0 per 10,000 vehicle-cycles: `p·μ/C` = 0.006 red runs per signal passage
//!   at C = 60 s) 48 vehicles in 60 s make about 0.6 red runs on the whole grid, and under
//!   0.1 within 150 m of the four units that send SPaT. The on and off runs were the same
//!   run (stream digest 86a68534 both). RLVW warns only from a SPaT it heard
//!   (`v2xw_node::apps` `signal_situation`), so here a unit stands at **every** junction,
//!   and the runners are many and late (share 1.0, mean 3 s, the 4 s cap): the test is of
//!   the warning on a runner, not of the rate, which `red_light_running_sits_in_the_field_band`
//!   pins in v2xw-mobility.
//! * **No IMA conflict from a violator on an all-signalised grid.** IMA warns of a
//!   remote on a *crossing* path (45-135 degrees) with both vehicles moving at 1 m/s or
//!   more and arrival times within its window (`v2xw_node::apps::ima`). On the grid every
//!   junction is signalised with no all-red (`procedural.rs` signal plans), so a failure
//!   to yield can only act at a permissive green (`v2xw_mobility::engine` decision pass:
//!   `junction.signal` `None | GreenYield`), whose foe is the *opposing* stream, 180
//!   degrees, LTA's case and not IMA's; and a red runner enters while the cross queue is
//!   still standing in its start-up reaction (speed under 1 m/s). Measured: IMA 58 -> 56
//!   (36 units, 120 s). IMA's violator is the driver who fails to give way at an
//!   **unsignalised** junction, so its check runs the same grid with priority to the
//!   right (`signalised: false`) and every driver failing to yield to slow foes.

use std::path::{Path, PathBuf};

use v2xw_engine::{Engine, NullRecorder, RunReport, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// `connected-intersections` cut to a 3 x 3 grid for `duration_s`, with a SPaT and MAP
/// unit at every junction when `signalised`, or no signals and no units when not.
fn grid(signalised: bool, duration_s: f64) -> Scenario {
    let path = scenarios().join("connected-intersections.yaml");
    let text = std::fs::read_to_string(&path).expect("the shipped scenario reads");
    let head = text
        .split("\n  rsus:\n")
        .next()
        .expect("the scenario lists units");
    let tail = text
        .split("\nweather:")
        .nth(1)
        .expect("the scenario has a weather block");
    let mut units = String::new();
    if signalised {
        units.push_str("\n  rsus:\n");
        for site in 0..9 {
            units.push_str(&format!(
                "    - site: {site}\n      roles: [spat, map]\n      profile: rsu/commsignia-its-rs4\n"
            ));
        }
    }
    let text = format!("{head}{units}\nweather:{tail}")
        .replace("cols: 6", "cols: 3")
        .replace("rows: 6", "rows: 3")
        .replace(
            "signalised: true",
            if signalised {
                "signalised: true"
            } else {
                "signalised: false"
            },
        )
        .replace("rsu_at_junctions: true", &format!("rsu_at_junctions: {signalised}"))
        .replace(
            "sets: [bsm, spat, map, srm, ssm]",
            if signalised {
                "sets: [bsm, spat, map, srm, ssm]"
            } else {
                // No signal, no unit: the vehicles' own BSMs are all IMA reads.
                "sets: [bsm]"
            },
        );
    let mut s = Scenario::parse(&text, path.parent()).expect("the cut scenario parses");
    s.time.duration_s = duration_s;
    s.metrics = vec![];
    s
}

fn run(mut s: Scenario, violations: Option<serde_json::Value>) -> RunReport {
    // The fault that proves the checks can fail: `V2XW_FAULT=viol-off` drops the
    // violations block from the "on" run, which is then the off run again.
    let fault = std::env::var("V2XW_FAULT").is_ok_and(|f| f == "viol-off");
    if let Some(v) = violations.filter(|_| !fault) {
        s.actors.vehicles.drivers.violations =
            Some(serde_json::from_value(v).expect("the violations block"));
    }
    let mut engine = Engine::build(s, "").expect("builds");
    // Nothing is kept: the report carries the tallies, and a 120 s run of nine units
    // writes millions of records (a memory recorder passed 4.5 GB).
    let mut recorder = NullRecorder::new();
    engine.run(&mut recorder).expect("runs")
}

fn tally(r: &RunReport, app: &str) -> v2xw_engine::app_truth::AppTally {
    r.apps.get(app).copied().unwrap_or_default()
}

#[test]
fn rlvw_fires_on_drivers_who_run_the_red_unscripted() {
    let off = run(grid(true, 120.0), None);
    let on = run(
        grid(true, 120.0),
        Some(serde_json::json!({
            "enabled": true,
            "red_run_share": 1.0,
            "red_run_tolerance_mean_s": 3.0,
        })),
    );
    let (a, b) = (tally(&off, "rlvw"), tally(&on, "rlvw"));
    eprintln!(
        "rlvw issued {} -> {}, true {} -> {}, missed {} -> {} ({} / {} vehicles)",
        a.issued, b.issued, a.true_warnings, b.true_warnings, a.missed, b.missed,
        off.actors_spawned, on.actors_spawned
    );
    // RLVW warns a driver who will enter on red: with the runners on, some do, and the
    // labeller (which reads the truth) confirms some of the warnings. The off run's true
    // warnings are the default population's late amber goers (measured: 1 in 120 s).
    assert!(
        b.issued > a.issued,
        "RLVW did not react to the red-light runners: {} -> {}",
        a.issued,
        b.issued
    );
    assert!(
        b.true_warnings > a.true_warnings,
        "no more true RLVW warnings with red-light runners: {} -> {}",
        a.true_warnings,
        b.true_warnings
    );
}

#[test]
fn ima_fires_on_drivers_who_fail_to_yield_unscripted() {
    let off = run(grid(false, 120.0), None);
    let on = run(
        grid(false, 120.0),
        Some(serde_json::json!({
            "enabled": true,
            "fail_to_yield_share": 1.0,
        })),
    );
    let (a, b) = (tally(&off, "ima"), tally(&on, "ima"));
    eprintln!(
        "ima issued {} -> {}, true {} -> {}, missed {} -> {} ({} / {} vehicles)",
        a.issued, b.issued, a.true_warnings, b.true_warnings, a.missed, b.missed,
        off.actors_spawned, on.actors_spawned
    );
    // IMA warns of a crossing vehicle arriving at about the same time: a driver who
    // pulls out in front of a slow foe it should have given way to is one.
    assert!(
        b.issued > a.issued,
        "IMA did not react to the drivers who fail to yield: {} -> {}",
        a.issued,
        b.issued
    );
    assert!(
        b.true_warnings > a.true_warnings,
        "no more true IMA warnings with drivers who fail to yield: {} -> {}",
        a.true_warnings,
        b.true_warnings
    );
}
