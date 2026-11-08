//! A long run's budget, on a shortened run: memory that does not grow with simulated time
//! and a cost per simulated minute that does not climb.
//!
//! A test binary of its own, so the process footprint it reads is this run's and not a
//! neighbouring test's.
//!
//! The run is `tests/data/longrun-grid.yaml` (every SCMS entity, attackers, revocations,
//! every metric) for eight simulated minutes, with a progress line every 30 s. Once the
//! fleet has filled the grid it asserts:
//!
//! * **the fleet**: it is in by [`WARM_S`] — the nodes live at the end are within
//!   [`FLEET_SLACK`] of those live then — so the windows below measure the run's length and
//!   not its fleet growing;
//! * **memory**: the footprint grows by less than the budget between [`WARM_S`] and the
//!   end, two simulated minutes later;
//! * **the engine's stores**: every collection `Engine::census` reports that holds work in
//!   flight (the event heap, pending receptions, frames on the air, inboxes, orphaned
//!   fates) stays within twice its size at [`WARM_S`], and the per-node stores stay
//!   within the nodes ever created;
//! * **cost**: no 30-second slice after [`FILLED_S`] costs more than three times the
//!   cheapest one (generous: the machine is shared, and a cost that climbs with the run —
//!   the CRL walk the soak found — climbs far past it).
//!
//! Shown able to fail: run for 240 s (windows 120-240 s, as the test once read them), the
//! fleet assertion fails ("19 nodes at 120 s, 43 at 240 s"). The `Tally`-in-a-`Vec` fault
//! (the defect a 20-minute Midtown run found at 957 MB) has not yet been measured against
//! the new windows.

use std::path::Path;

use v2xw_cli::run::{LongRun, RunOptions, run};

/// The most the footprint may grow over two simulated minutes once the fleet is in, MB.
const MEMORY_BUDGET_MB: f64 = 24.0;

/// The run's length, simulated seconds.
const DURATION_S: f64 = 480.0;

/// When the fleet is in, simulated seconds: the grid's 900 veh/h Poisson demand fills it in
/// about 270 s (nodes 8, 19, 43, 47 and then 46-49 at 30, 120, 240, 270 and 300-480 s).
/// The test once read its windows from 120 s to 240 s, where the fleet grew from 19 nodes to
/// 43 and took the footprint (+24.0 MB) and the slice cost (2.7 s to 11 s) up with it.
const FILLED_S: f64 = 300.0;

/// The start of the two-minute memory and store window, simulated seconds.
const WARM_S: f64 = DURATION_S - 120.0;

/// How much larger the live fleet may be at the end than at [`WARM_S`].
const FLEET_SLACK: f64 = 1.25;

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
        duration_s: Some(DURATION_S),
        rate_veh_per_h: None,
        veremi: None,
        json: false,
        long: LongRun {
            progress_every_s: Some(30.0),
            quiet: true,
            ..LongRun::default()
        },
    })
    .expect("run");
    assert!(
        outcome.metric_samples > 10_000,
        "{} samples",
        outcome.metric_samples
    );

    let lines: Vec<serde_json::Value> = std::fs::read_to_string(out.join("progress.jsonl"))
        .expect("progress")
        .lines()
        .map(|l| serde_json::from_str(l).expect("json"))
        .collect();
    assert_eq!(
        lines.len(),
        (DURATION_S / 30.0) as usize,
        "a line every 30 s of {DURATION_S} s"
    );
    let at = |t: f64| {
        lines
            .iter()
            .find(|l| (l["t_s"].as_f64().unwrap_or(0.0) - t).abs() < 1e-6)
            .unwrap_or_else(|| panic!("no line at {t} s"))
    };

    let census = |t: f64, k: &str| at(t)["census"][k].as_u64().unwrap_or(0);

    // The fleet is in: a window over a growing fleet measures the fleet.
    let (fleet_warm, fleet_end) = (census(WARM_S, "nodes"), census(DURATION_S, "nodes"));
    assert!(
        fleet_end as f64 <= FLEET_SLACK * fleet_warm as f64,
        "the fleet was still growing: {fleet_warm} nodes at {WARM_S} s, {fleet_end} at {DURATION_S} s"
    );

    eprintln!(
        "fleet {fleet_warm} -> {fleet_end} nodes; footprint {:?} -> {:?} MB ({WARM_S} -> {DURATION_S} s)",
        at(WARM_S)["footprint_mb"].as_f64(),
        at(DURATION_S)["footprint_mb"].as_f64()
    );

    // Memory.
    if let (Some(warm), Some(end)) = (
        at(WARM_S)["footprint_mb"].as_f64(),
        at(DURATION_S)["footprint_mb"].as_f64(),
    ) {
        assert!(
            end - warm < MEMORY_BUDGET_MB,
            "the footprint grew {:.1} MB from {WARM_S} s ({warm:.1} MB) to {DURATION_S} s ({end:.1} MB)",
            end - warm
        );
    }

    // The engine's stores.
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
        let (mid, end) = (census(WARM_S, k), census(DURATION_S, k));
        assert!(
            end <= 2 * mid + 64,
            "{k}: {mid} at {WARM_S} s, {end} at {DURATION_S} s"
        );
    }
    let created = outcome.report.nodes_created;
    for k in [
        "nodes",
        "inboxes",
        "node_phase",
        "j2945_quality_at",
        "p2.detectors",
    ] {
        assert!(
            census(DURATION_S, k) <= created,
            "{k} beyond the {created} nodes created"
        );
    }
    // Nodes that left are not kept: the live stores follow the fleet, not the run.
    assert!(
        census(DURATION_S, "j2945_quality_at") <= census(DURATION_S, "nodes"),
        "a retired node's J2945/1 state was kept"
    );

    // Cost.
    let slices: Vec<f64> = lines
        .iter()
        .filter(|l| l["t_s"].as_f64().unwrap_or(0.0) > FILLED_S)
        .filter_map(|l| l["slice_wall_s"].as_f64())
        .collect();
    let cheapest = slices.iter().copied().fold(f64::INFINITY, f64::min);
    for (i, s) in slices.iter().enumerate() {
        assert!(
            *s <= 3.0 * cheapest + 0.5,
            "slice {i} after {FILLED_S} s took {s:.2} s against the cheapest {cheapest:.2} s: {slices:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&out);
}
