//! Threshold protocols in the credential system: round structure, message counts, and the
//! behaviour with slow, offline and malicious committee members — run over the real SCMS
//! flows, not in isolation.

mod common;

use common::{DEVICE_A, DEVICE_B, provisioned};
use v2xw_core::ids::NodeId;
use v2xw_core::time::Duration;
use v2xw_proto::fault::{Fault, FaultKind};
use v2xw_proto::scms::params::{ScmsParams, ThresholdDeployment};
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::spec::Centrality;
use v2xw_proto::threshold::session::CommitteeStats;
use v2xw_proto::threshold::{
    self, Committee, ProtocolParams, THRESHOLD_IDS, ThresholdOp, ThresholdProtocol, frost,
};

const PERIODS: u32 = 2;
const JMAX: u32 = 2;

fn committee(n: u16, t: u16) -> Committee {
    Committee {
        n,
        t,
        members: (0..n).map(|k| NodeId::new(500 + u32::from(k))).collect(),
        coordinator: Some(NodeId::new(499)),
    }
}

fn deployment(t: Option<ThresholdDeployment>) -> ScmsRun {
    let params = ScmsParams {
        threshold: t,
        ..ScmsParams::default().quick()
    };
    let mut run = ScmsRun::new(params).expect("certificates encode");
    run.add_device(DEVICE_A);
    run.add_device(DEVICE_B);
    run
}

fn stats(run: &ScmsRun, role: &str) -> CommitteeStats {
    run.state
        .committees
        .stats
        .get(role)
        .copied()
        .unwrap_or_default()
}

fn forever(node: NodeId, kind: FaultKind) -> Fault {
    Fault {
        node,
        kind,
        from: 0,
        until: u64::MAX,
    }
}

fn held(run: &ScmsRun) -> usize {
    run.state.devices[&DEVICE_A].credentials.len()
}

// ------------------------------------------------------------------------------------
// Round structure, from the specifications
// ------------------------------------------------------------------------------------

#[test]
fn a_committee_appears_as_a_distributed_role() {
    let c = committee(7, 4);
    assert_eq!(c.centrality(), Centrality::Distributed { n: 7, t: 4 });
    assert_eq!(c.quorum(), 5);
}

#[test]
fn every_registered_protocol_has_rounds_setup_and_a_knowledge_statement() {
    let c = committee(5, 2);
    for id in THRESHOLD_IDS {
        let p = threshold::build(id, &ProtocolParams::default()).expect("registered");
        assert!(p.sign_round_trips() >= 1, "{id}");
        assert!(p.sign_bytes(3, 1, 93) > 0, "{id}");
        assert!(p.setup_bytes(ThresholdOp::Dkg, &c) > 0, "{id}: key generation costs bytes");
        assert!(p.setup_rounds(ThresholdOp::Dkg, &c) > 0, "{id}");
        assert!(!p.knowledge().is_empty(), "{id}");
        assert!(p.identifiable_abort(), "{id}: every shipped protocol identifies a cheater");
    }
}

#[test]
fn frost_is_two_rounds_cggmp21_four_and_linkage_release_one() {
    let rounds = |id: &str, pre: bool| {
        threshold::build(
            id,
            &ProtocolParams {
                preprocessed: pre,
                ..ProtocolParams::default()
            },
        )
        .expect("registered")
        .sign_round_trips()
    };
    assert_eq!(rounds("frost-p256", false), 2, "RFC 9591 §5: commit, sign");
    assert_eq!(rounds("frost-p256", true), 1, "RFC 9591 §5.1: preprocessed commitments");
    assert_eq!(rounds("cggmp21-ecdsa", false), 4, "CGGMP21: 3 presign + 1 online");
    assert_eq!(rounds("cggmp21-ecdsa", true), 1, "presignature pool");
    assert_eq!(rounds("shamir-feldman", false), 1);
    assert_eq!(rounds("umbrella", false), 2, "FROST inside");
}

#[test]
fn signing_costs_scale_with_the_quorum_and_key_generation_with_the_committee() {
    let f = frost::Frost::default();
    assert!(f.sign_bytes(5, 1, 93) > f.sign_bytes(2, 1, 93));
    let few = f.setup_bytes(ThresholdOp::Dkg, &committee(3, 1));
    let many = f.setup_bytes(ThresholdOp::Dkg, &committee(9, 4));
    assert!(many > few, "a broadcast round costs n − 1 messages");
}

#[test]
fn a_round_that_expects_several_attempts_costs_several_attempts() {
    // The interface's answer to CELI25's 4–5 attempts per threshold ML-DSA signature.
    let f = frost::Frost::default();
    let c = committee(4, 2);
    let mut rounds = f.dkg_rounds(&c);
    let once = rounds.iter().map(|r| r.bytes(c.n)).sum::<u64>();
    for r in &mut rounds {
        r.attempts = 5;
    }
    let five = rounds.iter().map(|r| r.bytes(c.n)).sum::<u64>();
    assert_eq!(five, 5 * once);
}

// ------------------------------------------------------------------------------------
// A committee PCA in the SCMS
// ------------------------------------------------------------------------------------

/// FROST 3-of-5 on the PCA only, asking exactly a quorum unless told otherwise.
fn pca_only(fanout_all: bool) -> ThresholdDeployment {
    ThresholdDeployment {
        crl: false,
        linkage: None,
        fanout_all,
        ..ThresholdDeployment::default()
    }
}

#[test]
fn a_threshold_pca_issues_every_batch_through_two_rounds_with_the_counted_messages() {
    let mut run = deployment(Some(pca_only(false)));
    provisioned(&mut run, DEVICE_A, 0, PERIODS, JMAX);
    assert_eq!(held(&run), (PERIODS * JMAX) as usize);
    let s = stats(&run, "pca");
    // One session per i-period batch; two rounds each; a quorum of 3 asked in each round,
    // each answering once: 2 rounds × (3 requests + 3 responses).
    assert_eq!(s.sessions, u64::from(PERIODS));
    assert_eq!(s.completed, u64::from(PERIODS));
    assert_eq!(s.rounds, 2 * u64::from(PERIODS));
    assert_eq!(s.messages, u64::from(PERIODS) * 2 * (3 + 3));
    assert_eq!((s.restarts, s.timeouts, s.failed), (0, 0, 0));
    // No single server signed: the PCA charged no certificate signature of its own.
    let pca = run.state.nodes.pca;
    let pca_signs = run
        .kernel
        .ops
        .get(&(pca, "primitive/ecdsa-p256-sha256", "sign"))
        .copied()
        .unwrap_or(0);
    assert_eq!(pca_signs, 0);
    // The committee's latency is in the provisioning: at least two member round trips.
    let t = ThresholdDeployment::default().member_link_latency;
    assert!(s.mean_latency_ns() >= 4 * t.as_nanos(), "{s:?}");
}

#[test]
fn asking_every_member_costs_more_bytes_and_shrugs_off_a_slow_one() {
    let mut quorum_only = deployment(Some(pca_only(false)));
    provisioned(&mut quorum_only, DEVICE_A, 0, PERIODS, JMAX);
    let mut all = deployment(Some(pca_only(true)));
    // Member 0 is slow by a second: asked only as part of a fixed quorum it would set the
    // pace; asked with everyone, the first three answers carry the round.
    let slow = all.committee_members("pca")[0];
    all.set_faults(vec![forever(
        slow,
        FaultKind::Slow {
            extra: Duration::from_secs(1),
        },
    )]);
    provisioned(&mut all, DEVICE_A, 0, PERIODS, JMAX);
    assert_eq!(held(&all), (PERIODS * JMAX) as usize);
    let (q, a) = (stats(&quorum_only, "pca"), stats(&all, "pca"));
    assert!(a.bytes > q.bytes, "five asked in round one: {} vs {}", a.bytes, q.bytes);
    assert!(
        a.latency_ns_max < Duration::from_secs(1).as_nanos(),
        "the slow member did not set the pace: {a:?}"
    );
}

#[test]
fn an_offline_member_is_timed_out_excluded_and_replaced() {
    let mut run = deployment(Some(pca_only(false)));
    let down = run.committee_members("pca")[0];
    run.set_faults(vec![forever(down, FaultKind::Offline)]);
    provisioned(&mut run, DEVICE_A, 0, PERIODS, JMAX);
    assert_eq!(held(&run), (PERIODS * JMAX) as usize, "4 of 5 still make a quorum of 3");
    let s = stats(&run, "pca");
    assert_eq!(s.completed, u64::from(PERIODS));
    assert_eq!(s.timeouts, u64::from(PERIODS), "one timeout per session");
    assert!(s.exclusions >= u64::from(PERIODS));
    // The timeout is in the latency.
    let timeout = ThresholdDeployment::default().round_timeout;
    assert!(s.mean_latency_ns() >= timeout.as_nanos(), "{s:?}");
    assert!(run.state.faults.dropped > 0);
}

#[test]
fn too_few_live_members_fail_the_session_and_nothing_is_issued() {
    let t = ThresholdDeployment {
        max_attempts: 2,
        retry_after: Duration::from_secs(1),
        ..pca_only(true)
    };
    let mut run = deployment(Some(t));
    let m = run.committee_members("pca");
    run.set_faults(m[..3].iter().map(|n| forever(*n, FaultKind::Offline)).collect());
    provisioned(&mut run, DEVICE_A, 0, PERIODS, JMAX);
    assert_eq!(held(&run), 0, "2 of 5 cannot make a quorum of 3");
    let s = stats(&run, "pca");
    assert_eq!(s.completed, 0);
    assert!(s.failed >= 1);
    assert!(run.state.threshold_failures.get("pca").copied().unwrap_or(0) >= 1);
}

#[test]
fn a_member_that_comes_back_lets_a_waiting_session_finish() {
    // Ten attempts twenty seconds apart outlast a two-minute outage.
    let t = ThresholdDeployment {
        retry_after: Duration::from_secs(20),
        ..pca_only(true)
    };
    let mut run = deployment(Some(t));
    let m = run.committee_members("pca");
    // Three of five are down for the first two minutes of the run.
    let two_minutes = Duration::from_secs(120).as_nanos();
    run.set_faults(
        m[..3]
            .iter()
            .map(|n| Fault {
                node: *n,
                kind: FaultKind::Offline,
                from: 0,
                until: two_minutes,
            })
            .collect(),
    );
    provisioned(&mut run, DEVICE_A, 0, PERIODS, JMAX);
    assert_eq!(held(&run), (PERIODS * JMAX) as usize);
    let s = stats(&run, "pca");
    assert_eq!(s.failed, 0);
    assert!(s.restarts > 0);
}

#[test]
fn a_slow_member_the_quorum_depends_on_is_waited_for_not_excluded() {
    // Two of five down and a third slower than the round timeout: excluding the slow one
    // would leave no quorum, so the coordinator waits for it.
    let mut run = deployment(Some(pca_only(true)));
    let m = run.committee_members("pca");
    run.set_faults(vec![
        forever(m[0], FaultKind::Offline),
        forever(m[1], FaultKind::Offline),
        forever(
            m[2],
            FaultKind::Slow {
                extra: Duration::from_millis(800),
            },
        ),
    ]);
    provisioned(&mut run, DEVICE_A, 0, PERIODS, JMAX);
    assert_eq!(held(&run), (PERIODS * JMAX) as usize);
    let s = stats(&run, "pca");
    assert_eq!(s.failed, 0, "{s:?}");
    assert!(s.timeouts > 0);
    // Every round waited on the slow member: at least two of its 800 ms per session.
    assert!(s.mean_latency_ns() >= Duration::from_millis(1_600).as_nanos(), "{s:?}");
}

#[test]
fn a_malicious_member_is_caught_by_its_share_excluded_and_the_session_restarts() {
    let mut run = deployment(Some(pca_only(true)));
    let liar = run.committee_members("pca")[0];
    run.set_faults(vec![forever(liar, FaultKind::Malicious)]);
    provisioned(&mut run, DEVICE_A, 0, PERIODS, JMAX);
    assert_eq!(held(&run), (PERIODS * JMAX) as usize);
    let s = stats(&run, "pca");
    // Caught once: identifiable abort names the cheater, and the committee leaves it out of
    // every later session rather than letting it spoil each one.
    assert_eq!(s.invalid_shares, 1, "{s:?}");
    assert!(s.restarts >= 1);
    assert_eq!(s.failed, 0);
    assert_eq!(
        run.state.committees.caught.values().map(|c| c.len()).sum::<usize>(),
        1
    );
}

#[test]
fn threshold_ecdsa_is_far_slower_than_frost_unless_presigned() {
    let latency = |protocol: &'static str, preprocessed: bool| {
        let mut run = deployment(Some(ThresholdDeployment {
            protocol,
            protocol_params: ProtocolParams {
                preprocessed,
                ..ProtocolParams::default()
            },
            ..pca_only(false)
        }));
        provisioned(&mut run, DEVICE_A, 0, 1, JMAX);
        stats(&run, "pca").mean_latency_ns()
    };
    let frost = latency("frost-p256", false);
    let ecdsa = latency("cggmp21-ecdsa", false);
    let pooled = latency("cggmp21-ecdsa", true);
    // CGGMP21's measured presigning (≈ 2.6 s per presignature per party) dominates.
    assert!(ecdsa > 10 * frost, "cggmp21 {ecdsa} vs frost {frost}");
    assert!(pooled < ecdsa / 10, "a presignature pool leaves only the online round");
}

#[test]
fn an_umbrella_authority_signs_one_umbrella_per_k_leaves() {
    let mut per_cert = deployment(Some(pca_only(false)));
    provisioned(&mut per_cert, DEVICE_A, 0, PERIODS, JMAX);
    let mut umbrella = deployment(Some(ThresholdDeployment {
        protocol: "umbrella",
        protocol_params: ProtocolParams {
            umbrella_leaves: JMAX,
            ..ProtocolParams::default()
        },
        ..pca_only(false)
    }));
    provisioned(&mut umbrella, DEVICE_A, 0, PERIODS, JMAX);
    assert_eq!(held(&umbrella), (PERIODS * JMAX) as usize);
    let (p, u) = (stats(&per_cert, "pca"), stats(&umbrella, "pca"));
    assert_eq!(p.sessions, u.sessions);
    assert!(u.bytes < p.bytes, "1 signature per umbrella of {JMAX} leaves: {u:?} vs {p:?}");
}

// ------------------------------------------------------------------------------------
// Two-of-two against t-of-n revocation
// ------------------------------------------------------------------------------------

/// Provisions DEVICE_A, then (after `fault_after`) applies `faults`, reports, investigates.
fn revoke(t: Option<ThresholdDeployment>, faults: impl Fn(&ScmsRun, u64) -> Vec<Fault>) -> ScmsRun {
    let mut run = deployment(t);
    provisioned(&mut run, DEVICE_A, 0, 1, JMAX);
    let now = run.kernel.now();
    let f = faults(&run, now);
    run.set_faults(f);
    let (lv0, lv1) = {
        let d = &run.state.devices[&DEVICE_A];
        (d.credentials[&(0, 0)].lv, d.credentials[&(0, 1)].lv)
    };
    run.submit_report(DEVICE_B, 0, lv0);
    run.submit_report(DEVICE_B, 0, lv1);
    run.run().expect("the reports run");
    run.investigate(0, 1, 0, JMAX).expect("two reports");
    run.run().expect("the investigation runs");
    run
}

fn after(node: NodeId, from: u64) -> Fault {
    Fault {
        node,
        kind: FaultKind::Offline,
        from,
        until: u64::MAX,
    }
}

#[test]
fn two_of_two_linkage_cannot_revoke_with_one_authority_down() {
    let healthy = revoke(None, |_, _| Vec::new());
    assert_eq!(healthy.state.crlg.entries.len(), 1);
    let blocked = revoke(None, |r, now| vec![after(r.state.nodes.la2, now)]);
    assert!(
        blocked.state.crlg.entries.is_empty(),
        "with LA2 down the MA can neither link the reports nor get ls₂(i)"
    );
}

#[test]
fn t_of_n_linkage_revokes_with_members_down_and_names_the_same_device() {
    let t = ThresholdDeployment {
        pca: false,
        crl: false,
        linkage: Some((5, 3)),
        ..ThresholdDeployment::default()
    };
    let healthy = revoke(None, |_, _| Vec::new());
    let committee = revoke(Some(t), |r, now| {
        let m = r.committee_members("linkage");
        // Two of five down: a quorum of three still releases both seeds. (The LAs still
        // answer the "same device?" question of a two-report investigation, so they stay
        // up here; the release is what the committee replaces.)
        vec![after(m[0], now), after(m[1], now)]
    });
    assert_eq!(committee.state.crlg.entries.len(), 1);
    // The released seeds are the ones two-of-two would have released.
    let (a, b) = (&healthy.state.crlg.entries[0], &committee.state.crlg.entries[0]);
    assert_eq!((a.ls1_i, a.ls2_i, a.i), (b.ls1_i, b.ls2_i, b.i));
    let s = stats(&committee, "linkage");
    assert_eq!(s.completed, 1);
    assert_eq!(s.items, 2, "two seeds per revocation");
    // Three down of five is one too many.
    let failed = revoke(
        Some(ThresholdDeployment {
            max_attempts: 2,
            retry_after: Duration::from_secs(1),
            ..t
        }),
        |r, now| {
            let m = r.committee_members("linkage");
            m[..3].iter().map(|n| after(*n, now)).collect()
        },
    );
    assert!(failed.state.crlg.entries.is_empty());
}

#[test]
fn a_threshold_crl_generator_publishes_only_after_its_committee_signs() {
    let t = ThresholdDeployment {
        pca: false,
        linkage: None,
        ..ThresholdDeployment::default()
    };
    let run = revoke(Some(t), |_, _| Vec::new());
    assert_eq!(run.state.crl_store.entries.len(), 1);
    let s = stats(&run, "crl");
    assert_eq!(s.completed, 1);
    let crlg = run.state.nodes.crlg;
    let single_signs = run
        .kernel
        .ops
        .get(&(crlg, "primitive/ecdsa-p256-sha256", "sign"))
        .copied()
        .unwrap_or(0);
    assert_eq!(single_signs, 0, "no single server signed the list");
}

#[test]
fn committees_are_deterministic() {
    let once = || {
        let mut run = deployment(Some(ThresholdDeployment::default()));
        let liar = run.committee_members("pca")[1];
        run.set_faults(vec![forever(liar, FaultKind::Malicious)]);
        provisioned(&mut run, DEVICE_A, 0, PERIODS, JMAX);
        (
            run.kernel.steps.len(),
            run.kernel.now(),
            stats(&run, "pca"),
        )
    };
    assert_eq!(once(), once());
}
