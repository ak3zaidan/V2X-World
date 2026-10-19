//! The SCMS settings audit (docs/guides/config-inventory-scms.md): every knob of
//! `security.scms.lifecycle` reaches the model, is refused out of range, and is published
//! with a label, an explanation, a cited default and a level.
//!
//! The same-seed runs that show each knob moving the metrics and the PKI records are
//! `tests/expectations/scms-knobs.yaml` (`v2xw sensitivity --expect`) and
//! docs/design/scms-real/testing/scms-knobs.md; these tests are the fast half.

use std::path::Path;

use serde_json::{Value, json};
use v2xw_engine::Scenario;
use v2xw_engine::phase2::LifecycleParams;

/// The credential-lifecycle scenario on the real calendar with no lifecycle keys: every
/// number is the model's default.
fn base_doc() -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let s = Scenario::load(root.join("scenarios").join("credential-lifecycle.yaml"))
        .expect("the shipped scenario loads");
    let mut doc = serde_json::to_value(&s).expect("serialises");
    doc["time"]["calendar"] = json!("real");
    doc["security"]["protocol"]["params"] = json!({});
    doc
}

fn params(doc: &Value) -> String {
    let s = Scenario::from_document(doc.clone()).expect("deserialises");
    format!("{:?}", LifecycleParams::from_scenario(&s).expect("lifecycle params"))
}

/// One legal value per field, different from the model's default on the real calendar.
const MOVES: &[(&str, f64)] = &[
    ("i_period_s", 300.0),
    ("cert_lifetime_s", 700_000.0),
    ("certs_per_period", 7.0),
    ("pool_periods", 3.0),
    ("topup_below_periods", 2.0),
    ("max_periods_ahead", 10.0),
    ("cert_shuffle_threshold", 50.0),
    ("cert_shuffle_window_s", 600.0),
    ("report_shuffle_threshold", 40.0),
    ("report_shuffle_window_s", 60.0),
    ("first_batch_delay_s", 9.0),
    ("download_poll_interval_s", 11.0),
    ("max_download_polls", 3.0),
    ("download_request_tolerance_s", 30.0),
    ("enrolment_lifetime_s", 1_000_000.0),
    ("reenrol_lead_s", 5_000.0),
    ("reenrol_download_delay_s", 13.0),
    ("crl_cadence_s", 600.0),
    ("crl_fetch_interval_s", 120.0),
    ("crl_broadcast_interval_s", 2.0),
    ("ma_review_s", 100.0),
    ("ma_max_open_cases", 4.0),
];

#[test]
fn every_lifecycle_field_moves_the_model_exactly_as_its_protocol_param_does() {
    let base = base_doc();
    let default = params(&base);
    let schema_fields = v2xw_engine::scenario::publish::fields();
    let published: Vec<&str> = schema_fields
        .iter()
        .filter_map(|f| f["x-path"].as_str())
        .filter_map(|p| p.strip_prefix("security.scms.lifecycle."))
        .collect();
    assert_eq!(
        published.len(),
        MOVES.len(),
        "a lifecycle field has no move here, or a move names no field: {published:?}"
    );
    for (key, v) in MOVES {
        assert!(published.contains(key), "{key} is not published");
        // A count is written as a whole number, as a person (and the form) writes it.
        let v = if v.fract() == 0.0 { json!(*v as u64) } else { json!(v) };
        let mut typed = base.clone();
        typed["security"]["scms"] = json!({"lifecycle": {*key: v}});
        let mut param = base.clone();
        param["security"]["protocol"]["params"] = json!({*key: v});
        let (t, p) = (params(&typed), params(&param));
        assert_ne!(t, default, "security.scms.lifecycle.{key} = {v} changed nothing");
        assert_eq!(
            t, p,
            "security.scms.lifecycle.{key} and security.protocol.params.{key} disagree"
        );
    }
}

#[test]
fn the_typed_field_wins_over_the_protocol_param() {
    let mut doc = base_doc();
    doc["security"]["protocol"]["params"] = json!({"certs_per_period": 5});
    doc["security"]["scms"] = json!({"lifecycle": {"certs_per_period": 9}});
    let s = Scenario::from_document(doc).expect("deserialises");
    let p = LifecycleParams::from_scenario(&s).expect("params");
    assert_eq!((p.jmax, p.scms.certs_per_period), (9, 9));
}

fn errors(doc: Value) -> Vec<String> {
    let s = Scenario::from_document(doc).expect("deserialises");
    v2xw_engine::scenario::validate(&s)
        .into_iter()
        .map(|e| e.to_string())
        .collect()
}

#[test]
fn the_loader_refuses_what_the_model_cannot_run() {
    // Inside every range, and consistent: loads.
    let mut ok = base_doc();
    ok["security"]["scms"] =
        json!({"lifecycle": {"certs_per_period": 60, "ma_review_s": 0}, "tls": "1.2"});
    assert_eq!(errors(ok), Vec::<String>::new());
    let mut tls = base_doc();
    tls["security"]["scms"] = json!({"tls": "1.1"});
    assert!(
        errors(tls).iter().any(|m| m.contains("security.scms.tls")),
        "TLS 1.1 was not refused"
    );
    for (lifecycle, field) in [
        (json!({"certs_per_period": 61}), "security.scms.lifecycle.certs_per_period"),
        (json!({"certs_per_period": 0}), "security.scms.lifecycle.certs_per_period"),
        (json!({"max_download_polls": 0}), "security.scms.lifecycle.max_download_polls"),
        (json!({"ma_max_open_cases": 0}), "security.scms.lifecycle.ma_max_open_cases"),
        (json!({"crl_fetch_interval_s": 0.0}), "security.scms.lifecycle.crl_fetch_interval_s"),
        (json!({"cert_lifetime_s": 600_000.0}), "security.scms.lifecycle.cert_lifetime_s"),
        (
            json!({"enrolment_lifetime_s": 1000.0, "reenrol_lead_s": 1000.0}),
            "security.scms.lifecycle.reenrol_lead_s",
        ),
        // A compressed i-period on the real calendar.
        (json!({"i_period_s": 60.0}), "security.scms.lifecycle.i_period_s"),
    ] {
        let mut doc = base_doc();
        doc["security"]["scms"] = json!({"lifecycle": lifecycle.clone()});
        let e = errors(doc);
        assert!(
            e.iter().any(|m| m.contains(field)),
            "{lifecycle} was not refused at {field}: {e:?}"
        );
    }
}

#[test]
fn every_lifecycle_field_is_published_with_words_a_default_and_a_level() {
    let fields = v2xw_engine::scenario::publish::fields();
    let mut basic = 0;
    for f in &fields {
        let Some(path) = f["x-path"].as_str() else {
            continue;
        };
        let Some(key) = path.strip_prefix("security.scms.lifecycle.") else {
            continue;
        };
        for k in ["x-explain", "x-recommended", "x-level", "title", "x-range"] {
            assert!(f[k].as_str().is_some_and(|s| !s.is_empty()), "{path}: no {k}");
        }
        // The request shuffle acts only on a first request, which a run makes before the
        // vehicle joins (tests/scms_knobs_runs.rs): published as partial, with the reason.
        let expected = if key.starts_with("cert_shuffle_") { "partial" } else { "wired" };
        assert_eq!(f["x-status"], expected, "{path}");
        assert!(f["unit"].as_str().is_some_and(|u| !u.is_empty()), "{path}: no unit");
        // Absent, the CRL follows each provider's rule, which is not a number.
        if key == "crl_cadence_s" {
            assert!(f.get("default").is_none_or(Value::is_null), "{path}");
        } else {
            assert!(f["default"].is_number(), "{path}: no default");
        }
        basic += usize::from(f["x-level"] == "basic");
    }
    assert!(basic >= 5, "only {basic} SCMS knobs are basic");
}

#[test]
fn the_published_defaults_are_the_models() {
    // Every published default, written back, leaves the model where it was.
    let base = base_doc();
    let default = params(&base);
    let mut lifecycle = serde_json::Map::new();
    for f in v2xw_engine::scenario::publish::fields() {
        let Some(key) = f["x-path"]
            .as_str()
            .and_then(|p| p.strip_prefix("security.scms.lifecycle."))
        else {
            continue;
        };
        // ma_review_s is replayed too: on the real calendar its default is the manual
        // review the model runs when it is absent.
        if f["default"].is_number() {
            lifecycle.insert(key.to_string(), f["default"].clone());
        }
    }
    let mut doc = base.clone();
    doc["security"]["scms"] = json!({"lifecycle": lifecycle});
    assert_eq!(params(&doc), default, "a published default is not the model's");
}
