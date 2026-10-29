//! `traffic.emergency_yield` reaches the run (WP-20, P20.5).
//!
//! - The expectation row of `tests/expectations/traffic.yaml` holds on its family
//!   (`scenarios/sensitivity/catalogue.yaml`, `emergency`: the procedural base of
//!   `emergency-priority.yaml`, one vehicle in ten an emergency vehicle, 120 s): with
//!   yielding on, the emergency vehicles drive further in the same window
//!   (`report.emergency_distance_m`).
//! - Naming the block with yielding off changes nothing but that report leaf.
//!
//! The corridor measurement (an ambulance through a congested 1 km arterial, three seeds)
//! is `crates/v2xw-mobility/tests/emergency_yield.rs`.

use std::path::{Path, PathBuf};

use v2xw_engine::Scenario;
use v2xw_engine::scenario::schema::EmergencyYieldSettings;
use v2xw_engine::sensitivity::{expect, sweep};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
fn the_emergency_yield_row_holds() {
    let path = root().join("scenarios/sensitivity/catalogue.yaml");
    let fam = sweep::Families::load(&path).expect("families file loads");
    let mut exp = sweep::load_expectations(&[root().join("crates/v2xw-engine/tests/expectations")])
        .expect("rows parse");
    exp.retain(|e| e.setting.starts_with("traffic.emergency_yield"));
    assert_eq!(exp.len(), 1, "the distance row");
    let (settings, unplanned) = expect::expect_plan(&fam, &exp);
    assert!(unplanned.is_empty(), "{unplanned:?}");
    let bases = fam.bases(&path).expect("bases load");
    let result =
        sweep::run_cached(settings, vec![], &bases, &exp, 1, None, |_, _, _| {}).expect("runs");
    for v in &result.verdicts {
        eprintln!(
            "{} {}: {:?} -> {:?} ({})",
            v.expectation.setting, v.expectation.metric, v.base, v.value, v.note
        );
    }
    assert_eq!(result.verdicts.len(), 1);
    let failed: Vec<&sweep::Verdict> = result.verdicts.iter().filter(|v| !v.pass).collect();
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn naming_the_block_off_changes_nothing_but_the_report_leaf() {
    let mut s =
        Scenario::load(root().join("scenarios/connected-intersections.yaml")).expect("loads");
    s.time.duration_s = 60.0;
    let mut named = s.clone();
    named
        .traffic
        .get_or_insert_with(Default::default)
        .emergency_yield = Some(EmergencyYieldSettings {
        enabled: Some(false),
        ..EmergencyYieldSettings::default()
    });
    let run = |sc: Scenario| {
        let mut engine = v2xw_engine::Engine::build(sc, "").expect("builds");
        let mut rec = v2xw_engine::DigestRecorder::new();
        let report = engine.run(&mut rec).expect("runs");
        (report, rec.digest_hex(), rec.frame_digest_hex())
    };
    let (plain, plain_digest, plain_frames) = run(s);
    let (mut with, with_digest, with_frames) = run(named);
    assert_eq!(plain.emergency_distance_m, 0, "reported without the block");
    assert!(
        with.emergency_distance_m > 0,
        "the emergency vehicles drove nowhere"
    );
    assert_eq!(
        plain_digest, with_digest,
        "naming the block off moved the records"
    );
    assert_eq!(
        plain_frames, with_frames,
        "naming the block off moved the frames"
    );
    with.emergency_distance_m = 0;
    assert_eq!(plain, with, "naming the block off moved the report");
}
