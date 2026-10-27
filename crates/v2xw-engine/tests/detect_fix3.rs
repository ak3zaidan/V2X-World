//! detect-fix3: the ETSI TS 103 759 suite hosted in `detection.local` (`detect/ts103759`).
//!
//! * **Settings.** The loader takes the suite with its card's parameters, its labels and
//!   the host keys, and refuses an unknown key, an unknown label and the suite listed
//!   beside another.
//! * **Inputs.** On a live run every message is judged, the box check runs on each BSM's
//!   J2735 VehicleSize and the receiver's own footprint (WP-10 left it unchecked), and the
//!   run report tallies each check per sender class. Shown red by turning the frame join
//!   off (`Phase2::wants_frame_extras` false): `boxIntersection` is then never evaluated.

use std::path::Path;

use serde_json::json;
use v2xw_engine::scenario::ModelChoice;
use v2xw_engine::scenario::validate::validate;
use v2xw_engine::{Engine, NullRecorder, Scenario};

fn phase2(duration_s: f64) -> Scenario {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let mut s = Scenario::load(root.join("scenarios").join("phase2-manhattan.yaml"))
        .expect("the shipped scenario loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root.join(&*path).to_string_lossy().into_owned();
    }
    s.time.duration_s = duration_s;
    for a in &mut s.threats.attackers {
        if let Some(w) = a.schedule.as_mut() {
            w.to_s = w.to_s.min(duration_s);
        }
    }
    s
}

fn ts(params: serde_json::Value) -> ModelChoice {
    ModelChoice {
        id: "detect/ts103759".to_string(),
        params,
    }
}

#[test]
fn the_ts_suite_is_selectable_with_its_knobs() {
    let mut s = phase2(10.0);
    s.detection.local = vec![ts(json!({
        "max_confidence_m": 12.0,
        "kalman_gate_z": 3.5,
        "environment": "urban",
        "beacon_profile": "bsm-j2945-1",
        "fusion": "leaky-bucket",
        "scoring": "catch",
        "report_interval_s": 2.0,
        "use_map": 1
    }))];
    assert!(validate(&s).is_empty(), "{:?}", validate(&s));
    s.detection.local = vec![ts(json!({"max_confidence": 12.0}))];
    assert!(!validate(&s).is_empty(), "a misspelt key was accepted");
    s.detection.local = vec![ts(json!({"fusion": "majority"}))];
    assert!(!validate(&s).is_empty(), "an unknown fusion label was accepted");
    s.detection.local = vec![ts(json!({"environment": "canyon"}))];
    assert!(!validate(&s).is_empty(), "an unknown environment was accepted");
    s.detection.local = vec![
        ts(serde_json::Value::Null),
        ModelChoice {
            id: "detect/legacy-12".to_string(),
            params: serde_json::Value::Null,
        },
    ];
    assert!(!validate(&s).is_empty(), "the suite was accepted beside another");
}

#[test]
fn the_ts_suite_judges_every_message_with_its_frame_and_own_inputs() {
    let mut s = phase2(20.0);
    s.detection.local = vec![ts(serde_json::Value::Null)];
    let errors = validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let report = engine.run(&mut NullRecorder::new()).expect("runs");
    let p = &report.phase2;
    let honest = p.ts_messages.get("honest").copied().unwrap_or_default();
    assert!(honest.evaluated > 0, "no honest message was judged: {:?}", p.ts_messages);
    let evaluated = |check: &str| -> u64 {
        p.ts_checks
            .get(check)
            .map_or(0, |by| by.values().map(|c| c.evaluated).sum())
    };
    assert!(
        evaluated("boxIntersection") > 0,
        "the box check never ran: no VehicleSize reached it"
    );
    assert!(evaluated("kalmanConsistencyTs") > 0, "the Kalman check never ran");
    // Every message is judged: the class tallies sum to the host's count.
    let classes: u64 = p.ts_messages.values().map(|c| c.evaluated).sum();
    assert_eq!(classes, p.messages_checked);
}
