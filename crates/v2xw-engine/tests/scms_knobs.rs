//! The SCMS settings audit (docs/reference/config-inventory-scms.md): every knob of
//! `security.scms.lifecycle` reaches the model, is refused out of range, and is published
//! with a label, an explanation, a cited default and a level.
//!
//! The same-seed runs that show each knob moving the metrics and the PKI records are
//! `tests/expectations/scms-knobs.yaml` (`v2xw sensitivity --expect`); these tests are the fast
//! half.

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
    format!(
        "{:?}",
        LifecycleParams::from_scenario(&s).expect("lifecycle params")
    )
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
    ("report_store_quota_bytes", 1_000.0),
    ("report_daily_multiplier", 0.5),
    ("report_decay_hour_et", 6.0),
    ("report_weight_random_position", 2.0),
    ("report_weight_constant_position", 3.0),
    ("report_weight_max_speed", 4.0),
    ("report_weight_max_acceleration", 5.0),
    ("report_weight_brake_acceleration", 6.0),
    // The keys that were once reachable only through `security.protocol.params`.
    ("ee_retry_timeout_s", 20.0),
    ("ee_retry_max_per_hour", 3.0),
    ("crl_background_entries", 1_234.0),
    ("clock_skew_s", 2.5),
    ("clock_skew_share", 0.5),
    ("root_rollover_at_s", 100.0),
    ("root_removal_at_s", 200.0),
    ("peer_root_removal_at_s", 300.0),
    ("modelled_reports", 1.0),
    // The time is a lifecycle number; the component and its provider are words.
    ("component_revoked_at_s", 50.0),
    // Root termination notice and test ballots.
    ("root_termination_notice_at_s", 40.0),
    // The scenario keys of the elector replacement and the root addition.
    ("elector_replacement_at_s", 40.0),
    ("root_addition_at_s", 40.0),
    ("test_ballot_interval_days", 365.0),
];

/// The lifecycle fields that are words, not numbers, and so have no
/// `security.protocol.params` spelling: read from the typed field only
/// (`phase2::component_revocation_of`; tests/component_crl.rs moves them).
const TYPED_ONLY: &[&str] = &[
    "component_revoked",
    "component_revoked_provider",
    // Gov C-10 and G-2 (D1 R5): read from the typed field by `LifecycleParams::from_scenario`.
    "ctl_genesis_sequence",
    "eac_review",
    // The post-quantum elector replacement.
    "elector_replacement_pq",
];

/// `crl_background_entries` is read by `Phase2::new`, not held in `LifecycleParams`: its
/// model value is the key the loader resolves (`lifecycle_param`, typed field winning).
fn background(doc: &Value) -> Option<f64> {
    let s = Scenario::from_document(doc.clone()).expect("deserialises");
    v2xw_engine::phase2::lifecycle_param(&s, "crl_background_entries")
}

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
        MOVES.len() + TYPED_ONLY.len(),
        "a lifecycle field has no move here, or a move names no field: {published:?}"
    );
    for key in TYPED_ONLY {
        assert!(published.contains(key), "{key} is not published");
    }
    for (key, v) in MOVES {
        assert!(published.contains(key), "{key} is not published");
        // A count is written as a whole number, as a person (and the form) writes it.
        let v = if v.fract() == 0.0 {
            json!(*v as u64)
        } else {
            json!(v)
        };
        // A switch is a boolean in the typed field and 0/1 in the protocol params.
        let typed_v = if *key == "modelled_reports" {
            json!(true)
        } else {
            v.clone()
        };
        let mut typed = base.clone();
        typed["security"]["scms"] = json!({"lifecycle": {*key: typed_v}});
        let mut param = base.clone();
        param["security"]["protocol"]["params"] = json!({*key: v});
        if *key == "crl_background_entries" {
            assert_eq!(background(&base), None);
            assert_eq!(background(&typed), Some(1_234.0), "{key} changed nothing");
            assert_eq!(background(&typed), background(&param), "{key} disagrees");
            continue;
        }
        let (t, p) = (params(&typed), params(&param));
        assert_ne!(
            t, default,
            "security.scms.lifecycle.{key} = {v} changed nothing"
        );
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
        (
            json!({"certs_per_period": 61}),
            "security.scms.lifecycle.certs_per_period",
        ),
        (
            json!({"certs_per_period": 0}),
            "security.scms.lifecycle.certs_per_period",
        ),
        (
            json!({"max_download_polls": 0}),
            "security.scms.lifecycle.max_download_polls",
        ),
        (
            json!({"ma_max_open_cases": 0}),
            "security.scms.lifecycle.ma_max_open_cases",
        ),
        (
            json!({"report_daily_multiplier": 1.5}),
            "security.scms.lifecycle.report_daily_multiplier",
        ),
        (
            json!({"report_decay_hour_et": 24}),
            "security.scms.lifecycle.report_decay_hour_et",
        ),
        (
            json!({"crl_fetch_interval_s": 0.0}),
            "security.scms.lifecycle.crl_fetch_interval_s",
        ),
        (
            json!({"cert_lifetime_s": 600_000.0}),
            "security.scms.lifecycle.cert_lifetime_s",
        ),
        (
            json!({"enrolment_lifetime_s": 1000.0, "reenrol_lead_s": 1000.0}),
            "security.scms.lifecycle.reenrol_lead_s",
        ),
        // A compressed i-period on the real calendar.
        (
            json!({"i_period_s": 60.0}),
            "security.scms.lifecycle.i_period_s",
        ),
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
        // A word field is a menu: labelled values, no range or unit.
        if TYPED_ONLY.contains(&key) {
            for k in ["x-explain", "x-recommended", "x-level", "title"] {
                assert!(
                    f[k].as_str().is_some_and(|s| !s.is_empty()),
                    "{path}: no {k}"
                );
            }
            assert_eq!(f["x-status"], "wired", "{path}");
            let menu = f["x-options"].as_array().expect("a menu");
            assert!(
                !menu.is_empty()
                    && menu
                        .iter()
                        .all(|o| o["label"].is_string() && o["description"].is_string()),
                "{path}: a menu with a label and a sentence per value"
            );
            if key == "component_revoked" {
                assert_eq!(f["default"], json!("none"), "{path}");
            }
            continue;
        }
        for k in ["x-explain", "x-recommended", "x-level", "title", "x-range"] {
            assert!(
                f[k].as_str().is_some_and(|s| !s.is_empty()),
                "{path}: no {k}"
            );
        }
        // The request shuffle acts only on a first request, which a run makes before the
        // vehicle joins (tests/scms_knobs_runs.rs): published as partial, with the reason.
        let expected = if key.starts_with("cert_shuffle_") {
            "partial"
        } else {
            "wired"
        };
        assert_eq!(f["x-status"], expected, "{path}");
        // A switch (modelled_reports) is on or off: no unit.
        assert!(
            f["kind"] == "boolean" || f["unit"].as_str().is_some_and(|u| !u.is_empty()),
            "{path}: no unit"
        );
        // Absent, the CRL follows each provider's rule, which is not a number, and a root
        // event never happens (`phase2::lifecycle_defaults`): no default.
        if key == "crl_cadence_s"
            || key.ends_with("root_rollover_at_s")
            || key.ends_with("root_removal_at_s")
            || key == "component_revoked_at_s"
            || key == "root_termination_notice_at_s"
            || key == "elector_replacement_at_s"
            || key == "root_addition_at_s"
        {
            assert!(f.get("default").is_none_or(Value::is_null), "{path}");
        } else if key == "modelled_reports" {
            assert_eq!(f["default"], json!(false), "{path}");
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
    assert_eq!(
        params(&doc),
        default,
        "a published default is not the model's"
    );
}

/// The two receiver constants the audit left fixed, now settings: the overdue-CRL
/// tolerance (CAMP 1.2.2 Table 67, two weeks) and the validated-certificate cache (4,096).
/// Each is published with words, a range, a unit and the node's own default, is refused
/// out of range, and reaches every device's trust store through the credential factory
/// (the in-run proof for the cache is `foundations.rs`, a cache of one re-validating).
#[test]
fn the_receiver_limits_are_published_refused_out_of_range_and_reach_the_trust_store() {
    use v2xw_engine::credentials::{CredentialFactory, ReceiverLimits};
    let fields = v2xw_engine::scenario::publish::fields();
    for (path, default) in [
        (
            "security.scms.crl_overdue_tolerance_s",
            f64::from(v2xw_node::rxval::OVERDUE_CRL_TOLERANCE_S),
        ),
        (
            "nodes.validated_cert_cache",
            v2xw_node::rxval::EE_CACHE as f64,
        ),
        // Unset, the window is the profile's; the default (US) scenario's is J2945/1
        // Table 11's.
        (
            "security.scms.rx_beacon_max_age_s",
            v2xw_node::rxval::US_BEACON_MAX_AGE_S,
        ),
        ("security.scms.rx_max_age_s", v2xw_node::rxval::MAX_AGE_S),
        (
            "security.scms.rx_max_future_s",
            v2xw_node::rxval::US_MAX_FUTURE_S,
        ),
    ] {
        let f = fields
            .iter()
            .find(|f| f["x-path"] == path)
            .unwrap_or_else(|| panic!("{path} is not published"));
        for k in [
            "x-explain",
            "x-recommended",
            "x-level",
            "title",
            "x-range",
            "unit",
        ] {
            assert!(
                f[k].as_str().is_some_and(|s| !s.is_empty()),
                "{path}: no {k}"
            );
        }
        assert_eq!(f["x-status"], "wired", "{path}");
        assert_eq!(f["default"].as_f64(), Some(default), "{path}");
    }
    // The overdue scope (F5-SCOPE): a choice, published with its two readings and the
    // node's default.
    let scope = fields
        .iter()
        .find(|f| f["x-path"] == "security.scms.crl_overdue_scope")
        .expect("security.scms.crl_overdue_scope is published");
    for k in ["x-explain", "x-recommended", "x-level", "title"] {
        assert!(
            scope[k].as_str().is_some_and(|s| !s.is_empty()),
            "scope: no {k}"
        );
    }
    assert_eq!(scope["x-status"], "wired");
    assert_eq!(scope["default"], "craca");
    assert_eq!(scope["enum"], json!(["craca", "all"]));
    assert_eq!(v2xw_node::rxval::OVERDUE_CRL_TOLERANCE_S, 14 * 86_400);
    // C2C-CC BSP RS_BSP_168 (via Vanetza's check_generation_time): 2 s, 10 min, 40 ms.
    assert_eq!(v2xw_node::rxval::BEACON_MAX_AGE_S, 2.0);
    assert_eq!(v2xw_node::rxval::MAX_AGE_S, 600.0);
    assert_eq!(v2xw_node::rxval::MAX_FUTURE_S, 0.040);

    let mut doc = base_doc();
    doc["security"]["scms"] = json!({
        "crl_overdue_tolerance_s": 3_600,
        "crl_overdue_scope": "all",
        "rx_max_future_s": 0.5
    });
    doc["nodes"]["validated_cert_cache"] = json!(7);
    assert_eq!(errors(doc.clone()), Vec::<String>::new());
    let s = Scenario::from_document(doc).expect("deserialises");
    let limits = ReceiverLimits::of(&s);
    assert_eq!(
        limits,
        ReceiverLimits {
            validated_cert_cache: Some(7),
            crl_overdue_tolerance_s: Some(3_600),
            crl_overdue_scope: Some(v2xw_node::rxval::CrlOverdueScope::All),
            // The keys left unset take the US profile's values (J2945/1 Table 11).
            generation_window: Some(v2xw_node::rxval::GenerationTimeWindow::from_secs(
                30.0, 600.0, 0.5
            )),
        }
    );
    let mut sec = v2xw_node::NodeSecurity::new(
        v2xw_core::time::WallClock::new(0),
        v2xw_node::CryptoMode::Modeled,
        0x20,
    );
    CredentialFactory::for_run(1, v2xw_core::time::WallClock::new(0), None)
        .with_receiver_limits(limits)
        .install_trust(&mut sec);
    assert_eq!(sec.trust().overdue_crl_tolerance_s(), 3_600);
    assert_eq!(
        sec.trust().crl_overdue_scope(),
        v2xw_node::rxval::CrlOverdueScope::All,
        "the overdue scope reaches the trust store"
    );
    assert_eq!(sec.trust().ee_cache().capacity(), 7);
    assert_eq!(
        sec.trust().generation_time_window().max_future_ns,
        500_000_000,
        "the future tolerance reaches the trust store"
    );
    assert_eq!(
        ReceiverLimits::of(&Scenario::from_document(base_doc()).expect("base")),
        ReceiverLimits {
            generation_window: Some(v2xw_node::rxval::GenerationTimeWindow::j2945_1()),
            ..ReceiverLimits::default()
        },
        "unset, every device keeps the node's defaults, and a US device gets the US window"
    );

    for (set, field) in [
        (
            json!({"security": {"scms": {"crl_overdue_tolerance_s": 40_000_000}}}),
            "security.scms.crl_overdue_tolerance_s",
        ),
        (
            json!({"nodes": {"validated_cert_cache": 0}}),
            "nodes.validated_cert_cache",
        ),
        (
            json!({"security": {"scms": {"rx_beacon_max_age_s": -1.0}}}),
            "security.scms.rx_beacon_max_age_s",
        ),
        (
            json!({"security": {"scms": {"rx_max_age_s": 40_000_000.0}}}),
            "security.scms.rx_max_age_s",
        ),
        (
            json!({"security": {"scms": {"rx_max_future_s": -0.1}}}),
            "security.scms.rx_max_future_s",
        ),
        (
            json!({"security": {"scms": {"crl_overdue_scope": "every"}}}),
            "security.scms.crl_overdue_scope",
        ),
    ] {
        let mut doc = base_doc();
        if let Some(t) = set["security"]["scms"].as_object() {
            doc["security"]["scms"] = Value::Object(t.clone());
        }
        if let Some(n) = set["nodes"]["validated_cert_cache"].as_u64() {
            doc["nodes"]["validated_cert_cache"] = json!(n);
        }
        let e = errors(doc);
        assert!(
            e.iter().any(|m| m.contains(field)),
            "{set} was not refused: {e:?}"
        );
    }
}

/// Verify-on-demand with misbehaviour detection warns, in the loader's warnings (the
/// manifest, `v2xw validate` and the settings window's Check all read this list): both
/// suites judge only verified messages (SM-MBR v1.0 §2.3.1.2.1), so detection goes nearly
/// blind (effects study, `ond-reports`: 897 reports to 79). `verify-all`, or no
/// detection, does not warn.
///
/// Fault injected (and reverted): `detection_policy_warnings` returning nothing fails the
/// first assertion.
#[test]
fn verify_on_demand_with_detection_warns() {
    let warn = |policy: &str, detection: bool| {
        let mut doc = base_doc();
        doc["security"]["verification_policy"] = json!(policy);
        if policy == "on-demand" {
            doc["security"]["on_demand_threshold"] = json!(0.5);
        }
        doc["detection"]["local"] = if detection {
            json!([{"id": "detect/us-sm-mbr"}])
        } else {
            json!([])
        };
        let s = Scenario::from_document(doc).expect("deserialises");
        assert_eq!(v2xw_engine::scenario::validate(&s), Vec::new(), "{policy}");
        v2xw_engine::scenario::detection_policy_warnings(&s)
    };
    let w = warn("on-demand", true);
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(
        w[0].contains("security.verification_policy") && w[0].contains("897"),
        "{w:?}"
    );
    assert_eq!(warn("prioritized", true).len(), 1);
    assert!(warn("verify-all", true).is_empty());
    // An empty list runs `detect/us-sm-mbr` under the US deployment, so it warns
    // too; where an empty list runs no detector (the CAMP reference) it does not.
    assert_eq!(
        warn("on-demand", false).len(),
        1,
        "an empty detection.local under the US system runs SM-MBR"
    );
    let mut camp = base_doc();
    camp["security"]["verification_policy"] = json!("on-demand");
    camp["security"]["on_demand_threshold"] = json!(0.5);
    camp["detection"]["local"] = json!([]);
    camp["security"]["scms"] = json!({"deployment": {"preset": "camp-reference"}});
    let camp = Scenario::from_document(camp).expect("deserialises");
    assert_eq!(v2xw_engine::scenario::validate(&camp), Vec::new());
    assert!(v2xw_engine::scenario::detection_policy_warnings(&camp).is_empty());
}

/// The US SM-MBR suite on raw GNSS fixes warns: on raw fixes it revoked 33 honest devices on
/// phase2-manhattan, whose vehicles had no receiver model. The empty `detection.local` runs
/// this suite under the US system. Raw fixes are asked for with `smoothing_tau_s: 0`; left
/// unset, the US profile runs the deployed receiver
/// (`scms_records::the_us_profile_runs_the_deployed_receiver_unless_told_otherwise`), which
/// does not warn, nor does a smoothed receiver or the research suite. Fault:
/// `us_raw_gnss_warnings` returning nothing -> the first assertion red.
#[test]
fn the_us_suite_on_raw_gnss_fixes_warns() {
    let warn = |local: serde_json::Value, tau: Option<f64>| {
        let mut doc = base_doc();
        doc["detection"]["local"] = local;
        if let Some(tau) = tau {
            doc["actors"]["vehicles"]["gnss"] = json!({"smoothing_tau_s": tau});
        }
        let s = Scenario::from_document(doc).expect("deserialises");
        v2xw_engine::scenario::us_raw_gnss_warnings(&s)
    };
    let w = warn(json!([{"id": "detect/us-sm-mbr"}]), Some(0.0));
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("smoothing_tau_s"), "{w:?}");
    assert_eq!(warn(json!([]), Some(0.0)).len(), 1, "the US default suite");
    assert!(
        warn(json!([{"id": "detect/us-sm-mbr"}]), None).is_empty(),
        "the deployed receiver"
    );
    assert!(warn(json!([{"id": "detect/us-sm-mbr"}]), Some(100.0)).is_empty());
    assert!(warn(json!([{"id": "detect/legacy-12"}]), Some(0.0)).is_empty());
}
