//! The per-device BSM timing model in the engine (`actors.vehicles.bsm_timing`, Paper 1
//! item B) and the US profile's RandomPosition settings (item C).
//!
//! * **Off reproduces the pinned run.** The I-S1 Phase 2 case (`tests/regression/
//!   i-s1-phase2-60s.json`) with the model off, and with the section present but
//!   `enabled: false`, gives the committed digest; turning the model on must move it (the
//!   fault that shows the pin can go red).
//! * **Determinism.** The same seed with the model on gives the byte-identical run; another
//!   seed, or another locked share, does not.
//! * **Settings.** The loader takes the knobs and refuses out-of-range ones.

mod regression;

use std::path::{Path, PathBuf};

use v2xw_engine::scenario::ModelChoice;
use v2xw_engine::scenario::schema::BsmTimingSettings;
use v2xw_engine::scenario::validate::validate;
use v2xw_engine::{DigestRecorder, Engine, Scenario};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// The Phase 2 path as `tests/scms_scale.rs` pins it (every third roadside unit, modeled
/// cryptography), cut to `duration_s`.
fn phase2(duration_s: f64) -> Scenario {
    let mut s = Scenario::load(root().join("scenarios").join("phase2-manhattan.yaml"))
        .expect("the shipped scenario loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root().join(&*path).to_string_lossy().into_owned();
    }
    s.actors.rsus = s
        .actors
        .rsus
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % 3 == 0)
        .map(|(_, r)| r)
        .collect();
    s.time.duration_s = duration_s;
    s.security.crypto_mode = serde_json::from_value(serde_json::json!("modeled")).expect("a mode");
    s
}

fn timing(s: &mut Scenario, enabled: bool, locked_share: Option<f64>) {
    s.actors.vehicles.bsm_timing = Some(BsmTimingSettings {
        enabled: Some(enabled),
        calibration: None,
        locked_share,
    });
}

fn run(s: Scenario) -> regression::Pinned {
    let errors = validate(&s);
    assert!(errors.is_empty(), "the test scenario must load: {errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = DigestRecorder::new();
    engine.run(&mut rec).expect("runs");
    regression::Pinned {
        record_digest: rec.digest_hex(),
        frame_digest: rec.frame_digest_hex(),
        records: rec.written(),
    }
}

#[test]
fn the_model_off_reproduces_the_pinned_run_and_on_moves_it() {
    let off = run(phase2(60.0));
    regression::check("i-s1-phase2-60s", &off);
    let mut disabled = phase2(60.0);
    timing(&mut disabled, false, Some(0.5));
    let disabled = run(disabled);
    assert_eq!(disabled.record_digest, off.record_digest, "enabled: false is off");
    assert_eq!(disabled.frame_digest, off.frame_digest);
    // The fault: the model on must change the run, or the pin above proves nothing about it.
    let mut on = phase2(60.0);
    timing(&mut on, true, None);
    let on = run(on);
    assert_ne!(on.record_digest, off.record_digest, "the model on left every record alone");
}

#[test]
fn the_same_seed_gives_the_same_run_with_the_model_on() {
    let on = |seed_delta: u64, share: Option<f64>| {
        let mut s = phase2(20.0);
        s.seed += seed_delta;
        timing(&mut s, true, share);
        run(s)
    };
    let a = on(0, None);
    let b = on(0, None);
    assert_eq!(a.record_digest, b.record_digest, "same seed, model on: records differ");
    assert_eq!(a.frame_digest, b.frame_digest, "same seed, model on: frames differ");
    assert_eq!(a.records, b.records);
    // Shown red: another seed, or every device locked, is another run.
    assert_ne!(on(1, None).record_digest, a.record_digest);
    assert_ne!(on(0, Some(1.0)).record_digest, a.record_digest);
}

#[test]
fn the_settings_load_and_out_of_range_ones_are_refused() {
    let mut s = phase2(10.0);
    timing(&mut s, true, Some(0.5));
    assert!(validate(&s).is_empty(), "{:?}", validate(&s));
    timing(&mut s, true, Some(1.5));
    assert!(!validate(&s).is_empty(), "a share of 1.5 was accepted");
    s.actors.vehicles.bsm_timing = Some(BsmTimingSettings {
        enabled: Some(true),
        calibration: Some("/nonexistent/bsm-timing.yaml".to_string()),
        locked_share: None,
    });
    assert!(!validate(&s).is_empty(), "an unreadable calibration was accepted");
    // The US profile's RandomPosition choices, by name.
    let mut us = phase2(10.0);
    us.detection.local = vec![ModelChoice {
        id: "detect/us-sm-mbr".to_string(),
        params: serde_json::json!({"reading": "asn1_v1_1", "time_base": "secmark", "max_gap_s": 0.2}),
    }];
    assert!(validate(&us).is_empty(), "{:?}", validate(&us));
    us.detection.local[0].params = serde_json::json!({"reading": "v2_0"});
    assert!(!validate(&us).is_empty(), "an unknown reading was accepted");
}
