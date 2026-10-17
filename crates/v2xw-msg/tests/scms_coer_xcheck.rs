//! COER cross-check of the generated IEEE 1609.2 / 1609.2.1 bindings against an
//! independent codec.
//!
//! `tests/fixtures/scms_coer_xcheck.txt` holds 200 random values of each of 45 types,
//! drawn, constraint-checked and encoded by `asn1tools` (`tools/asn1-xcheck/xcheck.py gen`),
//! which compiles the same committed ASN.1 files with its own parser and its own X.696
//! encoder and shares no code with `rasn-compiler` or `rasn`. For every value this test
//! decodes asn1tools' bytes with the generated Rust type and re-encodes it: COER is
//! canonical, so the two encoders agree on a value exactly when rasn's bytes equal
//! asn1tools' bytes. Any disagreement — a wrong tag, a lost extension marker, an OPTIONAL
//! the compiler made mandatory, a fixed-size string encoded with a length — fails here.
//!
//! The reverse direction (bytes rasn produced, re-encoded by asn1tools) is
//! `tests/scms_pdus.rs` with `xcheck.py verify`.

use std::collections::BTreeMap;

use v2xw_msg::sec_types::{
    ieee1609_dot2 as d2, ieee1609_dot2_crl as crl, ieee1609_dot2_crl_base_types as crlb,
    ieee1609_dot2_dot1_aca_ee_interface as aca_ee, ieee1609_dot2_dot1_aca_ra_interface as aca_ra,
    ieee1609_dot2_dot1_cert_management as cm, ieee1609_dot2_dot1_eca_ee_interface as eca_ee,
    ieee1609_dot2_dot1_ee_ra_interface as ee_ra, ieee1609_dot2_dot1_protocol as p,
};

const CORPUS: &str = include_str!("fixtures/scms_coer_xcheck.txt");
const PER_TYPE: usize = 200;

type RoundTrip = fn(&[u8]) -> Result<Vec<u8>, String>;

fn rt<T: rasn::Decode + rasn::Encode>(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let value: T = rasn::coer::decode(bytes).map_err(|e| format!("decode: {e}"))?;
    rasn::coer::encode(&value).map_err(|e| format!("encode: {e}"))
}

/// ASN.1 type name (as the fixture spells it) -> the generated Rust type.
fn table() -> BTreeMap<&'static str, RoundTrip> {
    let entries: [(&'static str, RoundTrip); 45] = [
        ("ScmsPdu", rt::<p::ScmsPdu>),
        ("SecurityMgmtSsp", rt::<p::SecurityMgmtSsp>),
        ("EeEcaCertRequestSpdu", rt::<p::EeEcaCertRequestSpdu>),
        ("EcaEeCertResponseSpdu", rt::<p::EcaEeCertResponseSpdu>),
        ("EeRaCertRequestSpdu", rt::<p::EeRaCertRequestSpdu>),
        ("RaEeCertAckSpdu", rt::<p::RaEeCertAckSpdu>),
        ("RaEeCertInfoSpdu", rt::<p::RaEeCertInfoSpdu>),
        ("RaEeCertAndAcpcInfoSpdu", rt::<p::RaEeCertAndAcpcInfoSpdu>),
        ("EeRaDownloadRequestPlainSpdu", rt::<p::EeRaDownloadRequestPlainSpdu>),
        ("EeRaDownloadRequestSpdu", rt::<p::EeRaDownloadRequestSpdu>),
        ("EeRaSuccessorEnrollmentCertRequestSpdu", rt::<p::EeRaSuccessorEnrollmentCertRequestSpdu>),
        ("RaEeEnrollmentCertAckSpdu", rt::<p::RaEeEnrollmentCertAckSpdu>),
        ("RaAcaCertRequestSpdu", rt::<p::RaAcaCertRequestSpdu>),
        ("AcaRaCertResponseSpdu", rt::<p::AcaRaCertResponseSpdu>),
        ("AcaEeCertResponsePlainSpdu", rt::<p::AcaEeCertResponsePlainSpdu>),
        ("AcaEeCertResponsePrivateSpdu", rt::<p::AcaEeCertResponsePrivateSpdu>),
        ("AcaEeCertResponseCubkSpdu", rt::<p::AcaEeCertResponseCubkSpdu>),
        ("CompositeCrlSpdu", rt::<p::CompositeCrlSpdu>),
        ("CertificateChainSpdu", rt::<p::CertificateChainSpdu>),
        ("MultiSignedCtlSpdu", rt::<p::MultiSignedCtlSpdu>),
        ("CtlSignatureSpdu", rt::<p::CtlSignatureSpdu>),
        (
            "CertificateManagementInformationStatusSpdu",
            rt::<p::CertificateManagementInformationStatusSpdu>,
        ),
        ("EeRaCertRequest", rt::<ee_ra::EeRaCertRequest>),
        ("RaEeCertAck", rt::<ee_ra::RaEeCertAck>),
        ("RaEeCertInfo", rt::<ee_ra::RaEeCertInfo>),
        ("EeRaDownloadRequest", rt::<ee_ra::EeRaDownloadRequest>),
        ("EeEcaCertRequest", rt::<eca_ee::EeEcaCertRequest>),
        ("EcaEeCertResponse", rt::<eca_ee::EcaEeCertResponse>),
        ("AcaEeCertResponse", rt::<aca_ee::AcaEeCertResponse>),
        ("RaAcaCertRequest", rt::<aca_ra::RaAcaCertRequest>),
        ("AcaRaCertResponse", rt::<aca_ra::AcaRaCertResponse>),
        ("EncryptedIndividualPLV", rt::<aca_ra::EncryptedIndividualPLV>),
        ("CertManagementPdu", rt::<cm::CertManagementPdu>),
        ("FullIeeeTbsCtl", rt::<cm::FullIeeeTbsCtl>),
        ("ToBeSignedCtlSignature", rt::<cm::ToBeSignedCtlSignature>),
        ("CertificateManagementInfoStatus", rt::<cm::CertificateManagementInfoStatus>),
        ("CompositeCrl", rt::<cm::CompositeCrl>),
        ("CertificateChain", rt::<cm::CertificateChain>),
        ("Ieee1609Dot2Data", rt::<d2::Ieee1609Dot2Data>),
        ("Certificate", rt::<d2::Certificate>),
        ("ToBeSignedCertificate", rt::<d2::ToBeSignedCertificate>),
        ("SignedData", rt::<d2::SignedData>),
        ("HeaderInfo", rt::<d2::HeaderInfo>),
        ("SecuredCrl", rt::<crl::SecuredCrl>),
        ("CrlContents", rt::<crlb::CrlContents>),
    ];
    let map: BTreeMap<_, _> = entries.into_iter().collect();
    assert_eq!(map.len(), 45, "duplicate type name in the table");
    map
}

fn unhex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "odd-length hex");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

/// With `V2XW_XCHECK_DUMP=<file>`, appends `Type IN_HEX OUT_HEX` for each disagreement, for
/// diagnosis with `xcheck.py`. Off by default; the test never writes otherwise.
fn dump(name: &str, input: &[u8], output: &[u8]) {
    use std::io::Write as _;
    let Some(path) = std::env::var_os("V2XW_XCHECK_DUMP") else {
        return;
    };
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{name} {} {}", hex(input), hex(output));
    }
}

struct Value {
    name: &'static str,
    bytes: Vec<u8>,
    /// asn1tools flagged the value `NESTED`: a SEQUENCE extension addition or CHOICE
    /// extension alternative is present inside another present one.
    nested: bool,
}

fn corpus() -> Vec<Value> {
    CORPUS
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let mut parts = l.split(' ');
            let name = parts.next().expect("type name");
            let bytes = unhex(parts.next().expect("hex"));
            let nested = match parts.next() {
                None => false,
                Some("NESTED") => true,
                Some(other) => panic!("unknown corpus flag {other:?}"),
            };
            Value { name, bytes, nested }
        })
        .collect()
}

/// The headline check: rasn's COER bytes equal asn1tools' for every value in the corpus,
/// except the `NESTED` class, which hits the rasn defect pinned below.
#[test]
fn rasn_reencodes_every_asn1tools_value_byte_for_byte() {
    let table = table();
    let mut failures = Vec::new();
    let mut per_type: BTreeMap<&str, usize> = BTreeMap::new();
    let (mut nested, mut nested_agree) = (0usize, 0usize);
    for v in corpus() {
        *per_type.entry(v.name).or_default() += 1;
        let Some(round_trip) = table.get(v.name) else {
            failures.push(format!("{}: not in the Rust table", v.name));
            continue;
        };
        let outcome = round_trip(&v.bytes);
        if v.nested {
            nested += 1;
            nested_agree += usize::from(outcome.as_ref().is_ok_and(|a| *a == v.bytes));
            continue;
        }
        match outcome {
            Ok(again) if again == v.bytes => {}
            Ok(again) => {
                dump(v.name, &v.bytes, &again);
                failures.push(format!(
                    "{}: re-encoding differs ({} bytes in, {} out; first difference at byte {})",
                    v.name,
                    v.bytes.len(),
                    again.len(),
                    v.bytes
                        .iter()
                        .zip(&again)
                        .position(|(a, b)| a != b)
                        .unwrap_or(v.bytes.len().min(again.len()))
                ));
            }
            Err(e) => {
                dump(v.name, &v.bytes, &[]);
                failures.push(format!("{}: {e}", v.name));
            }
        }
    }
    let mut by_type: BTreeMap<&str, usize> = BTreeMap::new();
    for f in &failures {
        *by_type.entry(f.split(':').next().unwrap_or("")).or_default() += 1;
    }
    let mut seen = std::collections::BTreeSet::new();
    let first_per_type: Vec<_> = failures
        .iter()
        .filter(|f| seen.insert(f.split(':').next().unwrap_or("").to_string()))
        .map(|f| f.chars().take(400).collect::<String>())
        .collect();
    assert!(
        failures.is_empty(),
        "{} corpus values disagree with asn1tools.\nby type: {by_type:?}\nfirst of each:\n{first_per_type:#?}",
        failures.len()
    );
    // Every type is covered, at the promised depth, outside the exempt class.
    for name in table.keys() {
        let n = per_type.get(name).copied().unwrap_or(0);
        assert!(n >= PER_TYPE, "{name}: {n} corpus values, want at least {PER_TYPE}");
    }
    assert_eq!(per_type.len(), table.len(), "corpus and table name different types");
    let total: usize = per_type.values().sum();
    assert!(
        nested * 20 < total,
        "{nested} of {total} values are exempt as NESTED; the exemption must stay a small class"
    );
    eprintln!(
        "xcheck: {} values agree byte for byte; {nested} NESTED values exempt ({nested_agree} of them agree)",
        total - nested
    );
}

/// The corpus is not trivially small values: it exercises long-form lengths and large
/// nested structures. If a regenerated corpus lost these, the check above would get
/// weaker without failing.
#[test]
fn the_corpus_exercises_the_hard_parts_of_coer() {
    let values = corpus();
    let long_form = values
        .iter()
        .filter(|v| v.bytes.windows(2).any(|w| w[0] == 0x81 || w[0] == 0x82))
        .count();
    let big = values.iter().filter(|v| v.bytes.len() > 500).count();
    assert!(long_form > 1000, "only {long_form} values contain a long-form length byte");
    assert!(big > 200, "only {big} values exceed 500 bytes");
}

/// **Known rasn 0.28 defect, pinned.** rasn's OER `encode_extension_addition` swaps its
/// output and scratch buffers to measure the open type's length; when the value being
/// encoded contains another extension addition (or a CHOICE extension alternative), the
/// inner call measures the *outer* buffer and the output is corrupt — asn1tools cannot
/// decode it. In the corpus this is exactly the `NESTED` class (243 values: `HeaderInfo`
/// and `SignedData` whose `HeaderInfo.requestedCertificate` carries a certificate with an
/// extension addition present). The US-profile 1609.2.1 SPDUs do not use
/// `requestedCertificate` (their `HeaderInfo` is `psid` only, 03 §3.1), so the engine's
/// SCMS traffic does not reach it today.
///
/// This test asserts the defect is still there. When a rasn upgrade fixes it, the test
/// fails: delete it and the `NESTED` exemption above.
#[test]
fn rasn_defect_nested_extension_additions_corrupt_the_encoding() {
    let shortest = corpus()
        .into_iter()
        .filter(|v| v.nested && v.name == "HeaderInfo")
        .min_by_key(|v| v.bytes.len())
        .expect("the corpus has NESTED HeaderInfo values");
    let again = rt::<d2::HeaderInfo>(&shortest.bytes).expect("rasn decodes the X.696 bytes");
    assert_ne!(
        again, shortest.bytes,
        "rasn now re-encodes a nested extension addition correctly: remove the NESTED exemption"
    );
    assert!(
        rasn::coer::decode::<d2::HeaderInfo>(&again).is_err() || rt::<d2::HeaderInfo>(&again).ok() != Some(again.clone()),
        "the corrupt encoding should not survive a second round trip unchanged"
    );
}

/// **Known rasn 0.28.14 defect, pinned.** `Psid ::= INTEGER (0..MAX)` is a non-negative
/// semi-constrained integer: X.696 §10 encodes it as a length octet and the minimum number
/// of unsigned octets. rasn's `Integer::to_unsigned_bytes_be` calls `to_signed_bytes_be`
/// for values held as big integers (2^63 and up), adding a spurious 0x00. Every PSID in
/// use is far below 2^63, and below that bound rasn is right (checked here at the edges);
/// the corpus generator caps unbounded integers at 63 bits for this reason. rasn 0.29.0
/// fixes the function; when the workspace moves to it, this test fails: delete the cap in
/// `tools/asn1-xcheck/xcheck.py` and this test.
#[test]
fn rasn_defect_psid_at_or_above_2_pow_63() {
    use v2xw_msg::sec_types::ieee1609_dot2_base_types::Psid;
    let enc = |v: u128| rasn::coer::encode(&Psid(rasn::types::Integer::from(v))).expect("encodes");
    // Correct below the bound (X.696: length, then minimal unsigned octets).
    assert_eq!(enc(0x80), [0x01, 0x80]);
    assert_eq!(enc(0x8007), [0x02, 0x80, 0x07]);
    assert_eq!(enc(0x7fff_ffff_ffff_ffff), [0x08, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
    // Wrong at and above it: X.696 wants 08 80 00 00 00 00 00 00 00.
    assert_eq!(
        enc(0x8000_0000_0000_0000),
        [0x09, 0x00, 0x80, 0, 0, 0, 0, 0, 0, 0],
        "rasn now encodes 2^63 correctly: remove the 63-bit cap in xcheck.py and this test"
    );
}
