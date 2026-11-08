//! SCMS records on a built engine: the misbehaviour evidence under modelled signatures and
//! the records that follow it, the elector and root scenario keys, the CRL poll's internet
//! leg, the RSU's CRL `GET`, one list in one frame, and the deployed receiver under the US
//! profile. Each test's fault is named in it.

use std::path::{Path, PathBuf};

use serde_json::json;
use v2xw_engine::scenario::ModelChoice;
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// A small US-deployment grid on the compressed calendar: 20 s i-periods, 10 s CRL polls,
/// one ConstPos attacker from 5 s, automatic MA decisions, a fixed-latency Uu.
fn scenario(duration_s: f64, lifecycle: serde_json::Value) -> Scenario {
    let mut s = Scenario::load(root().join("scenarios/phase1-grid.yaml")).expect("loads");
    s.time.duration_s = duration_s;
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = json!(5);
        params["rows"] = json!(8);
    }
    s.world.buildings.enabled = false;
    s.actors.vehicles.demand.rate_veh_per_h = Some(1_200.0);
    s.security.verification_policy = "verify-all".to_string();
    s.metrics = vec!["all".to_string()];
    let id = v2xw_engine::phase2::CAMP_SCMS.to_string();
    s.actors.backend.protocol = Some(id.clone());
    let mut params = json!({
        "i_period_s": 20, "cert_lifetime_s": 21, "certs_per_period": 3, "pool_periods": 2,
        "topup_below_periods": 1, "cert_shuffle_window_s": 1, "first_batch_delay_s": 1,
        "download_poll_interval_s": 1, "report_shuffle_window_s": 2, "crl_cadence_s": 5,
        "crl_fetch_interval_s": 5
    });
    if let (Some(p), Some(l)) = (params.as_object_mut(), lifecycle.as_object()) {
        for (k, v) in l {
            p.insert(k.clone(), v.clone());
        }
    }
    s.security.protocol = Some(ModelChoice { id, params });
    s.time.calendar = v2xw_engine::scenario::Calendar::Compressed;
    s.net.uu = Some(ModelChoice {
        id: "cellular/uu/fixed-latency".to_string(),
        params: json!({"preset": "4g-east-coast"}),
    });
    s.detection.local = vec![ModelChoice::new(v2xw_engine::phase2::LEGACY_12)];
    s.threats.attackers = vec![v2xw_engine::scenario::schema::Attacker {
        id: "threat/attacker/legacy/ConstPos".to_string(),
        count: Some(1),
        schedule: Some(v2xw_engine::scenario::schema::DilationWindow {
            from_s: 5.0,
            to_s: duration_s,
        }),
        ..Default::default()
    }];
    s
}

fn with_scms(s: Scenario, scms: serde_json::Value) -> Scenario {
    let mut doc = serde_json::to_value(&s).expect("serialises");
    let target = &mut doc["security"]["scms"];
    if target.is_null() {
        *target = json!({});
    }
    if let (Some(t), Some(m)) = (target.as_object_mut(), scms.as_object()) {
        for (k, v) in m {
            t.insert(k.clone(), v.clone());
        }
    }
    Scenario::from_document(doc).expect("deserialises")
}

fn rsu(s: &mut Scenario, roles: &[&str]) {
    s.actors.rsus.push(v2xw_engine::scenario::Rsu {
        site: None,
        position_m: Some([450.0, 300.0, 0.0]),
        roles: roles.iter().map(ToString::to_string).collect(),
        profile: None,
        backhaul: None,
        backhaul_params: None,
    });
}

fn run(s: Scenario) -> (RunReport, MemoryRecorder) {
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    let report = engine.run(&mut recorder).expect("runs");
    (report, recorder)
}

fn records(r: &MemoryRecorder, channel: &str) -> Vec<serde_json::Value> {
    r.records()
        .iter()
        .filter(|(_, rec)| rec.channel == channel)
        .map(|(_, rec)| serde_json::from_slice(&rec.json).expect("json"))
        .collect()
}

/// Under `crypto_mode: modeled` an MA checks the evidence's BSMs
/// through the modelled backend, as a receiver would, so every report it holds was verified
/// (without it, 514 of 514 and 262 of 262 would read false). Fault: `verify_modelled`
/// answering false -> every report refused, `opened` 0, red. Also, on one run:
/// - U3b: the `reports-in` stage of `pki.case` names the rule's threshold (fault: the field
///   dropped from `case_evidence` -> red);
/// - U8: `node.security` names the next change, at or after the record's instant, and
///   the record after that instant shows a higher change count (fault: `next_change` None);
/// - U10: `backend.state`'s RA names `next_shuffle_release_ns` while reports wait, and it
///   is one window after the first waiting instant (fault: the field not set);
/// - U9: `pki.state`'s elector `cert` is a HashedId8 whose COER travels in `certificates`
///   (fault: the first 8 bytes of the HashedId48, as before).
#[test]
fn a_modelled_run_verifies_the_evidence_and_records_the_next_events() {
    let mut s = with_scms(scenario(60.0, json!({})), json!({"pki_view": true}));
    s.security.crypto_mode = v2xw_engine::scenario::CryptoModeSpec::Modeled;
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    let r = engine.run(&mut rec).expect("runs");
    assert!(r.phase2.cases_opened > 0, "no case: {:?}", r.phase2);
    let phase2 = engine.phase2_mut().expect("the SCMS runs");
    let mut opened = 0;
    for run in phase2.scms_runs() {
        assert!(
            run.state.pdu_rejections.is_empty(),
            "{:?}",
            run.state.pdu_rejections
        );
        for o in &run.state.ma.opened_reports {
            opened += 1;
            assert!(o.evidence_verified, "modelled evidence must verify");
        }
    }
    assert!(opened > 0, "no MA opened a report");

    // The records, each read on its own so one fault build shows each going red.
    let mut red: Vec<String> = Vec::new();

    // U3b.
    let cases = records(&rec, "pki.case");
    let named = cases
        .iter()
        .filter(|c| c["stage"] == "reports-in")
        .flat_map(|c| {
            c["detail"]["evidence"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        })
        .find(|e| !e["threshold"].is_null());
    match named {
        Some(e)
            if e["threshold"]["rule"] == v2xw_engine::phase2::MA_CORROBORATED
                && e["threshold"]["values"]
                    .as_array()
                    .is_some_and(|v| v.iter().any(|x| x["name"] == "min_events")) => {}
        other => red.push(format!(
            "U3b: no report names the rule's threshold: {other:?}"
        )),
    }

    // U8.
    let rows = records(&rec, "node.security");
    let with_next: Vec<_> = rows.iter().filter(|r| r["next_change"].is_u64()).collect();
    if with_next.is_empty() {
        red.push("U8: no node.security names a next change".to_string());
    }
    if let Some(r) = with_next
        .iter()
        .find(|r| r["next_change"].as_u64() < r["t"].as_u64())
    {
        red.push(format!("U8: a next change in the past: {r}"));
    }

    // U10.
    let window = 2_000_000_000u64;
    let mut armed = 0;
    for snap in records(&rec, "backend.state") {
        let t = snap["t"].as_u64().unwrap_or(0);
        for e in snap["entities"].as_array().into_iter().flatten() {
            if !e["id"].as_str().is_some_and(|id| id.ends_with("ra")) {
                continue;
            }
            if let Some(at) = e["state"]["next_shuffle_release_ns"].as_u64() {
                armed += 1;
                if at < t || at > t + window {
                    red.push(format!(
                        "U10: the release at {at} is outside the window of {t}"
                    ));
                }
            }
        }
    }
    if armed == 0 {
        red.push("U10: no snapshot named the shuffle's release".to_string());
    }

    // U9.
    let states = records(&rec, "pki.state");
    let first = states.first().expect("a pki.state");
    let electors: Vec<&serde_json::Value> = first["governance"]
        .as_array()
        .expect("governance")
        .iter()
        .filter(|c| c["role"] == "elector")
        .collect();
    // The production CTL of 13 Aug 2025 approves five electors, quorum 3.
    assert_eq!(electors.len(), 5, "the US deployment's electors");
    for e in electors {
        let id8 = e["cert"].as_str().unwrap_or_default();
        if first["certificates"].get(id8).is_none() {
            red.push(format!(
                "U9: elector certificate {id8} is not on the record"
            ));
        }
    }
    assert!(red.is_empty(), "{red:#?}");
}

/// The scenario keys of `Governance::replace_elector` and `Governance::add_root`: each is a
/// decision and a decided CTL, published at its SLA deadline (5 business days for an elector
/// replacement, 3 for a root addition, `ctl_publication_deadline`), so a 40 s run holds it
/// pending and publishes nothing more than the base. With `elector_replacement_pq: dual`
/// (G-11's key) the transition CTL carries the incoming elector's ML-DSA-65 signature.
/// Fault: the arm in `governance_due` removed -> the decision counts stay at the base's and
/// no CTL is pending, red.
#[test]
fn the_elector_and_root_keys_decide_and_publish_a_trust_list() {
    use v2xw_proto::scms::governance::{
        CTL_ADD_ELECTOR_BUSINESS_DAYS, CTL_ADD_ROOT_BUSINESS_DAYS, GovernanceEvent, SignedCtl,
    };
    const DAY_NS: u64 = 86_400_000_000_000;
    let gov_run = |scms: Option<serde_json::Value>| {
        let s = match scms {
            Some(m) => with_scms(scenario(40.0, json!({})), m),
            None => scenario(40.0, json!({})),
        };
        assert!(v2xw_engine::scenario::validate::validate(&s).is_empty());
        let mut engine = Engine::build(s, "").expect("builds");
        let report = engine.run(&mut MemoryRecorder::new()).expect("runs");
        let gov = &engine.phase2().expect("a backend").scms().state.gov;
        let log: Vec<GovernanceEvent> = gov.log.clone();
        let pending: Vec<SignedCtl> = gov.pending.iter().map(|(_, c)| c.clone()).collect();
        (report, log, pending)
    };
    // Decided at 10 s, published `days` business days later: never sooner, and within the
    // weekends and holidays a week of business days can span.
    let decided = |log: &[GovernanceEvent], kind: &str, days: u32| {
        let e = log
            .iter()
            .find(|e| e.kind == kind)
            .unwrap_or_else(|| panic!("no {kind}: {log:?}"));
        assert_eq!(e.decided_at, 10 * 1_000_000_000, "{e:?}");
        let gap = e.published_at.expect("a publication time") - e.decided_at;
        assert!(
            gap >= u64::from(days) * DAY_NS && gap <= u64::from(days + 6) * DAY_NS,
            "{e:?}"
        );
    };
    let (base, _, base_pending) = gov_run(None);
    let (replaced, log, pending) = gov_run(Some(
        json!({"lifecycle": {"elector_replacement_at_s": 10.0, "elector_replacement_pq": "dual"}}),
    ));
    assert_eq!(replaced.phase2.gov_decisions, base.phase2.gov_decisions + 1);
    decided(&log, "eac-replace-elector", CTL_ADD_ELECTOR_BUSINESS_DAYS);
    assert_eq!(pending.len(), base_pending.len() + 1);
    assert!(
        !pending
            .last()
            .expect("the transition CTL")
            .pq_signatures
            .is_empty(),
        "dual"
    );
    assert_eq!(
        replaced.phase2.ctls_published, base.phase2.ctls_published,
        "before its deadline"
    );
    let (added, log, pending) = gov_run(Some(json!({"lifecycle": {"root_addition_at_s": 10.0}})));
    assert_eq!(added.phase2.gov_decisions, base.phase2.gov_decisions + 1);
    decided(&log, "eac-add-root", CTL_ADD_ROOT_BUSINESS_DAYS);
    assert_eq!(pending.len(), base_pending.len() + 1);
    assert_eq!(
        added.phase2.ctls_published, base.phase2.ctls_published,
        "before its deadline"
    );
}

/// A cellular CRL poll crosses the internet between the city and the provider's region,
/// both ways, as every other device exchange does (RASITE: moving the
/// sites moved no fetch). The regional model against the flat one, same seed: the mean rises
/// by about the round trip. Fault: `internet_round_trip_ns` 0 -> the two means are equal, red.
#[test]
fn a_cellular_crl_poll_pays_the_internet_leg() {
    let fetch = |r: &RunReport| {
        r.phase2
            .access
            .exchange_latency
            .get("crl-fetch")
            .copied()
            .expect("CRL fetches")
    };
    let (region, _) = run(with_scms(
        scenario(40.0, json!({})),
        json!({"deployment": {"preset": "us-production-2025", "latency": "region"}}),
    ));
    let (flat, _) = run(with_scms(
        scenario(40.0, json!({})),
        json!({"deployment": {"preset": "us-production-2025", "latency": "flat"}}),
    ));
    let (r, f) = (fetch(&region), fetch(&flat));
    assert!(r.count > 0 && f.count > 0);
    assert!(
        r.mean_ms() > f.mean_ms() + 0.5,
        "region {} ms against flat {} ms",
        r.mean_ms(),
        f.mean_ms()
    );
}

/// A roadside unit in the certificate cycle asks for its CRL before it gets it: the `GET`
/// goes up its line and is charged. Fault: the uplink `rsu_send` and its note removed ->
/// `rsu_crl_requests` 0, red.
#[test]
fn a_units_crl_download_charges_its_request() {
    let mut s = scenario(40.0, json!({}));
    rsu(&mut s, &["crl"]);
    let (r, _) = run(s);
    assert!(r.phase2.rsus_in_cert_cycle > 0, "no unit in the cycle");
    assert!(r.phase2.rsu_crls_installed > 0, "no unit fetched a list");
    assert!(r.phase2.rsu_crl_requests >= r.phase2.rsu_crls_installed);
    assert!(r.phase2.rsu_crl_request_bytes >= 100 * r.phase2.rsu_crl_requests);
}

/// One list, one SDU (decided by realism, 2026-11-02): a list that fits goes out whole,
/// one frame a broadcast; one over the network layer's MTU is refused there and counted,
/// never split into frames of entries (a 33 kB list was 25 frames a broadcast, 11 % of the
/// air). Devices still fetch it over IP. Fault: the old split by `per_frame` -> frames on the
/// air for the large list, red.
#[test]
fn a_roadside_unit_airs_one_list_in_one_frame_or_none() {
    let mut small = scenario(40.0, json!({}));
    rsu(&mut small, &["crl"]);
    let (small, _) = run(small);
    assert!(small.phase2.crl_broadcasts > 0, "a short list goes out");
    assert_eq!(small.phase2.crl_frames_refused_mtu, 0);

    let mut large = scenario(40.0, json!({"crl_background_entries": 1000}));
    rsu(&mut large, &["crl"]);
    let (large, _) = run(large);
    assert_eq!(large.phase2.crl_broadcasts, 0, "a 33 kB list was aired");
    assert!(large.phase2.crl_frames_refused_mtu > 0);
    assert!(large.phase2.crl_downloads > 0, "devices still fetch it");
}

/// an empty `detection.local` under the US deployment runs the SM-MBR suite,
/// and its receiver is the deployed one; an explicit 0 is the raw fix, kept to study it (it
/// warns), and the research suite stays on what it names. Fault: `us_receiver` a no-op -> the
/// first assertion red.
#[test]
fn the_us_profile_runs_the_deployed_receiver_unless_told_otherwise() {
    let params = |s: &Scenario| v2xw_engine::wiring::gnss_params(s);
    let mut us = scenario(40.0, json!({}));
    us.detection.local.clear();
    assert_eq!(
        v2xw_engine::phase2::local_suite_id(&us),
        v2xw_engine::phase2::US_SM_MBR
    );
    let p = params(&us);
    assert_eq!((p.smoothing_tau_s, p.static_hold_mps), (100.0, 0.3));
    assert!(v2xw_engine::scenario::us_raw_gnss_warnings(&us).is_empty());

    let mut doc = serde_json::to_value(&us).expect("serialises");
    doc["actors"]["vehicles"]["gnss"] = json!({"smoothing_tau_s": 0.0});
    let raw = Scenario::from_document(doc).expect("deserialises");
    assert_eq!(params(&raw).smoothing_tau_s, 0.0, "the explicit raw fix");
    assert_eq!(v2xw_engine::scenario::us_raw_gnss_warnings(&raw).len(), 1);

    let research = scenario(40.0, json!({}));
    assert_eq!(params(&research).smoothing_tau_s, 0.0, "the research suite");
}
