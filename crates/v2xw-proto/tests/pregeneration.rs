//! WP-B3a leftovers: request once, the RA pre-generates, the device tops off by download.
//!
//! - `ra-maxReqs` = 1: a second butterfly request is refused until the policy changes
//!   ([SMP] §2.1.21 Table 3; [CAMP122] SCMS-520).
//! - The RA keeps `ra-maxPreloadTime` (the policy's `max_periods_ahead`, 3 years) generated
//!   from the one request and adds a week at every i-period boundary ([PRIMER] p.6), inside
//!   `ra-maxGenDelay` = 1 week.
//! - A top-off is `GET /download/info` (`Download-Req` `<hash>.info`, enrolment-signed,
//!   5 s TTL), answered with one COER `Time32`, then the batch download ([CAMP122] §6.3).
//! - The device polls a batch that is not ready every `download-minWait` (5 min), at most
//!   `download-maxReqs` (100) more times (the 408 rule, [SMP] §4.3.1).
//! - ACA blindness: nothing the ACA receives or keeps names a device or links two of its
//!   certificates; only the RA maps the ACA's per-certificate handle back.

mod common;

use common::{DEVICE_A, DEVICE_B, provisioned};
use v2xw_core::ids::NodeId;
use v2xw_core::time::Duration;
use v2xw_proto::fault::{Fault, FaultKind};
use v2xw_proto::scms::msg::Refusal;
use v2xw_proto::scms::params::ScmsParams;
use v2xw_proto::scms::pdu::{self, SpduKind};
use v2xw_proto::scms::pdu_prov;
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::stage::StageId;

const WEEK_NS: u64 = 7 * 86_400 * 1_000_000_000;

fn run_with(p: ScmsParams, devices: &[NodeId]) -> ScmsRun {
    let mut run = ScmsRun::new(p).expect("certificates encode");
    for d in devices {
        run.add_device(*d);
    }
    run
}

fn bytes_run(devices: &[NodeId]) -> ScmsRun {
    let mut p = ScmsParams::default().quick();
    p.pdu.keep_bytes = true;
    run_with(p, devices)
}

fn count(run: &ScmsRun, k: SpduKind) -> usize {
    run.state.pdus.iter().filter(|p| p.kind == k).count()
}

/// The file names of every `EeRaDownloadRequestSpdu` the RA received, in order.
fn download_names(run: &ScmsRun) -> Vec<String> {
    let pki = run.state.pki.as_ref().unwrap();
    let ra = run.state.nodes.ra.index();
    run.state
        .pdus
        .iter()
        .filter(|p| p.kind == SpduKind::EeRaDownloadRequest && p.to == ra)
        .filter_map(|p| {
            let b = p.bytes.as_ref()?;
            pdu_prov::ra_open_download_request(pki, b.bytes(), 0, u32::MAX)
                .ok()
                .map(|o| o.filename)
        })
        .collect()
}

#[test]
fn a_second_butterfly_request_is_refused_until_the_policy_changes() {
    let mut run = bytes_run(&[DEVICE_A]);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    assert!(run.state.devices[&DEVICE_A].ra_account.is_some());
    run.provision(DEVICE_A, 1, 1, 2);
    run.run().expect("runs");
    assert_eq!(run.state.ra.max_reqs_refused, 1);
    assert_eq!(run.refusal_of(DEVICE_A), Some(Refusal::RequestLimit));
    assert!(
        !run.state.devices[&DEVICE_A]
            .credentials
            .contains_key(&(1, 0))
    );
    assert_eq!(run.state.ra.accepted, 1, "one request accepted");
    // A policy change is a parameter change: the next request is accepted.
    let mut policy = run.state.gov.gpf.policy;
    policy.certs_per_period = 1;
    run.decide_policy_at(policy, run.kernel.now());
    run.run().expect("policy runs");
    run.provision(DEVICE_A, 1, 1, 2);
    run.run().expect("runs");
    assert_eq!(run.state.ra.accepted, 2);
    assert_eq!(run.state.ra.max_reqs_refused, 1);
    assert!(
        run.state.devices[&DEVICE_A]
            .credentials
            .contains_key(&(1, 0))
    );
}

#[test]
fn a_weekly_top_off_is_an_info_download_on_the_one_request() {
    // NYC: 60 certificates a week, one week ahead, topped off weekly ([CVP-P2] items 21,
    // 24; [CVP-LOG] p.16 weekly top-off monitoring). One request in all.
    let mut p = ScmsParams::default().quick();
    p.pdu.keep_bytes = true;
    p.certs_per_period = 60;
    let mut run = run_with(p, &[DEVICE_A]);
    provisioned(&mut run, DEVICE_A, 614, 1, 60);
    let h8 = run.state.devices[&DEVICE_A]
        .request_hash8
        .expect("real path");
    for week in 1..=3u32 {
        let t = run.topup(DEVICE_A, 614 + week, 60);
        run.run().expect("runs");
        assert!(run.kernel.stages.at(t, StageId::Installed).is_some());
        assert_eq!(
            run.state.devices[&DEVICE_A].credentials.len(),
            60 * (week as usize + 1)
        );
    }
    assert_eq!(
        count(&run, SpduKind::EeRaCertRequest),
        2,
        "one request, two hops"
    );
    assert_eq!(run.state.ra.accepted, 1);
    assert_eq!(run.state.ra.info_served, 3);
    assert_eq!(run.state.ra.pregen_periods, 3);
    assert_eq!(run.state.devices[&DEVICE_A].info_requests, 3);
    assert_eq!(run.state.pca.issued_count, 240);
    // The RA saw: the first batch, then per week `.info` and that week's batch.
    let info = pdu_prov::info_filename(h8);
    let mut want = vec![pdu::batch_filename(h8, 614)];
    for week in 1..=3u32 {
        want.push(info.clone());
        want.push(pdu::batch_filename(h8, 614 + week));
    }
    assert_eq!(download_names(&run), want);
    // Every pre-generated week walks the request's one chain at each LA.
    for la in &run.state.la {
        assert_eq!(la.chains.len(), 1, "one chain per request");
    }
    let dev = &run.state.devices[&DEVICE_A];
    assert!(
        dev.credentials
            .keys()
            .all(|&(i, j)| dev.credential_is_usable(i, j))
    );
    assert!(
        run.state.pdu_rejections.is_empty(),
        "{:?}",
        run.state.pdu_rejections
    );
}

#[test]
fn the_ra_keeps_three_years_generated_and_adds_a_week_at_each_boundary() {
    // THEA preloaded 3 years ([CVP-LOG] p.16); the RA keeps ra-maxPreloadTime = 3 years
    // (156 weeks) generated and adds a week every week ([PRIMER] p.6).
    let mut run = bytes_run(&[DEVICE_A]);
    provisioned(&mut run, DEVICE_A, 614, 1, 1);
    let account = run.state.ra.accounts[&DEVICE_A].clone();
    let accepted = account.accepted_at;
    // The last week inside the horizon is there now; the next arrives at the boundary.
    assert_eq!(run.state.pregen_due(&account, 614 + 155), Some(accepted));
    assert_eq!(run.state.pregen_due(&account, 614 + 156), Some(WEEK_NS));
    assert_eq!(run.state.pregen_due(&account, 614 + 157), Some(2 * WEEK_NS));
    assert_eq!(run.state.pregen_due(&account, 613), None, "before firstI");

    let t = run.topup(DEVICE_A, 614 + 155, 1);
    run.run().expect("runs");
    let done = run
        .kernel
        .stages
        .at(t, StageId::Installed)
        .expect("installed");
    assert!(done < 600 * 1_000_000_000, "available at once: {done}");
    assert_eq!(run.state.ra.pregen_deferred, 0);

    let t = run.topup(DEVICE_A, 614 + 156, 1);
    run.run().expect("runs");
    assert_eq!(run.state.ra.pregen_deferred, 1);
    let expanded = run
        .kernel
        .stages
        .at(t, StageId::Expanded)
        .expect("expanded");
    let done = run
        .kernel
        .stages
        .at(t, StageId::Installed)
        .expect("installed");
    // Generated at the boundary, within ra-maxGenDelay (1 week) of it, and only then.
    assert!(
        (WEEK_NS..WEEK_NS + 1_000_000_000).contains(&expanded),
        "{expanded}"
    );
    assert!((WEEK_NS..2 * WEEK_NS).contains(&done), "{done}");
    // A period before firstI is never generated: the device is told so and stops.
    let t = run.topup(DEVICE_A, 613, 1);
    run.run().expect("runs");
    assert!(run.kernel.stages.at(t, StageId::Installed).is_none());
    assert_eq!(run.state.devices[&DEVICE_A].topoffs_unavailable, 1);
}

/// On the real US calendar (WIRE) the weekly job runs at the real i-period boundary, not
/// a whole week after the run's start: `pregen_due` counts boundaries on the run's own
/// grid. Hand derivation: t0 = 2026-10-16 12:00 UTC = Unix 1 792 152 000 lies in i = 614,
/// which began 2026-10-13 09:00 UTC = Unix 1 791 882 000 (see `scms_wiring.rs`); i = 615
/// begins 604 800 s later, at Unix 1 792 486 800, which is 334 800 s after t0.
#[test]
fn on_the_real_calendar_the_next_week_is_generated_at_the_real_boundary() {
    let mut p = ScmsParams::default().quick();
    p.pdu.keep_bytes = true;
    p.calendar_epoch_unix_s = Some(v2xw_proto::scms::governance::US_IPERIOD_EPOCH_UNIX_S);
    p.pdu.t0_unix_s = 1_792_152_000;
    let mut run = run_with(p, &[DEVICE_A]);
    assert_eq!(run.state.params.period_at(0), 614);
    provisioned(&mut run, DEVICE_A, 614, 1, 1);
    let account = run.state.ra.accounts[&DEVICE_A].clone();
    let next = 334_800 * 1_000_000_000;
    assert!(account.accepted_at < next);
    assert_eq!(
        run.state.pregen_due(&account, 614 + 155),
        Some(account.accepted_at)
    );
    assert_eq!(run.state.pregen_due(&account, 614 + 156), Some(next));
    assert_eq!(
        run.state.pregen_due(&account, 614 + 157),
        Some(next + WEEK_NS)
    );
}

#[test]
fn the_preload_horizon_is_held_between_two_weeks_and_three_years() {
    use v2xw_proto::scms::params::{PreloadPolicy, RA_MAX_PRELOAD_PERIODS, RA_MIN_PRELOAD_PERIODS};
    assert_eq!((RA_MIN_PRELOAD_PERIODS, RA_MAX_PRELOAD_PERIODS), (2, 156));
    assert_eq!(PreloadPolicy::Thea3Years.device_periods(), 156);
    assert_eq!(PreloadPolicy::Nyc1Week.device_periods(), 1);
    for (policy, horizon) in [(1u32, 2u32), (2, 2), (40, 40), (156, 156), (400, 156)] {
        let mut p = ScmsParams::default().quick();
        p.max_periods_ahead = policy;
        let mut run = run_with(p, &[DEVICE_A]);
        provisioned(&mut run, DEVICE_A, 0, 1, 1);
        let a = run.state.ra.accounts[&DEVICE_A].clone();
        assert_eq!(
            run.state.pregen_due(&a, horizon - 1),
            Some(a.accepted_at),
            "policy {policy}"
        );
        assert_eq!(
            run.state.pregen_due(&a, horizon),
            Some(WEEK_NS),
            "policy {policy}"
        );
    }
}

#[test]
fn the_info_answer_is_one_time32_the_device_acts_on() {
    let mut run = bytes_run(&[DEVICE_A]);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    let t = run.topup(DEVICE_A, 1, 2);
    run.run().expect("runs");
    let step = run
        .kernel
        .steps
        .iter()
        .find(|s| s.step == "info-response")
        .expect("an .info answer");
    assert_eq!(step.bytes, 4, "one COER Time32");
    // The device came back for the batch no earlier than that time (Time32 seconds).
    let ack = run.kernel.stages.at(t, StageId::Acknowledged).unwrap();
    let dl = run.kernel.stages.at(t, StageId::Downloaded).unwrap();
    let wait = run.state.params.first_batch_delay.as_nanos();
    assert!(dl >= ack + wait - 1_000_000_000, "ack {ack} dl {dl}");
}

#[test]
fn an_info_request_naming_another_devices_file_is_refused() {
    let mut run = bytes_run(&[DEVICE_A, DEVICE_B]);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    provisioned(&mut run, DEVICE_B, 0, 1, 2);
    // Device A asks for B's `.info` (its own enrolment key, B's request hash).
    let b8 = run.state.devices[&DEVICE_B].request_hash8;
    run.state.devices.get_mut(&DEVICE_A).unwrap().request_hash8 = b8;
    let t = run.topup(DEVICE_A, 1, 2);
    run.run().expect("runs");
    assert!(run.kernel.stages.at(t, StageId::Acknowledged).is_none());
    assert!(run.kernel.stages.at(t, StageId::Installed).is_none());
    assert_eq!(run.state.ra.info_served, 0);
    assert!(
        run.state
            .pdu_rejections
            .iter()
            .any(|r| r.kind == SpduKind::EeRaDownloadRequest)
    );
}

#[test]
fn a_batch_that_never_comes_is_polled_every_five_minutes_at_most_one_hundred_more_times() {
    // download-minWait 5 min, download-maxReqs 100 ([SMP] Table 3), the 408 reading
    // "retry after minWait, up to maxReqs" ([SMP] §4.3.1).
    let mut run = bytes_run(&[DEVICE_A]);
    let pca = run.state.nodes.pca;
    run.set_faults(vec![Fault {
        node: pca,
        kind: FaultKind::Offline,
        from: 0,
        until: u64::MAX,
    }]);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    assert!(run.state.devices[&DEVICE_A].credentials.is_empty());
    let dev = DEVICE_A.index();
    let sends: Vec<u64> = run
        .state
        .pdus
        .iter()
        .filter(|p| p.kind == SpduKind::EeRaDownloadRequest && p.from == dev)
        .map(|p| p.at)
        .collect();
    assert_eq!(sends.len(), 101, "the first request and 100 retries");
    let five_min = Duration::from_secs(300).as_nanos();
    for w in sends.windows(2) {
        let gap = w[1] - w[0];
        assert!(
            (five_min..five_min + 1_000_000_000).contains(&gap),
            "gap {gap}"
        );
    }
}

#[test]
fn the_aca_can_link_no_certificate_to_a_device_or_to_another() {
    let mut run = bytes_run(&[DEVICE_A, DEVICE_B]);
    provisioned(&mut run, DEVICE_A, 0, 2, 2);
    provisioned(&mut run, DEVICE_B, 0, 2, 2);
    run.topup(DEVICE_A, 2, 2);
    run.run().expect("runs");
    let st = &run.state;
    assert_eq!(st.pca.issued.len(), 10);
    let pca = format!("{:?}", st.pca);
    let pca_node = st.nodes.pca.index();
    let to_aca: Vec<&[u8]> = st
        .pdus
        .iter()
        .filter(|p| p.to == pca_node)
        .filter_map(|p| p.bytes.as_ref().map(|b| b.bytes()))
        .collect();
    assert_eq!(to_aca.len(), 10, "one RaAcaCertRequestSpdu per certificate");
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let contains = |hay: &[u8], needle: &[u8]| hay.windows(needle.len()).any(|w| w == needle);
    for dev in [DEVICE_A, DEVICE_B] {
        let d = &st.devices[&dev];
        let cat = d.caterpillar.as_ref().unwrap();
        let enrol = d.enrolment_credential.as_ref().unwrap();
        let mut names: Vec<Vec<u8>> = vec![
            d.request_hash8.unwrap().to_vec(),
            pdu::cert_id8(&enrol.cert).to_vec(),
            d.ra_account.unwrap().to_vec(),
            cat.signing_public().compressed().unwrap().to_vec(),
            cat.encryption_public().compressed().unwrap().to_vec(),
        ];
        names.push(dev.index().to_be_bytes().to_vec());
        // Nothing the ACA keeps names the device ...
        assert!(!pca.contains(&format!("{dev:?}")), "node id in ACA state");
        for n in &names[..5] {
            assert!(!pca.contains(&hex(n)), "device identifier in ACA state");
            // ... and nothing it receives does.
            for b in &to_aca {
                assert!(!contains(b, n), "device identifier in an RA→ACA request");
            }
        }
    }
    // Nothing the ACA keeps is shared by two certificates.
    let lookups: Vec<_> = st.pca.issued.values().copied().collect();
    for (k, a) in lookups.iter().enumerate() {
        for b in &lookups[k + 1..] {
            assert_ne!(a.handle, b.handle);
            assert_ne!(a.lci1, b.lci1);
            assert_ne!(a.lci2, b.lci2);
        }
    }
    // The RA alone maps a handle back to the device.
    for ((i, _), l) in &st.pca.issued {
        let req = st.ra.handles[&l.handle];
        let owner = st.ra.requests[&req].device;
        assert!(
            st.devices[&owner]
                .credentials
                .keys()
                .any(|&(ci, _)| ci == *i)
        );
    }
}

#[test]
fn a_device_with_an_account_is_still_refused_a_top_off_once_blocklisted() {
    let mut run = bytes_run(&[DEVICE_A]);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    run.state.ra.blocklist.insert(DEVICE_A);
    let t = run.topup(DEVICE_A, 1, 2);
    run.run().expect("runs");
    assert!(run.kernel.stages.at(t, StageId::Installed).is_none());
    assert_eq!(run.topup_refusal_of(DEVICE_A), Some(Refusal::Blocklisted));
    assert_eq!(
        run.state.ra.pregen_periods, 0,
        "generation stops (SCMS-509)"
    );
}

/// "Generation stops for blacklisted devices" ([CAMP122] SCMS-509) also for a weekly job
/// the RA armed before the MA's decision: the timer fires, and the RA checks its blocklist
/// before it expands a single key.
#[test]
fn a_weekly_job_armed_before_the_blocklist_generates_nothing() {
    let mut run = bytes_run(&[DEVICE_A]);
    provisioned(&mut run, DEVICE_A, 614, 1, 1);
    // A top-off for the first week past the 3-year horizon arms the job for its boundary.
    let _ = run.topup(DEVICE_A, 614 + 156, 1);
    run.run_until(WEEK_NS / 2).expect("runs");
    assert_eq!(run.state.ra.pregen_deferred, 1, "armed for the boundary");
    let before = run.state.ra.pregen_periods;
    run.state.ra.blocklist.insert(DEVICE_A);
    run.run().expect("runs");
    assert_eq!(
        run.state.ra.pregen_blocked, 1,
        "the job ran into the blocklist"
    );
    assert_eq!(
        run.state.ra.pregen_periods, before,
        "and generated nothing (SCMS-509)"
    );
    assert!(
        !run.state.devices[&DEVICE_A]
            .credentials
            .contains_key(&(614 + 156, 0))
    );
}

/// The grid a device's CRL gate reads its clock against (`ScmsParams::period_grid`) is the
/// one the backend's `period_at` uses, on the real calendar and on a run-relative epoch,
/// on both sides of a boundary (06-validation-summary §6 item 22). Fault that turns it
/// red: the calendar origin's sign flipped, or the run epoch left out.
#[test]
fn the_period_grid_a_device_reads_is_the_backends() {
    let at = |grid: (i128, u64), t: u64| -> u32 {
        u32::try_from((i128::from(t) - grid.0).max(0) / i128::from(grid.1)).unwrap()
    };
    let mut cal = ScmsParams::default();
    cal.calendar_epoch_unix_s = Some(v2xw_proto::scms::governance::US_IPERIOD_EPOCH_UNIX_S);
    cal.pdu.t0_unix_s = 1_792_152_000;
    let mut rel = ScmsParams::default();
    rel.epoch = 5_000_000_000;
    for p in [cal, rel] {
        let grid = p.period_grid();
        let next = p.period_start(p.period_at(0) + 1);
        for t in [
            0,
            1,
            4_999_999_999,
            5_000_000_000,
            next - 1,
            next,
            next + WEEK_NS,
        ] {
            assert_eq!(at(grid, t), p.period_at(t), "t = {t}");
        }
    }
}
