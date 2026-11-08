//! Acceptance on a built engine: the default deployment is the two US providers on
//! one CTL, every node's trust reaches the other provider's ACA through its root, and a
//! provider kept off the CTL is untrusted.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use v2xw_engine::{Engine, Scenario};
use v2xw_msg::sec_types::Certificate;
use v2xw_sec::chain::{ChainPolicy, validate};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn load(deployment: Option<serde_json::Value>) -> Scenario {
    // Four signalised intersections with roadside units (a procedural grid: no map file),
    // on an SCMS backend.
    let mut s = Scenario::load(
        root()
            .join("scenarios")
            .join("connected-intersections.yaml"),
    )
    .expect("loads");
    s.time.duration_s = 2.0;
    s.threats.attackers.clear();
    s.security.protocol = Some(
        serde_json::from_value(serde_json::json!({"id": "protocol/scms/camp"})).expect("choice"),
    );
    s.security.scms = deployment.map(|d| {
        serde_json::from_value(serde_json::json!({"deployment": d})).expect("a deployment")
    });
    s
}

struct Trust {
    roots: Vec<Certificate>,
    cas: Vec<Certificate>,
    acas: Vec<Certificate>,
    warnings: Vec<String>,
}

fn trust(s: Scenario) -> Trust {
    let engine = Engine::build(s, "").expect("builds");
    let (roots, cas) = engine.phase2().expect("an SCMS run").trust_bundle();
    Trust {
        roots: roots.iter().map(|c| (**c).clone()).collect(),
        cas: cas.iter().map(|c| (**c).clone()).collect(),
        acas: engine
            .credential_issuers()
            .iter()
            .map(|c| (**c).clone())
            .collect(),
        warnings: engine.manifest().warnings.clone(),
    }
}

fn id8(c: &Certificate) -> [u8; 8] {
    v2xw_sec::hashedid::id8_bytes(&v2xw_sec::cert::cert_id8(c).expect("hashes"))
}

/// The path from `leaf` to a self-signed certificate through `pool`, by issuer digest.
fn path(leaf: &Certificate, pool: &[Certificate]) -> Vec<Certificate> {
    use v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier;
    let mut out = vec![leaf.clone()];
    loop {
        let d = match &out.last().unwrap().0.issuer {
            IssuerIdentifier::sha256AndDigest(d) | IssuerIdentifier::sha384AndDigest(d) => {
                let mut b = [0u8; 8];
                b.copy_from_slice(&d.0[..]);
                b
            }
            _ => return out,
        };
        match pool.iter().find(|c| id8(c) == d) {
            Some(c) if out.len() < 8 => out.push(c.clone()),
            _ => return out,
        }
    }
}

/// Validates provider `k`'s ACA as a node of the run would: its path through the chain
/// file, under the 1609.2 chain rules, to a root the node installed.
fn aca_verifies(t: &Trust, k: usize) -> bool {
    let anchors: BTreeSet<[u8; 32]> = t
        .roots
        .iter()
        .map(|r| v2xw_sec::cert::hashed_id32(r).expect("hashes"))
        .collect();
    let mut pool = t.cas.clone();
    pool.extend(t.roots.iter().cloned());
    let chain = path(&t.acas[k], &pool);
    let start = t.acas[k].0.to_be_signed.validity_period.start.0.0;
    validate(&chain, &ChainPolicy::us_device(anchors, start + 3_600)).is_ok()
}

/// The default deployment is the US system: two providers, each with its own ACA whose
/// path every node validates to that provider's root on the CTL, and the manifest says
/// which assumptions the topology rests on. With provider-b's root off the CTL, every
/// node still trusts provider-a and no longer reaches provider-b's ACA.
#[test]
fn a_mixed_fleet_trusts_both_providers_and_only_through_the_ctl() {
    let both = trust(load(None));
    assert_eq!(both.acas.len(), 2, "one ACA per provider");
    assert_ne!(id8(&both.acas[0]), id8(&both.acas[1]));
    assert_eq!(both.roots.len(), 2, "both roots installed from the CTL");
    assert!(aca_verifies(&both, 0), "provider-a's ACA");
    assert!(aca_verifies(&both, 1), "provider-b's ACA, across providers");
    assert!(
        both.warnings
            .iter()
            .any(|w| w.starts_with("SCMS deployment us-production-2025")),
        "{:?}",
        both.warnings
    );
    assert!(
        both.warnings
            .iter()
            .any(|w| w.contains("ASSUMPTION: provider-b runs in region"))
    );

    let rogue = trust(load(Some(serde_json::json!({
        "providers": [{"name": "provider-b", "on_ctl": false}]
    }))));
    assert_eq!(rogue.acas.len(), 2, "the rogue provider still issues");
    assert_eq!(rogue.roots.len(), 1);
    assert!(aca_verifies(&rogue, 0));
    assert!(
        !aca_verifies(&rogue, 1),
        "provider-b is untrusted off the CTL"
    );
    assert!(rogue.warnings.iter().any(|w| w.contains("NOT on the CTL")));

    let camp = trust(load(Some(serde_json::json!({"preset": "camp-reference"}))));
    assert_eq!(
        (camp.acas.len(), camp.roots.len()),
        (1, 1),
        "today's single SCMS"
    );
}

/// `security.scms` is refused where it would do nothing, and a refused butterfly or an
/// unknown provider is refused at load.
#[test]
fn the_deployment_settings_are_refused_where_they_cannot_apply() {
    let refused = |d: serde_json::Value| {
        let s = load(Some(d));
        Engine::build(s, "").err().map(|e| e.to_string())
    };
    let e = refused(serde_json::json!({"options": {"butterfly": "unified"}})).expect("refused");
    assert!(e.contains("butterfly"), "{e}");
    let e = refused(serde_json::json!({"providers": [{"name": "provider-z"}]})).expect("refused");
    assert!(e.contains("provider-z"), "{e}");
    let e = refused(serde_json::json!({"providers": [{"name": "provider-b", "fleet_share": 0.9}]}))
        .expect("shares that do not sum to 1");
    assert!(e.contains("sum"), "{e}");
    let mut s = load(Some(serde_json::json!({"preset": "camp-reference"})));
    s.security.protocol = None;
    s.actors.backend.protocol = None;
    let e = Engine::build(s, "")
        .expect_err("no SCMS: refused")
        .to_string();
    assert!(e.contains("security.scms"), "{e}");
}

/// every provider's CRL Generator publishes at `t0`, an
/// empty list when nothing stands on it, and renews it a week later, as a CRACA of a
/// deployment that has run for years does. Before, a Generator published nothing until
/// its first revocation, so the two-week overdue rule could never act on a CRACA that
/// had not revoked in the run.
#[test]
fn every_provider_publishes_its_crl_at_the_start() {
    let mut s = load(None);
    s.time.duration_s = 10.0;
    let mut engine = Engine::build(s, "").expect("builds");
    let runs = engine.phase2().expect("an SCMS run").scms_runs();
    assert_eq!(runs.len(), 2, "two providers");
    for (k, r) in runs.iter().enumerate() {
        let st = &r.state;
        assert!(st.crlg.entries.is_empty(), "provider {k}: nothing stands");
        let first = st
            .crlg
            .issued
            .iter()
            .find(|c| c.series == 1)
            .unwrap_or_else(|| panic!("provider {k} published no CRL at t0"));
        assert_eq!((first.at, first.entries, first.delta), (0, 0, false));
        assert_eq!(first.next_crl - first.issue_date, 7 * 86_400);
        assert!(
            st.crl_store.secured.as_ref().map(|p| p.bytes()) == Some(first.pdu.bytes()),
            "provider {k}: the store serves it from t0"
        );
    }
    let report = engine
        .run(&mut v2xw_engine::DigestRecorder::new())
        .expect("runs");
    // Every device joins holding both providers' lists, verified by its own trust (it
    // was on the road before the run), so the overdue rule has a list to act on.
    let devices = engine.phase2().unwrap().device_nodes();
    assert!(!devices.is_empty(), "devices joined");
    for d in devices {
        assert_eq!(engine.node_crls_held(d), 2, "{d:?} holds both t0 lists");
    }
    assert_eq!(report.phase2.crl_frames_rejected, 0);
}

/// with provider-b off the CTL, nothing it
/// certifies verifies at provider-a's receivers. Its roadside units' CRL frames (and the
/// reports relayed over the air) are sized, not encoded, and verified on the engine's
/// word alone: 7, then 280, provider-b messages verified at provider-a in the realism
/// runs. The engine now attaches the signer's certificate to every sized frame and each
/// receiver holds it to its own installed roots: here, no CRL frame is accepted
/// unchecked. The refusal under a dropped root is proven at the node
/// (`v2xw-node` tests/rx_validation.rs, the sized-frame tests).
#[test]
fn nothing_a_provider_off_the_ctl_certifies_verifies() {
    let mut s = load(Some(serde_json::json!({
        "providers": [{"name": "provider-b", "on_ctl": false}]
    })));
    s.time.duration_s = 30.0;
    // A standing list from t0 and every unit broadcasting it, so units of both providers
    // put sized CRL frames on the air from the start.
    s.security.protocol = Some(
        serde_json::from_value(serde_json::json!({
            "id": "protocol/scms/camp",
            "params": {"crl_background_entries": 3}
        }))
        .expect("choice"),
    );
    for r in &mut s.actors.rsus {
        r.roles.push("crl".into());
    }
    let mut engine = Engine::build(s, "").expect("builds");
    let report = engine
        .run(&mut v2xw_engine::DigestRecorder::new())
        .expect("runs");
    let p = &report.phase2;
    let get = |k: &str| p.verified_by_provider.get(k).copied().unwrap_or(0);
    assert!(p.crl_broadcasts > 0, "the units broadcast the list");
    // Every sized CRL frame a vehicle took off the air carried its signer's certificate
    // and was held to the vehicle's roots: none was accepted on the engine's word.
    assert_eq!(
        report.receiver_validation.chain_unchecked, 0,
        "{:?}",
        report.receiver_validation
    );
    assert!(
        get("provider-a<-provider-a") > 0,
        "{:?}",
        p.verified_by_provider
    );
    assert_eq!(
        get("provider-a<-provider-b"),
        0,
        "{:?}",
        p.verified_by_provider
    );
}

/// OF-3: a device joins holding every provider's `t0` CRL,
/// and the records now show it. Each `node.security` row lists the signed CRLs the node's
/// own receiver holds by (crlCraca, crlSeries, nextCrl), and `pki.flow` carries the
/// pre-run fetch, one `SecuredCrl` PDU from the CRL Store per list per device, marked
/// `preload` like its pre-run enrolment. Faults: emit `node.security` without the node's
/// lists (`Engine::emit_node_security`), or skip `ScmsRun::note_prerun_crl`, and this
/// fails.
#[test]
fn a_device_records_the_t0_lists_it_holds_and_their_fetch() {
    let mut s = load(None);
    s.time.duration_s = 10.0;
    let mut engine = Engine::build(s, "").expect("builds");
    // Each provider's t0 list: its CRACA and nextCrl, out of the signed CRL itself.
    let lists: BTreeSet<(String, u64)> = engine
        .phase2()
        .expect("an SCMS run")
        .scms_runs()
        .iter()
        .map(|r| {
            let first = r
                .state
                .crlg
                .issued
                .iter()
                .find(|c| c.series == 1)
                .expect("a t0 CRL");
            let opened = v2xw_sec::crl::decode_crl(first.pdu.bytes()).expect("decodes");
            (
                v2xw_core::hash::hex_encode(&opened.contents.crl_craca.0[..]),
                u64::from(first.next_crl),
            )
        })
        .collect();
    assert_eq!(lists.len(), 2, "two providers, two CRACAs");
    let mut rec = v2xw_engine::MemoryRecorder::new();
    engine.run(&mut rec).expect("runs");
    let rows = |channel: &str| -> Vec<serde_json::Value> {
        rec.records()
            .iter()
            .filter(|(_, r)| r.channel == channel)
            .map(|(_, r)| serde_json::from_slice(&r.json).expect("json"))
            .collect()
    };
    let security = rows("node.security");
    assert!(!security.is_empty(), "no node.security rows");
    let mut missing = Vec::new();
    for row in &security {
        let held: BTreeSet<(String, u64)> = row["crls"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| c["series"] == 1)
            .map(|c| {
                (
                    c["craca"].as_str().unwrap_or_default().to_string(),
                    c["next_crl"].as_u64().unwrap_or(0),
                )
            })
            .collect();
        if !lists.is_subset(&held) {
            missing.push(format!("node {} at {}: {:?}", row["node"], row["t"], held));
        }
    }
    assert!(
        missing.is_empty(),
        "{} of {} rows do not show both t0 lists {:?}: {:?}",
        missing.len(),
        security.len(),
        lists,
        &missing[..missing.len().min(3)]
    );
    let fetched = rows("pki.flow")
        .iter()
        .filter(|v| v["kind"] == "pdu" && v["flow"] == "preload" && v["name"] == "SecuredCrl")
        .count();
    let devices = engine.phase2().expect("phase 2").device_nodes().len();
    assert!(devices > 0, "devices joined");
    assert_eq!(
        fetched,
        2 * devices,
        "pre-run CRL fetches on pki.flow: one per list per device"
    );
}

/// [SM-MBR] §2.3: under the US system an empty `detection.local` runs the
/// US profile, because a US OBU detects and files the PSID-0x20 subtypes. The default
/// deployment (`us-production-2025`) names `detect/us-sm-mbr` and arms its receivers; the
/// CAMP reference design keeps the empty list's meaning (no local detector), and an
/// explicit list is kept as written. Fault: drop the `is_us()` condition from
/// `phase2::us_default_detector` -> the CAMP reference also names SM-MBR and the test
/// goes red.
#[test]
fn a_us_deployment_defaults_to_the_sm_mbr_profile() {
    let suite = |s: Scenario| {
        assert!(s.detection.local.is_empty() || s.detection.local.len() == 1);
        let engine = Engine::build(s, "").expect("builds");
        let p2 = engine.phase2().expect("an SCMS run");
        (p2.report().local_suite, p2.detection_on())
    };
    let us = load(Some(serde_json::json!({"preset": "us-production-2025"})));
    assert!(us.detection.local.is_empty());
    assert_eq!(
        suite(us),
        (v2xw_engine::phase2::US_SM_MBR.to_string(), true),
        "the US deployment's default local detector"
    );
    assert_eq!(
        suite(load(None)).0,
        v2xw_engine::phase2::US_SM_MBR,
        "no deployment block is the US deployment"
    );
    assert_eq!(
        suite(load(Some(serde_json::json!({"preset": "camp-reference"})))),
        (String::new(), false),
        "the CAMP reference keeps the empty list: no local detector"
    );
    let mut named = load(Some(serde_json::json!({"preset": "us-production-2025"})));
    named.detection.local = vec![v2xw_engine::scenario::ModelChoice::new(
        v2xw_engine::phase2::LEGACY_12,
    )];
    assert_eq!(
        suite(named),
        (v2xw_engine::phase2::LEGACY_12.to_string(), true),
        "an explicit list is kept"
    );
}

/// The US default suite with its BrakeAcceleration check ([SM-MBR] §2.3.1.3.4): a US device files BrakeAcceleration
/// (Implausible, subtype 3) and the report reaches the MA. A small grid fleet on the
/// compressed calendar under the default deployment and an empty `detection.local` (so
/// the US default runs); every BSM a detector judges reads as brakes on at +0.5 m/s²
/// (`forge_brake_acceleration_for_test`: no honest vehicle sends one). Fault: remove the
/// `BrakeAcceleration` arm of `phase2::us_subtype` -> the reports are unmapped and not
/// filed, none reaches the MA, and the test goes red.
#[test]
fn a_us_device_files_brake_acceleration() {
    use serde_json::json;
    use v2xw_engine::NullRecorder;
    use v2xw_engine::scenario::ModelChoice;
    let mut s = Scenario::load(root().join("scenarios/phase1-grid.yaml")).expect("loads");
    s.time.duration_s = 40.0;
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
            "crl_fetch_interval_s": 30,
            "ma_review_s": 30
        }),
    });
    s.time.calendar = v2xw_engine::scenario::Calendar::Compressed;
    s.net.uu = Some(ModelChoice {
        id: "cellular/uu/fixed-latency".to_string(),
        params: json!({"preset": "4g-east-coast"}),
    });
    s.threats.attackers.clear();
    s.detection.local.clear();
    let mut engine = Engine::build(s, "").expect("builds");
    let p2 = engine.phase2_mut().expect("an SCMS run");
    assert_eq!(p2.report().local_suite, v2xw_engine::phase2::US_SM_MBR);
    p2.forge_brake_acceleration_for_test();
    let r = engine.run(&mut NullRecorder::new()).expect("runs");
    let p = &r.phase2;
    let brake: usize = engine
        .phase2()
        .expect("an SCMS run")
        .scms_runs()
        .iter()
        .flat_map(|run| run.state.ma.opened_reports.iter())
        .filter(|o| {
            o.report
                .subtypes
                .contains(&v2xw_proto::scms::mbr::Subtype::BrakeAcceleration)
        })
        .count();
    println!(
        "sent {} real {} unmapped {} not filed {:?} at MA {} brake-acceleration opened {brake}",
        p.reports_sent,
        p.reports_mbr_real,
        p.reports_mbr_unmapped,
        p.reports_not_filed_by,
        p.reports_at_ma
    );
    assert!(
        brake > 0,
        "no SM-MBR with subtype 3 (BrakeAcceleration) reached an MA"
    );
}
