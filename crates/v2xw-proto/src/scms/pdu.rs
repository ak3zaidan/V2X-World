//! The real IEEE 1609.2.1 SCMS PDU layer (WP-B1).
//!
//! Every EE↔SCMS message of the US profile is an `Ieee1609Dot2Data` carrying a `ScmsPdu`
//! (`version 2`) under the security-management PSID 35, protected by one of the 1609.2.1
//! parameterised wrappers (03 §3.1, [ASN-2.1] `Ieee1609Dot2Dot1Protocol`):
//!
//! | wrapper | here |
//! |---|---|
//! | `Ieee1609Dot2Data-Unsecured{T}` | [`unsecured`] |
//! | `Ieee1609Dot2Data-Signed{T, 35}` | [`signed`]: `headerInfo` carries `psid` only, signer `SignerSingleCert` |
//! | `Ieee1609Dot2Data-Encrypted{T}` | [`encrypt_to_cert`] (`certRecipInfo`), [`encrypt_to_key`] (`rekRecipInfo`) |
//! | `…-SignedEncrypted{T, 35}` | [`signed_encrypted`]: sign, then encrypt |
//! | `…-EncryptedSigned{T, 35}` | [`encrypted_signed`]: encrypt to a response key, then sign |
//! | `…-SignedCertRequest{T, Signer}` | [`signed_cert_request`]: `SignerSelf` or `SignerSingleCert` |
//! | `…-SignedEncryptedCertRequest{T, Signer}` | [`signed_encrypted_cert_request`] |
//! | `…-SymmEncryptedSingleRecipient{T}` | [`symm_encrypt_single`]: one `symmRecipInfo` |
//!
//! Openers ([`open_signed`], [`open_signed_cert_request`], [`decrypt_cert`],
//! [`decrypt_symm`]) check the profile rules a receiver applies: the protocol version,
//! the PSID, the absent header fields, exactly one signer certificate, the signature
//! under the signer's key (an implicit certificate's key is reconstructed with ECQV), and
//! that the signer certificate was issued by the deployment's PKI. A failure is a typed
//! [`PduError`], which the run records: a forged, replayed or tampered PDU is an
//! observable event, not a silent drop.
//!
//! Sizes: a message that carries a PDU is charged the PDU's encoded length
//! ([`crate::sizes::WireSize::real`]), so invariant I-S3 holds as equality by
//! construction; `tests/pdu_layer.rs` checks the charged size against the bytes.
//!
//! Randomness: keys, ECIES ephemerals, data keys and nonces come from [`Drbg`], a SHA-256
//! counter generator keyed by the run's master seed and a label. It is separate from the
//! [`v2xw_core::rng`] streams on purpose, so turning the layer on does not shift a single
//! draw any existing model makes.
//!
//! Scope of WP-B1 (00 §4.2): the layer, the PKI the signers need, and the PDUs of the
//! enrolment and provisioning exchanges the run already performs. The field-level
//! profile of the enrolment certificate (region, series 4, wildcard permissions) is WP-B2;
//! per-certificate `RaAcaCertRequest` and the ACA's private response in the run are
//! WP-B3a; misbehaviour reports are WP-B4.

use std::sync::Arc;

use p256::Scalar;
use rasn::types::{FixedOctetString, Integer, OctetString, SequenceOf};
use v2xw_msg::sec_types::ieee1609_dot2::{
    Certificate, CertificateId, CertificateType, EncryptedData, EncryptedDataEncryptionKey,
    Ieee1609Dot2Content, Ieee1609Dot2Data, IssuerIdentifier, One28BitCcmCiphertext,
    PKRecipientInfo, RecipientInfo, SequenceOfCertificate, SequenceOfPsidGroupPermissions,
    SequenceOfRecipientInfo, SignerIdentifier, SymmRecipientInfo, SymmetricCiphertext,
    ToBeSignedCertificate, VerificationKeyIndicator,
};
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    BasePublicEncryptionKey, CrlSeries, Duration as Ieee1609Duration, EccP256CurvePoint,
    EciesP256EncryptedKey, HashedId3, HashedId8, Hostname, IValue, Opaque, Psid, PsidSsp,
    PublicEncryptionKey, SequenceOfPsidSsp, ServiceSpecificPermissions, SymmAlgorithm,
    SymmetricEncryptionKey, Time32, Uint8, Uint16, Uint32, ValidityPeriod,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_eca_ee_interface::{
    EcaEeCertResponse, EcaEeInterfacePdu, EeEcaCertRequest,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_ee_ra_interface::{
    AdditionalParams, ButterflyExpansion, ButterflyParamsOriginal, EeRaCertRequest,
    EeRaDownloadRequest, EeRaInterfacePdu, RaEeCertAck, RaEeCertInfo,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::{
    AcaSsp, DcmSsp, EcaSsp, IcaSsp, LaSsp, MaSsp, RaSsp, RootCaSsp, ScmsPdu, ScmsPduContent,
    ScopedCertificateRequest, SecurityMgmtSsp, SignedCertificateRequest,
};
use v2xw_sec::butterfly::Caterpillar;
use v2xw_sec::cert::{self, CertHash, CertKey};
use v2xw_sec::ec::{self, Point};
use v2xw_sec::sig1609::{self, Hash1609, Signer1609};
use v2xw_sec::{ccm, ecies, hashedid};

/// `SecurityMgmtPsid ::= Psid (35)` [ASN-2.1] Protocol.
pub const SECURITY_MGMT_PSID: u64 = 35;
/// The BSM PSID (0x20) a pseudonym certificate is requested for.
pub const BSM_PSID: u64 = 0x20;
/// `ScmsPdu.version` (2) [ASN-2.1].
pub const SCMS_PDU_VERSION: u8 = 2;
/// `Ieee1609Dot2Data.protocolVersion` (3) [IEEE 1609.2].
pub const PROTOCOL_VERSION: u8 = 3;
/// The version of every 1609.2.1 request/response structure (`Uint8 (2)`).
const STRUCT_VERSION: u8 = 2;

/// Why a PDU could not be built or did not pass a receiver's checks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PduError {
    /// COER encode or decode failed.
    #[error("COER: {0}")]
    Coer(String),
    /// The structure is not the wrapper the receiver expects.
    #[error("not the expected SPDU shape: {0}")]
    Shape(&'static str),
    /// `protocolVersion` or `ScmsPdu.version` is wrong.
    #[error("wrong version: {0}")]
    Version(&'static str),
    /// The header PSID is not 35.
    #[error("headerInfo.psid is {0}, not the security-management PSID 35")]
    Psid(u64),
    /// A header field the 1609.2.1 `Signed` wrapper requires ABSENT is present.
    #[error("headerInfo.{0} must be absent in a 1609.2.1 signed SPDU")]
    HeaderField(&'static str),
    /// The signer is not one attached certificate (`SignerSingleCert`) or `self`.
    #[error("signer identifier: {0}")]
    Signer(&'static str),
    /// The signer certificate's issuer is not in the deployment's PKI, or its signature
    /// does not verify under that issuer.
    #[error("signer certificate does not chain to the deployment PKI")]
    UntrustedSigner,
    /// The signature does not verify.
    #[error("signature does not verify")]
    BadSignature,
    /// No `RecipientInfo` is addressed to this receiver.
    #[error("no RecipientInfo is addressed to this receiver")]
    NotForMe,
    /// ECIES or CCM failed: wrong key or tampered ciphertext.
    #[error("decryption failed: {0}")]
    Decrypt(&'static str),
    /// A key or point is unusable.
    #[error("key: {0}")]
    Key(&'static str),
}

/// Result of this module.
pub type Result<T> = core::result::Result<T, PduError>;

fn coer<T: rasn::Encode>(v: &T) -> Result<Vec<u8>> {
    rasn::coer::encode(v).map_err(|e| PduError::Coer(e.to_string()))
}

fn decode<T: rasn::Decode>(b: &[u8]) -> Result<T> {
    rasn::coer::decode(b).map_err(|e| PduError::Coer(e.to_string()))
}

/// An encoded SPDU as it travels in a message: shared, so forwarding is free.
#[derive(Clone, PartialEq, Eq)]
pub struct Pdu(pub Arc<Vec<u8>>);

impl Pdu {
    /// Wraps encoded bytes.
    pub fn new(bytes: Vec<u8>) -> Pdu {
        Pdu(Arc::new(bytes))
    }
    /// The bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
    /// The encoded length.
    pub fn len(&self) -> u32 {
        u32::try_from(self.0.len()).unwrap_or(u32::MAX)
    }
    /// Whether it is empty (never, for a real SPDU).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// HashedId8 of the bytes as sent: 1609.2.1's `requestHash` ("the hash of the
    /// corresponding EeRaCertRequestSpdu", [ASN-2.1] EeRaInterface `RaEeCertAck`).
    pub fn hashed_id8(&self) -> [u8; 8] {
        hashedid::id8_bytes(&hashedid::hashed_id8(&self.0))
    }
}

impl core::fmt::Debug for Pdu {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Pdu({} B)", self.0.len())
    }
}

// ---------------------------------------------------------------------------------------
// Deterministic randomness
// ---------------------------------------------------------------------------------------

/// SHA-256 counter generator: `SHA-256("v2xw/scms-pdu/1" ‖ seed_le ‖ label ‖ counter_le)`.
#[derive(Debug, Clone)]
pub struct Drbg {
    seed: u64,
    label: Vec<u8>,
    counter: u64,
}

impl Drbg {
    /// A generator for `label` under the run's master seed.
    pub fn new(seed: u64, label: &[u8]) -> Drbg {
        Drbg {
            seed,
            label: label.to_vec(),
            counter: 0,
        }
    }
    /// The next 32 bytes.
    pub fn next32(&mut self) -> [u8; 32] {
        let mut b = Vec::with_capacity(32 + self.label.len());
        b.extend_from_slice(b"v2xw/scms-pdu/1");
        b.extend_from_slice(&self.seed.to_le_bytes());
        b.extend_from_slice(&self.label);
        b.extend_from_slice(&self.counter.to_le_bytes());
        self.counter += 1;
        v2xw_core::hash::sha256(&b)
    }
    /// A nonzero P-256 scalar.
    pub fn scalar(&mut self) -> Scalar {
        loop {
            let s = ec::scalar_from_be_mod_n(&self.next32());
            if s != Scalar::ZERO {
                return s;
            }
        }
    }
    /// 16 bytes (an AES key).
    pub fn key16(&mut self) -> [u8; 16] {
        let b = self.next32();
        let mut k = [0u8; 16];
        k.copy_from_slice(&b[..16]);
        k
    }
    /// 12 bytes (a CCM nonce).
    pub fn nonce12(&mut self) -> [u8; 12] {
        let b = self.next32();
        let mut k = [0u8; 12];
        k.copy_from_slice(&b[..12]);
        k
    }
}

// ---------------------------------------------------------------------------------------
// The deployment PKI the signers need
// ---------------------------------------------------------------------------------------

/// An SCMS component: its certificate, signing key and (for the RA and MA) the private
/// half of the certificate's `encryptionKey`.
#[derive(Clone)]
pub struct Authority {
    /// The certificate's `id.name`.
    pub name: String,
    /// The certificate (explicit).
    pub cert: Certificate,
    /// The signing key.
    pub key: Signer1609,
    /// The P-256 signing scalar (ECQV issuance needs it); `None` for the P-384 root.
    pub private: Option<Scalar>,
    /// The ECIES private key matching `toBeSigned.encryptionKey`, when the certificate
    /// carries one.
    pub enc_private: Option<Scalar>,
}

impl core::fmt::Debug for Authority {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Authority({})", self.name)
    }
}

impl Authority {
    /// The whole-certificate HashedId8 (canonical COER, SHA-256 or SHA-384 per issuer).
    pub fn id8(&self) -> [u8; 8] {
        cert_id8(&self.cert)
    }
    /// The certificate's ECIES public key.
    pub fn enc_public(&self) -> Option<Point> {
        self.enc_private.as_ref().map(Point::mul_base)
    }
}

/// A certificate's whole-certificate HashedId8 (all zero if it does not encode).
pub fn cert_id8(c: &Certificate) -> [u8; 8] {
    cert::cert_id8(c).map_or([0; 8], |h| hashedid::id8_bytes(&h))
}

/// The SCMS components of one provider, with real explicit certificates:
/// a P-384 self-signed root, a P-256 ICA under it, and the ECA, RA, ACA, MA, the two LAs
/// and the DCM under the ICA. Each holds PSID 35 with its 1609.2.1 `SecurityMgmtSsp`;
/// the RA and MA certificates carry an `encryptionKey` (03 §3.2: requests are encrypted
/// to the RA; reports to the MA). The provider label is neutral (owner decision).
///
/// Per-type certificate profiles (gap row 13) and the CTL/elector governance (WP-B5) are
/// outside this package; these certificates are what a signer needs to produce, and a
/// receiver to verify, real SCMS SPDUs.
#[derive(Debug, Clone)]
pub struct ScmsPki {
    /// Root CA (P-384, `self(sha384)`).
    pub root: Authority,
    /// Intermediate CA.
    pub ica: Authority,
    /// Enrolment CA.
    pub eca: Authority,
    /// Registration Authority.
    pub ra: Authority,
    /// Authorization (pseudonym) CA.
    pub aca: Authority,
    /// Misbehaviour Authority.
    pub ma: Authority,
    /// Linkage Authority 1.
    pub la1: Authority,
    /// Linkage Authority 2.
    pub la2: Authority,
    /// Device Configuration Manager.
    pub dcm: Authority,
}

fn opaque_ssp(ssp: &SecurityMgmtSsp) -> Result<ServiceSpecificPermissions> {
    Ok(ServiceSpecificPermissions::opaque(coer(ssp)?.into()))
}

fn public_encryption_key(p: &Point) -> Result<PublicEncryptionKey> {
    Ok(PublicEncryptionKey::new(
        SymmAlgorithm::aes128Ccm,
        BasePublicEncryptionKey::eciesNistP256(
            p.to_ecc_point()
                .map_err(|_| PduError::Key("identity point"))?,
        ),
    ))
}

#[allow(clippy::too_many_arguments)]
fn component_tbs(
    name: &str,
    start: u32,
    years: u16,
    ssp: &SecurityMgmtSsp,
    issues: bool,
    enc: Option<&Point>,
    vki: VerificationKeyIndicator,
) -> Result<ToBeSignedCertificate> {
    Ok(ToBeSignedCertificate::new(
        CertificateId::name(Hostname(name.into())),
        HashedId3(FixedOctetString::from([0u8; 3])),
        CrlSeries(Uint16(0)),
        ValidityPeriod::new(
            Time32(Uint32(start)),
            Ieee1609Duration::years(Uint16(years)),
        ),
        None,
        None,
        Some(SequenceOfPsidSsp(vec![PsidSsp::new(
            Psid(Integer::from(SECURITY_MGMT_PSID)),
            Some(opaque_ssp(ssp)?),
        )])),
        issues.then(|| SequenceOfPsidGroupPermissions(vec![cert::issue_all_group()])),
        None,
        None,
        enc.map(public_encryption_key).transpose()?,
        vki,
        None,
        None,
        None,
        None,
    ))
}

impl ScmsPki {
    /// The provider's components, keys drawn from `seed`, certificates valid from the
    /// 1609.2 `Time32` `start`.
    pub fn new(seed: u64, start: u32) -> Result<ScmsPki> {
        let mut g = Drbg::new(seed, b"pki");
        // Root: P-384, self-signed with SHA-384, the form of the published US roots.
        let mut d384 = [0u8; 48];
        let a = g.next32();
        let b = g.next32();
        d384[..32].copy_from_slice(&a);
        d384[32..].copy_from_slice(&b[..16]);
        d384[0] &= 0x7f; // below n for P-384
        let root_key = Signer1609::p384(&d384).ok_or(PduError::Key("P-384 root scalar"))?;
        let root_tbs = component_tbs(
            "root.provider-a",
            start,
            12,
            &SecurityMgmtSsp::root(RootCaSsp::new(Uint8(2))),
            true,
            None,
            VerificationKeyIndicator::verificationKey(root_key.public_key()),
        )?;
        let root_cert = cert::issue_explicit_cert(root_tbs, None, CertHash::Sha384, |dg| {
            Ok(root_key.sign_digest(dg))
        })
        .map_err(|_| PduError::Shape("root certificate"))?;
        let root = Authority {
            name: "root.provider-a".into(),
            cert: root_cert,
            key: root_key,
            private: None,
            enc_private: None,
        };
        let ica = Self::issue(
            &root,
            &mut g,
            "ica.provider-a",
            6,
            &SecurityMgmtSsp::ica(IcaSsp::new(Uint8(2))),
            true,
            false,
            start,
        )?;
        let eca = Self::issue(
            &ica,
            &mut g,
            "eca.provider-a",
            3,
            &SecurityMgmtSsp::eca(EcaSsp::new(Uint8(2))),
            false,
            false,
            start,
        )?;
        let ra = Self::issue(
            &ica,
            &mut g,
            "ra.provider-a",
            3,
            &SecurityMgmtSsp::ra(RaSsp::new(Uint8(2))),
            false,
            true,
            start,
        )?;
        let aca = Self::issue(
            &ica,
            &mut g,
            "aca.provider-a",
            3,
            &SecurityMgmtSsp::aca(AcaSsp::new(Uint8(2))),
            false,
            false,
            start,
        )?;
        let ma = Self::issue(
            &ica,
            &mut g,
            "ma.provider-a",
            3,
            &SecurityMgmtSsp::ma(MaSsp::new(
                Uint8(2),
                v2xw_msg::sec_types::ieee1609_dot2_base_types::SequenceOfPsid(vec![Psid(
                    Integer::from(BSM_PSID),
                )]),
            )),
            false,
            true,
            start,
        )?;
        let la1 = Self::issue(
            &ica,
            &mut g,
            "la1.provider-a",
            3,
            &SecurityMgmtSsp::la(LaSsp::new(Uint8(2), Uint16(1))),
            false,
            false,
            start,
        )?;
        let la2 = Self::issue(
            &ica,
            &mut g,
            "la2.provider-b",
            3,
            &SecurityMgmtSsp::la(LaSsp::new(Uint8(2), Uint16(2))),
            false,
            false,
            start,
        )?;
        let dcm = Self::issue(
            &ica,
            &mut g,
            "dcm.provider-a",
            3,
            &SecurityMgmtSsp::dcm(DcmSsp::new(Uint8(2))),
            false,
            false,
            start,
        )?;
        Ok(ScmsPki {
            root,
            ica,
            eca,
            ra,
            aca,
            ma,
            la1,
            la2,
            dcm,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn issue(
        issuer: &Authority,
        g: &mut Drbg,
        name: &str,
        years: u16,
        ssp: &SecurityMgmtSsp,
        issues: bool,
        with_enc: bool,
        start: u32,
    ) -> Result<Authority> {
        let d = g.scalar();
        let key = Signer1609::p256(&ec::scalar_to_be32(&d)).ok_or(PduError::Key("P-256"))?;
        let enc_private = with_enc.then(|| g.scalar());
        let enc_public = enc_private.as_ref().map(Point::mul_base);
        let tbs = component_tbs(
            name,
            start,
            years,
            ssp,
            issues,
            enc_public.as_ref(),
            VerificationKeyIndicator::verificationKey(key.public_key()),
        )?;
        let issuer_key = match &issuer.key {
            Signer1609::P256(_) => CertKey::P256(Point::mul_base(
                issuer
                    .private
                    .as_ref()
                    .ok_or(PduError::Key("issuer scalar"))?,
            )),
            Signer1609::P384(k) => CertKey::P384(k.public_sec1(true)),
        };
        let cert = cert::issue_explicit_cert(
            tbs,
            Some((&issuer.cert, &issuer_key)),
            CertHash::Sha256,
            |dg| Ok(issuer.key.sign_digest(dg)),
        )
        .map_err(|_| PduError::Shape("component certificate"))?;
        Ok(Authority {
            name: name.into(),
            cert,
            key,
            private: Some(d),
            enc_private,
        })
    }

    /// Every component, root first.
    pub fn all(&self) -> [&Authority; 9] {
        [
            &self.root, &self.ica, &self.eca, &self.ra, &self.aca, &self.ma, &self.la1, &self.la2,
            &self.dcm,
        ]
    }

    /// The component whose whole-certificate HashedId8 is `id8`.
    pub fn by_id8(&self, id8: &[u8; 8]) -> Option<&Authority> {
        self.all().into_iter().find(|a| &a.id8() == id8)
    }

    /// The verification key of `cert` if it chains to this PKI: an explicit component
    /// certificate whose issuer signature verifies, or an implicit end-entity
    /// certificate whose key is reconstructed (ECQV-1609) under its issuer.
    pub fn signer_key(&self, c: &Certificate) -> Result<(Hash1609, Vec<u8>)> {
        let issuer = match &c.0.issuer {
            IssuerIdentifier::R_self(_) => {
                // Only the root is self-issued, and only the root as held here counts.
                if cert_id8(c) != self.root.id8() {
                    return Err(PduError::UntrustedSigner);
                }
                return sig1609::explicit_key(c).ok_or(PduError::UntrustedSigner);
            }
            IssuerIdentifier::sha256AndDigest(h) | IssuerIdentifier::sha384AndDigest(h) => self
                .by_id8(&hashedid::id8_bytes(h))
                .ok_or(PduError::UntrustedSigner)?,
            _ => return Err(PduError::UntrustedSigner),
        };
        match c.0.r_type {
            CertificateType::explicit => {
                if !sig1609::verify_explicit_cert(c, Some(&issuer.cert)) {
                    return Err(PduError::UntrustedSigner);
                }
                sig1609::explicit_key(c).ok_or(PduError::UntrustedSigner)
            }
            CertificateType::implicit => {
                let VerificationKeyIndicator::reconstructionValue(pu) =
                    &c.0.to_be_signed.verify_key_indicator
                else {
                    return Err(PduError::Shape(
                        "implicit certificate without reconstructionValue",
                    ));
                };
                let p_u = point_of(pu)?;
                let q_ca =
                    Point::mul_base(issuer.private.as_ref().ok_or(PduError::UntrustedSigner)?);
                let e = cert::implicit_e(c, &issuer.cert).map_err(|_| PduError::Shape("ECQV e"))?;
                let q = p_u.mul(&e).add(&q_ca);
                let sec1 = q
                    .compressed()
                    .ok_or(PduError::Key("reconstructed key at infinity"))?;
                Ok((Hash1609::Sha256, sec1.to_vec()))
            }
            _ => Err(PduError::Shape("certificate type")),
        }
    }
}

/// The P-256 point a 1609.2 `EccP256CurvePoint` carries (compressed or uncompressed).
pub fn point_of(p: &EccP256CurvePoint) -> Result<Point> {
    let mut sec1 = Vec::with_capacity(65);
    match p {
        EccP256CurvePoint::compressed_y_0(x) => {
            sec1.push(0x02);
            sec1.extend_from_slice(x);
        }
        EccP256CurvePoint::compressed_y_1(x) => {
            sec1.push(0x03);
            sec1.extend_from_slice(x);
        }
        EccP256CurvePoint::uncompressedP256(u) => {
            sec1.push(0x04);
            sec1.extend_from_slice(&u.x);
            sec1.extend_from_slice(&u.y);
        }
        _ => return Err(PduError::Key("x-only or fill is not a full point")),
    }
    Point::from_sec1(&sec1).map_err(|_| PduError::Key("not on P-256"))
}

// ---------------------------------------------------------------------------------------
// End-entity credentials
// ---------------------------------------------------------------------------------------

/// A device's real enrolment credential: the implicit certificate the ECA issued and the
/// private key the device reconstructed from `privateKeyInfo`.
#[derive(Clone)]
pub struct EnrolmentCredential {
    /// The implicit enrolment certificate.
    pub cert: Certificate,
    /// The reconstructed signing key `d = e·k_U + r`.
    pub key: Signer1609,
}

impl core::fmt::Debug for EnrolmentCredential {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "EnrolmentCredential({:02x?})", cert_id8(&self.cert))
    }
}

/// The ECA's issuance of an implicit enrolment certificate on the request key `R_U`
/// (ECQV-1609, 02 §6) with the request's `toBeSigned`. Returns the certificate and `r`
/// (`privateKeyInfo`).
///
/// The request's `toBeSigned` ([`ee_eca_cert_request`]) is `id none`, `cracaId 000000`,
/// `crlSeries 4`, `certRequestPermissions` one group (`all`); the full US enrolment
/// profile (region 840/124/484, `id.name`) and the ECA's checks of it are WP-B2.
pub fn issue_enrolment(
    pki: &ScmsPki,
    tbs: &ToBeSignedCertificate,
    request_public: &Point,
    g: &mut Drbg,
) -> Result<(Certificate, Scalar)> {
    let eca_d = pki
        .eca
        .private
        .as_ref()
        .ok_or(PduError::Key("ECA scalar"))?;
    let issued = cert::issue_implicit(
        tbs,
        &pki.eca.cert,
        &Point::mul_base(eca_d),
        eca_d,
        request_public,
        || g.scalar(),
    )
    .map_err(|_| PduError::Shape("ECQV issuance"))?;
    Ok((issued.certificate, issued.private_key_contribution))
}

fn enrolment_tbs(
    request_public: &Point,
    valid_from: u32,
    hours: u16,
) -> Result<ToBeSignedCertificate> {
    Ok(ToBeSignedCertificate::new(
        CertificateId::none(()),
        HashedId3(FixedOctetString::from([0u8; 3])),
        CrlSeries(Uint16(4)),
        ValidityPeriod::new(
            Time32(Uint32(valid_from)),
            Ieee1609Duration::hours(Uint16(hours)),
        ),
        None,
        None,
        None,
        None,
        Some(SequenceOfPsidGroupPermissions(
            vec![cert::issue_all_group()],
        )),
        None,
        None,
        VerificationKeyIndicator::verificationKey(sig1609_key_of(request_public)?),
        None,
        None,
        None,
        None,
    ))
}

fn sig1609_key_of(
    p: &Point,
) -> Result<v2xw_msg::sec_types::ieee1609_dot2_base_types::PublicVerificationKey> {
    Ok(
        v2xw_msg::sec_types::ieee1609_dot2_base_types::PublicVerificationKey::ecdsaNistP256(
            p.to_ecc_point()
                .map_err(|_| PduError::Key("identity point"))?,
        ),
    )
}

/// The device's side of ECQV: `d = e·k_U + r`, checked against the reconstructed public
/// key `e·P_U + Q_ECA` before it is accepted.
pub fn reconstruct_enrolment(
    pki: &ScmsPki,
    cert: &Certificate,
    k_u: &Scalar,
    r: &Scalar,
) -> Result<EnrolmentCredential> {
    let e = cert::implicit_e(cert, &pki.eca.cert).map_err(|_| PduError::Shape("ECQV e"))?;
    let d = e * k_u + r;
    let (_, q) = pki.signer_key(cert)?;
    if Point::mul_base(&d).compressed().map(|c| c.to_vec()) != Some(q) {
        return Err(PduError::Key("d·G is not e·P_U + Q_ECA"));
    }
    Ok(EnrolmentCredential {
        cert: cert.clone(),
        key: Signer1609::p256(&ec::scalar_to_be32(&d)).ok_or(PduError::Key("P-256"))?,
    })
}

// ---------------------------------------------------------------------------------------
// ScmsPdu contents
// ---------------------------------------------------------------------------------------

fn pdu(content: ScmsPduContent) -> ScmsPdu {
    ScmsPdu::new(Uint8(SCMS_PDU_VERSION), content)
}

fn id8(b: [u8; 8]) -> HashedId8 {
    hashedid::id8_from_bytes(b)
}

/// `eca-ee.eeEcaCertRequest` (03 §3.3): an implicit enrolment certificate on `R_U`.
pub fn ee_eca_cert_request(
    generation_time: u32,
    request_public: &Point,
    valid_from: u32,
    hours: u16,
) -> Result<ScmsPdu> {
    Ok(pdu(ScmsPduContent::eca_ee(
        EcaEeInterfacePdu::eeEcaCertRequest(EeEcaCertRequest::new(
            Uint8(STRUCT_VERSION),
            Time32(Uint32(generation_time)),
            CertificateType::implicit,
            enrolment_tbs(request_public, valid_from, hours)?,
            None,
        )),
    )))
}

/// `eca-ee.ecaEeCertResponse`: the enrolment certificate, the ECA chain up to the root,
/// and `privateKeyInfo` = `r`.
pub fn eca_ee_cert_response(
    generation_time: u32,
    request_hash: [u8; 8],
    pki: &ScmsPki,
    certificate: Certificate,
    r: &Scalar,
) -> ScmsPdu {
    pdu(ScmsPduContent::eca_ee(
        EcaEeInterfacePdu::ecaEeCertResponse(EcaEeCertResponse::new(
            Uint8(STRUCT_VERSION),
            Time32(Uint32(generation_time)),
            id8(request_hash),
            SequenceOfCertificate(vec![
                pki.eca.cert.clone(),
                pki.ica.cert.clone(),
                pki.root.cert.clone(),
            ]),
            certificate,
            Some(OctetString::from(ec::scalar_to_be32(r).to_vec())),
        )),
    ))
}

/// `ee-ra.eeRaCertRequest` with the original butterfly parameters (03 §3.3): `tbsCert`
/// `id none`, `cracaId 000000`, `crlSeries 0`, appPermissions BSM, `verifyKeyIndicator`
/// the caterpillar signing key `A`, validity the first batch's (one week + 1 h); the
/// `additionalParams` carry `ck`, `H` (the caterpillar encryption key) and `ek`.
pub fn ee_ra_cert_request(
    generation_time: u32,
    cat_signing: &Point,
    cat_encryption: &Point,
    ck: &[u8; 16],
    ek: &[u8; 16],
    first_batch_start: u32,
) -> Result<ScmsPdu> {
    let tbs = ToBeSignedCertificate::new(
        CertificateId::none(()),
        HashedId3(FixedOctetString::from([0u8; 3])),
        CrlSeries(Uint16(0)),
        ValidityPeriod::new(
            Time32(Uint32(first_batch_start)),
            Ieee1609Duration::minutes(Uint16(10_140)),
        ),
        None,
        None,
        Some(SequenceOfPsidSsp(vec![PsidSsp::new(
            Psid(Integer::from(BSM_PSID)),
            None,
        )])),
        None,
        None,
        None,
        None,
        VerificationKeyIndicator::verificationKey(sig1609_key_of(cat_signing)?),
        None,
        None,
        None,
        None,
    );
    Ok(pdu(ScmsPduContent::ee_ra(
        EeRaInterfacePdu::eeRaCertRequest(EeRaCertRequest::new(
            Uint8(STRUCT_VERSION),
            Time32(Uint32(generation_time)),
            CertificateType::explicit,
            tbs,
            Some(AdditionalParams::original(ButterflyParamsOriginal::new(
                ButterflyExpansion::aes128(OctetString::from(ck.to_vec())),
                public_encryption_key(cat_encryption)?,
                ButterflyExpansion::aes128(OctetString::from(ek.to_vec())),
            ))),
        )),
    )))
}

/// [`ee_ra_cert_request`] from a caterpillar.
pub fn ee_ra_cert_request_from(
    generation_time: u32,
    cat: &Caterpillar,
    first_batch_start: u32,
) -> Result<ScmsPdu> {
    ee_ra_cert_request(
        generation_time,
        &cat.signing_public(),
        &cat.encryption_public(),
        cat.ck(),
        cat.ek(),
        first_batch_start,
    )
}

/// `ee-ra.raEeCertAck` (03 §3.3): `firstI` present for butterfly.
pub fn ra_ee_cert_ack(
    generation_time: u32,
    request_hash: [u8; 8],
    first_i: u16,
    next_dl_time: u32,
) -> ScmsPdu {
    pdu(ScmsPduContent::ee_ra(EeRaInterfacePdu::raEeCertAck(
        RaEeCertAck::new(
            Uint8(STRUCT_VERSION),
            Time32(Uint32(generation_time)),
            id8(request_hash),
            Some(IValue(Uint16(first_i))),
            Time32(Uint32(next_dl_time)),
        ),
    )))
}

/// `ee-ra.raEeCertInfo` (the `.info` file, `acpcTreeId` ABSENT, sent unsecured).
pub fn ra_ee_cert_info(
    generation_time: u32,
    current_i: u16,
    request_hash: [u8; 8],
    next_dl_time: u32,
) -> ScmsPdu {
    pdu(ScmsPduContent::ee_ra(EeRaInterfacePdu::raEeCertInfo(
        RaEeCertInfo::new(
            Uint8(STRUCT_VERSION),
            Time32(Uint32(generation_time)),
            IValue(Uint16(current_i)),
            id8(request_hash),
            Time32(Uint32(next_dl_time)),
            None,
        ),
    )))
}

/// `ee-ra.eeRaDownloadRequest`.
pub fn ee_ra_download_request(generation_time: u32, filename: &str) -> ScmsPdu {
    pdu(ScmsPduContent::ee_ra(
        EeRaInterfacePdu::eeRaDownloadRequest(EeRaDownloadRequest::new(
            Time32(Uint32(generation_time)),
            filename.into(),
        )),
    ))
}

/// The batch file name of 1609.2.1 §8.2.2 as the US profile uses it:
/// `<requestHash, 16 upper-case hex>_<i, upper-case hex>.zip` (03 §4.2: `AB09281C9867DE53_F.zip`
/// is i = 15).
pub fn batch_filename(request_hash: [u8; 8], i: u32) -> String {
    let mut s = String::with_capacity(32);
    for b in request_hash {
        s.push_str(&format!("{b:02X}"));
    }
    s.push_str(&format!("_{i:X}.zip"));
    s
}

/// The `.info` file name: `<requestHash, 16 upper-case hex>.info` (03 §4.2).
pub fn info_filename(request_hash: [u8; 8]) -> String {
    let mut s = String::with_capacity(24);
    for b in request_hash {
        s.push_str(&format!("{b:02X}"));
    }
    s.push_str(".info");
    s
}

// ---------------------------------------------------------------------------------------
// Wrappers
// ---------------------------------------------------------------------------------------

/// `Ieee1609Dot2Data-Unsecured{ScmsPdu}`.
pub fn unsecured(p: &ScmsPdu) -> Result<Vec<u8>> {
    coer(&Ieee1609Dot2Data::new(
        Uint8(PROTOCOL_VERSION),
        Ieee1609Dot2Content::unsecuredData(Opaque(coer(p)?.into())),
    ))
}

/// `Ieee1609Dot2Data-Signed{ScmsPdu, SecurityMgmtPsid}`: header `psid 35` only, signer
/// `SignerSingleCert` (the certificate attached, never a digest).
pub fn signed(p: &ScmsPdu, signer: &Certificate, key: &Signer1609) -> Result<Vec<u8>> {
    signed_bytes(coer(p)?, signer, key)
}

fn signed_bytes(payload: Vec<u8>, signer: &Certificate, key: &Signer1609) -> Result<Vec<u8>> {
    let d = sig1609::sign_spdu(
        sig1609::header_psid_only(SECURITY_MGMT_PSID),
        payload,
        sig1609::SpduSigner::Certificate(signer.clone()),
        key,
    )
    .map_err(|e| PduError::Coer(e.to_string()))?;
    coer(&d)
}

/// `Ieee1609Dot2Data-Encrypted{T}` to a certificate's `encryptionKey`: one `certRecipInfo`
/// (recipientId = the certificate's HashedId8, ECIES P1 = SHA-256 of its canonical COER,
/// 02 §8.1), AES-128-CCM over `plaintext`.
pub fn encrypt_to_cert(
    plaintext: &[u8],
    recipient: &Certificate,
    recipient_key: &Point,
    g: &mut Drbg,
) -> Result<Vec<u8>> {
    let canon = cert::canonical_coer(recipient).map_err(|_| PduError::Shape("recipient cert"))?;
    let p1 = ecies::p1(ecies::RecipientInfoKind::CertRecipInfo, &canon);
    let ri = |pk| RecipientInfo::certRecipInfo(pk);
    encrypt_pk(plaintext, recipient_key, cert_id8(recipient), &p1, ri, g)
}

/// `Ieee1609Dot2Data-Encrypted{T}` to a bare response key (`rekRecipInfo`, P1 = SHA-256(""),
/// 02 §8.1): the butterfly response to `J_ij`.
pub fn encrypt_to_key(plaintext: &[u8], recipient_key: &Point, g: &mut Drbg) -> Result<Vec<u8>> {
    let rid = hashedid::id8_bytes(&hashedid::hashed_id8(&coer(&public_encryption_key(
        recipient_key,
    )?)?));
    let p1 = ecies::p1(ecies::RecipientInfoKind::RekRecipInfo, &[]);
    encrypt_pk(
        plaintext,
        recipient_key,
        rid,
        &p1,
        RecipientInfo::rekRecipInfo,
        g,
    )
}

fn encrypt_pk(
    plaintext: &[u8],
    recipient_key: &Point,
    rid: [u8; 8],
    p1: &[u8; 32],
    ri: impl Fn(PKRecipientInfo) -> RecipientInfo,
    g: &mut Drbg,
) -> Result<Vec<u8>> {
    let data_key = g.key16();
    let nonce = g.nonce12();
    let v = g.scalar();
    let wrap = ecies::encrypt(recipient_key, &data_key, p1, &v)
        .map_err(|_| PduError::Key("ECIES wrap"))?;
    let ct = ccm::encrypt(&data_key, &nonce, plaintext).map_err(|_| PduError::Key("CCM"))?;
    let enc_key = EncryptedDataEncryptionKey::eciesNistP256(EciesP256EncryptedKey::new(
        wrap.v
            .to_ecc_point()
            .map_err(|_| PduError::Key("identity"))?,
        OctetString::from_slice(&wrap.c),
        OctetString::from_slice(&wrap.t),
    ));
    let recipients = SequenceOfRecipientInfo(SequenceOf::from(vec![ri(PKRecipientInfo::new(
        id8(rid),
        enc_key,
    ))]));
    coer(&Ieee1609Dot2Data::new(
        Uint8(PROTOCOL_VERSION),
        Ieee1609Dot2Content::encryptedData(EncryptedData::new(
            recipients,
            SymmetricCiphertext::aes128ccm(One28BitCcmCiphertext::new(
                OctetString::from_slice(&nonce),
                Opaque(OctetString::from(ct)),
            )),
        )),
    ))
}

/// `Ieee1609Dot2Data-SymmEncryptedSingleRecipient{T}`: one `symmRecipInfo` whose
/// recipientId is the HashedId8 of the COER `SymmetricEncryptionKey` (02 §8.1) and whose
/// `encKey` is the fresh data key CCM-encrypted under `shared` (LA→ACA PLVs, 03 §3.3).
pub fn symm_encrypt_single(plaintext: &[u8], shared: &[u8; 16], g: &mut Drbg) -> Result<Vec<u8>> {
    let rid = symm_recipient_id(shared)?;
    let data_key = g.key16();
    let n1 = g.nonce12();
    let n2 = g.nonce12();
    let wrapped = ccm::encrypt(shared, &n1, &data_key).map_err(|_| PduError::Key("CCM"))?;
    let ct = ccm::encrypt(&data_key, &n2, plaintext).map_err(|_| PduError::Key("CCM"))?;
    let enc_key = SymmetricCiphertext::aes128ccm(One28BitCcmCiphertext::new(
        OctetString::from_slice(&n1),
        Opaque(OctetString::from(wrapped)),
    ));
    let recipients = SequenceOfRecipientInfo(SequenceOf::from(vec![RecipientInfo::symmRecipInfo(
        SymmRecipientInfo::new(id8(rid), enc_key),
    )]));
    coer(&Ieee1609Dot2Data::new(
        Uint8(PROTOCOL_VERSION),
        Ieee1609Dot2Content::encryptedData(EncryptedData::new(
            recipients,
            SymmetricCiphertext::aes128ccm(One28BitCcmCiphertext::new(
                OctetString::from_slice(&n2),
                Opaque(OctetString::from(ct)),
            )),
        )),
    ))
}

/// HashedId8 of COER `SymmetricEncryptionKey::aes128Ccm(key)`.
pub fn symm_recipient_id(key: &[u8; 16]) -> Result<[u8; 8]> {
    let k = SymmetricEncryptionKey::aes128Ccm(OctetString::from(key.to_vec()));
    Ok(hashedid::id8_bytes(&hashedid::hashed_id8(&coer(&k)?)))
}

/// `…-SignedEncrypted{ScmsPdu, 35}`: [`signed`], then [`encrypt_to_cert`].
pub fn signed_encrypted(
    p: &ScmsPdu,
    signer: &Certificate,
    key: &Signer1609,
    recipient: &Certificate,
    recipient_key: &Point,
    g: &mut Drbg,
) -> Result<Vec<u8>> {
    encrypt_to_cert(&signed(p, signer, key)?, recipient, recipient_key, g)
}

/// `…-EncryptedSigned{ScmsPdu, 35}`: encrypt to a response key (`rekRecipInfo`), then
/// sign the encrypted `Ieee1609Dot2Data` (carried as the signed payload) — the
/// `AcaEeCertResponsePrivateSpdu` shape (03 §3.2).
pub fn encrypted_signed(
    p: &ScmsPdu,
    response_key: &Point,
    signer: &Certificate,
    key: &Signer1609,
    g: &mut Drbg,
) -> Result<Vec<u8>> {
    let inner = Ieee1609Dot2Data::new(
        Uint8(PROTOCOL_VERSION),
        Ieee1609Dot2Content::unsecuredData(Opaque(coer(p)?.into())),
    );
    let enc = encrypt_to_key(&coer(&inner)?, response_key, g)?;
    signed_bytes(enc, signer, key)
}

/// Who signs a certificate request.
#[derive(Clone, Copy)]
pub enum RequestSigner<'a> {
    /// `SignerSelf`: signed with the key being certified; signer input empty.
    SelfSigned(&'a Signer1609),
    /// `SignerSingleCert`: the enrolment (or RA) certificate and its key.
    Cert(&'a Certificate, &'a Signer1609),
}

/// `Ieee1609Dot2Data-SignedCertRequest{T, Signer}`: `content.signedCertificateRequest`
/// CONTAINING `SignedCertificateRequest`, the signature over
/// `H(H(COER tbsRequest) ‖ H(signer input))` with the signer input empty for `self` and
/// the signer's certificate otherwise ([ASN-2.1] Protocol, `SignedCertificateRequest`).
pub fn signed_cert_request(p: &ScmsPdu, signer: RequestSigner<'_>) -> Result<Vec<u8>> {
    let tbs = coer(p)?;
    let (sid, sig, hash) = match signer {
        RequestSigner::SelfSigned(k) => (
            SignerIdentifier::R_self(()),
            k.sign_1609(&tbs, b""),
            k.hash(),
        ),
        RequestSigner::Cert(c, k) => {
            let input =
                sig1609::canonical_cert_coer(c).map_err(|e| PduError::Coer(e.to_string()))?;
            (
                SignerIdentifier::certificate(SequenceOfCertificate(vec![c.clone()])),
                k.sign_1609(&tbs, &input),
                k.hash(),
            )
        }
    };
    let scr = SignedCertificateRequest::new(
        hash.to_asn1(),
        ScopedCertificateRequest(p.clone()),
        sid,
        sig,
    );
    coer(&Ieee1609Dot2Data::new(
        Uint8(PROTOCOL_VERSION),
        Ieee1609Dot2Content::signedCertificateRequest(Opaque(coer(&scr)?.into())),
    ))
}

/// `…-SignedEncryptedCertRequest{T, Signer}`: [`signed_cert_request`], then
/// [`encrypt_to_cert`].
pub fn signed_encrypted_cert_request(
    p: &ScmsPdu,
    signer: RequestSigner<'_>,
    recipient: &Certificate,
    recipient_key: &Point,
    g: &mut Drbg,
) -> Result<Vec<u8>> {
    encrypt_to_cert(
        &signed_cert_request(p, signer)?,
        recipient,
        recipient_key,
        g,
    )
}

// ---------------------------------------------------------------------------------------
// Openers
// ---------------------------------------------------------------------------------------

fn outer(bytes: &[u8]) -> Result<Ieee1609Dot2Data> {
    let d: Ieee1609Dot2Data = decode(bytes)?;
    if d.protocol_version.0 != PROTOCOL_VERSION {
        return Err(PduError::Version("protocolVersion"));
    }
    if coer(&d)? != bytes {
        return Err(PduError::Shape("not canonical COER"));
    }
    Ok(d)
}

fn scms_pdu(bytes: &[u8]) -> Result<ScmsPdu> {
    let p: ScmsPdu = decode(bytes)?;
    if p.version.0 != SCMS_PDU_VERSION {
        return Err(PduError::Version("ScmsPdu.version"));
    }
    Ok(p)
}

/// Opens `Ieee1609Dot2Data-Unsecured{ScmsPdu}`.
pub fn open_unsecured(bytes: &[u8]) -> Result<ScmsPdu> {
    match outer(bytes)?.content {
        Ieee1609Dot2Content::unsecuredData(Opaque(b)) => scms_pdu(&b),
        _ => Err(PduError::Shape("expected unsecuredData")),
    }
}

/// What [`open_signed`] returns.
#[derive(Debug, Clone)]
pub struct OpenedSigned {
    /// The protected payload bytes (an `ScmsPdu`, or for EncryptedSigned the encrypted
    /// `Ieee1609Dot2Data`).
    pub payload: Vec<u8>,
    /// The signer's certificate.
    pub signer: Certificate,
}

/// Opens and verifies `Ieee1609Dot2Data-Signed{T, 35}`: PSID 35, every other header
/// field absent, `SignerSingleCert`, the signer chains to `pki`, the signature verifies.
pub fn open_signed(bytes: &[u8], pki: &ScmsPki) -> Result<OpenedSigned> {
    let d = outer(bytes)?;
    if let Ieee1609Dot2Content::signedData(sd) = &d.content {
        if !matches!(&sd.signer, SignerIdentifier::certificate(v) if v.0.len() == 1) {
            return Err(PduError::Signer(
                "SignerSingleCert: exactly one certificate",
            ));
        }
    }
    let o = sig1609::open_signed(&d)
        .ok_or(PduError::Shape("expected signedData over unsecuredData"))?;
    if o.psid != SECURITY_MGMT_PSID {
        return Err(PduError::Psid(o.psid));
    }
    let h = &o.header;
    for (absent, name) in [
        (h.generation_time.is_none(), "generationTime"),
        (h.expiry_time.is_none(), "expiryTime"),
        (h.generation_location.is_none(), "generationLocation"),
        (h.p2pcd_learning_request.is_none(), "p2pcdLearningRequest"),
        (h.missing_crl_identifier.is_none(), "missingCrlIdentifier"),
        (h.encryption_key.is_none(), "encryptionKey"),
    ] {
        if !absent {
            return Err(PduError::HeaderField(name));
        }
    }
    let sig1609::SpduSigner::Certificate(signer) = o.signer.clone() else {
        return Err(PduError::Signer("SignerSingleCert"));
    };
    let (hash, key) = pki.signer_key(&signer)?;
    if hash != o.hash {
        return Err(PduError::BadSignature);
    }
    let input = sig1609::canonical_cert_coer(&signer).map_err(|e| PduError::Coer(e.to_string()))?;
    if !sig1609::verify_digest(
        hash,
        &key,
        &sig1609::digest_1609(hash, &o.tbs_coer, &input),
        &o.signature,
    ) {
        return Err(PduError::BadSignature);
    }
    Ok(OpenedSigned {
        payload: o.payload,
        signer,
    })
}

/// [`open_signed`], then the payload as an `ScmsPdu`.
pub fn open_signed_pdu(bytes: &[u8], pki: &ScmsPki) -> Result<(ScmsPdu, Certificate)> {
    let o = open_signed(bytes, pki)?;
    Ok((scms_pdu(&o.payload)?, o.signer))
}

/// Who signed a certificate request, as [`open_signed_cert_request`] found it.
#[derive(Debug, Clone)]
pub enum RequestSignerFound {
    /// `self`, verified under the requested `verifyKeyIndicator`.
    SelfSigned,
    /// A certificate that chains to the PKI.
    Cert(Certificate),
}

/// Opens and verifies `Ieee1609Dot2Data-SignedCertRequest`. A self-signed request is
/// verified under the key its own `tbsCert.verifyKeyIndicator` carries.
pub fn open_signed_cert_request(
    bytes: &[u8],
    pki: &ScmsPki,
) -> Result<(ScmsPdu, RequestSignerFound)> {
    let Ieee1609Dot2Content::signedCertificateRequest(Opaque(b)) = outer(bytes)?.content else {
        return Err(PduError::Shape("expected signedCertificateRequest"));
    };
    let scr: SignedCertificateRequest = decode(&b)?;
    let p = scr.tbs_request.0.clone();
    if p.version.0 != SCMS_PDU_VERSION {
        return Err(PduError::Version("ScmsPdu.version"));
    }
    let tbs = coer(&p)?;
    let hash = Hash1609::from_asn1(&scr.hash_algorithm_id).ok_or(PduError::Shape("hashId"))?;
    let (key_hash, key, input, found) = match &scr.signer {
        SignerIdentifier::R_self(()) => {
            let vki = match &p.content {
                ScmsPduContent::eca_ee(EcaEeInterfacePdu::eeEcaCertRequest(r)) => {
                    &r.tbs_cert.verify_key_indicator
                }
                ScmsPduContent::ee_ra(EeRaInterfacePdu::eeRaCertRequest(r)) => {
                    &r.tbs_cert.verify_key_indicator
                }
                _ => {
                    return Err(PduError::Signer(
                        "self-signed request of a type that cannot be",
                    ));
                }
            };
            let VerificationKeyIndicator::verificationKey(k) = vki else {
                return Err(PduError::Signer(
                    "self-signed request without a verificationKey",
                ));
            };
            let (h, k) =
                sig1609::verification_key_sec1(k).ok_or(PduError::Key("verificationKey"))?;
            (h, k, Vec::new(), RequestSignerFound::SelfSigned)
        }
        SignerIdentifier::certificate(v) if v.0.len() == 1 => {
            let c = &v.0[0];
            let (h, k) = pki.signer_key(c)?;
            let input =
                sig1609::canonical_cert_coer(c).map_err(|e| PduError::Coer(e.to_string()))?;
            (h, k, input, RequestSignerFound::Cert(c.clone()))
        }
        _ => return Err(PduError::Signer("SignerSelf or SignerSingleCert")),
    };
    if key_hash != hash
        || !sig1609::verify_digest(
            hash,
            &key,
            &sig1609::digest_1609(hash, &tbs, &input),
            &scr.signature,
        )
    {
        return Err(PduError::BadSignature);
    }
    Ok((p, found))
}

/// Decrypts an `encryptedData` addressed by `certRecipInfo` to `recipient` (whose
/// `encryptionKey` private half is `private`). Returns the plaintext.
pub fn decrypt_cert(bytes: &[u8], recipient: &Certificate, private: &Scalar) -> Result<Vec<u8>> {
    let canon = cert::canonical_coer(recipient).map_err(|_| PduError::Shape("recipient cert"))?;
    let p1 = ecies::p1(ecies::RecipientInfoKind::CertRecipInfo, &canon);
    decrypt_pk(bytes, cert_id8(recipient), &p1, private, |r| match r {
        RecipientInfo::certRecipInfo(pk) => Some(pk),
        _ => None,
    })
}

/// Decrypts an `encryptedData` addressed by `rekRecipInfo` to `private·G`.
pub fn decrypt_key(bytes: &[u8], private: &Scalar) -> Result<Vec<u8>> {
    let rid = hashedid::id8_bytes(&hashedid::hashed_id8(&coer(&public_encryption_key(
        &Point::mul_base(private),
    )?)?));
    let p1 = ecies::p1(ecies::RecipientInfoKind::RekRecipInfo, &[]);
    decrypt_pk(bytes, rid, &p1, private, |r| match r {
        RecipientInfo::rekRecipInfo(pk) => Some(pk),
        _ => None,
    })
}

fn ccm_parts(c: &SymmetricCiphertext) -> Result<([u8; 12], Vec<u8>)> {
    let SymmetricCiphertext::aes128ccm(c) = c else {
        return Err(PduError::Decrypt("not aes128ccm"));
    };
    let nonce: [u8; 12] = c.nonce[..]
        .try_into()
        .map_err(|_| PduError::Decrypt("nonce width"))?;
    Ok((nonce, c.ccm_ciphertext.0.to_vec()))
}

fn decrypt_pk(
    bytes: &[u8],
    rid: [u8; 8],
    p1: &[u8; 32],
    private: &Scalar,
    pick: impl Fn(&RecipientInfo) -> Option<&PKRecipientInfo>,
) -> Result<Vec<u8>> {
    let Ieee1609Dot2Content::encryptedData(enc) = outer(bytes)?.content else {
        return Err(PduError::Shape("expected encryptedData"));
    };
    let pk = enc
        .recipients
        .0
        .iter()
        .filter_map(&pick)
        .find(|pk| hashedid::id8_bytes(&pk.recipient_id) == rid)
        .ok_or(PduError::NotForMe)?;
    let EncryptedDataEncryptionKey::eciesNistP256(k) = &pk.enc_key else {
        return Err(PduError::Decrypt("not eciesNistP256"));
    };
    let wrapped = ecies::EciesP256EncryptedKey {
        v: point_of(&k.v)?,
        c: k.c[..]
            .try_into()
            .map_err(|_| PduError::Decrypt("ECIES c width"))?,
        t: k.t[..]
            .try_into()
            .map_err(|_| PduError::Decrypt("ECIES t width"))?,
    };
    let data_key = ecies::decrypt(private, &wrapped, p1).map_err(|_| PduError::Decrypt("ECIES"))?;
    let (nonce, ct) = ccm_parts(&enc.ciphertext)?;
    ccm::decrypt(&data_key, &nonce, &ct).map_err(|_| PduError::Decrypt("CCM"))
}

/// Decrypts a `SymmEncryptedSingleRecipient` with the shared key.
pub fn decrypt_symm(bytes: &[u8], shared: &[u8; 16]) -> Result<Vec<u8>> {
    let Ieee1609Dot2Content::encryptedData(enc) = outer(bytes)?.content else {
        return Err(PduError::Shape("expected encryptedData"));
    };
    if enc.recipients.0.len() != 1 {
        return Err(PduError::Shape(
            "SymmEncryptedSingleRecipient has exactly one RecipientInfo",
        ));
    }
    let RecipientInfo::symmRecipInfo(s) = &enc.recipients.0[0] else {
        return Err(PduError::Shape("symmRecipInfo"));
    };
    if hashedid::id8_bytes(&s.recipient_id) != symm_recipient_id(shared)? {
        return Err(PduError::NotForMe);
    }
    let (n1, wrapped) = ccm_parts(&s.enc_key)?;
    let dk = ccm::decrypt(shared, &n1, &wrapped).map_err(|_| PduError::Decrypt("CCM key wrap"))?;
    let data_key: [u8; 16] = dk[..]
        .try_into()
        .map_err(|_| PduError::Decrypt("data key width"))?;
    let (n2, ct) = ccm_parts(&enc.ciphertext)?;
    ccm::decrypt(&data_key, &n2, &ct).map_err(|_| PduError::Decrypt("CCM"))
}

// ---------------------------------------------------------------------------------------
// Emission log
// ---------------------------------------------------------------------------------------

/// The 1609.2.1 SPDU types the run emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub enum SpduKind {
    /// `EeEcaCertRequestSpdu` (SignedCertRequest, SignerSelf).
    EeEcaCertRequest,
    /// `EcaEeCertResponseSpdu` (Signed, ECA).
    EcaEeCertResponse,
    /// `EeRaCertRequestSpdu` (SignedEncryptedCertRequest, enrolment certificate → RA).
    EeRaCertRequest,
    /// `RaEeCertAckSpdu` (Signed, RA).
    RaEeCertAck,
    /// `EeRaDownloadRequestSpdu` (SignedEncrypted, enrolment certificate → RA).
    EeRaDownloadRequest,
}

impl SpduKind {
    /// The 1609.2.1 type name.
    pub const fn name(self) -> &'static str {
        match self {
            SpduKind::EeEcaCertRequest => "EeEcaCertRequestSpdu",
            SpduKind::EcaEeCertResponse => "EcaEeCertResponseSpdu",
            SpduKind::EeRaCertRequest => "EeRaCertRequestSpdu",
            SpduKind::RaEeCertAck => "RaEeCertAckSpdu",
            SpduKind::EeRaDownloadRequest => "EeRaDownloadRequestSpdu",
        }
    }
}

/// One SPDU the run put on a link, for the recorder, the oracle and attack studies.
#[derive(Debug, Clone)]
pub struct EmittedPdu {
    /// Its type.
    pub kind: SpduKind,
    /// Sender node index.
    pub from: u32,
    /// Receiver node index.
    pub to: u32,
    /// Simulated send time (ns).
    pub at: u64,
    /// The bytes (kept only when [`crate::scms::params::PduLayer::keep_bytes`] is set).
    pub bytes: Option<Pdu>,
    /// Encoded length.
    pub len: u32,
}

/// A PDU a receiver refused, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedPdu {
    /// Its type.
    pub kind: SpduKind,
    /// Receiver node index.
    pub at_node: u32,
    /// Simulated time (ns).
    pub at: u64,
    /// Why.
    pub reason: PduError,
}

#[cfg(test)]
#[path = "pdu_tests.rs"]
mod tests;
