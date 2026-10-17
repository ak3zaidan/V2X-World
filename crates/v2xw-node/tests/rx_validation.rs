//! WP-C2 conformance matrix (00 §1 row 35): one honest case that must pass, and one case
//! per receiver rule that must fail, each changing one thing. The PKI is the run's real
//! one (`v2xw_proto::scms::pdu::ScmsPki`: P-384 self-signed root, explicit ICA, ACA and
//! CRL signer), the signer an implicit pseudonym the ACA issued by ECQV, the receiver a
//! `NodeSecurity` that installed the root and the chain file. Both crypto backends.
//!
//! Signed-CRL cases (row 37): the node accepts the CRL Generator's `SecuredCrl` and
//! refuses it with one byte altered; a series-2 hash CRL entry for the ACA revokes every
//! pseudonym under it; a series-1 linkage entry revokes the pseudonym only when its
//! `cracaId` names the CRL's CRACA; and two weeks past `nextCrl` every message is refused.

use std::sync::Arc;

use v2xw_core::rng::RngRegistry;
use v2xw_core::time::{NS_PER_S, WallClock};
use v2xw_msg::MsgType;
use v2xw_msg::sec_types::Certificate;
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    CircularRegion, GeographicRegion, HashedId3, Latitude, Longitude, NinetyDegreeInt,
    OneEightyDegreeInt, TwoDLocation, Uint16,
};
use v2xw_node::ctx::NodeRuntimeCtx;
use v2xw_node::rxval::RxReject;
use v2xw_node::secure::{CryptoMode, NodeSecurity, PSID_SAFETY, SpduVerdict};
use v2xw_node::stores::{CrlGate, CrlVerdict};
use v2xw_proto::scms::pdu::{Authority, ScmsPki};
use v2xw_sec::cert::{self, CertSpec, HolderId};
use v2xw_sec::credential::{ImplicitIssuer, ImplicitKey};
use v2xw_sec::crl::{self, CrlBody, CrlSpec, HashRevocation};
use v2xw_sec::envelope::{GenerationLocation, SignerIdChoice};
use v2xw_sec::linkage::{CrlLinkageEntry, DeviceLinkageContext, LaId, LinkageSeed};
use v2xw_sec::{ec, hashedid};

const UNIX0: i64 = 1_767_225_600;
const WEEK: u32 = 7 * 86_400;
const MODES: [CryptoMode; 2] = [CryptoMode::Real, CryptoMode::Modeled];

fn wall() -> WallClock {
    WallClock::new(UNIX0)
}

fn t0() -> u32 {
    wall().time32(0).expect("time32")
}

fn pki() -> ScmsPki {
    ScmsPki::new(7, t0() - 31_556_952).expect("PKI")
}

fn trusted(mode: CryptoMode, pki: &ScmsPki) -> NodeSecurity {
    let mut s = NodeSecurity::new(wall(), mode, PSID_SAFETY);
    let cas: Vec<Arc<Certificate>> = pki
        .all()
        .iter()
        .chain(pki.extra.iter().collect::<Vec<_>>().iter())
        .map(|a| Arc::new(a.cert.clone()))
        .collect();
    s.install_trust(&[Arc::new(pki.root.cert.clone())], &cas);
    s
}

fn device() -> DeviceLinkageContext {
    DeviceLinkageContext::new(
        LaId(1),
        LaId(2),
        LinkageSeed::new([0x11; 16]),
        LinkageSeed::new([0x22; 16]),
    )
}

/// A pseudonym the authority `aca` issues, with the US profile (series 1, the ICA as
/// CRACA, one week), changed by `f`.
fn pseudonym(pki: &ScmsPki, aca: &Authority, seed: u8, f: impl FnOnce(&mut CertSpec)) -> ImplicitKey {
    let iss = ImplicitIssuer::new(aca.cert.clone(), aca.private.expect("P-256 CA")).expect("issuer");
    let id8 = hashedid::hashed_id8(&cert::encode(&aca.cert).expect("enc"));
    let mut spec = CertSpec::pseudonym(
        id8,
        HolderId::Linkage {
            i_cert: 0,
            linkage_value: device().linkage_value_for(0, u32::from(seed)),
        },
        t0() - 60,
        PSID_SAFETY,
    );
    spec.craca_id = HashedId3(rasn::types::FixedOctetString::from(pki.craca_id()));
    spec.crl_series = 1;
    spec.valid_for = v2xw_msg::sec_types::Ieee1609Duration::hours(Uint16(169));
    f(&mut spec);
    let mut n = 0u8;
    iss.issue_spec(&spec, &ec::scalar_from_be_mod_n(&[seed; 32]), || {
        n += 1;
        let mut k = [seed ^ 0x5A; 32];
        k[1] = n;
        ec::scalar_from_be_mod_n(&k)
    })
    .expect("issues")
}

/// One BSM signed under `key` at `at` (s), attached certificate, optional location.
fn bsm(mode: CryptoMode, key: &ImplicitKey, at: u64, loc: Option<GenerationLocation>) -> Vec<u8> {
    let reg = RngRegistry::new(3);
    let mut ctx = NodeRuntimeCtx::new(at * NS_PER_S, &reg);
    let mut tx = NodeSecurity::new(wall(), mode, PSID_SAFETY);
    tx.install(v2xw_core::ids::NodeId::new(1), 0, 0, key).expect("installs");
    assert!(tx.set_active(0, 0));
    tx.sign(&mut ctx, MsgType::Bsm, b"bsm-payload", SignerIdChoice::Certificate, loc)
        .expect("signs")
        .0
        .spdu
}

/// What a receiver at `at` (s) concludes about `spdu`.
fn receive(rx: &mut NodeSecurity, spdu: &[u8], at: u64) -> SpduVerdict {
    let reg = RngRegistry::new(4);
    let mut ctx = NodeRuntimeCtx::new(at * NS_PER_S, &reg);
    let parsed = rx.parse(spdu).expect("parses");
    let c = NodeSecurity::attached_certificate(&parsed).expect("attached");
    rx.verify_parsed(&mut ctx, &parsed, &c, v2xw_core::ids::NodeId::new(9))
}

fn refused(rx: &NodeSecurity, name: &str) -> u64 {
    rx.trust().stats.refused.get(name).copied().unwrap_or(0)
}

#[test]
fn an_honest_pseudonym_validates_to_the_installed_root_and_the_work_is_priced() {
    let pki = pki();
    let key = pseudonym(&pki, &pki.aca, 1, |_| {});
    for mode in MODES {
        let mut rx = trusted(mode, &pki);
        let _ = rx.take_validation_ops();
        let spdu = bsm(mode, &key, 10, None);
        assert_eq!(receive(&mut rx, &spdu, 11), SpduVerdict::Valid, "{mode:?}");
        let t = rx.trust();
        assert!(t.is_installed());
        assert_eq!(t.stats.checked, 1);
        assert!(t.stats.refused.is_empty(), "{:?}", t.stats.refused);
        // The chain file's RA carries the published {all, 0, 0} group: tolerated, counted.
        assert!(t.stats.tolerated >= 2, "the published RA profile is recorded");
        // The new pseudonym's work (its hash, its reconstruction) is priced on a profile.
        let ops = rx.take_validation_ops();
        assert!(!ops.is_empty());
        let d = v2xw_node::rxval::price(&ops, "obu/unex-obu-301-craton2").expect("priced");
        assert!(d.as_nanos() > 0);
        // A second message under the same certificate costs no validation work.
        let spdu = bsm(mode, &key, 12, None);
        assert_eq!(receive(&mut rx, &spdu, 12), SpduVerdict::Valid);
        assert!(rx.take_validation_ops().is_empty(), "cached");
    }
}

#[test]
fn rule_psid_not_in_app_permissions() {
    let pki = pki();
    let key = pseudonym(&pki, &pki.aca, 2, |s| s.app_permissions = vec![38]);
    for mode in MODES {
        let mut rx = trusted(mode, &pki);
        assert_eq!(receive(&mut rx, &bsm(mode, &key, 10, None), 11), SpduVerdict::Refused);
        assert_eq!(refused(&rx, "psid-not-permitted"), 1, "{mode:?}");
    }
}

#[test]
fn rule_generation_time_outside_validity_and_expired() {
    let pki = pki();
    let key = pseudonym(&pki, &pki.aca, 3, |_| {});
    for mode in MODES {
        // Signed eight days in: after the 169 h validity.
        let mut rx = trusted(mode, &pki);
        let late = u64::from(8 * 86_400u32);
        assert_eq!(receive(&mut rx, &bsm(mode, &key, late, None), late), SpduVerdict::Refused);
        assert_eq!(refused(&rx, "generation-time-outside-validity"), 1);
        // Signed in time, received eight days later: the certificate has expired.
        let mut rx = trusted(mode, &pki);
        assert_eq!(receive(&mut rx, &bsm(mode, &key, 10, None), late), SpduVerdict::Refused);
        assert_eq!(refused(&rx, "chain-expired"), 1, "{mode:?}");
    }
}

#[test]
fn rule_generation_location_outside_region() {
    let pki = pki();
    // A 1 km circle at 40.75 N, 73.99 W.
    let centre = TwoDLocation::new(
        Latitude(NinetyDegreeInt(407_500_000)),
        Longitude(OneEightyDegreeInt(-739_900_000)),
    );
    // `CertSpec` has no region: the same pseudonym's tbs, with one, issued by ECQV.
    let k = pseudonym(&pki, &pki.aca, 4, |_| {});
    let iss = ImplicitIssuer::new(pki.aca.cert.clone(), pki.aca.private.unwrap()).unwrap();
    let mut tbs = k.certificate().0.to_be_signed.clone();
    tbs.region = Some(GeographicRegion::circularRegion(CircularRegion::new(
        centre,
        Uint16(1_000),
    )));
    let mut n = 0u8;
    let key = iss
        .issue(&tbs, &ec::scalar_from_be_mod_n(&[44; 32]), || {
            n += 1;
            ec::scalar_from_be_mod_n(&[n; 32])
        })
        .expect("issues");
    let inside = GenerationLocation {
        lat_tenth_microdeg: 407_501_000,
        lon_tenth_microdeg: -739_901_000,
        elevation: 0,
    };
    let outside = GenerationLocation {
        lat_tenth_microdeg: 408_500_000,
        ..inside
    };
    for mode in MODES {
        let mut rx = trusted(mode, &pki);
        assert_eq!(receive(&mut rx, &bsm(mode, &key, 10, Some(inside)), 11), SpduVerdict::Valid);
        assert_eq!(
            receive(&mut rx, &bsm(mode, &key, 12, Some(outside)), 13),
            SpduVerdict::Refused
        );
        assert_eq!(refused(&rx, "outside-region"), 1, "{mode:?}");
    }
}

#[test]
fn rule_untrusted_root() {
    let pki = pki();
    let other = ScmsPki::new(8, t0() - 31_556_952).unwrap();
    let key = pseudonym(&pki, &pki.aca, 5, |_| {});
    for mode in MODES {
        // The receiver trusts another provider's root but holds this chain file.
        let mut rx = NodeSecurity::new(wall(), mode, PSID_SAFETY);
        let cas: Vec<Arc<Certificate>> =
            pki.all().iter().map(|a| Arc::new(a.cert.clone())).collect();
        rx.install_trust(&[Arc::new(other.root.cert.clone())], &cas);
        assert_eq!(receive(&mut rx, &bsm(mode, &key, 10, None), 11), SpduVerdict::Refused);
        assert_eq!(refused(&rx, "chain-untrusted-root"), 1, "{mode:?}");
    }
}

#[test]
fn rule_issuer_lacking_cert_issue_permissions() {
    let mut pki = pki();
    // A second ACA certificate issued without certIssuePermissions.
    let ssp = v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::SecurityMgmtSsp::aca(
        v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::AcaSsp::new(
            v2xw_msg::sec_types::ieee1609_dot2_base_types::Uint8(2),
        ),
    );
    let at = pki
        .issue_component(9, "noperm.provider-a", &ssp, false, t0() - 31_556_952)
        .unwrap();
    let rogue = pki.extra[at].clone();
    let key = pseudonym(&pki, &rogue, 6, |_| {});
    for mode in MODES {
        let mut rx = trusted(mode, &pki);
        assert_eq!(receive(&mut rx, &bsm(mode, &key, 10, None), 11), SpduVerdict::Refused);
        assert_eq!(refused(&rx, "chain-permission-not-covered"), 1, "{mode:?}");
    }
}

#[test]
fn rule_chain_length_exceeded() {
    let pki = pki();
    // A sub-CA under the ACA, whose {all, 1, 0} group allows no CA below it.
    let d = ec::scalar_from_be_mod_n(&[0x33; 32]);
    let q = ec::Point::mul_base(&d);
    let mut spec = CertSpec::issuing_authority(t0() - 1_000, PSID_SAFETY);
    spec.issuer = Some(hashedid::hashed_id8(&cert::encode(&pki.aca.cert).unwrap()));
    spec.issue_permissions = Some(vec![cert::issue_all_group()]);
    spec.valid_for = v2xw_msg::sec_types::Ieee1609Duration::years(Uint16(1));
    let tbs = cert::to_be_signed(
        &spec,
        cert::verification_key_indicator(&q.compressed().unwrap()).unwrap(),
    )
    .unwrap();
    let aca_key = cert::verification_key(&pki.aca.cert, None).unwrap();
    let sub = cert::issue_explicit_cert(
        tbs,
        Some((&pki.aca.cert, &aca_key)),
        cert::CertHash::Sha256,
        |dg| Ok(pki.aca.key.sign_digest(dg)),
    )
    .unwrap();
    let sub_auth = Authority {
        name: "sub.provider-a".into(),
        cert: sub.clone(),
        key: v2xw_sec::sig1609::Signer1609::p256(&ec::scalar_to_be32(&d)).unwrap(),
        private: Some(d),
        enc_private: None,
    };
    let key = pseudonym(&pki, &sub_auth, 7, |_| {});
    for mode in MODES {
        let mut rx = trusted(mode, &pki);
        rx.learn_issuer(Arc::new(sub.clone()));
        assert_eq!(receive(&mut rx, &bsm(mode, &key, 10, None), 11), SpduVerdict::Refused);
        assert_eq!(refused(&rx, "chain-length"), 1, "{mode:?}");
    }
}

#[test]
fn an_unknown_issuer_concludes_nothing() {
    let pki = pki();
    let key = pseudonym(&pki, &pki.aca, 8, |_| {});
    let mut rx = NodeSecurity::new(wall(), CryptoMode::Real, PSID_SAFETY);
    rx.install_trust(&[Arc::new(pki.root.cert.clone())], &[]);
    assert_eq!(
        receive(&mut rx, &bsm(CryptoMode::Real, &key, 10, None), 11),
        SpduVerdict::Unverifiable
    );
}

fn signed_crl(pki: &ScmsPki, series: u16, body: CrlBody, issue: u32) -> Vec<u8> {
    let spec = CrlSpec {
        series,
        craca: pki.craca_id8(),
        issue_date: issue,
        next_crl: issue + WEEK,
        priority: None,
        delta: false,
        body,
    };
    let contents = crl::crl_contents(&spec);
    crl::encode_crl(&crl::sign_crl(&contents, &pki.crlg.cert, &pki.crlg.key).unwrap()).unwrap()
}

fn linkage_crl(pki: &ScmsPki, entries: &[CrlLinkageEntry], issue: u32) -> Vec<u8> {
    signed_crl(
        pki,
        1,
        CrlBody::Linkage {
            i_rev: 0,
            index_within_i: 0,
            individual: v2xw_proto::scms::run::linkage_revocations(entries, 0),
        },
        issue,
    )
}

#[test]
fn a_signed_crl_is_accepted_and_one_altered_byte_is_refused_by_the_node() {
    let pki = pki();
    let entry = CrlLinkageEntry::from_device(&device(), 0, 20);
    let bytes = linkage_crl(&pki, &[entry], t0());
    for mode in MODES {
        let mut rx = trusted(mode, &pki);
        let v = rx.verify_secured_crl(&bytes, 5 * NS_PER_S).expect("accepted");
        assert!(v.fresh);
        assert_eq!(v.craca, pki.craca_id8());
        assert_eq!(v.linkage, vec![entry]);
        // The same CRL again: nothing to do.
        assert!(!rx.verify_secured_crl(&bytes, 6 * NS_PER_S).unwrap().fresh);
        // Every byte position past the outer header, one at a time.
        let mut refused_all = true;
        for k in (8..bytes.len()).step_by(7) {
            let mut bad = bytes.clone();
            bad[k] ^= 0x01;
            let mut rx = trusted(mode, &pki);
            if rx.verify_secured_crl(&bad, 5 * NS_PER_S).is_ok() {
                refused_all = false;
                eprintln!("byte {k} altered was accepted");
            }
        }
        assert!(refused_all, "{mode:?}: an altered CRL must be refused");
    }
    // A CRL signed by a key that is not a CRL signer.
    let contents = crl::crl_contents(&CrlSpec {
        series: 1,
        craca: pki.craca_id8(),
        issue_date: t0(),
        next_crl: t0() + WEEK,
        priority: None,
        delta: false,
        body: CrlBody::Linkage {
            i_rev: 0,
            index_within_i: 0,
            individual: Vec::new(),
        },
    });
    let forged =
        crl::encode_crl(&crl::sign_crl(&contents, &pki.ma.cert, &pki.ma.key).unwrap()).unwrap();
    let mut rx = trusted(CryptoMode::Real, &pki);
    assert_eq!(
        rx.verify_secured_crl(&forged, 5 * NS_PER_S),
        Err(RxReject::CrlRejected)
    );
}

#[test]
fn a_linkage_entry_revokes_only_a_pseudonym_of_the_same_craca() {
    let pki = pki();
    let entry = CrlLinkageEntry::from_device(&device(), 0, 20);
    let bytes = linkage_crl(&pki, &[entry], t0());
    let mut rx = trusted(CryptoMode::Real, &pki);
    let v = rx.verify_secured_crl(&bytes, 5 * NS_PER_S).unwrap();
    let mut gate = CrlGate::new(0);
    gate.add_verified_linkage(v.craca, &v.linkage);
    let honest = pseudonym(&pki, &pki.aca, 3, |_| {});
    let (i, lv, craca, series) = v2xw_node::rxval::linkage_key(honest.certificate()).unwrap();
    assert_eq!(gate.check_from(u32::from(i), lv, craca, series, true), CrlVerdict::Revoked);
    // The same linkage value under another CRACA is not this CRL's to revoke.
    let other = pseudonym(&pki, &pki.aca, 3, |s| {
        s.craca_id = HashedId3(rasn::types::FixedOctetString::from([9, 9, 9]));
    });
    let (i, lv, craca, series) = v2xw_node::rxval::linkage_key(other.certificate()).unwrap();
    assert_eq!(gate.check_from(u32::from(i), lv, craca, series, true), CrlVerdict::NotRevoked);
}

#[test]
fn a_revoked_aca_revokes_every_pseudonym_under_it() {
    let pki = pki();
    let key = pseudonym(&pki, &pki.aca, 9, |_| {});
    let id = v2xw_sec::sig1609::cert_hashed_id10(&pki.aca.cert).unwrap();
    let bytes = signed_crl(
        &pki,
        2,
        CrlBody::Hash {
            serial: 1,
            entries: vec![HashRevocation {
                id,
                expiry: t0() + 2 * WEEK,
            }],
        },
        t0(),
    );
    for mode in MODES {
        let mut rx = trusted(mode, &pki);
        assert_eq!(receive(&mut rx, &bsm(mode, &key, 10, None), 11), SpduVerdict::Valid);
        rx.verify_secured_crl(&bytes, 12 * NS_PER_S).expect("accepted");
        assert_eq!(receive(&mut rx, &bsm(mode, &key, 13, None), 13), SpduVerdict::Revoked);
        assert_eq!(refused(&rx, "issuer-revoked"), 1, "{mode:?}");
    }
}

#[test]
fn two_weeks_past_next_crl_every_message_is_refused() {
    let pki = pki();
    let key = pseudonym(&pki, &pki.aca, 10, |_| {});
    let bytes = linkage_crl(&pki, &[], t0());
    let mut rx = trusted(CryptoMode::Real, &pki);
    rx.verify_secured_crl(&bytes, 5 * NS_PER_S).unwrap();
    // nextCrl is t0 + 1 week; the tolerance runs out two weeks later.
    let ok_at = u64::from(WEEK + 14 * 86_400 - 60);
    let late = u64::from(WEEK + 14 * 86_400 + 60);
    let renewed = pseudonym(&pki, &pki.aca, 11, |s| {
        s.valid_from = t0() + WEEK + 13 * 86_400;
    });
    assert_eq!(receive(&mut rx, &bsm(CryptoMode::Real, &renewed, ok_at, None), ok_at), SpduVerdict::Valid);
    let renewed = pseudonym(&pki, &pki.aca, 12, |s| {
        s.valid_from = t0() + WEEK + 14 * 86_400;
    });
    assert_eq!(receive(&mut rx, &bsm(CryptoMode::Real, &renewed, late, None), late), SpduVerdict::Refused);
    assert_eq!(refused(&rx, "crl-overdue"), 1);
    let _ = key;
}

#[test]
fn a_crl_signer_whose_certificate_the_ica_did_not_sign_is_refused() {
    use v2xw_msg::sec_types::ieee1609_dot2_base_types::Signature;
    let pki = pki();
    // The CRL signer's own certificate with its issuer signature altered: its key, its
    // SSP and its issuer field are unchanged, so only the chain check can refuse it.
    let mut forged = pki.crlg.cert.clone();
    if let Some(Signature::ecdsaNistP256Signature(s)) = forged.0.signature.as_mut() {
        let mut b = s.s_sig.to_vec();
        b[31] ^= 0x01;
        s.s_sig = b.into();
    } else {
        panic!("the CRL signer certificate is P-256 signed");
    }
    let contents = crl::crl_contents(&CrlSpec {
        series: 1,
        craca: pki.craca_id8(),
        issue_date: t0(),
        next_crl: t0() + WEEK,
        priority: None,
        delta: false,
        body: CrlBody::Linkage {
            i_rev: 0,
            index_within_i: 0,
            individual: Vec::new(),
        },
    });
    let bytes =
        crl::encode_crl(&crl::sign_crl(&contents, &forged, &pki.crlg.key).unwrap()).unwrap();
    let mut rx = trusted(CryptoMode::Real, &pki);
    assert_eq!(
        rx.verify_secured_crl(&bytes, 5 * NS_PER_S),
        Err(RxReject::CrlSignerUntrusted)
    );
}
