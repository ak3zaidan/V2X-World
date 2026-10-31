//! A misbehaviour report is signed with a pseudonym certificate valid when it is signed,
//! or it waits in the device's report store until one is (protocol/flow gaps package,
//! item 2; [SM-MBR] §2.2 and §3).
//!
//! The conformance run's first live window crosses the 614 → 615 week rollover at 600 s.
//! On main, 30 reports in it went out unsigned with the label "no pseudonym certificate
//! valid at the report's time". Measured here, all 30 were filed by roadside units (31 RSU
//! reports, R-UNSEC): an RSU holds application certificates only, so it has nothing to
//! sign an SM-MBR with. The vehicle-side rollover fix (edge 1b/1c) had already removed
//! the vehicle cases, and no vehicle report falls into the gap in this window
//! (`reports_held_for_signer` 0). The vehicle hold (`Phase2::report_waits_for_signer`)
//! is the defensive rule for a vehicle whose pool has run dry. Since R-UNSEC a unit's own
//! detections are not filed at all (the US profile has no roadside reporter): the first
//! test checks they are counted under their own reason and never leave sized-only.
//!
//! The same window checks the conditional CRL poll (item 3): every due poll is a request,
//! answered 412 when the copy is current ([SMP] §4.3.2). Fault: skip a poll that would
//! find nothing new (the old behaviour) and no 412 is ever counted.

use std::path::Path;
use std::sync::OnceLock;

use v2xw_engine::{Engine, NullRecorder, RunReport, Scenario};

const VEHICLE_GAP: &str = "key: no pseudonym certificate valid at the report's time";
const RSU: &str = "a roadside unit holds no pseudonym certificate to sign an SM-MBR with";

fn first_window() -> &'static RunReport {
    static RUN: OnceLock<RunReport> = OnceLock::new();
    RUN.get_or_init(run_first_window)
}

fn run_first_window() -> RunReport {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = Scenario::load(root.join("scenarios/scms-conformance.yaml")).expect("loads");
    s.time.duration_s = 660.0;
    s.time.fast_forward.clear();
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = NullRecorder::new();
    engine.run(&mut rec).expect("runs")
}

#[test]
fn no_vehicle_report_goes_out_unsigned_across_the_week_rollover() {
    let r = first_window();
    let p = &r.phase2;
    println!(
        "unsecured {:?}; held for a signer {}; real {}; from RSUs {}",
        p.reports_mbr_unsecured_by,
        p.reports_held_for_signer,
        p.reports_mbr_real,
        p.reports_from_rsus
    );
    assert_eq!(
        p.reports_mbr_unsecured_by
            .get(VEHICLE_GAP)
            .copied()
            .unwrap_or(0),
        0,
        "a vehicle report went out with no valid pseudonym: {:?}",
        p.reports_mbr_unsecured_by
    );
    assert!(p.reports_mbr_real > 0, "real SM-MBRs still flow");
    // R-UNSEC: the US profile has no roadside reporter ([SM-MBR] §3 and §2.3.1.4.1; USDOT
    // RSU Specification v4.1), so a unit's own detections are not filed: counted under
    // their own reason, never sent sized-only on the backhaul, never a vehicle's gap.
    // Fault: put back the roadside exemption in `Phase2::files_report`, and the units'
    // reports leave on the backhaul with no bytes again.
    assert_eq!(
        p.reports_from_rsus, 0,
        "a roadside unit filed {} sized-only reports",
        p.reports_from_rsus
    );
    assert!(
        p.reports_not_filed_by.get(RSU).copied().unwrap_or(0) > 0,
        "no roadside detection was counted as not filed: {:?}",
        p.reports_not_filed_by
    );
    assert_eq!(
        p.reports_mbr_unsecured_by.get(RSU).copied().unwrap_or(0),
        0,
        "{:?}",
        p.reports_mbr_unsecured_by
    );
}

#[test]
fn every_due_crl_poll_asks_and_a_current_copy_is_answered_412() {
    let p = &first_window().phase2;
    println!(
        "CRL polls answered 412: {}; downloads {}",
        p.crl_polls_not_modified, p.crl_downloads
    );
    // No new list reaches the fleet in the first 660 s (main: 0 downloads), so the
    // polls find the copy current.
    assert!(
        p.crl_polls_not_modified > 0,
        "the fleet polled hourly with random phases, so some poll fell in the window"
    );
}

/// The NYC Manhattan scenario with every third roadside unit (as `tests/phase2.rs`): a
/// five-minute run on the compressed calendar that reaches a revocation.
fn manhattan() -> Scenario {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = Scenario::load(root.join("scenarios/phase2-manhattan.yaml")).expect("loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root.join(&*path).to_string_lossy().into_owned();
    }
    s.actors.rsus = s
        .actors
        .rsus
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % 3 == 0)
        .map(|(_, r)| r)
        .collect();
    s
}

type Blocklists = Vec<std::collections::BTreeSet<v2xw_core::ids::NodeId>>;

/// One Manhattan run: its report, each provider's RA blocklist, and per provider the
/// MA's subject sources (rows read from the certificate, from a shared report, from the
/// engine's map).
fn decided(forget_map: bool) -> (RunReport, Blocklists, Vec<[usize; 3]>) {
    let mut engine = Engine::build(manhattan(), "").expect("builds");
    if forget_map {
        engine
            .phase2_mut()
            .expect("an SCMS run")
            .forget_digest_map_for_test();
    }
    let report = engine.run(&mut NullRecorder::new()).expect("runs");
    let p2 = engine.phase2().expect("an SCMS run");
    let blocklists = p2
        .scms_runs()
        .iter()
        .map(|r| r.state.ra.blocklist.clone())
        .collect();
    let sources = p2
        .scms_runs()
        .iter()
        .map(|r| {
            use v2xw_proto::scms::run::SubjectFrom as F;
            let rows = &r.state.ma.report_subjects;
            let n = |f: F| rows.iter().filter(|s| s.from == f).count();
            [n(F::SuspectCertificate), n(F::Shared), n(F::EngineMap)]
        })
        .collect();
    (report, blocklists, sources)
}

/// SC-P1 ([SM-MBR] §3; [CAMP122] §5): a report is decided without the engine's map.
/// The MA reads the accused certificate from the report it opened and the issuing ACA
/// resolves it from its own table, so with the engine's digest-to-device map gone
/// (`Phase2::forget_digest_map_for_test`) the MA opens the same cases and the RA
/// blocklists the same devices, the attacker among them. Every report the MA received
/// on this real-report run names its subject from the certificate
/// (live-scms `LS.MA.subject-from-cert`). Fault: restore the early return on an
/// unmapped digest in `Phase2::report_at_proxy` -> no report reaches the MA,
/// `cases_opened` is 0 and the test goes red.
#[test]
fn a_report_is_decided_without_the_engine_map() {
    let (base, base_blocked, _) = decided(false);
    let (blind, blind_blocked, sources) = decided(true);
    let (b, h) = (&base.phase2, &blind.phase2);
    println!(
        "cases {} / {}; decisions {} / {}; linked {} / {}; blocked {:?} / {:?}; sources {:?}",
        b.cases_opened,
        h.cases_opened,
        b.ma_revoke_decisions,
        h.ma_revoke_decisions,
        b.decisions_linked,
        h.decisions_linked,
        base_blocked,
        blind_blocked,
        sources
    );
    assert!(b.cases_opened > 0, "the baseline opened no case");
    assert!(b.revoked_attackers >= 1, "the baseline revoked no attacker");
    assert_eq!(
        h.cases_opened, b.cases_opened,
        "the MA opens the same cases"
    );
    assert!(
        blind_blocked.iter().any(|s| !s.is_empty()),
        "nothing was blocklisted without the map"
    );
    assert_eq!(
        blind_blocked, base_blocked,
        "the same devices are blocklisted"
    );
    assert!(sources.iter().any(|s| s[0] > 0), "no MA read a certificate");
    for (k, s) in sources.iter().enumerate() {
        assert_eq!(
            s[2], 0,
            "provider {k}: a real report's subject came from the engine's map"
        );
    }
}

/// A small grid fleet on the compressed calendar with one `ConstPos` attacker among ten
/// vehicles, a 20 s i-period (so the attacker moves to a new pseudonym inside the run)
/// and a 30 s operator review, so a decision about its next pseudonym is taken while the
/// first case is still in review and its entry not yet on the list.
fn one_attacker_changes_pseudonym() -> Scenario {
    use serde_json::json;
    use v2xw_engine::scenario::ModelChoice;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = Scenario::load(root.join("scenarios/phase1-grid.yaml")).expect("loads");
    s.time.duration_s = 90.0;
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
    s.threats.attackers = serde_json::from_value(json!([
        {"id": "threat/attacker/legacy/ConstPos", "count": 1}
    ]))
    .expect("attackers");
    s.detection = serde_json::from_value(json!({
        "local": [{"id": "detect/legacy-12"}],
        "ma": {"id": "threat/ma/corroborated"}
    }))
    .expect("detection");
    s
}

/// One run of [`one_attacker_changes_pseudonym`]: the report, the CRL entries every
/// provider's Generator holds, and the `linked` cases across the MAs.
fn linked_run(forget_map: bool) -> (RunReport, usize, usize) {
    let mut engine = Engine::build(one_attacker_changes_pseudonym(), "").expect("builds");
    if forget_map {
        engine
            .phase2_mut()
            .expect("an SCMS run")
            .forget_digest_map_for_test();
    }
    let report = engine.run(&mut NullRecorder::new()).expect("runs");
    let p2 = engine.phase2().expect("an SCMS run");
    let entries = p2
        .scms_runs()
        .iter()
        .map(|r| r.state.crlg.entries.len())
        .sum();
    let linked = p2
        .scms_runs()
        .iter()
        .flat_map(|r| r.state.ma.cases.values())
        .filter(|c| c.outcome() == Some(v2xw_proto::scms::run::CaseOutcome::Linked))
        .count();
    (report, entries, linked)
}

/// SC-P3 ([CAMP122] §5; Whyte et al., VNC 2013, §V): the MA de-duplicates its decisions
/// by the linkage chain both LAs name, not by the engine's device. The attacker is
/// decided under one pseudonym, then under the next while the first case is in review;
/// the second case runs its PCA lookup and both LA calls, finds the chain pair already
/// decided and ends `linked`: no second blocklist and no second entry (a decision after
/// the entry is on the list is `decisions_already_covered`, as before). With the engine's
/// digest map gone (SC-P1's hook) nothing changes, because the MA never read it. Fault:
/// drop the `decided_chains` check in `run/revoke.rs` -> the second case revokes again
/// (two entries, no linked case) and the test goes red.
#[test]
fn a_second_pseudonym_of_a_decided_device_links_without_a_second_revocation() {
    let (r, entries, linked) = linked_run(false);
    let p = &r.phase2;
    println!(
        "decisions {} linked {} covered {} cases {} unresolved {} entries {} linked cases {} \
         attackers revoked {} honest revoked {} repeated {}",
        p.ma_revoke_decisions,
        p.decisions_linked,
        p.decisions_already_covered,
        p.cases_opened,
        p.cases_unresolved,
        entries,
        linked,
        p.revoked_attackers,
        p.revoked_honest,
        p.revocations_repeated
    );
    // Every decision is accounted for once: the first revokes, the one taken while it is
    // in review is linked by the chain, and any taken after its entry is on the
    // Generator's list is covered by the list (`start_cases_of`; SC-P4 moves that check
    // to the published list). Measured on 476ab68d+: 4 = 1 + 1 linked + 2 covered.
    assert!(p.ma_revoke_decisions >= 2, "a second pseudonym was decided");
    assert_eq!(p.decisions_linked, 1, "the second is linked to the first");
    assert_eq!(
        p.ma_revoke_decisions,
        1 + p.decisions_linked + p.decisions_already_covered,
        "one revoking decision; the rest linked or covered"
    );
    assert_eq!(p.cases_opened, 2, "the linked decision cost a case of its own");
    assert_eq!(entries, 1, "one CRL entry for the device");
    assert_eq!(linked, 1, "one case ends `linked`");
    assert_eq!(p.revoked_attackers, 1);
    assert_eq!(p.revocations_repeated, 0);
    assert_eq!(p.cases_unresolved, 0, "a linked case is not unresolved");

    let (h, h_entries, h_linked) = linked_run(true);
    let q = &h.phase2;
    assert_eq!(
        (
            q.ma_revoke_decisions,
            q.decisions_linked,
            h_entries,
            h_linked
        ),
        (p.ma_revoke_decisions, p.decisions_linked, entries, linked),
        "the engine's map plays no part in the linking"
    );
}
