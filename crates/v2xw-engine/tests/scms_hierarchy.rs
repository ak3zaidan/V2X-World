//! Deeper CA hierarchies (docs/design/scms-real/08-settings-ux-and-gaps.md §8, piece C3a):
//! a provider with `n` intermediate CA levels issues every component under `ica-n`, each
//! level's `certIssuePermissions` make the chain exact, a pseudonym's chain is
//! `n + 3` long and validates under the 1609.2 chain rules, more than five levels are
//! refused at the receiver's chain limit, and one level is today's hierarchy byte for
//! byte.

use std::collections::BTreeSet;

use v2xw_core::ids::NodeId;
use v2xw_msg::sec_types::Certificate;
use v2xw_proto::ScmsParams;
use v2xw_proto::scms::deployment::{Deployment, MAX_ICA_LEVELS, Preset};
use v2xw_proto::scms::governance::{DeviceTrust, ica_level};
use v2xw_proto::scms::run::ScmsRun;
use v2xw_sec::chain::{ChainPolicy, MAX_CHAIN_LENGTH, validate};

const S: u64 = 1_000_000_000;

/// A certificate's `id.name` (`ica-2.provider-a`), if it has one.
fn name_of(c: &Certificate) -> Option<String> {
    match &c.0.to_be_signed.id {
        v2xw_msg::sec_types::ieee1609_dot2::CertificateId::name(h) => Some(h.0.clone()),
        _ => None,
    }
}

fn id8(c: &Certificate) -> [u8; 8] {
    v2xw_sec::hashedid::id8_bytes(&v2xw_sec::cert::cert_id8(c).expect("hashes"))
}

/// The path from `leaf` to a self-signed certificate through `pool`, by issuer digest.
fn path(leaf: &Certificate, pool: &[Certificate]) -> Vec<Certificate> {
    use v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier;
    let mut out = vec![leaf.clone()];
    loop {
        let d = match &out.last().expect("non-empty").0.issuer {
            IssuerIdentifier::sha256AndDigest(d) | IssuerIdentifier::sha384AndDigest(d) => {
                let mut b = [0u8; 8];
                b.copy_from_slice(&d.0[..]);
                b
            }
            _ => return out,
        };
        match pool.iter().find(|c| id8(c) == d) {
            Some(c) if out.len() < 2 * MAX_CHAIN_LENGTH => out.push(c.clone()),
            _ => return out,
        }
    }
}

/// The home run of the US deployment with provider-a at `a` levels and provider-b at `b`.
fn run_with(a: u32, b: u32) -> ScmsRun {
    let mut d = Deployment::preset(Preset::UsProduction2025);
    d.providers[0].ica_levels = a;
    d.providers[1].ica_levels = b;
    ScmsRun::new_deployed(ScmsParams::default().quick(), 0, d).expect("builds")
}

/// Every chain-file certificate of the run, and its roots, as a node's pool.
fn pool_and_anchors(run: &ScmsRun) -> (Vec<Certificate>, BTreeSet<[u8; 32]>) {
    let st = &run.state;
    let pki = st.pki.as_ref().expect("the PDU layer is on");
    let mut pkis = vec![&**pki];
    pkis.extend(st.peer_pki.iter().map(|(_, p)| &**p));
    let mut pool = Vec::new();
    let mut anchors = BTreeSet::new();
    for p in pkis {
        anchors.insert(v2xw_sec::cert::hashed_id32(&p.root.cert).expect("hashes"));
        pool.push(p.root.cert.clone());
        pool.extend(p.all().into_iter().map(|a| a.cert.clone()));
        pool.extend(p.extra.iter().map(|a| a.cert.clone()));
    }
    (pool, anchors)
}

/// Accept test 1: with `ica_levels: 3` a pseudonym certificate's chain is root, ICA,
/// ica-2, ica-3, ACA, pseudonym: 6 long, and it validates under the 1609.2 chain rules
/// with no error; a device installing the local chain file (both providers, provider-b
/// at 2 levels) validates every certificate in it. Fault: every level carries
/// `{minChainLength 2, chainLengthRange 0}`.
#[test]
fn a_three_level_hierarchy_validates_end_to_end() {
    let mut run = run_with(3, 2);
    let device = NodeId::new(5_000_701);
    run.preload(device, 0, 1, 2).expect("provisions");
    let c = run.state.devices[&device]
        .credentials
        .values()
        .next()
        .expect("a pseudonym");
    let pseudonym = c
        .implicit
        .as_ref()
        .expect("a real certificate")
        .certificate
        .clone();
    let (pool, anchors) = pool_and_anchors(&run);
    let chain = path(&pseudonym, &pool);
    let names: Vec<Option<String>> = chain.iter().map(name_of).collect();
    assert_eq!(
        chain.len(),
        6,
        "root, 3 ICA levels, ACA, pseudonym: {names:?}"
    );
    let start = pseudonym.0.to_be_signed.validity_period.start.0.0;
    let got = validate(&chain, &ChainPolicy::us_device(anchors, start + 3_600));
    assert!(got.is_ok(), "{:?}", got.err());

    let g = &run.state.gov;
    let mut t = DeviceTrust {
        anchors: g.anchors(),
        ..DeviceTrust::default()
    };
    t.install_ctl(&g.ctl).expect("the genesis CTL installs");
    let n = t.install_chain(&g.lccf, S);
    assert!(n.is_ok(), "the chain file: {n:?}");
    for (label, levels) in [("provider-a", 3), ("provider-b", 2)] {
        for k in 2..=levels {
            let role = v2xw_proto::scms::governance::ica_level_role(k);
            let want = format!("{role}.{label}");
            let in_file = g
                .lccf
                .certs
                .iter()
                .any(|c| name_of(c).as_deref() == Some(want.as_str()));
            assert!(in_file, "{role}.{label} is not in the chain file");
        }
    }
}

/// Accept test 3: six levels are refused with the chain limit in the message, by the
/// deployment and by the loader.
#[test]
fn six_levels_are_refused_at_the_chain_limit() {
    assert_eq!(MAX_ICA_LEVELS, 5);
    let mut d = Deployment::preset(Preset::UsProduction2025);
    d.providers[0].ica_levels = 6;
    let e = d.validate().expect_err("six levels cannot run").to_string();
    assert!(
        e.contains("ica_levels") && e.contains("at most 8 certificates"),
        "{e}"
    );
    d.providers[0].ica_levels = 0;
    assert!(d.validate().is_err());
    d.providers[0].ica_levels = MAX_ICA_LEVELS;
    d.validate()
        .expect("five levels fit: root + 5 + ACA + pseudonym = 8");
}

/// FNV-1a over `bytes`: a pin on the chain file's bytes.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// The local chain file of `main`'s default US run (both providers' roots, ICAs and
/// components, signed): its length and FNV-1a, pinned on this tree after
/// `phase2::the_phase_2_path_reproduces_its_regression_digest` showed the governance
/// bytes unchanged from `main`.
const MAIN_LCCF: (usize, u64) = (6_878, 1_100_630_834_571_523_792);

/// Accept test 5: one level is today's hierarchy. The default run's local chain file is
/// `main`'s byte for byte (pinned), and `ica_levels: 1` set explicitly builds the same
/// certificates and chain files. Fault: the level-1 ICA is keyed and named like a deeper
/// level.
#[test]
fn one_level_is_todays_hierarchy_byte_for_byte() {
    let default = ScmsRun::new_deployed(
        ScmsParams::default().quick(),
        0,
        Deployment::preset(Preset::UsProduction2025),
    )
    .expect("builds");
    let lccf = &default.state.gov.lccf.spdu;
    assert_eq!(
        (lccf.len(), fnv(lccf)),
        MAIN_LCCF,
        "the default chain file moved"
    );
    let one = run_with(1, 1);
    assert_eq!(default.state.gov.lccf.spdu, one.state.gov.lccf.spdu);
    assert_eq!(default.state.gov.gccf.spdu, one.state.gov.gccf.spdu);
    let (a, _) = pool_and_anchors(&default);
    let (b, _) = pool_and_anchors(&one);
    assert_eq!(a, b);
    assert!(
        one.state
            .gov
            .lccf
            .certs
            .iter()
            .filter_map(name_of)
            .all(|n| !n.starts_with("ica-")),
        "a deeper level in the one-level chain file"
    );
    assert_eq!(ica_level("ica"), Some(1));
    assert_eq!(ica_level("ica-6"), None);
}

/// The scenario `connected-intersections` on an SCMS backend with `deployment`, for two
/// seconds (the setup of `tests/scms_deployment.rs`).
fn scenario(deployment: serde_json::Value) -> v2xw_engine::Scenario {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = v2xw_engine::Scenario::load(root.join("scenarios/connected-intersections.yaml"))
        .expect("loads");
    s.time.duration_s = 2.0;
    s.threats.attackers.clear();
    s.security.protocol = Some(
        serde_json::from_value(serde_json::json!({"id": "protocol/scms/camp"})).expect("choice"),
    );
    s.security.scms = Some(
        serde_json::from_value(serde_json::json!({"deployment": deployment})).expect("a block"),
    );
    s
}

/// Accept test 2: a provider with 3 ICA levels makes the run not the US system, and the
/// manifest names the level count; the scenario loads and every node validates both
/// providers' ACAs through the chain-file certificates it installs. Fault: `is_us`
/// ignores `ica_levels`.
#[test]
fn a_deeper_hierarchy_runs_and_is_flagged_not_us() {
    let s = scenario(serde_json::json!({"providers": [
        {"name": "provider-b", "ica_levels": 3}
    ]}));
    s.validate().expect("ica_levels 3 loads");
    let d = v2xw_engine::phase2::deployment_of(&s, true).expect("builds");
    assert_eq!(d.providers[1].ica_levels, 3);
    assert!(!d.is_us(), "three ICA levels are not the US system");
    let base =
        v2xw_engine::phase2::deployment_of(&scenario(serde_json::json!({})), true).expect("builds");
    assert!(base.is_us());
    // The settings window's banner (`scenario.validate`'s `us_system`) names it too.
    let us = v2xw_engine::phase2::us_system(&s).expect("an SCMS run has a verdict");
    assert!(!us.is_us);
    assert!(
        us.reasons
            .iter()
            .any(|r| r == "provider-b has 3 ICA levels"),
        "{:?}",
        us.reasons
    );

    let engine = v2xw_engine::Engine::build(s, "").expect("builds");
    let w = &engine.manifest().warnings;
    assert!(
        w.iter().any(|l| l.contains("NOT the US system"))
            && w.iter().any(|l| l.contains("provider-b has 3 ICA levels")),
        "{w:#?}"
    );
    let p2 = engine.phase2().expect("an SCMS run");
    let (roots, cas) = p2.trust_bundle();
    let anchors: BTreeSet<[u8; 32]> = roots
        .iter()
        .map(|r| v2xw_sec::cert::hashed_id32(r).expect("hashes"))
        .collect();
    let mut pool: Vec<Certificate> = cas.iter().map(|c| (**c).clone()).collect();
    pool.extend(roots.iter().map(|c| (**c).clone()));
    let acas = engine.credential_issuers();
    assert_eq!(acas.len(), 2);
    let lens: Vec<usize> = acas
        .iter()
        .map(|aca| {
            let chain = path(aca, &pool);
            let start = aca.0.to_be_signed.validity_period.start.0.0;
            let got = validate(
                &chain,
                &ChainPolicy::us_device(anchors.clone(), start + 3_600),
            );
            assert!(got.is_ok(), "{:?}", got.err());
            chain.len()
        })
        .collect();
    assert_eq!(
        lens,
        vec![3, 5],
        "provider-a: root, ICA, ACA; provider-b: two more levels"
    );
}

/// 08 §8, piece C3b, failure-node names: `actors.backend.failures[].node` naming an ICA
/// level (`ica-2`, `provider-b/ica-3`), the ICA or the root is refused as an offline CA,
/// naming why (it takes no request during a run, so its failure would change nothing)
/// and the online roles a failure can name. 08 §8 first proposed accepting `ica-2` … with
/// no effect; a setting that silently does nothing is refused instead. Fault: the
/// offline-CA check removed (the name then falls to the generic unknown-role message).
#[test]
fn a_failure_of_an_offline_ca_is_refused_by_name() {
    for name in ["ica-2", "provider-b/ica-3", "ica", "root"] {
        let mut s = scenario(serde_json::json!({"providers": [
            {"name": "provider-b", "ica_levels": 3}
        ]}));
        s.actors.backend.failures = vec![
            serde_json::from_value(serde_json::json!({"node": name, "mode": "offline"}))
                .expect("a failure"),
        ];
        let e = s
            .validate()
            .expect_err("an offline CA cannot fail")
            .to_string();
        assert!(
            e.contains("is an offline CA") && e.contains("crlg"),
            "{name}: {e}"
        );
    }
}

/// The signature verifications a provider-a device pays to learn provider-b's chain by
/// P2PCD when provider-b runs `levels` ICA levels, and how many certificates it learned.
/// The device holds the home provider's chain file and both CTL roots (the
/// `chain_file: home-provider` case); the response is IEEE 1609.2's `caCerts`, the
/// requested ACA first and each certificate issued by the next, up to the root's child.
fn cost_to_learn_peer_chain(levels: u32) -> (usize, usize) {
    use std::sync::Arc;
    use v2xw_node::secure::{CryptoMode, NodeSecurity, PSID_SAFETY};
    use v2xw_sec::primitive::PrimitiveId;
    let run = run_with(1, levels);
    let st = &run.state;
    let home = st.pki.as_ref().expect("the PDU layer is on");
    let (_, peer) = st
        .peer_pki
        .iter()
        .find(|(l, _)| l == "provider-b")
        .expect("provider-b");
    let mut sec = NodeSecurity::new(
        v2xw_core::time::WallClock::new(1_767_225_600),
        CryptoMode::Real,
        PSID_SAFETY,
    );
    let roots = [
        Arc::new(home.root.cert.clone()),
        Arc::new(peer.root.cert.clone()),
    ];
    let cas: Vec<Arc<Certificate>> = home
        .all()
        .into_iter()
        .chain(home.extra.iter())
        .map(|a| Arc::new(a.cert.clone()))
        .collect();
    sec.install_trust(&roots, &cas);
    let _ = sec.take_validation_ops();
    // caCerts: the ACA, ica-n … ica-2, the ICA.
    let mut response = vec![peer.aca.cert.clone()];
    for k in (2..=levels).rev() {
        let want = format!(
            "{}.provider-b",
            v2xw_proto::scms::governance::ica_level_role(k)
        );
        let c = peer
            .extra
            .iter()
            .find(|a| name_of(&a.cert).as_deref() == Some(want.as_str()))
            .unwrap_or_else(|| panic!("{want} is not in provider-b's PKI"));
        response.push(c.cert.clone());
    }
    response.push(peer.ica.cert.clone());
    let learned = sec.p2pcd_learn(&response);
    let verifies = sec
        .take_validation_ops()
        .iter()
        .filter(|(p, _, _)| *p == PrimitiveId::ECDSA_P256_SHA256 || *p == PrimitiveId::ECDSA_P384)
        .count();
    (learned, verifies)
}

/// 08 §8, piece C3b: a receiver's first verification of a foreign chain pays one
/// signature verification per certificate it did not hold, so each ICA level adds
/// exactly one. Learning provider-b's chain costs the ACA and the ICA at one level (2)
/// and n + 1 at n levels; the root it chains to was verified when the CTL installed it,
/// and nothing already verified is verified again. Fault: `RxTrust::build_path` charges
/// every link of every path (each learned certificate re-verifying everything above it,
/// root included, gives 2 + 3 + … + (n + 2)).
#[test]
fn each_ica_level_costs_a_foreign_receiver_one_verification() {
    for levels in [1u32, 3, 5] {
        let (learned, verifies) = cost_to_learn_peer_chain(levels);
        let want = levels as usize + 1;
        assert_eq!(
            learned, want,
            "{levels} levels: the ACA and every level learned"
        );
        assert_eq!(
            verifies, want,
            "{levels} levels: one verification per certificate learned"
        );
    }
}

/// What the P2PCD run of accept test 4 did, per provider (deployment order).
#[derive(Debug, Default)]
struct LearnRun {
    /// Per provider: Σ `learned` of each node of that provider that learned anything.
    learned_by_node: [Vec<u64>; 2],
    /// Per provider: Σ `requests_sent`, Σ `learn_unanchored`.
    requests: [u64; 2],
    unanchored: [u64; 2],
    /// Learning responses put on the air.
    on_air: u64,
    /// SPaT, MAP and BSM receptions a provider-a vehicle verified from a provider-b
    /// sender.
    a_verified_b: u64,
}

/// `connected-intersections` for 12 s with two providers half and half (the setup of
/// `tests/p2pcd_air.rs`), provider-b at `levels` ICA levels, under
/// `chain_file: home-provider`: a device holds only its own provider's CA certificates
/// and learns the other's by P2PCD.
fn learn_run(levels: u32) -> LearnRun {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = v2xw_engine::Scenario::load(root.join("scenarios/connected-intersections.yaml"))
        .expect("loads");
    s.time.duration_s = 12.0;
    s.threats.attackers.clear();
    s.messages.sets = vec!["bsm".into(), "spat".into(), "map".into()];
    s.security.protocol = Some(
        serde_json::from_value(serde_json::json!({"id": "protocol/scms/camp"})).expect("choice"),
    );
    s.security.scms = Some(
        serde_json::from_value(serde_json::json!({
            "deployment": {
                "preset": "us-production-2025",
                "providers": [
                    {"name": "provider-a", "fleet_share": 0.5},
                    {"name": "provider-b", "fleet_share": 0.5, "ica_levels": levels}
                ]
            },
            "chain_file": "home-provider",
        }))
        .expect("an scms section"),
    );
    let mut engine = v2xw_engine::Engine::build(s, "").expect("builds");
    let mut rec = v2xw_engine::MemoryRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    let seed = engine.scenario().seed;
    let p2 = engine.phase2().expect("an SCMS run");
    let provider_of = |node: u64| p2.deployment().provider_of(seed, node).min(1);
    let mut learned: std::collections::BTreeMap<u64, u64> = std::collections::BTreeMap::new();
    let mut out = LearnRun {
        on_air: report.p2pcd_responses_on_air,
        ..LearnRun::default()
    };
    for (_, r) in rec.records() {
        if !matches!(r.channel, "node.p2pcd" | "node.rx") {
            continue;
        }
        let v: serde_json::Value = serde_json::from_slice(&r.json).expect("json");
        // `node.p2pcd` names its node; `node.rx` its receiver `rx` and sender `tx`.
        let Some(node) = v["node"].as_u64().or(v["rx"].as_u64()) else {
            continue;
        };
        let k = provider_of(node);
        match r.channel {
            "node.p2pcd" => {
                *learned.entry(node).or_insert(0) += v["learned"].as_u64().unwrap_or(0);
                out.requests[k] += v["requests_sent"].as_u64().unwrap_or(0);
                out.unanchored[k] += v["learn_unanchored"].as_u64().unwrap_or(0);
            }
            _ => {
                if k == 0
                    && v["tx"].as_u64().is_some_and(|f| provider_of(f) == 1)
                    && matches!(v["msg_type"].as_str(), Some("spat" | "map" | "bsm"))
                    && v["outcome"] == "delivered"
                    && v["verification"] == "verified"
                {
                    out.a_verified_b += 1;
                }
            }
        }
    }
    for (node, n) in learned {
        if n > 0 {
            out.learned_by_node[provider_of(node)].push(n);
        }
    }
    eprintln!("ica_levels {levels}: {out:?}");
    out
}

/// Accept test 4 (08 §8, piece C3b): under `chain_file: home-provider`, a provider-a
/// device's first verification of provider-b's certificates, with provider-b at 3 ICA
/// levels, learns n + 1 = 4 CA certificates (the ACA and every level) and then verifies
/// provider-b's traffic; provider-b's devices learn provider-a's one-level chain (2).
///
/// How the four arrive: IEEE 1609.2 out-of-band P2PCD (the US flavour) answers one
/// `p2pcdLearningRequest` with one `Ieee1609dot2Peer2PeerPDU` whose `caCerts` holds the
/// requested certificate and each issuer up to the root's child (`v2xw_sec::p2pcd`), so
/// the n + 1 certificates come in one exchange, not one exchange each as 08 §8 first
/// read it. Fault: the chain file omits the middle level (`Governance::with_levels`
/// skips ica-2): provider-b's devices then cannot serve a chain that reaches the root,
/// provider-a's devices learn nothing anchored and verify none of provider-b's traffic.
#[test]
fn a_foreign_deeper_chain_is_learned_whole_by_p2pcd() {
    let r = learn_run(3);
    assert!(r.on_air > 0, "no learning response was put on the air");
    assert!(
        !r.learned_by_node[0].is_empty(),
        "no provider-a device learned provider-b's chain: {r:?}"
    );
    assert!(
        r.learned_by_node[0].iter().all(|n| *n == 4),
        "a provider-a device learns the ACA and the 3 levels, once: {r:?}"
    );
    assert!(
        r.learned_by_node[1].iter().all(|n| *n == 2),
        "a provider-b device learns provider-a's ACA and ICA, once: {r:?}"
    );
    assert_eq!(
        r.unanchored,
        [0, 0],
        "every learned certificate reached a root"
    );
    assert!(
        r.a_verified_b > 0,
        "after learning, no provider-a device verified provider-b's traffic: {r:?}"
    );
}
