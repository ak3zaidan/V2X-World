use super::*;
use crate::sec_types::ieee1609_dot2_base_types::{
    EccP256CurvePointUncompressedP256, EcdsaP256Signature, HashAlgorithm,
};
use rasn::types::OctetString;

// ---------------------------------------------------------------------------------------
// The edition difference, pinned against the committed modules
// ---------------------------------------------------------------------------------------

/// Strips ASN.1 comments (`/* */` and `-- … --`/end of line) and whitespace, keeping one
/// normalised code line per source line. Non-ASCII bytes (CP1252, comments only) become
/// `?`.
fn code_lines(path: &std::path::Path) -> Vec<String> {
    let raw = std::fs::read(path).expect("module file");
    let text: String = raw
        .iter()
        .map(|&b| if b.is_ascii() { b as char } else { '?' })
        .collect();
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut prev = ' ';
            for d in chars.by_ref() {
                if d == '\n' {
                    out.push('\n');
                }
                if prev == '*' && d == '/' {
                    break;
                }
                prev = d;
            }
        } else if c == '-' && chars.peek() == Some(&'-') {
            chars.next();
            let mut prev = ' ';
            for d in chars.by_ref() {
                if d == '\n' {
                    out.push('\n');
                    break;
                }
                if prev == '-' && d == '-' {
                    break;
                }
                prev = d;
            }
        } else {
            out.push(c);
        }
    }
    out.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect()
}

/// Lines only in `a`, and lines only in `b`, counted with multiplicity.
fn line_diff(a: &[String], b: &[String]) -> (Vec<String>, Vec<String>) {
    let mut only_b = b.to_vec();
    let mut only_a = Vec::new();
    for l in a {
        if let Some(i) = only_b.iter().position(|x| x == l) {
            only_b.remove(i);
        } else {
            only_a.push(l.clone());
        }
    }
    only_a.sort();
    only_b.sort();
    (only_a, only_b)
}

/// The three code differences [`WireEdition`] handles are *all* the code differences
/// between the committed 2022 and 2025 editions of the four 1609.2 modules (02 §1). If
/// someone updates a module, this fails until the edition rules are reviewed.
#[test]
fn the_2022_and_2025_modules_differ_exactly_where_the_edition_rules_say() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/asn1/etsi");
    let diff = |m: &str| {
        let a = code_lines(&root.join(format!("ieee1609.2-2022/{m}.asn")));
        let b = code_lines(&root.join(format!("ieee1609.2/{m}.asn")));
        line_diff(&a, &b)
    };
    let s = |v: &[&str]| {
        let mut out = v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        out.sort();
        out
    };

    let (old, new) = diff("Ieee1609Dot2");
    assert_eq!(
        old,
        s(&[
            "dot2(2) base(1) schema(1) major-version-2(2) minor-version-6(6)}",
            "extDataHash PRESENT",
            "appExtensions SequenceOfAppExtensions,",
            "certIssueExtensions SequenceOfCertIssueExtensions,",
            "certRequestExtension SequenceOfCertRequestExtensions",
        ])
    );
    assert_eq!(
        new,
        s(&[
            "dot2(2) base(1) schema(1) major-version-2(2) minor-version-7(7)}",
            "extDataHash PRESENT,",
            "omitted ABSENT",
            "appExtensions SequenceOfAppExtensions OPTIONAL,",
            "certIssueExtensions SequenceOfCertIssueExtensions OPTIONAL,",
            "certRequestExtension SequenceOfCertRequestExtensions OPTIONAL",
        ])
    );

    let (old, new) = diff("Ieee1609Dot2BaseTypes");
    assert_eq!(
        old,
        s(&["base(1) base-types(2) major-version-2(2) minor-version-4(4)}"])
    );
    assert_eq!(
        new,
        s(&["base(1) base-types(2) major-version-2(2) minor-version-5(5)}"])
    );

    assert_eq!(diff("Ieee1609Dot2Crl"), (vec![], vec![]));

    let (old, new) = diff("Ieee1609Dot2CrlBaseTypes");
    assert_eq!(
        old,
        s(&[
            "crl(3) base-types(2) major-version-3(3) minor-version-2(2)}",
            "groupsSingleSeed SequenceOfGroupSingleSeedCrlEntry OPTIONAL",
        ])
    );
    assert_eq!(
        new,
        s(&[
            "crl(3) base-types(2) major-version-3(3) minor-version-3(3)}",
            "groupsSingleSeed SequenceOfGroupSingleSeedCrlEntry OPTIONAL,",
            "iPeriodInfo SequenceOfIPeriodInfo OPTIONAL",
            "IPeriodInfo ::= SEQUENCE {",
            "startOfNextIPeriod Time32,",
            "iPeriodLength Duration,",
            "guaranteedToIValue IValue",
            "}",
            "SequenceOfIPeriodInfo ::= SEQUENCE OF IPeriodInfo",
        ])
    );
}

// ---------------------------------------------------------------------------------------
// Fixtures from the asn1tools corpus (decoded, then edited)
// ---------------------------------------------------------------------------------------

const CORPUS: &str = include_str!("../../tests/fixtures/scms_coer_xcheck.txt");

fn first<T: rasn::Decode>(name: &str, pick: impl Fn(&T) -> bool) -> T {
    CORPUS
        .lines()
        .filter(|l| l.starts_with(&format!("{name} ")) && !l.ends_with(" NESTED"))
        .map(|l| {
            let hex = l.split(' ').nth(1).expect("hex");
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex"))
                .collect();
            rasn::coer::decode::<T>(&bytes).expect("corpus value decodes")
        })
        .find(|v| pick(v))
        .expect("a corpus value matching the filter")
}

fn coer<T: rasn::Encode>(v: &T) -> Vec<u8> {
    rasn::coer::encode(v).expect("encodes")
}

// ---------------------------------------------------------------------------------------
// Edition checks
// ---------------------------------------------------------------------------------------

#[test]
fn a_2022_certificate_that_uses_its_extension_must_carry_all_three_fields() {
    let mut tbs: ToBeSignedCertificate = first("ToBeSignedCertificate", |_| true);
    tbs.flags = None;
    tbs.app_extensions = None;
    tbs.cert_issue_extensions = None;
    tbs.cert_request_extension = None;
    // No extension at all: valid in both, and conforming changes nothing.
    assert_eq!(check_tbs_certificate(&tbs, WireEdition::Ieee2022), Ok(()));
    let before = coer(&tbs);
    conform_tbs_certificate(&mut tbs, WireEdition::Ieee2022);
    assert_eq!(
        coer(&tbs),
        before,
        "no extension -> identical bytes in both editions"
    );

    // `flags` alone (usesCubk): fine in 2025, not in 2022.
    let mut bits = rasn::types::BitString::repeat(false, 8);
    bits.set(0, true);
    tbs.flags = Some(bits);
    assert_eq!(check_tbs_certificate(&tbs, WireEdition::Ieee2025), Ok(()));
    assert_eq!(
        check_tbs_certificate(&tbs, WireEdition::Ieee2022),
        Err(ScmsTypeError::MissingMandatoryExtension("appExtensions"))
    );
    let mut conformed = tbs.clone();
    conform_tbs_certificate(&mut conformed, WireEdition::Ieee2022);
    assert_eq!(
        check_tbs_certificate(&conformed, WireEdition::Ieee2022),
        Ok(())
    );
    // Three empty SEQUENCE OF: one quantity octet (01) and a zero count (00) each, inside
    // a one-byte open type: 3 x (len 02, 01 00) = 9 bytes, plus 1 bitmap byte, as the
    // extension bitmap grows from flags-only to all four.
    assert_eq!(coer(&conformed).len(), coer(&tbs).len() + 9);
    // 2025 leaves it alone.
    let mut untouched = tbs.clone();
    conform_tbs_certificate(&mut untouched, WireEdition::Ieee2025);
    assert_eq!(untouched, tbs);
}

#[test]
fn ip_period_info_is_a_2025_field() {
    let mut crl: CrlContents = first("CrlContents", |c: &CrlContents| {
        matches!(c.type_specific, TypeSpecificCrlContents::fullLinkedCrl(_))
    });
    if let TypeSpecificCrlContents::fullLinkedCrl(l) = &mut crl.type_specific {
        l.i_period_info = None;
    }
    assert_eq!(check_crl_contents(&crl, WireEdition::Ieee2022), Ok(()));
    if let TypeSpecificCrlContents::fullLinkedCrl(l) = &mut crl.type_specific {
        l.i_period_info =
            Some(crate::sec_types::ieee1609_dot2_crl_base_types::SequenceOfIPeriodInfo(Vec::new()));
    }
    assert_eq!(check_crl_contents(&crl, WireEdition::Ieee2025), Ok(()));
    assert_eq!(
        check_crl_contents(&crl, WireEdition::Ieee2022),
        Err(ScmsTypeError::IPeriodInfoIn2022)
    );
}

// ---------------------------------------------------------------------------------------
// Canonicalisation
// ---------------------------------------------------------------------------------------

fn h(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// The P-256 base point G (FIPS 186-4 D.1.2.3; SEC 2 §2.4.2). Its y is odd.
const GX: &str = "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296";
const GY: &str = "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5";
/// The P-256 field prime p.
const P: &str = "ffffffff00000001000000000000000000000000ffffffffffffffffffffffff";

/// p - y, big-endian: the y of -G, which is even because p and G.y are both odd.
fn neg_y(y: &[u8]) -> Vec<u8> {
    let p = h(P);
    let mut out = vec![0u8; 32];
    let mut borrow = 0i16;
    for i in (0..32).rev() {
        let mut d = i16::from(p[i]) - i16::from(y[i]) - borrow;
        borrow = i16::from(d < 0);
        if d < 0 {
            d += 256;
        }
        out[i] = d as u8;
    }
    out
}

fn uncompressed(x: &[u8], y: &[u8]) -> EccP256CurvePoint {
    EccP256CurvePoint::uncompressedP256(EccP256CurvePointUncompressedP256::new(
        OctetString::from_slice(x),
        OctetString::from_slice(y),
    ))
}

/// SEC 1 §2.3.3: the compressed form is 02||x for even y, 03||x for odd y. G compresses to
/// 03 6b17d1f2…, -G to 02 6b17d1f2….
#[test]
fn compression_keeps_x_and_the_parity_of_y() {
    let (gx, gy) = (h(GX), h(GY));
    assert_eq!(
        compressed_p256(&uncompressed(&gx, &gy)),
        Ok(EccP256CurvePoint::compressed_y_1(OctetString::from_slice(
            &gx
        )))
    );
    let ny = neg_y(&gy);
    assert_eq!(ny.last().map(|b| b & 1), Some(0), "-G has even y");
    assert_eq!(
        compressed_p256(&uncompressed(&gx, &ny)),
        Ok(EccP256CurvePoint::compressed_y_0(OctetString::from_slice(
            &gx
        )))
    );
    // On the wire (COER CHOICE tag [2] / [3], then the 32 x octets) that is SEC 1's
    // 02||x / 03||x with the tag octet in place of the SEC 1 prefix.
    let enc = coer(&compressed_p256(&uncompressed(&gx, &gy)).unwrap());
    assert_eq!(enc[0], 0x83);
    assert_eq!(&enc[1..], &gx[..]);
    assert!(compressed_p256(&EccP256CurvePoint::x_only(OctetString::from_slice(&gx))).is_err());
}

/// A certificate with an uncompressed key and a full-point rSig canonicalises to exactly
/// the bytes of the same certificate written canonically — so it hashes to the same
/// HashedId8 — and the non-canonical bytes differ.
#[test]
fn a_certificate_canonicalises_to_compressed_key_and_x_only_r() {
    let (gx, gy) = (h(GX), h(GY));
    let s = OctetString::from_slice(&[0x5a; 32]);
    let mut cert: Certificate = first("Certificate", |c: &Certificate| {
        c.0.to_be_signed.encryption_key.is_none()
    });
    cert.0.to_be_signed.verify_key_indicator = VerificationKeyIndicator::verificationKey(
        PublicVerificationKey::ecdsaNistP256(uncompressed(&gx, &gy)),
    );
    cert.0.signature = Some(Signature::ecdsaNistP256Signature(EcdsaP256Signature::new(
        uncompressed(&gx, &gy),
        s.clone(),
    )));

    let mut canonical = cert.clone();
    canonical.0.to_be_signed.verify_key_indicator =
        VerificationKeyIndicator::verificationKey(PublicVerificationKey::ecdsaNistP256(
            EccP256CurvePoint::compressed_y_1(OctetString::from_slice(&gx)),
        ));
    canonical.0.signature = Some(Signature::ecdsaNistP256Signature(EcdsaP256Signature::new(
        EccP256CurvePoint::x_only(OctetString::from_slice(&gx)),
        s,
    )));

    assert_ne!(coer(&cert), coer(&canonical));
    canonicalize_certificate(&mut cert).expect("canonicalises");
    assert_eq!(coer(&cert), coer(&canonical));
    // Idempotent.
    canonicalize_certificate(&mut cert).expect("canonicalises");
    assert_eq!(coer(&cert), coer(&canonical));
}

/// HeaderInfo: only `encryptionKey` is canonicalised; fields after the extension marker
/// (`requestedCertificate`) are left as received.
#[test]
fn header_info_canonicalisation_stops_at_the_extension_marker() {
    let (gx, gy) = (h(GX), h(GY));
    let mut header: HeaderInfo = first("HeaderInfo", |_| true);
    header.encryption_key = Some(EncryptionKey::public(PublicEncryptionKey::new(
        crate::sec_types::ieee1609_dot2_base_types::SymmAlgorithm::aes128Ccm,
        BasePublicEncryptionKey::eciesNistP256(uncompressed(&gx, &gy)),
    )));
    let mut cert: Certificate = first("Certificate", |_| true);
    cert.0.to_be_signed.verify_key_indicator =
        VerificationKeyIndicator::reconstructionValue(uncompressed(&gx, &gy));
    header.requested_certificate = Some(cert.clone());

    canonicalize_header_info(&mut header).expect("canonicalises");
    let Some(EncryptionKey::public(k)) = &header.encryption_key else {
        panic!("kept")
    };
    assert_eq!(
        k.public_key,
        BasePublicEncryptionKey::eciesNistP256(EccP256CurvePoint::compressed_y_1(
            OctetString::from_slice(&gx)
        ))
    );
    assert_eq!(
        header.requested_certificate,
        Some(cert),
        "requestedCertificate untouched"
    );
}

/// A signed SPDU canonicalises through its payload: the inner signed data's signature
/// becomes x-only too, and the hash algorithm is left alone.
#[test]
fn signed_data_canonicalisation_recurses_into_the_payload() {
    let (gx, gy) = (h(GX), h(GY));
    let sig = || {
        Signature::ecdsaNistP256Signature(EcdsaP256Signature::new(
            uncompressed(&gx, &gy),
            OctetString::from_slice(&[1; 32]),
        ))
    };
    let mut inner: SignedData = first("SignedData", |s: &SignedData| {
        s.hash_id == HashAlgorithm::sha256
    });
    inner.signature = sig();
    inner.tbs_data.payload.data = None;
    let mut outer = inner.clone();
    outer.tbs_data.payload.data = Some(Ieee1609Dot2Data::new(
        crate::sec_types::Uint8(3),
        Ieee1609Dot2Content::signedData(inner),
    ));
    canonicalize_signed_data(&mut outer).expect("canonicalises");
    let x_only = EccP256CurvePoint::x_only(OctetString::from_slice(&gx));
    let Signature::ecdsaNistP256Signature(s) = &outer.signature else {
        panic!()
    };
    assert_eq!(s.r_sig, x_only);
    let Some(Ieee1609Dot2Data {
        content: Ieee1609Dot2Content::signedData(sd),
        ..
    }) = &outer.tbs_data.payload.data
    else {
        panic!("payload kept")
    };
    let Signature::ecdsaNistP256Signature(s) = &sd.signature else {
        panic!()
    };
    assert_eq!(s.r_sig, x_only, "the payload's signature is canonical too");
}
