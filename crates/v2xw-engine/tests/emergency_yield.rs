//! `traffic.emergency_yield` reaches the run.
//!
//! - The expectation row of `tests/expectations/traffic.yaml` holds on its family
//!   (`scenarios/sensitivity/catalogue.yaml`, `emergency`: the procedural base of
//!   `emergency-priority.yaml`, one vehicle in ten an emergency vehicle, 120 s): with
//!   yielding on, the emergency vehicles drive further in the same window
//!   (`report.emergency_distance_m`).
//! - Naming the block with yielding off changes nothing but that report leaf.
//! - Under the US SCMS an emergency vehicle signs its signal request (SRM) under
//!   its OBU identification certificate and the junction grants it; an ordinary vehicle's
//!   SRM, signed under a pseudonym, is refused (`psid-not-permitted`); and once the MA has
//!   revoked the identification certificate and the unit has installed the series-3
//!   list, the vehicle's requests are no longer served.
//!
//! The corridor measurement (an ambulance through a congested 1 km arterial, three seeds)
//! is `crates/v2xw-mobility/tests/emergency_yield.rs`.

use std::path::{Path, PathBuf};

use v2xw_engine::Scenario;
use v2xw_engine::scenario::schema::EmergencyYieldSettings;
use v2xw_engine::sensitivity::{expect, sweep};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
#[ignore = "slow: whole runs; CI runs it in the slow-test job (tools/ci/slow-tests.sh)"]
fn the_emergency_yield_row_holds() {
    let path = root().join("scenarios/sensitivity/catalogue.yaml");
    let fam = sweep::Families::load(&path).expect("families file loads");
    let mut exp = sweep::load_expectations(&[root().join("crates/v2xw-engine/tests/expectations")])
        .expect("rows parse");
    exp.retain(|e| e.setting.starts_with("traffic.emergency_yield"));
    assert_eq!(exp.len(), 1, "the distance row");
    let (settings, unplanned) = expect::expect_plan(&fam, &exp);
    assert!(unplanned.is_empty(), "{unplanned:?}");
    let bases = fam.bases(&path).expect("bases load");
    let result =
        sweep::run_cached(settings, vec![], &bases, &exp, 1, None, |_, _, _| {}).expect("runs");
    for v in &result.verdicts {
        eprintln!(
            "{} {}: {:?} -> {:?} ({})",
            v.expectation.setting, v.expectation.metric, v.base, v.value, v.note
        );
    }
    assert_eq!(result.verdicts.len(), 1);
    let failed: Vec<&sweep::Verdict> = result.verdicts.iter().filter(|v| !v.pass).collect();
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
#[ignore = "slow: whole runs; CI runs it in the slow-test job (tools/ci/slow-tests.sh)"]
fn naming_the_block_off_changes_nothing_but_the_report_leaf() {
    let mut s =
        Scenario::load(root().join("scenarios/connected-intersections.yaml")).expect("loads");
    s.time.duration_s = 60.0;
    let mut named = s.clone();
    named
        .traffic
        .get_or_insert_with(Default::default)
        .emergency_yield = Some(EmergencyYieldSettings {
        enabled: Some(false),
        ..EmergencyYieldSettings::default()
    });
    let run = |sc: Scenario| {
        let mut engine = v2xw_engine::Engine::build(sc, "").expect("builds");
        let mut rec = v2xw_engine::DigestRecorder::new();
        let report = engine.run(&mut rec).expect("runs");
        (report, rec.digest_hex(), rec.frame_digest_hex())
    };
    let (plain, plain_digest, plain_frames) = run(s);
    let (mut with, with_digest, with_frames) = run(named);
    assert_eq!(plain.emergency_distance_m, 0, "reported without the block");
    assert!(
        with.emergency_distance_m > 0,
        "the emergency vehicles drove nowhere"
    );
    assert_eq!(
        plain_digest, with_digest,
        "naming the block off moved the records"
    );
    assert_eq!(
        plain_frames, with_frames,
        "naming the block off moved the frames"
    );
    with.emergency_distance_m = 0;
    assert_eq!(plain, with, "naming the block off moved the report");
}

/// `connected-intersections.yaml` (four SPaT/MAP units on the middle avenue,
/// SRM and SSM on) under the US SCMS on the compressed calendar (a CRL publication and a
/// unit CRL poll every 10 s), half the fleet emergency vehicles, 24 vehicles that all
/// join at the start, every SPDU verified.
fn priority_scms(duration_s: f64) -> Scenario {
    use serde_json::json;
    use v2xw_engine::scenario::ModelChoice;
    let mut s = Scenario::load(root().join("scenarios/connected-intersections.yaml"))
        .expect("the shipped scenario loads");
    s.time.duration_s = duration_s;
    s.metrics = vec![];
    s.world.buildings.enabled = false;
    for class in s.actors.vehicles.classes.values_mut() {
        class.fraction = 0.5;
    }
    s.actors.vehicles.demand.rate_veh_per_h = Some(3_600_000.0);
    s.actors.vehicles.demand.params = json!({ "max_total_vehicles": 24 });
    s.security.verification_policy = "verify-all".to_string();
    let id = v2xw_engine::phase2::CAMP_SCMS.to_string();
    s.actors.backend.protocol = Some(id.clone());
    s.security.protocol = Some(ModelChoice {
        id,
        params: json!({
            "i_period_s": 20,
            "cert_lifetime_s": 21,
            "certs_per_period": 3,
            "pool_periods": 3,
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
    let errors: Vec<String> = v2xw_engine::scenario::validate(&s)
        .into_iter()
        .map(|e| e.to_string())
        .collect();
    assert_eq!(errors, Vec::<String>::new(), "the scenario loads");
    s
}

/// Acceptance test 3. The emergency vehicles' SRMs, signed under their OBU
/// identification certificates, reach a junction's controller; every ordinary vehicle's
/// unit is made to send SRMs too (`Engine::forge_srm_for_test`), signed under the
/// pseudonyms that are all the SCMS gave it, and every one is refused
/// `psid-not-permitted`: no controller hears a requester that is not an identification
/// certificate.
///
/// Fault: keep SRM in the pseudonym PSIDs (`phase2.rs`, `Phase2::new`'s identification-certificate
/// filter).
/// The forged SRMs verify, their pseudonyms are heard as requesters, and the test goes red.
#[test]
fn a_forged_srm_from_a_normal_vehicle_is_refused() {
    let mut engine = v2xw_engine::Engine::build(priority_scms(40.0), "").expect("builds");
    engine.forge_srm_for_test();
    let mut rec = v2xw_engine::NullRecorder::new();
    let r = engine.run(&mut rec).expect("runs");
    let p = &r.phase2;
    let signers = engine
        .phase2()
        .expect("the SCMS runs")
        .identification_signers();
    let heard = engine.priority_requests_heard();
    let forged: Vec<_> = heard.keys().filter(|d| !signers.contains_key(*d)).collect();
    let granted: Vec<_> = heard.keys().filter(|d| signers.contains_key(*d)).collect();
    let refused = |k: &str| r.receiver_validation.refused.get(k).copied().unwrap_or(0);
    println!(
        "ident preloaded {} renewed {} refused {} unsigned {} signers {} heard {} granted {} \
         forged {} psid-not-permitted {} errors {} {}",
        p.ident_certs_preloaded,
        p.ident_certs_renewed,
        p.ident_certs_refused,
        p.srm_unsigned_no_ident,
        signers.len(),
        heard.len(),
        granted.len(),
        forged.len(),
        refused("psid-not-permitted"),
        p.backend_errors,
        p.first_backend_error
    );
    assert!(
        p.ident_certs_preloaded > 0,
        "no emergency vehicle was issued one"
    );
    assert_eq!(p.ident_certs_refused, 0, "a unit refused its certificate");
    assert_eq!(p.backend_errors, 0, "{}", p.first_backend_error);
    assert!(
        !granted.is_empty(),
        "no emergency vehicle's SRM reached a controller"
    );
    assert!(
        forged.is_empty(),
        "{} requesters signed under a pseudonym were served",
        forged.len()
    );
    assert!(
        refused("psid-not-permitted") > 0,
        "no forged SRM reached a unit, so nothing was tested"
    );
}

/// Acceptance test 4. At 15 s the MA revokes every emergency vehicle's identification
/// certificate (`Phase2::revoke_identification`). Before, their SRMs are served; the
/// series-3 hash CRL is published at the next 10 s cadence and the units install it with
/// their composite CRL, after which each SRM signed under a revoked certificate is
/// refused `revoked-by-hash` and no controller hears that requester again.
///
/// Fault: leave series 3 out of the composite CRL (`phase2.rs`, the CRL feed's and the
/// roadside path's hash CRLs). No unit installs the list, the vehicles keep being served,
/// and the test goes red.
#[test]
#[ignore = "slow: whole runs; CI runs it in the slow-test job (tools/ci/slow-tests.sh)"]
fn a_revoked_identification_certificate_stops_the_grants() {
    let at_s = 15.0;
    let cutoff_s = 35.0;
    let mut engine = v2xw_engine::Engine::build(priority_scms(60.0), "").expect("builds");
    let mut rec = v2xw_engine::NullRecorder::new();
    let at = (at_s * 1e9) as u64;
    engine
        .advance(at - 1, &mut rec)
        .expect("runs to the decision");
    let p2 = engine.phase2_mut().expect("the SCMS runs");
    let revoked = p2.identification_signers();
    let holders: std::collections::BTreeSet<_> = revoked.values().copied().collect();
    for node in &holders {
        p2.revoke_identification(*node, at).expect("revocable");
    }
    let r = engine.run(&mut rec).expect("runs");
    let heard = engine.priority_requests_heard();
    let before = heard
        .iter()
        .filter(|(d, (_, first, _))| revoked.contains_key(*d) && *first < at)
        .count();
    let cutoff = (cutoff_s * 1e9) as u64;
    let late: Vec<_> = heard
        .iter()
        .filter(|(d, (_, _, last))| revoked.contains_key(*d) && *last > cutoff)
        .map(|(_, (_, _, last))| *last as f64 / 1e9)
        .collect();
    let by_hash = r
        .receiver_validation
        .refused
        .get("revoked-by-hash")
        .copied()
        .unwrap_or(0);
    println!(
        "holders {} revocations {} served before {} served after {cutoff_s} s: {late:?} \
         revoked-by-hash {by_hash} rsu crls {} errors {} {}",
        holders.len(),
        r.phase2.ident_revocations,
        before,
        r.phase2.rsu_crls_installed,
        r.phase2.backend_errors,
        r.phase2.first_backend_error
    );
    assert_eq!(
        r.receiver_validation.refused.get("psid-not-permitted"),
        None,
        "an honest message was refused for its PSID"
    );
    assert_eq!(r.phase2.ident_revocations, holders.len() as u64);
    assert!(
        before > 0,
        "no emergency vehicle was served before the decision"
    );
    assert!(
        late.is_empty(),
        "revoked requesters were still served after the CRL install: {late:?}"
    );
    assert!(
        by_hash > 0,
        "no unit refused a revoked identification certificate, so nothing was tested"
    );
}
