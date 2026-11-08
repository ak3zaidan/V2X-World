//! `v2xw check-traffic` on a run driven by the queue tier (scenarios/tests/queue-tier.yaml):
//! the queue tier has no car-following kinematics, so the microscopic rows must not judge it.

use std::path::{Path, PathBuf};

use v2xw_cli::check_traffic::{CheckOptions, check_recording, check_traffic};
use v2xw_cli::run::{RunOptions, run};

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

#[test]
fn the_queue_tier_is_not_judged_by_the_microscopic_rows() {
    let out = std::env::temp_dir()
        .join("v2xw-cli-tests")
        .join("check-traffic-queue-tier");
    let _ = std::fs::remove_dir_all(&out);
    run(&RunOptions {
        scenario: repo_root().join("scenarios/tests/queue-tier.yaml"),
        out: Some(out.clone()),
        build_utc: Some("2026-11-02T00:00:00Z".to_string()),
        keyframe_ms: 1000,
        node_only: false,
        record: true,
        attach_world: false,
        verify: false,
        stream_digest: false,
        duration_s: None,
        rate_veh_per_h: None,
        veremi: None,
        city: None,
        json: false,
        long: v2xw_cli::run::LongRun::default(),
    })
    .expect("the run");
    let opts = CheckOptions::new(&out);
    let scoped = check_traffic(&opts).expect("the check");
    assert!(
        scoped.queue_tier_samples > 0,
        "no queue-tier sample counted"
    );
    for name in ["jump", "overlap", "decel", "jerk", "red_entry"] {
        assert_eq!(row(&scoped, name), 0, "{name} judged a queue-tier vehicle");
    }
    // The same recording judged as if every vehicle were microscopic: the queue model's
    // own link-to-link moves are what the rows flagged before.
    let scenario =
        v2xw_engine::Scenario::load(out.join("scenario.resolved.yaml")).expect("scenario");
    let world = v2xw_engine::wiring::build_world(&scenario).expect("world");
    let unscoped = check_recording(&world, &out.join("recording.mcap"), &opts).expect("check");
    assert_eq!(unscoped.queue_tier_samples, 0);
    assert!(
        row(&unscoped, "jump") + row(&unscoped, "decel") + row(&unscoped, "overlap") > 0,
        "the queue run shows no link-to-link move: the scope test proves nothing"
    );
}
