//! WP-B4: the Misbehaviour Authority's case table (gap row 27), its decision modes (row
//! 28), and a real US misbehaviour report driving a revocation end to end (row 25).

mod common;

use common::{DEVICE_A, DEVICE_B, deployment, provisioned};
use v2xw_core::ids::NodeId;
use v2xw_core::time::{Duration, NS_PER_S};
use v2xw_proto::scms::mbr::{self, Evidence, MbrType, MisbehaviorReport, SignedBsms, Subtype};
use v2xw_proto::scms::msg::SecuredReport;
use v2xw_proto::scms::params::MaMode;
use v2xw_proto::scms::pdu::{Drbg, Pdu, SpduKind};
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::stage::{FlowId, StageId};
use v2xw_sec::envelope::GenerationLocation;
use v2xw_sec::sig1609::Signer1609;

const CASES: u32 = 100;

fn dev(k: u32) -> NodeId {
    NodeId::new(1_000 + k)
}

#[test]
fn a_hundred_concurrent_cases_all_complete_with_every_stage_stamped() {
    let mut run = deployment(CASES + 1);
    for k in 0..CASES {
        run.enrol(dev(k));
    }
    run.run().expect("enrolment runs");
    for k in 0..CASES {
        run.provision(dev(k), 0, 1, 1);
    }
    run.run().expect("provisioning runs");
    let reporter = dev(CASES);
    for k in 0..CASES {
        let lv = run.state.devices[&dev(k)].credentials[&(0, 0)].lv;
        run.submit_report(reporter, 0, lv);
    }
    run.run().expect("the reports run");
    assert_eq!(run.state.ma.reports.len(), CASES as usize);
    // Every case is opened before any has run a step: they are all open at once.
    let mut runs = Vec::new();
    for k in 0..CASES as usize {
        runs.push(run.revoke(k, 0, 1).expect("the case table has room"));
    }
    assert_eq!(run.state.ma.open_cases(), CASES as usize);
    run.run().expect("the cases run");
    assert_eq!(run.state.ma.open_cases(), 0);
    assert_eq!(run.state.ma.cases.len(), CASES as usize);
    assert!(run.state.ma.cases.values().all(|c| c.done && c.resolved));
    for (res, crl) in &runs {
        for s in [StageId::Decision, StageId::Resolved, StageId::Blocklisted] {
            assert!(run.kernel.stages.at(*res, s).is_some(), "{res:?} {s:?}");
        }
        for s in [StageId::Issued, StageId::Published] {
            assert!(run.kernel.stages.at(*crl, s).is_some(), "{crl:?} {s:?}");
        }
    }
    for k in 0..CASES {
        assert!(run.state.ra.blocklist.contains(&dev(k)), "device {k}");
    }
    assert!(!run.state.ra.blocklist.contains(&reporter));
    assert_eq!(run.state.crl_store.entries.len(), CASES as usize);
    // The last full CRL lists all hundred devices.
    let last = run
        .state
        .crlg
        .issued
        .iter()
        .rev()
        .find(|c| !c.delta)
        .unwrap();
    assert_eq!(last.entries, CASES);
}

#[test]
fn a_full_case_table_refuses_and_frees_its_slot_when_a_case_ends() {
    let mut run = deployment(3);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    provisioned(&mut run, DEVICE_B, 0, 1, 2);
    for d in [DEVICE_A, DEVICE_B] {
        let lv = run.state.devices[&d].credentials[&(0, 0)].lv;
        run.submit_report(dev(2), 0, lv);
    }
    run.run().expect("runs");
    run.state.params.ma.max_open_cases = 1;
    assert!(run.revoke(0, 0, 2).is_some());
    assert!(run.revoke(1, 0, 2).is_none(), "full");
    run.run().expect("runs");
    assert!(run.revoke(1, 0, 2).is_some(), "the slot was freed");
}

#[test]
fn manual_review_waits_for_the_operator_and_automatic_does_not() {
    for (mode, wait) in [
        (
            MaMode::ManualReview {
                review: Duration::from_secs(8 * 3600),
            },
            8 * 3600,
        ),
        (MaMode::Automatic, 0),
    ] {
        let mut run = deployment(2);
        run.state.params.ma.mode = mode;
        provisioned(&mut run, DEVICE_A, 0, 1, 2);
        let lv = run.state.devices[&DEVICE_A].credentials[&(0, 0)].lv;
        run.submit_report(DEVICE_B, 0, lv);
        run.run().expect("runs");
        let asked = run.kernel.now();
        let (res, crl) = run.request_revocation(0, 0, 2).expect("held");
        assert!(run.case_open());
        run.run().expect("runs");
        let decided = run.kernel.stages.at(res, StageId::Decision).unwrap();
        assert!(
            decided >= asked + wait * NS_PER_S && decided < asked + (wait + 1) * NS_PER_S,
            "{mode:?}: decided {decided}, asked {asked}"
        );
        assert!(run.kernel.stages.at(crl, StageId::Published).is_some());
        assert!(run.state.ra.blocklist.contains(&DEVICE_A));
    }
}

fn implicit(
    run: &ScmsRun,
    d: NodeId,
    j: u32,
) -> (v2xw_msg::sec_types::ieee1609_dot2::Certificate, Signer1609) {
    let c = run.state.devices[&d].credentials[&(0, j)]
        .implicit
        .as_ref()
        .unwrap();
    (
        c.certificate.clone(),
        Signer1609::p256(&v2xw_sec::ec::scalar_to_be32(&c.private)).unwrap(),
    )
}

fn speeding(run: &ScmsRun, accused: NodeId) -> MisbehaviorReport {
    let (cert, key) = implicit(run, accused, 0);
    let bsm = mbr::sign_bsm(&[0x5a; 40], 1_000_000, &cert, &key).unwrap();
    MisbehaviorReport {
        generation_time: 600_000,
        location: GenerationLocation {
            lat_tenth_microdeg: 407_440_000,
            lon_tenth_microdeg: -739_900_000,
            elevation: 4_096,
        },
        mbr_type: MbrType::Implausible,
        subtypes: vec![Subtype::MaxSpeed { threshold_mps: 90 }],
        evidence: Evidence::Suspect(SignedBsms {
            bsms: vec![bsm],
            cert,
        }),
    }
}

#[test]
fn a_real_report_reaches_the_ma_through_the_ra_and_drives_the_revocation() {
    let mut run = deployment(2);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    provisioned(&mut run, DEVICE_B, 0, 1, 2);
    let report = speeding(&run, DEVICE_A);
    let r = run.submit_mbr(DEVICE_B, &report).expect("a real report");
    run.run().expect("runs");
    let st = &run.state;
    assert!(st.pdu_rejections.is_empty(), "{:?}", st.pdu_rejections);
    for s in [
        StageId::Detect,
        StageId::ReportSent,
        StageId::ProxyForwarded,
        StageId::Shuffled,
        StageId::ReportReceived,
    ] {
        assert!(run.kernel.stages.at(r, s).is_some(), "{s:?}");
    }
    // Device → proxy → RA carries body + header; RA → MA the body only.
    let body = st
        .pdus
        .iter()
        .find(|p| p.kind == SpduKind::SecuredMisbehaviorReport)
        .unwrap()
        .len;
    let step = |name: &str| {
        run.kernel
            .steps
            .iter()
            .find(|s| s.run == r && s.step == name && s.flow == FlowId::Report)
            .unwrap()
            .bytes
    };
    assert!(step("report") > body);
    assert_eq!(step("report-proxied"), step("report"));
    assert_eq!(step("report-forward"), body);
    // The MA opened it, and the evidence is the accused's own signature.
    let o = &st.ma.opened_reports[0];
    assert_eq!(o.report, report);
    assert!(o.evidence_verified);
    // The subject is the accused certificate's linkage data: the revocation finds A.
    assert_eq!(
        st.ma.reports[0].subject_lv,
        st.devices[&DEVICE_A].credentials[&(0, 0)].lv
    );
    run.revoke(0, 0, 2).expect("held");
    run.run().expect("runs");
    assert!(run.state.ra.blocklist.contains(&DEVICE_A));
    assert!(!run.state.ra.blocklist.contains(&DEVICE_B));
}

/// The bytes `submit_mbr` would send for `reporter`, with the header signed by
/// `header_from`'s enrolment certificate (or none).
fn upload(run: &mut ScmsRun, reporter: NodeId, header_from: Option<NodeId>) -> SecuredReport {
    let pki = run.state.pki.clone().unwrap();
    let (cert, key) = implicit(run, reporter, 0);
    let report = speeding(run, DEVICE_A);
    let mut g = Drbg::new(5, b"ma-cases");
    let body = mbr::secure_report(
        &mbr::encode_report(&report).unwrap(),
        1,
        &cert,
        &key,
        &pki.ma.cert,
        &pki.ma.enc_public().unwrap(),
        &mut g,
    )
    .unwrap();
    let header = header_from.map(|d| {
        let e = run.state.devices[&d].enrolment_credential.clone().unwrap();
        Pdu::new(
            mbr::upload_header(1, &e, &pki.ra.cert, &pki.ra.enc_public().unwrap(), &mut g).unwrap(),
        )
    });
    SecuredReport {
        report: Pdu::new(body),
        header,
    }
}

#[test]
fn the_ra_and_the_ma_refuse_unauthenticated_misattributed_and_tampered_uploads() {
    let mut run = deployment(2);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    provisioned(&mut run, DEVICE_B, 0, 1, 2);
    let lv = run.state.devices[&DEVICE_A].credentials[&(0, 0)].lv;
    // 1. No Download-Req header: an unauthenticated POST.
    let s = upload(&mut run, DEVICE_B, None);
    run.submit_secured(DEVICE_B, 0, lv, s);
    // 2. B's session presenting A's enrolment-signed header.
    let s = upload(&mut run, DEVICE_B, Some(DEVICE_A));
    run.submit_secured(DEVICE_B, 0, lv, s);
    // 3. A valid header, but the body was altered on the way (the RA cannot tell).
    let mut s = upload(&mut run, DEVICE_B, Some(DEVICE_B));
    let mut b = s.report.bytes().to_vec();
    let n = b.len();
    b[n - 30] ^= 0x40;
    s.report = Pdu::new(b);
    run.submit_secured(DEVICE_B, 0, lv, s);
    // 4. A valid one.
    let s = upload(&mut run, DEVICE_B, Some(DEVICE_B));
    run.submit_secured(DEVICE_B, 0, lv, s);
    run.run().expect("runs");
    let st = &run.state;
    let rej: Vec<_> = st
        .pdu_rejections
        .iter()
        .filter(|r| r.kind == SpduKind::SecuredMisbehaviorReport)
        .collect();
    assert_eq!(rej.len(), 3, "{rej:?}");
    let ra = st.nodes.ra.index();
    let ma = st.nodes.ma.index();
    assert_eq!(
        rej.iter().filter(|r| r.at_node == ra).count(),
        2,
        "two at the RA"
    );
    assert_eq!(
        rej.iter().filter(|r| r.at_node == ma).count(),
        1,
        "one at the MA"
    );
    assert_eq!(st.ma.opened_reports.len(), 1);
    assert_eq!(st.ma.reports.len(), 1);
}

/// Realism R-CASE (docs/design/scms-real/testing/realism.md): under manual review the case
/// board dates "reports in" from the report's arrival at the MA, the operator's review time
/// before the decision. It was stamped at the decision, so the board never showed a case
/// waiting for review.
#[test]
fn the_case_board_dates_reports_in_from_the_arrival_not_the_decision() {
    use v2xw_proto::scms::pki_view::{self, PkiTracker};
    let review = 8 * 3600;
    let mut run = deployment(2);
    run.state.params.ma.mode = MaMode::ManualReview {
        review: Duration::from_secs(review),
    };
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    let lv = run.state.devices[&DEVICE_A].credentials[&(0, 0)].lv;
    run.submit_report(DEVICE_B, 0, lv);
    run.run().expect("runs");
    let arrived = *run
        .state
        .ma
        .report_arrived
        .last()
        .expect("the arrival is kept");
    let (res, _) = run.request_revocation(0, 0, 2).expect("held");
    run.run().expect("runs");
    let decided = run
        .kernel
        .stages
        .at(res, StageId::Decision)
        .expect("decided");
    let mut tracker = PkiTracker::default();
    let now = run.kernel.now();
    let cases = pki_view::drain_cases(std::slice::from_ref(&run), &mut tracker, now, &|_, _| None);
    let at = |s: &str| cases.iter().find(|c| c.stage == s).map(|c| c.t);
    assert_eq!(at("reports-in"), Some(arrived), "{cases:?}");
    assert_eq!(at("decision"), Some(decided));
    assert!(
        decided >= arrived + review * NS_PER_S,
        "{arrived} -> {decided}"
    );
}

/// Realism R-CORR (docs/design/scms-real/testing/realism.md): the case board's
/// `corroborated` is the instant the MA's pipeline opened the case, and the operator's
/// decision follows the 8 h review later. Fault (the first form, `corroborated` stamped
/// at the decision): the stage lands 8 h late, on the decision itself.
#[test]
fn the_case_board_dates_corroboration_from_the_case_opening_not_the_decision() {
    use v2xw_proto::scms::pki_view::{self, PkiTracker};
    let review = 8 * 3600;
    let mut run = deployment(2);
    run.state.params.ma.mode = MaMode::ManualReview {
        review: Duration::from_secs(review),
    };
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    let lv = run.state.devices[&DEVICE_A].credentials[&(0, 0)].lv;
    run.submit_report(DEVICE_B, 0, lv);
    run.run().expect("runs");
    let opened = run.kernel.now();
    let (res, _) = run.request_revocation(0, 0, 2).expect("held");
    run.run().expect("runs");
    let decided = run
        .kernel
        .stages
        .at(res, StageId::Decision)
        .expect("decided");
    let mut tracker = PkiTracker::default();
    let now = run.kernel.now();
    let cases = pki_view::drain_cases(std::slice::from_ref(&run), &mut tracker, now, &|_, _| None);
    let at = |s: &str| cases.iter().find(|c| c.stage == s).map(|c| c.t);
    assert_eq!(at("corroborated"), Some(opened), "{cases:?}");
    assert_eq!(at("decision"), Some(decided));
    assert!(
        decided >= opened + review * NS_PER_S,
        "{opened} -> {decided}"
    );
}
