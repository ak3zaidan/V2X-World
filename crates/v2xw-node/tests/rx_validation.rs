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
fn pseudonym(
    pki: &ScmsPki,
    aca: &Authority,
    seed: u8,
    f: impl FnOnce(&mut CertSpec),
) -> ImplicitKey {
    let iss =
        ImplicitIssuer::new(aca.cert.clone(), aca.private.expect("P-256 CA")).expect("issuer");
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
    tx.install(v2xw_core::ids::NodeId::new(1), 0, 0, key)
        .expect("installs");
    assert!(tx.set_active(0, 0));
    tx.sign(
        &mut ctx,
        MsgType::Bsm,
        b"bsm-payload",
        SignerIdChoice::Certificate,
        loc,
    )
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
        assert!(
            t.stats.tolerated >= 2,
            "the published RA profile is recorded"
        );
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
        assert_eq!(
            receive(&mut rx, &bsm(mode, &key, 10, None), 11),
            SpduVerdict::Refused
        );
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
        assert_eq!(
            receive(&mut rx, &bsm(mode, &key, late, None), late),
            SpduVerdict::Refused
        );
        assert_eq!(refused(&rx, "generation-time-outside-validity"), 1);
        // Signed in time, received eight days later: the certificate has expired.
        let mut rx = trusted(mode, &pki);
        assert_eq!(
            receive(&mut rx, &bsm(mode, &key, 10, None), late),
            SpduVerdict::Refused
        );
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
        assert_eq!(
            receive(&mut rx, &bsm(mode, &key, 10, Some(inside)), 11),
            SpduVerdict::Valid
        );
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
        assert_eq!(
            receive(&mut rx, &bsm(mode, &key, 10, None), 11),
            SpduVerdict::Refused
        );
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
        assert_eq!(
            receive(&mut rx, &bsm(mode, &key, 10, None), 11),
            SpduVerdict::Refused
        );
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
        assert_eq!(
            receive(&mut rx, &bsm(mode, &key, 10, None), 11),
            SpduVerdict::Refused
        );
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
        let v = rx
            .verify_secured_crl(&bytes, 5 * NS_PER_S)
            .expect("accepted");
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
    assert_eq!(
        gate.check_from(u32::from(i), lv, craca, series, true),
        CrlVerdict::Revoked
    );
    // The same linkage value under another CRACA is not this CRL's to revoke.
    let other = pseudonym(&pki, &pki.aca, 3, |s| {
        s.craca_id = HashedId3(rasn::types::FixedOctetString::from([9, 9, 9]));
    });
    let (i, lv, craca, series) = v2xw_node::rxval::linkage_key(other.certificate()).unwrap();
    assert_eq!(
        gate.check_from(u32::from(i), lv, craca, series, true),
        CrlVerdict::NotRevoked
    );
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
        assert_eq!(
            receive(&mut rx, &bsm(mode, &key, 10, None), 11),
            SpduVerdict::Valid
        );
        rx.verify_secured_crl(&bytes, 12 * NS_PER_S)
            .expect("accepted");
        assert_eq!(
            receive(&mut rx, &bsm(mode, &key, 13, None), 13),
            SpduVerdict::Revoked
        );
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
    assert_eq!(
        receive(
            &mut rx,
            &bsm(CryptoMode::Real, &renewed, ok_at, None),
            ok_at
        ),
        SpduVerdict::Valid
    );
    let renewed = pseudonym(&pki, &pki.aca, 12, |s| {
        s.valid_from = t0() + WEEK + 14 * 86_400;
    });
    assert_eq!(
        receive(&mut rx, &bsm(CryptoMode::Real, &renewed, late, None), late),
        SpduVerdict::Refused
    );
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

// ---------------------------------------------------------------------------------------
// The validated-certificate cache (rxval module documentation): first sight pays the
// chain and the reconstruction, a repeat pays the signature only; capacity, peer-cache
// eviction and CRL installation send a signer back to first sight.
// ---------------------------------------------------------------------------------------

use v2xw_sec::primitive::{PrimitiveId, PrimitiveOpKind};

/// The validation work a new implicit pseudonym costs a receiver whose CA path is already
/// validated: SHA-256 over the certificate's COER (its HashedId8) and one ECQV
/// reconstruction (IEEE 1609.2 §5.3.1 hashing, 02 §6).
fn first_sight(key: &ImplicitKey) -> Vec<(PrimitiveId, PrimitiveOpKind, u32)> {
    let coer = cert::encode(key.certificate()).expect("encodes").len();
    vec![
        (PrimitiveId::SHA_256, PrimitiveOpKind::Verify, coer as u32),
        (PrimitiveId::ECQV_P256, PrimitiveOpKind::Verify, 0),
    ]
}

fn id8_of(key: &ImplicitKey) -> [u8; 8] {
    hashedid::id8_bytes(&cert::cert_id8(key.certificate()).expect("id8"))
}

/// Receives one BSM under `key` and returns the validation work it cost.
fn work(
    rx: &mut NodeSecurity,
    mode: CryptoMode,
    key: &ImplicitKey,
    at: u64,
) -> Vec<(PrimitiveId, PrimitiveOpKind, u32)> {
    assert_eq!(
        receive(rx, &bsm(mode, key, at, None), at),
        SpduVerdict::Valid
    );
    rx.take_validation_ops()
}

#[test]
fn first_sight_pays_the_ee_link_and_reconstruction_and_a_repeat_only_the_signature() {
    let pki = pki();
    let a = pseudonym(&pki, &pki.aca, 1, |_| {});
    let b = pseudonym(&pki, &pki.aca, 2, |_| {});
    for mode in MODES {
        let mut rx = trusted(mode, &pki);
        // Installing the chain file validated every CA path once.
        assert!(!rx.take_validation_ops().is_empty());
        let ca = rx.trust().stats.ca_validations;
        assert_eq!(work(&mut rx, mode, &a, 10), first_sight(&a), "{mode:?}");
        for t in 11..16 {
            assert!(
                work(&mut rx, mode, &a, t).is_empty(),
                "{mode:?} repeat at {t}"
            );
        }
        // A new certificate is a new first sight; the CA path is not validated again.
        assert_eq!(work(&mut rx, mode, &b, 16), first_sight(&b));
        let s = &rx.trust().stats;
        assert_eq!((s.ee_validations, s.ee_hits, s.ca_validations), (2, 5, ca));
        // The first sight costs time on a real profile.
        let d =
            v2xw_node::rxval::price(&first_sight(&a), "obu/unex-obu-301-craton2").expect("priced");
        assert!(d.as_nanos() > 0);
    }
}

#[test]
fn the_validated_cache_evicts_the_least_recently_used_signer() {
    let pki = pki();
    let [a, b, c] = [1u8, 2, 3].map(|s| pseudonym(&pki, &pki.aca, s, |_| {}));
    let mode = CryptoMode::Modeled;
    let mut rx = trusted(mode, &pki);
    rx.trust_mut().set_ee_capacity(2);
    let _ = rx.take_validation_ops();
    let miss = |rx: &mut NodeSecurity, k: &ImplicitKey, t| work(rx, mode, k, t) == first_sight(k);
    let hit = |rx: &mut NodeSecurity, k: &ImplicitKey, t| work(rx, mode, k, t).is_empty();
    assert!(miss(&mut rx, &a, 10));
    assert!(miss(&mut rx, &b, 11));
    assert!(hit(&mut rx, &a, 12)); // a is now the most recent; b the least
    assert!(miss(&mut rx, &c, 13)); // evicts b
    assert!(hit(&mut rx, &a, 14));
    assert!(miss(&mut rx, &b, 15)); // evicts c
    assert!(miss(&mut rx, &c, 16)); // evicts a
    assert!(hit(&mut rx, &b, 17));
    assert_eq!(rx.trust().ee_cache().len(), 2);
    assert_eq!(rx.trust().stats.ee_evictions, 3);
}

#[test]
fn a_certificate_the_peer_cache_forgets_is_validated_again() {
    let pki = pki();
    let a = pseudonym(&pki, &pki.aca, 4, |_| {});
    let mode = CryptoMode::Real;
    let mut rx = trusted(mode, &pki);
    let _ = rx.take_validation_ops();
    assert_eq!(work(&mut rx, mode, &a, 10), first_sight(&a));
    assert!(work(&mut rx, mode, &a, 11).is_empty());
    rx.trust_mut().forget(id8_of(&a));
    assert!(!rx.trust().ee_cache().contains(&id8_of(&a)));
    assert_eq!(work(&mut rx, mode, &a, 12), first_sight(&a));
    assert_eq!(rx.trust().stats.ee_evictions, 1);
    // Forgetting a certificate the node never validated changes nothing.
    rx.trust_mut().forget([0xAB; 8]);
    assert_eq!(rx.trust().stats.ee_evictions, 1);
}

#[test]
fn a_fresh_crl_sends_the_signers_it_governs_back_to_first_sight() {
    let pki = pki();
    let a = pseudonym(&pki, &pki.aca, 5, |_| {});
    // Another device's entry: this CRL revokes nobody the receiver hears.
    let other = DeviceLinkageContext::new(
        LaId(3),
        LaId(4),
        LinkageSeed::new([0x33; 16]),
        LinkageSeed::new([0x44; 16]),
    );
    let linkage = linkage_crl(&pki, &[CrlLinkageEntry::from_device(&other, 0, 20)], t0());
    let ca_hash = signed_crl(
        &pki,
        2,
        CrlBody::Hash {
            serial: 1,
            entries: vec![HashRevocation {
                id: [0xEE; 10],
                expiry: t0() + 2 * WEEK,
            }],
        },
        t0(),
    );
    for mode in MODES {
        let mut rx = trusted(mode, &pki);
        let _ = rx.take_validation_ops();
        assert_eq!(work(&mut rx, mode, &a, 10), first_sight(&a));
        assert!(work(&mut rx, mode, &a, 11).is_empty());
        // A series-1 CRL of the pseudonym's CRACA governs it: validated again.
        let v = rx
            .verify_secured_crl(&linkage, 12 * NS_PER_S)
            .expect("accepted");
        assert!(v.fresh);
        let _ = rx.take_validation_ops(); // the CRL's own verification
        assert_eq!(rx.trust().stats.invalidated, 1, "{mode:?}");
        assert_eq!(work(&mut rx, mode, &a, 13), first_sight(&a), "{mode:?}");
        assert!(work(&mut rx, mode, &a, 14).is_empty());
        // The same CRL again is no change and invalidates nothing.
        let v = rx
            .verify_secured_crl(&linkage, 15 * NS_PER_S)
            .expect("accepted");
        assert!(!v.fresh);
        assert!(rx.take_validation_ops().is_empty());
        assert!(work(&mut rx, mode, &a, 16).is_empty());
        assert_eq!(rx.trust().stats.invalidated, 1);
        // A series-2 hash CRL governs the ACA, not the pseudonym: the CA path above it is
        // validated again (explicit links, no reconstruction), the EE verdict is kept.
        rx.verify_secured_crl(&ca_hash, 17 * NS_PER_S)
            .expect("accepted");
        let _ = rx.take_validation_ops();
        assert!(rx.trust().stats.invalidated > 1);
        let ca = rx.trust().stats.ca_validations;
        let ops = work(&mut rx, mode, &a, 18);
        assert!(!ops.is_empty(), "{mode:?}");
        assert!(ops.iter().all(|o| o.0 != PrimitiveId::ECQV_P256), "{ops:?}");
        assert_eq!(rx.trust().stats.ca_validations, ca + 1);
        assert!(work(&mut rx, mode, &a, 19).is_empty());
    }
}

/// A series-1 CRL with an explicit `indexWithinI`, full or delta.
fn indexed_crl(
    pki: &ScmsPki,
    index: u8,
    delta: bool,
    entries: &[CrlLinkageEntry],
    issue: u32,
) -> Vec<u8> {
    let spec = CrlSpec {
        series: 1,
        craca: pki.craca_id8(),
        issue_date: issue,
        next_crl: issue + WEEK,
        priority: None,
        delta,
        body: CrlBody::Linkage {
            i_rev: 0,
            index_within_i: index,
            individual: v2xw_proto::scms::run::linkage_revocations(entries, 0),
        },
    };
    let contents = crl::crl_contents(&spec);
    crl::encode_crl(&crl::sign_crl(&contents, &pki.crlg.cert, &pki.crlg.key).unwrap()).unwrap()
}

/// R2 at the node: a genuine CRL, full or delta, whose 1609.2 counter is not above the
/// installed one's for that CRACA and series is stale. Before the fix only an older
/// `issueDate` on a *full* CRL was refused, so an earlier list issued in the same second
/// was accepted, and any old delta was accepted and moved the node's `nextCrl` (its
/// overdue clock) back to the old list's.
#[test]
fn an_older_crl_full_or_delta_is_stale_even_in_the_same_second() {
    let pki = pki();
    let entry = CrlLinkageEntry::from_device(&device(), 0, 20);
    let v0 = indexed_crl(&pki, 0, false, &[], t0());
    let v1 = indexed_crl(&pki, 1, false, &[entry], t0());
    let old_delta = indexed_crl(&pki, 0, true, &[], t0() - 3_600);
    let d2 = indexed_crl(&pki, 2, true, &[entry], t0() + 60);
    let mut rx = trusted(CryptoMode::Real, &pki);
    assert!(rx.verify_secured_crl(&v1, 5 * NS_PER_S).unwrap().fresh);
    assert_eq!(
        rx.verify_secured_crl(&v0, 6 * NS_PER_S),
        Err(RxReject::CrlStale),
        "an earlier list of the same second"
    );
    assert_eq!(
        rx.verify_secured_crl(&old_delta, 7 * NS_PER_S),
        Err(RxReject::CrlStale),
        "an earlier delta"
    );
    // The installed list again is a repeat, not a refusal; a later delta is installed.
    assert!(!rx.verify_secured_crl(&v1, 8 * NS_PER_S).unwrap().fresh);
    assert!(rx.verify_secured_crl(&d2, 9 * NS_PER_S).unwrap().fresh);
    assert_eq!(
        rx.trust().stats.crls_refused.get("crl-stale").copied(),
        Some(2)
    );
}

// -----------------------------------------------------------------------------------
// Edge-case package, item 2: a node's clock. A receiver checks the signer's validity
// against its own current time (IEEE 1609.2 §5.2), and a sender stamps `generationTime`
// on its own clock (§6.3.9), so a skewed clock moves both edges.
// -----------------------------------------------------------------------------------

/// One BSM signed under `key` at true time `at` (s) by a sender whose clock stands
/// `offset_s` off the truth.
fn skewed_bsm(mode: CryptoMode, key: &ImplicitKey, at: u64, offset_s: i64) -> Vec<u8> {
    let reg = RngRegistry::new(3);
    let mut ctx = NodeRuntimeCtx::new(at * NS_PER_S, &reg);
    let mut tx = NodeSecurity::new(wall(), mode, PSID_SAFETY);
    tx.set_clock_offset(offset_s * NS_PER_S as i64);
    tx.install(v2xw_core::ids::NodeId::new(1), 0, 0, key)
        .expect("installs");
    assert!(tx.set_active(0, 0));
    tx.sign(
        &mut ctx,
        MsgType::Bsm,
        b"bsm-payload",
        SignerIdChoice::Certificate,
        None,
    )
    .expect("signs")
    .0
    .spdu
}

#[test]
fn a_receiver_whose_clock_runs_ahead_refuses_a_certificate_still_valid() {
    let pki = pki();
    let key = pseudonym(&pki, &pki.aca, 13, |_| {});
    for mode in MODES {
        let spdu = bsm(mode, &key, 10, None);
        // True time 100 s: well inside the 169 h validity. An accurate receiver accepts.
        let mut rx = trusted(mode, &pki);
        assert_eq!(receive(&mut rx, &spdu, 100), SpduVerdict::Valid, "{mode:?}");
        // A receiver whose clock is 170 h ahead believes the certificate has expired.
        let mut rx = trusted(mode, &pki);
        rx.set_clock_offset(170 * 3_600 * NS_PER_S as i64);
        assert_eq!(
            receive(&mut rx, &spdu, 100),
            SpduVerdict::Refused,
            "{mode:?}"
        );
        assert_eq!(refused(&rx, "chain-expired"), 1, "{mode:?}");
    }
}

#[test]
fn a_sender_whose_clock_lags_stamps_its_own_time_and_keeps_an_expired_certificate() {
    let pki = pki();
    let key = pseudonym(&pki, &pki.aca, 14, |_| {});
    let late = u64::from(8 * 86_400u32);
    for mode in MODES {
        // True time: eight days in, after the validity. The sender's clock is two days
        // behind, so it believes the certificate is current and stamps its own time.
        let spdu = skewed_bsm(mode, &key, late, -2 * 86_400);
        let mut rx = trusted(mode, &pki);
        let parsed = rx.parse(&spdu).expect("parses");
        let believed = (late - 2 * 86_400) * NS_PER_S;
        assert_eq!(
            parsed.generation_time.map(|g| g / 1_000_000),
            Some(u64::from(wall().time32(believed).unwrap())),
            "generationTime is the sender's believed time ({mode:?})"
        );
        // An accurate receiver: the generation time is inside the validity, the
        // certificate is not current at the receiver's time.
        assert_eq!(receive(&mut rx, &spdu, late), SpduVerdict::Refused);
        assert_eq!(
            refused(&rx, "generation-time-outside-validity"),
            0,
            "{mode:?}"
        );
        assert_eq!(refused(&rx, "chain-expired"), 1, "{mode:?}");
        // A receiver whose clock lags by the same two days accepts it.
        let mut rx = trusted(mode, &pki);
        rx.set_clock_offset(-2 * 86_400 * NS_PER_S as i64);
        assert_eq!(
            receive(&mut rx, &spdu, late),
            SpduVerdict::Valid,
            "{mode:?}"
        );
    }
}

/// R-TRUSTLEAK (testing/matrix.md): a frame the engine sizes rather than encodes (a
/// roadside unit's CRL frame, a report relayed over the air) reaches the receiver with its
/// signer's certificate and no SPDU. It used to verify on the engine's word alone, so a
/// provider's frames still verified after the CTL dropped its root. The receiver now holds
/// the certificate to its installed roots before the modelled signature counts, and hands
/// its linkage data to the CRL check.
#[test]
fn a_sized_frame_is_held_to_the_installed_roots() {
    let pki = pki();
    let other = ScmsPki::new(8, t0() - 31_556_952).unwrap();
    let key = pseudonym(&pki, &pki.aca, 16, |_| {});
    let cert: Certificate = key.certificate().as_ref().clone();
    let roots = |p: &ScmsPki| vec![Arc::new(p.root.cert.clone())];
    let reg = RngRegistry::new(4);
    for mode in MODES {
        let mut ctx = NodeRuntimeCtx::new(11 * NS_PER_S, &reg);
        let mut rx = trusted(mode, &pki);
        let (v, linkage) = rx.verify_sized(&mut ctx, Some(&cert), true);
        assert_eq!(v, SpduVerdict::Valid, "{mode:?}");
        assert!(linkage.is_some(), "its linkage data reaches the CRL check");
        assert_eq!(
            rx.verify_sized(&mut ctx, Some(&cert), false).0,
            SpduVerdict::Invalid,
            "a bad modelled signature still fails"
        );
        // The CTL drops the root: refused from then on, whatever the engine says.
        assert_eq!(rx.apply_ctl(&roots(&other), &[]), (1, 1));
        assert_eq!(
            rx.verify_sized(&mut ctx, Some(&cert), true).0,
            SpduVerdict::Refused,
            "{mode:?}"
        );
        assert_eq!(refused(&rx, "chain-untrusted-root"), 1, "{mode:?}");
        // A receiver that never held this provider's chain file has concluded nothing.
        let mut stranger = trusted(mode, &other);
        assert_eq!(
            stranger.verify_sized(&mut ctx, Some(&cert), true).0,
            SpduVerdict::Unverifiable
        );
        // No certificate to check: the engine's word, and counted as unchecked.
        let before = stranger.trust().stats.chain_unchecked;
        assert_eq!(
            stranger.verify_sized(&mut ctx, None, true),
            (SpduVerdict::Valid, None)
        );
        assert_eq!(stranger.trust().stats.chain_unchecked, before + 1);
    }
}

/// R-TRUSTLEAK at a roadside unit: a misbehaviour report relayed over the air is a sized
/// frame. The unit forwarded any whose modelled signature was good, so a provider off the
/// CTL could use the infrastructure of the other provider as a relay. It now forwards one
/// only when the attached certificate chains to a root it trusts.
#[test]
fn a_unit_forwards_a_sized_report_only_under_a_trusted_root() {
    use v2xw_core::geom::Vec3;
    use v2xw_node::rsu::{Backhaul, RsuConfig, RsuRole, RsuRoles, RsuRuntime};
    use v2xw_node::runtime::RxFrame;
    use v2xw_node::stores::VerificationState;
    let pki = pki();
    let other = ScmsPki::new(8, t0() - 31_556_952).unwrap();
    let trusted_key = pseudonym(&pki, &pki.aca, 17, |_| {});
    let rogue_key = pseudonym(&other, &other.aca, 18, |_| {});
    let cas: Vec<Arc<Certificate>> = pki.all().iter().map(|a| Arc::new(a.cert.clone())).collect();
    let relay = |key: &ImplicitKey| {
        let profile = v2xw_node::profiles::get(v2xw_node::profiles::DEFAULT_RSU)
            .expect("the default roadside profile")
            .clone();
        let mut cfg = RsuConfig::with_roles(RsuRoles::NONE.with(RsuRole::ReportForwarding));
        cfg.wall = wall();
        let mut u = RsuRuntime::new(
            v2xw_core::ids::NodeId::new(50),
            profile,
            cfg,
            Backhaul::fibre(),
            0,
        );
        u.stores_mut().crl.set_period(0);
        u.security_mut()
            .install_trust(&[Arc::new(pki.root.cert.clone())], &cas);
        let cert = Arc::clone(key.certificate());
        let frame = RxFrame {
            signer: Some(cert::cert_id8(&cert).expect("digest")),
            msg_type: MsgType::Mbr,
            bytes: 1_200,
            claimed_pos: Some(Vec3::new(120.0, 100.0, 1.5)),
            claimed_speed_mps: 12.0,
            claimed_heading_rad: 0.0,
            claimed_generation_time: 10 * NS_PER_S,
            full_certificate: true,
            signature_valid: true,
            spdu: None,
            signer_certificate: Some(cert),
            bsm: Default::default(),
        };
        let reg = RngRegistry::new(55);
        let mut ctx = NodeRuntimeCtx::new(11 * NS_PER_S, &reg);
        let out = u.step(&mut ctx, vec![frame]);
        (
            out.forwarded.len(),
            out.delivered.first().map(|m| m.verification),
            u.security().trust().stats.chain_unchecked,
        )
    };
    let (n, v, unchecked) = relay(&trusted_key);
    assert_eq!(
        (n, v, unchecked),
        (1, Some(VerificationState::Verified), 0),
        "under the unit's root: checked, verified, forwarded"
    );
    let (n, v, _) = relay(&rogue_key);
    assert_eq!(n, 0, "under a root the unit does not trust: not forwarded");
    assert_ne!(v, Some(VerificationState::Verified));
}

/// A sized frame (no SPDU) signed under `key`, with the engine's modelled verdict "good
/// signature" and the signer certificate attached, as the engine hands a receiver an RSU
/// CRL frame or a relayed report.
fn sized_frame(key: &ImplicitKey, msg_type: MsgType) -> v2xw_node::runtime::RxFrame {
    let cert = Arc::clone(key.certificate());
    v2xw_node::runtime::RxFrame {
        signer: Some(cert::cert_id8(&cert).expect("digest")),
        msg_type,
        bytes: 300,
        claimed_pos: Some(v2xw_core::geom::Vec3::new(20.0, 0.0, 1.5)),
        claimed_speed_mps: 10.0,
        claimed_heading_rad: 0.0,
        claimed_generation_time: 10 * NS_PER_S,
        full_certificate: true,
        signature_valid: true,
        spdu: None,
        signer_certificate: Some(cert),
        bsm: Default::default(),
    }
}

/// A three-dimensional fix at the origin, so the receiver has a position to judge by.
fn fix() -> v2xw_core::belief::PositionEstimate {
    let mut p = v2xw_core::belief::PositionEstimate::no_fix(0);
    p.semi_major_m = 4.0;
    p.semi_minor_m = 3.0;
    p.fix = v2xw_core::belief::FixQuality::ThreeD;
    p
}

/// The three outcomes a receiver's sized-frame fallback must give, whatever the engine's
/// modelled signature says: under its installed root, verified; under a root it never held,
/// not verified; under a root the CTL has since dropped, not verified.
fn sized_fallback_cases(
    pki: &ScmsPki,
    other: &ScmsPki,
) -> (
    ImplicitKey,
    ImplicitKey,
    Vec<Arc<Certificate>>,
    Vec<Arc<Certificate>>,
) {
    let trusted_key = pseudonym(pki, &pki.aca, 19, |_| {});
    let rogue_key = pseudonym(other, &other.aca, 20, |_| {});
    let cas: Vec<Arc<Certificate>> = pki.all().iter().map(|a| Arc::new(a.cert.clone())).collect();
    (
        trusted_key,
        rogue_key,
        vec![Arc::new(pki.root.cert.clone())],
        cas,
    )
}

/// R-TRUSTLEAK on a vehicle's receiver (06-validation-summary §9: only the engine and RSU
/// paths had been proved red). `ObuRuntime::classify` falls back to
/// `NodeSecurity::verify_sized` for a frame with no SPDU. Fault: make that branch return
/// the engine's modelled verdict (`(frame.signature_valid, None)`), as before the fix, and
/// the rogue and dropped-root frames come out `Verified`.
#[test]
fn an_obu_accepts_a_sized_frame_only_under_a_root_it_trusts() {
    use v2xw_node::generate::ServiceSet;
    use v2xw_node::policy::VerifyAll;
    use v2xw_node::runtime::{NodeConfig, ObuRuntime};
    use v2xw_node::stores::VerificationState;
    let pki = pki();
    let other = ScmsPki::new(8, t0() - 31_556_952).unwrap();
    let (trusted_key, rogue_key, roots, cas) = sized_fallback_cases(&pki, &other);
    let verdict = |key: &ImplicitKey, drop_root: bool| {
        let config = NodeConfig {
            services: ServiceSet::SAE,
            wall: wall(),
            ..NodeConfig::default()
        };
        let mut rx = ObuRuntime::new(
            v2xw_core::ids::NodeId::new(2),
            v2xw_node::profiles::get(v2xw_node::profiles::REFERENCE_OBU)
                .expect("the reference profile")
                .clone(),
            Box::new(VerifyAll::new()),
            config,
            0,
        );
        rx.stores_mut().crl.set_period(0);
        rx.set_belief(fix());
        rx.security_mut().install_trust(&roots, &cas);
        if drop_root {
            let gone = vec![Arc::new(other.root.cert.clone())];
            assert_eq!(rx.security_mut().apply_ctl(&gone, &[]), (1, 1));
        }
        let reg = RngRegistry::new(61);
        let mut out = Vec::new();
        // Three seconds of 10 ms steps: the validation is priced (chain links, the
        // reconstruction), so the verdict lands some hundred milliseconds later.
        for k in 0..300u64 {
            let mut ctx = NodeRuntimeCtx::new(10 * NS_PER_S + k * 10_000_000, &reg);
            let inbox = if k == 0 {
                vec![sized_frame(key, MsgType::Bsm)]
            } else {
                Vec::new()
            };
            out.extend(rx.step(&mut ctx, inbox, 0.0).delivered);
        }
        out.iter()
            .find(|m| m.msg_type == MsgType::Bsm)
            .map(|m| m.verification)
    };
    assert_eq!(
        verdict(&trusted_key, false),
        Some(VerificationState::Verified)
    );
    // Not delivered at all (the signer cannot be checked, so a P2PCD learn is due) or
    // delivered as anything but verified.
    assert_ne!(
        verdict(&rogue_key, false),
        Some(VerificationState::Verified),
        "under a root the vehicle never held"
    );
    assert_eq!(
        verdict(&trusted_key, true),
        Some(VerificationState::Invalid),
        "under a root the CTL dropped: refused (chain-untrusted-root)"
    );
}

/// Audit db1-3 (OA.COST.unverifiable-free, docs/design/scms-real/testing/audit-db1-3.md):
/// a receiver that cannot verify a signer — here its issuer is one the node does not hold,
/// so the implicit certificate's key cannot even be reconstructed (IEEE 1609.2 §5.2) —
/// runs no signature check, so it is charged none. `node.verify` names no primitive, gives
/// the reason `unknown-signer`, and under `verify-all` (which holds unknown signers back)
/// the outcome `dropped`, so `unverified_ratio` does not count it as delivered. A signer it
/// can verify still pays its signature. Fault: charge the signature again (drop the
/// `Unverified` branch in `ObuRuntime`), and the stranger's frame costs the profile's
/// verify.
#[test]
fn a_signer_the_receiver_cannot_verify_is_charged_no_signature_check() {
    use v2xw_node::generate::ServiceSet;
    use v2xw_node::policy::VerifyAll;
    use v2xw_node::runtime::{NodeConfig, ObuRuntime};
    let pki = pki();
    let other = ScmsPki::new(8, t0() - 31_556_952).unwrap();
    let (trusted_key, rogue_key, roots, cas) = sized_fallback_cases(&pki, &other);
    let config = NodeConfig {
        services: ServiceSet::SAE,
        wall: wall(),
        ..NodeConfig::default()
    };
    let mut rx = ObuRuntime::new(
        v2xw_core::ids::NodeId::new(2),
        v2xw_node::profiles::get(v2xw_node::profiles::REFERENCE_OBU)
            .expect("the reference profile")
            .clone(),
        Box::new(VerifyAll::new()),
        config,
        0,
    );
    rx.stores_mut().crl.set_period(0);
    rx.set_belief(fix());
    rx.security_mut().install_trust(&roots, &cas);
    let reg = RngRegistry::new(61);
    let mut recs = Vec::new();
    // A trusted signer first (it also pays the CA validations of the installed trust),
    // then the stranger, then the trusted signer again (a cached repeat: its signature
    // alone), each a second apart.
    let order = [&trusted_key, &rogue_key, &trusted_key];
    for k in 0..300u64 {
        let mut ctx = NodeRuntimeCtx::new(10 * NS_PER_S + k * 10_000_000, &reg);
        let inbox = if k % 100 == 0 {
            vec![sized_frame(order[(k / 100) as usize], MsgType::Bsm)]
        } else {
            Vec::new()
        };
        let _ = rx.step(&mut ctx, inbox, 0.0);
        recs.extend(
            ctx.take_emitted()
                .into_iter()
                .filter(|r| r.channel == "node.verify")
                .map(|r| serde_json::from_slice::<serde_json::Value>(&r.json).unwrap()),
        );
    }
    assert_eq!(recs.len(), 3, "one task per frame: {recs:?}");
    let (first, stranger, repeat) = (&recs[0], &recs[1], &recs[2]);
    assert_eq!(first["outcome"], "valid", "{first}");
    assert_eq!(repeat["outcome"], "valid", "{repeat}");
    assert_eq!(repeat["primitive"], "ecdsa-p256-verify");
    let paid = repeat["cost_us"].as_u64().unwrap();
    assert!(paid > 0, "a verified repeat pays its signature: {repeat}");
    assert_eq!(stranger["reason"], "unknown-signer", "{stranger}");
    assert_eq!(
        stranger["outcome"], "dropped",
        "verify-all holds it back: {stranger}"
    );
    assert!(
        stranger["primitive"].is_null(),
        "no primitive ran: {stranger}"
    );
    assert_eq!(
        stranger["cost_us"].as_u64(),
        Some(0),
        "no signature check, no chain link validated: {stranger} (a verified repeat cost {paid} us)"
    );
}

/// R-TRUSTLEAK on a VRU handset's receiver: `VruDeviceRuntime::classify` has the same
/// sized-frame fallback. Same fault, same red.
#[test]
fn a_vru_handset_accepts_a_sized_frame_only_under_a_root_it_trusts() {
    use v2xw_node::stores::VerificationState;
    use v2xw_node::vru::{VruConfig, VruDeviceRuntime, VruServices};
    let pki = pki();
    let other = ScmsPki::new(8, t0() - 31_556_952).unwrap();
    let (trusted_key, rogue_key, roots, cas) = sized_fallback_cases(&pki, &other);
    let verdict = |key: &ImplicitKey, drop_root: bool| {
        let mut rx = VruDeviceRuntime::new(
            v2xw_core::ids::NodeId::new(3),
            v2xw_node::profiles::get(v2xw_node::profiles::GENERIC_VRU_DEVICE)
                .expect("the shipped VRU-device profile")
                .clone(),
            VruConfig {
                services: VruServices::SAE,
                wall: wall(),
                ..VruConfig::default()
            },
            0,
        );
        rx.stores_mut().crl.set_period(0);
        rx.set_belief(fix());
        rx.security_mut().install_trust(&roots, &cas);
        if drop_root {
            let gone = vec![Arc::new(other.root.cert.clone())];
            assert_eq!(rx.security_mut().apply_ctl(&gone, &[]), (1, 1));
        }
        let reg = RngRegistry::new(62);
        let mut out = Vec::new();
        // Three seconds of 10 ms steps: the validation is priced (chain links, the
        // reconstruction), so the verdict lands some hundred milliseconds later.
        for k in 0..300u64 {
            let mut ctx = NodeRuntimeCtx::new(10 * NS_PER_S + k * 10_000_000, &reg);
            let inbox = if k == 0 {
                vec![sized_frame(key, MsgType::Cam)]
            } else {
                Vec::new()
            };
            out.extend(rx.step(&mut ctx, inbox, 0.0).delivered);
        }
        out.iter()
            .find(|m| m.msg_type == MsgType::Cam)
            .map(|m| m.verification)
    };
    assert_eq!(
        verdict(&trusted_key, false),
        Some(VerificationState::Verified)
    );
    // Not delivered at all (the signer cannot be checked, so a P2PCD learn is due) or
    // delivered as anything but verified.
    assert_ne!(
        verdict(&rogue_key, false),
        Some(VerificationState::Verified),
        "under a root the handset never held"
    );
    assert_eq!(
        verdict(&trusted_key, true),
        Some(VerificationState::Invalid),
        "under a root the CTL dropped: refused (chain-untrusted-root)"
    );
}

// Edge-case package, item 8: a provider's root removed from the CTL mid-run.
#[test]
fn a_root_a_newer_ctl_drops_is_refused_from_then_on_and_an_unchanged_ctl_forgets_nothing() {
    let pki = pki();
    let other = ScmsPki::new(8, t0() - 31_556_952).unwrap();
    let key = pseudonym(&pki, &pki.aca, 15, |_| {});
    let roots = |p: &ScmsPki| vec![Arc::new(p.root.cert.clone())];
    for mode in MODES {
        let mut rx = trusted(mode, &pki);
        let spdu = bsm(mode, &key, 10, None);
        assert_eq!(receive(&mut rx, &spdu, 11), SpduVerdict::Valid);
        // The same CTL again: nothing added or removed, the signer stays validated.
        assert_eq!(rx.apply_ctl(&roots(&pki), &[]), (0, 0));
        let hits = rx.trust().stats.ee_hits;
        assert_eq!(receive(&mut rx, &spdu, 12), SpduVerdict::Valid);
        assert_eq!(rx.trust().stats.ee_hits, hits + 1, "{mode:?}");
        // A CTL adding the other provider's root, then one removing this one: the
        // removal alone (nothing added) must forget what chained to the dropped root.
        let both = [roots(&pki), roots(&other)].concat();
        assert_eq!(rx.apply_ctl(&both, &[]), (1, 0));
        assert_eq!(receive(&mut rx, &spdu, 13), SpduVerdict::Valid);
        assert_eq!(rx.apply_ctl(&roots(&other), &[]), (0, 1));
        assert_eq!(
            receive(&mut rx, &spdu, 14),
            SpduVerdict::Refused,
            "{mode:?}"
        );
        assert_eq!(refused(&rx, "chain-untrusted-root"), 1, "{mode:?}");
    }
}

/// `security.scms.crl_overdue_tolerance_s` (SCMS settings audit): a device configured with
/// a one-day tolerance refuses, a day and a minute past `nextCrl`, the message the
/// profile's two weeks still accept; and the default is the profile's.
#[test]
fn a_configured_overdue_tolerance_moves_the_refusal() {
    let pki = pki();
    let bytes = linkage_crl(&pki, &[], t0());
    let at = u64::from(WEEK + 86_400 + 60);
    let renewed = pseudonym(&pki, &pki.aca, 13, |s| {
        s.valid_from = t0() + WEEK + 86_400;
    });
    let mut profile = trusted(CryptoMode::Real, &pki);
    assert_eq!(profile.trust().overdue_crl_tolerance_s(), 14 * 86_400);
    profile.verify_secured_crl(&bytes, 5 * NS_PER_S).unwrap();
    assert_eq!(
        receive(&mut profile, &bsm(CryptoMode::Real, &renewed, at, None), at),
        SpduVerdict::Valid
    );
    let mut strict = trusted(CryptoMode::Real, &pki);
    strict.trust_mut().set_overdue_crl_tolerance(86_400);
    assert_eq!(strict.trust().overdue_crl_tolerance_s(), 86_400);
    strict.verify_secured_crl(&bytes, 5 * NS_PER_S).unwrap();
    assert_eq!(
        receive(&mut strict, &bsm(CryptoMode::Real, &renewed, at, None), at),
        SpduVerdict::Refused
    );
    assert_eq!(refused(&strict, "crl-overdue"), 1);
}

/// Rerun §9 item 3: a message a 1609.2 receiver rule refused is reported as that rule, not
/// as a bad signature. The security stack keeps the rule of its last refusal for the node
/// to read once, and the handset's report on the frame carries it
/// (`RxDisposition::Refused`), so `node.rx` names a stale list `crl-overdue` where it read
/// `signature-invalid` (97,679 of them in a multi-week run). Faults that turn it red:
/// `NodeSecurity`'s signer validation not keeping the rule, or `VruDeviceRuntime`'s booking
/// reporting `Delivered(Invalid)` regardless.
#[test]
fn a_receiver_rule_refusal_is_reported_as_the_rule_not_as_a_bad_signature() {
    use v2xw_node::runtime::RxDisposition;
    use v2xw_node::vru::{VruConfig, VruDeviceRuntime, VruServices};
    let pki = pki();
    let bytes = linkage_crl(&pki, &[], t0());
    let at = u64::from(WEEK + 86_400 + 60);
    let renewed = pseudonym(&pki, &pki.aca, 13, |s| {
        s.valid_from = t0() + WEEK + 86_400;
    });
    let mut strict = trusted(CryptoMode::Real, &pki);
    strict.trust_mut().set_overdue_crl_tolerance(86_400);
    strict.verify_secured_crl(&bytes, 5 * NS_PER_S).unwrap();
    assert_eq!(
        receive(&mut strict, &bsm(CryptoMode::Real, &renewed, at, None), at),
        SpduVerdict::Refused
    );
    assert_eq!(strict.take_last_refusal(), Some(RxReject::CrlOverdue));
    assert_eq!(strict.take_last_refusal(), None, "the rule is read once");

    // A handset under a root its CTL dropped: the frame's report names the chain rule.
    let other = ScmsPki::new(8, t0() - 31_556_952).unwrap();
    let (trusted_key, _, roots, cas) = sized_fallback_cases(&pki, &other);
    let mut rx = VruDeviceRuntime::new(
        v2xw_core::ids::NodeId::new(3),
        v2xw_node::profiles::get(v2xw_node::profiles::GENERIC_VRU_DEVICE)
            .expect("the shipped VRU-device profile")
            .clone(),
        VruConfig {
            services: VruServices::SAE,
            wall: wall(),
            ..VruConfig::default()
        },
        0,
    );
    rx.stores_mut().crl.set_period(0);
    rx.set_belief(fix());
    rx.security_mut().install_trust(&roots, &cas);
    let gone = vec![Arc::new(other.root.cert.clone())];
    assert_eq!(rx.security_mut().apply_ctl(&gone, &[]), (1, 1));
    let reg = RngRegistry::new(62);
    let mut reports = Vec::new();
    for k in 0..300u64 {
        let mut ctx = NodeRuntimeCtx::new(10 * NS_PER_S + k * 10_000_000, &reg);
        let inbox = if k == 0 {
            vec![sized_frame(&trusted_key, MsgType::Cam)]
        } else {
            Vec::new()
        };
        reports.extend(rx.step(&mut ctx, inbox, 0.0).rx_reports);
    }
    let dispositions: Vec<RxDisposition> = reports.iter().map(|r| r.disposition).collect();
    println!("{dispositions:?}");
    assert_eq!(dispositions.len(), 1, "one report for the one frame");
    assert!(
        matches!(dispositions[0], RxDisposition::Refused(r) if r.name() == "chain-untrusted-root"),
        "the refused frame was reported as {:?}",
        dispositions[0]
    );
}

/// R-CCSIG on a VRU handset (06 §6 item 11): in the Partially-Hybrid mode a handset checks
/// the issuer's post-quantum signature once on each certificate it learns, as a vehicle
/// does (`VruConfig::cert_verify_op`), and the check is charged to the frame's
/// verification. Before this a handset priced only the classical message check. Fault that
/// turns it red: `VruDeviceRuntime::learn_or_request` not counting the learned
/// certificate.
#[test]
fn a_vru_handset_checks_each_learned_certificate_once_in_the_partially_hybrid_mode() {
    use v2xw_node::vru::{VruConfig, VruDeviceRuntime, VruServices};
    let pki = pki();
    let other = ScmsPki::new(8, t0() - 31_556_952).unwrap();
    let (trusted_key, _, roots, cas) = sized_fallback_cases(&pki, &other);
    let checks = |op: Option<&'static str>| {
        let mut rx = VruDeviceRuntime::new(
            v2xw_core::ids::NodeId::new(3),
            v2xw_node::profiles::get(v2xw_node::profiles::GENERIC_VRU_DEVICE)
                .expect("the shipped VRU-device profile")
                .clone(),
            VruConfig {
                services: VruServices::SAE,
                wall: wall(),
                cert_verify_op: op,
                ..VruConfig::default()
            },
            0,
        );
        rx.stores_mut().crl.set_period(0);
        rx.set_belief(fix());
        rx.security_mut().install_trust(&roots, &cas);
        let reg = RngRegistry::new(62);
        let mut charged = Vec::new();
        for k in 0..300u64 {
            let mut ctx = NodeRuntimeCtx::new(10 * NS_PER_S + k * 10_000_000, &reg);
            // The same signer twice with its certificate: learned once, checked once.
            let inbox = if k == 0 || k == 150 {
                vec![sized_frame(&trusted_key, MsgType::Cam)]
            } else {
                Vec::new()
            };
            charged.extend(
                rx.step(&mut ctx, inbox, 0.0)
                    .rx_reports
                    .iter()
                    .filter_map(|r| Some(r.verify_done? - r.verify_start?)),
            );
        }
        (rx.security().trust().stats.cert_pq_verifications, charged)
    };
    let (none, plain) = checks(None);
    let (once, hybrid) = checks(Some("ecdsa-p256-verify"));
    println!("checks {none} / {once}; service times {plain:?} / {hybrid:?}");
    assert_eq!(none, 0, "a check outside the Partially-Hybrid mode");
    assert_eq!(once, 1, "each learned certificate is checked once");
    assert!(
        hybrid.first() > plain.first(),
        "the certificate check was not charged to the frame's verification"
    );
}
