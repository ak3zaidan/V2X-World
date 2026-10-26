//! A deployment of several providers on one CTL (WP-D1; gap rows 41–42).

use super::*;
use crate::scms::deployment::{Deployment, Preset};

const S: SimTime = NS_PER_S;
const DAY: SimTime = 86_400 * NS_PER_S;

fn us() -> (Governance, Deployment) {
    let d = Deployment::preset(Preset::UsProduction2025);
    let p = ScmsParams {
        electors: d.electors,
        elector_quorum: d.quorum,
        ..ScmsParams::default()
    };
    let config = GovernanceConfig {
        peers: PeerProvider::of_deployment(&d),
        ..GovernanceConfig::default()
    };
    let g = Governance::ceremony_with(&p, &d.nodes(0).online, &RngRegistry::new(7), 0, config);
    (g, d)
}

fn device(g: &Governance) -> DeviceTrust {
    let mut t = DeviceTrust {
        anchors: g.anchors(),
        ..DeviceTrust::default()
    };
    t.install_ctl(&g.ctl).expect("genesis CTL");
    t.install_chain(&g.lccf, S)
        .expect("the local chain verifies");
    t
}

/// Gap row 41: the CTL lists exactly the preset's roots, one per provider, and each
/// provider's components are certified under its own root and ICA.
#[test]
fn the_ctl_lists_exactly_one_root_per_provider() {
    let (g, d) = us();
    assert_eq!(g.ctl.roots.len(), d.providers.len());
    let ids: Vec<[u8; 32]> = d
        .providers
        .iter()
        .map(|p| g.root_of(&p.name).expect("every provider has a root"))
        .collect();
    assert_eq!(ids, g.ctl.roots);
    assert_ne!(ids[0], ids[1]);
    for role in ["ica", "eca", "pca", "ra", "la1", "la2", "ma", "crlg", "pg"] {
        let a = g.cert(role).expect("home");
        let b = g.peer_cert("provider-b", role).expect("peer");
        assert_ne!(
            cert_hashed_id8(a).unwrap(),
            cert_hashed_id8(b).unwrap(),
            "{role}"
        );
    }
    // Each provider's DCM is its own and certified under its own ICA: it signs the
    // enrolment responses it relays to that provider's devices, so with one certificate per
    // component (audit-gov §4 item 1) the peer's DCM is in its chain set too.
    let dcm_a = g.cert("dcm").expect("home DCM");
    let dcm_b = g.peer_cert("provider-b", "dcm").expect("peer DCM");
    assert_ne!(
        cert_hashed_id8(dcm_a).unwrap(),
        cert_hashed_id8(dcm_b).unwrap()
    );
    // Provider-b's ICA is issued by provider-b's root, not provider-a's.
    let ica_b = g.peer_cert("provider-b", "ica").unwrap();
    assert_eq!(
        issuer_id8(dcm_b),
        Some(cert_hashed_id8(ica_b).unwrap()),
        "the peer DCM is issued by the peer ICA"
    );
    assert_eq!(
        issuer_id8(ica_b),
        Some(cert_hashed_id8(g.peer_cert("provider-b", "root").unwrap()).unwrap())
    );
    // The genesis CTL is signed by all five shared electors.
    assert_eq!(g.ctl.signers.len(), 5);
}

/// Gap row 42: a device of provider A validates provider B's ACA, RA and CRL signer from
/// the chain file to B's root on the CTL, and verifies a signature by B's CRL Generator.
#[test]
fn a_device_on_one_provider_trusts_the_other_through_the_ctl() {
    let (g, _) = us();
    let mut t = device(&g);
    let root_b = g.root_of("provider-b").unwrap();
    for role in ["ica", "pca", "ra", "ma", "crlg"] {
        let id = cert_hashed_id8(g.peer_cert("provider-b", role).unwrap()).unwrap();
        assert_eq!(
            t.anchored_at.get(&id),
            Some(&root_b),
            "{role} anchored at B's root"
        );
    }
    let digest = [9u8; 32];
    let (crlg_b, sig) = g.peer_sign_as("provider-b", "crlg", &digest).unwrap();
    assert!(t.verify_signed(crlg_b, &digest, &sig));
}

/// Gap row 42, the converse: once the EAC removes provider B's root, a device that
/// installs the new CTL discards B's authorities and stops verifying B's signatures,
/// while provider A's stay trusted; the re-issued chain file installs without B.
#[test]
fn removing_one_providers_root_untrusts_only_that_provider() {
    let (mut g, _) = us();
    let mut t = device(&g);
    let digest = [5u8; 32];
    let (crlg_a, sig_a) = g.sign_as("crlg", &digest).unwrap();
    let (crlg_b, sig_b) = g.peer_sign_as("provider-b", "crlg", &digest).unwrap();
    let root_b = g.root_of("provider-b").unwrap();
    let at = g.remove_root(root_b, 10 * DAY).expect("approved");
    assert_eq!(g.publish_due(at), 1);
    assert_eq!(g.ctl.roots, vec![g.root_of("provider-a").unwrap()]);
    t.install_ctl(&g.ctl).expect("the removal CTL installs");
    assert!(
        t.purged >= 9,
        "B's ICA and components were discarded: {}",
        t.purged
    );
    assert!(!t.verify_signed(crlg_b, &digest, &sig_b));
    assert!(t.verify_signed(crlg_a, &digest, &sig_a));
    let pca_b = cert_hashed_id8(g.peer_cert("provider-b", "pca").unwrap()).unwrap();
    assert!(!t.chain.contains_key(&pca_b));
    g.sign_global(g.gpf.policy);
    g.sign_local();
    assert!(
        g.lccf
            .certs
            .iter()
            .all(|c| cert_hashed_id8(c).unwrap() != pca_b),
        "the re-issued chain file leaves B out"
    );
    t.install_chain(&g.lccf, at)
        .expect("A's chain still installs");
}

/// A one-provider deployment (no peers) is today's ceremony, certificate for certificate.
#[test]
fn no_peers_is_todays_ceremony() {
    let p = ScmsParams::default();
    let a = Governance::ceremony(&p, &ScmsNodes::default(), &RngRegistry::new(7), 0);
    let b = Governance::ceremony_with(
        &p,
        &ScmsNodes::default(),
        &RngRegistry::new(7),
        0,
        GovernanceConfig {
            peers: PeerProvider::of_deployment(&Deployment::preset(Preset::CampReference)),
            ..GovernanceConfig::default()
        },
    );
    assert_eq!(a.ctl.spdu, b.ctl.spdu);
    assert_eq!(a.lccf.spdu, b.lccf.spdu);
}

/// Writes the two-provider genesis CTL and chain files for the independent oracle
/// (`oracle.py real`) when `V2XW_GOV_DUMP` names a directory; a no-op otherwise.
#[test]
fn dump_two_provider_artefacts_for_the_oracle() {
    let Ok(dir) = std::env::var("V2XW_GOV_DUMP") else {
        return;
    };
    let (g, _) = us();
    let w = |name: &str, b: &[u8]| std::fs::write(format!("{dir}/{name}"), b).unwrap();
    w("us2-ctl0.oer", &g.ctl.spdu);
    w("us2-gccf1.oer", &g.gccf.spdu);
    w("us2-lccf1.oer", &g.lccf.spdu);
}
