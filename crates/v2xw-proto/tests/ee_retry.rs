//! The EE request retry of [CAMP122] §5.2.8.3 (edge-case package, item 3).
//!
//! - SCMS-522: "the EE shall retry if no response is received within 10 seconds".
//! - SCMS-523: at most 10 retries per 60 minutes.
//!
//! Before this package a request delivered to an RA that was down was dropped, nothing
//! told the device, and it never asked again: one outage during a top-up starved the
//! vehicle for the rest of the run (validation/protocols.md B6). Each test below also runs
//! with the retry turned off (`ee_retry_timeout` = 0, the old behaviour) and shows the
//! outcome the fix changes, so it is proved able to fail.

mod common;

use common::{DEVICE_A, provisioned};
use v2xw_core::time::{Duration, SimTime};
use v2xw_proto::fault::{Fault, FaultKind};
use v2xw_proto::scms::params::ScmsParams;
use v2xw_proto::scms::pdu::SpduKind;
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::stage::StageId;

const S: u64 = 1_000_000_000;

fn params(retry: bool) -> ScmsParams {
    let mut p = ScmsParams::default().quick();
    p.pdu.keep_bytes = true;
    if !retry {
        p.ee_retry_timeout = Duration::ZERO;
    }
    p
}

fn run_with(p: ScmsParams) -> ScmsRun {
    let mut run = ScmsRun::new(p).expect("certificates encode");
    run.add_device(DEVICE_A);
    run
}

fn ra_fault(run: &ScmsRun, kind: FaultKind, from: SimTime, until: SimTime) -> Fault {
    Fault {
        node: run.state.nodes.ra,
        kind,
        from,
        until,
    }
}

/// A weekly top-off whose `.info` request meets an RA that is down for five minutes.
fn topoff_through_outage(retry: bool) -> (ScmsRun, Option<SimTime>, SimTime) {
    let mut run = run_with(params(retry));
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    assert!(run.state.devices[&DEVICE_A].ra_account.is_some());
    let start = run.kernel.now();
    let f = ra_fault(&run, FaultKind::Offline, start, start + 300 * S);
    run.set_faults(vec![f]);
    let t = run.topup_at(DEVICE_A, start, 1, 2);
    run.run().expect("runs");
    let installed = run.kernel.stages.at(t, StageId::Installed);
    (run, installed, start)
}

/// The retry counters, for a failure message.
fn counters(run: &ScmsRun) -> (u32, u32, u32) {
    let d = &run.state.devices[&DEVICE_A];
    (d.ee_retries, d.ee_retries_capped, d.duplicate_answers)
}

#[test]
fn a_top_off_retries_through_an_ra_outage_and_completes_at_the_cap() {
    let (run, installed, start) = topoff_through_outage(true);
    let dev = &run.state.devices[&DEVICE_A];
    let at = installed.expect("the top-off completes once the RA is back");
    // Ten retries 10 s apart (t+10 … t+100 s) all meet the outage; the eleventh is held
    // by SCMS-523 until the first leaves the 60-minute window (t+3,610 s) and succeeds.
    assert_eq!(dev.ee_retries, 11, "{:?}", counters(&run));
    assert_eq!(dev.ee_retries_capped, 1);
    assert!(
        at >= start + 3_610 * S && at < start + 3_700 * S,
        "installed {} s after the request",
        (at - start) / S
    );
    assert!(dev.credentials.contains_key(&(1, 0)) && dev.credentials.contains_key(&(1, 1)));
    // Every retry was dropped at the RA: 10 retries plus the first request, each a
    // FileRequest and an InfoRequest.
    assert!(
        run.state.faults.dropped >= 22,
        "{}",
        run.state.faults.dropped
    );
}

#[test]
fn without_the_retry_one_outage_loses_the_top_off_for_good() {
    // The behaviour before the package (and the fault the test above is proved against).
    let (run, installed, _) = topoff_through_outage(false);
    assert_eq!(installed, None);
    let dev = &run.state.devices[&DEVICE_A];
    assert_eq!(dev.ee_retries, 0);
    assert!(!dev.credentials.contains_key(&(1, 0)));
}

/// A first butterfly request to an RA that answers 15 s late: the device has retried
/// by then, so the RA sees the same request twice.
fn slow_first_request(retry: bool) -> ScmsRun {
    let mut run = run_with(params(retry));
    run.enrol(DEVICE_A);
    run.run().expect("enrolment runs");
    let now = run.kernel.now();
    let f = ra_fault(
        &run,
        FaultKind::Slow {
            extra: Duration::from_secs(15),
        },
        now,
        u64::MAX,
    );
    run.set_faults(vec![f]);
    run.provision(DEVICE_A, 0, 2, 3);
    run.run().expect("provisioning runs");
    run
}

#[test]
fn a_retried_request_is_acknowledged_again_and_issued_once() {
    let run = slow_first_request(true);
    let dev = &run.state.devices[&DEVICE_A];
    let ra = &run.state.ra;
    assert!(dev.ee_retries >= 1, "{:?}", counters(&run));
    assert_eq!(ra.accepted, 1, "one request, one account");
    assert!(ra.retransmissions_reacked >= 1);
    assert_eq!(
        ra.max_reqs_refused, 0,
        "a retransmission is not a second request"
    );
    assert_eq!(run.refusal_of(DEVICE_A), None);
    assert!(dev.duplicate_answers >= 1, "the second ack is dropped");
    // Exactly the six certificates asked for, each issued once.
    assert_eq!(dev.credentials.len(), 6);
    assert_eq!(run.state.pca.issued_count, 6);
    // The retransmission is the same SPDU: one requestHash, which names the batch files.
    let sent: std::collections::BTreeSet<Vec<u8>> = run
        .state
        .pdus
        .iter()
        .filter(|p| p.kind == SpduKind::EeRaCertRequest && p.from == DEVICE_A.index())
        .filter_map(|p| p.bytes.as_ref().map(|b| b.bytes().to_vec()))
        .collect();
    assert_eq!(
        sent.len(),
        1,
        "one request, sent more than once, byte for byte"
    );
}

#[test]
fn a_slow_ra_without_the_retry_sees_one_request() {
    let run = slow_first_request(false);
    assert_eq!(run.state.devices[&DEVICE_A].ee_retries, 0);
    assert_eq!(run.state.ra.retransmissions_reacked, 0);
    assert_eq!(run.state.devices[&DEVICE_A].credentials.len(), 6);
}

#[test]
fn a_run_where_every_request_is_answered_retries_nothing() {
    let mut run = run_with(params(true));
    provisioned(&mut run, DEVICE_A, 0, 2, 3);
    let t = run.topup(DEVICE_A, 2, 3);
    run.run().expect("runs");
    assert!(run.kernel.stages.at(t, StageId::Installed).is_some());
    let dev = &run.state.devices[&DEVICE_A];
    assert_eq!((dev.ee_retries, dev.duplicate_answers), (0, 0));
}

#[test]
fn a_batch_download_retries_through_a_partition() {
    // The device's path to the RA (the Location Obscurer Proxy) is cut after the ack,
    // across the first batch poll (first_batch_delay 60 s, quick shuffle 60 s): the RA
    // itself keeps working, the download request is retried and the batch arrives. (An
    // outage of the RA itself at this point also loses the LAs' and the ACA's answers
    // to it, which no specification has them retry: the batch is then never made.)
    let mut run = run_with(params(true));
    run.enrol(DEVICE_A);
    run.run().expect("enrolment runs");
    let now = run.kernel.now();
    let f = Fault {
        node: run.state.nodes.lop,
        kind: FaultKind::Offline,
        from: now + 30 * S,
        until: now + 200 * S,
    };
    run.set_faults(vec![f]);
    let t = run.provision(DEVICE_A, 0, 1, 2);
    run.run().expect("runs");
    let dev = &run.state.devices[&DEVICE_A];
    assert!(
        run.kernel.stages.at(t, StageId::Installed).is_some(),
        "{:?}",
        counters(&run)
    );
    assert!(dev.ee_retries >= 1);
    assert_eq!(dev.credentials.len(), 2);
}

// ---------------------------------------------------------------------------------------
// Edge-case item 5: a list is renewed by its nextCrl, so a healthy deployment never
// reaches `overdue_CRL_tolerance` ([CAMP122] Table 67, 2 weeks past nextCrl).
// ---------------------------------------------------------------------------------------

const DAY: u64 = 86_400 * S;

fn standing(reissue: bool) -> ScmsRun {
    let mut p = params(true);
    p.crl.reissue = reissue;
    let mut run = run_with(p);
    run.seed_standing_crl(5);
    run
}

fn next_crl_of(run: &ScmsRun) -> u32 {
    let pdu = run.state.crl_store.secured.as_ref().expect("a signed CRL");
    v2xw_sec::crl::decode_crl(pdu.bytes())
        .expect("decodes")
        .contents
        .next_crl
        .0
        .0
}

#[test]
fn an_idle_crl_generator_renews_its_list_every_next_crl() {
    let mut run = standing(true);
    assert_eq!(run.state.crl_store.versions, 1);
    let first = next_crl_of(&run);
    run.run_until(8 * DAY).expect("runs");
    assert_eq!(run.state.crlg.reissued, 1);
    assert_eq!(run.state.crl_store.versions, 2);
    let second = next_crl_of(&run);
    assert_eq!(
        second,
        first + 7 * 86_400,
        "issued at the old nextCrl, a week on"
    );
    assert_eq!(run.state.crl_store.entries.len(), 5, "the same entries");
    run.run_until(22 * DAY).expect("runs");
    assert_eq!(run.state.crlg.reissued, 3, "days 7, 14 and 21");
}

#[test]
fn without_renewal_the_list_goes_past_its_overdue_tolerance() {
    // The engine before the package: the Generator signs only when an entry arrives, so
    // three weeks with no revocation leave every device holding a list whose nextCrl is
    // more than two weeks gone, and refusing every certificate under that CRACA.
    let mut run = standing(false);
    let first = next_crl_of(&run);
    run.run_until(22 * DAY).expect("runs");
    assert_eq!(run.state.crlg.reissued, 0);
    assert_eq!(next_crl_of(&run), first);
    let now32 = first + 15 * 86_400;
    assert!(now32 > first + v2xw_node_overdue_tolerance());
}

/// `overdue_CRL_tolerance`, 2 weeks ([CAMP122] Table 67; `v2xw_node::rxval`).
fn v2xw_node_overdue_tolerance() -> u32 {
    14 * 86_400
}

// ---------------------------------------------------------------------------------------
// Edge-case item 6: a standing CRL under the provider's own LA pair costs what F2 measured
// for a real series-1 linkage CRL, 33.08 B an entry (04-validation-report.md; Brecht ~40).
// ---------------------------------------------------------------------------------------

fn standing_len(n: u32) -> usize {
    let mut run = run_with(params(true));
    run.seed_standing_crl(n);
    let la: Vec<_> = run.state.la.iter().map(|l| l.la_id).collect();
    assert!(
        run.state
            .crl_store
            .entries
            .iter()
            .all(|e| e.la_id1 == la[0] && e.la_id2 == la[1]),
        "every entry under the provider's LA1 and LA2"
    );
    run.state.crl_store.secured.as_ref().expect("signed").len() as usize
}

#[test]
fn a_standing_crl_costs_thirty_three_bytes_an_entry() {
    let per = (standing_len(2_000) - standing_len(1_000)) as f64 / 1_000.0;
    assert!((32.5..33.5).contains(&per), "{per} B an entry");
}
