//! IEEE 1609.2.1 SCMS types: wire edition rules and 1609.2 canonicalisation.
//!
//! The generated types live in [`crate::sec_types`] (one generation unit, so a
//! `Certificate` inside an `EeRaCertRequest` is the same Rust type `v2xw-sec` builds).
//! This module adds the two things the generated types cannot express:
//!
//! * **Which edition the bytes follow** ([`WireEdition`], owner decision 1, provisional:
//!   1609.2-2022 / 1609.2.1-2022 by default, 2025 selectable). The generated types are the
//!   2025 superset; [`check_tbs_certificate`], [`conform_tbs_certificate`],
//!   [`check_crl_contents`] and [`check_countersignature`] apply the three code-level
//!   differences between the editions (pinned by a test against the committed modules).
//! * **Canonicalisation before hashing** (IEEE 1609.2 §6.1.2 and the `Canonicalization`
//!   notes in the ASN.1, quoted at each function): every elliptic-curve point in a public
//!   verification key, a reconstruction value or a public encryption key is encoded
//!   compressed (`compressed-y-0` / `compressed-y-1`); the `rSig` of an ECDSA signature is
//!   `x-only`. A HashedId8 or a signature is computed over the COER encoding *after* these
//!   rules (02 §3), so a structure that is not canonical hashes to the wrong value.

use crate::sec_types::ieee1609_dot2::HeaderInfo;
use crate::sec_types::ieee1609_dot2::{
    Certificate, Ieee1609Dot2Content, Ieee1609Dot2Data, SequenceOfAppExtensions,
    SequenceOfCertIssueExtensions, SequenceOfCertRequestExtensions, SignedData, SignerIdentifier,
    ToBeSignedCertificate, ToBeSignedData, VerificationKeyIndicator,
};
use crate::sec_types::ieee1609_dot2_base_types::{
    BasePublicEncryptionKey, EccP256CurvePoint, EccP384CurvePoint, EncryptionKey,
    PublicEncryptionKey, PublicVerificationKey, Signature,
};
use crate::sec_types::ieee1609_dot2_crl_base_types::{CrlContents, TypeSpecificCrlContents};

/// The edition of IEEE 1609.2 / 1609.2.1 the wire bytes follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WireEdition {
    /// IEEE 1609.2-2022 with 1609.2.1-2022: what SCMS Manager's 1609.2.1 profile v1.0.3
    /// profiles and what the US deployment runs (02 §1, 03 §0). The default.
    #[default]
    Ieee2022,
    /// IEEE 1609.2-2025 (with 1609.2.1-2022, which imports it `WITH SUCCESSORS`).
    Ieee2025,
}

/// A value that the chosen edition does not allow, or that cannot be canonicalised.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScmsTypeError {
    /// 1609.2-2022 `ToBeSignedCertificate`: once any extension addition is present,
    /// `appExtensions`, `certIssueExtensions` and `certRequestExtension` are mandatory.
    #[error(
        "1609.2-2022 ToBeSignedCertificate uses its extensions but omits `{0}`, which that edition makes mandatory"
    )]
    MissingMandatoryExtension(&'static str),
    /// `CrlContents` carries `iPeriodInfo`, which only 1609.2-2025 defines.
    #[error(
        "`iPeriodInfo` in a CRL is a 1609.2-2025 field; the 2022 edition has no such component"
    )]
    IPeriodInfoIn2022,
    /// The value breaks the `Countersignature` subtype constraint of the edition.
    #[error("not a valid Countersignature for this edition: {0}")]
    Countersignature(&'static str),
    /// A key point is `x-only` or `fill`: it cannot be put in compressed form without
    /// recovering y, which is curve arithmetic this layer does not do.
    #[error("a public key point in `{0}` form cannot be canonicalised (compressed form needs y)")]
    NotCompressible(&'static str),
    /// `rSig` is `fill`, which has no x coordinate.
    #[error("the signature's rSig is `fill`; it has no x coordinate to keep")]
    RSigFill,
}

type Result<T> = core::result::Result<T, ScmsTypeError>;

// ---------------------------------------------------------------------------------------
// Editions
// ---------------------------------------------------------------------------------------

/// Checks a `ToBeSignedCertificate` against the edition's rules.
///
/// 1609.2-2022 declares `appExtensions`, `certIssueExtensions` and `certRequestExtension`
/// after the extension marker *without* `OPTIONAL`; 1609.2-2025 makes them `OPTIONAL`
/// (02 §1). So a 2022 value that uses the extension (any of `flags` or the three present)
/// must carry all three. A value with no extension at all is valid in both editions and
/// encodes identically.
pub fn check_tbs_certificate(tbs: &ToBeSignedCertificate, edition: WireEdition) -> Result<()> {
    if edition == WireEdition::Ieee2025 || !uses_extensions(tbs) {
        return Ok(());
    }
    if tbs.app_extensions.is_none() {
        return Err(ScmsTypeError::MissingMandatoryExtension("appExtensions"));
    }
    if tbs.cert_issue_extensions.is_none() {
        return Err(ScmsTypeError::MissingMandatoryExtension(
            "certIssueExtensions",
        ));
    }
    if tbs.cert_request_extension.is_none() {
        return Err(ScmsTypeError::MissingMandatoryExtension(
            "certRequestExtension",
        ));
    }
    Ok(())
}

/// Makes a `ToBeSignedCertificate` valid for the edition: under 2022, if the extension
/// is used, absent mandatory extension fields become empty sequences. Under 2025 nothing
/// changes. Call before encoding a certificate for signing.
pub fn conform_tbs_certificate(tbs: &mut ToBeSignedCertificate, edition: WireEdition) {
    if edition == WireEdition::Ieee2022 && uses_extensions(tbs) {
        tbs.app_extensions
            .get_or_insert_with(|| SequenceOfAppExtensions(Vec::new()));
        tbs.cert_issue_extensions
            .get_or_insert_with(|| SequenceOfCertIssueExtensions(Vec::new()));
        tbs.cert_request_extension
            .get_or_insert_with(|| SequenceOfCertRequestExtensions(Vec::new()));
    }
}

fn uses_extensions(tbs: &ToBeSignedCertificate) -> bool {
    tbs.flags.is_some()
        || tbs.app_extensions.is_some()
        || tbs.cert_issue_extensions.is_some()
        || tbs.cert_request_extension.is_some()
}

/// Checks `CrlContents` against the edition: `iPeriodInfo` exists only in 2025.
pub fn check_crl_contents(crl: &CrlContents, edition: WireEdition) -> Result<()> {
    let linked = match &crl.type_specific {
        TypeSpecificCrlContents::fullLinkedCrl(l) | TypeSpecificCrlContents::deltaLinkedCrl(l) => {
            Some(l)
        }
        _ => None,
    };
    if edition == WireEdition::Ieee2022 && linked.is_some_and(|l| l.i_period_info.is_some()) {
        return Err(ScmsTypeError::IPeriodInfoIn2022);
    }
    Ok(())
}

/// Checks a `Countersignature` (an `Ieee1609Dot2Data` of form signedData whose payload has
/// `data ABSENT, extDataHash PRESENT`, plus `omitted ABSENT` in 2025).
pub fn check_countersignature(cs: &Ieee1609Dot2Data, edition: WireEdition) -> Result<()> {
    let Ieee1609Dot2Content::signedData(sd) = &cs.content else {
        return Err(ScmsTypeError::Countersignature("content is not signedData"));
    };
    let payload = &sd.tbs_data.payload;
    if payload.data.is_some() {
        return Err(ScmsTypeError::Countersignature(
            "payload.data must be absent",
        ));
    }
    if payload.ext_data_hash.is_none() {
        return Err(ScmsTypeError::Countersignature(
            "payload.extDataHash must be present",
        ));
    }
    if edition == WireEdition::Ieee2025 && payload.omitted.is_some() {
        return Err(ScmsTypeError::Countersignature(
            "payload.omitted must be absent (2025)",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------
// Canonicalisation
// ---------------------------------------------------------------------------------------

/// The compressed form of a P-256 (or Brainpool P-256) point.
///
/// SEC 1 §2.3.3: the compressed point keeps x and the parity of y (`compressed-y-0` for
/// even y, `compressed-y-1` for odd y). An uncompressed point is converted; a compressed
/// one is kept; `x-only` and `fill` cannot be (y is unknown).
pub fn compressed_p256(p: &EccP256CurvePoint) -> Result<EccP256CurvePoint> {
    Ok(match p {
        EccP256CurvePoint::compressed_y_0(_) | EccP256CurvePoint::compressed_y_1(_) => p.clone(),
        EccP256CurvePoint::uncompressedP256(u) => {
            if u.y.last().is_some_and(|b| b & 1 == 1) {
                EccP256CurvePoint::compressed_y_1(u.x.clone())
            } else {
                EccP256CurvePoint::compressed_y_0(u.x.clone())
            }
        }
        EccP256CurvePoint::x_only(_) => return Err(ScmsTypeError::NotCompressible("x-only")),
        EccP256CurvePoint::fill(()) => return Err(ScmsTypeError::NotCompressible("fill")),
    })
}

/// The compressed form of a P-384 (or Brainpool P-384) point; as [`compressed_p256`].
pub fn compressed_p384(p: &EccP384CurvePoint) -> Result<EccP384CurvePoint> {
    Ok(match p {
        EccP384CurvePoint::compressed_y_0(_) | EccP384CurvePoint::compressed_y_1(_) => p.clone(),
        EccP384CurvePoint::uncompressedP384(u) => {
            if u.y.last().is_some_and(|b| b & 1 == 1) {
                EccP384CurvePoint::compressed_y_1(u.x.clone())
            } else {
                EccP384CurvePoint::compressed_y_0(u.x.clone())
            }
        }
        EccP384CurvePoint::x_only(_) => return Err(ScmsTypeError::NotCompressible("x-only")),
        EccP384CurvePoint::fill(()) => return Err(ScmsTypeError::NotCompressible("fill")),
    })
}

/// `PublicVerificationKey` note: "Both forms of point are encoded in compressed form".
pub fn canonicalize_public_verification_key(k: &mut PublicVerificationKey) -> Result<()> {
    match k {
        PublicVerificationKey::ecdsaNistP256(p)
        | PublicVerificationKey::ecdsaBrainpoolP256r1(p)
        | PublicVerificationKey::ecsigSm2(p) => *p = compressed_p256(p)?,
        PublicVerificationKey::ecdsaNistP384(p)
        | PublicVerificationKey::ecdsaBrainpoolP384r1(p) => {
            *p = compressed_p384(p)?;
        }
    }
    Ok(())
}

/// `PublicEncryptionKey`: "the elliptic curve point is encoded in compressed form".
pub fn canonicalize_public_encryption_key(k: &mut PublicEncryptionKey) -> Result<()> {
    match &mut k.public_key {
        BasePublicEncryptionKey::eciesNistP256(p)
        | BasePublicEncryptionKey::eciesBrainpoolP256r1(p)
        | BasePublicEncryptionKey::ecencSm2(p) => *p = compressed_p256(p)?,
    }
    Ok(())
}

/// `EcdsaP256Signature` / `EcdsaP384Signature`: "the EccP256CurvePoint in rSig is
/// represented in the form x-only". SM2 signatures carry `rSig` as an octet string and are
/// left alone.
pub fn canonicalize_signature(s: &mut Signature) -> Result<()> {
    match s {
        Signature::ecdsaNistP256Signature(sig) | Signature::ecdsaBrainpoolP256r1Signature(sig) => {
            sig.r_sig = match &sig.r_sig {
                EccP256CurvePoint::x_only(x)
                | EccP256CurvePoint::compressed_y_0(x)
                | EccP256CurvePoint::compressed_y_1(x) => EccP256CurvePoint::x_only(x.clone()),
                EccP256CurvePoint::uncompressedP256(u) => EccP256CurvePoint::x_only(u.x.clone()),
                EccP256CurvePoint::fill(()) => return Err(ScmsTypeError::RSigFill),
            };
        }
        Signature::ecdsaNistP384Signature(sig) | Signature::ecdsaBrainpoolP384r1Signature(sig) => {
            sig.r_sig = match &sig.r_sig {
                EccP384CurvePoint::x_only(x)
                | EccP384CurvePoint::compressed_y_0(x)
                | EccP384CurvePoint::compressed_y_1(x) => EccP384CurvePoint::x_only(x.clone()),
                EccP384CurvePoint::uncompressedP384(u) => EccP384CurvePoint::x_only(u.x.clone()),
                EccP384CurvePoint::fill(()) => return Err(ScmsTypeError::RSigFill),
            };
        }
        Signature::sm2Signature(_) => {}
    }
    Ok(())
}

/// `ToBeSignedCertificate`: applies to the `PublicEncryptionKey` and the
/// `VerificationKeyIndicator` (whose reconstruction value is also compressed).
pub fn canonicalize_tbs_certificate(tbs: &mut ToBeSignedCertificate) -> Result<()> {
    match &mut tbs.verify_key_indicator {
        VerificationKeyIndicator::verificationKey(k) => canonicalize_public_verification_key(k)?,
        VerificationKeyIndicator::reconstructionValue(p) => *p = compressed_p256(p)?,
    }
    if let Some(k) = &mut tbs.encryption_key {
        canonicalize_public_encryption_key(k)?;
    }
    Ok(())
}

/// `CertificateBase`: applies to the `ToBeSignedCertificate` and to the `Signature`.
pub fn canonicalize_certificate(cert: &mut Certificate) -> Result<()> {
    canonicalize_tbs_certificate(&mut cert.0.to_be_signed)?;
    if let Some(sig) = &mut cert.0.signature {
        canonicalize_signature(sig)?;
    }
    Ok(())
}

/// `HeaderInfo`: applies to `encryptionKey` when it is a public key. "The
/// canonicalization does not apply to any fields after the extension marker", so a
/// `requestedCertificate` is left exactly as received.
pub fn canonicalize_header_info(h: &mut HeaderInfo) -> Result<()> {
    if let Some(EncryptionKey::public(k)) = &mut h.encryption_key {
        canonicalize_public_encryption_key(k)?;
    }
    Ok(())
}

/// `SignedData`: applies to the `ToBeSignedData` (its `HeaderInfo`, and its payload when
/// that is `data`, recursively) and to the `Signature`. The signer is not part of it; use
/// [`canonicalize_signer_identifier`] for the certificates there.
pub fn canonicalize_signed_data(sd: &mut SignedData) -> Result<()> {
    canonicalize_to_be_signed_data(&mut sd.tbs_data)?;
    canonicalize_signature(&mut sd.signature)
}

fn canonicalize_to_be_signed_data(tbs: &mut ToBeSignedData) -> Result<()> {
    if let Some(data) = &mut tbs.payload.data {
        canonicalize_data(data)?;
    }
    canonicalize_header_info(&mut tbs.header_info)
}

/// `Ieee1609Dot2Data`: applies to the `SignedData` if the content is `signedData`.
pub fn canonicalize_data(data: &mut Ieee1609Dot2Data) -> Result<()> {
    if let Ieee1609Dot2Content::signedData(sd) = &mut data.content {
        canonicalize_signed_data(sd)?;
    }
    Ok(())
}

/// `SignerIdentifier`: applies to every `Certificate` in `certificate`.
pub fn canonicalize_signer_identifier(s: &mut SignerIdentifier) -> Result<()> {
    if let SignerIdentifier::certificate(certs) = s {
        for c in &mut certs.0 {
            canonicalize_certificate(c)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
