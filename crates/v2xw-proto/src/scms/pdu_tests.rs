//! Unit tests of the 1609.2.1 SPDU layer: every wrapper round-trips, and every receiver
//! rule refuses the SPDU that breaks it.

use super::*;
use v2xw_msg::sec_types::ieee1609_dot2::HeaderInfo;
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{Time64, Uint64};

const T: u32 = 694_310_405;

fn pki() -> ScmsPki {
    ScmsPki::new(7, T - 1_000).expect("PKI")
}

fn info() -> ScmsPdu {
    ra_ee_cert_info(T, 3, [0xAB; 8], T + 60)
}

/// Signs `info()` with a header and signer of the caller's choosing, to build the SPDUs a
/// receiver must refuse.
fn signed_with(header: HeaderInfo, sid: sig1609::SpduSigner, a: &Authority) -> Vec<u8> {
    let d = sig1609::sign_spdu(header, coer(&info()).unwrap(), sid, &a.key).unwrap();
    coer(&d).unwrap()
}

#[test]
fn every_component_certificate_chains_to_the_root() {
    let k = pki();
    for a in k.all() {
        assert!(k.signer_key(&a.cert).is_ok(), "{}", a.name);
    }
    let other = ScmsPki::new(8, T - 1_000).unwrap();
    assert_eq!(k.signer_key(&other.ra.cert), Err(PduError::UntrustedSigner));
    assert_eq!(
        k.signer_key(&other.root.cert),
        Err(PduError::UntrustedSigner)
    );
}

#[test]
fn signed_round_trips_and_every_profile_rule_is_enforced() {
    let k = pki();
    let ok = signed(&info(), &k.ra.cert, &k.ra.key).unwrap();
    let (p, signer) = open_signed_pdu(&ok, &k).unwrap();
    assert_eq!(p, info());
    assert_eq!(cert_id8(&signer), k.ra.id8());
    // A flipped signature byte (the last byte ends `sSig`).
    let mut bad = ok.clone();
    *bad.last_mut().unwrap() ^= 1;
    assert_eq!(open_signed(&bad, &k).unwrap_err(), PduError::BadSignature);
    // PSID 36.
    let ra_cert = || sig1609::SpduSigner::Certificate(k.ra.cert.clone());
    let b = signed_with(sig1609::header_psid_only(36), ra_cert(), &k.ra);
    assert_eq!(open_signed(&b, &k).unwrap_err(), PduError::Psid(36));
    // generationTime present.
    let mut h = sig1609::header_psid_only(SECURITY_MGMT_PSID);
    h.generation_time = Some(Time64(Uint64(1)));
    let b = signed_with(h, ra_cert(), &k.ra);
    assert_eq!(
        open_signed(&b, &k).unwrap_err(),
        PduError::HeaderField("generationTime")
    );
    // Signed by another PKI's RA with its own certificate attached.
    let other = ScmsPki::new(8, T - 1_000).unwrap();
    let b = signed(&info(), &other.ra.cert, &other.ra.key).unwrap();
    assert_eq!(open_signed(&b, &k).unwrap_err(), PduError::UntrustedSigner);
    // The RA's certificate with the ACA's key.
    let b = signed(&info(), &k.ra.cert, &k.aca.key).unwrap();
    assert_eq!(open_signed(&b, &k).unwrap_err(), PduError::BadSignature);
    // `self` instead of SignerSingleCert.
    let b = signed_with(
        sig1609::header_psid_only(SECURITY_MGMT_PSID),
        sig1609::SpduSigner::SelfSigned,
        &k.ra,
    );
    assert!(matches!(
        open_signed(&b, &k).unwrap_err(),
        PduError::Signer(_)
    ));
}

#[test]
fn signed_encrypted_opens_only_for_its_recipient_and_detects_tampering() {
    let k = pki();
    let mut g = Drbg::new(1, b"t");
    let ma_key = k.ma.enc_public().unwrap();
    let b = signed_encrypted(&info(), &k.ra.cert, &k.ra.key, &k.ma.cert, &ma_key, &mut g).unwrap();
    let plain = decrypt_cert(&b, &k.ma.cert, &k.ma.enc_private.unwrap()).unwrap();
    assert_eq!(open_signed_pdu(&plain, &k).unwrap().0, info());
    // The RA is not the recipient.
    assert_eq!(
        decrypt_cert(&b, &k.ra.cert, &k.ra.enc_private.unwrap()).unwrap_err(),
        PduError::NotForMe
    );
    // The right recipient id with the wrong private key.
    assert_eq!(
        decrypt_cert(&b, &k.ma.cert, &k.ra.enc_private.unwrap()).unwrap_err(),
        PduError::Decrypt("ECIES")
    );
    // A flipped ciphertext byte.
    let mut t = b.clone();
    *t.last_mut().unwrap() ^= 1;
    assert_eq!(
        decrypt_cert(&t, &k.ma.cert, &k.ma.enc_private.unwrap()).unwrap_err(),
        PduError::Decrypt("CCM")
    );
}

#[test]
fn certificate_requests_self_and_enrolment_signed() {
    let k = pki();
    let mut g = Drbg::new(2, b"t");
    let k_u = g.scalar();
    let req = ee_eca_cert_request(T, &Point::mul_base(&k_u), T, 52_596).unwrap();
    let key = Signer1609::p256(&ec::scalar_to_be32(&k_u)).unwrap();
    let b = signed_cert_request(&req, RequestSigner::SelfSigned(&key)).unwrap();
    let (p, s) = open_signed_cert_request(&b, &k).unwrap();
    assert_eq!(p, req);
    assert!(matches!(s, RequestSignerFound::SelfSigned));
    // Self-signed with a key other than the requested one.
    let wrong = Signer1609::p256(&ec::scalar_to_be32(&g.scalar())).unwrap();
    let b = signed_cert_request(&req, RequestSigner::SelfSigned(&wrong)).unwrap();
    assert_eq!(
        open_signed_cert_request(&b, &k).unwrap_err(),
        PduError::BadSignature
    );
    // ECQV enrolment, then a request signed with the enrolment certificate.
    let ScmsPduContent::eca_ee(EcaEeInterfacePdu::eeEcaCertRequest(r)) = &req.content else {
        panic!("eeEcaCertRequest")
    };
    let (cert, rr) = issue_enrolment(&k, &r.tbs_cert, &Point::mul_base(&k_u), &mut g).unwrap();
    let cred = reconstruct_enrolment(&k, &cert, &k_u, &rr).unwrap();
    assert!(reconstruct_enrolment(&k, &cert, &k_u, &(rr + Scalar::ONE)).is_err());
    let cat = Caterpillar::from_seed(&[5u8; 64]).unwrap();
    let ra_req = ee_ra_cert_request_from(T, &cat, T + 3600).unwrap();
    let ra_key = k.ra.enc_public().unwrap();
    let b = signed_encrypted_cert_request(
        &ra_req,
        RequestSigner::Cert(&cred.cert, &cred.key),
        &k.ra.cert,
        &ra_key,
        &mut g,
    )
    .unwrap();
    let plain = decrypt_cert(&b, &k.ra.cert, &k.ra.enc_private.unwrap()).unwrap();
    let (p, s) = open_signed_cert_request(&plain, &k).unwrap();
    assert_eq!(p, ra_req);
    assert!(matches!(s, RequestSignerFound::Cert(c) if c == cred.cert));
    // The same request signed by a key the ECA did not certify.
    let b = signed_cert_request(&ra_req, RequestSigner::Cert(&cred.cert, &wrong)).unwrap();
    assert_eq!(
        open_signed_cert_request(&b, &k).unwrap_err(),
        PduError::BadSignature
    );
}

#[test]
fn encrypted_signed_and_symmetric_wrappers_round_trip() {
    let k = pki();
    let mut g = Drbg::new(3, b"t");
    let h = g.scalar();
    let b = encrypted_signed(
        &info(),
        &Point::mul_base(&h),
        &k.aca.cert,
        &k.aca.key,
        &mut g,
    )
    .unwrap();
    let o = open_signed(&b, &k).unwrap();
    assert_eq!(cert_id8(&o.signer), k.aca.id8());
    let inner = decrypt_key(&o.payload, &h).unwrap();
    assert_eq!(open_unsecured(&inner).unwrap(), info());
    assert_eq!(
        decrypt_key(&o.payload, &g.scalar()).unwrap_err(),
        PduError::NotForMe
    );
    let shared = [0x42u8; 16];
    let b = symm_encrypt_single(&[1, 2, 3, 4, 5, 6, 7, 8, 9], &shared, &mut g).unwrap();
    assert_eq!(
        decrypt_symm(&b, &shared).unwrap(),
        vec![1, 2, 3, 4, 5, 6, 7, 8, 9]
    );
    assert_eq!(
        decrypt_symm(&b, &[0x43; 16]).unwrap_err(),
        PduError::NotForMe
    );
    let u = unsecured(&info()).unwrap();
    assert_eq!(open_unsecured(&u).unwrap(), info());
}

/// 03 §4.2: `AB09281C9867DE53_F.zip` is i = 15.
#[test]
fn download_file_names_follow_the_profile() {
    let h = [0xAB, 0x09, 0x28, 0x1C, 0x98, 0x67, 0xDE, 0x53];
    assert_eq!(batch_filename(h, 15), "AB09281C9867DE53_F.zip");
    assert_eq!(info_filename(h), "AB09281C9867DE53.info");
}

/// A certificate that names this PKI's ICA as its issuer but was signed by another key
/// does not chain, so nothing it signs is trusted.
#[test]
fn a_certificate_naming_our_issuer_but_signed_by_another_key_is_untrusted() {
    let k = pki();
    let other = ScmsPki::new(8, T - 1_000).unwrap();
    let ica_key = CertKey::P256(Point::mul_base(k.ica.private.as_ref().unwrap()));
    let forged = cert::issue_explicit_cert(
        other.ra.cert.0.to_be_signed.clone(),
        Some((&k.ica.cert, &ica_key)),
        CertHash::Sha256,
        |d| Ok(other.ica.key.sign_digest(d)),
    )
    .unwrap();
    assert_eq!(k.signer_key(&forged), Err(PduError::UntrustedSigner));
    let b = signed(&info(), &forged, &other.ra.key).unwrap();
    assert_eq!(open_signed(&b, &k).unwrap_err(), PduError::UntrustedSigner);
}

/// The RA binds the signed butterfly parameters to the ones it will expand: a request
/// whose signed `ck` differs from the expected one is refused.
#[test]
fn the_ra_refuses_a_request_whose_signed_butterfly_parameters_differ() {
    let k = pki();
    let mut g = Drbg::new(4, b"t");
    let k_u = g.scalar();
    let req = ee_eca_cert_request(T, &Point::mul_base(&k_u), T, 52_596).unwrap();
    let ScmsPduContent::eca_ee(EcaEeInterfacePdu::eeEcaCertRequest(r)) = &req.content else {
        panic!("eeEcaCertRequest")
    };
    let (cert, rr) = issue_enrolment(&k, &r.tbs_cert, &Point::mul_base(&k_u), &mut g).unwrap();
    let cred = reconstruct_enrolment(&k, &cert, &k_u, &rr).unwrap();
    let cat = Caterpillar::from_seed(&[6u8; 64]).unwrap();
    let ra_req = ee_ra_cert_request_from(T, &cat, T + 3600).unwrap();
    let ra_key = k.ra.enc_public().unwrap();
    let b = signed_encrypted_cert_request(
        &ra_req,
        RequestSigner::Cert(&cred.cert, &cred.key),
        &k.ra.cert,
        &ra_key,
        &mut g,
    )
    .unwrap();
    let mut want = ButterflyOriginal {
        a: cat.signing_public(),
        h: cat.encryption_public(),
        ck: *cat.ck(),
        ek: *cat.ek(),
    };
    assert_eq!(
        ra_open_ee_ra_cert_request(&k, &b, &want).unwrap(),
        cred.cert
    );
    want.ck[0] ^= 1;
    assert!(matches!(
        ra_open_ee_ra_cert_request(&k, &b, &want).unwrap_err(),
        PduError::Shape(_)
    ));
}

/// A component certificate checked once is not re-verified for every PDU it signs, and a
/// remembered pass never vouches for a different certificate under the same issuer.
#[test]
fn a_component_certificate_is_verified_once_and_a_forgery_is_still_refused() {
    let k = pki();
    assert!(k.signer_key(&k.aca.cert).is_ok());
    // The RA opens one ACA response per pseudonym certificate: 2,000 here, and the ACA
    // certificate's ECDSA signature is verified for none of them again.
    let before = explicit_cert_checks_run();
    for _ in 0..2_000 {
        assert!(k.signer_key(&k.aca.cert).is_ok());
    }
    let checks = explicit_cert_checks_run() - before;
    assert_eq!(
        checks, 0,
        "2,000 look-ups of the ACA certificate verified it {checks} times"
    );
    // The same body under the same issuer, signed by another key: another hash, checked
    // afresh, refused.
    let other = ScmsPki::new(8, T - 1_000).unwrap();
    let ica_key = CertKey::P256(Point::mul_base(k.ica.private.as_ref().unwrap()));
    let forged = cert::issue_explicit_cert(
        k.aca.cert.0.to_be_signed.clone(),
        Some((&k.ica.cert, &ica_key)),
        CertHash::Sha256,
        |d| Ok(other.ica.key.sign_digest(d)),
    )
    .unwrap();
    let before = explicit_cert_checks_run();
    assert_eq!(k.signer_key(&forged), Err(PduError::UntrustedSigner));
    assert_eq!(
        explicit_cert_checks_run() - before,
        1,
        "the forgery was checked afresh"
    );
    assert!(k.signer_key(&k.aca.cert).is_ok());
}

// ---------------------------------------------------------------------------------------
// WP-35: the information status (A16)
// ---------------------------------------------------------------------------------------

fn status(ra: Option<u32>) -> InfoStatus {
    InfoStatus {
        crl: vec![CrlStatusLine {
            craca_id8: [1, 2, 3, 4, 5, 6, 7, 8],
            series: 1,
            issue_date: 0x0A0B_0C0D,
        }],
        ctl: vec![CtlStatusLine {
            series_id: v2xw_sec::ctl::US_CTL_SERIES_ID,
            sequence: 5,
            last_update: 0x0102_0304,
        }],
        ca_ccf: 0x1122_3344,
        ma: vec![MaStatusLine {
            psids: vec![0x20],
            updated: 0x5566_7788,
        }],
        ra,
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The status encodes byte for byte as canonical COER of the cached ASN.1
/// (`Ieee1609Dot2Dot1CertManagement.asn` l.341-410): the preamble's second bit is `ra`'s
/// presence (the first is the extension bit), each `SEQUENCE OF` carries a one-octet
/// quantity length, each inner `SEQUENCE` its own extension preamble, and a `Psid` its
/// length. The vector is written from the X.696 rules, not from the encoder; the same
/// layout is in the asn1tools corpus (`scms_coer_xcheck.txt` l.7007).
///
/// fault: `ra` encoded as mandatory (absent sent as 0) -> red.
#[test]
fn the_info_status_encodes_as_coer_with_ra_optional() {
    let body = concat!(
        "0101",
        "00",
        "0102030405060708",
        "0001",
        "0a0b0c0d", // crl: one CrlInfoStatus
        "0101",
        "00",
        "0348000000000000",
        "0005",
        "01020304", // ctl: the US series, seq 5
        "11223344", // caCcf
        "0101",
        "00",
        "0101",
        "0120",
        "55667788", // ma: psids {0x20}
    );
    assert_eq!(
        hex(&info_status_coer(&status(Some(0x99AA_BBCC))).unwrap()),
        format!("40{body}99aabbcc"),
        "sent by an RA: ra present"
    );
    assert_eq!(
        hex(&info_status_coer(&status(None)).unwrap()),
        format!("00{body}"),
        "sent by a DC: ra absent"
    );
}

/// The RA signs the status with its certificate attached; a device opens it only from the
/// RA and only with the `ra` date an RA must send (l.337).
#[test]
fn an_info_status_is_signed_by_the_ra_and_opens_only_from_it() {
    let pki = pki();
    let s = status(Some(T));
    let spdu = ra_info_status_spdu(&pki, &s).unwrap();
    assert_eq!(open_info_status(&spdu, &pki).unwrap(), s);
    let by_eca = signed(&cert_info_status(&s), &pki.eca.cert, &pki.eca.key).unwrap();
    assert!(matches!(
        open_info_status(&by_eca, &pki),
        Err(PduError::Signer(_))
    ));
    let no_ra = ra_info_status_spdu(&pki, &status(None)).unwrap();
    assert!(matches!(
        open_info_status(&no_ra, &pki),
        Err(PduError::Shape(_))
    ));
    let mut bad = spdu.clone();
    let n = bad.len();
    bad[n - 1] ^= 1;
    assert!(
        open_info_status(&bad, &pki).is_err(),
        "a changed signature is refused"
    );
}

/// A device downloads only what is strictly newer than what it holds: nothing when the
/// dates match, the composite CRL for a newer CRL (or a CRACA it holds none of), the chain
/// file for a CTL or chain-file change alone, and the MA or RA certificate when its date
/// moved.
///
/// fault: the CRL date comparison `>=` (an equal date refetched) -> red.
#[test]
fn info_status_fetches_only_newer_lists() {
    let s = status(Some(T));
    let held = HeldStatus {
        crl: s.crl.clone(),
        ctl: s.ctl.clone(),
        ca_ccf: s.ca_ccf,
        ma_updated: 0x5566_7788,
        ra: T,
    };
    assert_eq!(
        status_fetch(&s, &held),
        StatusFetch::default(),
        "all current"
    );
    assert!(!status_fetch(&s, &held).any());
    let mut newer = s.clone();
    newer.crl[0].issue_date += 1;
    let f = status_fetch(&newer, &held);
    assert!(f.composite_crl && !f.chain_file && !f.ma_certificate && !f.ra_certificate);
    let mut other_craca = s.clone();
    other_craca.crl.push(CrlStatusLine {
        craca_id8: [9; 8],
        series: 1,
        issue_date: 1,
    });
    assert!(
        status_fetch(&other_craca, &held).composite_crl,
        "a CRACA it holds no list of"
    );
    let mut ctl = s.clone();
    ctl.ctl[0].sequence += 1;
    let f = status_fetch(&ctl, &held);
    assert!(f.chain_file && !f.composite_crl);
    let mut ccf = s.clone();
    ccf.ca_ccf += 1;
    assert!(status_fetch(&ccf, &held).chain_file);
    let mut ma = s.clone();
    ma.ma[0].updated += 1;
    assert_eq!(
        status_fetch(&ma, &held),
        StatusFetch {
            ma_certificate: true,
            ..StatusFetch::default()
        }
    );
    let mut ra = s.clone();
    ra.ra = Some(T + 1);
    assert!(status_fetch(&ra, &held).ra_certificate);
    let mut older = s;
    older.crl[0].issue_date -= 1;
    assert!(
        !status_fetch(&older, &held).any(),
        "an older date is not newer"
    );
}
