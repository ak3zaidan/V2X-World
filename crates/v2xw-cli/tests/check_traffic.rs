//! `v2xw check-traffic` over a real short run, and the faults that prove its rows can fail.

use std::path::{Path, PathBuf};

use v2xw_cli::check_traffic::{CheckOptions, check_recording, check_traffic};
use v2xw_cli::run::{RunOptions, run};
use v2xw_world::SignalState;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives two levels below the workspace root")
        .to_path_buf()
}

fn row(c: &v2xw_cli::check_traffic::TrafficCheck, name: &str) -> u64 {
    c.rows
        .iter()
        .find(|r| r.name == name)
        .map_or(0, |r| r.count)
}

/// One 40 s run of the signalised grid at a busy demand, then the check on it and on a
/// world whose plans the recorded lamps do not match.
#[test]
fn check_traffic_passes_a_clean_run_and_fails_a_tampered_plan() {
    let out = std::env::temp_dir()
        .join("v2xw-cli-tests")
        .join("check-traffic");
    let _ = std::fs::remove_dir_all(&out);
    run(&RunOptions {
        scenario: repo_root().join("scenarios/phase1-grid.yaml"),
        out: Some(out.clone()),
        build_utc: Some("2026-09-22T00:00:00Z".to_string()),
        keyframe_ms: 1000,
        node_only: false,
        record: true,
        attach_world: false,
        verify: false,
        stream_digest: false,
        duration_s: Some(40.0),
        rate_veh_per_h: Some(6000.0),
        city: None,
        json: false,
        long: v2xw_cli::run::LongRun::default(),
    })
    .expect("the run");

    let opts = CheckOptions::new(&out);
    let clean = check_traffic(&opts).expect("the check");
    assert!(
        clean.vehicles > 20,
        "a busy run: {} vehicles",
        clean.vehicles
    );
    assert!(clean.steps >= 390, "every step read: {}", clean.steps);
    for r in &clean.rows {
        assert!(r.ok, "{} failed on a clean run: {:?}", r.name, r.examples);
    }
    assert_eq!(row(&clean, "lamp_phase_unknown"), 0);

    // Fault 1: the checker's world has every movement on a permanent protected green, so
    // every vehicle the run held at a red now stands "on green".
    let scenario =
        v2xw_engine::Scenario::load(out.join("scenario.resolved.yaml")).expect("scenario");
    let mut world = v2xw_engine::wiring::build_world(&scenario).expect("world");
    for plan in &mut world.signals {
        for phase in &mut plan.phases {
            for s in &mut phase.states {
                *s = SignalState::Green;
            }
        }
    }
    let all_green = check_recording(&world, &out.join("recording.mcap"), &opts).expect("check");
    assert!(
        row(&all_green, "stopped_on_green") > 0 || row(&all_green, "lamp_phase_unknown") > 0,
        "a tampered plan must show"
    );
    assert!(!all_green.ok);
}
