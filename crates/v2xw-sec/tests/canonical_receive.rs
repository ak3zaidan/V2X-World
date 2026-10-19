//! Crypto-formats validation (docs/design/scms-real/validation/crypto-formats.md, CF-CANON
//! and CF-EXPIRY): the receive side of the 1609.2 canonicalisation and header rules.
//!
//! * A sender may transmit an elliptic-curve point compressed *or uncompressed*, and an
//!   ECDSA `rSig` as x-only *or* as the point R, "at the sender's discretion"
//!   ([ASN-2] `EccP256CurvePoint`, `EcdsaP256Signature` notes). Every hash of a
//!   certificate (its HashedId8/10, the signer-identifier input of §5.3.1) is over the
//!   canonical form: points compressed, `rSig` x-only ([ASN-2] `CertificateBase` note
//!   "the canonicalization applies to the ToBeSignedCertificate and to the Signature";
//!   02 §3). A receiver that hashed the bytes as transmitted names the same certificate
//!   by a different HashedId8, misses it in its caches and on a hash CRL, and computes a
//!   different signing digest.
//! * `HeaderInfo.expiryTime`: "If both generationTime and expiryTime are present, the
//!   signed SPDU is invalid if generationTime is not strictly earlier than expiryTime"
//!   ([ASN-2] `HeaderInfo` note).
//!
//! The published-chain test runs whenever the artefact cache of
//! `tools/scms-artefacts/fetch.py` is present (owner decision 4) and says so otherwise.

mod common;

use std::sync::Arc;

use common::{PSID_CAM, T0_UNIX, build_pki};
use v2xw_core::time::{Duration, WallClock};
use v2xw_msg::codec::MsgType;
use v2xw_msg::sec_types::ieee1609_dot2::VerificationKeyIndicator;
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    EccP256CurvePointUncompressedP256, PublicVerificationKey, Uint64,
};
use v2xw_msg::sec_types::{
    Certificate, EccP256CurvePoint, Ieee1609Dot2Content, Ieee1609Dot2Data, IssuerIdentifier,
    Signature, Time64,
};
use v2xw_sec::cert;
use v2xw_sec::crypto::Real;
use v2xw_sec::ec::Point;
use v2xw_sec::envelope::{
    CrlStore, Envelope, HeaderInfoSpec, PeerCertCache, SecurityEnvelope, SecurityEnvelopeInfo,
    SignerHandle, SignerIdChoice,
};
use v2xw_sec::hashedid::{certificate_digest, certificate_hashed_id10};
use v2xw_sec::testctx::TestCtx;

/// The same certificate as a peer may legitimately transmit it: the verification key
/// uncompressed and the signature's `rSig` as `compressed-y-0` (the R form).
fn transmitted_form(c: &Certificate) -> Certificate {
    let mut n = c.clone();
    if let VerificationKeyIndicator::verificationKey(PublicVerificationKey::ecdsaNistP256(p)) =
        &mut n.0.to_be_signed.verify_key_indicator
    {
        let sec1: Vec<u8> = match &*p {
            EccP256CurvePoint::compressed_y_0(x) => [&[0x02u8][..], x].concat(),
            EccP256CurvePoint::compressed_y_1(x) => [&[0x03u8][..], x].concat(),
            other => panic!("the canonical key is compressed, not {other:?}"),
        };
        let (x, y) = Point::from_sec1(&sec1)
            .expect("on the curve")
            .xy()
            .expect("affine");
        *p = EccP256CurvePoint::uncompressedP256(EccP256CurvePointUncompressedP256::new(
            x.to_vec().into(),
            y.to_vec().into(),
        ));
    }
    if let Some(Signature::ecdsaNistP256Signature(s)) = &mut n.0.signature
        && let EccP256CurvePoint::x_only(x) = &s.r_sig
    {
        s.r_sig = EccP256CurvePoint::compressed_y_0(x.clone());
    }
    n
}

fn id10(c: &Certificate) -> [u8; 10] {
    let mut o = [0u8; 10];
    o.copy_from_slice(&certificate_hashed_id10(c).expect("id10").0[..]);
    o
}

/// The transmitted form is a different byte string with the canonical identity, and every
/// receive-side store (peer cache, hash CRL, signer handle) treats it as that certificate.
#[test]
fn a_transmitted_non_canonical_certificate_keeps_its_canonical_identity() {
    let mut ctx = TestCtx::new(31);
    let mut backend = Real::new();
    let pki = build_pki(&mut ctx, &mut backend, 1);
    let ee = &pki.entities[0];
    for (what, c) in [
        ("root", pki.root.as_ref()),
        ("pca", pki.pca.as_ref()),
        ("ee", ee.certificate.as_ref()),
    ] {
        let t = transmitted_form(c);
        assert_ne!(
            cert::encode(&t).unwrap(),
            cert::encode(c).unwrap(),
            "{what}: the transmitted form is a different encoding"
        );
        assert_eq!(
            cert::canonical_coer(&t).unwrap(),
            cert::encode(c).unwrap(),
            "{what}"
        );
        assert_eq!(
            certificate_digest(&t).unwrap(),
            cert::cert_id8(c).unwrap(),
            "{what}: HashedId8 is over the canonical encoding (1609.2 6.1.2)"
        );
        assert_eq!(id10(&t), cert::cert_id10(c).unwrap(), "{what}: HashedId10");
    }

    let t = transmitted_form(&ee.certificate);
    let mut cache = PeerCertCache::new();
    assert_eq!(
        cache.insert(Arc::new(t.clone()), false).unwrap(),
        *ee.signer.digest(),
        "the peer cache files it under the HashedId8 its signer uses"
    );
    let mut crl = CrlStore::new();
    crl.revoke_hash(certificate_hashed_id10(&ee.certificate).unwrap());
    assert!(
        crl.revokes_certificate(&t).unwrap(),
        "a hash CRL entry for the certificate revokes its transmitted form"
    );
    let h = SignerHandle::new(ee.node, ee.key, Arc::new(t)).unwrap();
    assert_eq!(
        h.cert_coer(),
        ee.signer.cert_coer(),
        "the signer-identifier input is the canonical certificate"
    );
    assert_eq!(h.digest(), ee.signer.digest());
}

/// The same on SCMS Manager's published production chain (2025-08-13): each certificate's
/// transmitted form hashes to the published certificate's HashedId8, the value its
/// subordinates name as their issuer.
#[test]
fn the_published_chain_keeps_its_issuer_links_in_transmitted_form() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/scms-artefacts/files/production-2025-08-13/cert-chain_0.oer");
    let Ok(bytes) = std::fs::read(&p) else {
        eprintln!(
            "SKIPPED: {} absent; run python3 -I tools/scms-artefacts/fetch.py",
            p.display()
        );
        return;
    };
    let (_, others) = v2xw_sec::ctl::decode_certificate_chain(&bytes).expect("decodes");
    let certs = &others;
    assert_eq!(certs.len(), 6, "root, ICA, ECA, RA and two ACAs");
    let mut links = 0;
    for c in certs {
        let id = certificate_digest(c).unwrap();
        assert_eq!(certificate_digest(&transmitted_form(c)).unwrap(), id);
        for s in certs {
            if let IssuerIdentifier::sha256AndDigest(d) = &s.0.issuer
                && *d == id
            {
                links += 1;
            }
        }
    }
    assert_eq!(
        links, 5,
        "every non-root certificate names an issuer in the file"
    );
}

/// `generationTime` not strictly before `expiryTime` makes the SPDU invalid; a sender
/// cannot produce one and a receiver refuses one.
#[test]
fn an_spdu_that_expires_at_or_before_its_generation_is_invalid() {
    let mut ctx = TestCtx::new(32);
    let mut backend = Real::new();
    let pki = build_pki(&mut ctx, &mut backend, 1);
    let env = Envelope::ieee1609(WallClock::new(T0_UNIX));
    let e = &pki.entities[0];
    let spec = |expiry| HeaderInfoSpec {
        psid: PSID_CAM,
        msg_type: Some(MsgType::Cam),
        expiry,
        ..HeaderInfoSpec::default()
    };
    let ok = env
        .sign(
            &mut ctx,
            &mut backend,
            &e.signer,
            b"payload",
            &spec(Some(Duration::from_secs(1))),
            SignerIdChoice::Digest,
        )
        .expect("an SPDU that expires a second after it is generated signs");
    env.parse(ok.bytes()).expect("and parses");
    assert!(
        env.sign(
            &mut ctx,
            &mut backend,
            &e.signer,
            b"payload",
            &spec(Some(Duration::ZERO)),
            SignerIdChoice::Digest,
        )
        .is_err(),
        "a sender refuses an SPDU whose expiryTime equals its generationTime"
    );
    for delta in [0i64, -1] {
        let mut d: Ieee1609Dot2Data = rasn::coer::decode(ok.bytes()).unwrap();
        let Ieee1609Dot2Content::signedData(s) = &mut d.content else {
            panic!("signed");
        };
        let h = &mut s.tbs_data.header_info;
        let g = h.generation_time.as_ref().unwrap().0.0;
        h.expiry_time = Some(Time64(Uint64(g.checked_add_signed(delta).unwrap())));
        let bytes = rasn::coer::encode(&d).unwrap();
        assert!(
            env.parse(&bytes).is_err(),
            "expiryTime = generationTime {delta:+} us is refused (1609.2 HeaderInfo)"
        );
    }
}
