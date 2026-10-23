//! R-CCSIG (docs/design/scms-real/testing/final-bench.md §3): the NDSS 2024 Partially-Hybrid
//! certificate cycle (`fragmenter/cert-cycle-partial-hybrid`) under a hybrid
//! `security.signature`.
//!
//! The design [Twardokus, Bindel, Rahbari and McCarthy, NDSS 2024, §III, Fig. 4] keeps ECDSA
//! on every BSM ("verification cost and message size are unchanged"; frames 2-5 of a cycle
//! are 204 B) and carries only the hybrid certificate, in the first α SPDUs of each
//! five-SPDU cycle. Before the fix the engine also put the hybrid signature on every
//! message: under hybrid ML-DSA-44 (2,420 B of post-quantum signature) every BSM exceeded
//! the 1,400 B WSMP MTU and the stress run transmitted nothing (50,180 MTU refusals), and
//! under Falcon-512 every BSM was 1,045 B instead of about 380 B.
//!
//! These tests run that combination on the Phase 1 grid and check what the design says:
//! the messages fit and are ECDSA-sized and ECDSA-verified, the certificate travels in
//! α = 3 fragments of the ML-DSA-44 hybrid certificate, a receiver pays the certificate's
//! post-quantum signature once per certificate it learns, and the validator refuses the
//! combinations the design cannot run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use v2xw_engine::scenario::ModelChoice;
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};
use v2xw_metrics::EventLedger;
use v2xw_metrics::channels::NodeTxView;

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// The fragmentation tests' small dense grid, with an OBU that prices the post-quantum
/// operations and the certificate cycle under `signature`.
fn grid(signature: &str) -> Scenario {
    let mut s = Scenario::load(scenarios().join("phase1-grid.yaml")).expect("loads");
    s.time.duration_s = 6.0;
    s.world.buildings.enabled = false;
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = serde_json::json!(4);
        params["rows"] = serde_json::json!(6);
    }
    s.actors.vehicles.demand.rate_veh_per_h = Some(20_000.0);
    s.metrics = vec!["all".to_string()];
    s.nodes.default_obu = "obu/generic-automotive-soc-no-hsm".to_string();
    s.net.fragmenter = Some(ModelChoice::new(v2xw_net::FRAGMENTER_CERT_CYCLE_ID));
    s.security.signature = signature.to_string();
    s
}

fn errors(s: &Scenario) -> String {
    v2xw_engine::scenario::validate::validate(s)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

fn run(s: Scenario) -> (RunReport, MemoryRecorder) {
    let e = errors(&s);
    assert!(e.is_empty(), "the test scenario must validate: {e}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    let report = engine.run(&mut recorder).expect("runs");
    (report, recorder)
}

fn ledger(recorder: &MemoryRecorder) -> EventLedger {
    let mut ledger = EventLedger::new();
    for (_, r) in recorder.records() {
        ledger.ingest(r);
    }
    ledger
}

/// The hybrid ML-DSA-44 certificate the cycle carries: the 858 B Falcon-512 anchor
/// (NDSS Fig. 4) plus the difference in certificate growth between the two hybrids
/// (`v2xw_engine::frag::FragPlan::for_signature`): ML-DSA-44 key 1,312 B and signature
/// 2,420 B [FIPS 204 Table 2] against Falcon-512's 897 B and 666 B.
const MLDSA44_HYBRID_CERT: u64 = 858 + (1_312 + 2_420) - (897 + 666);

#[test]
fn hybrid_mldsa44_messages_are_ecdsa_and_only_the_certificate_is_hybrid() {
    let (report, recorder) = run(grid("hybrid-mldsa44-ecdsa-p256"));
    let l = ledger(&recorder);
    let bsm: Vec<&NodeTxView> = l
        .tx
        .iter()
        .filter(|t| t.msg_type.as_deref() == Some("bsm"))
        .collect();
    println!(
        "BSMs {}, MTU refusals {}, certificate groups {}, PQ certificate checks {}",
        bsm.len(),
        report.net_mtu_refusals,
        l.reassembly.len(),
        report.receiver_validation.cert_pq_verifications
    );
    // The PQHMC failure: every message refused at the MTU, nothing on the air.
    assert!(bsm.len() > 100, "no BSM was transmitted");
    assert_eq!(report.net_mtu_refusals, 0, "a message was refused at the MTU");

    // Per sender, five SPDUs a cycle: α = 3 carry a ~1,009 B certificate fragment, the
    // last two carry none and are plain ECDSA BSMs. A hybrid ML-DSA-44 signature alone
    // would make every one of them over 2,400 B.
    let mut by_node: BTreeMap<u32, Vec<u64>> = BTreeMap::new();
    for t in &bsm {
        by_node
            .entry(t.node.index())
            .or_default()
            .push(t.bytes_on_wire);
    }
    let mut cycles = 0;
    for sizes in by_node.values() {
        for c in sizes.chunks_exact(5) {
            let plain = c[3].max(c[4]);
            assert!(
                plain < 450,
                "a BSM without a fragment is ECDSA-sized (about 200-330 B): {c:?}"
            );
            assert!(
                c[..3].iter().all(|&b| b >= plain + 900 && b < 1_500),
                "the first three SPDUs carry a third of the hybrid certificate: {c:?}"
            );
            cycles += 1;
        }
    }
    assert!(cycles > 10, "too few whole cycles: {cycles}");

    // The certificate is the hybrid ML-DSA-44 one, in three fragments.
    assert!(!l.reassembly.is_empty(), "no certificate group was followed");
    for g in &l.reassembly {
        assert_eq!(g.kind, "certificate");
        assert_eq!(g.fragments, 3);
        assert_eq!(g.bytes, MLDSA44_HYBRID_CERT);
    }

    // Every message is verified with ECDSA; the certificate's post-quantum signature is
    // checked once per certificate a receiver learns.
    let ops: BTreeMap<String, u64> = recorder
        .records()
        .iter()
        .filter(|(_, r)| r.channel == "node.verify")
        .filter_map(|(_, r)| serde_json::from_slice::<serde_json::Value>(&r.json).ok())
        .filter_map(|v| v["primitive"].as_str().map(str::to_string))
        .fold(BTreeMap::new(), |mut m, p| {
            *m.entry(p).or_insert(0) += 1;
            m
        });
    println!("verification operations {ops:?}");
    assert!(!ops.is_empty(), "nothing was verified");
    assert_eq!(
        ops.keys().collect::<Vec<_>>(),
        vec!["ecdsa-p256-verify"],
        "a message was verified with the hybrid operation"
    );
    assert!(
        report.receiver_validation.cert_pq_verifications > 0,
        "no receiver paid for a certificate's post-quantum signature"
    );
    assert!(
        report.receiver_validation.cert_pq_verifications < ops.values().sum::<u64>(),
        "the certificate check is once per certificate, not once per message"
    );
}

#[test]
fn the_validator_refuses_what_the_partially_hybrid_cycle_cannot_run() {
    // A pure post-quantum scheme has no classical half to sign a message with.
    let e = errors(&grid("ml-dsa-44"));
    assert!(
        e.contains("net.fragmenter") && e.contains("purely post-quantum"),
        "{e}"
    );
    // A certificate the cycle cannot carry in τ = 5 fragments under the MTU.
    let mut s = grid("hybrid-falcon512-ecdsa-p256");
    s.net.fragmenter = Some(ModelChoice {
        id: v2xw_net::FRAGMENTER_CERT_CYCLE_ID.to_string(),
        params: serde_json::json!({"hybrid_cert_bytes": 9_000}),
    });
    let e = errors(&s);
    assert!(e.contains("cannot travel"), "{e}");
    // The design's own combinations validate.
    for sig in [
        "hybrid-falcon512-ecdsa-p256",
        "hybrid-mldsa44-ecdsa-p256",
        "ecdsa-p256",
    ] {
        let e = errors(&grid(sig));
        assert!(e.is_empty(), "{sig}: {e}");
    }
}
