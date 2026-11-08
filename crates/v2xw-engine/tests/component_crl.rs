//! On a built engine: a provider revokes one of its SCMS components
//! (`security.scms.lifecycle.component_revoked*`), the certificate goes on the hash CRL of
//! the `(cracaId, crlSeries)` it names ([SMP] Table 1: series 2 for an ICA-issued
//! component, 256 for a root-issued one), the list travels in the composite CRL beside
//! the linkage CRL, and every receiver that installs it refuses what the component
//! certified (`RxReject::IssuerRevoked`, IEEE 1609.2 §5.1.3).

use std::path::{Path, PathBuf};

use serde_json::json;
use v2xw_engine::scenario::ModelChoice;
use v2xw_engine::{Engine, NullRecorder, Scenario};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// A 10-vehicle grid on the US deployment (two providers on one CTL) on the compressed
/// calendar: 20 s i-periods, a CRL publication every 10 s, a vehicle CRL poll every 10 s.
fn scenario(lifecycle: Option<serde_json::Value>) -> Scenario {
    let mut s = Scenario::load(root().join("scenarios/phase1-grid.yaml")).expect("loads");
    s.time.duration_s = 60.0;
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = json!(5);
        params["rows"] = json!(8);
    }
    s.world.buildings.enabled = false;
    s.actors.vehicles.demand.rate_veh_per_h = Some(3_600_000.0);
    s.actors.vehicles.demand.params = json!({ "max_total_vehicles": 10 });
    s.security.verification_policy = "verify-all".to_string();
    let id = v2xw_engine::phase2::CAMP_SCMS.to_string();
    s.actors.backend.protocol = Some(id.clone());
    s.security.protocol = Some(ModelChoice {
        id,
        params: json!({
            "i_period_s": 20,
            "cert_lifetime_s": 21,
            "certs_per_period": 3,
            "pool_periods": 2,
            "topup_below_periods": 1,
            "cert_shuffle_window_s": 1,
            "first_batch_delay_s": 1,
            "download_poll_interval_s": 1,
            "report_shuffle_window_s": 5,
            "crl_cadence_s": 10,
            "crl_fetch_interval_s": 10,
            "ma_review_s": 30
        }),
    });
    s.time.calendar = v2xw_engine::scenario::Calendar::Compressed;
    s.net.uu = Some(ModelChoice {
        id: "cellular/uu/fixed-latency".to_string(),
        params: json!({"preset": "4g-east-coast"}),
    });
    s.threats.attackers.clear();
    if let Some(l) = lifecycle {
        let mut doc = serde_json::to_value(&s).expect("serialises");
        doc["security"]["scms"] = json!({ "lifecycle": l });
        s = Scenario::from_document(doc).expect("deserialises");
    }
    s
}

fn errors(s: &Scenario) -> Vec<String> {
    v2xw_engine::scenario::validate(s)
        .into_iter()
        .map(|e| e.to_string())
        .collect()
}

/// Acceptance test 3: at `component_revoked_at_s` the home provider revokes its
/// pseudonym CA. Before the decision no receiver refuses anything as issuer-revoked;
/// once the series-2 list (S2X: CRACA ICA; ISS: CRACA root) is published and the devices
/// install it with their composite CRL, the frames their pseudonyms sign are refused.
///
/// Fault: the composite carries no hash CRL (`phase2.rs` crl feed without
/// `crl_store.hash_crls`, the roadside path without `crl_broadcast.hash_crls`). No
/// receiver installs the list, the `issuer-revoked` count stays 0, and the test goes red.
#[test]
fn a_revoked_pca_makes_receivers_refuse_its_pseudonyms() {
    let at_s = 15.0;
    let s = scenario(Some(
        json!({"component_revoked": "pca", "component_revoked_at_s": at_s}),
    ));
    assert_eq!(errors(&s), Vec::<String>::new(), "the scenario loads");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = NullRecorder::new();
    let at = (at_s * 1e9) as u64;
    engine
        .advance(at - 1, &mut rec)
        .expect("runs to the decision");
    let before = engine.receiver_validation_now();
    let issuer = |rx: &v2xw_engine::run::ReceiverValidationReport| {
        rx.refused.get("issuer-revoked").copied().unwrap_or(0)
    };
    assert!(
        before.checked > 0,
        "receivers verified frames before the decision"
    );
    assert_eq!(issuer(&before), 0, "nothing is revoked before the decision");
    let r = engine.run(&mut rec).expect("runs");
    let p = &r.phase2;
    println!(
        "revocations {} refused {} component entries {} crl versions {} rx before {:?} after {:?} errors {} {}",
        p.component_revocations,
        p.component_revocations_refused,
        p.crl_component_entries,
        p.crl_versions_published,
        before.refused,
        r.receiver_validation.refused,
        p.backend_errors,
        p.first_backend_error
    );
    assert_eq!(p.component_revocations, 1, "the provider decided once");
    assert_eq!(p.component_revocations_refused, 0);
    assert!(
        p.crl_component_entries >= 1,
        "the PCA certificate is on a signed component CRL"
    );
    assert!(
        issuer(&r.receiver_validation) > 0,
        "receivers that installed the component list refuse the revoked PCA's pseudonyms: {:?}",
        r.receiver_validation.refused
    );
}

/// The component-CRL keys' loader rules: a role needs its time, the time must be
/// inside the run, the provider must be one the deployment runs, an unknown role is
/// refused, and the S2X ICA (no certificate signs series 2 under the root) is refused.
/// Off by default: a scenario without the keys schedules nothing.
#[test]
fn the_component_revocation_keys_are_checked_by_the_loader() {
    use v2xw_proto::scms::deployment::{Deployment, Preset};
    use v2xw_proto::scms::governance::CertProfile;
    let has = |s: &Scenario, path: &str| errors(s).iter().any(|m| m.contains(path));
    let at = "security.scms.lifecycle.component_revoked_at_s";
    let role = "security.scms.lifecycle.component_revoked";
    let provider = "security.scms.lifecycle.component_revoked_provider";
    assert!(has(
        &scenario(Some(json!({"component_revoked": "pca"}))),
        at
    ));
    assert!(has(
        &scenario(Some(
            json!({"component_revoked": "pca", "component_revoked_at_s": 61})
        )),
        at
    ));
    assert!(has(
        &scenario(Some(json!({"component_revoked_at_s": 10}))),
        at
    ));
    assert!(has(
        &scenario(Some(
            json!({"component_revoked": "root", "component_revoked_at_s": 10})
        )),
        role
    ));
    assert!(has(
        &scenario(Some(json!({"component_revoked": "pca",
            "component_revoked_at_s": 10, "component_revoked_provider": "nobody"}))),
        provider
    ));
    let d = Deployment::preset(Preset::UsProduction2025);
    for p in &d.providers {
        let s = scenario(Some(json!({"component_revoked": "ica",
            "component_revoked_at_s": 10, "component_revoked_provider": p.name})));
        assert_eq!(
            has(&s, role),
            p.cert_profile != CertProfile::Iss,
            "ICA of {} ({:?})",
            p.name,
            p.cert_profile
        );
    }
    let none = scenario(Some(json!({"component_revoked": "none"})));
    assert_eq!(errors(&none), Vec::<String>::new());
    // Published with a menu, a unit and a step.
    let fields = v2xw_engine::scenario::publish::fields();
    let field = |p: &str| {
        fields
            .iter()
            .find(|f| f["x-path"] == p)
            .unwrap_or_else(|| panic!("{p} is published"))
            .clone()
    };
    let menu = field(role);
    let labels: Vec<&str> = menu["x-options"]
        .as_array()
        .expect("a menu")
        .iter()
        .filter_map(|o| o["label"].as_str())
        .collect();
    assert!(labels.contains(&"Pseudonym CA (ACA)"), "{labels:?}");
    assert_eq!(menu["default"], json!("none"));
    let t = field(at);
    assert_eq!(
        (t["unit"].as_str(), t["x-step"].as_f64()),
        (Some("s"), Some(1.0))
    );
    assert!(field(provider)["x-options-by-preset"].is_object());
}
