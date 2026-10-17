//! Unit tests of the US enrolment profile, direct authorisation and successor enrolment
//! (WP-B2): the good request passes, and every rule refuses the request that breaks it.

use super::*;
use crate::scms::pdu::{Drbg, RequestSigner};
use v2xw_sec::sig1609::Signer1609;
use p256::Scalar;
use v2xw_sec::ec;

const T: u32 = 694_310_405;
const SIX_YEARS: u64 = 6 * 365 * 86_400;

fn pki() -> ScmsPki {
    ScmsPki::new(7, T - 1_000).expect("PKI")
}

fn key(s: &Scalar) -> Signer1609 {
    Signer1609::p256(&ec::scalar_to_be32(s)).unwrap()
}

fn good(valid_from: u32) -> EnrolmentRequest<'static> {
    EnrolmentRequest {
        name: "obu-0001.ee",
        regions: &US_ENROLMENT_REGIONS,
        valid_from,
        lifetime_s: SIX_YEARS,
    }
}

fn inner(p: &ScmsPdu) -> EeEcaCertRequest {
    let ScmsPduContent::eca_ee(EcaEeInterfacePdu::eeEcaCertRequest(r)) = &p.content else {
        panic!("eeEcaCertRequest")
    };
    r.clone()
}

#[test]
fn six_years_is_whole_hours() {
    assert_eq!(
        lifetime_duration(SIX_YEARS),
        Ieee1609Duration::hours(Uint16(52_560))
    );
    assert_eq!(
        lifetime_duration(10 * 31_556_952),
        Ieee1609Duration::years(Uint16(10))
    );
}

#[test]
fn a_canonical_request_verifies_only_under_the_registered_canonical_key() {
    let mut g = Drbg::new(1, b"t");
    let (c, k_u) = (g.scalar(), g.scalar());
    let cpub = Point::mul_base(&c);
    let id = canonical_id(&cpub).unwrap();
    assert_eq!(id.len(), 64);
    let p = ee_eca_cert_request_us(T, &Point::mul_base(&k_u), &good(T), Some(&id)).unwrap();
    let b = pdu::signed_cert_request(&p, RequestSigner::SelfSigned(&key(&c))).unwrap();
    let known = |s: &str| (s == id).then_some(cpub);
    let (r, auth) = open_ee_eca_cert_request(&b, &known).unwrap();
    assert_eq!(auth, EnrolAuth::Canonical(id.clone()));
    assert_eq!(r, inner(&p));
    // An identifier the operator never registered.
    assert_eq!(
        open_ee_eca_cert_request(&b, &|_| None).unwrap_err(),
        PduError::Policy("canonicalId is not registered with this ECA")
    );
    // Registered, but the request was signed with the new key instead of the canonical one.
    let b2 = pdu::signed_cert_request(&p, RequestSigner::SelfSigned(&key(&k_u))).unwrap();
    assert_eq!(
        open_ee_eca_cert_request(&b2, &known).unwrap_err(),
        PduError::BadSignature
    );
    // The generic B1 opener cannot verify a canonical request (it uses the requested key).
    assert!(pdu::open_signed_cert_request(&b, &pki()).is_err());
    // Without canonicalId the request is self-signed by the new key.
    let p3 = ee_eca_cert_request_us(T, &Point::mul_base(&k_u), &good(T), None).unwrap();
    let b3 = pdu::signed_cert_request(&p3, RequestSigner::SelfSigned(&key(&k_u))).unwrap();
    assert_eq!(
        open_ee_eca_cert_request(&b3, &|_| None).unwrap().1,
        EnrolAuth::SelfSigned
    );
}

#[test]
fn the_eca_refuses_every_request_that_breaks_the_us_profile() {
    let k = pki();
    let mut g = Drbg::new(2, b"t");
    let r_u = Point::mul_base(&g.scalar());
    let ok = inner(&ee_eca_cert_request_us(T, &r_u, &good(T), None).unwrap());
    check_us_profile(&ok, &k.eca.cert, T).expect("the US profile request passes");
    let refused = |f: &dyn Fn(&mut EeEcaCertRequest), now: u32| {
        let mut r = ok.clone();
        f(&mut r);
        match check_us_profile(&r, &k.eca.cert, now) {
            Err(PduError::Policy(w)) => w,
            other => panic!("expected a policy refusal, got {other:?}"),
        }
    };
    assert_eq!(
        refused(&|r| r.r_type = CertificateType::explicit, T),
        "type is implicit"
    );
    assert_eq!(
        refused(&|r| r.tbs_cert.region = Some(us_region(&[840, 250])), T),
        "region is USA, Canada and/or Mexico only"
    );
    assert_eq!(
        refused(&|r| r.tbs_cert.region = None, T),
        "region is USA, Canada and/or Mexico only"
    );
    assert_eq!(
        refused(&|r| r.tbs_cert.crl_series = CrlSeries(Uint16(0)), T),
        "crlSeries is 4"
    );
    assert_eq!(
        refused(&|r| r.tbs_cert.id = CertificateId::none(()), T),
        "id.name is present"
    );
    assert_eq!(
        refused(
            &|r| r.tbs_cert.craca_id = HashedId3(FixedOctetString::from([1u8, 0, 0])),
            T
        ),
        "cracaId is 000000"
    );
    assert_eq!(
        refused(
            &|r| r.tbs_cert.cert_issue_permissions = Some(wildcard_request_permissions()),
            T
        ),
        "certIssuePermissions is absent"
    );
    assert_eq!(
        refused(&|r| r.tbs_cert.cert_request_permissions = None, T),
        "certRequestPermissions is the one wildcard group"
    );
    assert_eq!(
        refused(
            &|r| {
                r.tbs_cert.cert_request_permissions =
                    Some(SequenceOfPsidGroupPermissions(vec![cert_issue_all()]));
            },
            T
        ),
        "certRequestPermissions is the one wildcard group"
    );
    assert_eq!(
        refused(&|_| (), T + MAX_REQUEST_AGE_S + 1),
        "the request is at most one week old"
    );
    assert_eq!(
        refused(&|_| (), T - CLOCK_TOLERANCE_S - 1),
        "generationTime is not in the future"
    );
    // 20 years from now: past the ECA's 15-year certificate.
    assert_eq!(
        refused(
            &|r| {
                r.tbs_cert.validity_period.duration = Ieee1609Duration::years(Uint16(20));
            },
            T
        ),
        "the requested validity lies inside the ECA's"
    );
    assert_eq!(
        refused(&|r| r.version = Uint8(1), T),
        "EeEcaCertRequest.version is 2"
    );
}

fn cert_issue_all() -> PsidGroupPermissions {
    v2xw_sec::cert::issue_all_group()
}

#[test]
fn the_issued_enrolment_certificate_carries_the_us_profile() {
    let k = pki();
    let mut g = Drbg::new(3, b"t");
    let k_u = g.scalar();
    let r = inner(&ee_eca_cert_request_us(T, &Point::mul_base(&k_u), &good(T), None).unwrap());
    let (cert, rr) = pdu::issue_enrolment(&k, &r.tbs_cert, &Point::mul_base(&k_u), &mut g).unwrap();
    let t = &cert.0.to_be_signed;
    assert_eq!(cert.0.r_type, CertificateType::implicit);
    assert_eq!(tbs_name(t), Some("obu-0001.ee"));
    assert_eq!(t.crl_series.0.0, 4);
    assert_eq!(t.region, Some(us_region(&[840, 124, 484])));
    assert_eq!(t.cert_request_permissions, Some(wildcard_request_permissions()));
    assert!(t.app_permissions.is_none());
    assert_eq!(v2xw_sec::ctl::validity(&cert), (u64::from(T), u64::from(T) + SIX_YEARS));
    pdu::reconstruct_enrolment(&k, &cert, &k_u, &rr).expect("d·G = e·P_U + Q_ECA");
}

/// A device enrolled at `T` and its successor request at `start` on a fresh key.
fn enrolled_and_successor(
    start: u32,
    name: &'static str,
) -> (ScmsPki, Certificate, Vec<u8>, EeEcaCertRequest) {
    let k = pki();
    let mut g = Drbg::new(4, b"t");
    let k_u = g.scalar();
    let r = inner(&ee_eca_cert_request_us(T, &Point::mul_base(&k_u), &good(T), None).unwrap());
    let (cert, rr) = pdu::issue_enrolment(&k, &r.tbs_cert, &Point::mul_base(&k_u), &mut g).unwrap();
    let cred = pdu::reconstruct_enrolment(&k, &cert, &k_u, &rr).unwrap();
    let k_new = g.scalar();
    let req = EnrolmentRequest {
        name,
        valid_from: start,
        ..good(start)
    };
    let p = ee_eca_cert_request_us(T + 10, &Point::mul_base(&k_new), &req, None).unwrap();
    let inner_bytes = pdu::signed_cert_request(&p, RequestSigner::SelfSigned(&key(&k_new))).unwrap();
    let outer = ee_ra_successor_request(&inner_bytes).unwrap();
    let ra_key = k.ra.enc_public().unwrap();
    let spdu = pdu::signed_encrypted_cert_request(
        &outer,
        RequestSigner::Cert(&cred.cert, &cred.key),
        &k.ra.cert,
        &ra_key,
        &mut g,
    )
    .unwrap();
    (k, cert, spdu, inner(&p))
}

#[test]
fn a_successor_request_opens_at_the_ra_and_must_start_when_the_current_one_ends() {
    let end = T + u32::try_from(SIX_YEARS).unwrap();
    let (k, cert, spdu, new) = enrolled_and_successor(end, "obu-0001.ee");
    let (current, inner_bytes) = ra_open_successor_request(&k, &spdu).expect("the RA opens it");
    assert_eq!(current, cert);
    let (r, auth) = open_ee_eca_cert_request(&inner_bytes, &|_| None).unwrap();
    assert_eq!((r.clone(), auth), (new, EnrolAuth::SelfSigned));
    check_successor(&r, &current).expect("no overlap, no gap");
    check_us_profile(&r, &k.eca.cert, T + 10).expect("the successor meets the profile");

    for (start, why) in [
        (end - 1, "the successor starts when the current certificate ends (no overlap)"),
        (end + 1, "the successor starts when the current certificate ends (no overlap)"),
    ] {
        let (_, cert, _, new) = enrolled_and_successor(start, "obu-0001.ee");
        assert_eq!(check_successor(&new, &cert), Err(PduError::Policy(why)));
    }
    let (_, cert, _, new) = enrolled_and_successor(end, "obu-9999.ee");
    assert_eq!(
        check_successor(&new, &cert),
        Err(PduError::Policy("the successor keeps id.name"))
    );
    // Another RA cannot open it.
    let other = ScmsPki::new(8, T - 1_000).unwrap();
    assert!(ra_open_successor_request(&other, &spdu).is_err());
}

#[test]
fn the_successor_file_name_is_the_request_hash() {
    assert_eq!(
        successor_filename([0xAB, 0x09, 0x28, 0x1C, 0x98, 0x67, 0xDE, 0x53]),
        "AB09281C9867DE53.zip"
    );
}
