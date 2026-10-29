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
