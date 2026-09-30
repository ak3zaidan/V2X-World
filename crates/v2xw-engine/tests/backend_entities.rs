//! Every credential-system entity wired into a run, as a researcher sees it: the backend's
//! state on `backend.state`, the enrolment certificate's renewal and expiry, the ETSI
//! butterfly authorization, compromised roadside units, and the time a post-quantum
//! signature costs.
//!
//! Each check is paired with the same scenario with the thing under test removed, and
//! asserts the difference.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use v2xw_engine::scenario::ModelChoice;
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// The Phase 1 grid, five avenues by eight streets, with the SCMS on.
fn grid(duration_s: f64, rate: f64) -> Scenario {
    let mut s =
        Scenario::load(scenarios().join("phase1-grid.yaml")).expect("the shipped grid loads");
    s.time.duration_s = duration_s;
    s.actors.vehicles.demand.rate_veh_per_h = Some(rate);
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = json!(5);
        params["rows"] = json!(8);
    }
    s.metrics = vec!["all".to_string()];
    s.actors.backend.protocol = Some(v2xw_engine::phase2::CAMP_SCMS.to_string());
    s
}

fn lifecycle(s: &mut Scenario, protocol: &str, params: Value) {
    s.actors.backend.protocol = Some(protocol.to_string());
    s.security.protocol = Some(ModelChoice {
        id: protocol.to_string(),
        params,
    });
    if protocol == v2xw_engine::phase2::ETSI_PKI {
        s.security.envelope = "etsi103097".to_string();
    }
}

fn cellular(s: &mut Scenario) {
    s.net.uu = Some(ModelChoice {
        id: "cellular/uu/fixed-latency".to_string(),
        params: json!({"preset": "4g-east-coast"}),
    });
}

fn run(s: Scenario) -> (RunReport, MemoryRecorder) {
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "the test scenario must load: {errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    let report = engine.run(&mut recorder).expect("runs");
    (report, recorder)
}

fn records(r: &MemoryRecorder, channel: &str) -> Vec<Value> {
    r.records()
        .iter()
        .filter(|(_, rec)| rec.channel == channel)
        .map(|(_, rec)| serde_json::from_slice(&rec.json).expect("json"))
        .collect()
}

/// A 20 s i-period, three certificates a period, two held, a top-up when one is left.
fn compressed() -> Value {
    json!({
        "i_period_s": 20,
        "cert_lifetime_s": 21,
        "certs_per_period": 3,
        "pool_periods": 2,
        "topup_below_periods": 1,
        "cert_shuffle_window_s": 1,
        "first_batch_delay_s": 1,
        "download_poll_interval_s": 1
    })
}

fn entity<'a>(view: &'a Value, id: &str) -> &'a Value {
    view["entities"]
        .as_array()
        .and_then(|a| a.iter().find(|e| e["id"] == id))
        .unwrap_or_else(|| panic!("no {id} in the view"))
}

fn edge(view: &Value, from: &str, to: &str) -> bool {
    view["edges"]
        .as_array()
        .is_some_and(|a| a.iter().any(|e| e["from"] == from && e["to"] == to))
}

#[test]
fn the_backend_state_names_every_scms_entity_once_a_second() {
    let mut s = grid(30.0, 400.0);
    lifecycle(&mut s, v2xw_engine::phase2::CAMP_SCMS, compressed());
    cellular(&mut s);
    let (report, rec) = run(s);
    let views = records(&rec, "backend.state");
    println!(
        "{} snapshots; {} top-ups completed",
        views.len(),
        report.phase2.topups_completed
    );
    assert!(
        (28..=32).contains(&views.len()),
        "one snapshot a simulated second over 30 s, got {}",
        views.len()
    );
    let ts: Vec<u64> = views.iter().filter_map(|v| v["t"].as_u64()).collect();
    assert!(ts.windows(2).all(|w| w[0] < w[1]), "snapshots go forward");
    let last = views.last().expect("one");
    assert_eq!(last["system"], "scms");
    for id in [
        "manager",
        "pg",
        "electors",
        "root",
        "ica",
        "dcm",
        "eca",
        "lop",
        "ra",
        "la1",
        "la2",
        "pca",
        "ma",
        "crlg",
        "crl-store",
        "crl-broadcast",
        "ee",
    ] {
        let e = entity(last, id);
        assert!(
            e["name"].as_str().is_some_and(|n| !n.is_empty()),
            "{id} unnamed"
        );
    }
    assert!(entity(last, "root")["state"]["certs_issued"].as_u64() >= Some(5));
    assert_eq!(entity(last, "root")["online"], false);
    assert!(entity(last, "ee")["state"]["vehicles"].as_u64() > Some(0));
    // Every vehicle was bootstrapped and provisioned before the run: the PCA and the LAs
    // hold their state even though the scratch kernel's messages are not in the log.
    assert!(entity(last, "pca")["state"]["certs_issued"].as_u64() > Some(0));
    assert!(entity(last, "la1")["state"]["chains"].as_u64() > Some(0));
    assert!(entity(last, "dcm")["state"]["devices_bootstrapped"].as_u64() > Some(0));
    assert!(entity(last, "ee")["state"]["trust_installed"].as_u64() > Some(0));
    // The in-run top-ups: device -> LOP -> RA -> LAs/PCA and back through the LOP.
    assert!(report.phase2.topups_completed > 0, "no top-up to draw");
    for (a, b) in [
        ("ee", "lop"),
        ("lop", "ra"),
        ("ra", "la1"),
        ("ra", "la2"),
        ("ra", "pca"),
        ("ra", "lop"),
        ("lop", "ee"),
    ] {
        assert!(edge(last, a, b), "no {a} -> {b} edge");
    }
    assert!(
        !edge(last, "ee", "ra"),
        "a vehicle reached the RA without the LOP"
    );
    let ra = entity(last, "ra");
    assert!(ra["queue"]["served"].as_u64() > Some(0));
    assert!(ra["traffic"]["received"].as_u64() > Some(0));

    // The control: no credential system, no snapshots.
    let mut plain = grid(10.0, 400.0);
    plain.actors.backend.protocol = None;
    plain.security.protocol = None;
    let (_, rec) = run(plain);
    assert!(records(&rec, "backend.state").is_empty());
}

#[test]
fn an_expiring_enrolment_is_renewed_and_an_expired_one_cannot_top_up() {
    // Enrolment certificates that last 40 s, so most of a fleet's run out inside a minute.
    let base = |lead: f64| {
        let mut s = grid(60.0, 1_800.0);
        let mut p = compressed();
        p["enrolment_lifetime_s"] = json!(40);
        p["reenrol_lead_s"] = json!(lead);
        lifecycle(&mut s, v2xw_engine::phase2::CAMP_SCMS, p);
        cellular(&mut s);
        s
    };
    let (renewing, rec) = run(base(20.0));
    let p = &renewing.phase2;
    println!(
        "renewing: {} started, {} completed, {} refused; {} top-ups refused",
        p.reenrolments_started, p.reenrolments_completed, p.reenrolments_refused, p.topups_refused
    );
    assert!(
        p.reenrolments_started > 0,
        "no certificate came within its lead"
    );
    assert!(p.reenrolments_completed > 0, "no successor was installed");
    assert!(
        records(&rec, "sec.cert")
            .iter()
            .any(|r| r["event"] == "reenrolled")
    );
    assert_eq!(p.backend_errors, 0, "{}", p.first_backend_error);

    // The control: no renewal lead. A certificate is renewed only once it has already run
    // out, the ECA will not renew an expired one, and a vehicle whose enrolment lapsed can
    // sign no request: it asks for no more pseudonyms and its pool runs dry. (A request
    // signed with an expired certificate is refused by the RA as well; the protocol test
    // `governance.rs` drives that one.)
    let (lapsing, rec) = run(base(0.0));
    let q = &lapsing.phase2;
    println!(
        "lapsing: {} renewals refused, {} completed, {} top-ups completed, {} starved \
         (renewing: {} top-ups completed, {} starved)",
        q.reenrolments_refused,
        q.reenrolments_completed,
        q.topups_completed,
        q.vehicles_starved,
        p.topups_completed,
        p.vehicles_starved
    );
    assert!(
        q.reenrolments_refused > 0,
        "an expired certificate was renewed"
    );
    assert_eq!(q.reenrolments_completed, 0);
    assert_eq!(q.topups_refused_blocklisted, 0);
    assert!(
        q.topups_completed < p.topups_completed,
        "vehicles whose enrolment lapsed kept topping up"
    );
    assert!(
        records(&rec, "sec.cert")
            .iter()
            .any(|r| r["event"] == "reenrol-refused")
    );
}

#[test]
fn the_etsi_butterfly_authorization_tops_up_and_the_aa_sees_counts_only() {
    let base = |butterfly: bool| {
        let mut s = grid(45.0, 400.0);
        let mut p = compressed();
        p["etsi_butterfly"] = json!(u8::from(butterfly));
        lifecycle(&mut s, v2xw_engine::phase2::ETSI_PKI, p);
        cellular(&mut s);
        s
    };
    let (bf, rec) = run(base(true));
    let p = &bf.phase2;
    let views = records(&rec, "backend.state");
    let last = views.last().expect("snapshots");
    assert_eq!(last["system"], "ccms");
    let aa = entity(last, "aa");
    println!(
        "butterfly: {} top-ups completed, AA {} batches, {} tickets, {} validations",
        p.topups_completed,
        aa["state"]["butterfly_batches"],
        aa["state"]["tickets_issued"],
        aa["state"]["validations_requested"]
    );
    assert!(p.topups_completed > 0, "no butterfly batch was installed");
    assert!(aa["state"]["butterfly_batches"].as_u64() > Some(0));
    assert_eq!(aa["state"]["validations_requested"], 0);
    assert!(edge(last, "ea", "aa") && edge(last, "aa", "ea"));
    assert_eq!(p.backend_errors, 0, "{}", p.first_backend_error);
    // The trust lists: the TLM and the Root CA signed one list each, and every station
    // that joined fetched both from the Distribution Centre over its own access.
    let cpoc = entity(last, "cpoc");
    println!(
        "trust lists: {} fetches, {} installed; DC answered {}, {} current; ECTL {}",
        p.trust_fetches,
        p.trust_lists_installed,
        cpoc["state"]["fetches_answered"],
        cpoc["state"]["answered_current"],
        entity(last, "tlm")["state"]["ectl_sequence"]
    );
    assert!(p.trust_lists_installed > 0, "no station installed the ECTL");
    assert!(cpoc["state"]["fetches_answered"].as_u64() >= Some(p.trust_fetches));
    assert_eq!(entity(last, "tlm")["state"]["ectl_sequence"], 1);
    assert_eq!(entity(last, "rca")["state"]["ca_crl_sequence"], 1);
    assert!(edge(last, "ee", "cpoc") && edge(last, "cpoc", "ee"));
    assert!(edge(last, "tlm", "cpoc") && edge(last, "rca", "cpoc"));
    assert!(
        records(&rec, "sec.cert")
            .iter()
            .any(|r| r["event"] == "trust-list-installed")
    );

    let (standard, rec) = run(base(false));
    let views = records(&rec, "backend.state");
    let aa = entity(views.last().expect("snapshots"), "aa");
    assert!(standard.phase2.topups_completed > 0);
    assert_eq!(aa["state"]["butterfly_batches"], 0);
    assert!(aa["state"]["validations_requested"].as_u64() > Some(0));
}

/// Eight report-forwarding units over the grid, a ConstPos attacker and no modems: every
/// report reaches the authority through a unit.
fn relayed(compromised: Option<&str>) -> Scenario {
    let mut s = grid(60.0, 900.0);
    s.security.verification_policy = "verify-all".to_string();
    s.detection.local = vec![ModelChoice::new(v2xw_engine::phase2::LEGACY_12)];
    s.threats.attackers = vec![v2xw_engine::scenario::schema::Attacker {
        id: "threat/attacker/legacy/ConstPos".to_string(),
        count: Some(1),
        schedule: Some(v2xw_engine::scenario::schema::DilationWindow {
            from_s: 5.0,
            to_s: 60.0,
        }),
        ..Default::default()
    }];
    lifecycle(
        &mut s,
        v2xw_engine::phase2::CAMP_SCMS,
        json!({"report_shuffle_window_s": 1}),
    );
    for x in [150.0, 450.0, 750.0, 1050.0] {
        for y in [150.0, 450.0] {
            s.actors.rsus.push(v2xw_engine::scenario::Rsu {
                site: None,
                position_m: Some([x, y, 0.0]),
                roles: vec!["report-forward".to_string()],
                profile: None,
                backhaul: None,
            });
        }
    }
    if let Some(kind) = compromised {
        s.threats.compromised_rsus = (0..8).collect();
        s.threats.compromised_rsu_attack = Some(ModelChoice {
            id: v2xw_threat::attack_rsu::MODEL_ID.to_string(),
            params: json!({"kind": kind}),
        });
    }
    s
}

#[test]
fn a_compromised_unit_drops_or_poisons_the_reports_it_forwards() {
    let (honest, _) = run(relayed(None));
    let (suppressing, _) = run(relayed(Some("SuppressForwardedReports")));
    let (poisoning, rec) = run(relayed(Some("PoisonForwardedReports")));
    let (h, s, p) = (&honest.phase2, &suppressing.phase2, &poisoning.phase2);
    println!(
        "honest: {} relayed, {} at the proxy, {} revoked honest | suppress: {} dropped, {} \
         at the proxy | poison: {} forged, {} at the proxy, {} decisions, {} honest revoked",
        h.reports_uploaded_relay,
        h.reports_received,
        h.revoked_honest,
        s.rsu_reports_suppressed,
        s.reports_received,
        p.rsu_reports_poisoned,
        p.reports_received,
        p.ma_revoke_decisions,
        p.revoked_honest
    );
    assert!(
        h.reports_uploaded_relay > 0,
        "nothing was relayed, so this proves nothing"
    );
    assert_eq!(h.compromised_rsus, 0);
    assert_eq!(h.rsu_reports_suppressed + h.rsu_reports_poisoned, 0);
    assert_eq!(s.compromised_rsus, 8);
    assert!(
        s.rsu_reports_suppressed > 0,
        "no relayed report was dropped"
    );
    assert!(
        s.reports_received < h.reports_received,
        "suppression let as many reports through as honest units"
    );
    assert!(p.rsu_reports_poisoned > 0, "no forwarded report was forged");
    // The forgeries reach the authority as the units' own, trusted reports.
    assert!(p.reports_received > 0);
    let ma = records(&rec, "ma.report");
    assert!(!ma.is_empty(), "the authority ingested nothing");
}

#[test]
fn a_compromised_unit_cannot_forge_the_crl() {
    let base = |compromised: bool| {
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
        cellular(&mut s);
        lifecycle(
            &mut s,
            v2xw_engine::phase2::CAMP_SCMS,
            json!({
                "i_period_s": 30, "cert_lifetime_s": 31, "certs_per_period": 3,
                "cert_shuffle_window_s": 1, "first_batch_delay_s": 1,
                "download_poll_interval_s": 1, "report_shuffle_window_s": 2,
                "crl_cadence_s": 0, "crl_fetch_interval_s": 600,
                "crl_broadcast_interval_s": 2
            }),
        );
        for x in [150.0, 450.0, 750.0, 1050.0] {
            for y in [150.0, 450.0] {
                s.actors.rsus.push(v2xw_engine::scenario::Rsu {
                    site: None,
                    position_m: Some([x, y, 0.0]),
                    roles: vec!["crl".to_string()],
                    profile: None,
                    backhaul: None,
                });
            }
        }
        if compromised {
            s.threats.compromised_rsus = (0..8).collect();
            s.threats.compromised_rsu_attack = Some(ModelChoice {
                id: v2xw_threat::attack_rsu::MODEL_ID.to_string(),
                params: json!({"kind": "FalseCrl", "fabricated_entries": 2}),
            });
        }
        s
    };
    let (honest, _) = run(base(false));
    let (forging, _) = run(base(true));
    let (h, f) = (&honest.phase2, &forging.phase2);
    println!(
        "honest: {} revoked attackers, {} top-ups refused as blocklisted",
        h.revoked_attackers, h.topups_refused_blocklisted
    );
    println!(
        "honest: {} issued, {} RSU frames, {} installs, {} rejected | forging: {} frames \
         forged, {} rejected, {} installs",
        h.crls_issued,
        h.crl_broadcasts,
        h.crls_installed,
        h.crl_frames_rejected,
        f.rsu_crl_frames_forged,
        f.crl_frames_rejected,
        f.crls_installed
    );
    // The revoked attacker's next top-up is refused at the RA: its enrolment certificate
    // is on the blocklist (the passive half of revocation).
    assert!(h.revoked_attackers >= 1, "the attacker was not revoked");
    assert!(
        h.topups_refused_blocklisted >= 1,
        "a revoked enrolment got a top-up"
    );
    assert!(
        h.crls_issued > 0 && h.crl_broadcasts > 0,
        "no list was broadcast"
    );
    assert!(
        h.crls_installed > 0,
        "honest units' lists must be installed"
    );
    assert_eq!(h.crl_frames_rejected, 0, "an honest list was rejected");
    assert!(f.rsu_crl_frames_forged > 0, "no frame was forged");
    assert!(f.crl_frames_rejected > 0, "a forged frame was accepted");
    assert!(
        f.crls_installed < h.crls_installed,
        "the forged frames installed as much as the honest ones"
    );
}

#[test]
fn a_hybrid_signature_costs_its_post_quantum_time() {
    let base = |signature: &str| {
        let mut s = grid(20.0, 1_800.0);
        s.actors.backend.protocol = None;
        s.nodes.default_obu = "obu/generic-automotive-soc-no-hsm".to_string();
        s.security.signature = signature.to_string();
        s.security.verification_policy = "verify-all".to_string();
        // A hybrid SPDU is above the MTU with a full certificate; fragment it.
        s.net.fragmenter = Some(ModelChoice::new("fragmenter/generic-sdu"));
        s
    };
    let costs = |rec: &MemoryRecorder| {
        // A verification's service time: when its server started it to when it finished.
        let verify: Vec<u64> = records(rec, "node.verify")
            .iter()
            .filter_map(|r| Some((r["t_done"].as_u64()? - r["t_start"].as_u64()?) / 1_000))
            .collect();
        let sign: Vec<u64> = records(rec, "node.tx")
            .iter()
            .filter_map(|r| {
                Some(
                    r["t_signed"]
                        .as_u64()?
                        .saturating_sub(r["t_sign_start"].as_u64()?),
                )
            })
            .collect();
        (verify, sign)
    };
    let (_, classical) = run(base("ecdsa-p256"));
    let (_, hybrid) = run(base("hybrid-falcon512-ecdsa-p256"));
    let v = records(&classical, "node.verify");
    println!("{} node.verify records; first: {:?}", v.len(), v.first());
    let (cv, cs) = costs(&classical);
    let (hv, hs) = costs(&hybrid);
    let mean = |v: &[u64]| v.iter().sum::<u64>() as f64 / v.len().max(1) as f64;
    println!(
        "verify: P-256 {:?} us, hybrid {:?} us; sign (start to signed): P-256 {:.0} ns, \
         hybrid {:.0} ns",
        cv.first(),
        hv.first(),
        mean(&cs),
        mean(&hs)
    );
    assert!(!cv.is_empty() && !hv.is_empty(), "nothing was verified");
    // The generic automotive SoC: ECDSA P-256 verify 645 us, Falcon-512 verify 127 us.
    assert!(cv.iter().all(|&c| c == 645), "{:?}", &cv[..cv.len().min(5)]);
    assert!(
        hv.iter().all(|&c| c == 645 + 127),
        "{:?}",
        &hv[..hv.len().min(5)]
    );
    // Signing: 244 us against 244 + 1,625 us. The stamp includes the J2945/1 hand-off
    // jitter, which only adds, so the shortest frame bounds the service time from above
    // and a hybrid frame can never be shorter than its two signatures.
    let min = |v: &[u64]| v.iter().copied().min().unwrap_or(0);
    assert!(
        min(&hs) >= 1_869_000,
        "a hybrid frame signed in {} ns",
        min(&hs)
    );
    assert!(min(&cs) < 1_869_000, "a P-256 frame took {} ns", min(&cs));
    assert!(
        mean(&hs) - mean(&cs) > 1_200_000.0,
        "the hybrid's signing time is not charged: {} vs {}",
        mean(&hs),
        mean(&cs)
    );

    // The reference OBU publishes no post-quantum figure, and a hybrid on it is refused.
    let mut refused = base("hybrid-mldsa44-ecdsa-p256");
    refused.nodes.default_obu = "obu/unex-obu-301-craton2".to_string();
    let errors = v2xw_engine::scenario::validate::validate(&refused);
    assert!(
        errors
            .iter()
            .any(|e| e.to_string().contains("does not publish both halves")),
        "{errors:?}"
    );
}

/// A hybrid `security.signature` is the credential system's scheme too: the same top-ups
/// run, but every certificate the PCA issues carries the post-quantum key and signature,
/// the PCA signs each one twice, and the device generates a post-quantum key per
/// certificate because there is no post-quantum butterfly (`v2xw_proto::hybrid`).
#[test]
fn a_hybrid_signature_is_the_credential_systems_scheme_too() {
    let base = |signature: &str| {
        let mut s = grid(30.0, 400.0);
        lifecycle(&mut s, v2xw_engine::phase2::CAMP_SCMS, compressed());
        cellular(&mut s);
        s.nodes.default_obu = "obu/generic-automotive-soc-no-hsm".to_string();
        s.security.signature = signature.to_string();
        s.net.fragmenter = Some(ModelChoice::new("fragmenter/generic-sdu"));
        s
    };
    let (classic, crec) = run(base("ecdsa-p256"));
    let (hybrid, hrec) = run(base("hybrid-falcon512-ecdsa-p256"));
    let last = |rec: &MemoryRecorder| records(rec, "backend.state").pop().expect("a view");
    let (cv, hv) = (last(&crec), last(&hrec));
    assert_eq!(cv["signature"], "ecdsa-p256");
    assert_eq!(hv["signature"], "hybrid-falcon512-ecdsa-p256");
    let op = |v: &Value, id: &str, key: &str| entity(v, id)["ops"][key].as_u64().unwrap_or(0);
    let edge_bytes = |v: &Value, from: &str, to: &str| {
        v["edges"]
            .as_array()
            .and_then(|a| a.iter().find(|e| e["from"] == from && e["to"] == to))
            .and_then(|e| e["bytes"].as_u64())
            .unwrap_or(0)
    };
    println!(
        "top-ups: classic {} hybrid {}; LOP->device bytes classic {} hybrid {}; device->LOP \
         classic {} hybrid {}; PCA falcon signs {}",
        classic.phase2.topups_completed,
        hybrid.phase2.topups_completed,
        edge_bytes(&cv, "lop", "ee"),
        edge_bytes(&hv, "lop", "ee"),
        edge_bytes(&cv, "ee", "lop"),
        edge_bytes(&hv, "ee", "lop"),
        op(&hv, "pca", "falcon-512 sign"),
    );
    assert!(classic.phase2.topups_completed > 0 && hybrid.phase2.topups_completed > 0);
    // Classical: no post-quantum operation anywhere in the backend.
    assert_eq!(op(&cv, "pca", "falcon-512 sign"), 0);
    // Hybrid: the PCA signed each certificate's post-quantum half, the device made a key
    // for each and checked each signature.
    assert!(op(&hv, "pca", "falcon-512 sign") >= hybrid.phase2.certs_topped_up);
    assert!(op(&hv, "ee", "falcon-512 keygen") >= hybrid.phase2.certs_topped_up);
    assert!(op(&hv, "ee", "falcon-512 verify") >= hybrid.phase2.certs_topped_up);
    // Per top-up, the downloads and uploads over the device's cellular link grew by at
    // least the post-quantum key and signature of each certificate.
    let per = |bytes: u64, n: u64| bytes as f64 / n.max(1) as f64;
    let extra = f64::from(897 + 666) * 3.0;
    let down = |v: &Value, r: &v2xw_engine::RunReport| {
        per(edge_bytes(v, "lop", "ee"), r.phase2.topups_completed)
    };
    assert!(
        down(&hv, &hybrid) - down(&cv, &classic) >= extra,
        "download per top-up {} vs {}",
        down(&hv, &hybrid),
        down(&cv, &classic)
    );
    let up = |v: &Value, r: &v2xw_engine::RunReport| {
        per(edge_bytes(v, "ee", "lop"), r.phase2.topups_started)
    };
    assert!(up(&hv, &hybrid) > up(&cv, &classic) + 3.0 * 897.0);
    assert_eq!(
        hybrid.phase2.backend_errors, 0,
        "{}",
        hybrid.phase2.first_backend_error
    );
}

/// The shipped `scenarios/credential-lifecycle.yaml` — what a researcher opens to watch the
/// SCMS work — loads, validates and, in its first three minutes, does what its header says:
/// top-ups through the LOP, enrolment renewals at the ECA, and misbehaviour reports
/// reaching the MA.
#[test]
fn the_credential_lifecycle_scenario_shows_every_stage() {
    let mut s = Scenario::load(scenarios().join("credential-lifecycle.yaml"))
        .expect("the shipped scenario loads");
    // The shipped file validates as it is; three minutes are enough to see every stage, and
    // the attack window is cut to the shorter run.
    assert!(v2xw_engine::scenario::validate::validate(&s).is_empty());
    s.time.duration_s = 180.0;
    if let Some(w) = s.threats.attackers[0].schedule.as_mut() {
        w.to_s = 180.0;
    }
    let (report, rec) = run(s);
    let p = &report.phase2;
    println!(
        "top-ups {}/{} ({} certs), renewals {}/{}, reports {} sent {} at the MA, {} \
         decisions, {} CRL versions, {} installs, {} RSU CRL frames",
        p.topups_completed,
        p.topups_started,
        p.certs_topped_up,
        p.reenrolments_completed,
        p.reenrolments_started,
        p.reports_sent,
        p.reports_at_ma,
        p.ma_revoke_decisions,
        p.crl_versions_published,
        p.crls_installed,
        p.crl_broadcasts
    );
    assert!(p.topups_completed > 0, "no top-up completed");
    assert!(p.reenrolments_completed > 0, "no enrolment was renewed");
    assert!(p.reports_at_ma > 0, "no report reached the MA");
    assert_eq!(p.backend_errors, 0, "{}", p.first_backend_error);
    let last = records(&rec, "backend.state").pop().expect("a view");
    for (a, b) in [
        ("ee", "lop"),
        ("lop", "ra"),
        ("ra", "pca"),
        ("ra", "la1"),
        ("lop", "ee"),
    ] {
        assert!(edge(&last, a, b), "no {a} -> {b} edge");
    }
}

/// The shipped `scenarios/ccms-lifecycle.yaml`, the European counterpart: CAMs over
/// GeoNetworking, butterfly tickets, the Distribution Centre's trust lists and TS 103 759
/// reports, in its first three minutes.
#[test]
fn the_ccms_lifecycle_scenario_shows_every_stage() {
    let mut s = Scenario::load(scenarios().join("ccms-lifecycle.yaml"))
        .expect("the shipped scenario loads");
    assert!(v2xw_engine::scenario::validate::validate(&s).is_empty());
    s.time.duration_s = 180.0;
    if let Some(w) = s.threats.attackers[0].schedule.as_mut() {
        w.to_s = 180.0;
    }
    let (report, rec) = run(s);
    let p = &report.phase2;
    let last = records(&rec, "backend.state").pop().expect("a view");
    let aa = entity(&last, "aa");
    println!(
        "top-ups {}/{} ({} tickets), trust fetches {} ({} installed), reports {} sent {} at \
         the MA, {} decisions, {} refused as blocklisted; AA {} batches, {} validations",
        p.topups_completed,
        p.topups_started,
        p.certs_topped_up,
        p.trust_fetches,
        p.trust_lists_installed,
        p.reports_sent,
        p.reports_at_ma,
        p.ma_revoke_decisions,
        p.topups_refused_blocklisted,
        aa["state"]["butterfly_batches"],
        aa["state"]["validations_requested"]
    );
    assert_eq!(last["system"], "ccms");
    assert!(p.topups_completed > 0, "no ticket batch was installed");
    assert!(aa["state"]["butterfly_batches"].as_u64() > Some(0));
    assert!(p.trust_lists_installed > 0, "no station installed the ECTL");
    assert!(p.reports_at_ma > 0, "no report reached the MA");
    assert_eq!(p.backend_errors, 0, "{}", p.first_backend_error);
    for (a, b) in [
        ("ee", "ea"),
        ("ea", "aa"),
        ("aa", "ea"),
        ("ee", "cpoc"),
        ("cpoc", "ee"),
    ] {
        assert!(edge(&last, a, b), "no {a} -> {b} edge");
    }
}
