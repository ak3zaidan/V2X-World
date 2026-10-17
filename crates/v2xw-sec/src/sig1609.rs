//! IEEE 1609.2 signatures over certificates and `SignedData`, for both curves the US SCMS
//! uses: ECDSA NIST P-256 with SHA-256 (every CA, RA, MA, CRL signer and end entity) and
//! ECDSA NIST P-384 with SHA-384 (the five SCMS Manager electors, 01 §3.1).
//!
//! This is the shared signature layer for the CRL ([`crate::crl`]) and CTL
//! ([`crate::ctl`]) formats. It is deliberately the bare 1609.2 rule set and no policy:
//!
//! * **What is hashed** [IEEE 1609.2-2022 §5.3.1, 02 §7]: the signature input is
//!   `H( H(tbs) ‖ H(signer identifier input) )`, where `tbs` is the COER of the
//!   `ToBeSignedData` (or the certificate's `ToBeSignedCertificate`), and the signer
//!   identifier input is the canonical COER of the signer's certificate, or the empty
//!   string when the signer is `self` (a self-signed root or elector). `H` is the hash the
//!   signer's curve implies: SHA-256 for P-256, SHA-384 for P-384.
//! * **Canonical form** (02 §3): a certificate is hashed after
//!   [`v2xw_msg::scms_types::canonicalize_certificate`] (compressed points, `x-only`
//!   `rSig`), so a peer that transmits uncompressed points still hashes to the same bits.
//! * **Which hash identifies a certificate**: the one its `issuer` names
//!   (`sha256AndDigest` / `self(sha256)` → SHA-256, `sha384AndDigest` / `self(sha384)` →
//!   SHA-384). `HashedId8/10/32` are the low-order bytes of that digest and `HashedId48`
//!   is the whole SHA-384 digest [1609.2 §6.3.25-ff; 1609.2.1 `CtlRootCaEntry` note:
//!   a 48-octet hash "is truncated to the low-order 32 bytes"].
//!
//! Checked against the real SCMS Manager production and test CTLs and certificate-chain
//! files in `tests/real_artefacts.rs` (every elector signature and every published
//! certificate signature verifies; one flipped byte does not) and against the A0 oracle.

use ::p256::ecdsa::signature::hazmat::{PrehashSigner, PrehashVerifier};
use sha2::{Digest, Sha256, Sha384};
use v2xw_msg::scms_types::canonicalize_certificate;
use v2xw_msg::sec_types::ieee1609_dot2::{
    Certificate, HeaderInfo, Ieee1609Dot2Content, Ieee1609Dot2Data, IssuerIdentifier,
    SequenceOfCertificate, SignedData, SignedDataPayload, SignerIdentifier, ToBeSignedCertificate,
    ToBeSignedData, VerificationKeyIndicator,
};
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    EccP256CurvePoint, EccP384CurvePoint, EcdsaP256Signature, EcdsaP384Signature, HashAlgorithm,
    Opaque, Psid, PublicVerificationKey, Signature, Uint8,
};

/// The two hash functions of the US profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hash1609 {
    /// SHA-256: P-256 signatures and every non-elector certificate.
    Sha256,
    /// SHA-384: P-384 signatures and the elector certificates.
    Sha384,
}

impl Hash1609 {
    /// The digest of `bytes`.
    pub fn digest(self, bytes: &[u8]) -> Vec<u8> {
        match self {
            Hash1609::Sha256 => Sha256::digest(bytes).to_vec(),
            Hash1609::Sha384 => Sha384::digest(bytes).to_vec(),
        }
    }

    /// The ASN.1 `HashAlgorithm` value.
    pub fn to_asn1(self) -> HashAlgorithm {
        match self {
            Hash1609::Sha256 => HashAlgorithm::sha256,
            Hash1609::Sha384 => HashAlgorithm::sha384,
        }
    }

    /// From the ASN.1 value; `None` for SM3, which the US profile does not use.
    pub fn from_asn1(h: &HashAlgorithm) -> Option<Hash1609> {
        match h {
            HashAlgorithm::sha256 => Some(Hash1609::Sha256),
            HashAlgorithm::sha384 => Some(Hash1609::Sha384),
            _ => None,
        }
    }
}

/// Why a certificate or signature could not be processed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SigError {
    /// COER encoding or decoding failed.
    #[error("COER: {0}")]
    Coer(String),
    /// The certificate cannot be put in canonical form (e.g. an `x-only` public key).
    #[error("canonicalisation: {0}")]
    Canonical(String),
    /// A curve, hash or key form this profile does not support.
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
}

/// COER of any value.
pub fn coer<T: rasn::Encode>(value: &T) -> Result<Vec<u8>, SigError> {
    rasn::coer::encode(value).map_err(|e| SigError::Coer(e.to_string()))
}

/// The canonical COER encoding of a certificate (02 §3): what is hashed to identify it
/// and what a signer-identifier input contains.
pub fn canonical_cert_coer(cert: &Certificate) -> Result<Vec<u8>, SigError> {
    let mut c = cert.clone();
    canonicalize_certificate(&mut c).map_err(|e| SigError::Canonical(e.to_string()))?;
    coer(&c)
}

/// The hash a certificate is identified by: the one its `issuer` field names.
pub fn cert_hash_alg(cert: &Certificate) -> Result<Hash1609, SigError> {
    match &cert.0.issuer {
        IssuerIdentifier::sha256AndDigest(_) => Ok(Hash1609::Sha256),
        IssuerIdentifier::sha384AndDigest(_) => Ok(Hash1609::Sha384),
        IssuerIdentifier::R_self(h) => {
            Hash1609::from_asn1(h).ok_or(SigError::Unsupported("self-signed with SM3"))
        }
        _ => Err(SigError::Unsupported("SM3 issuer")),
    }
}

/// The whole-certificate hash: the issuer-named hash over the canonical COER.
pub fn cert_hash(cert: &Certificate) -> Result<Vec<u8>, SigError> {
    Ok(cert_hash_alg(cert)?.digest(&canonical_cert_coer(cert)?))
}

fn low<const N: usize>(h: &[u8]) -> [u8; N] {
    let mut out = [0u8; N];
    out.copy_from_slice(&h[h.len() - N..]);
    out
}

/// `HashedId8` of a certificate: the low-order 8 bytes of [`cert_hash`].
pub fn cert_hashed_id8(cert: &Certificate) -> Result<[u8; 8], SigError> {
    Ok(low::<8>(&cert_hash(cert)?))
}

/// `HashedId10` of a certificate (a hash-based CRL entry).
pub fn cert_hashed_id10(cert: &Certificate) -> Result<[u8; 10], SigError> {
    Ok(low::<10>(&cert_hash(cert)?))
}

/// `HashedId32` of a certificate (a CTL `rootCaApprove` entry): the low-order 32 bytes,
/// which is all of a SHA-256 digest and the truncation of a SHA-384 one.
pub fn cert_hashed_id32(cert: &Certificate) -> Result<[u8; 32], SigError> {
    Ok(low::<32>(&cert_hash(cert)?))
}

/// `HashedId48` of a certificate (a CTL `electorApprove` entry): the whole SHA-384
/// digest. An error for a certificate identified by SHA-256.
pub fn cert_hashed_id48(cert: &Certificate) -> Result<[u8; 48], SigError> {
    let h = cert_hash(cert)?;
    if h.len() != 48 {
        return Err(SigError::Unsupported("HashedId48 of a SHA-256 certificate"));
    }
    Ok(low::<48>(&h))
}

/// `H( H(tbs) ‖ H(signer_input) )` [1609.2 §5.3.1].
pub fn digest_1609(h: Hash1609, tbs: &[u8], signer_input: &[u8]) -> Vec<u8> {
    let inner = [h.digest(tbs), h.digest(signer_input)].concat();
    h.digest(&inner)
}

/// An explicit verification key as SEC 1 bytes, with its curve's hash. `None` for an
/// `x-only`/`fill` point, a Brainpool or SM2 key, or a reconstruction value.
pub fn verification_key_sec1(k: &PublicVerificationKey) -> Option<(Hash1609, Vec<u8>)> {
    match k {
        PublicVerificationKey::ecdsaNistP256(p) => Some((Hash1609::Sha256, p256_sec1(p)?)),
        PublicVerificationKey::ecdsaNistP384(p) => Some((Hash1609::Sha384, p384_sec1(p)?)),
        _ => None,
    }
}

fn p256_sec1(p: &EccP256CurvePoint) -> Option<Vec<u8>> {
    Some(match p {
        EccP256CurvePoint::compressed_y_0(x) => [&[2u8][..], x].concat(),
        EccP256CurvePoint::compressed_y_1(x) => [&[3u8][..], x].concat(),
        EccP256CurvePoint::uncompressedP256(u) => [&[4u8][..], &u.x, &u.y].concat(),
        _ => return None,
    })
}

fn p384_sec1(p: &EccP384CurvePoint) -> Option<Vec<u8>> {
    Some(match p {
        EccP384CurvePoint::compressed_y_0(x) => [&[2u8][..], x].concat(),
        EccP384CurvePoint::compressed_y_1(x) => [&[3u8][..], x].concat(),
        EccP384CurvePoint::uncompressedP384(u) => [&[4u8][..], &u.x, &u.y].concat(),
        _ => return None,
    })
}

/// The explicit public key a certificate carries. `None` for an implicit certificate.
pub fn explicit_key(cert: &Certificate) -> Option<(Hash1609, Vec<u8>)> {
    match &cert.0.to_be_signed.verify_key_indicator {
        VerificationKeyIndicator::verificationKey(k) => verification_key_sec1(k),
        _ => None,
    }
}

fn r_of_256(p: &EccP256CurvePoint) -> Option<&[u8]> {
    match p {
        EccP256CurvePoint::x_only(x)
        | EccP256CurvePoint::compressed_y_0(x)
        | EccP256CurvePoint::compressed_y_1(x) => Some(x),
        EccP256CurvePoint::uncompressedP256(u) => Some(&u.x),
        _ => None,
    }
}

fn r_of_384(p: &EccP384CurvePoint) -> Option<&[u8]> {
    match p {
        EccP384CurvePoint::x_only(x)
        | EccP384CurvePoint::compressed_y_0(x)
        | EccP384CurvePoint::compressed_y_1(x) => Some(x),
        EccP384CurvePoint::uncompressedP384(u) => Some(&u.x),
        _ => None,
    }
}

/// Verifies a 1609.2 `Signature` over `digest` under the SEC 1 key `key` of curve `h`.
/// `false` for any mismatch of curve, length or value: signatures off the wire are data.
pub fn verify_digest(h: Hash1609, key: &[u8], digest: &[u8], sig: &Signature) -> bool {
    match (h, sig) {
        (Hash1609::Sha256, Signature::ecdsaNistP256Signature(s)) => {
            let (Some(r), Ok(vk)) = (
                r_of_256(&s.r_sig),
                ::p256::ecdsa::VerifyingKey::from_sec1_bytes(key),
            ) else {
                return false;
            };
            let (Ok(r), Ok(sv)) = (<[u8; 32]>::try_from(r), <[u8; 32]>::try_from(&s.s_sig[..]))
            else {
                return false;
            };
            let Ok(sig) = ::p256::ecdsa::Signature::from_scalars(r, sv) else {
                return false;
            };
            digest.len() == 32 && vk.verify_prehash(digest, &sig).is_ok()
        }
        (Hash1609::Sha384, Signature::ecdsaNistP384Signature(s)) => {
            let Some(r) = r_of_384(&s.r_sig) else {
                return false;
            };
            let (Ok(r), Ok(sv), Ok(d)) = (
                <[u8; 48]>::try_from(r),
                <[u8; 48]>::try_from(&s.s_sig[..]),
                <[u8; 48]>::try_from(digest),
            ) else {
                return false;
            };
            crate::p384::verify_digest(key, &d, &crate::p384::P384Signature { r, s: sv })
        }
        _ => false,
    }
}

/// The COER a certificate's signature is computed over: its `toBeSigned` in canonical
/// form (1609.2 §6.4.3: the certificate is canonicalised before it is signed).
pub fn cert_tbs_coer(cert: &Certificate) -> Result<Vec<u8>, SigError> {
    let mut c = cert.clone();
    canonicalize_certificate(&mut c).map_err(|e| SigError::Canonical(e.to_string()))?;
    coer::<ToBeSignedCertificate>(&c.0.to_be_signed)
}

/// Verifies an explicit certificate's signature: by its own key when self-signed
/// (`issuer = None`, signer input the empty string), else by `issuer`'s explicit key
/// with the issuer's canonical COER as signer input. The hash is the one `cert.issuer`
/// names, which must agree with the signing key's curve.
pub fn verify_explicit_cert(cert: &Certificate, issuer: Option<&Certificate>) -> bool {
    let inner = || -> Result<bool, SigError> {
        let Some(sig) = &cert.0.signature else {
            return Ok(false);
        };
        let h = cert_hash_alg(cert)?;
        let (kh, key, signer_input) = match issuer {
            None => {
                if !matches!(cert.0.issuer, IssuerIdentifier::R_self(_)) {
                    return Ok(false);
                }
                let Some((kh, key)) = explicit_key(cert) else {
                    return Ok(false);
                };
                (kh, key, Vec::new())
            }
            Some(iss) => {
                let Some((kh, key)) = explicit_key(iss) else {
                    return Ok(false);
                };
                // The issuer field must name this issuer.
                let id8 = cert_hashed_id8(iss)?;
                let named = match &cert.0.issuer {
                    IssuerIdentifier::sha256AndDigest(d) | IssuerIdentifier::sha384AndDigest(d) => {
                        d.0[..] == id8[..]
                    }
                    _ => false,
                };
                if !named {
                    return Ok(false);
                }
                (kh, key, canonical_cert_coer(iss)?)
            }
        };
        if kh != h {
            return Ok(false);
        }
        let digest = digest_1609(h, &cert_tbs_coer(cert)?, &signer_input);
        Ok(verify_digest(h, &key, &digest, sig))
    };
    inner().unwrap_or(false)
}

/// A signing key of either US-profile curve. Deterministic (RFC 6979) so a run's bytes are
/// a pure function of its keys (ADR 0004).
#[derive(Clone)]
pub enum Signer1609 {
    /// ECDSA P-256 / SHA-256.
    P256(::p256::ecdsa::SigningKey),
    /// ECDSA P-384 / SHA-384.
    P384(crate::p384::P384SigningKey),
}

impl core::fmt::Debug for Signer1609 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Signer1609::P256(_) => "Signer1609::P256(..)",
            Signer1609::P384(_) => "Signer1609::P384(..)",
        })
    }
}

impl Signer1609 {
    /// A P-256 key from its 32-byte big-endian scalar (`None` unless `1 ≤ d < n`).
    pub fn p256(d: &[u8; 32]) -> Option<Self> {
        ::p256::ecdsa::SigningKey::from_bytes(d.into())
            .ok()
            .map(Signer1609::P256)
    }

    /// A P-384 key from its 48-byte big-endian scalar.
    pub fn p384(d: &[u8; 48]) -> Option<Self> {
        crate::p384::P384SigningKey::from_be48(d).map(Signer1609::P384)
    }

    /// The hash this key signs with.
    pub fn hash(&self) -> Hash1609 {
        match self {
            Signer1609::P256(_) => Hash1609::Sha256,
            Signer1609::P384(_) => Hash1609::Sha384,
        }
    }

    /// The public key as a canonical (compressed) `PublicVerificationKey`.
    pub fn public_key(&self) -> PublicVerificationKey {
        match self {
            Signer1609::P256(k) => {
                let ep = k.verifying_key().to_encoded_point(true);
                let b = ep.as_bytes();
                let x = b[1..].to_vec().into();
                PublicVerificationKey::ecdsaNistP256(if b[0] == 2 {
                    EccP256CurvePoint::compressed_y_0(x)
                } else {
                    EccP256CurvePoint::compressed_y_1(x)
                })
            }
            Signer1609::P384(k) => {
                let b = k.public_sec1(true);
                let x = b[1..].to_vec().into();
                PublicVerificationKey::ecdsaNistP384(if b[0] == 2 {
                    EccP384CurvePoint::compressed_y_0(x)
                } else {
                    EccP384CurvePoint::compressed_y_1(x)
                })
            }
        }
    }

    /// Signs a digest of this key's hash; the result is canonical (`x-only` `rSig`).
    pub fn sign_digest(&self, digest: &[u8]) -> Signature {
        match self {
            Signer1609::P256(k) => {
                let sig: ::p256::ecdsa::Signature =
                    k.sign_prehash(digest).expect("a 32-byte prehash is signable");
                let (r, s) = sig.split_bytes();
                Signature::ecdsaNistP256Signature(EcdsaP256Signature::new(
                    EccP256CurvePoint::x_only(r.to_vec().into()),
                    s.to_vec().into(),
                ))
            }
            Signer1609::P384(k) => {
                let mut d = [0u8; 48];
                d.copy_from_slice(digest);
                let s = k.sign_digest(&d);
                Signature::ecdsaNistP384Signature(EcdsaP384Signature::new(
                    EccP384CurvePoint::x_only(s.r.to_vec().into()),
                    s.s.to_vec().into(),
                ))
            }
        }
    }

    /// Signs `tbs` as 1609.2 does: over `H(H(tbs) ‖ H(signer_input))`.
    pub fn sign_1609(&self, tbs: &[u8], signer_input: &[u8]) -> Signature {
        self.sign_digest(&digest_1609(self.hash(), tbs, signer_input))
    }
}

/// The `SignerIdentifier` choices a signed SCMS SPDU may carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpduSigner {
    /// `certificate`: the signer's certificate is attached (CTL elector signatures,
    /// CRLs as the US profile issues them).
    Certificate(Certificate),
    /// `digest`: the signer's `HashedId8`; the verifier must already hold the certificate.
    Digest([u8; 8]),
    /// `self`: no certificate; the signer input is the empty string.
    SelfSigned,
}

/// A signed `Ieee1609Dot2Data` taken apart for verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedSigned {
    /// `headerInfo.psid` (saturated to `u64`).
    pub psid: u64,
    /// The whole `headerInfo`, for profile rules on absent fields.
    pub header: HeaderInfo,
    /// The canonical COER of `tbsData`: what the signature covers.
    pub tbs_coer: Vec<u8>,
    /// The bytes of the inner `unsecuredData` (the protected payload).
    pub payload: Vec<u8>,
    /// Who signed.
    pub signer: SpduSigner,
    /// The declared `hashId`.
    pub hash: Hash1609,
    /// The signature.
    pub signature: Signature,
}

/// Builds `Ieee1609Dot2Data{signedData}` around `payload` carried as inner
/// `unsecuredData`, with the given header and signer, signed by `key`.
pub fn sign_spdu(
    header: HeaderInfo,
    payload: Vec<u8>,
    signer: SpduSigner,
    key: &Signer1609,
) -> Result<Ieee1609Dot2Data, SigError> {
    let inner = Ieee1609Dot2Data::new(Uint8(3), Ieee1609Dot2Content::unsecuredData(Opaque(payload.into())));
    let tbs = ToBeSignedData::new(Box::new(SignedDataPayload::new(Some(inner), None, None)), header);
    let tbs_coer = coer(&tbs)?;
    let (signer_input, sid) = match signer {
        SpduSigner::Certificate(c) => (
            canonical_cert_coer(&c)?,
            SignerIdentifier::certificate(SequenceOfCertificate(vec![c])),
        ),
        // The signature covers the certificate either way, so a digest signer needs it too.
        SpduSigner::Digest(_) => {
            return Err(SigError::Unsupported("sign_spdu needs the signer's certificate"));
        }
        SpduSigner::SelfSigned => (Vec::new(), SignerIdentifier::R_self(())),
    };
    let signature = key.sign_1609(&tbs_coer, &signer_input);
    Ok(Ieee1609Dot2Data::new(
        Uint8(3),
        Ieee1609Dot2Content::signedData(SignedData::new(
            key.hash().to_asn1(),
            tbs,
            sid,
            signature,
        )),
    ))
}

/// Takes a signed SPDU apart. `None` unless it is `signedData` whose payload is an inner
/// `unsecuredData` (no `extDataHash`), signed with a US-profile hash.
pub fn open_signed(data: &Ieee1609Dot2Data) -> Option<OpenedSigned> {
    let Ieee1609Dot2Content::signedData(sd) = &data.content else {
        return None;
    };
    let hash = Hash1609::from_asn1(&sd.hash_id)?;
    let inner = sd.tbs_data.payload.data.as_ref()?;
    if sd.tbs_data.payload.ext_data_hash.is_some() {
        return None;
    }
    let Ieee1609Dot2Content::unsecuredData(Opaque(payload)) = &inner.content else {
        return None;
    };
    let signer = match &sd.signer {
        SignerIdentifier::certificate(SequenceOfCertificate(v)) if v.len() == 1 => {
            SpduSigner::Certificate(v[0].clone())
        }
        SignerIdentifier::digest(d) => {
            let mut b = [0u8; 8];
            b.copy_from_slice(&d.0[..]);
            SpduSigner::Digest(b)
        }
        SignerIdentifier::R_self(()) => SpduSigner::SelfSigned,
        _ => return None,
    };
    Some(OpenedSigned {
        psid: u64::try_from(&sd.tbs_data.header_info.psid.0).unwrap_or(u64::MAX),
        header: sd.tbs_data.header_info.clone(),
        tbs_coer: coer(&sd.tbs_data).ok()?,
        payload: payload.to_vec(),
        signer,
        hash,
        signature: sd.signature.clone(),
    })
}

/// Verifies an opened SPDU's signature under `signer_cert` (the attached certificate,
/// or the one a digest resolved to). The declared `hashId` must match the key's curve.
pub fn verify_opened(o: &OpenedSigned, signer_cert: &Certificate) -> bool {
    let Some((h, key)) = explicit_key(signer_cert) else {
        return false;
    };
    if h != o.hash {
        return false;
    }
    let Ok(input) = canonical_cert_coer(signer_cert) else {
        return false;
    };
    verify_digest(h, &key, &digest_1609(h, &o.tbs_coer, &input), &o.signature)
}

/// A `HeaderInfo` with only `psid` set: the SecuredCrl rule (1609.2 `Ieee1609Dot2Crl`)
/// and what the 1609.2.1 SCMS SPDUs without times use.
pub fn header_psid_only(psid: u64) -> HeaderInfo {
    HeaderInfo::new(
        Psid(rasn::types::Integer::from(psid)),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1609.2's empty-string example (§6.3.25 note): SHA-256("") ends `…a495991b7852b855`.
    #[test]
    fn low_order_bytes_are_the_suffix() {
        let h = Hash1609::Sha256.digest(b"");
        assert_eq!(low::<8>(&h), [0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52, 0xb8, 0x55]);
        let h = Hash1609::Sha384.digest(b"");
        // SHA-384("") = 38b060a7…274edebfe76f65fbd51ad2f14898b95b (FIPS 180-4 / NIST examples).
        assert_eq!(&low::<4>(&h), &[0x48, 0x98, 0xb9, 0x5b]);
    }

    #[test]
    fn sign_then_verify_both_curves_and_a_flipped_digest_fails() {
        for s in [
            Signer1609::p256(&[7u8; 32]).unwrap(),
            Signer1609::p384(&[9u8; 48]).unwrap(),
        ] {
            let sig = s.sign_1609(b"tbs", b"signer");
            let (h, key) = verification_key_sec1(&s.public_key()).unwrap();
            let mut d = digest_1609(h, b"tbs", b"signer");
            assert!(verify_digest(h, &key, &d, &sig));
            d[0] ^= 1;
            assert!(!verify_digest(h, &key, &d, &sig));
        }
    }
}
