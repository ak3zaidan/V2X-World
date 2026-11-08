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
        veremi: None,
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
            phase.states.fill(SignalState::Green);
        }
    }
    let all_green = check_recording(&world, &out.join("recording.mcap"), &opts).expect("check");
    assert!(
        row(&all_green, "stopped_on_green") > 0 || row(&all_green, "lamp_phase_unknown") > 0,
        "a tampered plan must show"
    );
    assert!(!all_green.ok);
}

/// Under the NEMA dual ring the lamps follow the running controller and check-traffic reads them
/// so: drivers who obey the rings enter on no red. Read through the load-time plan, the same
/// recording's lamps match none of its phases (149 apparent red entries in 180 s).
#[test]
fn check_traffic_reads_the_running_controller_under_nema() {
    let out = std::env::temp_dir()
        .join("v2xw-cli-tests")
        .join("check-traffic-nema");
    let _ = std::fs::remove_dir_all(&out);
    run(&RunOptions {
        scenario: repo_root().join("scenarios/tests/traffic-grid-nema.yaml"),
        out: Some(out.clone()),
        build_utc: Some("2026-09-22T00:00:00Z".to_string()),
        keyframe_ms: 1000,
        node_only: false,
        record: true,
        attach_world: false,
        verify: false,
        stream_digest: false,
        duration_s: Some(90.0),
        rate_veh_per_h: None,
        veremi: None,
        city: None,
        json: false,
        long: v2xw_cli::run::LongRun::default(),
    })
    .expect("the run");
    let opts = CheckOptions::new(&out);
    let c = check_traffic(&opts).expect("the check");
    assert!(c.vehicles > 10, "{} vehicles", c.vehicles);
    assert_eq!(row(&c, "lamp_phase_unknown"), 0);
    assert_eq!(
        row(&c, "red_entry"),
        0,
        "{:?}",
        c.rows.iter().find(|r| r.name == "red_entry")
    );
    // The same recording against the load-time plan: the check can tell.
    let scenario =
        v2xw_engine::Scenario::load(out.join("scenario.resolved.yaml")).expect("scenario");
    let world = v2xw_engine::wiring::build_world(&scenario).expect("world");
    let stale = check_recording(&world, &out.join("recording.mcap"), &opts).expect("check");
    assert!(
        row(&stale, "lamp_phase_unknown") > 0,
        "the rings' lamps must not match the load-time plan"
    );
}

/// An events-level recording has no kinematics: every row would read 0 and pass, a check that
/// could not fail. It is refused, naming why.
#[test]
fn check_traffic_refuses_a_recording_with_no_traffic_in_it() {
    let out = std::env::temp_dir()
        .join("v2xw-cli-tests")
        .join("check-traffic-events");
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
        duration_s: Some(5.0),
        rate_veh_per_h: Some(3000.0),
        veremi: None,
        city: None,
        json: false,
        long: v2xw_cli::run::LongRun {
            record_level: v2xw_cli::stream::RecordLevel::Events,
            ..v2xw_cli::run::LongRun::default()
        },
    })
    .expect("the run");
    let refused = check_traffic(&CheckOptions::new(&out)).expect_err("nothing to check");
    assert!(
        refused.to_string().contains("no traffic to check"),
        "{refused}"
    );
}
