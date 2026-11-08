//! A provider's further ACA instances (`aca/1`, ...).
//!
//! * each ACA instance's `certs_issued` in `pki.state` is its own issuance, equal to the
//!   `AcaRaCertResponseSpdu`s it sent (it once read provider-b's
//!   `aca/0` at 1,566 against 786: the counter summed both instances);
//! * a home root rollover re-certifies `aca-1` under the new ICA with fresh keys, retires
//!   its old certificate (what it issued still verifies), and the ICA counts the new one.

use v2xw_core::ids::NodeId;
use v2xw_msg::sec_types::ieee1609_dot2::{Certificate, IssuerIdentifier};
use v2xw_proto::ScmsParams;
use v2xw_proto::scms::deployment::{Deployment, Preset, Region};
use v2xw_proto::scms::pdu::SpduKind;
use v2xw_proto::scms::pki_view::{PkiTracker, pki_state};
use v2xw_proto::scms::run::ScmsRun;

fn deployment() -> Deployment {
    let mut d = Deployment::preset(Preset::UsProduction2025);
    d.providers[1].region = Region::assumed("us-west").expect("in the catalogue");
    d
}

fn issuer8(c: &Certificate) -> [u8; 8] {
    match &c.0.issuer {
        IssuerIdentifier::sha256AndDigest(h) | IssuerIdentifier::sha384AndDigest(h) => {
            let mut b = [0u8; 8];
            b.copy_from_slice(&h.0[..]);
            b
        }
        _ => [0; 8],
    }
}

#[test]
fn each_aca_instance_counts_its_own_certificates() {
    let home =
        ScmsRun::new_deployed(ScmsParams::default().quick(), 0, deployment()).expect("builds");
    let mut peer = ScmsRun::new_peer(&home, 1).expect("provider-b's run builds");
    assert_eq!(peer.state.aca_extra.len(), 1, "provider-b's aca/1");
    peer.preload(NodeId::new(5_000_131), 0, 1, 20)
        .expect("provisions");
    let st = &peer.state;
    let sent_by = |n: u32| {
        st.pdus
            .iter()
            .filter(|x| x.kind == SpduKind::AcaRaCertResponse && x.from == n)
            .count() as u64
    };
    let (n0, n1) = (st.nodes.pca.index(), st.aca_extra[0].index());
    let (s0, s1) = (sent_by(n0), sent_by(n1));
    let sent = |n: u32| if n == n0 { s0 } else { s1 };
    assert!(s0 > 0 && s1 > 0, "both instances issued");
    let runs = [peer];
    let s = pki_state(&runs, 0, &mut PkiTracker::default()).expect("a state");
    let acas: Vec<_> = s
        .providers
        .iter()
        .flat_map(|q| q.components.iter())
        .filter(|c| c.provider == "provider-b" && c.role == "aca")
        .collect();
    assert_eq!(acas.len(), 2, "{acas:?}");
    for c in acas {
        let n = if c.instance == 0 { n0 } else { n1 };
        assert_eq!(
            c.counters
                .get("certs_issued")
                .and_then(serde_json::Value::as_u64),
            Some(sent(n)),
            "{} counts its own issuance",
            c.id
        );
    }
    assert_eq!(runs[0].state.pca.issued_count, sent(n0) + sent(n1));
}

#[test]
fn a_root_rollover_recertifies_the_second_aca() {
    let mut d = deployment();
    d.providers[0].components.insert("aca".into(), 2);
    let mut home = ScmsRun::new_deployed(ScmsParams::default().quick(), 0, d).expect("builds");
    assert_eq!(home.state.aca_extra.len(), 1, "provider-a's aca/1");
    let label = home.state.gov.config.provider.clone();
    let before = NodeId::new(5_000_221);
    home.preload(before, 0, 1, 4).expect("provisions");
    let old = std::sync::Arc::clone(home.state.pki.as_ref().unwrap());
    let old1 = old.aca_at(1).clone();
    assert!(old1.name.starts_with("aca-1."));
    let st = &mut home.state;
    let (_, new_root, publish) = st.gov.rollover_root(&st.rng, &st.nodes, 1_000);
    st.gov.publish_due(publish);
    assert!(st.gov.ctl.roots.contains(&new_root));
    let ica_mid = st.gov.counters.ica_certs_issued;
    let next = v2xw_proto::scms::run::pdu_pki_of(&st.gov, &label).expect("builds");
    assert!(st.follow_pki(&label, std::sync::Arc::new(old.superseded_by(next))));
    let pki = std::sync::Arc::clone(home.state.pki.as_ref().unwrap());
    let new1 = pki.aca_at(1);
    assert!(new1.name.starts_with("aca-1."), "{}", new1.name);
    assert_eq!(
        issuer8(&new1.cert),
        pki.ica.id8(),
        "aca-1 is re-certified under the new ICA"
    );
    assert_ne!(
        new1.cert.0.to_be_signed.verify_key_indicator,
        old1.cert.0.to_be_signed.verify_key_indicator,
        "with fresh keys"
    );
    assert_eq!(pki.aca_instances(), 1, "one aca-1, not two");
    assert!(
        pki.retired.iter().any(|a| a.cert == old1.cert),
        "the old one retired"
    );
    assert_eq!(
        home.state.gov.counters.ica_certs_issued,
        ica_mid + 1,
        "the ICA counts the re-certified aca-1"
    );
    // What the old aca-1 issued still verifies; a joiner's pseudonyms from aca-1 chain to
    // the new root.
    let creds = |run: &ScmsRun, dev: NodeId, aca: [u8; 8]| -> Vec<Certificate> {
        run.state.devices[&dev]
            .credentials
            .values()
            .filter_map(|c| c.implicit.as_ref())
            .filter(|i| i.aca_id8 == aca)
            .map(|i| (*i.certificate).clone())
            .collect()
    };
    let olds = creds(&home, before, old1.id8());
    assert!(!olds.is_empty(), "the old aca-1 issued some");
    assert!(olds.iter().all(|c| pki.signer_key(c).is_ok()));
    let joiner = NodeId::new(5_000_222);
    home.preload(joiner, 0, 1, 4).expect("provisions");
    let news = creds(&home, joiner, pki.aca_at(1).id8());
    assert!(!news.is_empty(), "the new aca-1 issues");
    assert!(news.iter().all(|c| pki.signer_key(c).is_ok()));
    assert!(
        home.state.pdu_rejections.is_empty(),
        "{:?}",
        home.state.pdu_rejections
    );
}

/// The chain from `leaf` up to its self-signed root through `pki`'s certificates (core,
/// further and retired), by issuer digest.
fn chain_in(leaf: &Certificate, pki: &v2xw_proto::scms::pdu::ScmsPki) -> Vec<Certificate> {
    let mut out = vec![leaf.clone()];
    while out.len() < 16 {
        let d = issuer8(out.last().expect("non-empty"));
        match pki.by_id8(&d) {
            Some(a) if a.cert != *out.last().expect("non-empty") => out.push(a.cert.clone()),
            _ => break,
        }
    }
    out
}

fn validates(c: &Certificate, pki: &v2xw_proto::scms::pdu::ScmsPki) -> Result<usize, String> {
    use v2xw_sec::chain::{ChainPolicy, validate};
    let chain = chain_in(c, pki);
    let anchors =
        std::iter::once(v2xw_sec::cert::hashed_id32(&pki.root.cert).expect("hashes")).collect();
    let at = c.0.to_be_signed.validity_period.start.0.0 + 3_600;
    validate(&chain, &ChainPolicy::us_device(anchors, at))
        .map(|_| chain.len())
        .map_err(|e| format!("{e:?}"))
}

/// With `ica_levels: 3` on provider-b, an ACA instance issued by the top ICA while the
/// core ACA hangs under `ica-3` would have every reception refused `chain-length`: the
/// exact `certIssuePermissions` (`{4, 0}` at the top level) refuse an ACA one level below
/// it. Each ACA instance
/// must be issued by the core ACA's issuer, name its CRACA, and validate. Fault: the
/// instance issued under `self.ica` again (pdu.rs `issue_aca_instance`).
#[test]
fn a_second_aca_hangs_under_the_last_ica_level() {
    for levels in [1u32, 3] {
        let mut d = deployment();
        d.providers[1].ica_levels = levels;
        let home = ScmsRun::new_deployed(ScmsParams::default().quick(), 0, d).expect("builds");
        let peer = ScmsRun::new_peer(&home, 1).expect("provider-b's run builds");
        let pki = peer.state.pki.as_ref().expect("the PDU layer is on");
        let (core, second) = (pki.aca_at(0), pki.aca_at(1));
        assert!(second.name.starts_with("aca-1."), "{}", second.name);
        assert_eq!(
            issuer8(&second.cert),
            issuer8(&core.cert),
            "{levels} levels: same issuer"
        );
        assert_eq!(
            second.cert.0.to_be_signed.craca_id, core.cert.0.to_be_signed.craca_id,
            "{levels} levels: same CRACA"
        );
        let n = validates(&second.cert, pki).expect("aca-1's chain validates");
        assert_eq!(
            n,
            validates(&core.cert, pki).expect("the core ACA's chain validates")
        );
        assert_eq!(n, 2 + levels as usize, "root, the levels, the ACA");
    }
}

/// The same after a home root rollover with two ICA levels: the new generation keeps its
/// own `ica-2` (it was replaced by the old generation's), the old one is retired, and
/// the re-certified `aca-1` validates under the new root. Fault: `superseded_by` sets
/// `next.extra` to the old generation's further components again.
#[test]
fn a_rollover_keeps_the_new_ica_levels() {
    let mut d = deployment();
    d.providers[0].components.insert("aca".into(), 2);
    d.providers[0].ica_levels = 2;
    let mut home = ScmsRun::new_deployed(ScmsParams::default().quick(), 0, d).expect("builds");
    let label = home.state.gov.config.provider.clone();
    let old = std::sync::Arc::clone(home.state.pki.as_ref().unwrap());
    let old_level = old
        .extra
        .iter()
        .find(|a| a.name.starts_with("ica-2."))
        .expect("ica-2")
        .clone();
    let st = &mut home.state;
    let (_, new_root, publish) = st.gov.rollover_root(&st.rng, &st.nodes, 1_000);
    st.gov.publish_due(publish);
    assert!(st.gov.ctl.roots.contains(&new_root));
    let next = v2xw_proto::scms::run::pdu_pki_of(&st.gov, &label).expect("builds");
    let new_level = next
        .extra
        .iter()
        .find(|a| a.name.starts_with("ica-2."))
        .expect("new ica-2")
        .cert
        .clone();
    assert_ne!(new_level, old_level.cert);
    assert!(st.follow_pki(&label, std::sync::Arc::new(old.superseded_by(next))));
    let pki = std::sync::Arc::clone(home.state.pki.as_ref().unwrap());
    let held: Vec<_> = pki
        .extra
        .iter()
        .filter(|a| a.name.starts_with("ica-2."))
        .collect();
    assert_eq!(held.len(), 1, "one ica-2 in the new PKI");
    assert_eq!(held[0].cert, new_level, "the new generation's ica-2");
    assert!(
        pki.retired.iter().any(|a| a.cert == old_level.cert),
        "the old ica-2 retired"
    );
    assert_eq!(issuer8(&pki.aca_at(1).cert), issuer8(&pki.aca_at(0).cert));
    assert_eq!(
        validates(&pki.aca_at(1).cert, &pki).expect("aca-1 validates"),
        4
    );
}

/// Under `ica_levels: 3`, a CRL whose CRACA named the wrong certificate would be skipped
/// as unverifiable by every receiver. Its CRL Generator is ICA-issued (the
/// SAESOL profile), so under a deeper hierarchy `ica-3` issues it, but the PKI named the
/// top ICA as the CRL signer's issuer, the CRACA every pseudonym and the CRL's
/// `crlCraca` carry. The CRACA must be the certificate that really issued the CRL
/// signer, at every depth. Fault: `crl_signer_issuer` falls back to `self.ica` again.
#[test]
fn the_cracas_are_the_crl_signers_real_issuer_at_every_depth() {
    for levels in [1u32, 3] {
        let mut d = deployment();
        d.providers[1].ica_levels = levels;
        let home = ScmsRun::new_deployed(ScmsParams::default().quick(), 0, d).expect("builds");
        let peer = ScmsRun::new_peer(&home, 1).expect("provider-b's run builds");
        for pki in [
            home.state.pki.as_ref().unwrap(),
            peer.state.pki.as_ref().unwrap(),
        ] {
            assert_eq!(
                pki.crl_signer_issuer().id8(),
                issuer8(&pki.crlg.cert),
                "{} at {levels} levels: the CRACA issued the CRL signer",
                pki.crlg.name
            );
            assert_eq!(pki.craca_id8(), issuer8(&pki.crlg.cert));
        }
    }
}
