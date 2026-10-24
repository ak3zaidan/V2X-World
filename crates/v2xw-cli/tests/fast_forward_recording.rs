//! A real-calendar run with a `time.fast_forward` window records and reads back.
//!
//! Found by the SCMS effects round (docs/design/scms-real/testing/effects.md): every
//! `v2xw run` of `credential-weeks.yaml` (or any fast-forward scenario) wrote its whole
//! recording and then failed the read-back with "195301000000000 ns between keyframes
//! exceeds the 1000000000 ns cadence", so it left no `run-report.json`. Inside a window
//! the world is frozen and no snapshot is written, which is what the manifest's
//! `time_dilation_windows` declare; the reader now accepts a gap across a declared window
//! and still refuses one anywhere else.
//!
//! Faults that turn it red (each injected with `tools/mutate/mutate.py` and reverted):
//! `declared_skip` returning `false` (the keyframe check refuses the window), and the
//! snapshot-gap check ignoring `declared_skip` (the mobility-step check refuses it).

use std::path::{Path, PathBuf};

use v2xw_cli::run::{RunOptions, run};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate lives two levels below the workspace root")
        .to_path_buf()
}

fn options(scenario: PathBuf, out: PathBuf) -> RunOptions {
    RunOptions {
        scenario,
        out: Some(out),
        build_utc: Some("2026-10-18T00:00:00Z".to_string()),
        keyframe_ms: 1000,
        node_only: false,
        record: true,
        attach_world: false,
        verify: true,
        stream_digest: false,
        duration_s: None,
        rate_veh_per_h: None,
        veremi: None,
        city: None,
        json: false,
        long: v2xw_cli::run::LongRun::default(),
    }
}

#[test]
fn a_fast_forward_run_writes_a_recording_that_verifies() {
    let dir = std::env::temp_dir()
        .join("v2xw-cli-tests")
        .join("fast-forward-recording");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let base = repo_root().join("scenarios/credential-weeks.yaml");
    // 40 s of radio, 1,200 s skipped (across the 614 -> 615 rollover at 600 s), 20 s of
    // radio after it.
    let file = dir.join("ff.yaml");
    std::fs::write(
        &file,
        format!(
            "schema: v2xw/scenario/1\n\
             meta: {{name: ff-recording, base: {}}}\n\
             time: {{duration_s: 1260.0, fast_forward: [{{from_s: 40.0, to_s: 1240.0}}]}}\n\
             actors: {{vehicles: {{demand: {{rate_veh_per_h: 300.0}}}}}}\n\
             threats: {{attackers: []}}\n",
            base.display()
        ),
    )
    .expect("write");
    let out = run(&options(file, dir.join("out"))).expect("the run and its read-back succeed");
    assert_eq!(out.report.fast_forward.windows, 1, "the window was taken");
    let v = out.verified.as_ref().expect("the recording was read back");
    assert!(v.frames > 0 && v.records > 0, "{v:?}");
}
