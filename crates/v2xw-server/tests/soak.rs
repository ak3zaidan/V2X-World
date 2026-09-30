//! The live server over a long run: what it keeps is bounded by what is alive.
//!
//! The long soak through the page (`ui/apps/studio/e2e-engine/soak.spec.ts`) found the
//! server's footprint growing for as long as vehicles came and went: every vehicle that had
//! ever driven kept its message-feed log (with the capacity of up to 4,096 receptions), its
//! security rows, and the link history of every pair it had been part of. Those stores are
//! now pruned behind the stream, and `run.status` reports their sizes (`engine.stores`).
//!
//! * `the_projector_keeps_what_is_alive_not_what_has_been` runs a small grid whose fleet
//!   turns over several times and holds the stores to the live population, in the suite.
//! * `soak_probe` prints the stores and the process's resident size every
//!   `V2XW_SOAK_EVERY_S` simulated seconds of any scenario (`V2XW_SOAK_SCENARIO`), to find
//!   what grows:
//!   `V2XW_SOAK_SCENARIO=… cargo test -p v2xw-server --test soak -- --ignored --nocapture soak_probe`

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use v2xw_server::Run;
use v2xw_server::live::{LiveEngine, LiveOptions};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<crate>/..")
        .to_path_buf()
}

/// `phase1-grid.yaml` on a `cols` × `rows` grid for `seconds` at 3,000 veh/h: trips of a
/// minute or two across the grid, so over a few simulated minutes the fleet turns over.
fn churn_scenario(cols: u32, rows: u32, seconds: u32) -> PathBuf {
    let source = repo_root().join("scenarios/phase1-grid.yaml");
    let text = std::fs::read_to_string(&source)
        .expect("read phase1-grid.yaml")
        .replace("duration_s: 60.0", &format!("duration_s: {seconds}.0"))
        .replace("rate_veh_per_h: 30.0", "rate_veh_per_h: 3000.0")
        .replace("      cols: 13\n", &format!("      cols: {cols}\n"))
        .replace("      rows: 34\n", &format!("      rows: {rows}\n"));
    let dir = std::env::temp_dir().join("v2xw-server-soak");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join(format!("churn-{cols}x{rows}.yaml"));
    std::fs::write(&path, text).expect("write scenario");
    path
}

fn serve(path: &Path, retain_bytes: usize) -> Arc<Run> {
    let engine = LiveEngine::open(
        path,
        LiveOptions {
            build_utc: "2026-09-30T00:00:00Z".to_string(),
            paused: false,
            speed: 0.0,
            retain_bytes,
            ..LiveOptions::default()
        },
    )
    .expect("build");
    let world_json = engine.world_json().to_string();
    Run::new(Box::new(engine), world_json).expect("run")
}

/// This process's resident size, MB, from `ps` (a probe's reading, not an assertion's).
fn rss_mb() -> Option<f64> {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()?;
    let kb: f64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
    Some(kb / 1024.0)
}

/// Streams the run to its end, calling `sample(t_s, diagnostics)` every `every_s`
/// simulated seconds and once at the end.
fn stream(run: &Run, step_s: f64, every_s: f64, mut sample: impl FnMut(f64, &Value)) {
    let mut steps = 0u64;
    let mut next = every_s;
    for _ in 0..10_000_000 {
        match run.tick() {
            Ok(true) => {
                steps += 1;
                let t = steps as f64 * step_s;
                if t + 1e-9 >= next {
                    next += every_s;
                    sample(t, &run.diagnostics());
                }
            }
            Ok(false) => {
                if run.state() == v2xw_server::RunState::Finished {
                    break;
                }
            }
            Err(e) => panic!("the engine aborted: {e}"),
        }
    }
    sample(steps as f64 * step_s, &run.diagnostics());
}

fn n(v: &Value, key: &str) -> u64 {
    v["stores"][key]
        .as_u64()
        .unwrap_or_else(|| panic!("run.status engine.stores.{key}: {v}"))
}

/// A fleet that turns over several times leaves the server holding what is alive: the
/// message feed's logs and the security rows are no more than the live radios plus those
/// that left within the last 40 simulated seconds (the 30 s grace, and the projector's lead
/// over the stream), and the link histories are pairs of those. Before the pruning each of
/// these grew with every vehicle that had ever driven.
///
/// The bound is computed from the reconstruction's own counts (radios live, radios ever),
/// sampled every 5 s, and not from the pruning's bookkeeping, so a pruning that stopped
/// working cannot also move the bound it is held to.
#[test]
fn the_projector_keeps_what_is_alive_not_what_has_been() {
    // A 2 × 2 grid: short trips, so the fleet turns over within the run while it stays small
    // enough for a debug build (the 3 × 3 grid needed 300 s and six minutes of wall clock).
    let run = serve(&churn_scenario(2, 2, 240), 64 * 1024 * 1024);
    let mut samples: Vec<(f64, Value)> = Vec::new();
    stream(&run, 0.1, 5.0, |t, d| samples.push((t, d.clone())));
    let (t_end, last) = samples.last().expect("a sample");
    assert!(*t_end >= 239.9, "the run reached its end: {t_end}");
    let departed = |d: &Value| n(d, "nodes_ever") - n(d, "nodes_live");
    // The precondition: the fleet did turn over, so a store that kept every node would be
    // well above the live population (measured on this grid: 45 radios ever, 15 live).
    let (live, ever) = (n(last, "nodes_live"), n(last, "nodes_ever"));
    assert!(
        ever >= live + 20,
        "the fleet did not turn over (ever {ever}, live {live}): {last}"
    );
    // A node's first step can name it before its actor is mapped; a handful of slack.
    let slack = 4;
    let mut checked = 0;
    for (t, d) in &samples {
        if *t < 60.0 {
            continue;
        }
        let Some((_, then)) = samples.iter().rev().find(|(u, _)| *u <= t - 40.0) else {
            continue;
        };
        let recent = departed(d) - departed(then);
        let bound = n(d, "nodes_live") + recent + slack;
        for key in ["feed_nodes", "security_nodes"] {
            let held = n(d, key);
            assert!(
                held <= bound,
                "t = {t:.0} s: {key} {held} is more than the {} live radios and the {recent} \
                 that left in the last 40 s ({} ever): {d}",
                n(d, "nodes_live"),
                n(d, "nodes_ever")
            );
        }
        let pairs = n(d, "link_pairs");
        assert!(
            pairs <= bound * bound,
            "t = {t:.0} s: {pairs} link histories among {bound} radios alive or recently \
             retired ({} ever): {d}",
            n(d, "nodes_ever")
        );
        checked += 1;
    }
    assert!(checked > 20, "{checked} samples checked");
    for (t, d) in samples.iter().filter(|(t, _)| (*t as u64) % 30 == 0) {
        eprintln!("t={t:.0} s stores {}", d["stores"]);
    }
}

/// Prints the stores and the resident size over any scenario, to find what grows.
#[test]
#[ignore = "a probe: set V2XW_SOAK_SCENARIO (and V2XW_SOAK_EVERY_S, V2XW_SOAK_RETAIN_MB)"]
fn soak_probe() {
    let path = std::env::var("V2XW_SOAK_SCENARIO")
        .map(PathBuf::from)
        .unwrap_or_else(|_| churn_scenario(3, 3, 300));
    let every: f64 = std::env::var("V2XW_SOAK_EVERY_S")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30.0);
    let retain_mb: usize = std::env::var("V2XW_SOAK_RETAIN_MB")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200);
    let run = serve(&path, retain_mb * 1024 * 1024);
    let started = std::time::Instant::now();
    let step_s = run.descriptor().cadence.mobility_step.as_secs_f64();
    stream(&run, step_s, every, |t, d| {
        eprintln!(
            "{}",
            json!({
                "t_s": (t * 10.0).round() / 10.0,
                "wall_s": started.elapsed().as_secs(),
                "rss_mb": rss_mb(),
                "retained_steps": d["retained_steps"],
                "retained_mb": d["retained_bytes"].as_u64().map(|b| b / (1024 * 1024)),
                "last_telemetry_nodes": d["last_telemetry_nodes"],
                "stores": d["stores"],
            })
        );
    });
}
