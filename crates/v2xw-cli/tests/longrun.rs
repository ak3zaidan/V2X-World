//! Long runs: checkpoint and resume, and the outputs a run of days keeps on disk.
//!
//! The scenario is `tests/data/longrun-grid.yaml`: the credential-lifecycle grid (every SCMS
//! entity, attackers, revocations, the CRL) with every metric, shortened to 90 s.
//!
//! # Each check is shown to be capable of failing
//!
//! * The resume test's equality is paired with two controls: the digest at the checkpoint
//!   is not the digest at the end (so equal digests are not equal because nothing moved),
//!   and a checkpoint whose recorded digest is altered is refused (so the replay check is
//!   a check). With `MetricsStream::resume` not cutting the file back to the checkpoint's
//!   length, the metrics comparison fails: the samples between the checkpoint and the
//!   crash appear twice.
//! * The segmented recording's content digest is compared with an unsegmented run's.

use std::path::{Path, PathBuf};

use v2xw_cli::run::{LongRun, RunOptions, resume, run};
use v2xw_cli::stream::RecordLevel;

fn scenario() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/longrun-grid.yaml")
}

fn out_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("v2xw-cli-longrun").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn options(out: &Path, long: LongRun) -> RunOptions {
    RunOptions {
        scenario: scenario(),
        out: Some(out.to_path_buf()),
        build_utc: Some("2026-10-07T00:00:00Z".to_string()),
        keyframe_ms: 1000,
        node_only: false,
        record: true,
        attach_world: false,
        verify: true,
        duration_s: None,
        rate_veh_per_h: None,
        city: None,
        json: false,
        long,
    }
}

fn checkpointed(stop_at_s: Option<f64>) -> LongRun {
    LongRun {
        progress_every_s: Some(15.0),
        quiet: true,
        checkpoint_every_s: Some(30.0),
        record_level: RecordLevel::Full,
        stop_at_s,
    }
}

/// The manifest's file digests, less the recordings': an MCAP file's own bytes are not
/// reproducible (its summary section is written from a `HashMap`; `crate::run`'s module
/// note), which is why the content digest exists.
fn file_digests(dir: &Path) -> Vec<(String, String)> {
    let m: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).expect("manifest"))
            .expect("json");
    m["files"]
        .as_array()
        .expect("files")
        .iter()
        .map(|f| {
            (
                f["path"].as_str().unwrap_or_default().to_string(),
                f["sha256"].as_str().unwrap_or_default().to_string(),
            )
        })
        .filter(|(p, _)| !p.ends_with(".mcap"))
        .collect()
}

fn read(dir: &Path, name: &str) -> Vec<u8> {
    std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{}: {e}", dir.join(name).display()))
}

/// A run interrupted after its first checkpoint and resumed from it finishes with exactly the
/// outputs of the run that was never interrupted: the record digest over every record, the
/// content digest over every recorded message, `metrics.json`, `run-report.json` and the
/// manifest's data digest — and the same as a run recorded in one file without checkpoints.
#[test]
fn a_resumed_run_is_the_run_that_was_never_interrupted() {
    let whole_dir = out_dir("whole");
    let whole = run(&options(&whole_dir, checkpointed(None))).expect("uninterrupted run");
    assert!(
        whole.recording_segments.len() >= 3,
        "90 s at a checkpoint every 30 s records three segments: {:?}",
        whole.recording_segments
    );

    // Interrupted at 45 s, fifteen seconds after its 30 s checkpoint, as a crash leaves it:
    // the metrics and the progress of those fifteen seconds are on disk, the segment being
    // written is never finished.
    let cut_dir = out_dir("cut");
    let err = run(&options(&cut_dir, checkpointed(Some(45.0)))).expect_err("stopped");
    assert!(
        matches!(err, v2xw_cli::CliError::Interrupted { at_s } if (at_s - 45.0).abs() < 1e-6),
        "{err}"
    );
    let checkpoint = cut_dir.join("checkpoint.json");
    let ck: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&checkpoint).expect("checkpoint")).expect("json");
    assert_eq!(ck["segments_done"], 1);
    assert!(
        std::fs::metadata(cut_dir.join("metrics.json")).expect("metrics").len()
            > ck["metrics_bytes"].as_u64().expect("metrics_bytes"),
        "the crash left metrics past the checkpoint, which the resume must cut"
    );

    // Control: a checkpoint whose digest does not match what the rebuilt run reaches is
    // refused, and refused before any output is touched.
    let forged_dir = out_dir("forged");
    std::fs::create_dir_all(&forged_dir).expect("dir");
    for entry in std::fs::read_dir(&cut_dir).expect("cut dir") {
        let p = entry.expect("entry").path();
        std::fs::copy(&p, forged_dir.join(p.file_name().expect("name"))).expect("copy");
    }
    let mut forged = ck.clone();
    forged["record_digest"] = serde_json::json!("00".repeat(32));
    std::fs::write(
        forged_dir.join("checkpoint.json"),
        serde_json::to_vec(&forged).expect("json"),
    )
    .expect("write");
    let metrics_before = read(&forged_dir, "metrics.json");
    let refused = resume(&forged_dir.join("checkpoint.json"), true, false).expect_err("refused");
    assert!(
        matches!(refused, v2xw_cli::CliError::Diverged { .. }),
        "{refused}"
    );
    assert_eq!(
        read(&forged_dir, "metrics.json"),
        metrics_before,
        "a refused resume left the outputs as they were"
    );

    let resumed = resume(&checkpoint, true, false).expect("resumed");
    assert_eq!(resumed.resumed.map(|r| r.from_s.round()), Some(30.0));

    // Control: the digest moved between the checkpoint and the end, so the equalities below
    // are about the whole run.
    assert_ne!(
        ck["record_digest"].as_str(),
        whole.record_digest.as_deref(),
        "the checkpoint's digest is the end's: nothing ran after it"
    );
    assert_eq!(resumed.record_digest, whole.record_digest, "record digest");
    assert_eq!(resumed.content_digest, whole.content_digest, "content digest");
    assert!(whole.content_digest.is_some());
    assert_eq!(resumed.metric_samples, whole.metric_samples);
    assert_eq!(resumed.recording_segments, whole.recording_segments);
    assert_eq!(
        read(&cut_dir, "metrics.json"),
        read(&whole_dir, "metrics.json"),
        "metrics.json"
    );
    assert_eq!(
        read(&cut_dir, "run-report.json"),
        read(&whole_dir, "run-report.json"),
        "run-report.json"
    );
    let digests = file_digests(&whole_dir);
    assert_eq!(digests.len(), 3, "scenario, metrics and report: {digests:?}");
    assert_eq!(file_digests(&cut_dir), digests, "the manifest's file digests");
    // The progress file carries on rather than starting again.
    let lines = |d: &Path| {
        String::from_utf8(read(d, "progress.jsonl"))
            .expect("utf8")
            .lines()
            .map(|l| serde_json::from_str::<serde_json::Value>(l).expect("json")["t_s"].as_f64())
            .collect::<Vec<_>>()
    };
    assert_eq!(lines(&cut_dir), lines(&whole_dir), "progress lines");

    // One recording in one file, no checkpoints: the same messages, the same digest.
    let single_dir = out_dir("single");
    let single = run(&options(&single_dir, LongRun::default())).expect("single-file run");
    assert!(single.recording_segments.is_empty());
    assert_eq!(single.record_digest, whole.record_digest);
    assert_eq!(single.content_digest, whole.content_digest);
    assert_eq!(read(&single_dir, "metrics.json"), read(&whole_dir, "metrics.json"));
}

/// The metrics level keeps the samples and the run's summary records and drops the scene:
/// the recording is a fraction of the full one, the record digest (what the engine emitted)
/// is unchanged, and `metrics.json` is identical.
#[test]
fn the_metrics_level_records_a_fraction_and_changes_nothing_computed() {
    let full_dir = out_dir("level-full");
    let full = run(&options(&full_dir, LongRun::default())).expect("full");
    let lean_dir = out_dir("level-metrics");
    let lean = run(&options(
        &lean_dir,
        LongRun {
            record_level: RecordLevel::Metrics,
            ..LongRun::default()
        },
    ))
    .expect("metrics level");
    assert_eq!(lean.record_digest, full.record_digest);
    assert_eq!(read(&lean_dir, "metrics.json"), read(&full_dir, "metrics.json"));
    let (f, l) = (
        full.recording_bytes.expect("bytes"),
        lean.recording_bytes.expect("bytes"),
    );
    assert!(l * 5 < f, "metrics level {l} B against full {f} B");
    assert!(lean.verified.as_ref().is_some_and(|v| v.records > 0 && v.frames == 0));
}
