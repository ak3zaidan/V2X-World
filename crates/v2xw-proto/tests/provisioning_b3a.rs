//! WP-B3a in the run: the shuffle presets release at the count or at the age, whichever
//! comes first; a THEA-style preload is one request for three years; the NYC taxi
//! profile installs 60 real implicit certificates a week; every installed certificate is
//! the ACA's implicit one with a key the device derived.

mod common;

use common::{DEVICE_A, DEVICE_B};
use v2xw_core::time::Duration;
use v2xw_proto::scms::params::{ScmsParams, ShufflePreset};
use v2xw_proto::scms::pdu::SpduKind;
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::stage::{FlowId, StageId};

fn run_with(p: ScmsParams, devices: &[v2xw_core::ids::NodeId]) -> ScmsRun {
    let mut run = ScmsRun::new(p).expect("certificates encode");
    for d in devices {
        run.add_device(*d);
        run.enrol(*d);
    }
    run.run().expect("enrolment runs");
    run
}

fn count(run: &ScmsRun, k: SpduKind) -> usize {
    run.state.pdus.iter().filter(|p| p.kind == k).count()
}

#[test]
fn the_presets_carry_the_cited_numbers() {
    let mut p = ScmsParams::default();
    assert_eq!(
        (p.shuffle_threshold, p.shuffle_window),
        (1_000, Duration::from_secs(2 * 86_400))
    );
    ShufflePreset::Brecht.apply(&mut p);
    assert_eq!(
        (p.shuffle_threshold, p.shuffle_window),
        (10_000, Duration::from_secs(86_400))
    );
    ShufflePreset::Camp122.apply(&mut p);
    assert_eq!(p.shuffle_threshold, 1_000);
    assert_eq!(
        (p.download_poll_interval, p.max_download_polls),
        (Duration::from_secs(300), 100),
        "download-minWait 5 min, download-maxReqs 100"
    );
}

#[test]
fn the_shuffle_releases_at_the_age_when_the_count_is_not_reached() {
    // One request, CAMP 1.2.2 window of 2 days: nothing reaches the ACA before the age.
    let mut run = run_with(ScmsParams::default(), &[DEVICE_A]);
    let t0 = run.kernel.now();
    let f = run.provision(DEVICE_A, 0, 1, 1);
    run.run().expect("runs");
    let pre = run.kernel.stages.at(f, StageId::PreLinkageReady).unwrap();
    let shuffled = run.kernel.stages.at(f, StageId::Shuffled).unwrap();
    let age = Duration::from_secs(2 * 86_400).as_nanos();
    assert!(
        (age..age + Duration::from_secs(1).as_nanos()).contains(&(shuffled - pre)),
        "released at the age (plus the RA's service time): {}",
        shuffled - pre
    );
    // The device polled at nextDlTime and installed a usable implicit certificate.
    let dev = &run.state.devices[&DEVICE_A];
    assert_eq!(dev.credentials.len(), 1);
    assert!(dev.credentials[&(0, 0)].implicit.is_some() && dev.credential_is_usable(0, 0));
    assert!(run.kernel.stages.at(f, StageId::Installed).unwrap() > t0 + 2 * 86_400 * 1_000_000_000);
    assert!(
        run.state.pdu_rejections.is_empty(),
        "{:?}",
        run.state.pdu_rejections
    );
}

#[test]
fn the_shuffle_releases_at_the_count_before_the_age() {
    let mut p = ScmsParams::default();
    p.shuffle_threshold = 2;
    let mut run = run_with(p, &[DEVICE_A, DEVICE_B]);
    let fa = run.provision(DEVICE_A, 0, 1, 1);
    let fb = run.provision(DEVICE_B, 0, 1, 1);
    run.run().expect("runs");
    for f in [fa, fb] {
        let pre = run.kernel.stages.at(f, StageId::PreLinkageReady).unwrap();
        let shuffled = run.kernel.stages.at(f, StageId::Shuffled).unwrap();
        assert!(
            shuffled - pre < Duration::from_secs(60).as_nanos(),
            "released by the count, not the 2-day age"
        );
    }
    for d in [DEVICE_A, DEVICE_B] {
        assert!(run.state.devices[&d].credential_is_usable(0, 0));
    }
}

#[test]
fn a_thea_style_three_year_preload_is_one_request_for_156_periods() {
    // THEA downloaded 3 years at once ([CVP-LOG] p.16); 156 i-periods is the RA's maximum
    // ([PRIMER] p.7, ra-maxPreloadTime 3 years). One request, one ack; a request for more
    // is clipped. jmax 1 keeps the test fast; 156 × 20 = 3,120 is the cited batch.
    let mut run = run_with(ScmsParams::default().quick(), &[DEVICE_A]);
    run.provision(DEVICE_A, 614, 157, 1);
    run.run().expect("runs");
    assert_eq!(
        run.state.ra.clipped, 1,
        "157 periods clipped to the 3-year maximum"
    );
    assert_eq!(
        count(&run, SpduKind::EeRaCertRequest),
        2,
        "one request, two hops"
    );
    let dev = &run.state.devices[&DEVICE_A];
    assert_eq!(dev.credentials.len(), 156);
    assert_eq!(
        run.state.ra.batch_files(DEVICE_A).len(),
        156,
        "one zip per i-period"
    );
    assert!(
        dev.credentials
            .keys()
            .all(|&(i, j)| dev.credential_is_usable(i, j))
    );
    assert_eq!(count(&run, SpduKind::AcaEeCertResponsePrivate), 156);
    assert_eq!(
        count(&run, SpduKind::EeRaDownloadRequest),
        2 * 156,
        "a signed request per file"
    );
}

#[test]
fn the_nyc_taxi_profile_installs_60_certificates_a_week() {
    // NYC: 60 certificates a week for taxis, loaded one week ahead ([CVP-P2] items 21, 24).
    let mut p = ScmsParams::default().quick();
    p.certs_per_period = 60;
    let mut run = run_with(p, &[DEVICE_A]);
    run.provision(DEVICE_A, 614, 1, 60);
    run.run().expect("runs");
    for week in 1..=2u32 {
        run.topup(DEVICE_A, 614 + week, 60);
        run.run().expect("runs");
        let dev = &run.state.devices[&DEVICE_A];
        assert_eq!(dev.credentials.len(), 60 * (week as usize + 1));
    }
    let dev = &run.state.devices[&DEVICE_A];
    assert!(
        dev.credentials
            .keys()
            .all(|&(i, j)| dev.credential_is_usable(i, j))
    );
    let lens: std::collections::BTreeSet<usize> = dev
        .credentials
        .values()
        .map(|c| {
            v2xw_sec::cert::encode(&c.implicit.as_ref().unwrap().certificate)
                .unwrap()
                .len()
        })
        .collect();
    assert_eq!(
        lens.len(),
        1,
        "every pseudonym certificate is the same size: {lens:?}"
    );
    assert_eq!(run.state.pca.issued_count, 180);
    let _ = FlowId::Topup;
}
