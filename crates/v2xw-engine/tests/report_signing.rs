//! A misbehaviour report is signed with a pseudonym certificate valid when it is signed,
//! or it waits in the device's report store until one is (protocol/flow gaps package,
//! item 2; [SM-MBR] §2.2 and §3).
//!
//! The conformance run's first live window crosses the 614 → 615 week rollover at 600 s.
//! On main, 30 reports in it went out unsigned with the label "no pseudonym certificate
//! valid at the report's time". Measured here, all 30 were filed by roadside units (31 RSU
//! reports): an RSU holds application certificates only, so it has nothing to
//! sign an SM-MBR with. The vehicle-side rollover fix (edge 1b/1c) had already removed
//! the vehicle cases, and no vehicle report falls into the gap in this window
//! (`reports_held_for_signer` 0). The vehicle hold (`Phase2::report_waits_for_signer`)
//! is the defensive rule for a vehicle whose pool has run dry. A unit's own
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
#[ignore = "slow: whole runs; CI runs it in the slow-test job (tools/ci/slow-tests.sh)"]
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
    // the US profile has no roadside reporter ([SM-MBR] §3 and §2.3.1.4.1; USDOT
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
#[ignore = "slow: whole runs; CI runs it in the slow-test job (tools/ci/slow-tests.sh)"]
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

/// The NYC Manhattan scenario with every third roadside unit (as `tests/report_to_revocation.rs`): a
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

/// [SM-MBR] §3; [CAMP122] §5: a report is decided without the engine's map.
/// The MA reads the accused certificate from the report it opened and the issuing ACA
/// resolves it from its own table, so with the engine's digest-to-device map gone
/// (`Phase2::forget_digest_map_for_test`) the MA opens the same cases and the RA
/// blocklists the same devices, the attacker among them. Every report the MA received
/// on this real-report run names its subject from the certificate. Fault: restore the early return on an
/// unmapped digest in `Phase2::report_at_proxy` -> no report reaches the MA,
/// `cases_opened` is 0 and the test goes red.
#[test]
#[cfg_attr(
    not(v2xw_map = "manhattan"),
    ignore = "needs worlds/cache/manhattan.osm.xml, which is gitignored"
)]
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

/// [CAMP122] §5; Whyte et al., VNC 2013, §V: the MA de-duplicates its decisions
/// by the linkage chain both LAs name, not by the engine's device. The attacker is
/// decided under one pseudonym, then under the next while the first case is in review;
/// the second case runs its PCA lookup and both LA calls, finds the chain pair already
/// decided and ends `linked`: no second blocklist and no second entry (a decision after
/// the entry is published is `decisions_already_covered`). With the engine's
/// digest map gone (`Phase2::forget_digest_map_for_test`) nothing changes, because the MA never read it. Fault:
/// drop the `decided_chains` check in `run/revoke.rs` -> the second case revokes again
/// (two entries, no linked case) and the test goes red.
#[test]
#[ignore = "slow: whole runs; CI runs it in the slow-test job (tools/ci/slow-tests.sh)"]
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
    // published list is covered by it (`start_cases_of`; the CRL Store's list). Under the
    // J2945/1 BSM schedule one more pseudonym is decided while the first is in review:
    // 4 = 1 + 2 linked + 1 covered, and every linked decision costs a case of its own.
    assert!(p.ma_revoke_decisions >= 2, "a second pseudonym was decided");
    assert!(p.decisions_linked >= 1, "the second is linked to the first");
    assert_eq!(
        p.ma_revoke_decisions,
        1 + p.decisions_linked + p.decisions_already_covered,
        "one revoking decision; the rest linked or covered"
    );
    assert_eq!(
        p.cases_opened,
        1 + p.decisions_linked,
        "each linked decision cost a case of its own"
    );
    assert_eq!(entries, 1, "one CRL entry for the device");
    assert_eq!(
        linked as u64, p.decisions_linked,
        "each linked decision's case ends `linked`"
    );
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

/// [`one_attacker_changes_pseudonym`] on the real calendar from `t0` (RFC 3339): the US
/// one-week i-period and the CRL Generator's "within 1 business day" rule, which publishes
/// at the next 10:00 ET business-day slot after a change ([SM-PROV] §3.4.9), with the
/// operator's review shortened to 30 s so the attacker is decided inside the run. Returns
/// the report, the entries every provider's Generator holds, the entries its CRL Store
/// has published, and the `linked` cases.
fn real_calendar_run(t0: &str, duration_s: f64) -> (RunReport, usize, usize, usize) {
    use serde_json::json;
    use v2xw_engine::scenario::ModelChoice;
    let mut s = one_attacker_changes_pseudonym();
    s.time.calendar = v2xw_engine::scenario::Calendar::Real;
    s.time.t0 = t0.to_string();
    s.time.duration_s = duration_s;
    // A new pseudonym every minute (the US week's certificates are concurrently valid),
    // so the attacker is decided again under the next one.
    s.security.pseudonym_change.period_s = Some(60.0);
    let id = v2xw_engine::phase2::CAMP_SCMS.to_string();
    s.security.protocol = Some(ModelChoice {
        id,
        params: json!({
            "report_shuffle_window_s": 5,
            "ma_review_s": 30
        }),
    });
    let mut engine = Engine::build(s, "").expect("builds");
    let report = engine.run(&mut NullRecorder::new()).expect("runs");
    let p2 = engine.phase2().expect("an SCMS run");
    let runs = p2.scms_runs();
    let entries = runs.iter().map(|r| r.state.crlg.entries.len()).sum();
    let published = runs.iter().map(|r| r.state.crl_store.entries.len()).sum();
    let linked = runs
        .iter()
        .flat_map(|r| r.state.ma.cases.values())
        .filter(|c| c.outcome() == Some(v2xw_proto::scms::run::CaseOutcome::Linked))
        .count();
    (report, entries, published, linked)
}

/// [BRECHT] §VI-D (the MA consults the CRL): before it opens a case the MA
/// checks the list its provider has **published** (the CRL Store's), not the CRL
/// Generator's working set. On the real calendar a decision is published at the next
/// business-day slot, so an entry stays pending for hours:
/// - from 04:00 ET (the slot is six hours off), every later decision about the attacker
///   finds nothing published: none is `decisions_already_covered`, and each opens a case
///   that the LAs' chain identifiers end `linked`;
/// - from 09:59 ET (the slot comes 60 s in, between the two decisions), the decision
///   taken after the list is published is covered by it, with no case.
///
/// Fault: build `covered` from `crlg.entries` -> decisions are counted covered before
/// publication and the test goes red.
#[test]
#[ignore = "slow: whole runs; CI runs it in the slow-test job (tools/ci/slow-tests.sh)"]
fn the_ma_counts_a_decision_covered_only_after_publication() {
    let show = |name: &str, r: &RunReport, entries, published, linked| {
        let p = &r.phase2;
        println!(
            "{name}: decisions {} linked {} covered {} cases {} unresolved {} entries {} \
             published {} linked cases {} revoked attackers {} repeated {}",
            p.ma_revoke_decisions,
            p.decisions_linked,
            p.decisions_already_covered,
            p.cases_opened,
            p.cases_unresolved,
            entries,
            published,
            linked,
            p.revoked_attackers,
            p.revocations_repeated
        );
    };
    // 2026-01-06 (a Tuesday) 04:00 ET: decided at about 19 s and again under the next
    // pseudonym at about 75 s; the attacker has left the grid by 110 s.
    let (r, entries, published, linked) = real_calendar_run("2026-01-06T09:00:00Z", 400.0);
    show("pending", &r, entries, published, linked);
    let p = &r.phase2;
    assert_eq!(entries, 1, "the attacker's entry is at the Generator");
    assert_eq!(published, 0, "nothing is published before 10:00 ET");
    assert!(
        p.ma_revoke_decisions >= 2,
        "a later decision about the attacker"
    );
    assert_eq!(
        p.decisions_already_covered, 0,
        "a decision was counted covered by a list not yet published"
    );
    assert!(p.decisions_linked >= 1, "the later decision ends linked");
    assert_eq!(linked as u64, p.decisions_linked);
    assert_eq!(
        p.ma_revoke_decisions,
        1 + p.decisions_linked + p.decisions_already_covered
    );
    assert_eq!(p.revocations_repeated, 0);

    // 2026-01-06 09:59 ET: the 10:00 slot publishes the entry 60 s into the run, after
    // the first case (opened at about 19 s, resolved after the 30 s review) and before
    // the decision about the next pseudonym (about 75 s).
    let (r, entries, published, linked) = real_calendar_run("2026-01-06T14:59:00Z", 400.0);
    show("published", &r, entries, published, linked);
    let q = &r.phase2;
    assert_eq!(entries, 1);
    assert_eq!(published, 1, "the 10:00 ET slot published the entry");
    assert_eq!(
        q.decisions_already_covered, 1,
        "the decision after publication is covered by the published list"
    );
    assert_eq!(q.decisions_linked, 0, "and opens no case");
    assert_eq!(q.cases_opened, 1);
    assert_eq!(
        q.ma_revoke_decisions,
        1 + q.decisions_linked + q.decisions_already_covered
    );
    assert_eq!(q.revocations_repeated, 0);
}
