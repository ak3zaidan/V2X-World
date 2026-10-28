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

/// The `.info` answer is one Time32 and the device comes back for the batch no earlier.
/// The week asked for is the first beyond the 156-week horizon, which the weekly job
/// generates at the boundary (one week after acceptance), so the answer names the boundary
/// plus the generation time. (Before the RA served a past-due week as held, this test
/// asked for week 1, which the RA had held since acceptance; the wait it saw was the lazy
/// job's.)
#[test]
fn the_info_answer_is_one_time32_the_device_acts_on() {
    let mut run = bytes_run(&[DEVICE_A]);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    let t = run.topup(DEVICE_A, 156, 2);
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
    assert!(ack < WEEK_NS, "asked before the boundary: {ack}");
    assert!(dl >= WEEK_NS + wait - 1_000_000_000, "ack {ack} dl {dl}");
}

/// A top-off of a week the RA's schedule reached before the request finds the file held
/// ([PRIMER] p.6: the RA keeps every week within `ra-maxPreloadTime` generated and adds one
/// a week; 1609.2.1 §6.3.5.4: the top-off is the `.info` request and the download). The
/// `.info` answer is now, the device downloads at once, and the top-off itself crosses no
/// RA-ACA hop; the week's certificates were issued all the same. Fault (2026-10-27): with
/// `ScmsRun::generate_past_due` returning at once, the device waits `first_batch_delay`
/// (60 s) for a job run at its request, and the ACA hops are in the top-off.
#[test]
fn a_week_that_fell_due_before_the_request_is_held_and_downloaded_at_once() {
    let mut run = bytes_run(&[DEVICE_A]);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    let issued = run.state.pca.issued_count;
    let mark = run.kernel.steps.len();
    let t = run.topup(DEVICE_A, 1, 2);
    run.run().expect("runs");
    let ack = run.kernel.stages.at(t, StageId::Acknowledged).unwrap();
    let dl = run.kernel.stages.at(t, StageId::Downloaded).unwrap();
    let wait = run.state.params.first_batch_delay.as_nanos();
    assert!(
        wait >= 10_000_000_000,
        "the quick profile's generation time"
    );
    assert!(dl < ack + wait / 6, "ack {ack} dl {dl}");
    assert!(run.kernel.stages.at(t, StageId::Installed).is_some());
    assert_eq!(run.state.pca.issued_count, issued + 2, "week 1 was issued");
    let pca = run.state.nodes.pca;
    let to_aca = run.kernel.steps[mark..]
        .iter()
        .filter(|s| s.to == pca)
        .count();
    assert_eq!(to_aca, 0, "the top-off crossed an RA-ACA hop");
    assert_eq!(run.state.devices[&DEVICE_A].credentials.len(), 4);
}

/// The `.info` answer for a week whose weekly job is still running is the file the RA
/// wrote when the job started: the job's start plus the RA's generation time
/// (`first_batch_delay`, 60 s), whenever the device asks (SCMS-576, [CAMP122] §5.2.7.8: the
/// RA keeps each EE's `.info` current and serves it as it is; 1609.2.1 `RaEeCertInfo`).
/// Device B's top-off before the boundary arms the weekly job for week 770 at the boundary,
/// which generates it for A's account too; the ACA is slow from the boundary, so the job is
/// still running when A asks 30 s later. A comes back at boundary + 60 s. Fault
/// (2026-10-27): the RA answering `now + first_batch_delay` for such a week, as it did
/// before (A came back at boundary + 90 s).
#[test]
fn a_device_asking_while_its_week_is_generated_reads_the_jobs_planned_time() {
    let mut run = bytes_run(&[DEVICE_A, DEVICE_B]);
    provisioned(&mut run, DEVICE_A, 614, 1, 1);
    provisioned(&mut run, DEVICE_B, 614, 1, 1);
    let week = 614 + 156;
    let account = run.state.ra.accounts[&DEVICE_A].clone();
    assert_eq!(run.state.pregen_due(&account, week), Some(WEEK_NS));
    let pca = run.state.nodes.pca;
    run.set_faults(vec![Fault {
        node: pca,
        kind: FaultKind::Slow {
            extra: Duration::from_secs(120),
        },
        from: WEEK_NS,
        until: u64::MAX,
    }]);
    let tb = run.topup(DEVICE_B, week, 1);
    let asked = WEEK_NS + 30_000_000_000;
    run.run_until(asked).expect("runs to the ask");
    assert!(
        run.state.ra.accounts[&DEVICE_A]
            .generated
            .contains(&week),
        "the weekly job took A's account into its batch"
    );
    let ta = run.topup_at(DEVICE_A, asked, week, 1);
    run.run().expect("runs");
    let dev = DEVICE_A.index();
    let sends: Vec<u64> = run
        .state
        .pdus
        .iter()
        .filter(|p| p.kind == SpduKind::EeRaDownloadRequest && p.from == dev && p.at >= asked)
        .map(|p| p.at)
        .collect();
    assert!(sends.len() >= 2, "the .info request and a download: {sends:?}");
    let wait = run.state.params.first_batch_delay.as_nanos();
    let planned = WEEK_NS + wait;
    assert!(
        (planned - 1_000_000_000..planned + 1_000_000_000).contains(&sends[1]),
        "A came back at {} s after the boundary, not at the job's planned {} s",
        (sends[1] - WEEK_NS) as f64 / 1e9,
        wait / 1_000_000_000
    );
    assert!(run.kernel.stages.at(ta, StageId::Installed).is_some());
    assert!(run.kernel.stages.at(tb, StageId::Installed).is_some());
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
    // 2 × (2 periods × 2) at the requests, then the weekly job adds week 2 for every
    // account (A asked; B's week is in the same population batch): 8 + 2 × 2.
    assert_eq!(st.pca.issued.len(), 12);
    let pca = format!("{:?}", st.pca);
    let pca_node = st.nodes.pca.index();
    let to_aca: Vec<&[u8]> = st
        .pdus
        .iter()
        .filter(|p| p.to == pca_node)
        .filter_map(|p| p.bytes.as_ref().map(|b| b.bytes()))
        .collect();
    assert_eq!(to_aca.len(), 12, "one RaAcaCertRequestSpdu per certificate");
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
        // The owner holds it, or (B's week 2, made by the weekly job before B asked) the
        // RA holds it in the owner's batch file.
        let suffix = format!("_{i:X}.zip");
        assert!(
            st.devices[&owner]
                .credentials
                .keys()
                .any(|&(ci, _)| ci == *i)
                || st
                    .ra
                    .batch_files(owner)
                    .iter()
                    .any(|(n, _)| n.ends_with(&suffix))
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

/// The weekly job is one population batch, shuffled across devices before the ACA sees it
/// (audit-prov PV.aca-cannot-group; [CAMP122] 5.2.7.6.4; [BRECHT] §V): A asks for week 1
/// and B's week 1 goes out in the same batch. Each RA→ACA message carries one request, the
/// order interleaves the two devices, and the RA files each answer into the right device's
/// batch file by the request's hash alone.
#[test]
fn the_weekly_job_sends_one_batch_shuffled_across_every_account() {
    // The week is the first past a 2-week ra-maxPreloadTime ([SMP] Table 3 floor), so the
    // weekly job runs at its boundary on the run's clock. (Week 1, inside the horizon, is
    // held since acceptance and generated off the run's clock, `generate_past_due`.)
    let mut p = ScmsParams::default().quick();
    p.pdu.keep_bytes = true;
    p.max_periods_ahead = 2;
    let mut run = run_with(p, &[DEVICE_A, DEVICE_B]);
    provisioned(&mut run, DEVICE_A, 0, 1, 20);
    provisioned(&mut run, DEVICE_B, 0, 1, 20);
    let mark = run.state.pdus.len();
    run.topup(DEVICE_A, 2, 20);
    run.run().expect("runs");
    let st = &run.state;
    let pca = st.nodes.pca.index();
    let device_of = |b: &[u8]| {
        let h = v2xw_core::hash::sha256(b);
        let rh = st.ra.handles.get(&h).expect("the RA keeps every handle");
        st.ra.requests[rh].device
    };
    let reqs: Vec<_> = st.pdus[mark..]
        .iter()
        .filter(|p| p.kind == SpduKind::RaAcaCertRequest && p.to == pca)
        .collect();
    assert_eq!(reqs.len(), 40, "week 2 of both accounts");
    let mut steps: Vec<_> = reqs.iter().map(|p| p.step).collect();
    steps.sort_unstable();
    steps.dedup();
    assert_eq!(steps.len(), 40, "one request per RA→ACA message");
    let order: Vec<NodeId> = reqs
        .iter()
        .map(|p| device_of(p.bytes.as_ref().unwrap().bytes()))
        .collect();
    assert_eq!(order.iter().filter(|d| **d == DEVICE_B).count(), 20);
    let switches = order.windows(2).filter(|w| w[0] != w[1]).count();
    assert!(
        switches >= 10,
        "the batch left grouped by device: {switches} switches"
    );
    // B never asked, yet its file for week 2 is whole, and A holds its week: the RA routed
    // all 40 answers by hash.
    for d in [DEVICE_B] {
        let files = st.ra.batch_files(d);
        let week1 = files
            .iter()
            .find(|(n, _)| n.ends_with("_2.zip"))
            .expect("week 2 filed");
        assert_eq!(week1.1.len(), 20, "{d:?}: {week1:?}");
    }
    assert_eq!(st.ra.unrouted_responses, 0);
    assert_eq!(run.state.devices[&DEVICE_A].credentials.len(), 40);
}

/// [PRIMER] p.6: the RA "adds one week every week". At each i-period boundary every
/// account gets the week that comes into `ra-maxPreloadTime`, whether or not its device
/// asks for it, in one population batch on the run's clock: its RA-ACA requests leave at
/// the boundary (within `ra-maxGenDelay`). Before, the engine generated only weeks a
/// device asked for, so with the 3-year horizon the RA never reached the LAs or the ACA
/// in the run (scms-fix3-ra RA.boundary-job). Fault (2026-10-27): `arm_boundary_jobs`
/// returning at once.
#[test]
fn every_account_gets_its_next_horizon_week_at_the_boundary_without_asking() {
    let mut run = bytes_run(&[DEVICE_A, DEVICE_B]);
    provisioned(&mut run, DEVICE_A, 614, 1, 1);
    provisioned(&mut run, DEVICE_B, 614, 1, 1);
    let far = 614 + 156;
    let before = count(&run, SpduKind::RaAcaCertRequest);
    // Half a week: no boundary yet, nothing added.
    run.run_until(WEEK_NS / 2).expect("runs");
    assert_eq!(count(&run, SpduKind::RaAcaCertRequest), before);
    run.run_until(WEEK_NS + 600 * 1_000_000_000).expect("runs");
    assert_eq!(
        run.state.ra.boundary_jobs, 1,
        "one week, one job, both accounts"
    );
    for d in [DEVICE_A, DEVICE_B] {
        assert!(run.state.ra.accounts[&d].generated.contains(&far), "{d:?}");
    }
    let added: Vec<u64> = run
        .state
        .pdus
        .iter()
        .filter(|p| p.kind == SpduKind::RaAcaCertRequest)
        .skip(before)
        .map(|p| p.at)
        .collect();
    assert_eq!(added.len(), 2, "one certificate (jmax 1) per account");
    assert!(
        added
            .iter()
            .all(|t| (WEEK_NS..WEEK_NS + 600 * 1_000_000_000).contains(t)),
        "at the boundary: {added:?}"
    );
    // Neither device asked: no top-off ran.
    assert_eq!(run.state.ra.pregen_deferred, 0);
}
