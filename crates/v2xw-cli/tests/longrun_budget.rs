//! A long run's budget, on a shortened run: memory that does not grow with simulated time
//! and a cost per simulated minute that does not climb.
//!
//! A test binary of its own, so the process footprint it reads is this run's and not a
//! neighbouring test's.
//!
//! The run is `tests/data/longrun-grid.yaml` (every SCMS entity, attackers, revocations,
//! every metric) for four simulated minutes, with a progress line every 30 s. From the
//! second minute on — the fleet has filled the grid by then — it asserts:
//!
//! * **memory**: the footprint grows by less than the budget between the end of the second
//!   minute and the end of the fourth;
//! * **the engine's stores**: every collection `Engine::census` reports that holds work in
//!   flight (the event heap, pending receptions, frames on the air, inboxes, orphaned
//!   fates) stays within twice its size at the second minute, and the per-node stores stay
//!   within the nodes ever created;
//! * **cost**: no 30-second slice after the first minute costs more than three times the
//!   cheapest one (generous: the machine is shared, and a cost that climbs with the run —
//!   the CRL walk the soak found — climbs far past it).
//!
//! Shown able to fail: with `Tally` keeping every metric sample in a `Vec` again (the
//! defect a 20-minute Midtown run found at 957 MB), the footprint grows by about BUDGET_RED
//! and the memory assertion fails.

use std::path::Path;

use v2xw_cli::run::{LongRun, RunOptions, run};

/// The most the footprint may grow over two simulated minutes once the fleet is in, MB.
const MEMORY_BUDGET_MB: f64 = 24.0;

#[test]
fn a_long_run_holds_its_memory_and_its_cost_flat() {
    let out = std::env::temp_dir().join("v2xw-cli-longrun-budget");
    let _ = std::fs::remove_dir_all(&out);
    let outcome = run(&RunOptions {
        scenario: Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/longrun-grid.yaml"),
        out: Some(out.clone()),
        build_utc: Some("2026-10-07T00:00:00Z".to_string()),
        keyframe_ms: 1000,
        node_only: false,
        record: false,
        attach_world: false,
        verify: false,
        stream_digest: false,
        city: None,
        duration_s: Some(240.0),
        rate_veh_per_h: None,
        city: None,
        json: false,
        long: LongRun {
            progress_every_s: Some(30.0),
            quiet: true,
            ..LongRun::default()
        },
    })
    .expect("run");
    assert!(outcome.metric_samples > 10_000, "{} samples", outcome.metric_samples);

    let lines: Vec<serde_json::Value> = std::fs::read_to_string(out.join("progress.jsonl"))
        .expect("progress")
        .lines()
        .map(|l| serde_json::from_str(l).expect("json"))
        .collect();
    assert_eq!(lines.len(), 8, "a line every 30 s of 240 s");
    let at = |t: f64| {
        lines
            .iter()
            .find(|l| (l["t_s"].as_f64().unwrap_or(0.0) - t).abs() < 1e-6)
            .unwrap_or_else(|| panic!("no line at {t} s"))
    };

    // Memory.
    if let (Some(warm), Some(end)) = (
        at(120.0)["footprint_mb"].as_f64(),
        at(240.0)["footprint_mb"].as_f64(),
    ) {
        assert!(
            end - warm < MEMORY_BUDGET_MB,
            "the footprint grew {:.1} MB from 120 s ({warm:.1} MB) to 240 s ({end:.1} MB)",
            end - warm
        );
    }

    // The engine's stores.
    let census = |t: f64, k: &str| at(t)["census"][k].as_u64().unwrap_or(0);
    for k in [
        "scheduler.heap",
        "rx_pending",
        "frames",
        "inbox_frames",
        "orphaned_rx",
        "frag_groups",
        "transfers",
        "p2.in_flight",
        "p2.queue",
    ] {
        let (mid, end) = (census(120.0, k), census(240.0, k));
        assert!(end <= 2 * mid + 64, "{k}: {mid} at 120 s, {end} at 240 s");
    }
    let created = outcome.report.nodes_created;
    for k in ["nodes", "inboxes", "node_phase", "j2945_quality_at", "p2.detectors"] {
        assert!(census(240.0, k) <= created, "{k} beyond the {created} nodes created");
    }
    // Nodes that left are not kept: the live stores follow the fleet, not the run.
    assert!(
        census(240.0, "j2945_quality_at") <= census(240.0, "nodes"),
        "a retired node's J2945/1 state was kept"
    );

    // Cost.
    let slices: Vec<f64> = lines
        .iter()
        .filter(|l| l["t_s"].as_f64().unwrap_or(0.0) > 60.0)
        .filter_map(|l| l["slice_wall_s"].as_f64())
        .collect();
    let cheapest = slices.iter().copied().fold(f64::INFINITY, f64::min);
    for (i, s) in slices.iter().enumerate() {
        assert!(
            *s <= 3.0 * cheapest + 0.5,
            "slice {i} after the first minute took {s:.2} s against the cheapest {cheapest:.2} s: {slices:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&out);
}
