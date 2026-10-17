//! Unit tests of the governance entities and the device trust store (WP-B5).

use super::*;

const S: SimTime = NS_PER_S;
const DAY: SimTime = 86_400 * NS_PER_S;

fn params() -> ScmsParams {
    // The US deployment, stated rather than inherited from the defaults.
    ScmsParams {
        electors: 5,
        elector_quorum: 3,
        ..ScmsParams::default()
    }
}

fn rng() -> RngRegistry {
    RngRegistry::new(7)
}

fn gov() -> Governance {
    Governance::ceremony(&params(), &ScmsNodes::default(), &rng(), 0)
}

fn fresh(g: &Governance) -> DeviceTrust {
    DeviceTrust {
        anchors: g.anchors(),
        ..DeviceTrust::default()
    }
}

fn bootstrapped(g: &Governance) -> DeviceTrust {
    let mut t = fresh(g);
    t.install_ctl(&g.ctl)
        .expect("the genesis CTL is signed by every elector");
    t.install_chain(&g.lccf, S)
        .expect("the local chain verifies");
    t.install_policy(&g.lpf).expect("the RA's policy verifies");
    t
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

#[test]
fn a_bootstrapped_device_trusts_every_online_authority() {
    let g = gov();
    let t = bootstrapped(&g);
    assert_eq!(t.anchors.len(), 5, "five elector anchors (Profile §2.1.15)");
    for role in ICA_ISSUED
        .iter()
        .chain(ROOT_ISSUED.iter())
        .chain(["root", "ica"].iter())
    {
        assert!(t.trusts_role(role), "{role} not trusted");
    }
    assert_eq!(t.ctl_sequence, Some(0));
    assert_eq!(t.policy().map(|p| p.certs_per_period), Some(20));
    assert_eq!(t.rejected, 0);
    assert_eq!(
        t.tolerated, 2,
        "the published-RA chain-length conflict, and only it"
    );
}

/// Gap row 31 acceptance: a CTL signed by 2 of the 5 electors is refused, 3 of 5 accepted.
#[test]
fn two_of_five_is_refused_and_three_of_five_accepted() {
    let g = gov();
    for (n, ok) in [(2u32, false), (3, true), (5, true)] {
        let mut t = fresh(&g);
        let signers: Vec<u32> = (0..n).collect();
        let r = t.install_ctl(&g.ctl_endorsed_by(&signers));
        if ok {
            assert_eq!(r, Ok(n), "{n} of 5");
        } else {
            assert_eq!(
                r,
                Err(TrustError::CtlQuorum {
                    valid: 2,
                    needed: 3
                })
            );
            assert_eq!(t.ctl_sequence, None);
            assert!(t.roots.is_empty(), "no root is approved by a refused CTL");
        }
    }
    // Any three, not the first three.
    let mut t = fresh(&g);
    assert_eq!(t.install_ctl(&g.ctl_endorsed_by(&[4, 1, 3])), Ok(3));
}

/// The 1609.2.1 rule: a CTL's `quorum` governs the *next* CTL, and only a CTL that meets
/// the quorum in force can change it. One stolen elector key cannot; three can, and then
/// one key suffices for every later CTL — an observable weakness of the design.
#[test]
fn the_quorum_can_only_be_lowered_by_a_quorum() {
    let g = gov();
    let mut t = bootstrapped(&g);
    assert_eq!(
        t.install_ctl(&g.forge_next_ctl(1, &[0])),
        Err(TrustError::CtlQuorum {
            valid: 1,
            needed: 3
        })
    );
    assert_eq!(t.install_ctl(&g.forge_next_ctl(1, &[0, 1, 2])), Ok(3));
    assert_eq!(t.ctl_trust.as_ref().map(|c| c.quorum), Some(1));
    let mut g2 = g.shadow();
    g2.ctl.sequence = 1;
    assert_eq!(t.install_ctl(&g2.forge_next_ctl(1, &[0])), Ok(1));
}

#[test]
fn a_tampered_policy_or_chain_is_refused() {
    let g = gov();
    let mut t = bootstrapped(&g);
    // New labels on the old signed bytes: the device reads what was signed.
    let mut relabelled = g.lpf.clone();
    relabelled.version += 1;
    relabelled.policy.certs_per_period = 100;
    assert_eq!(t.install_policy(&relabelled), Err(TrustError::BadSignature));
    // A flipped signature byte.
    let mut flipped = g.policy_file(Scope::Local, 2, g.lpf.policy, "ra");
    let n = flipped.spdu.len();
    flipped.spdu[n - 1] ^= 1;
    assert_eq!(t.install_policy(&flipped), Err(TrustError::BadSignature));
    // A component certificate altered inside the chain file (its signature's last byte).
    let mut chain = g.lccf.clone();
    chain.version += 1;
    let n = chain.spdu.len();
    chain.spdu[n - 1] ^= 1;
    assert_eq!(t.install_chain(&chain, S), Err(TrustError::BadSignature));
    assert_eq!(t.rejected, 3);
    assert_eq!(t.policy().map(|p| p.certs_per_period), Some(20));
}

/// A policy file signed by an authority the device trusts but whose SSP is not the RA's
/// (or the PG's) is refused.
#[test]
fn a_policy_signed_by_the_wrong_authority_is_refused() {
    let g = gov();
    let mut t = bootstrapped(&g);
    let by_eca = g.policy_file(Scope::Local, 2, g.lpf.policy, "eca");
    assert_eq!(t.install_policy(&by_eca), Err(TrustError::WrongSigner));
    let global_by_ra = g.policy_file(Scope::Global, 2, g.lpf.policy, "ra");
    assert_eq!(
        t.install_policy(&global_by_ra),
        Err(TrustError::WrongSigner)
    );
    let by_ra = g.policy_file(Scope::Local, 2, g.lpf.policy, "ra");
    assert_eq!(t.install_policy(&by_ra), Ok(()));
}

#[test]
fn a_policy_decision_reaches_the_device_as_a_newer_signed_file() {
    let mut g = gov();
    let mut t = bootstrapped(&g);
    let mut p = g.gpf.policy;
    p.certs_per_period = 10;
    let v = g.decide_policy(p);
    assert_eq!(v, 2);
    t.install_policy(&g.lpf).expect("verifies");
    assert_eq!(t.policy().map(|p| p.certs_per_period), Some(10));
    assert_eq!(t.install_policy(&g.lpf), Err(TrustError::Stale));
    assert_eq!(g.counters.manager_decisions, 1);
}

#[test]
fn the_ceremony_is_deterministic() {
    let a = gov();
    let b = gov();
    assert_eq!(a.ctl, b.ctl);
    assert_eq!(a.lccf, b.lccf);
    assert_eq!(a.gpf, b.gpf);
    let c = Governance::ceremony(&params(), &ScmsNodes::default(), &RngRegistry::new(8), 0);
    assert_ne!(
        a.ctl.roots, c.ctl.roots,
        "a different seed makes different keys"
    );
    assert_ne!(a.ctl.electors, c.ctl.electors);
}

/// Gap row 31 acceptance: removing a root makes that provider's authorities untrusted on
/// a device once it installs the CTL, and the CRL Generator's signature stops verifying.
#[test]
fn root_removal_propagates_to_devices() {
    let mut g = gov();
    let mut t = bootstrapped(&g);
    let digest = [7u8; 32];
    let (crlg, sig) = g.sign_as("crlg", &digest).expect("crlg certified");
    assert!(t.verify_signed(crlg, &digest, &sig));
    let root = g.ctl.roots[0];
    let decided = 10 * DAY;
    let at = g.remove_root(root, decided).expect("approved root");
    assert_eq!(at, decided + 2 * DAY, "removal within 48 h");
    assert_eq!(g.publish_due(at - 1), 0, "not before the deadline");
    assert_eq!(g.publish_due(at), 1);
    assert_eq!(g.ctl.sequence, 1);
    assert!(g.ctl.roots.is_empty());
    assert_eq!(t.install_ctl(&g.ctl), Ok(5));
    assert!(t.roots.is_empty());
    assert!(!t.trusts_role("pca") && !t.trusts_role("ra") && !t.trusts_role("crlg"));
    assert!(t.purged >= 13, "every authority certificate was discarded");
    assert!(!t.verify_signed(crlg, &digest, &sig));
    // The provider's chain file no longer installs either.
    assert_eq!(t.install_chain(&g.lccf, at), Err(TrustError::UnknownIssuer));
    assert!(g.log.iter().any(|e| e.kind == "eac-remove-root"));
}

/// An elector change must be installed in sequence: a device that missed it cannot verify
/// a later CTL signed with the new elector's help, and catches up from the history.
#[test]
fn an_elector_replacement_is_installed_in_sequence() {
    let mut g = gov();
    let mut early = bootstrapped(&g);
    let mut late = bootstrapped(&g);
    let (slot, at) = g.replace_elector(&rng(), 0, DAY).expect("slot 0 approved");
    assert_eq!(slot, 5);
    g.publish_due(at);
    let c1 = g.ctl.clone();
    assert_eq!(
        c1.signers,
        vec![1, 2, 3, 4],
        "the removed elector does not sign"
    );
    let dec = ctl::decode_ctl_spdu(&c1.spdu).expect("decodes");
    assert_eq!(dec.elector_remove().len(), 1);
    assert_eq!(dec.elector_approve().len(), 5);
    assert_eq!(early.install_ctl(&c1), Ok(4));
    // CTL 2 signed by the new elector and two old ones.
    let c2 = g.build_ctl(2, c1.effective, None, None, &[5, 1, 2]);
    assert_eq!(early.install_ctl(&c2), Ok(3));
    assert_eq!(
        late.install_ctl(&c2),
        Err(TrustError::CtlQuorum {
            valid: 2,
            needed: 3
        })
    );
    for c in g.ctls_after(late.ctl_sequence) {
        late.install_ctl(&c).expect("catch up in order");
    }
    assert_eq!(late.install_ctl(&c2), Ok(3));
    // The removed elector no longer counts.
    let c3 = g.build_ctl(3, c1.effective, None, None, &[0, 1, 2]);
    assert_eq!(
        early.install_ctl(&c3),
        Err(TrustError::CtlQuorum {
            valid: 2,
            needed: 3
        })
    );
    assert_eq!(g.anchors().len(), 5);
    assert_eq!(g.nodes.electors.len(), 5);
}

/// Root rollover: the new root is added by CTL, the components are re-certified under it,
/// devices keep trusting throughout, and removing the old root afterwards leaves the new
/// hierarchy trusted.
#[test]
fn a_root_rollover_keeps_devices_trusting() {
    let mut g = gov();
    let mut t = bootstrapped(&g);
    let online = ScmsNodes::default();
    let (old, new, at) = g.rollover_root(&rng(), &online, DAY);
    assert_ne!(old, new);
    g.publish_due(at);
    assert_eq!(g.ctl.roots.len(), 2);
    t.install_ctl(&g.ctl).expect("signed by all five");
    t.install_chain(&g.lccf, at)
        .expect("the re-certified chain verifies");
    let digest = [9u8; 32];
    let (crlg, sig) = g.sign_as("crlg", &digest).expect("crlg");
    assert!(
        t.verify_signed(crlg, &digest, &sig),
        "the new CRL signer is trusted"
    );
    let at2 = g.remove_root(old, at).expect("old root approved");
    g.publish_due(at2);
    t.install_ctl(&g.ctl).expect("removal CTL");
    assert!(t.verify_signed(crlg, &digest, &sig));
    for role in ["root", "ica", "pca", "ra", "eca", "ma", "pg", "crlg"] {
        assert!(t.trusts_role(role), "{role} lost after rollover");
    }
    assert!(t.purged >= 13, "the old hierarchy was discarded");
}

/// The SSPs and permission groups are byte-for-byte those of the published US component
/// certificates (production chain file 2025-08-13 and test chain file 2025-07-17, decoded
/// by the oracle's asn1tools encoder; see `third_party/scms-artefacts/PROVENANCE.md`).
#[test]
fn permissions_are_the_published_bytes() {
    let g = gov();
    let enc =
        |v: &Option<SequenceOfPsidGroupPermissions>| hex_of(&coer(v.as_ref().unwrap()).unwrap());
    let app = |role: &str| {
        hex_of(
            &coer(
                &g.cert(role)
                    .unwrap()
                    .0
                    .to_be_signed
                    .app_permissions
                    .clone()
                    .unwrap(),
            )
            .unwrap(),
        )
    };
    let tbs = |role: &str| g.cert(role).unwrap().0.to_be_signed.clone();
    // rootca.prod.s2x.io
    assert_eq!(
        app("root"),
        "0102800123800381000280020100800700010001010100"
    );
    assert_eq!(
        enc(&tbs("root").cert_issue_permissions),
        "0104e081010301ffc06080010100012301ffc06080010100012601ffc060800101800201008101ffc0"
    );
    // ica.prod.s2x.io
    assert_eq!(app("ica"), "01018001238003830002");
    assert_eq!(
        enc(&tbs("ica").cert_issue_permissions),
        "0102a0810102c06080010280012381800201008101ffc0"
    );
    // eca.prod.s2x.io
    assert_eq!(app("eca"), "01018001238003840002");
    assert_eq!(
        enc(&tbs("eca").cert_issue_permissions),
        "010200800101800123800101038c0002208140"
    );
    // aca.prod.s2x.io
    assert_eq!(app("pca"), "01018001238003850002");
    assert_eq!(enc(&tbs("pca").cert_issue_permissions), "01010081");
    // ra.v2x-scms.saesoltech.io (test): crp; ma.v2x-scms.saesoltech.io (test).
    assert_eq!(app("ra"), "010180012380038b0002");
    assert_eq!(enc(&tbs("ra").cert_request_permissions), "010180810100");
    assert_eq!(app("ma"), "010180012380078a000201010120");
    // crlg.preprod.v2x.isscms.com carries CrlSignerSsp version 1 (`86 00 01`); the
    // 2022 module fixes version 2, so the one octet differs and nothing else.
    assert_eq!(
        app("crlg"),
        "0102800123800386000280020100800d00010101040004000300010002"
    );
    assert_eq!(
        hex_of(&coer(&security_mgmt_ssp("elector")).unwrap()),
        "800002"
    );
    assert_eq!(hex_of(&coer(&security_mgmt_ssp("pg")).unwrap()), "820002");
    assert_eq!(
        hex_of(&coer(&security_mgmt_ssp("la1")).unwrap()),
        "8800020001"
    );
    let _ = hex("00");
}

fn hex_of(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Issuer, crlSeries, cracaId, lifetime, key and encryption key of every component
/// certificate follow the published pattern (Profile Table 1; production chain file).
#[test]
fn every_component_certificate_has_the_published_shape() {
    let g = gov();
    let root = g.cert("root").unwrap().clone();
    let ica = g.cert("ica").unwrap().clone();
    for spec in ROLE_SPECS {
        let c = g.cert(spec.role).unwrap();
        let t = &c.0.to_be_signed;
        let (issuer, series): (Option<&Certificate>, u16) = match spec.issuer {
            IssuedBy::SelfSigned => (None, 0),
            IssuedBy::Root if spec.role == "ica" => (Some(&root), 2),
            IssuedBy::Root => (Some(&root), 256),
            IssuedBy::Ica => (Some(&ica), 2),
        };
        assert_eq!(t.crl_series.0.0, series, "{}", spec.role);
        let craca = issuer.map_or([0; 3], hashed_id3);
        assert_eq!(&t.craca_id.0[..], &craca[..], "{}", spec.role);
        assert!(sig1609::verify_explicit_cert(c, issuer), "{}", spec.role);
        assert_eq!(
            issuer_id8(c),
            issuer.map(|i| cert_hashed_id8(i).unwrap()),
            "{}",
            spec.role
        );
        let (s, e) = ctl::validity(c);
        assert_eq!(e - s, spec.lifetime.seconds(), "{}", spec.role);
        assert_eq!(t.encryption_key.is_some(), spec.encryption, "{}", spec.role);
        assert_eq!(role_of(c), Some(spec.role));
        assert!(matches!(
            sig1609::explicit_key(c),
            Some((sig1609::Hash1609::Sha256, _))
        ));
        let cid = format!("{}.provider-a.sim", spec.host);
        assert_eq!(ctl::cert_name(c), Some(cid.as_str()));
    }
    assert!(g.encryption_scalar("ra").is_some() && g.encryption_scalar("pca").is_none());
}

/// The electors are SCMS Manager's published form: P-384, self-signed with SHA-384,
/// `80 00 02`, staggered 12/15/18/21/24-year validity from Time32 503 136 005.
#[test]
fn the_electors_are_the_published_form() {
    let g = gov();
    let a = g.anchors();
    assert_eq!(a.quorum, 3);
    for (k, c) in a.electors.iter().enumerate() {
        assert!(ctl::elector_cert_ok(c));
        assert_eq!(ctl::app_ssp(c, 35), Some(Some(vec![0x80, 0x00, 0x02])));
        let (s, e) = ctl::validity(c);
        assert_eq!(s, u64::from(ELECTOR_START_TIME32));
        assert_eq!(e - s, u64::from(ELECTOR_YEARS[k]) * YEAR_S);
    }
    let dec = ctl::decode_ctl_spdu(&g.ctl.spdu).unwrap();
    assert_eq!(dec.series(), US_CTL_SERIES_ID);
    assert_eq!(dec.quorum(), Some(3));
    assert_eq!(dec.signatures.len(), 5, "every elector responds");
}

#[test]
fn an_expired_component_certificate_is_refused() {
    let g = gov();
    let mut t = fresh(&g);
    t.install_ctl(&g.ctl).unwrap();
    // The ACA certificate lives 4 years; the chain file five years on carries it expired.
    let later = 5 * YEAR_S * NS_PER_S;
    assert_eq!(t.install_chain(&g.lccf, later), Err(TrustError::Expired));
}

/// A chain file whose certificate claims a permission its issuer cannot grant is refused
/// by the 1609.2 rules, even though every signature verifies.
#[test]
fn an_over_permissioned_certificate_is_refused() {
    let g = gov();
    let mut t = fresh(&g);
    t.install_ctl(&g.ctl).unwrap();
    // The ICA re-signs an ACA certificate that claims certIssuePermissions it may not
    // issue (minChainLength 2 under an ICA allowing chain length 1 below it).
    let ica = &g.authorities["ica"];
    let mut tbs = g.cert("pca").unwrap().0.to_be_signed.clone();
    tbs.cert_issue_permissions = Some(SequenceOfPsidGroupPermissions(vec![group(
        SubjectPermissions::all(()),
        3,
        0,
        true,
        false,
    )]));
    let bad = issue(tbs, Some((&ica.cert, &ica.key)), &ica.key);
    let home = ctl::decode_ctl_spdu(&g.ctl.spdu).unwrap();
    let mut certs = g.chain_certs();
    certs.push(bad);
    let file = ChainFile {
        scope: Scope::Local,
        version: 9,
        certs: certs.clone(),
        ctl_sequence: 0,
        spdu: ctl::encode_certificate_chain(&home.raw, certs).unwrap(),
    };
    assert_eq!(t.install_chain(&file, S), Err(TrustError::NotPermitted));
}

/// SLA deadlines: an addition within 3 business days, a removal within 48 h.
#[test]
fn ctl_publication_follows_the_eac_deadlines() {
    let clock = GovernanceConfig::default().clock;
    // t0 is Tuesday 2026-01-06 09:00 UTC: three business days is Friday.
    assert_eq!(clock.to_rfc3339_at(0), "2026-01-06T09:00:00Z");
    assert_eq!(
        ctl_publication_deadline(CtlChangeKind::AddRoot, 0, clock),
        3 * DAY
    );
    // Decided Friday: Monday, Tuesday, Wednesday.
    assert_eq!(
        ctl_publication_deadline(CtlChangeKind::AddRoot, 3 * DAY, clock),
        8 * DAY
    );
    assert_eq!(
        ctl_publication_deadline(CtlChangeKind::RemoveRoot, 3 * DAY, clock),
        5 * DAY
    );
}

#[test]
fn adding_a_provider_root_and_a_test_ballot_are_recorded() {
    let mut g = gov();
    let (id, at) = g.add_root(&rng(), "provider-b", 0);
    assert_eq!(g.ctl.roots.len(), 1, "not before publication");
    g.publish_due(at);
    assert!(g.ctl.roots.contains(&id));
    let mut t = bootstrapped(&g);
    assert_eq!(t.roots.len(), 2);
    assert_eq!(g.test_ballot(at), vec![0, 1, 2, 3, 4]);
    assert_eq!(g.counters.test_ballots, 1);
    assert!(t.install_ctl(&g.ctl).is_err(), "already installed");
    let kinds: Vec<&str> = g.log.iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec!["ctl-genesis", "eac-add-root", "test-ballot"]);
}

/// Writes every artefact for the independent oracle (`oracle.py real`) when
/// `V2XW_GOV_DUMP` names a directory; a no-op otherwise.
#[test]
fn dump_artefacts_for_the_oracle() {
    let Ok(dir) = std::env::var("V2XW_GOV_DUMP") else {
        return;
    };
    let mut g = gov();
    let w = |name: &str, b: &[u8]| std::fs::write(format!("{dir}/{name}"), b).unwrap();
    w("ctl0.oer", &g.ctl.spdu);
    w("gccf1.oer", &g.gccf.spdu);
    w("lccf1.oer", &g.lccf.spdu);
    w("gpf1.oer", &g.gpf.spdu);
    w("lpf1.oer", &g.lpf.spdu);
    let (_, at) = g.replace_elector(&rng(), 0, DAY).unwrap();
    g.publish_due(at);
    let (_, _, at) = g.rollover_root(&rng(), &ScmsNodes::default(), at);
    g.publish_due(at);
    w("ctl2.oer", &g.ctl.spdu);
    w("lccf-rollover.oer", &g.lccf.spdu);
}
