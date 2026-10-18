//! Per-provider SCMS settings (WP-D1 follow-up): each provider's MA decision mode and
//! CRL publication rule reach that provider's own run, the Backend view shows every
//! provider's backend, and the eMBMS bearer carries every provider's signed CRL.
//!
//! Faults that turn these red (each injected and reverted): the per-run settings loop
//! removed (every peer keeps the home provider's rules), the peer views not merged, and
//! the eMBMS list read from the home provider's feed only.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use v2xw_core::time::Duration;
use v2xw_engine::scenario::ModelChoice;
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};
use v2xw_proto::scms::params::MaMode;

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios")
}

fn grid(duration_s: f64, rate: f64) -> Scenario {
    let mut s = Scenario::load(scenarios().join("phase1-grid.yaml")).expect("loads");
    s.time.duration_s = duration_s;
    s.actors.vehicles.demand.rate_veh_per_h = Some(rate);
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = json!(5);
        params["rows"] = json!(8);
    }
    s.actors.backend.protocol = Some(v2xw_engine::phase2::CAMP_SCMS.to_string());
    s.security.protocol = Some(ModelChoice {
        id: v2xw_engine::phase2::CAMP_SCMS.to_string(),
        params: json!({}),
    });
    s.net.uu = Some(ModelChoice {
        id: "cellular/uu/fixed-latency".to_string(),
        params: json!({"preset": "4g-east-coast"}),
    });
    s
}

fn with_providers(s: &mut Scenario, providers: Value) {
    s.security.scms = Some(
        serde_json::from_value(json!({
            "deployment": {"preset": "us-production-2025", "providers": providers}
        }))
        .expect("a security.scms block"),
    );
}

fn rules(e: &Engine) -> Vec<(MaMode, bool, bool)> {
    e.phase2()
        .expect("a backend")
        .scms_runs()
        .iter()
        .map(|r| {
            let p = &r.state.params;
            (p.ma.mode, p.crl.business_day_sla, p.publish_on_cadence)
        })
        .collect()
}

#[test]
fn each_provider_runs_its_own_ma_mode_and_crl_publication_rule() {
    // The US preset: both providers on the manual-review MA (8 h) and the one-business-day
    // SLA (the real calendar).
    let base = Engine::build(grid(5.0, 100.0), "").expect("builds");
    let eight_h = MaMode::ManualReview {
        review: Duration::from_secs(8 * 3_600),
    };
    let r = rules(&base);
    assert_eq!(r.len(), 2);
    assert!(r.iter().all(|x| x.0 == eight_h && x.1), "{r:?}");

    let mut s = grid(5.0, 100.0);
    with_providers(
        &mut s,
        json!([
            {"name": "provider-a", "ma_mode": "automatic", "crl_publication": "on-decision"},
            {"name": "provider-b", "ma_mode": "manual-review", "ma_review_s": 3600,
             "crl_publication": "cadence"}
        ]),
    );
    let e = Engine::build(s, "").expect("builds");
    let r = rules(&e);
    assert_eq!(r[0].0, MaMode::Automatic);
    assert!(!r[0].1 && !r[0].2, "provider-a publishes on decision: {r:?}");
    assert_eq!(
        r[1].0,
        MaMode::ManualReview {
            review: Duration::from_secs(3_600)
        }
    );
    assert!(!r[1].1, "provider-b publishes on the cadence, not the SLA: {r:?}");
    assert_eq!(r[1].2, base.phase2().unwrap().scms_runs()[1].state.params.publish_on_cadence);

    // Only provider-b named: provider-a keeps the run-wide rule.
    let mut s = grid(5.0, 100.0);
    with_providers(&mut s, json!([{"name": "provider-b", "ma_mode": "automatic"}]));
    let r = rules(&Engine::build(s, "").expect("builds"));
    assert_eq!((r[0].0, r[1].0), (eight_h, MaMode::Automatic));
}

#[test]
fn a_bad_per_provider_setting_is_refused_by_name() {
    for (providers, field) in [
        (json!([{"name": "provider-b", "ma_mode": "lenient"}]), "ma_mode"),
        (
            json!([{"name": "provider-b", "ma_mode": "automatic", "ma_review_s": 60}]),
            "ma_review_s",
        ),
        (json!([{"name": "provider-b", "ma_review_s": 60}]), "ma_review_s"),
        (
            json!([{"name": "provider-a", "crl_publication": "weekly"}]),
            "crl_publication",
        ),
    ] {
        let mut s = grid(5.0, 100.0);
        with_providers(&mut s, providers);
        let err = Engine::build(s, "").expect_err("refused").to_string();
        assert!(err.contains(field), "{err}");
    }
}

fn records(r: &MemoryRecorder, channel: &str) -> Vec<Value> {
    r.records()
        .iter()
        .filter(|(_, rec)| rec.channel == channel)
        .map(|(_, rec)| serde_json::from_slice(&rec.json).expect("json"))
        .collect()
}

fn run(s: Scenario) -> (RunReport, MemoryRecorder, Engine) {
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    (report, rec, engine)
}

/// The Backend view names the home provider's entities by role and every other
/// provider's as `<provider>/<role>`, each with its own counters.
#[test]
fn the_backend_view_shows_every_providers_backend() {
    let (_, rec, _) = run(grid(6.0, 600.0));
    let views = records(&rec, "backend.state");
    let last = views.last().expect("a backend.state snapshot");
    let ids: Vec<&str> = last["entities"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["id"].as_str())
        .collect();
    for id in ["ra", "pca", "ma", "crlg", "provider-b/ra", "provider-b/ma", "provider-b/crlg"] {
        assert!(ids.contains(&id), "no {id}: {ids:?}");
    }
    let b_ra = last["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == "provider-b/ra")
        .unwrap();
    assert_eq!(b_ra["state"]["provider"], "provider-b");
    assert!(b_ra["name"].as_str().unwrap().contains("provider-b"));
}

/// `crl_delivery: mbms` with the whole fleet on provider-b: the revocation is decided by
/// provider-b's MA and signed by its CRL Generator, the bearer carries that CRL, and the
/// vehicles verify and install it with no frame refused.
#[test]
fn the_mbms_bearer_carries_every_providers_signed_crl() {
    let mut s = grid(90.0, 900.0);
    s.security.verification_policy = "verify-all".to_string();
    s.detection.local = vec![ModelChoice::new(v2xw_engine::phase2::LEGACY_12)];
    s.threats.attackers = vec![v2xw_engine::scenario::schema::Attacker {
        id: "threat/attacker/legacy/ConstPos".to_string(),
        count: Some(1),
        schedule: Some(v2xw_engine::scenario::schema::DilationWindow {
            from_s: 5.0,
            to_s: 90.0,
        }),
        ..Default::default()
    }];
    s.net.uu = Some(ModelChoice {
        id: "cellular/uu/fixed-latency".to_string(),
        params: json!({"preset": "4g-east-coast", "crl_delivery": "mbms"}),
    });
    s.security.protocol = Some(ModelChoice {
        id: v2xw_engine::phase2::CAMP_SCMS.to_string(),
        params: json!({
            "i_period_s": 30, "cert_lifetime_s": 31, "certs_per_period": 3,
            "cert_shuffle_window_s": 1, "first_batch_delay_s": 1,
            "download_poll_interval_s": 1, "report_shuffle_window_s": 2,
            "crl_cadence_s": 0, "crl_fetch_interval_s": 2
        }),
    });
    s.time.calendar = v2xw_engine::scenario::Calendar::Compressed;
    with_providers(
        &mut s,
        json!([
            {"name": "provider-a", "fleet_share": 0.0},
            {"name": "provider-b", "fleet_share": 1.0}
        ]),
    );
    let (report, _, engine) = run(s);
    let p = &report.phase2;
    assert_eq!(p.backend_errors, 0, "{}", p.first_backend_error);
    let runs = engine.phase2().unwrap().scms_runs();
    assert!(runs[0].state.crlg.entries.is_empty(), "provider-a revoked nothing");
    assert!(!runs[1].state.crlg.entries.is_empty(), "provider-b revoked");
    assert!(p.access.mbms_broadcasts > 0, "no eMBMS broadcast went out");
    assert!(p.crls_installed > 0, "no vehicle installed provider-b's list");
    assert_eq!(p.crl_frames_rejected, 0, "a vehicle refused the bearer's CRL");
}
