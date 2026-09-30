//! Every SCMS entity in the loop with a device, including the ones that never touch a
//! pseudonym request: the DCM's trust bundle, the CRL Generator's signature, the RA's
//! enrolment checks and policy, the ECA's successor enrolment, the SCMS Manager's policy
//! decision and the LOP on every device-to-RA message.
//!
//! Each positive check is paired with the negative one the real system must also pass:
//! an expired enrolment certificate cannot get a top-up, a revoked enrolment cannot get a
//! top-up or a successor, a device without the trust bundle cannot accept a CRL, a CRL
//! altered after signing is refused.

mod common;

use common::{DEVICE_A, DEVICE_B, deployment, provisioned};
use v2xw_core::time::{Duration, NS_PER_S};
use v2xw_proto::net::Transport;
use v2xw_proto::scms::msg::{EnrolmentCert, Refusal};
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::stage::{FlowId, StageId};
use v2xw_proto::view::EdgeTracker;

const JMAX: u32 = 2;

fn revoke_a(run: &mut ScmsRun) {
    let (lv0, lv1) = {
        let d = &run.state.devices[&DEVICE_A];
        (d.credentials[&(0, 0)].lv, d.credentials[&(0, 1)].lv)
    };
    run.submit_report(DEVICE_B, 0, lv0);
    run.submit_report(DEVICE_B, 0, lv1);
    run.run().expect("reports run");
    run.investigate(0, 1, 0, JMAX).expect("two reports");
    run.run().expect("the investigation runs");
}

#[test]
fn the_dcm_bootstrap_installs_the_trust_list_chain_and_policy() {
    let mut run = deployment(1);
    run.enrol(DEVICE_A);
    run.run().expect("enrolment runs");
    let d = &run.state.devices[&DEVICE_A];
    assert_eq!(
        d.trust.ctl_sequence,
        Some(1),
        "the electors' list is installed"
    );
    assert_eq!(d.trust.anchors.len(), 3, "three elector anchors");
    for role in ["root", "ica", "pca", "ra", "eca", "crlg", "ma", "pg"] {
        assert!(
            d.trust.trusts_role(role),
            "{role} is not in the device's chain"
        );
    }
    assert_eq!(d.policy().map(|p| p.certs_per_period), Some(20));
    assert_eq!(d.trust.rejected, 0);
    assert!(
        d.trust.verifications >= 3,
        "the bundle was verified, not accepted"
    );
    assert_eq!(run.state.dcm.bundles, 1);
    let e = d.enrolment.expect("an enrolment certificate");
    assert_eq!(e.generation, 0);
    assert_eq!(
        e.valid_until - e.valid_from,
        run.state.params.enrolment_lifetime.as_nanos()
    );
}

#[test]
fn an_unbootstrapped_device_refuses_the_crl_and_a_bootstrapped_one_enforces_it() {
    let mut run = deployment(3);
    provisioned(&mut run, DEVICE_A, 0, 1, JMAX);
    revoke_a(&mut run);
    assert_eq!(run.state.crl_store.entries.len(), 1, "one entry published");
    assert!(
        run.state.crl_store.signature.is_some(),
        "and signed by the CRL Generator"
    );

    // DEVICE_C (1002) was never bootstrapped: no chain to the CRL Generator.
    let c = v2xw_core::ids::NodeId::new(1_002);
    let dist = run.distribute_crl(c);
    run.run().expect("runs");
    assert_eq!(run.state.devices[&c].crls_rejected, 1);
    assert!(run.state.devices[&c].crl.is_empty());
    assert!(run.kernel.stages.at(dist, StageId::Enforced).is_none());

    // The subject itself was bootstrapped and enforces it against its own certificates.
    let dist = run.distribute_crl(DEVICE_A);
    run.run().expect("runs");
    assert!(run.kernel.stages.at(dist, StageId::Enforced).is_some());
    assert_eq!(run.state.devices[&DEVICE_A].crls_rejected, 0);
    assert!(run.state.devices[&DEVICE_A].silenced);
}

#[test]
fn a_crl_altered_after_signing_is_refused() {
    let mut run = deployment(2);
    provisioned(&mut run, DEVICE_A, 0, 1, JMAX);
    revoke_a(&mut run);
    // An entry added between the Generator and the Store: the signature no longer covers
    // the list, and the device discards it whole.
    let mut forged = run.state.crl_store.entries[0];
    forged.i += 1;
    run.state.crl_store.entries.push(forged);
    run.distribute_crl(DEVICE_A);
    run.run().expect("runs");
    let d = &run.state.devices[&DEVICE_A];
    assert_eq!(d.crls_rejected, 1);
    assert!(!d.silenced, "a list that does not verify revokes nothing");
}

#[test]
fn provisioning_waits_for_enrolment_and_an_unenrolled_device_is_refused() {
    let mut run = deployment(2);
    // Enrolment and provisioning started together: the request waits for the certificate.
    run.enrol(DEVICE_A);
    let p = run.provision(DEVICE_A, 0, 1, JMAX);
    run.run().expect("runs");
    assert!(run.kernel.stages.at(p, StageId::Installed).is_some());
    assert_eq!(run.state.devices[&DEVICE_A].credentials.len(), 2);

    // A device that never enrolled has no certificate to sign with; the RA refuses it.
    let p = run.provision(DEVICE_B, 0, 1, JMAX);
    run.run().expect("runs");
    assert!(run.kernel.stages.at(p, StageId::Installed).is_none());
    assert_eq!(run.refusal_of(DEVICE_B), Some(Refusal::UnknownEnrolment));
    assert_eq!(run.state.ra.refused_enrolment, 1);
    assert!(run.state.devices[&DEVICE_B].credentials.is_empty());
}

#[test]
fn an_expired_enrolment_cannot_get_a_top_up_until_it_is_renewed() {
    let mut run = deployment(1);
    provisioned(&mut run, DEVICE_A, 0, 1, JMAX);
    let now = run.kernel.now();
    let issued = run.state.devices[&DEVICE_A].enrolment.expect("enrolled");

    // The certificate the vehicle holds has run out.
    run.set_enrolment(
        DEVICE_A,
        EnrolmentCert {
            valid_until: now,
            ..issued
        },
    );
    let t = run.topup(DEVICE_A, 1, JMAX);
    run.run().expect("runs");
    assert!(run.kernel.stages.at(t, StageId::Installed).is_none());
    assert_eq!(run.refusal_of(DEVICE_A), Some(Refusal::EnrolmentExpired));
    assert!(
        !run.state.devices[&DEVICE_A]
            .credentials
            .contains_key(&(1, 0))
    );

    // An expired certificate cannot sign its own successor request either: the device
    // must go back to its bootstrap channel.
    let r = run.reenrol_at(DEVICE_A, run.kernel.now());
    run.run().expect("runs");
    assert!(run.kernel.stages.at(r, StageId::Installed).is_none());
    assert_eq!(run.state.eca.refused, 1);

    // Renewed while still valid, the successor is issued and the top-up goes through.
    let now = run.kernel.now();
    run.set_enrolment(
        DEVICE_A,
        EnrolmentCert {
            valid_until: Duration::from_secs(3_600).after(now),
            ..issued
        },
    );
    let r = run.reenrol_at(DEVICE_A, now);
    run.run().expect("runs");
    assert_eq!(
        run.kernel.stages.stages(r),
        vec![StageId::Requested, StageId::Certified, StageId::Installed]
    );
    let renewed = run.state.devices[&DEVICE_A].enrolment.expect("renewed");
    assert_eq!(renewed.generation, 1);
    assert!(renewed.valid_until > now + 3_600 * NS_PER_S);
    let t = run.topup(DEVICE_A, 1, JMAX);
    run.run().expect("runs");
    assert!(run.kernel.stages.at(t, StageId::Installed).is_some());
    assert!(
        run.state.devices[&DEVICE_A]
            .credentials
            .contains_key(&(1, 1))
    );
}

#[test]
fn a_revoked_enrolment_gets_neither_a_top_up_nor_a_successor() {
    let mut run = deployment(2);
    provisioned(&mut run, DEVICE_A, 0, 1, JMAX);
    revoke_a(&mut run);
    assert!(run.state.ra.blocklist.contains(&DEVICE_A));
    assert!(
        run.state.eca.blocklist.contains(&DEVICE_A),
        "the RA told the ECA"
    );

    let t = run.topup(DEVICE_A, 1, JMAX);
    run.run().expect("runs");
    assert!(run.kernel.stages.at(t, StageId::Installed).is_none());
    assert_eq!(run.refusal_of(DEVICE_A), Some(Refusal::Blocklisted));
    assert_eq!(run.state.ra.refused, 1);

    let r = run.reenrol_at(DEVICE_A, run.kernel.now());
    run.run().expect("runs");
    assert!(run.kernel.stages.at(r, StageId::Installed).is_none());
    assert_eq!(run.refusal_of(DEVICE_A), Some(Refusal::Blocklisted));
}

#[test]
fn a_policy_decision_reaches_devices_on_their_next_connection_and_the_ra_enforces_it() {
    let mut run = deployment(1);
    provisioned(&mut run, DEVICE_A, 0, 1, JMAX);
    assert_eq!(
        run.state.devices[&DEVICE_A]
            .trust
            .lpf
            .as_ref()
            .map(|l| l.version),
        Some(1)
    );

    let mut policy = run.state.gov.gpf.policy;
    policy.certs_per_period = 1;
    let flow = run.decide_policy_at(policy, run.kernel.now());
    run.run().expect("runs");
    assert_eq!(
        run.kernel.stages.stages(flow),
        vec![StageId::Decision, StageId::Issued, StageId::Published]
    );
    assert_eq!(run.state.gov.gpf.version, 2);
    assert_eq!(run.state.gov.lpf.version, 2);
    // Not yet at the device: it has not connected since.
    assert_eq!(
        run.state.devices[&DEVICE_A]
            .trust
            .lpf
            .as_ref()
            .map(|l| l.version),
        Some(1)
    );

    // The next top-up asks for two certificates; the RA grants the policy's one, and the
    // device installs the new policy file on the same connection.
    let t = run.topup(DEVICE_A, 1, JMAX);
    run.run().expect("runs");
    assert!(run.kernel.stages.at(t, StageId::Installed).is_some());
    let d = &run.state.devices[&DEVICE_A];
    assert_eq!(d.trust.lpf.as_ref().map(|l| l.version), Some(2));
    assert_eq!(d.policy().map(|p| p.certs_per_period), Some(1));
    assert!(d.credentials.contains_key(&(1, 0)));
    assert!(
        !d.credentials.contains_key(&(1, 1)),
        "clipped to the policy"
    );
    assert_eq!(run.state.ra.clipped, 1);
}

#[test]
fn every_device_to_ra_message_goes_through_the_lop() {
    let mut run = deployment(2);
    provisioned(&mut run, DEVICE_A, 0, 2, JMAX);
    revoke_a(&mut run);
    let (ra, lop) = (run.state.nodes.ra, run.state.nodes.lop);
    let devices = [DEVICE_A, DEVICE_B];
    for s in &run.kernel.steps {
        let direct =
            (devices.contains(&s.from) && s.to == ra) || (s.from == ra && devices.contains(&s.to));
        assert!(
            !direct,
            "{} went between a device and the RA directly",
            s.step
        );
    }
    let via_lop = run
        .kernel
        .steps
        .iter()
        .filter(|s| s.from == lop && s.to == ra)
        .count();
    assert!(
        via_lop >= 5,
        "request, files, polls and reports relayed: {via_lop}"
    );
    assert!(run.state.lop.upstream as usize == via_lop);
    assert!(run.state.lop.downstream > 0);
    // The LOP only ever relays: it signs and verifies nothing.
    let lop_ops: u64 = run
        .kernel
        .ops
        .iter()
        .filter(|((n, _, _), _)| *n == lop)
        .map(|(_, c)| c)
        .sum();
    assert_eq!(lop_ops, 0);
}

#[test]
fn the_backend_view_shows_every_entity_and_the_flows_between_them() {
    let mut run = deployment(2);
    provisioned(&mut run, DEVICE_A, 0, 1, JMAX);
    revoke_a(&mut run);
    run.distribute_crl(DEVICE_A);
    run.run().expect("runs");
    let mut tracker = EdgeTracker::default();
    let view = run.backend_view(run.kernel.now(), &mut tracker);
    let ids: Vec<&str> = view.entities.iter().map(|e| e.id.as_str()).collect();
    for id in [
        "manager",
        "pg",
        "electors",
        "root",
        "ica",
        "dcm",
        "eca",
        "lop",
        "ra",
        "la1",
        "la2",
        "pca",
        "ma",
        "crlg",
        "crl-store",
        "crl-broadcast",
        "ee",
    ] {
        assert!(ids.contains(&id), "{id} missing from {ids:?}");
    }
    let get = |id: &str| view.entities.iter().find(|e| e.id == id).expect("present");
    assert_eq!(get("pca").state["certs_issued"], 2);
    assert_eq!(get("la1").state["seeds_released"], 1);
    assert_eq!(get("eca").state["enrolment_certs_issued"], 1);
    assert!(get("ra").traffic.received > 0);
    assert!(
        get("ra")
            .ops
            .get("ecdsa-p256-sha256 sign")
            .copied()
            .unwrap_or(0)
            > 0
    );
    assert!(!get("root").online && get("root").state["certs_issued"].as_u64() >= Some(5));
    // Edges: devices talk to the LOP, the LOP to the RA, the RA to both LAs and the PCA.
    let edge = |a: &str, b: &str| view.edges.iter().any(|e| e.from == a && e.to == b);
    for (a, b) in [
        ("ee", "lop"),
        ("lop", "ra"),
        ("ra", "lop"),
        ("lop", "ee"),
        ("ra", "la1"),
        ("ra", "la2"),
        ("ra", "pca"),
        ("ma", "pca"),
        ("ma", "crlg"),
        ("crl-store", "ee"),
        ("dcm", "ee"),
    ] {
        assert!(edge(a, b), "no {a} -> {b} edge");
    }
    assert!(!edge("ee", "ra"), "a device never reaches the RA directly");
    assert!(view.flows.get("provisioning").copied().unwrap_or(0) >= 1);
    assert!(!view.recent.is_empty());
    // The view is JSON-serialisable, which is how it reaches a page.
    let json = serde_json::to_value(&view).expect("serialises");
    assert!(json["entities"].as_array().is_some_and(|a| a.len() == 17));
    // Cellular Uu is the transport on the device legs, the backend network behind.
    let t = view
        .edges
        .iter()
        .find(|e| e.from == "lop" && e.to == "ra")
        .map(|e| e.transport.clone());
    assert_eq!(t.as_deref(), Some(Transport::BackendNet.as_str()));
    let _ = FlowId::Reenrolment;
}
