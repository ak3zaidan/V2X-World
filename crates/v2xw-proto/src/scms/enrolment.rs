//! WP-B2: enrolment as the US profile specifies it, as real IEEE 1609.2.1 SPDUs.
//!
//! * **The request** ([`ee_eca_cert_request_us`]): `eca-ee.eeEcaCertRequest` for an
//!   implicit certificate whose `toBeSigned` is the US enrolment profile ([SMP] §6.3.1,
//!   03 §5): `id.name` unique, `cracaId 000000`, `crlSeries` 4, `region` the identified
//!   countries USA/Canada/Mexico (840/124/484), no `appPermissions`, no
//!   `certIssuePermissions`, and the wildcard `certRequestPermissions` of [SMP] §4.4 (one
//!   `PsidGroupPermissions` whose `subjectPermissions` is `all`, every other field its
//!   DEFAULT).
//! * **Direct authorisation** ([SMP] §2.1.7, 01 §5): the device's canonical key is made in
//!   the secure environment, the EE operator gives the ECA the canonical public key, and
//!   the request carries `canonicalId` and is signed (`SignerSelf`) with the canonical
//!   private key. The ECA looks the identifier up to verify ([`open_ee_eca_cert_request`]).
//! * **The ECA's checks** ([`check_us_profile`]): the request-validity rules of [SMP]
//!   §6.3.1 plus the maximum request age of one week (01 §4, Profile Table 3); a breach is
//!   a typed [`PduError::Policy`], which the run records.
//! * **Successor enrolment** ([`ee_ra_successor_request`], [`ra_open_successor_request`],
//!   [`check_successor`]): `ee-ra.eeRaSuccessorEnrollmentCertRequest`, an
//!   `EeEcaCertRequestSpdu` self-signed by the new key, wrapped in a
//!   SignedEncryptedCertRequest signed by the current enrolment certificate and encrypted
//!   to the RA ([ASN-2.1], 03 §3.2). The successor keeps the name, region and permissions
//!   and starts exactly when the current certificate ends: no validity overlap
//!   ([SM-REENR] §1–3).
//!
//! **Assumptions, labelled.** The text form of `canonicalId` is not in any free source: the
//! profile says only that the canonical identity is the HashedId32 of the canonical public
//! key. [`canonical_id`] hashes the COER `PublicVerificationKey` with SHA-256 and writes it
//! as 64 upper-case hex digits (UNCONFIRMED). The successor download file name follows the
//! CAMP re-enrolment service, `<requestHash, 16 upper-case hex>.zip` (03 §4.2,
//! [SM-REENR] §4); the 1609.2.1 v3 path table is paywalled (03 §4.1).

use rasn::types::{FixedOctetString, Integer, SequenceOf};
use v2xw_msg::sec_types::EndEntityType;
use v2xw_msg::sec_types::ieee1609_dot2::{
    Certificate, CertificateId, CertificateType, Ieee1609Dot2Content, IssuerIdentifier,
    PsidGroupPermissions, SequenceOfPsidGroupPermissions, SignerIdentifier, SubjectPermissions,
    ToBeSignedCertificate, VerificationKeyIndicator,
};
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    CrlSeries, Duration as Ieee1609Duration, GeographicRegion, HashedId3, Hostname,
    IdentifiedRegion, Opaque, PublicVerificationKey, SequenceOfIdentifiedRegion, Time32, Uint8,
    Uint16, Uint32, UnCountryId, ValidityPeriod,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_eca_ee_interface::{
    EcaEeInterfacePdu, EeEcaCertRequest,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_ee_ra_interface::EeRaInterfacePdu;
use v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::{
    EeEcaCertRequestSpdu, ScmsPdu, ScmsPduContent, SignedCertificateRequest,
};
use v2xw_sec::ec::Point;
use v2xw_sec::hashedid;
use v2xw_sec::sig1609::{self, Hash1609};

use crate::scms::pdu::{
    self, PduError, RequestSignerFound, Result, SCMS_PDU_VERSION, STRUCT_VERSION, ScmsPki,
};

/// The countries a US enrolment certificate may name: USA, Canada, Mexico (UN M.49 840,
/// 124, 484) ([SMP] §6.3.1).
pub const US_ENROLMENT_REGIONS: [u16; 3] = [840, 124, 484];

/// The CRL series of enrolment certificates ([SMP] §6.3.1; 01 §4: in practice the RA's
/// blocklist does the revoking).
pub const ENROLMENT_CRL_SERIES: u16 = 4;

/// The oldest an `EeEcaCertRequest` may be when the ECA receives it: one week (01 §4,
/// [SMP] Table 3, "ECA max request age").
pub const MAX_REQUEST_AGE_S: u32 = 7 * 86_400;

/// How far ahead of the ECA's clock a request's `generationTime` may be: 5 s, the clock
/// tolerance the profile gives the RA's download header (03 §2; [SMP] §6.2). Assumed to
/// apply at the ECA as well.
pub const CLOCK_TOLERANCE_S: u32 = 5;

/// The RA's URL the bootstrap installs with the RA certificate (03 §5): the RA host on
/// CAMP's RA service port 8892 (03 §4.2); the provider label is neutral.
pub const RA_URL: &str = "https://ra.provider-a:8892/";

/// What a device asks the ECA to certify.
#[derive(Debug, Clone, Copy)]
pub struct EnrolmentRequest<'a> {
    /// `id.name`, unique at the ECA.
    pub name: &'a str,
    /// The identified countries (UN M.49).
    pub regions: &'a [u16],
    /// The requested start of validity (Time32).
    pub valid_from: u32,
    /// The requested lifetime in seconds.
    pub lifetime_s: u64,
}

/// A lifetime as a 1609.2 `Duration`: whole hours while they fit a `Uint16` (up to about
/// 7.5 years, which covers CAMP's 6 and 7 years exactly), else whole years.
#[must_use]
pub fn lifetime_duration(lifetime_s: u64) -> Ieee1609Duration {
    match u16::try_from(lifetime_s / 3_600) {
        Ok(h) => Ieee1609Duration::hours(Uint16(h)),
        Err(_) => Ieee1609Duration::years(Uint16(
            u16::try_from(lifetime_s / 31_556_952).unwrap_or(u16::MAX),
        )),
    }
}

/// The wildcard `certRequestPermissions` of an enrolment certificate ([SMP] §4.4,
/// 6.3.4.2): one group, `subjectPermissions all`, `minChainLength`, `chainLengthRange` and
/// `eeType` at their DEFAULTs (1, 0, app).
#[must_use]
pub fn wildcard_request_permissions() -> SequenceOfPsidGroupPermissions {
    let mut app = rasn::types::FixedBitString::<8>::ZERO;
    app.set(0, true);
    SequenceOfPsidGroupPermissions(vec![PsidGroupPermissions::new(
        SubjectPermissions::all(()),
        Integer::from(1),
        Integer::from(0),
        EndEntityType(app),
    )])
}

fn us_region(regions: &[u16]) -> GeographicRegion {
    GeographicRegion::identifiedRegion(SequenceOfIdentifiedRegion(
        regions
            .iter()
            .map(|c| IdentifiedRegion::countryOnly(UnCountryId(Uint16(*c))))
            .collect::<SequenceOf<_>>(),
    ))
}

/// The US enrolment `toBeSigned` a device requests on `R_U` (03 §3.3, [SMP] §6.3.1).
pub fn us_enrolment_tbs(
    req: &EnrolmentRequest<'_>,
    request_public: &Point,
) -> Result<ToBeSignedCertificate> {
    Ok(ToBeSignedCertificate::new(
        CertificateId::name(Hostname(req.name.into())),
        HashedId3(FixedOctetString::from([0u8; 3])),
        CrlSeries(Uint16(ENROLMENT_CRL_SERIES)),
        ValidityPeriod::new(
            Time32(Uint32(req.valid_from)),
            lifetime_duration(req.lifetime_s),
        ),
        Some(us_region(req.regions)),
        None,
        None,
        None,
        Some(wildcard_request_permissions()),
        None,
        None,
        VerificationKeyIndicator::verificationKey(pdu::sig1609_key_of(request_public)?),
        None,
        None,
        None,
        None,
    ))
}

/// `eca-ee.eeEcaCertRequest` for the US enrolment profile; `canonical_id` present when the
/// request will be signed with the canonical key (direct authorisation).
pub fn ee_eca_cert_request_us(
    generation_time: u32,
    request_public: &Point,
    req: &EnrolmentRequest<'_>,
    canonical_id: Option<&str>,
) -> Result<ScmsPdu> {
    let canonical = canonical_id
        .map(|s| rasn::types::Ia5String::try_from(s.to_string()))
        .transpose()
        .map_err(|_| PduError::Shape("canonicalId is IA5"))?;
    Ok(pdu::pdu(ScmsPduContent::eca_ee(
        EcaEeInterfacePdu::eeEcaCertRequest(EeEcaCertRequest::new(
            Uint8(STRUCT_VERSION),
            Time32(Uint32(generation_time)),
            CertificateType::implicit,
            us_enrolment_tbs(req, request_public)?,
            canonical,
        )),
    )))
}

/// The canonical identity of a canonical public key: HashedId32 ([SMP] §2.1.7), here the
/// SHA-256 of the COER `PublicVerificationKey`, as 64 upper-case hex digits (the text form
/// is UNCONFIRMED; see the module note).
pub fn canonical_id(canonical_public: &Point) -> Result<String> {
    let k = pdu::sig1609_key_of(canonical_public)?;
    let h = Hash1609::Sha256.digest(&pdu::coer(&k)?);
    Ok(h.iter().map(|b| format!("{b:02X}")).collect())
}

/// How an enrolment request was authorised, as [`open_ee_eca_cert_request`] found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnrolAuth {
    /// `SignerSelf` under the requested key (no `canonicalId`).
    SelfSigned,
    /// `SignerSelf` under the registered canonical key named by `canonicalId`.
    Canonical(String),
}

/// Opens and verifies an `EeEcaCertRequestSpdu`. With `canonicalId` present the signature
/// is verified under the canonical key `canonical` returns for it (an unknown identifier
/// is a [`PduError::Policy`] refusal); otherwise under the requested `verificationKey`.
pub fn open_ee_eca_cert_request(
    bytes: &[u8],
    canonical: &dyn Fn(&str) -> Option<Point>,
) -> Result<(EeEcaCertRequest, EnrolAuth)> {
    let Ieee1609Dot2Content::signedCertificateRequest(Opaque(b)) = pdu::outer(bytes)?.content
    else {
        return Err(PduError::Shape("expected signedCertificateRequest"));
    };
    let scr: SignedCertificateRequest = pdu::decode(&b)?;
    let p = scr.tbs_request.0.clone();
    if p.version.0 != SCMS_PDU_VERSION {
        return Err(PduError::Version("ScmsPdu.version"));
    }
    let ScmsPduContent::eca_ee(EcaEeInterfacePdu::eeEcaCertRequest(r)) = p.content.clone() else {
        return Err(PduError::Shape("expected eca-ee.eeEcaCertRequest"));
    };
    if !matches!(scr.signer, SignerIdentifier::R_self(())) {
        return Err(PduError::Signer("an enrolment request is SignerSelf"));
    }
    let hash = Hash1609::from_asn1(&scr.hash_algorithm_id).ok_or(PduError::Shape("hashId"))?;
    let (key_hash, key, auth) = match &r.canonical_id {
        Some(id) => {
            let id = id.to_string();
            let q = canonical(&id).ok_or(PduError::Policy(
                "canonicalId is not registered with this ECA",
            ))?;
            let k = pdu::sig1609_key_of(&q)?;
            let (h, k) =
                sig1609::verification_key_sec1(&k).ok_or(PduError::Key("canonical key"))?;
            (h, k, EnrolAuth::Canonical(id))
        }
        None => {
            let VerificationKeyIndicator::verificationKey(k) = &r.tbs_cert.verify_key_indicator
            else {
                return Err(PduError::Signer(
                    "self-signed request without a verificationKey",
                ));
            };
            let (h, k) =
                sig1609::verification_key_sec1(k).ok_or(PduError::Key("verificationKey"))?;
            (h, k, EnrolAuth::SelfSigned)
        }
    };
    let tbs = pdu::coer(&p)?;
    if key_hash != hash
        || !sig1609::verify_digest(
            hash,
            &key,
            &sig1609::digest_1609(hash, &tbs, b""),
            &scr.signature,
        )
    {
        return Err(PduError::BadSignature);
    }
    Ok((r, auth))
}

/// The ECA's checks of an enrolment request against the US profile ([SMP] §6.3.1) at
/// Time32 `now`, given the ECA's own certificate. The checks that need the ECA's records
/// (the key never used before, the name unique, the canonical identity known) are the
/// run's.
pub fn check_us_profile(r: &EeEcaCertRequest, eca: &Certificate, now: u32) -> Result<()> {
    let t = &r.tbs_cert;
    let rule = |ok: bool, what: &'static str| {
        if ok {
            Ok(())
        } else {
            Err(PduError::Policy(what))
        }
    };
    rule(
        r.version.0 == STRUCT_VERSION,
        "EeEcaCertRequest.version is 2",
    )?;
    rule(r.r_type == CertificateType::implicit, "type is implicit")?;
    let g = r.generation_time.0.0;
    rule(
        g <= now.saturating_add(CLOCK_TOLERANCE_S),
        "generationTime is not in the future",
    )?;
    rule(
        now.saturating_sub(g) <= MAX_REQUEST_AGE_S,
        "the request is at most one week old",
    )?;
    rule(
        matches!(&t.id, CertificateId::name(Hostname(n)) if !n.is_empty()),
        "id.name is present",
    )?;
    rule(t.craca_id.0.as_slice() == [0u8; 3], "cracaId is 000000")?;
    rule(t.crl_series.0.0 == ENROLMENT_CRL_SERIES, "crlSeries is 4")?;
    let region_ok = match &t.region {
        Some(GeographicRegion::identifiedRegion(v)) => {
            !v.0.is_empty()
                && v.0.iter().all(|r| {
                    matches!(r, IdentifiedRegion::countryOnly(UnCountryId(Uint16(c)))
                        if US_ENROLMENT_REGIONS.contains(c))
                })
        }
        _ => false,
    };
    rule(region_ok, "region is USA, Canada and/or Mexico only")?;
    rule(t.app_permissions.is_none(), "appPermissions is absent")?;
    rule(
        t.cert_issue_permissions.is_none(),
        "certIssuePermissions is absent",
    )?;
    rule(
        t.cert_request_permissions.as_ref() == Some(&wildcard_request_permissions()),
        "certRequestPermissions is the one wildcard group",
    )?;
    rule(
        matches!(
            &t.verify_key_indicator,
            VerificationKeyIndicator::verificationKey(PublicVerificationKey::ecdsaNistP256(_))
        ),
        "verifyKeyIndicator is a P-256 verificationKey",
    )?;
    let (s, e) = request_validity(t);
    let (es, ee) = v2xw_sec::ctl::validity(eca);
    rule(
        s >= es && e <= ee,
        "the requested validity lies inside the ECA's",
    )?;
    Ok(())
}

/// `[start, end)` of a `toBeSigned`'s validity, in Time32 seconds.
#[must_use]
pub fn request_validity(t: &ToBeSignedCertificate) -> (u64, u64) {
    let s = u64::from(t.validity_period.start.0.0);
    (
        s,
        s + v2xw_sec::ctl::duration_s(&t.validity_period.duration),
    )
}

/// The requested key `R_U` of an enrolment request.
pub fn request_key(r: &EeEcaCertRequest) -> Result<Point> {
    let VerificationKeyIndicator::verificationKey(PublicVerificationKey::ecdsaNistP256(pt)) =
        &r.tbs_cert.verify_key_indicator
    else {
        return Err(PduError::Shape(
            "request key is not a P-256 verificationKey",
        ));
    };
    pdu::point_of(pt)
}

/// `id.name` of a `toBeSigned`.
#[must_use]
pub fn tbs_name(t: &ToBeSignedCertificate) -> Option<&str> {
    match &t.id {
        CertificateId::name(Hostname(n)) => Some(n.as_str()),
        _ => None,
    }
}

/// The ECA's successor rules ([SM-REENR] §1–3): the successor keeps the current
/// certificate's `id.name`, region and permissions, and its validity starts exactly when
/// the current certificate's ends — no overlap and no gap. The inner request is
/// self-signed by the new key, never canonical.
pub fn check_successor(new: &EeEcaCertRequest, current: &Certificate) -> Result<()> {
    let (n, c) = (&new.tbs_cert, &current.0.to_be_signed);
    let rule = |ok: bool, what: &'static str| {
        if ok {
            Ok(())
        } else {
            Err(PduError::Policy(what))
        }
    };
    rule(
        new.canonical_id.is_none(),
        "a successor request is self-signed by the new key",
    )?;
    rule(
        tbs_name(n).is_some() && tbs_name(n) == tbs_name(c),
        "the successor keeps id.name",
    )?;
    rule(n.region == c.region, "the successor keeps the region")?;
    rule(
        n.cert_request_permissions == c.cert_request_permissions,
        "the successor keeps the permissions",
    )?;
    let (ns, _) = request_validity(n);
    let (_, ce) = v2xw_sec::ctl::validity(current);
    rule(
        ns == ce,
        "the successor starts when the current certificate ends (no overlap)",
    )?;
    let VerificationKeyIndicator::reconstructionValue(cur) = &c.verify_key_indicator else {
        return Err(PduError::Policy(
            "the current enrolment certificate is implicit",
        ));
    };
    let fresh = match &n.verify_key_indicator {
        VerificationKeyIndicator::verificationKey(PublicVerificationKey::ecdsaNistP256(p)) => {
            pdu::point_of(p)? != pdu::point_of(cur)?
        }
        _ => false,
    };
    rule(fresh, "the successor is on a new key")?;
    Ok(())
}

/// `ee-ra.eeRaSuccessorEnrollmentCertRequest` carrying `inner`, an `EeEcaCertRequestSpdu`
/// as encoded (03 §3.2).
pub fn ee_ra_successor_request(inner: &[u8]) -> Result<ScmsPdu> {
    let spdu: EeEcaCertRequestSpdu = pdu::decode(inner)?;
    Ok(pdu::pdu(ScmsPduContent::ee_ra(
        EeRaInterfacePdu::eeRaSuccessorEnrollmentCertRequest(spdu),
    )))
}

/// The RA's side of a successor request: decrypt with the RA certificate's encryption
/// key, verify the outer `SignerSingleCert` signature under an enrolment certificate this
/// PKI's ECA issued, and return that certificate and the inner `EeEcaCertRequestSpdu`
/// bytes, which the RA forwards to the ECA.
pub fn ra_open_successor_request(pki: &ScmsPki, spdu: &[u8]) -> Result<(Certificate, Vec<u8>)> {
    let enc = pki
        .ra
        .enc_private
        .ok_or(PduError::Key("RA encryption key"))?;
    let plain = pdu::decrypt_cert(spdu, &pki.ra.cert, &enc)?;
    let (p, signer) = pdu::open_signed_cert_request(&plain, pki)?;
    let RequestSignerFound::Cert(current) = signer else {
        return Err(PduError::Signer(
            "a successor request is signed by the current enrolment certificate",
        ));
    };
    let issued_by_eca = matches!(&current.0.issuer,
        IssuerIdentifier::sha256AndDigest(h) if hashedid::id8_bytes(h) == pki.eca.id8());
    if !issued_by_eca {
        return Err(PduError::UntrustedSigner);
    }
    let ScmsPduContent::ee_ra(EeRaInterfacePdu::eeRaSuccessorEnrollmentCertRequest(inner)) =
        p.content
    else {
        return Err(PduError::Shape(
            "expected ee-ra.eeRaSuccessorEnrollmentCertRequest",
        ));
    };
    Ok((current, pdu::coer(&inner)?))
}

/// The successor download file name: `<requestHash, 16 upper-case hex>.zip` (CAMP's
/// `/ee-re-enrollment-download`, 03 §4.2; see the module note).
#[must_use]
pub fn successor_filename(request_hash: [u8; 8]) -> String {
    let mut s: String = request_hash.iter().map(|b| format!("{b:02X}")).collect();
    s.push_str(".zip");
    s
}

#[cfg(test)]
#[path = "enrolment_tests.rs"]
mod tests;
