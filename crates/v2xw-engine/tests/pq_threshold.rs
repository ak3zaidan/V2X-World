//! Post-quantum schemes and threshold committees, end to end through the engine: a scheme
//! added through the template is selectable and used everywhere, pure post-quantum and
//! ML-KEM change the credential system, committees run and report, and the loader refuses
//! what they would refuse.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use v2xw_engine::scenario::ModelChoice;
use v2xw_engine::scenario::schema::{BackendFailureSpec, ThresholdSpec};
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// The Phase 1 grid with the SCMS, a compressed calendar so top-ups happen, a cellular
/// modem on every car, and an OBU that prices post-quantum operations.
fn grid(signature: &str) -> Scenario {
    let mut s =
        Scenario::load(scenarios().join("phase1-grid.yaml")).expect("the shipped grid loads");
    s.time.duration_s = 30.0;
    s.actors.vehicles.demand.rate_veh_per_h = Some(400.0);
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = json!(5);
        params["rows"] = json!(8);
    }
    s.metrics = vec!["all".to_string()];
    let protocol = v2xw_engine::phase2::CAMP_SCMS.to_string();
    s.actors.backend.protocol = Some(protocol.clone());
    s.security.protocol = Some(ModelChoice {
        id: protocol,
        params: json!({
            "i_period_s": 20,
            "cert_lifetime_s": 21,
            "certs_per_period": 3,
            "pool_periods": 2,
            "topup_below_periods": 1,
            "cert_shuffle_window_s": 1,
            "first_batch_delay_s": 1,
            "download_poll_interval_s": 1
        }),
    });
    s.net.uu = Some(ModelChoice {
        id: "cellular/uu/fixed-latency".to_string(),
        params: json!({"preset": "4g-east-coast"}),
    });
    s.nodes.default_obu = "obu/generic-automotive-soc-no-hsm".to_string();
    s.net.fragmenter = Some(ModelChoice::new("fragmenter/generic-sdu"));
    s.security.signature = signature.to_string();
    s
}

fn run(s: Scenario) -> (RunReport, MemoryRecorder) {
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "the test scenario must load: {errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    let report = engine.run(&mut recorder).expect("runs");
    (report, recorder)
}

fn last_view(r: &MemoryRecorder) -> Value {
    r.records()
        .iter()
        .filter(|(_, rec)| rec.channel == "backend.state")
        .map(|(_, rec)| serde_json::from_slice::<Value>(&rec.json).expect("json"))
        .next_back()
        .expect("a backend view")
}

fn entity<'a>(view: &'a Value, id: &str) -> &'a Value {
    view["entities"]
        .as_array()
        .and_then(|a| a.iter().find(|e| e["id"] == id))
        .unwrap_or_else(|| panic!("no {id} in the view"))
}

fn op(view: &Value, id: &str, key: &str) -> u64 {
    entity(view, id)["ops"][key].as_u64().unwrap_or(0)
}

fn edge_bytes(v: &Value, from: &str, to: &str) -> u64 {
    v["edges"]
        .as_array()
        .and_then(|a| a.iter().find(|e| e["from"] == from && e["to"] == to))
        .and_then(|e| e["bytes"].as_u64())
        .unwrap_or(0)
}

/// `hybrid-mldsa65-ecdsa-p256` was added by copying `v2xw_sec::scheme::template` and adding
/// one registry line. Nothing else was edited for it — and it is offered by the published
/// schema, accepted by the loader, charged on the node and in the backend, and carried on
/// the air and in the credential system.
#[test]
fn a_scheme_added_through_the_template_is_selectable_end_to_end() {
    const ID: &str = "hybrid-mldsa65-ecdsa-p256";
    // Offered: the settings window's choice set for `security.signature` is the registry's.
    let offered = v2xw_engine::scenario::validate::CHOICES
        .iter()
        .find(|c| c.path == "security.signature")
        .expect("published");
    assert!(offered.values.contains(&ID), "{:?}", offered.values);

    let (classic, crec) = run(grid("ecdsa-p256"));
    let (pq, prec) = run(grid(ID));
    // Recorded as what it is.
    assert_eq!(pq.phase2.signature_scheme, ID);
    assert_eq!(pq.phase2.signature_mode, "hybrid");
    let (cv, pv) = (last_view(&crec), last_view(&prec));
    assert_eq!(pv["signature"], ID);
    // The credential system signs with both halves and the device verifies both.
    assert!(op(&pv, "pca", "ml-dsa-65 sign") > 0, "{}", entity(&pv, "pca")["ops"]);
    assert_eq!(op(&cv, "pca", "ml-dsa-65 sign"), 0);
    // Its certificates are larger: the same top-ups move more bytes to the devices.
    println!(
        "top-ups classic {} pq {}; LOP->ee bytes classic {} pq {}",
        classic.phase2.topups_completed,
        pq.phase2.topups_completed,
        edge_bytes(&cv, "lop", "ee"),
        edge_bytes(&pv, "lop", "ee")
    );
    assert!(pq.phase2.topups_completed > 0);
    assert!(edge_bytes(&pv, "lop", "ee") > edge_bytes(&cv, "lop", "ee"));
    // And on the air every message is several fragments.
    let air = |r: &RunReport| serde_json::to_value(r).expect("report")["frames_tx"].as_u64();
    if let (Some(a), Some(b)) = (air(&classic), air(&pq)) {
        assert!(b > a, "fragments: {b} vs {a}");
    }
}

/// A pure post-quantum signature carries no ECDSA half anywhere in the credential system,
/// and an ML-KEM `security.kem` puts encapsulations where ECIES was.
#[test]
fn pure_post_quantum_and_ml_kem_change_the_credential_system() {
    let mut s = grid("ml-dsa-44");
    s.security.kem = "ml-kem-768".to_string();
    let (report, rec) = run(s);
    assert_eq!(report.phase2.signature_mode, "post-quantum");
    assert_eq!(report.phase2.kem_scheme, "ml-kem-768");
    let v = last_view(&rec);
    assert!(op(&v, "pca", "ml-dsa-44 sign") > 0);
    assert_eq!(op(&v, "pca", "ecdsa-p256-sha256 sign"), 0, "no ECDSA half");
    assert!(op(&v, "pca", "ml-kem-768 verify") > 0, "{}", entity(&v, "pca")["ops"]);
    assert!(report.phase2.topups_completed > 0);
}

fn threshold(roles: &[&str]) -> ThresholdSpec {
    serde_json::from_value(json!({ "roles": roles })).expect("defaults")
}

/// Committees in place of the PCA, the CRL Generator and the linkage authorities run in
/// the engine, survive an offline member, and appear in the report and the backend view.
#[test]
fn committees_run_in_the_engine_and_report() {
    let mut s = grid("ecdsa-p256");
    s.security.threshold = Some(threshold(&["pca", "crl", "linkage"]));
    s.actors.backend.failures = vec![BackendFailureSpec {
        node: "pca-member-0".to_string(),
        mode: "offline".to_string(),
        from_s: 0.0,
        to_s: None,
        extra_ms: None,
    }];
    let (report, rec) = run(s);
    let pca = report.phase2.committees.get("pca").copied().unwrap_or_default();
    println!("pca committee: {pca:?}");
    assert!(pca.completed > 0, "{pca:?}");
    assert_eq!(pca.failed, 0);
    assert!(report.phase2.topups_completed > 0);
    let v = last_view(&rec);
    let e = entity(&v, "pca-committee");
    assert_eq!(e["state"]["quorum"], 3);
    assert_eq!(e["state"]["members_live"], 4);
}

#[test]
fn the_loader_refuses_what_the_committees_and_schemes_would() {
    let errs = |s: &Scenario| {
        v2xw_engine::scenario::validate::validate(s)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    };
    let mut s = grid("ecdsa-p256");
    s.security.kem = "ml-kem-9000".to_string();
    assert!(errs(&s).contains("security.kem"), "{}", errs(&s));

    let mut s = grid("ecdsa-p256");
    let mut t = threshold(&["pca"]);
    t.quorum = 6;
    s.security.threshold = Some(t);
    assert!(errs(&s).contains("cannot be met"), "{}", errs(&s));

    let mut s = grid("ecdsa-p256");
    let mut t = threshold(&["pca"]);
    t.protocol = "frost-on-a-napkin".to_string();
    s.security.threshold = Some(t);
    assert!(errs(&s).contains("security.threshold.protocol"), "{}", errs(&s));

    let mut s = grid("ecdsa-p256");
    s.actors.backend.failures = vec![BackendFailureSpec {
        node: "pca-member-0".to_string(),
        mode: "offline".to_string(),
        from_s: 0.0,
        to_s: None,
        extra_ms: None,
    }];
    // No committee configured: there is no pca-member-0 to take down.
    assert!(errs(&s).contains("failures[0].node"), "{}", errs(&s));
}

/// The shipped crypto study scenarios load.
#[test]
fn the_crypto_study_scenarios_validate() {
    for f in [
        "pq-signatures.yaml",
        "pq-full-stack.yaml",
        "pq-certificate-cycle.yaml",
        "threshold-revocation-2of2.yaml",
        "threshold-revocation-3of5.yaml",
        "threshold-pca.yaml",
    ] {
        let s = Scenario::load(scenarios().join(f)).unwrap_or_else(|e| panic!("{f}: {e}"));
        let errors = v2xw_engine::scenario::validate::validate(&s);
        assert!(errors.is_empty(), "{f}: {errors:?}");
    }
}
