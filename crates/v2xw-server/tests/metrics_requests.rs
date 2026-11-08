//! The UI's metric requests: U5 a stable series
//! id on every catalogue row, U13 a cumulative metric's run-wide value beside its bins,
//! U14 the catalogue's cumulative marker, scheduling-window stages and linkability's
//! coverage and reference as data, U15 a sparse range answered with its samples.
//!
//! A real live run of `phase1-grid.yaml` (procedural world, 20 s), as `tests/live.rs`.
//! Values are checked against the samples the binned path holds at one mobility step a bin
//! (`Introspect::metric_series`), not against the code under test.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::Value;
use v2xw_server::engine::{Engine, Query, RunState};
use v2xw_server::introspect::{Introspect, MetricInfo};
use v2xw_server::live::{LiveEngine, LiveOptions};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<crate>/..")
        .to_path_buf()
}

/// `phase1-grid.yaml` shortened to 20 s at 6,000 veh/h (as the grouped-query test in
/// `tests/live.rs`: at 30 veh/h a 20 s run has no vehicle), written to a scratch file.
fn scenario() -> PathBuf {
    let source = repo_root().join("scenarios/phase1-grid.yaml");
    let text = std::fs::read_to_string(&source).expect("read phase1-grid.yaml");
    assert!(text.contains("duration_s: 60.0") && text.contains("rate_veh_per_h: 30.0"));
    let text = text
        .replace("duration_s: 60.0", "duration_s: 20.0")
        .replace("rate_veh_per_h: 30.0", "rate_veh_per_h: 6000.0");
    let dir = std::env::temp_dir().join("v2xw-server-metrics-requests");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let path = dir.join("metrics-requests.yaml");
    std::fs::write(&path, text).expect("write scenario");
    path
}

struct Run {
    engine: LiveEngine,
    /// Every metric sample the stream carried: `(series name, step instant, value)`.
    samples: Vec<(String, u64, f64)>,
    end: u64,
}

/// One run, drained, shared by every test (a `Mutex` because queries take `&mut`).
fn run() -> &'static std::sync::Mutex<Run> {
    static RUN: OnceLock<std::sync::Mutex<Run>> = OnceLock::new();
    RUN.get_or_init(|| {
        let options = LiveOptions {
            build_utc: "2026-09-22T00:00:00Z".to_string(),
            paused: true,
            ..LiveOptions::default()
        };
        let mut engine = LiveEngine::open(scenario(), options).expect("build");
        let mut end = 0;
        for _ in 0..20_000 {
            match engine.step().expect("step") {
                Some(step) => end = step.sim_time,
                None if engine.state() == RunState::Finished => break,
                None => continue,
            }
        }
        // Every sample, through the binned path at one mobility step (100 ms) a bin: a
        // sample lands in the bin its instant opens, one to a bin, so the non-empty bins
        // are the samples themselves.
        let mut samples = Vec::new();
        for m in engine.metric_catalogue() {
            for (t, v) in Introspect::metric_series(&engine, &m.name, 0, end, 100_000_000, 1 << 20)
            {
                if let Some(v) = v {
                    samples.push((m.name.clone(), t, v));
                }
            }
        }
        std::sync::Mutex::new(Run {
            engine,
            samples,
            end,
        })
    })
}

fn catalogue() -> Vec<MetricInfo> {
    run().lock().unwrap().engine.metric_catalogue()
}

fn series(metric: &str, from: u64, to: u64, bin: u64) -> Value {
    let mut r = run().lock().unwrap();
    r.engine
        .query(&Query::Metrics {
            metrics: vec![metric.to_string()],
            t_from_ns: Some(from),
            t_to_ns: Some(to),
            bin_ns: bin,
            group_by: Vec::new(),
            filter: Default::default(),
            limit: 10_000,
        })
        .expect("metrics.query")
}

/// **U5.** Every catalogue row has a series id, unique in the catalogue and spelled as
/// the series the stream carries: `name`, `name.p50`/`.p95`/`.p99`, `name[value]`. The
/// `metrics.catalogue` answer carries it.
///
/// Fault: the series id is the metric's base name -> red.
#[test]
fn every_series_has_a_stable_id() {
    let cat = catalogue();
    assert!(cat.len() > 20, "{} rows", cat.len());
    let mut ids = BTreeSet::new();
    for m in &cat {
        assert!(ids.insert(m.series_id.clone()), "{} twice", m.series_id);
        assert_eq!(m.series_id, m.name, "{}", m.name);
        let suffix = &m.series_id[m.base.len()..];
        assert!(
            m.series_id.starts_with(&m.base)
                && (suffix.is_empty()
                    || [".p50", ".p95", ".p99"].contains(&suffix)
                    || (suffix.starts_with('[') && suffix.ends_with(']'))),
            "{} is not a series of {}",
            m.series_id,
            m.base
        );
    }
    assert!(
        cat.iter().any(|m| m.series_id != m.base),
        "the run has percentile or breakdown series"
    );
    let mut r = run().lock().unwrap();
    let answer = r.engine.query(&Query::MetricCatalogue).expect("catalogue");
    for row in answer["catalogue"].as_array().expect("rows") {
        assert_eq!(row["series_id"], row["name"], "{row}");
    }
}

/// **U14.** The detection metrics report the run to date and are marked `cumulative`; a
/// windowed metric (`pdr`) is `windowed`. The revocation stage metric marks the two
/// transitions a schedule sets (into `report_received`: the RA's report shuffle; into
/// `published`: the CRL publication), and linkability carries its observer's coverage and
/// that it is an upper bound, as data.
///
/// Fault: the detection provider's definitions are left windowed -> red.
#[test]
fn the_catalogue_says_what_each_value_aggregates() {
    let cat = catalogue();
    let of = |base: &str| -> Vec<&MetricInfo> { cat.iter().filter(|m| m.base == base).collect() };
    for name in ["det_recall", "time_to_decision", "false_accusations"] {
        let rows = of(name);
        assert!(!rows.is_empty(), "{name} is in the catalogue");
        for m in rows {
            assert_eq!(m.aggregate, "cumulative", "{}", m.name);
        }
    }
    for m in of("pdr") {
        assert_eq!(m.aggregate, "windowed", "{}", m.name);
    }
    let rev = of("revocation_latency_stage");
    assert!(!rev.is_empty());
    for m in rev {
        let kinds: Vec<(&str, &str)> = m
            .stage_kinds
            .iter()
            .map(|(s, k)| (s.as_str(), k.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [
                ("published", "scheduling-window"),
                ("report_received", "scheduling-window")
            ]
        );
    }
    // Linkability (installed with the security path, which this scenario does not run).
    let defs =
        v2xw_metrics::MetricProvider::defs(&v2xw_engine::privacy_metrics::PrivacyProvider::new());
    let l = defs
        .iter()
        .find(|d| d.name == "linkability")
        .expect("linkability");
    assert_eq!(l.aggregate, v2xw_metrics::Aggregate::Cumulative);
    let c = l.coverage.as_ref().expect("coverage");
    assert_eq!(
        (c.observer.as_str(), c.share),
        ("global passive eavesdropper", 1.0)
    );
    assert_eq!(l.reference.as_ref().expect("reference").kind, "upper-bound");
    for d in defs.iter().filter(|d| d.name != "linkability") {
        assert_eq!(d.aggregate, v2xw_metrics::Aggregate::Windowed, "{}", d.name);
    }
}

/// **U13.** A cumulative series' answer carries `run_wide`, its newest sample at or before
/// the range's end (which is not the mean of its bins); a windowed series carries none.
/// This grid run has no attacker, so its cumulative series (the detection metrics) carry
/// no samples and their `run_wide` is null; the newest-sample read itself is checked on
/// `pdr`, which has, at the run's middle and end.
///
/// Fault: `metric_latest` answers the oldest sample -> red.
#[test]
fn a_cumulative_series_names_its_run_wide_value() {
    let (end, samples) = {
        let r = run().lock().unwrap();
        (r.end, r.samples.clone())
    };
    let mine: Vec<(u64, f64)> = samples
        .iter()
        .filter(|(n, ..)| n == "pdr")
        .map(|(_, t, v)| (*t, *v))
        .collect();
    assert!(mine.len() >= 3, "pdr has samples: {}", mine.len());
    let mid = mine[mine.len() / 2].0;
    for to in [mid, end] {
        let want = mine.iter().rev().find(|(t, _)| *t <= to).unwrap();
        let got = {
            let r = run().lock().unwrap();
            Introspect::metric_latest(&r.engine, "pdr", to).expect("a newest sample")
        };
        assert_eq!(got.0, want.0, "at {to}");
        assert!((got.1 - want.1).abs() < 1e-6, "{got:?} vs {want:?}");
    }
    // The answer names it for a cumulative series, and only for one.
    let answer = series("time_to_decision", 0, end, 1_000_000_000);
    let run_wide = answer["run_wide"].as_object().expect("run_wide");
    assert!(run_wide.contains_key("time_to_decision"), "{answer}");
    let windowed = series("pdr", 0, end, 1_000_000_000);
    assert!(
        windowed["run_wide"].get("pdr").is_none(),
        "{}",
        windowed["run_wide"]
    );
}

/// **U15.** A range of fewer than eight bins comes back as the samples themselves, at their
/// own instants (`binned: false`); eight bins or more are binned as before.
///
/// Fault: `SPARSE_BINS` is 0 (never sparse) -> red.
#[test]
fn a_sparse_range_returns_its_samples_one_by_one() {
    let (end, samples) = {
        let r = run().lock().unwrap();
        (r.end, r.samples.clone())
    };
    let name = "pdr";
    let mine: Vec<(u64, f64)> = samples
        .iter()
        .filter(|(n, ..)| n == name)
        .map(|(_, t, v)| (*t, *v))
        .collect();
    assert!(mine.len() >= 8, "{name} has samples: {}", mine.len());
    // The whole run in one 20 s bin: one bin, so every sample comes back.
    let sparse = series(name, 0, end, 20_000_000_000);
    assert_eq!(sparse["binned"], false, "{sparse}");
    let rows: Vec<(u64, Option<f64>)> = sparse["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r[0].as_u64().unwrap(), r[1].as_f64()))
        .collect();
    let want: Vec<(u64, Option<f64>)> = mine.iter().map(|(t, v)| (*t, Some(*v))).collect();
    assert_eq!(rows.len(), want.len());
    for ((t, v), (wt, wv)) in rows.iter().zip(&want) {
        assert_eq!(t, wt);
        let (a, b) = (v.unwrap(), wv.unwrap());
        assert!((a - b).abs() < 1e-6, "{t}: {a} vs {b}");
    }
    // One-second bins over the run: binned.
    let dense = series(name, 0, end, 1_000_000_000);
    assert_eq!(dense["binned"], true);
}
