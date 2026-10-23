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
