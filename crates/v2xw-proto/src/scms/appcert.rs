//! Non-pseudonym end-entity certificates (WP-B3b): RSU application certificates and OBU
//! identification certificates, as IEEE 1609.2.1-2022 SPDUs and ECQV-1609 issuance.
//!
//! | | RSU application | OBU identification |
//! |---|---|---|
//! | holder | roadside unit (SPaT/MAP signing) | emergency or other privileged vehicle |
//! | validity | 1 week + 1 h, `hours(169)` ([CAMP122] Table 68 `rse_application_cert_validity` 169 h / overlap 1 h; 03 §6.3, §7) | 1 month + 1 h ([CAMP122] Table 3, SCMS-1370; 03 §7) |
//! | key | a fresh request key `R_U = k_U·G` per certificate, no butterfly | original butterfly, `(i, j = 0)` with a local i-value (03 §6.1 `baseCertNumber`) |
//! | request `additionalParams` | `encryptionKey` (the response key `U`) | `original {ck, H, ek}` |
//! | response | `AcaEeCertResponsePrivateSpdu`: encrypted to `U`, signed by the issuing ACA | the same, encrypted to the cocoon key `J_i0` |
//! | shuffle | none | none (the RA does not shuffle, 03 §7) |
//! | `id` | not `linkageData` ([SMP] §6.3.3); `none` here | the same |
//! | `crlSeries` | 3, revoked by HashedId10 on a hash CRL ([SMP] §6.3.3, Table 1; 03 §1 fact 7) | 3 |
//! | `cracaId` | HashedId3 of the CRACA: the ICA, because the US CRL signer is `issuerIsCraca` under the ICA (published CRL-signer SSP `CrlSsp{issuerIsCraca, 4,3,1,2}`, `governance.rs`) | the same |
//!
//! The exchange, every leg a real SPDU:
//!
//! ```text
//! EE  → RA : EeRaCertRequestSpdu        SignedEncryptedCertRequest, enrolment cert → RA
//! RA  → EE : RaEeCertAckSpdu            Signed, RA (firstI only for butterfly)
//! RA  → ACA: RaAcaCertRequestSpdu       SignedCertRequest, SignerSingleCert = RA
//!            {type implicit, flags 00, linkageInfo ABSENT, certEncKey U | J,
//!             tbsCert.verifyKeyIndicator = reconstructionValue(R_U | B_i0)}
//! ACA → RA : AcaRaCertResponseSpdu      Signed, ACA {requestHash, acaResponse.private}
//! EE  → RA : EeRaDownloadRequestSpdu    SignedEncrypted, enrolment cert → RA
//! RA  → EE : AcaEeCertResponsePrivateSpdu, untouched ("the RA zips it untouched", 03 §7)
//! ```
//!
//! The ASN.1 comments of 1609.2.1-2022 fix the field rules used here
//! (`tools/oracle/asn1/ieee1609.2.1-2022`): `EeRaCertRequest.tbsCert.verifyKeyIndicator`
//! is a `verificationKey` "even if type is implicit", and is "the input public key value
//! for implicit certificate generation" when butterfly expansion is not used;
//! `additionalParams.encryptionKey` is "the public key for encrypting the certificate if
//! the butterfly key mechanism is not used"; in `RaAcaCertRequest` "if type is implicit,
//! the request is valid only if tbsCert.verifyKeyIndicator is a reconstructionValue", and
//! `certEncKey` present with `cubk` clear selects `AcaEeCertResponsePrivateSpdu`; the
//! ACA replaces `cracaId` and `crlSeries`.
//!
//! **Assumptions (labelled, not cited):** the identification period is 30 days, so the
//! validity is `hours(721)` (CAMP says "1 month"); `id` is `none` for both types; the
//! identification certificate's `appPermissions` are BSM (0x20) and SRM (0x204096, signal
//! preemption), with no SSP; the RSU certificate carries SPaT/MAP (0x82) only.

use p256::Scalar;
use rasn::types::{FixedOctetString, Integer, OctetString};
use v2xw_msg::sec_types::ieee1609_dot2::{
    Certificate, CertificateId, CertificateType, IssuerIdentifier, ToBeSignedCertificate,
    VerificationKeyIndicator,
};
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    BasePublicEncryptionKey, CrlSeries, Duration as Ieee1609Duration, HashedId3, Psid, PsidSsp,
    PublicEncryptionKey, PublicVerificationKey, SequenceOfPsidSsp, SymmAlgorithm, Time32, Uint8,
    Uint16, Uint32, ValidityPeriod,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_aca_ee_interface::AcaEeInterfacePdu;
use v2xw_msg::sec_types::ieee1609_dot2_dot1_aca_ra_interface::{AcaRaInterfacePdu, AcaResponse};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_ee_ra_interface::{
    AdditionalParams, ButterflyExpansion, ButterflyParamsOriginal, EeRaCertRequest,
    EeRaInterfacePdu,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::{ScmsPdu, ScmsPduContent};
use v2xw_sec::butterfly::{self, Caterpillar};
use v2xw_sec::cert;
use v2xw_sec::crl::SERIES_APPLICATION_IDENTIFICATION;
use v2xw_sec::ec::{self, Point};
use v2xw_sec::hashedid;
use v2xw_sec::sig1609::Signer1609;

use crate::scms::pdu::{
    self, ButterflyOriginal, Drbg, PduError, RequestSigner, RequestSignerFound, Result, ScmsPki,
};

/// SPaT and MAP, `0x82` (130): the intersection-safety-and-awareness PSID
/// (`v2xw_msg::registry`).
pub const SPAT_MAP_PSID: u64 = 0x82;
/// The BSM PSID (0x20).
pub const BSM_PSID: u64 = 0x20;
/// SRM, traffic-signal-request (0x204096): signal preemption by an emergency vehicle.
pub const SRM_PSID: u64 = 0x20_4096;

// The SSPs of the CV-pilot certificates, as [CAMP122] Table 1 "Supported V2X
// Applications" prints them (PDF pp. 19–37, column "SSP Value to Appear in End Entity
// Certificates for CV Pilots"): BSM (0x20) "SSP: absent"; SPaT/MAP (0x82), TIM (0x83)
// and the SRM row print no value ("SAE subgroup has been tasked with developing SSP
// specification … draft available"; "No SSP definition currently known"), so the
// engine's application and identification certificates carry none. The one SSP the
// table publishes for an end-entity certificate is Wyoming's, below.

/// Vehicle-initiated distress notification, PSID `0pC0-00-02` = 0x4082 (16 514), Wyoming
/// DOT, on an OBU identification certificate ([CAMP122] Table 1 row 44).
pub const DISTRESS_NOTIFICATION_PSID: u64 = 0x4082;
/// Its SSP: a UPER J2945/2 DSRC-SSP, version 1, one `SSPentry {index
/// msg-travelerInformation (31), constraint SSPconstraintAll}`, "UPER encoded in hex:
/// 00 80 01 F0 40" ([CAMP122] Table 1 row 44).
pub const DISTRESS_NOTIFICATION_SSP: [u8; 5] = [0x00, 0x80, 0x01, 0xF0, 0x40];

/// The Wyoming distress-notification `PsidSsp`, whose COER [CAMP122] Table 1 row 44
/// prints as "80 02 40 82 80 05 00 80 01 F0 40" (opaque SSP). The engine's default
/// identification profile ([`AppCertKind::psids`]) does not include it: no published
/// source puts it on emergency vehicles. It is here for scenarios and attack studies that
/// model the Wyoming application.
#[must_use]
pub fn distress_notification_psid_ssp() -> PsidSsp {
    PsidSsp::new(
        Psid(Integer::from(DISTRESS_NOTIFICATION_PSID)),
        Some(
            v2xw_msg::sec_types::ieee1609_dot2_base_types::ServiceSpecificPermissions::opaque(
                DISTRESS_NOTIFICATION_SSP.to_vec().into(),
            ),
        ),
    )
}

/// Which non-pseudonym certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub enum AppCertKind {
    /// RSU application certificate: 1 week + 1 h, no butterfly, requested weekly.
    RsuApplication,
    /// OBU identification certificate: 1 month + 1 h, original butterfly, no shuffle.
    ObuIdentification,
}

impl AppCertKind {
    /// Stable name.
    pub const fn name(self) -> &'static str {
        match self {
            AppCertKind::RsuApplication => "rsu-application",
            AppCertKind::ObuIdentification => "obu-identification",
        }
    }

    /// Validity in hours: 169 for the RSU certificate ([CAMP122] Table 68); 30 days + 1 h
    /// for identification (assumption, see the module notes).
    pub const fn validity_hours(self) -> u16 {
        match self {
            AppCertKind::RsuApplication => 169,
            AppCertKind::ObuIdentification => 721,
        }
    }

    /// The `appPermissions` PSIDs.
    pub const fn psids(self) -> &'static [u64] {
        match self {
            AppCertKind::RsuApplication => &[SPAT_MAP_PSID],
            AppCertKind::ObuIdentification => &[BSM_PSID, SRM_PSID],
        }
    }

    /// Whether the request uses the original butterfly parameters.
    pub const fn butterfly(self) -> bool {
        matches!(self, AppCertKind::ObuIdentification)
    }
}

/// What the end entity keeps from a request to open the response.
#[derive(Clone)]
pub enum RequestKeys {
    /// No butterfly: the ECQV request key `k_U` and the response decryption key `u`.
    Plain {
        /// `k_U`, with `R_U = k_U·G` in the request.
        k_u: Scalar,
        /// `u`, with `U = u·G` the request's `additionalParams.encryptionKey`.
        response: Scalar,
    },
    /// Original butterfly at `(i, j = 0)`.
    Butterfly {
        /// The caterpillar.
        cat: Box<Caterpillar>,
        /// The local i-value.
        i: u32,
    },
}

impl core::fmt::Debug for RequestKeys {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RequestKeys::Plain { .. } => f.write_str("RequestKeys::Plain"),
            RequestKeys::Butterfly { i, .. } => write!(f, "RequestKeys::Butterfly(i={i})"),
        }
    }
}

impl RequestKeys {
    /// Fresh keys for `kind` at local period `i`, drawn from `g`.
    pub fn fresh(kind: AppCertKind, i: u32, g: &mut Drbg) -> Result<RequestKeys> {
        Ok(match kind {
            AppCertKind::RsuApplication => RequestKeys::Plain {
                k_u: g.scalar(),
                response: g.scalar(),
            },
            AppCertKind::ObuIdentification => {
                let mut seed = [0u8; butterfly::SEED_BYTES];
                seed[..32].copy_from_slice(&g.next32());
                seed[32..].copy_from_slice(&g.next32());
                RequestKeys::Butterfly {
                    cat: Box::new(
                        Caterpillar::from_seed(&seed).map_err(|_| PduError::Key("caterpillar"))?,
                    ),
                    i,
                }
            }
        })
    }

    /// The ECQV `k_U` the certificate's key is reconstructed from: `k_U`, or the cocoon
    /// signing private key `b_i0 = a + f₁(ck, (i, 0))`.
    pub fn ecqv_private(&self) -> Scalar {
        match self {
            RequestKeys::Plain { k_u, .. } => *k_u,
            RequestKeys::Butterfly { cat, i } => cat.cocoon_signing_private(*i, 0),
        }
    }

    /// The private key the response is encrypted to: `u`, or `h_i0 = h + f₂(ek, (i, 0))`.
    pub fn response_private(&self) -> Scalar {
        match self {
            RequestKeys::Plain { response, .. } => *response,
            RequestKeys::Butterfly { cat, i } => cat.encryption_private(*i, 0),
        }
    }
}

fn coer<T: rasn::Encode>(v: &T) -> Result<Vec<u8>> {
    rasn::coer::encode(v).map_err(|e| PduError::Coer(e.to_string()))
}

fn scms(content: ScmsPduContent) -> ScmsPdu {
    ScmsPdu::new(Uint8(pdu::SCMS_PDU_VERSION), content)
}

fn verification_key(p: &Point) -> Result<PublicVerificationKey> {
    Ok(PublicVerificationKey::ecdsaNistP256(
        p.to_ecc_point()
            .map_err(|_| PduError::Key("identity point"))?,
    ))
}

fn encryption_key(p: &Point) -> Result<PublicEncryptionKey> {
    Ok(PublicEncryptionKey::new(
        SymmAlgorithm::aes128Ccm,
        BasePublicEncryptionKey::eciesNistP256(
            p.to_ecc_point()
                .map_err(|_| PduError::Key("identity point"))?,
        ),
    ))
}

fn enc_point(k: &PublicEncryptionKey) -> Result<Point> {
    if k.supported_symm_alg != SymmAlgorithm::aes128Ccm {
        return Err(PduError::Shape("encryptionKey supportedSymmAlg aes128Ccm"));
    }
    let BasePublicEncryptionKey::eciesNistP256(p) = &k.public_key else {
        return Err(PduError::Shape("encryptionKey eciesNistP256"));
    };
    pdu::point_of(p)
}

/// The requested `toBeSigned` (1609.2.1 `EeRaCertRequest.tbsCert`): `id none`,
/// `cracaId 000000`, `crlSeries 0`, the kind's validity from `valid_from`, its
/// `appPermissions`, and `verificationKey` = the key the certificate derives from.
pub fn request_tbs(
    kind: AppCertKind,
    verify_key: &Point,
    valid_from: u32,
) -> Result<ToBeSignedCertificate> {
    Ok(ToBeSignedCertificate::new(
        CertificateId::none(()),
        HashedId3(FixedOctetString::from([0u8; 3])),
        CrlSeries(Uint16(0)),
        ValidityPeriod::new(
            Time32(Uint32(valid_from)),
            Ieee1609Duration::hours(Uint16(kind.validity_hours())),
        ),
        None,
        None,
        Some(SequenceOfPsidSsp(
            kind.psids()
                .iter()
                .map(|p| PsidSsp::new(Psid(Integer::from(*p)), None))
                .collect(),
        )),
        None,
        None,
        None,
        None,
        VerificationKeyIndicator::verificationKey(verification_key(verify_key)?),
        None,
        None,
        None,
        None,
    ))
}

/// `ee-ra.eeRaCertRequest` for `kind`: type implicit; no butterfly → `verificationKey`
/// `R_U` and `additionalParams.encryptionKey` `U`; identification → `verificationKey` `A`
/// and `additionalParams.original {ck, H, ek}`.
pub fn ee_ra_cert_request(
    kind: AppCertKind,
    generation_time: u32,
    keys: &RequestKeys,
    valid_from: u32,
) -> Result<ScmsPdu> {
    let (vk, params) = match keys {
        RequestKeys::Plain { k_u, response } => (
            Point::mul_base(k_u),
            AdditionalParams::encryptionKey(encryption_key(&Point::mul_base(response))?),
        ),
        RequestKeys::Butterfly { cat, .. } => (
            cat.signing_public(),
            AdditionalParams::original(ButterflyParamsOriginal::new(
                ButterflyExpansion::aes128(OctetString::from(cat.ck().to_vec())),
                encryption_key(&cat.encryption_public())?,
                ButterflyExpansion::aes128(OctetString::from(cat.ek().to_vec())),
            )),
        ),
    };
    if kind.butterfly() != matches!(keys, RequestKeys::Butterfly { .. }) {
        return Err(PduError::Shape(
            "request keys do not match the certificate kind",
        ));
    }
    Ok(scms(ScmsPduContent::ee_ra(
        EeRaInterfacePdu::eeRaCertRequest(EeRaCertRequest::new(
            Uint8(2),
            Time32(Uint32(generation_time)),
            CertificateType::implicit,
            request_tbs(kind, &vk, valid_from)?,
            Some(params),
        )),
    )))
}

/// What the RA learns from an opened request.
#[derive(Debug, Clone)]
pub struct OpenedRequest {
    /// The requested `toBeSigned`.
    pub tbs: ToBeSignedCertificate,
    /// `R_U` for the plain request; `A` for the butterfly one.
    pub verify_key: Point,
    /// `U` (plain).
    pub response_key: Option<Point>,
    /// `(A, H, ck, ek)` (butterfly).
    pub butterfly: Option<ButterflyOriginal>,
    /// The enrolment certificate that signed it.
    pub enrolment: Certificate,
}

impl OpenedRequest {
    /// The kind the request's shape says it is.
    pub fn kind(&self) -> AppCertKind {
        if self.butterfly.is_some() {
            AppCertKind::ObuIdentification
        } else {
            AppCertKind::RsuApplication
        }
    }

    /// `(request public key, certEncKey)` the RA asks the ACA for at local i-value `i`:
    /// `(R_U, U)`, or the cocoon keys `(B_i0, J_i0)` it expands (02 §9.2).
    pub fn aca_keys(&self, i: u32) -> Result<(Point, Point)> {
        match (&self.butterfly, &self.response_key) {
            (Some(b), _) => Ok(butterfly::ra_cocoon_keys(&b.a, &b.h, &b.ck, &b.ek, i, 0)),
            (None, Some(u)) => Ok((self.verify_key, *u)),
            (None, None) => Err(PduError::Shape("no response key")),
        }
    }
}

/// The RA's checks on a non-pseudonym `EeRaCertRequestSpdu`: decrypt with the RA key;
/// the inner `SignedCertificateRequest` is signed by one ECA-issued enrolment
/// certificate and verifies; `type implicit` ([SMP] §6.3.3: every profiled certificate is
/// implicit); `id` not `linkageData` (no butterfly → crlSeries 3 "and id must not be
/// LinkageData"; butterfly without LinkageData → 3); `cracaId 000000`, `crlSeries 0`;
/// `verificationKey` P-256; `additionalParams` either `encryptionKey` or `original`.
pub fn ra_open_request(pki: &ScmsPki, spdu: &[u8]) -> Result<OpenedRequest> {
    let enc = pki
        .ra
        .enc_private
        .ok_or(PduError::Key("RA encryption key"))?;
    let plain = pdu::decrypt_cert(spdu, &pki.ra.cert, &enc)?;
    let (p, signer) = pdu::open_signed_cert_request(&plain, pki)?;
    let RequestSignerFound::Cert(ec_cert) = signer else {
        return Err(PduError::Signer(
            "a certificate request is signed by an enrolment certificate",
        ));
    };
    let issued_by_eca = matches!(&ec_cert.0.issuer,
        IssuerIdentifier::sha256AndDigest(h) if hashedid::id8_bytes(h) == pki.eca.id8());
    if !issued_by_eca {
        return Err(PduError::UntrustedSigner);
    }
    let ScmsPduContent::ee_ra(EeRaInterfacePdu::eeRaCertRequest(r)) = p.content else {
        return Err(PduError::Shape("expected ee-ra.eeRaCertRequest"));
    };
    if r.r_type != CertificateType::implicit {
        return Err(PduError::Shape("US profile: the certificate is implicit"));
    }
    let t = &r.tbs_cert;
    if matches!(t.id, CertificateId::linkageData(_)) {
        return Err(PduError::Shape(
            "a non-pseudonym certificate's id is not LinkageData",
        ));
    }
    if t.craca_id.0[..] != [0u8; 3] || t.crl_series.0.0 != 0 {
        return Err(PduError::Shape("request cracaId 000000 and crlSeries 0"));
    }
    if t.app_permissions.is_none()
        || t.cert_issue_permissions.is_some()
        || t.cert_request_permissions.is_some()
    {
        return Err(PduError::Shape(
            "appPermissions present, issue/request permissions absent",
        ));
    }
    let VerificationKeyIndicator::verificationKey(PublicVerificationKey::ecdsaNistP256(vk)) =
        &t.verify_key_indicator
    else {
        return Err(PduError::Shape("verificationKey ecdsaNistP256"));
    };
    let verify_key = pdu::point_of(vk)?;
    let (response_key, bf) = match &r.additional_params {
        Some(AdditionalParams::encryptionKey(k)) => (Some(enc_point(k)?), None),
        Some(AdditionalParams::original(b)) => {
            let ButterflyExpansion::aes128(ck) = &b.signing_expansion else {
                return Err(PduError::Shape("signingExpansion aes128"));
            };
            let ButterflyExpansion::aes128(ek) = &b.encryption_expansion else {
                return Err(PduError::Shape("encryptionExpansion aes128"));
            };
            let width = |x: &OctetString| -> Result<[u8; 16]> {
                x[..]
                    .try_into()
                    .map_err(|_| PduError::Shape("expansion key is 16 bytes"))
            };
            (
                None,
                Some(ButterflyOriginal {
                    a: verify_key,
                    h: enc_point(&b.encryption_key)?,
                    ck: width(ck)?,
                    ek: width(ek)?,
                }),
            )
        }
        Some(_) => {
            return Err(PduError::Shape(
                "unified and compact-unified butterfly are not the US profile",
            ));
        }
        None => {
            return Err(PduError::Shape(
                "additionalParams: a response key or the butterfly parameters",
            ));
        }
    };
    Ok(OpenedRequest {
        tbs: r.tbs_cert,
        verify_key,
        response_key,
        butterfly: bf,
        enrolment: ec_cert,
    })
}

/// `aca-ra.raAcaCertRequest` (03 §3.3) for a non-pseudonym certificate: the one builder,
/// [`crate::scms::pdu_prov::aca_ra_request`], with no `linkageInfo`, `certEncKey` = the
/// response key and `tbsCert` = the requested one with `verifyKeyIndicator =
/// reconstructionValue(R)`.
///
/// # Errors
/// A key does not encode.
pub fn ra_aca_cert_request(
    generation_time: u32,
    requested: &ToBeSignedCertificate,
    request_public: &Point,
    cert_enc_key: &Point,
) -> Result<ScmsPdu> {
    crate::scms::pdu_prov::aca_ra_request(
        generation_time,
        requested,
        request_public,
        cert_enc_key,
        None,
    )
}

/// The RA's `RaAcaCertRequestSpdu`: [`ra_aca_cert_request`] as a `SignedCertRequest`
/// with `SignerSingleCert` = the RA certificate.
pub fn ra_aca_cert_request_spdu(
    pki: &ScmsPki,
    generation_time: u32,
    requested: &ToBeSignedCertificate,
    request_public: &Point,
    cert_enc_key: &Point,
) -> Result<Vec<u8>> {
    let p = ra_aca_cert_request(generation_time, requested, request_public, cert_enc_key)?;
    pdu::signed_cert_request(&p, RequestSigner::Cert(&pki.ra.cert, &pki.ra.key))
}

/// What the ACA produced.
#[derive(Debug, Clone)]
pub struct AcaIssued {
    /// The implicit certificate.
    pub cert: Certificate,
    /// `AcaEeCertResponsePrivateSpdu`.
    pub private_spdu: Vec<u8>,
    /// `AcaRaCertResponseSpdu` carrying it.
    pub aca_ra_spdu: Vec<u8>,
}

/// The `cracaId` the ACA puts in the certificates it issues: HashedId3 of the CRACA, the
/// certificate that issued the provider's CRL signer (`issuerIsCraca`): the root in the
/// ISS form, the ICA in the S2X form ([`ScmsPki::craca_id`], CF-CRACA).
pub fn craca_id(pki: &ScmsPki) -> Result<[u8; 3]> {
    let h = cert::cert_id3(&pki.crl_signer_issuer().cert)
        .map_err(|_| PduError::Shape("CRACA HashedId3"))?;
    Ok(h.0[..].try_into().unwrap_or([0; 3]))
}

/// The ACA for a non-pseudonym request: the one issuance,
/// [`crate::scms::pdu_prov::aca_issue_request`], with no LA keys (verify the RA's
/// `RaAcaCertRequestSpdu`, issue the implicit certificate by ECQV-1609 on the request's
/// reconstruction value with `cracaId` = [`craca_id`] and `crlSeries` 3, and answer with
/// `AcaEeCertResponsePrivateSpdu` inside a signed `AcaRaCertResponseSpdu`). A pseudonym
/// request (`linkageInfo` present) is refused.
///
/// # Errors
/// As [`crate::scms::pdu_prov::aca_issue_request`].
pub fn aca_issue(
    pki: &ScmsPki,
    request_spdu: &[u8],
    generation_time: u32,
    g: &mut Drbg,
) -> Result<AcaIssued> {
    let x = crate::scms::pdu_prov::aca_issue_request(pki, None, request_spdu, generation_time, g)?;
    if x.lv.is_some() {
        return Err(PduError::Shape(
            "linkageInfo is for pseudonym certificates (WP-B3a)",
        ));
    }
    Ok(AcaIssued {
        cert: x.certificate,
        private_spdu: x.private.bytes().to_vec(),
        aca_ra_spdu: x.to_ra.bytes().to_vec(),
    })
}

/// The RA's checks on an `AcaRaCertResponseSpdu`: signed by the ACA, `requestHash` is
/// the request it sent, the response is `private`. Returns the
/// `AcaEeCertResponsePrivateSpdu` bytes it stores for the download, unopened (the RA
/// cannot open it: it is encrypted to the EE).
pub fn ra_open_aca_response(pki: &ScmsPki, spdu: &[u8], request_hash8: [u8; 8]) -> Result<Vec<u8>> {
    let (p, signer) = pdu::open_signed_pdu(spdu, pki)?;
    if pdu::cert_id8(&signer) != pki.aca.id8() {
        return Err(PduError::Signer(
            "an AcaRaCertResponse is signed by the ACA",
        ));
    }
    let ScmsPduContent::aca_ra(AcaRaInterfacePdu::acaRaCertResponse(r)) = p.content else {
        return Err(PduError::Shape("expected aca-ra.acaRaCertResponse"));
    };
    if hashedid::id8_bytes(&r.request_hash) != request_hash8 {
        return Err(PduError::Shape(
            "requestHash is not the hash of the request sent",
        ));
    }
    let AcaResponse::private(private) = r.aca_response else {
        return Err(PduError::Shape("acaResponse.private"));
    };
    coer(&private)
}

/// An installed non-pseudonym credential.
#[derive(Clone)]
pub struct AppCredential {
    /// Which kind.
    pub kind: AppCertKind,
    /// Its local period.
    pub period: u32,
    /// The implicit certificate.
    pub cert: Certificate,
    /// The reconstructed signing key `d = e·k_U + r`.
    pub key: Signer1609,
    /// `validityPeriod.start` (Time32).
    pub valid_from: u32,
    /// `start + duration` (Time32, exclusive).
    pub valid_until: u32,
    /// HashedId10, what a series-3 hash CRL names it by.
    pub id10: [u8; 10],
}

impl AppCredential {
    /// The credential as a node's signing key: the implicit certificate, the ACA
    /// certificate `aca` that issued it, and `d`. `None` for a P-384 key, which the node
    /// stack's implicit credentials do not carry.
    #[must_use]
    pub fn implicit_key(&self, aca: &Certificate) -> Option<v2xw_sec::credential::ImplicitKey> {
        let Signer1609::P256(sk) = &self.key else {
            return None;
        };
        let d: Scalar = *sk.as_nonzero_scalar().as_ref();
        Some(v2xw_sec::credential::ImplicitKey::new(
            std::sync::Arc::new(self.cert.clone()),
            std::sync::Arc::new(aca.clone()),
            d,
        ))
    }
}

impl core::fmt::Debug for AppCredential {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "AppCredential({:?} p{} {:02x?})",
            self.kind, self.period, self.id10
        )
    }
}

/// The end entity's checks on `AcaEeCertResponsePrivateSpdu` and its installation:
/// signed by this PKI's ACA, decrypted with `keys`' response key; the certificate is
/// implicit, **issued by the same ACA that signed the response** (1609.2.1: otherwise
/// invalid), `crlSeries` 3 with `cracaId` = [`craca_id`], `id` not `linkageData`, the
/// requested validity and permissions; and the key `d = e·k_U + r` satisfies
/// `d·G = e·P_U + Q_ACA`.
pub fn ee_open_response(
    pki: &ScmsPki,
    spdu: &[u8],
    keys: &RequestKeys,
    kind: AppCertKind,
    period: u32,
    requested_from: u32,
) -> Result<AppCredential> {
    let o = pdu::open_signed(spdu, pki)?;
    let signer_id8 = pdu::cert_id8(&o.signer);
    if signer_id8 != pki.aca.id8() {
        return Err(PduError::Signer("the response is signed by the ACA"));
    }
    let inner = pdu::decrypt_key(&o.payload, &keys.response_private())?;
    let p = pdu::open_unsecured(&inner)?;
    let ScmsPduContent::aca_ee(AcaEeInterfacePdu::acaEeCertResponse(r)) = p.content else {
        return Err(PduError::Shape("expected aca-ee.acaEeCertResponse"));
    };
    let c = r.certificate;
    if c.0.r_type != CertificateType::implicit {
        return Err(PduError::Shape("an implicit certificate"));
    }
    let same_aca = matches!(&c.0.issuer,
        IssuerIdentifier::sha256AndDigest(h) if hashedid::id8_bytes(h) == signer_id8);
    if !same_aca {
        return Err(PduError::Signer(
            "the certificate is issued by the ACA that signed the response",
        ));
    }
    let t = &c.0.to_be_signed;
    if t.crl_series.0.0 != SERIES_APPLICATION_IDENTIFICATION {
        return Err(PduError::Shape("crlSeries 3"));
    }
    if t.craca_id.0[..] != craca_id(pki)?[..] {
        return Err(PduError::Shape("cracaId is the CRACA's HashedId3"));
    }
    if matches!(t.id, CertificateId::linkageData(_)) {
        return Err(PduError::Shape("no LinkageData"));
    }
    let want = request_tbs(kind, &Point::mul_base(&keys.ecqv_private()), requested_from)?;
    if t.validity_period != want.validity_period || t.app_permissions != want.app_permissions {
        return Err(PduError::Shape(
            "validity or permissions differ from the request",
        ));
    }
    let rb = r
        .private_key_info
        .ok_or(PduError::Shape("privateKeyInfo present"))?;
    let rb: [u8; 32] = rb[..]
        .try_into()
        .map_err(|_| PduError::Shape("privateKeyInfo 32 bytes"))?;
    let r_scalar = ec::scalar_from_be32(&rb).map_err(|_| PduError::Key("privateKeyInfo"))?;
    let e = cert::implicit_e(&c, &pki.aca.cert).map_err(|_| PduError::Shape("ECQV e"))?;
    let d = e * keys.ecqv_private() + r_scalar;
    let (_, q) = pki.signer_key(&c)?;
    if Point::mul_base(&d).compressed().map(|x| x.to_vec()) != Some(q) {
        return Err(PduError::Key("d·G is not e·P_U + Q_ACA"));
    }
    let id10 = cert::cert_id10(&c).map_err(|_| PduError::Shape("HashedId10"))?;
    let from = t.validity_period.start.0.0;
    Ok(AppCredential {
        kind,
        period,
        key: Signer1609::p256(&ec::scalar_to_be32(&d)).ok_or(PduError::Key("P-256"))?,
        valid_from: from,
        valid_until: from.saturating_add(u32::from(kind.validity_hours()) * 3600),
        id10,
        cert: c,
    })
}

/// The RSU's or OBU's whole request SPDU: [`ee_ra_cert_request`], signed by the
/// enrolment certificate and encrypted to the RA.
pub fn ee_request_spdu(
    pki: &ScmsPki,
    enrolment: &pdu::EnrolmentCredential,
    kind: AppCertKind,
    generation_time: u32,
    keys: &RequestKeys,
    valid_from: u32,
    g: &mut Drbg,
) -> Result<Vec<u8>> {
    let p = ee_ra_cert_request(kind, generation_time, keys, valid_from)?;
    let ra_key = pki
        .ra
        .enc_public()
        .ok_or(PduError::Key("RA encryption key"))?;
    pdu::signed_encrypted_cert_request(
        &p,
        RequestSigner::Cert(&enrolment.cert, &enrolment.key),
        &pki.ra.cert,
        &ra_key,
        g,
    )
}

/// `ee-ra.raEeCertAck` (03 §3.3): `firstI` present only for a butterfly request (it
/// names the download file, 1609.2.1 cl. 8.2.2); `nextDlTime` = when the file is ready.
pub fn ra_ee_cert_ack(
    generation_time: u32,
    request_hash: [u8; 8],
    first_i: Option<u16>,
    next_dl_time: u32,
) -> ScmsPdu {
    use v2xw_msg::sec_types::ieee1609_dot2_base_types::IValue;
    use v2xw_msg::sec_types::ieee1609_dot2_dot1_ee_ra_interface::RaEeCertAck;
    scms(ScmsPduContent::ee_ra(EeRaInterfacePdu::raEeCertAck(
        RaEeCertAck::new(
            Uint8(2),
            Time32(Uint32(generation_time)),
            hashedid::id8_from_bytes(request_hash),
            first_i.map(|i| IValue(Uint16(i))),
            Time32(Uint32(next_dl_time)),
        ),
    )))
}

/// The end entity's checks on the ack: RA-signed, `requestHash` = the request it sent,
/// `firstI` present exactly for a butterfly request. Returns `nextDlTime`.
pub fn ee_check_ack(pki: &ScmsPki, ack: &[u8], sent: [u8; 8], butterfly: bool) -> Result<u32> {
    let (p, signer) = pdu::open_signed_pdu(ack, pki)?;
    if pdu::cert_id8(&signer) != pki.ra.id8() {
        return Err(PduError::Signer("an ack is signed by the RA"));
    }
    let ScmsPduContent::ee_ra(EeRaInterfacePdu::raEeCertAck(a)) = p.content else {
        return Err(PduError::Shape("expected ee-ra.raEeCertAck"));
    };
    if hashedid::id8_bytes(&a.request_hash) != sent {
        return Err(PduError::Shape(
            "requestHash is not the hash of the request sent",
        ));
    }
    if a.first_i.is_some() != butterfly {
        return Err(PduError::Shape("firstI is present for butterfly only"));
    }
    Ok(a.next_dl_time.0.0)
}

/// The EE's `EeRaDownloadRequestSpdu`: `{generationTime, filename}`, signed by the
/// enrolment certificate and encrypted to the RA.
pub fn ee_download_request_spdu(
    pki: &ScmsPki,
    enrolment: &pdu::EnrolmentCredential,
    generation_time: u32,
    filename: &str,
    g: &mut Drbg,
) -> Result<Vec<u8>> {
    let p = pdu::ee_ra_download_request(generation_time, filename);
    let ra_key = pki
        .ra
        .enc_public()
        .ok_or(PduError::Key("RA encryption key"))?;
    pdu::signed_encrypted(
        &p,
        &enrolment.cert,
        &enrolment.key,
        &pki.ra.cert,
        &ra_key,
        g,
    )
}

/// The RA's checks on a download request: it decrypts, verifies under an enrolment
/// certificate of this PKI, and returns `(filename, signer)`.
pub fn ra_open_download_request(pki: &ScmsPki, spdu: &[u8]) -> Result<(String, Certificate)> {
    let enc = pki
        .ra
        .enc_private
        .ok_or(PduError::Key("RA encryption key"))?;
    let plain = pdu::decrypt_cert(spdu, &pki.ra.cert, &enc)?;
    let (p, signer) = pdu::open_signed_pdu(&plain, pki)?;
    let ScmsPduContent::ee_ra(EeRaInterfacePdu::eeRaDownloadRequest(r)) = p.content else {
        return Err(PduError::Shape("expected ee-ra.eeRaDownloadRequest"));
    };
    Ok((r.filename.to_string(), signer))
}
