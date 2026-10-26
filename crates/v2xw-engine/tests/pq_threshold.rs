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
    s.time.calendar = v2xw_engine::scenario::Calendar::Compressed;
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
    assert!(
        op(&pv, "pca", "ml-dsa-65 sign") > 0,
        "{}",
        entity(&pv, "pca")["ops"]
    );
    assert_eq!(op(&cv, "pca", "ml-dsa-65 sign"), 0);
    // Its certificates are larger: the same top-ups move more bytes to the devices.
    println!(
        "top-ups classic {} pq {}; RA->ee bytes classic {} pq {}",
        classic.phase2.topups_completed,
        pq.phase2.topups_completed,
        edge_bytes(&cv, "ra", "ee"),
        edge_bytes(&pv, "ra", "ee")
    );
    assert!(pq.phase2.topups_completed > 0);
    assert!(edge_bytes(&pv, "ra", "ee") > edge_bytes(&cv, "ra", "ee"));
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
    assert!(
        op(&v, "pca", "ml-kem-768 verify") > 0,
        "{}",
        entity(&v, "pca")["ops"]
    );
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
    let pca = report
        .phase2
        .committees
        .get("pca")
        .copied()
        .unwrap_or_default();
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
    assert!(
        errs(&s).contains("security.threshold.protocol"),
        "{}",
        errs(&s)
    );

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

/// What each signature scheme costs the same fleet on the air and in the backend. Not a
/// check — a measurement, run on demand:
///
/// ```sh
/// cargo test -p v2xw-engine --test pq_threshold fleet_costs -- --ignored --nocapture
/// ```
///
/// Prints one JSON line per scheme: frames and fragments on the air, delivery, reassembly
/// loss, MTU refusals, MAC access delay, the last sample of every channel-load, delivery
/// and latency metric, and the credential system's provisioning bytes and PCA time.
#[test]
#[ignore = "a measurement, not a check: run with --ignored --nocapture"]
fn fleet_costs_by_scheme() {
    let interesting = |m: &str| {
        [
            "pdr",
            "cbr",
            "latency",
            "verify",
            "sign",
            "airtime",
            "busy",
            "load",
            "frag",
            "reassembl",
        ]
        .iter()
        .any(|k| m.contains(k))
    };
    for (sig, kem, fragmenter) in [
        ("ecdsa-p256", "ecies-p256", "fragmenter/generic-sdu"),
        (
            "hybrid-falcon512-ecdsa-p256",
            "ecies-p256",
            "fragmenter/generic-sdu",
        ),
        (
            "hybrid-falcon512-ecdsa-p256",
            "ecies-p256",
            "fragmenter/cert-cycle-partial-hybrid",
        ),
        ("falcon-512", "ecies-p256", "fragmenter/generic-sdu"),
        (
            "hybrid-mldsa44-ecdsa-p256",
            "ecies-p256",
            "fragmenter/generic-sdu",
        ),
        ("ml-dsa-44", "ml-kem-768", "fragmenter/generic-sdu"),
        ("ml-dsa-87", "ml-kem-1024", "fragmenter/generic-sdu"),
        ("slh-dsa-sha2-128f", "ml-kem-768", "fragmenter/generic-sdu"),
    ] {
        // `PQ_FLEET_ONLY=<substring>` runs only the cells whose scheme or fragmenter
        // contains it.
        if let Ok(only) = std::env::var("PQ_FLEET_ONLY")
            && !sig.contains(&only)
            && !fragmenter.contains(&only)
        {
            continue;
        }
        let mut s = grid(sig);
        s.time.duration_s = 60.0;
        s.actors.vehicles.demand.rate_veh_per_h = Some(1500.0);
        s.security.kem = kem.to_string();
        s.net.fragmenter = Some(ModelChoice::new(fragmenter));
        let (r, rec) = run(s);
        let mut last: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new();
        for (_, x) in rec.records() {
            if x.channel != "metric.sample" {
                continue;
            }
            let v: Value = serde_json::from_slice(&x.json).expect("json");
            let name = v["metric"].as_str().unwrap_or("").to_string();
            if interesting(&name) {
                last.insert(format!("{name} {}", v["dims"]), v["value"].clone());
            }
        }
        let view = last_view(&rec);
        let pca_busy = entity(&view, "pca")["queue"]["busy_ns"]
            .as_u64()
            .unwrap_or(0);
        println!(
            "FLEET {}",
            json!({
                "signature": sig, "kem": kem, "fragmenter": fragmenter,
                "frames_transmitted": r.frames_transmitted,
                "fragments_sent": r.fragments_sent,
                "sdus_fragmented": r.sdus_fragmented,
                "reassembly_complete": r.reassembly_complete,
                "reassembly_lost": r.reassembly_lost,
                "reception_attempts": r.reception_attempts,
                "receptions_ok": r.receptions_ok,
                "net_mtu_refusals": r.net_mtu_refusals,
                "mac_grants": r.mac_grants,
                "mac_access_delay_ns": r.mac_access_delay_ns,
                "mac_drops": r.mac_drops,
                "rx_losses": r.rx_losses,
                "topups_completed": r.phase2.topups_completed,
                "certs_topped_up": r.phase2.certs_topped_up,
                "ra_to_ee_bytes": edge_bytes(&view, "ra", "ee"),
                "ee_to_ra_bytes": edge_bytes(&view, "ee", "ra"),
                "pca_busy_ns": pca_busy,
                "metrics": last,
            })
        );
    }
}

/// The threshold study scenarios, run whole: two-of-two against 3-of-5 revocation with
/// servers down, and the 3-of-5 PCA with failing members under each protocol. A
/// measurement, run on demand like [`fleet_costs_by_scheme`].
#[test]
#[ignore = "a measurement, not a check: run with --ignored --nocapture"]
fn threshold_study_scenarios() {
    let mut cells: Vec<(String, Scenario)> = Vec::new();
    for f in [
        "threshold-revocation-2of2.yaml",
        "threshold-revocation-3of5.yaml",
    ] {
        let mut s = Scenario::load(scenarios().join(f)).expect("loads");
        s.experiment = None;
        cells.push((f.to_string(), s));
    }
    for protocol in ["frost-p256", "cggmp21-ecdsa", "umbrella"] {
        let mut s = Scenario::load(scenarios().join("threshold-pca.yaml")).expect("loads");
        s.experiment = None;
        if let Some(t) = s.security.threshold.as_mut() {
            t.protocol = protocol.to_string();
        }
        cells.push((format!("threshold-pca {protocol}"), s));
    }
    for (name, s) in cells {
        let (r, _) = run(s);
        let p = &r.phase2;
        println!(
            "THRESHOLD {}",
            json!({
                "cell": name,
                "reports_at_ma": p.reports_at_ma,
                "ma_revoke_decisions": p.ma_revoke_decisions,
                "cases_opened": p.cases_opened,
                "cases_unresolved": p.cases_unresolved,
                "crls_issued": p.crls_issued,
                "crl_versions_published": p.crl_versions_published,
                "revocation_latency_ns": p.revocation_latency_ns,
                "revocation_stages": p.revocation_stages,
                "topups_started": p.topups_started,
                "topups_completed": p.topups_completed,
                "vehicles_starved": p.vehicles_starved,
                "committees": p.committees,
                "committee_failures": p.committee_failures,
                "backend_deliveries_dropped": p.backend_deliveries_dropped,
            })
        );
    }
}
