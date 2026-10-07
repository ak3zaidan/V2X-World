//! A checkpoint on request (`crate::signal`): a run asked to stop — as Ctrl-C, `kill` or a
//! closing terminal ask — stops at the next simulated second with a checkpoint and every
//! output up to it complete, and the resumed run is the uninterrupted one.
//!
//! A test binary of its own, because the request is a process-wide flag.
//!
//! Shown able to fail: with the poll left out of the loop (`next_poll` never set) the run
//! ignores the request and finishes, and the `expect_err` fails.

use std::path::Path;

use v2xw_cli::run::{LongRun, RunOptions, resume, run};

#[test]
fn a_run_asked_to_stop_checkpoints_and_resumes_into_the_same_run() {
    let base = std::env::temp_dir().join("v2xw-cli-longrun-signal");
    let _ = std::fs::remove_dir_all(&base);
    let opts = |out: &Path| RunOptions {
        scenario: Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/longrun-grid.yaml"),
        out: Some(out.to_path_buf()),
        build_utc: Some("2026-10-07T00:00:00Z".to_string()),
        keyframe_ms: 1000,
        node_only: false,
        record: true,
        attach_world: false,
        verify: true,
        stream_digest: false,
        city: None,
        duration_s: Some(60.0),
        rate_veh_per_h: None,
        json: false,
        long: LongRun {
            progress_every_s: Some(10.0),
            quiet: true,
            checkpoint_every_s: Some(20.0),
            ..LongRun::default()
        },
    };
    v2xw_cli::signal::clear();
    let whole = run(&opts(&base.join("whole"))).expect("uninterrupted");

    // Asked to stop once the run is past its first checkpoint, from another thread, as a
    // signal handler would.
    let asked = base.join("asked");
    let watch = asked.join("progress.jsonl");
    let asker = std::thread::spawn(move || {
        for _ in 0..6_000 {
            let lines = std::fs::read_to_string(&watch).map_or(0, |t| t.lines().count());
            if lines >= 3 {
                v2xw_cli::signal::request();
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    });
    let err = run(&opts(&asked)).expect_err("stopped on request");
    asker.join().expect("asker");
    v2xw_cli::signal::clear();
    let at = match err {
        v2xw_cli::CliError::StoppedOnRequest { at_s, .. } => at_s,
        other => panic!("{other}"),
    };
    assert!(at >= 30.0 && at < 60.0 && (at - at.round()).abs() < 1e-9, "stopped at {at} s");
    // Every segment on disk is a finished recording: nothing half-written was left.
    for entry in std::fs::read_dir(&asked).expect("dir") {
        let p = entry.expect("entry").path();
        if p.extension().is_some_and(|e| e == "mcap") {
            v2xw_record::Reader::open_paged(&p)
                .unwrap_or_else(|e| panic!("{} is not a finished recording: {e}", p.display()));
        }
    }

    let resumed = resume(&asked.join("checkpoint.json"), true, false).expect("resumed");
    assert_eq!(resumed.record_digest, whole.record_digest);
    assert_eq!(resumed.content_digest, whole.content_digest);
    assert_eq!(
        std::fs::read(asked.join("metrics.json")).expect("metrics"),
        std::fs::read(base.join("whole").join("metrics.json")).expect("metrics")
    );
}
