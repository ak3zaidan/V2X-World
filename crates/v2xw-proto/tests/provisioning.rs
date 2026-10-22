//! Butterfly provisioning yields certificates the device can actually use — and the
//! Registration Authority cannot link them to it.

mod common;

use common::{DEVICE_A, deployment, provisioned};
use v2xw_proto::stage::{FlowId, StageId};

#[test]
fn every_provisioned_credential_is_usable_by_the_device() {
    let mut run = deployment(1);
    provisioned(&mut run, DEVICE_A, 0, 3, 4);
    let dev = &run.state.devices[&DEVICE_A];
    assert_eq!(dev.credentials.len(), 12, "3 i-periods × 4 indices");
    for &(i, j) in dev.credentials.keys() {
        assert!(
            dev.credential_is_usable(i, j),
            "b'({i},{j}) = a + f1(ck,(i,j)) + c must match the certified public key"
        );
    }
}

/// The usability check is a P-256 base-point multiplication that the report path asks
/// for every pseudonym of every holding device at every backend step; it answers a repeat
/// from its memo (matrix.md, "Run cost": 35 min against under one), and only for the same
/// key pair. Faults: the memo lookup removed (the planted answer is ignored); the memo keyed
/// by (i, j) alone (the next test's swapped randomiser still reads usable).
#[test]
fn the_usability_check_answers_a_repeat_from_its_memo() {
    let mut run = deployment(1);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    let dev = &run.state.devices[&DEVICE_A];
    assert!(dev.credential_is_usable(0, 0));
    let (d, p, ok) = dev
        .usable_memo
        .entry((0, 0))
        .expect("the first answer is kept");
    assert!(ok);
    // Plant the opposite answer for the same key pair: a repeat must read the memo.
    dev.usable_memo.put((0, 0), d, p, false);
    assert!(
        !dev.credential_is_usable(0, 0),
        "a repeat for the same key pair is answered from the memo"
    );
    assert!(
        dev.credential_is_usable(0, 1),
        "another index is checked on its own"
    );
}

#[test]
fn a_credential_from_another_index_is_not_usable_for_this_one() {
    // The negative half: the identity is checked, not assumed. Swapping two credentials'
    // randomisers must break the match, or the check above would pass for anything.
    let mut run = deployment(1);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    let dev = run.state.devices.get_mut(&DEVICE_A).expect("device");
    assert!(dev.credential_is_usable(0, 0));
    // The real path (WP-B3a) issues implicit certificates: the per-certificate secret is
    // the ACA's private-key reconstruction value r (d = e·b_ij + r); the modelled path's
    // is the explicit randomiser c. Either way, the other index's value must not work.
    if dev.credentials[&(0, 0)].implicit.is_some() {
        let r0 = dev.credentials[&(0, 0)].implicit.as_ref().unwrap().r;
        let r1 = dev.credentials[&(0, 1)].implicit.as_ref().unwrap().r;
        assert_ne!(
            r0, r1,
            "the ACA's reconstruction value is fresh per certificate"
        );
        dev.credentials
            .get_mut(&(0, 0))
            .expect("cred")
            .implicit
            .as_mut()
            .unwrap()
            .r = r1;
    } else {
        let c0 = dev.credentials[&(0, 0)].c;
        let c1 = dev.credentials[&(0, 1)].c;
        assert_ne!(c0, c1, "the PCA draws a fresh randomiser per certificate");
        dev.credentials.get_mut(&(0, 0)).expect("cred").c = c1;
    }
    assert!(
        !dev.credential_is_usable(0, 0),
        "the wrong randomiser must not derive the certified key"
    );
}

#[test]
fn the_pca_issued_exactly_the_requested_certificates() {
    let mut run = deployment(1);
    provisioned(&mut run, DEVICE_A, 5, 2, 3);
    assert_eq!(run.state.pca.issued_count, 6);
    let dev = &run.state.devices[&DEVICE_A];
    for i in 5..7 {
        for j in 0..3 {
            assert!(dev.credentials.contains_key(&(i, j)), "({i},{j}) missing");
        }
    }
}

#[test]
fn provisioning_costs_real_queueing_time_and_real_bytes() {
    let mut run = deployment(1);
    provisioned(&mut run, DEVICE_A, 0, 1, 20);
    let flow_run = run.kernel.stages.runs_of(FlowId::Provisioning)[0];
    let requested = run
        .kernel
        .stages
        .at(flow_run, StageId::Requested)
        .expect("requested");
    let installed = run
        .kernel
        .stages
        .at(flow_run, StageId::Installed)
        .expect("installed");
    assert!(installed > requested, "provisioning takes simulated time");
    assert!(
        run.kernel.bytes_of_flow(FlowId::Provisioning) > 0,
        "and it puts bytes on links"
    );
    // Every entity on the path actually served requests.
    for node in [
        run.state.nodes.lop,
        run.state.nodes.ra,
        run.state.nodes.la1,
        run.state.nodes.la2,
        run.state.nodes.pca,
    ] {
        let q = run.kernel.queue(node).expect("hosted");
        assert!(q.served() > 0, "{node} served nothing");
    }
    // And the PCA was charged for the per-certificate work.
    let pca_signs = run
        .kernel
        .ops
        .get(&(run.state.nodes.pca, "primitive/ecdsa-p256-sha256", "sign"))
        .copied()
        .unwrap_or(0);
    // Two per issued certificate on the real path: the ACA signs the
    // AcaEeCertResponsePrivateSpdu and the AcaRaCertResponseSpdu around it (03 §3.2).
    assert_eq!(
        pca_signs,
        2 * 20,
        "two ACA signatures per issued certificate"
    );
}

#[test]
fn a_blocklisted_device_is_refused_a_top_up() {
    // The passive half of revocation, measured where it bites: the RA stops issuing.
    let mut run = deployment(1);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    run.state.ra.blocklist.insert(DEVICE_A);
    let before = run.state.pca.issued_count;
    run.topup(DEVICE_A, 1, 2);
    run.run().expect("runs");
    assert_eq!(run.state.ra.refused, 1);
    assert_eq!(
        run.state.pca.issued_count, before,
        "no certificate may be issued to a blocklisted enrolment certificate"
    );
}
