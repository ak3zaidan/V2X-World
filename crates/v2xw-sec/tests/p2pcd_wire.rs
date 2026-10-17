//! WP-C3: the P2PCD fields and PDU on the wire (IEEE 1609.2 clause 8).
//!
//! * `Ieee1609dot2Peer2PeerPDU` is byte-for-byte what asn1tools emits from the published
//!   1609.2-2022 module (`tests/vectors/p2pcd_response.json`, made by
//!   `tests/data/gen_p2pcd_response_vectors.py`), and decodes back to the same
//!   certificates; a trailing octet, a wrong version and an empty `caCerts` are refused.
//! * `p2pcdLearningRequest`, `inlineP2pcdRequest` and `requestedCertificate` survive a sign
//!   and parse, and are inside the signature (`tbsData`).
//! * The two request fields are mutually exclusive on both sides (1609.2-2022 §6.3.9), and
//!   an ETSI receiver refuses a learning request (TS 103 097 §5.2).

mod common;

use common::{PSID_CAM, T0_UNIX, build_pki};
use v2xw_core::time::WallClock;
use v2xw_msg::codec::MsgType;
use v2xw_msg::sec_types::{Certificate, HashedId3, coer};
use v2xw_sec::crypto::Real;
use v2xw_sec::envelope::{
    Envelope, HeaderInfoSpec, P2PCD_EXCLUSIVE_RULE, SecurityEnvelope, SecurityEnvelopeInfo,
    SignerIdChoice,
};
use v2xw_sec::p2pcd::{decode_learning_response, encode_learning_response};
use v2xw_sec::testctx::TestCtx;
use v2xw_sec::{SecError, cert};

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn id3(b: [u8; 3]) -> HashedId3 {
    HashedId3(rasn::types::FixedOctetString::from(b))
}

#[test]
fn the_learning_response_pdu_matches_asn1tools_byte_for_byte() {
    let doc: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/vectors/p2pcd_response.json"
        ))
        .expect("vector file"),
    )
    .expect("json");
    let vectors = doc["vectors"].as_array().expect("vectors");
    assert_eq!(vectors.len(), 2);
    for v in vectors {
        let certs: Vec<Certificate> = v["certs_coer"]
            .as_array()
            .expect("certs")
            .iter()
            .map(|c| coer::decode(MsgType::Crl, &hex(c.as_str().expect("hex"))).expect("cert"))
            .collect();
        let want = hex(v["pdu_coer"].as_str().expect("pdu"));
        let got = encode_learning_response(&certs).expect("encodes");
        assert_eq!(got, want, "{}: Rust and asn1tools disagree", v["name"]);
        let back = decode_learning_response(&want).expect("decodes");
        assert_eq!(back, certs, "{}: decode loses a certificate", v["name"]);

        let mut trailing = want.clone();
        trailing.push(0);
        assert!(
            decode_learning_response(&trailing).is_err(),
            "trailing octet"
        );
        let mut version = want.clone();
        version[0] = 2;
        assert!(decode_learning_response(&version).is_err(), "version 2");
    }
}

#[test]
fn the_p2pcd_header_fields_round_trip_inside_the_signature() {
    let mut ctx = TestCtx::new(1);
    let mut backend = Real::new();
    let pki = build_pki(&mut ctx, &mut backend, 1);
    let env = Envelope::ieee1609(WallClock::new(T0_UNIX));
    let e = &pki.entities[0];
    let sign = |ctx: &mut TestCtx, backend: &mut Real, hdr: &HeaderInfoSpec| {
        env.sign(ctx, backend, &e.signer, b"p", hdr, SignerIdChoice::Digest)
    };
    let base = HeaderInfoSpec {
        psid: PSID_CAM,
        msg_type: Some(MsgType::Bsm),
        ..HeaderInfoSpec::default()
    };

    // None of the three: the same bytes as before WP-C3.
    let plain = sign(&mut ctx, &mut backend, &base).expect("signs");
    let p = env.parse(plain.bytes()).expect("parses");
    assert!(p.p2pcd_learning_request.is_none() && p.requested_certificate.is_none());
    assert!(p.inline_p2pcd_request.is_empty());

    // Out-of-band learning request: three octets plus the preamble bit.
    let learn = HeaderInfoSpec {
        p2pcd_learning_request: Some(id3([1, 2, 3])),
        ..base.clone()
    };
    let l = sign(&mut ctx, &mut backend, &learn).expect("signs");
    assert_eq!(
        l.size(),
        plain.size() + 3,
        "HashedId3 adds exactly three octets"
    );
    let p = env.parse(l.bytes()).expect("parses");
    assert_eq!(p.p2pcd_learning_request, Some(id3([1, 2, 3])));
    // Inside tbsData: the parsed tbs re-encoding carries the three octets.
    assert!(p.tbs_coer.windows(3).any(|w| w == [1, 2, 3]));

    // Inline request and an inline response in one SPDU (legal together).
    let pca: Certificate = (*pki.pca).clone();
    let inline = HeaderInfoSpec {
        inline_p2pcd_request: vec![id3([9, 8, 7]), id3([6, 5, 4])],
        requested_certificate: Some(pca.clone()),
        ..base.clone()
    };
    let i = sign(&mut ctx, &mut backend, &inline).expect("signs");
    let p = env.parse(i.bytes()).expect("parses");
    assert_eq!(p.inline_p2pcd_request, vec![id3([9, 8, 7]), id3([6, 5, 4])]);
    assert_eq!(p.requested_certificate.as_deref(), Some(&pca));
    let pca_len = cert::encode(&pca).expect("enc").len() as u32;
    assert!(
        i.size() > plain.size() + pca_len,
        "the requested certificate rides in the SPDU"
    );

    // Mutually exclusive, on the send side ...
    let both = HeaderInfoSpec {
        p2pcd_learning_request: Some(id3([1, 2, 3])),
        inline_p2pcd_request: vec![id3([4, 5, 6])],
        ..base.clone()
    };
    match sign(&mut ctx, &mut backend, &both) {
        Err(SecError::ProfileViolation { rule, .. }) => assert_eq!(rule, P2PCD_EXCLUSIVE_RULE),
        other => panic!("expected the exclusivity refusal, got {other:?}"),
    }
}

/// The receive side of the exclusivity rule and the ETSI ban, on bytes a non-conformant
/// sender made: the SPDU is built with the rule switched off by encoding the header by
/// hand through the bindings.
#[test]
fn a_receiver_refuses_both_request_fields_and_an_etsi_learning_request() {
    use v2xw_msg::sec_types::ieee1609_dot2_base_types::SequenceOfHashedId3;
    use v2xw_msg::sec_types::{Ieee1609Dot2Content, Ieee1609Dot2Data};
    let mut ctx = TestCtx::new(2);
    let mut backend = Real::new();
    let pki = build_pki(&mut ctx, &mut backend, 1);
    let env = Envelope::ieee1609(WallClock::new(T0_UNIX));
    let e = &pki.entities[0];
    let hdr = HeaderInfoSpec {
        psid: PSID_CAM,
        msg_type: Some(MsgType::Bsm),
        p2pcd_learning_request: Some(id3([1, 2, 3])),
        ..HeaderInfoSpec::default()
    };
    let ok = env
        .sign(
            &mut ctx,
            &mut backend,
            &e.signer,
            b"p",
            &hdr,
            SignerIdChoice::Digest,
        )
        .expect("signs");
    assert!(env.parse(ok.bytes()).is_ok());
    // ETSI: the same bytes are refused (TS 103 097 §5.2).
    let etsi = Envelope::etsi(WallClock::new(T0_UNIX));
    assert!(matches!(
        etsi.parse(ok.bytes()),
        Err(SecError::ProfileViolation { .. })
    ));

    // Add an inlineP2pcdRequest beside the learning request, as a broken sender would.
    let mut data: Ieee1609Dot2Data = coer::decode(MsgType::Crl, ok.bytes()).expect("dec");
    if let Ieee1609Dot2Content::signedData(s) = &mut data.content {
        s.tbs_data.header_info.inline_p2pcd_request =
            Some(SequenceOfHashedId3(vec![id3([4, 5, 6])]));
    } else {
        panic!("signedData");
    }
    let both = coer::encode(MsgType::Crl, &data).expect("enc");
    match env.parse(&both) {
        Err(SecError::ProfileViolation { rule, .. }) => assert_eq!(rule, P2PCD_EXCLUSIVE_RULE),
        other => panic!("expected the exclusivity refusal, got {other:?}"),
    }
}
