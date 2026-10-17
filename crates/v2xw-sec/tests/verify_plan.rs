//! The verification plan: what a receiver must actually do, and what it costs.
//!
//! `verify_plan` is the seam between the security layer and the node runtime
//! (03-interfaces.md §6): the envelope says which primitive operations a received SPDU
//! implies and which stores they hit, and the runtime charges them against the receiving
//! node's hardware profile and queues them. Nothing here verifies anything — that is the
//! point. A receiver under load has to decide *whether* to verify before it spends the
//! 15.3 ms a Cortex-M4 needs (04-models.md §9.4), and it cannot decide that from a
//! function that has already spent it.
//!
//! What the tests below pin is the plan's *shape*, because that shape is what the
//! simulated load is made of:
//!
//! * a digest signer whose certificate is unknown produces no cryptographic work at all —
//!   it produces a P2PCD request, which is the finding NDSS 2024 §IV-A turns into a
//!   bandwidth argument (99.3 % of received certificates are already known);
//! * an attached certificate that the cache has not verified adds a *second* signature
//!   verification, which is what makes certificate attachment expensive in CPU as well as
//!   in bytes;
//! * an implicit certificate replaces that verification with a reconstruction, which is
//!   the whole trade ECQV offers;
//! * a revoked or expired certificate produces no work at all.

mod common;

use common::{PSID_CAM, T0_UNIX, build_pki, device_linkage};
use std::sync::Arc;
use v2xw_core::time::WallClock;
use v2xw_msg::codec::MsgType;
use v2xw_sec::cert::{self, CertSpec, HolderId};
use v2xw_sec::crypto::{CryptoBackendInfo, Real};
use v2xw_sec::envelope::{
    CrlStore, Envelope, HashSubject, HeaderInfoSpec, P2pcdKind, PlanOutcome, PrimitiveOp,
    RejectReason, SecurityEnvelope, SecurityEnvelopeInfo, SigSubject, SignerHandle, SignerIdChoice,
    StoreKind,
};
use v2xw_sec::hashedid::{certificate_digest, hashed_id3_of_id8};
use v2xw_sec::linkage::{CrlLinkageEntry, DEFAULT_JMAX};
use v2xw_sec::primitive::{PrimitiveCatalogue, PrimitiveId, profiles};
use v2xw_sec::testctx::TestCtx;
use v2xw_sec::{PeerCertCache, TrustStore};

fn hdr() -> HeaderInfoSpec {
    HeaderInfoSpec {
        psid: PSID_CAM,
        msg_type: Some(MsgType::Cam),
        ..HeaderInfoSpec::default()
    }
}

/// A digest signer whose certificate the receiver does not hold: no crypto, one lookup,
/// and a P2PCD request naming the certificate by its low-order three bytes.
#[test]
fn an_unknown_digest_signer_plans_a_p2pcd_request_and_no_cryptography() {
    let mut ctx = TestCtx::new(1);
    let mut backend = Real::new();
    let pki = build_pki(&mut ctx, &mut backend, 1);
    let envelope = Envelope::ieee1609(WallClock::new(T0_UNIX));
    let e = &pki.entities[0];

    let pdu = envelope
        .sign(
            &mut ctx,
            &mut backend,
            &e.signer,
            b"payload",
            &hdr(),
            SignerIdChoice::Digest,
        )
        .expect("signs");
    let parsed = envelope.parse(pdu.bytes()).expect("parses");
    let plan = envelope.verify_plan(
        &parsed,
        &PeerCertCache::new(),
        &pki.trust_store(),
        &CrlStore::new(),
    );

    match &plan.outcome {
        PlanOutcome::NeedCertificate(req) => {
            assert_eq!(req.kind, P2pcdKind::EndEntity);
            assert_eq!(&req.unknown, e.signer.digest());
            assert_eq!(
                req.id3,
                hashed_id3_of_id8(e.signer.digest()),
                "the HashedId3 is a truncation of the HashedId8, not a second hash"
            );
        }
        other => panic!("expected a P2PCD request, got {other:?}"),
    }
    assert_eq!(
        plan.ops,
        vec![PrimitiveOp::StoreLookup {
            store: StoreKind::PeerCertCache,
            key: e.signer.digest().clone(),
        }],
        "an unresolvable SPDU must cost one lookup and nothing else"
    );
    assert!(!plan.is_verifiable());
    // A plan of one free lookup costs nothing, which is the point: a receiver can
    // discover it cannot verify without spending a point multiplication.
    assert_eq!(
        plan.cost(PrimitiveCatalogue::standard(), profiles::CORTEX_M4_NRF52840),
        Some(v2xw_core::time::Duration::ZERO)
    );
}

/// The same SPDU once the certificate is cached: two hashes and one signature
/// verification, in the order a receiver performs them.
#[test]
fn a_known_digest_signer_plans_two_hashes_and_one_verification() {
    let mut ctx = TestCtx::new(2);
    let mut backend = Real::new();
    let pki = build_pki(&mut ctx, &mut backend, 1);
    let envelope = Envelope::ieee1609(WallClock::new(T0_UNIX));
    let e = &pki.entities[0];

    let pdu = envelope
        .sign(
            &mut ctx,
            &mut backend,
            &e.signer,
            b"payload",
            &hdr(),
            SignerIdChoice::Digest,
        )
        .expect("signs");
    let parsed = envelope.parse(pdu.bytes()).expect("parses");
    let plan = envelope.verify_plan(
        &parsed,
        &pki.warm_cache(),
        &pki.trust_store(),
        &CrlStore::new(),
    );

    assert!(plan.is_verifiable());
    let verifications = plan
        .ops
        .iter()
        .filter(|o| matches!(o, PrimitiveOp::VerifySignature { .. }))
        .count();
    assert_eq!(
        verifications, 1,
        "a certificate the cache has already verified is not re-verified"
    );
    let hashes: Vec<HashSubject> = plan
        .ops
        .iter()
        .filter_map(|o| match o {
            PrimitiveOp::Hash { what, .. } => Some(*what),
            _ => None,
        })
        .collect();
    assert_eq!(
        hashes,
        vec![HashSubject::ToBeSignedData, HashSubject::SignerIdentifier],
        "both halves of the §5.3.1 digest are planned, in order"
    );
    assert!(matches!(
        plan.ops.last(),
        Some(PrimitiveOp::VerifySignature {
            over: SigSubject::Spdu,
            ..
        })
    ));

    // The cost is dominated by the one point multiplication, which is the number a
    // verification-rate study is actually about.
    let cost = plan
        .cost(PrimitiveCatalogue::standard(), profiles::CORTEX_M4_NRF52840)
        .expect("the profile has an ECDSA anchor");
    assert_eq!(
        cost,
        v2xw_core::time::Duration::from_micros(15_300),
        "one ECDSA P-256 verification on a Cortex-M4 at 64 MHz"
    );
    println!(
        "MEASURED plan: {} ops, {} µs on {}",
        plan.ops.len(),
        cost.as_nanos() / 1000,
        profiles::CORTEX_M4_NRF52840
    );
}

/// An attached certificate the receiver has not verified before: the certificate's own
/// signature is a second verification, so the SPDU costs twice as much CPU as one whose
/// signer it already knows.
#[test]
fn a_new_attached_certificate_costs_a_second_verification() {
    let mut ctx = TestCtx::new(3);
    let mut backend = Real::new();
    let pki = build_pki(&mut ctx, &mut backend, 1);
    let envelope = Envelope::ieee1609(WallClock::new(T0_UNIX));
    let e = &pki.entities[0];

    let pdu = envelope
        .sign(
            &mut ctx,
            &mut backend,
            &e.signer,
            b"payload",
            &hdr(),
            SignerIdChoice::Certificate,
        )
        .expect("signs");
    let parsed = envelope.parse(pdu.bytes()).expect("parses");

    // The issuer is known (the PCA is cached) but the end-entity certificate is not.
    let mut cache = PeerCertCache::new();
    cache.insert(pki.pca.clone(), true).expect("encodes");
    let cold = envelope.verify_plan(&parsed, &cache, &pki.trust_store(), &CrlStore::new());
    assert!(cold.is_verifiable());
    let subjects: Vec<SigSubject> = cold
        .ops
        .iter()
        .filter_map(|o| match o {
            PrimitiveOp::VerifySignature { over, .. } => Some(*over),
            _ => None,
        })
        .collect();
    assert_eq!(
        subjects,
        vec![SigSubject::Certificate, SigSubject::Spdu],
        "the certificate is validated before the SPDU it signed"
    );
    let cold_cost = cold
        .cost(PrimitiveCatalogue::standard(), profiles::CORTEX_M4_NRF52840)
        .expect("costed");

    // Once the receiver has verified it, the same SPDU costs one verification.
    let warm = envelope.verify_plan(
        &parsed,
        &pki.warm_cache(),
        &pki.trust_store(),
        &CrlStore::new(),
    );
    let warm_cost = warm
        .cost(PrimitiveCatalogue::standard(), profiles::CORTEX_M4_NRF52840)
        .expect("costed");
    assert_eq!(
        cold_cost,
        warm_cost + v2xw_core::time::Duration::from_micros(15_300),
        "learning a certificate costs exactly one extra verification"
    );
    println!(
        "MEASURED verification cost: unknown signer {} µs, known signer {} µs",
        cold_cost.as_nanos() / 1000,
        warm_cost.as_nanos() / 1000
    );
}

/// An implicit (ECQV) certificate replaces the certificate signature verification with a
/// reconstruction — and is 67 bytes smaller. That is the trade SEC 4 offers, in one test.
#[test]
fn an_implicit_certificate_plans_a_reconstruction_instead_of_a_verification() {
    let mut ctx = TestCtx::new(4);
    let mut backend = Real::new();
    let pki = build_pki(&mut ctx, &mut backend, 1);
    let envelope = Envelope::ieee1609(WallClock::new(T0_UNIX));
    let e = &pki.entities[0];

    // The same holder, the same period, as an implicit certificate.
    let material = cert::public_key_material(&e.certificate).expect("extracts");
    let dev = device_linkage(1);
    let spec = CertSpec::pseudonym(
        certificate_digest(&pki.pca).expect("digest"),
        HolderId::Linkage {
            i_cert: 7,
            linkage_value: dev.linkage_value_for(7, 0),
        },
        common::T0_1609_SECONDS,
        PSID_CAM,
    );
    let implicit = Arc::new(cert::implicit(&spec, &material).expect("builds"));
    let signer = SignerHandle::new(e.node, e.key, implicit.clone()).expect("signer");

    let pdu = envelope
        .sign(
            &mut ctx,
            &mut backend,
            &signer,
            b"payload",
            &hdr(),
            SignerIdChoice::Certificate,
        )
        .expect("signs");
    let parsed = envelope.parse(pdu.bytes()).expect("parses");
    let mut cache = PeerCertCache::new();
    cache.insert(pki.pca.clone(), true).expect("encodes");
    let plan = envelope.verify_plan(&parsed, &cache, &pki.trust_store(), &CrlStore::new());

    assert!(plan.is_verifiable());
    assert!(
        plan.ops.iter().any(|o| matches!(
            o,
            PrimitiveOp::ReconstructImplicit {
                primitive
            } if *primitive == PrimitiveId::ECQV_P256
        )),
        "an implicit certificate must plan a reconstruction: {:?}",
        plan.ops
    );
    assert!(
        !plan.ops.iter().any(|o| matches!(
            o,
            PrimitiveOp::VerifySignature {
                over: SigSubject::Certificate,
                ..
            }
        )),
        "there is no certificate signature to verify"
    );

    // And the size trade the whole construction is for.
    let explicit_size = cert::encoded_size(&e.certificate).expect("encodes");
    let implicit_size = cert::encoded_size(&implicit).expect("encodes");
    println!(
        "MEASURED certificate sizes: explicit {explicit_size} B, implicit {implicit_size} B \
         (saving {} B)",
        explicit_size - implicit_size
    );
    assert_eq!(explicit_size - implicit_size, 67);
}

/// An unknown *issuer* is a P2PCD request for a CA certificate, not for the end entity's.
#[test]
fn an_unknown_issuer_plans_a_p2pcd_request_for_the_authority() {
    let mut ctx = TestCtx::new(5);
    let mut backend = Real::new();
    let pki = build_pki(&mut ctx, &mut backend, 1);
    let envelope = Envelope::ieee1609(WallClock::new(T0_UNIX));
    let e = &pki.entities[0];

    let pdu = envelope
        .sign(
            &mut ctx,
            &mut backend,
            &e.signer,
            b"payload",
            &hdr(),
            SignerIdChoice::Certificate,
        )
        .expect("signs");
    let parsed = envelope.parse(pdu.bytes()).expect("parses");
    // Nothing cached and nothing trusted but the root, so the PCA is missing.
    let plan = envelope.verify_plan(
        &parsed,
        &PeerCertCache::new(),
        &pki.trust_store(),
        &CrlStore::new(),
    );
    match &plan.outcome {
        PlanOutcome::NeedCertificate(req) => {
            assert_eq!(req.kind, P2pcdKind::CertificateAuthority);
            assert_eq!(
                req.unknown,
                certificate_digest(&pki.pca).expect("digest"),
                "the request names the issuer, not the signer"
            );
        }
        other => panic!("expected a CA request, got {other:?}"),
    }
    // No signature work is planned: there is nothing to chain to yet.
    assert!(
        !plan
            .ops
            .iter()
            .any(|o| matches!(o, PrimitiveOp::VerifySignature { .. }))
    );
}

/// Revocation and expiry short-circuit the plan: no signature work at all.
#[test]
fn a_revoked_or_expired_certificate_plans_no_cryptography() {
    let mut ctx = TestCtx::new(6);
    let mut backend = Real::new();
    let pki = build_pki(&mut ctx, &mut backend, 1);
    let envelope = Envelope::ieee1609(WallClock::new(T0_UNIX));
    let e = &pki.entities[0];
    let anchors = pki.trust_store();
    let cache = pki.warm_cache();

    let pdu = envelope
        .sign(
            &mut ctx,
            &mut backend,
            &e.signer,
            b"payload",
            &hdr(),
            SignerIdChoice::Certificate,
        )
        .expect("signs");
    let parsed = envelope.parse(pdu.bytes()).expect("parses");

    // A hash CRL entry.
    let mut hash_crl = CrlStore::new();
    hash_crl
        .revoke_hash(v2xw_sec::hashedid::certificate_hashed_id10(&e.certificate).expect("hash"));
    let plan = envelope.verify_plan(&parsed, &cache, &anchors, &hash_crl);
    assert_eq!(plan.outcome, PlanOutcome::Reject(RejectReason::Revoked));
    assert!(
        !plan
            .ops
            .iter()
            .any(|o| matches!(o, PrimitiveOp::VerifySignature { .. }))
    );

    // A linkage CRL entry revoking the device from period 7 forward, and one revoking it
    // from period 8 — which must *not* match a period-7 certificate (backward privacy).
    let mut linked = CrlStore::new();
    linked.add_linkage_entry(CrlLinkageEntry::from_device(
        &device_linkage(1),
        7,
        DEFAULT_JMAX,
    ));
    assert_eq!(
        envelope
            .verify_plan(&parsed, &cache, &anchors, &linked)
            .outcome,
        PlanOutcome::Reject(RejectReason::Revoked)
    );

    let mut later = CrlStore::new();
    later.add_linkage_entry(CrlLinkageEntry::from_device(
        &device_linkage(1),
        8,
        DEFAULT_JMAX,
    ));
    assert!(
        envelope
            .verify_plan(&parsed, &cache, &anchors, &later)
            .is_verifiable(),
        "a certificate from before the revocation period must still verify — that is \
         backward privacy, and a verifier that matched it would break the property the \
         simulator is measuring"
    );

    // And a different device's CRL entry must not match this one.
    let mut other = CrlStore::new();
    other.add_linkage_entry(CrlLinkageEntry::from_device(
        &device_linkage(99),
        7,
        DEFAULT_JMAX,
    ));
    assert!(
        envelope
            .verify_plan(&parsed, &cache, &anchors, &other)
            .is_verifiable()
    );
}

/// A CRL entry whose index range is wider than the default must still catch a
/// certificate beyond index 20.
///
/// The reason [`CrlStore::revokes_linkage_at_period`] exists: a certificate's
/// `linkageData` carries the period and the value but not the index, so the verifier
/// searches — and if it searched a fixed range instead of each entry's own, a device
/// issued more than 20 certificates per period would be revoked on paper and unrevoked in
/// the simulation, which is exactly the kind of silent wrong answer a revocation study
/// cannot survive.
#[test]
fn a_crl_entry_with_a_wide_index_range_still_catches_a_high_index() {
    let mut ctx = TestCtx::new(8);
    let mut backend = Real::new();
    let pki = build_pki(&mut ctx, &mut backend, 1);
    let envelope = Envelope::ieee1609(WallClock::new(T0_UNIX));
    let e = &pki.entities[0];
    let dev = device_linkage(1);

    // A certificate at index 40, well beyond the default range of 20.
    let material = cert::public_key_material(&e.certificate).expect("extracts");
    let spec = CertSpec::pseudonym(
        certificate_digest(&pki.pca).expect("digest"),
        HolderId::Linkage {
            i_cert: 7,
            linkage_value: dev.linkage_value_for(7, 40),
        },
        common::T0_1609_SECONDS,
        PSID_CAM,
    );
    let high = Arc::new(cert::implicit(&spec, &material).expect("builds"));
    let signer = SignerHandle::new(e.node, e.key, high.clone()).expect("signer");
    let pdu = envelope
        .sign(
            &mut ctx,
            &mut backend,
            &signer,
            b"payload",
            &hdr(),
            SignerIdChoice::Certificate,
        )
        .expect("signs");
    let parsed = envelope.parse(pdu.bytes()).expect("parses");
    let mut cache = PeerCertCache::new();
    cache.insert(pki.pca.clone(), true).expect("encodes");

    // The default range misses it — correctly, because the entry says it only covers 20.
    let mut narrow = CrlStore::new();
    narrow.add_linkage_entry(CrlLinkageEntry::from_device(&dev, 7, DEFAULT_JMAX));
    assert!(
        envelope
            .verify_plan(&parsed, &cache, &pki.trust_store(), &narrow)
            .is_verifiable(),
        "an entry covering 20 indices must not claim to revoke index 40"
    );

    // An entry that declares the wider range does catch it.
    let mut wide = CrlStore::new();
    wide.add_linkage_entry(CrlLinkageEntry::from_device(&dev, 7, 64));
    assert_eq!(
        envelope
            .verify_plan(&parsed, &cache, &pki.trust_store(), &wide)
            .outcome,
        PlanOutcome::Reject(RejectReason::Revoked),
        "an entry covering 64 indices must revoke index 40"
    );
}

/// An SPDU that is not `signedData`, and one whose signer is `self`, are refused rather
/// than planned.
#[test]
fn an_unsigned_or_self_signed_spdu_is_refused() {
    let envelope = Envelope::ieee1609(WallClock::new(T0_UNIX));

    // `unsecuredData` in the outer `Ieee1609Dot2Data`: not an SPDU this envelope handles.
    let plain = v2xw_msg::sec_types::Ieee1609Dot2Data::new(
        v2xw_msg::sec_types::Uint8(3),
        v2xw_msg::sec_types::Ieee1609Dot2Content::unsecuredData(v2xw_msg::sec_types::Opaque(
            rasn::types::OctetString::from_slice(b"just a payload"),
        )),
    );
    let bytes = v2xw_msg::sec_types::coer::encode(MsgType::Cam, &plain).expect("encodes");
    let err = envelope.parse(&bytes).expect_err("must refuse");
    assert!(
        matches!(err, v2xw_sec::SecError::NotSignedData { .. }),
        "{err}"
    );

    // Garbage.
    assert!(envelope.parse(&[0xff, 0xff, 0xff]).is_err());
    assert!(envelope.parse(&[]).is_err());
}

/// A self-signed end-entity certificate is only usable if it is itself a trust anchor.
#[test]
fn a_self_issued_signer_needs_to_be_a_trust_anchor() {
    let mut ctx = TestCtx::new(7);
    let mut backend = Real::new();
    let envelope = Envelope::ieee1609(WallClock::new(T0_UNIX));

    let key = {
        use v2xw_sec::crypto::CryptoBackend;
        backend
            .keygen(
                &mut ctx,
                PrimitiveId::ECDSA_P256_SHA256,
                v2xw_core::ids::NodeId::new(5),
            )
            .expect("keygen")
    };
    let material = backend
        .public_material(&backend.public_of(&key).expect("pub"))
        .expect("material");
    let rogue = Arc::new(
        cert::trust_anchor(
            &CertSpec::authority(common::T0_1609_SECONDS, PSID_CAM),
            &material,
        )
        .expect("builds"),
    );
    let signer =
        SignerHandle::new(v2xw_core::ids::NodeId::new(5), key, rogue.clone()).expect("signer");
    let pdu = envelope
        .sign(
            &mut ctx,
            &mut backend,
            &signer,
            b"payload",
            &hdr(),
            SignerIdChoice::Certificate,
        )
        .expect("signs");
    let parsed = envelope.parse(pdu.bytes()).expect("parses");

    // Not trusted: refused, with no signature work.
    let plan = envelope.verify_plan(
        &parsed,
        &PeerCertCache::new(),
        &TrustStore::new(),
        &CrlStore::new(),
    );
    assert_eq!(
        plan.outcome,
        PlanOutcome::Reject(RejectReason::UntrustedSelfSignedIssuer)
    );

    // Trusted: verifiable.
    let mut anchors = TrustStore::new();
    anchors.insert(rogue).expect("encodes");
    assert!(
        envelope
            .verify_plan(&parsed, &PeerCertCache::new(), &anchors, &CrlStore::new())
            .is_verifiable()
    );
}

// ------------------------------------------------------------------------------------
// The multi-level chain walk (WP-A3): verify_plan used to stop one level above the
// signer; it now walks to a trust anchor or a verified cache entry, one link operation
// per unverified certificate, and applies the non-cryptographic chain rules.

fn attached(seed: u64, psid: u64) -> (common::Pki, Envelope, v2xw_sec::ParsedSecured) {
    let mut ctx = TestCtx::new(seed);
    let mut backend = Real::new();
    let pki = build_pki(&mut ctx, &mut backend, 1);
    let envelope = Envelope::ieee1609(WallClock::new(T0_UNIX));
    let h = HeaderInfoSpec { psid, ..hdr() };
    let pdu = envelope
        .sign(
            &mut ctx,
            &mut backend,
            &pki.entities[0].signer,
            b"payload",
            &h,
            SignerIdChoice::Certificate,
        )
        .expect("signs");
    let parsed = envelope.parse(pdu.bytes()).expect("parses");
    (pki, envelope, parsed)
}

fn cert_links(plan: &v2xw_sec::VerifyPlan) -> usize {
    plan.ops
        .iter()
        .filter(|o| {
            matches!(
                o,
                PrimitiveOp::VerifySignature {
                    over: SigSubject::Certificate,
                    ..
                } | PrimitiveOp::ReconstructImplicit { .. }
            )
        })
        .count()
}

#[test]
fn an_unverified_cached_issuer_is_itself_verified_up_to_the_anchor() {
    let (pki, envelope, parsed) = attached(31, PSID_CAM);
    // The PCA is cached but not yet verified: the plan must verify the EE against it
    // and the PCA against the root — two links, not the old one.
    let mut cache = PeerCertCache::new();
    cache.insert(pki.pca.clone(), false).expect("encodes");
    let plan = envelope.verify_plan(&parsed, &cache, &pki.trust_store(), &CrlStore::new());
    assert_eq!(plan.outcome, PlanOutcome::Verifiable);
    assert_eq!(cert_links(&plan), 2, "{:?}", plan.ops);
    // Verified in the cache: one link, as before.
    let mut warm = PeerCertCache::new();
    warm.insert(pki.pca.clone(), true).expect("encodes");
    let plan = envelope.verify_plan(&parsed, &warm, &pki.trust_store(), &CrlStore::new());
    assert_eq!(plan.outcome, PlanOutcome::Verifiable);
    assert_eq!(cert_links(&plan), 1);
}

#[test]
fn a_missing_certificate_two_levels_up_is_requested_by_p2pcd() {
    let (pki, envelope, parsed) = attached(32, PSID_CAM);
    let mut cache = PeerCertCache::new();
    cache.insert(pki.pca.clone(), false).expect("encodes");
    // No trust anchors: the walk reaches the PCA's issuer, the root, and lacks it.
    let plan = envelope.verify_plan(&parsed, &cache, &TrustStore::new(), &CrlStore::new());
    match &plan.outcome {
        PlanOutcome::NeedCertificate(req) => {
            assert_eq!(req.kind, P2pcdKind::CertificateAuthority);
            assert_eq!(req.unknown, pki.root_digest, "the root, two levels up");
        }
        other => panic!("expected a request for the root, got {other:?}"),
    }
}

#[test]
fn an_spdu_under_a_psid_the_certificate_lacks_is_refused() {
    let (pki, envelope, parsed) = attached(33, PSID_CAM + 1);
    let plan = envelope.verify_plan(
        &parsed,
        &pki.warm_cache(),
        &pki.trust_store(),
        &CrlStore::new(),
    );
    assert_eq!(
        plan.outcome,
        PlanOutcome::Reject(RejectReason::PsidNotPermitted)
    );
}

#[test]
fn an_issuer_without_cert_issue_permissions_is_inconsistent() {
    use v2xw_sec::crypto::CryptoBackend;
    let mut ctx = TestCtx::new(34);
    let mut backend = Real::new();
    let envelope = Envelope::ieee1609(WallClock::new(T0_UNIX));
    let p = PrimitiveId::ECDSA_P256_SHA256;
    for issuing in [true, false] {
        let rk = backend
            .keygen(&mut ctx, p, v2xw_core::ids::NodeId::new(800))
            .expect("keygen");
        let rm = backend
            .public_material(&backend.public_of(&rk).expect("pub"))
            .expect("material");
        let spec = if issuing {
            CertSpec::issuing_authority(common::T0_1609_SECONDS, PSID_CAM)
        } else {
            CertSpec::authority(common::T0_1609_SECONDS, PSID_CAM)
        };
        let root = Arc::new(cert::trust_anchor(&spec, &rm).expect("root"));
        let root_coer = cert::encode(&root).expect("encodes");
        let ek = backend
            .keygen(&mut ctx, p, v2xw_core::ids::NodeId::new(801))
            .expect("keygen");
        let em = backend
            .public_material(&backend.public_of(&ek).expect("pub"))
            .expect("material");
        let ee_spec = CertSpec::pseudonym(
            certificate_digest(&root).expect("digest"),
            HolderId::None,
            common::T0_1609_SECONDS,
            PSID_CAM,
        );
        let ee = Arc::new(
            cert::issue_explicit(&ee_spec, &em, &root_coer, |d| {
                backend
                    .sign_prehashed(&mut ctx, &rk, d)?
                    .to_ieee1609_signature()
            })
            .expect("ee"),
        );
        let signer = SignerHandle::new(v2xw_core::ids::NodeId::new(801), ek, ee).expect("signer");
        let pdu = envelope
            .sign(
                &mut ctx,
                &mut backend,
                &signer,
                b"payload",
                &hdr(),
                SignerIdChoice::Certificate,
            )
            .expect("signs");
        let parsed = envelope.parse(pdu.bytes()).expect("parses");
        let mut anchors = TrustStore::new();
        anchors.insert(root).expect("encodes");
        let plan = envelope.verify_plan(&parsed, &PeerCertCache::new(), &anchors, &CrlStore::new());
        if issuing {
            assert_eq!(plan.outcome, PlanOutcome::Verifiable);
        } else {
            assert_eq!(
                plan.outcome,
                PlanOutcome::Reject(RejectReason::ChainInconsistent)
            );
        }
    }
}
