//! Dual implementation of chain validation and certificate issuance against the oracle
//! (WP-A3 acceptance: "dual implementation against the oracle on 500 random chains").
//!
//! `tests/vectors/xcheck/chains.json` is written by `tools/chain-xcheck/chain_xcheck.py`,
//! which builds every certificate, signature and ECQV issuance with the independent
//! oracle (`tools/oracle/scms_oracle`) and records the oracle's own `chain.validate`
//! verdict. `python -I tools/chain-xcheck/chain_xcheck.py verify <file>` regenerates it
//! byte for byte. For every chain this test requires:
//!
//! 1. `v2xw_sec::chain::validate` reaches the oracle's verdict, and for an injected fault
//!    reports the rule the fault breaks;
//! 2. the SPDU signed with the end-entity key verifies under the key our validator
//!    resolved exactly when it verifies under the oracle's (this is where implicit
//!    certificates are judged: a wrong ECQV hash gives a key nobody holds);
//! 3. every certificate of an untampered chain, re-issued by our code from the recorded
//!    inputs (explicit: issuer key and nonce; implicit: requester key, CA ephemeral, CA
//!    key), is byte-identical to the oracle's, and our `r` equals its `r`.

use std::collections::BTreeSet;

use p256::Scalar;
use serde_json::Value;
use v2xw_msg::codec::MsgType;
use v2xw_msg::sec_types::{Certificate, coer};
use v2xw_sec::cert::{self, CertHash, CertKey};
use v2xw_sec::chain::{self, ChainErrorKind, ChainPolicy};
use v2xw_sec::ec::{Point, scalar_from_be32};

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn scalar(s: &str) -> Scalar {
    scalar_from_be32(&hex(s).try_into().expect("32 bytes")).expect("scalar")
}

fn sig(r: &str, s: &str) -> v2xw_msg::sec_types::Signature {
    use v2xw_msg::sec_types::ieee1609_dot2_base_types::{EccP256CurvePoint, EcdsaP256Signature};
    v2xw_msg::sec_types::Signature::ecdsaNistP256Signature(EcdsaP256Signature::new(
        EccP256CurvePoint::x_only(rasn::types::OctetString::from(hex(r))),
        rasn::types::OctetString::from(hex(s)),
    ))
}

/// The rule each injected fault must be reported under.
fn expected_kinds(mutation: &str) -> &'static [ChainErrorKind] {
    use ChainErrorKind::*;
    match mutation {
        "sig-tamper" => &[BadSignature],
        "tbs-tamper" => &[BadSignature],
        "issuer-id" => &[IssuerMismatch],
        "validity" => &[ValidityNotContained],
        "region" => &[RegionNotContained],
        "psid" => &[PermissionNotCovered],
        "eetype" => &[EeTypeNotAllowed],
        "ssp" => &[SspNotCovered],
        "chain-length" | "min-zero" => &[ChainLength],
        "untrusted-root" => &[UntrustedRoot],
        "root-sig" => &[BadRootSignature],
        _ => &[],
    }
}

fn tampered_after_issuance(mutation: &str) -> bool {
    matches!(
        mutation,
        "sig-tamper" | "tbs-tamper" | "implicit-tamper" | "issuer-id" | "root-sig"
    )
}

#[test]
fn five_hundred_oracle_chains_get_the_oracle_verdict() {
    let path = format!(
        "{}/tests/vectors/xcheck/chains.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("corpus")).expect("json");
    assert_eq!(doc["edition"], "1609.2-2022");
    let chains = doc["chains"].as_array().unwrap();
    assert_eq!(chains.len(), 500, "the acceptance count");

    let (mut valid, mut implicit_certs, mut reissued, mut spdu_valid) = (0, 0, 0, 0);
    let mut mutations_seen = BTreeSet::new();
    for c in chains {
        let id = c["id"].as_u64().unwrap();
        let mutation = c["mutation"].as_str().unwrap();
        mutations_seen.insert(mutation.to_string());
        let certs: Vec<Certificate> = c["certs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|h| coer::decode(MsgType::Crl, &hex(h.as_str().unwrap())).expect("decode"))
            .collect();
        for (h, cc) in c["certs"].as_array().unwrap().iter().zip(&certs) {
            assert_eq!(
                hex(h.as_str().unwrap()),
                cert::encode(cc).unwrap(),
                "chain {id}: COER round trip"
            );
        }
        let anchors: BTreeSet<[u8; 32]> = c["anchors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| hex(a.as_str().unwrap()).try_into().unwrap())
            .collect();
        let policy = ChainPolicy {
            anchors: Some(anchors),
            now: None,
        };

        // 1. The verdict.
        let oracle_valid = c["oracle_valid"].as_bool().unwrap();
        let ours = chain::validate(&certs, &policy);
        assert_eq!(
            ours.is_ok(),
            oracle_valid,
            "chain {id} ({mutation}): oracle {:?}, ours {:?}",
            c["oracle_errors"],
            ours.as_ref().err()
        );
        if let Err(errs) = &ours {
            let want = expected_kinds(mutation);
            assert!(
                want.is_empty() || errs.iter().any(|e| want.contains(&e.kind)),
                "chain {id} ({mutation}): expected one of {want:?}, got {errs:?}"
            );
        } else {
            valid += 1;
        }

        // 2. The SPDU under the end-entity key.
        let spdu = &c["spdu"];
        let digest = hex(spdu["digest"].as_str().unwrap());
        let s = sig(spdu["r"].as_str().unwrap(), spdu["s"].as_str().unwrap());
        if let Ok(v) = &ours {
            let ok = cert::ecdsa_verify_digest(v.ee_key(), &digest, &s);
            assert_eq!(
                ok,
                spdu["valid"].as_bool().unwrap(),
                "chain {id} ({mutation}): SPDU verdict"
            );
            if ok {
                spdu_valid += 1;
            }
        }

        // 3. Re-issue every certificate from the recorded inputs.
        if tampered_after_issuance(mutation) {
            continue;
        }
        let iss = c["issuance"].as_array().unwrap();
        for n in 0..certs.len() {
            let rec = &iss[n];
            let g = |k: &str| rec[k].as_str().unwrap();
            let d_issuer = scalar(g("d_issuer"));
            let q_issuer = CertKey::P256(Point::mul_base(&d_issuer));
            let mine = match g("kind") {
                "self" | "explicit" => {
                    let issuer = (g("kind") == "explicit").then(|| (&certs[n + 1], &q_issuer));
                    let k = scalar(g("k"));
                    cert::issue_explicit_cert(
                        certs[n].0.to_be_signed.clone(),
                        issuer,
                        CertHash::Sha256,
                        |dg| cert::ecdsa_p256_sign_with_k(&d_issuer, &k, &dg.try_into().unwrap()),
                    )
                    .unwrap()
                }
                "implicit" => {
                    implicit_certs += 1;
                    let CertKey::P256(q) = q_issuer else { unreachable!() };
                    let k = scalar(g("k"));
                    let ru = Point::mul_base(&scalar(g("k_u")));
                    let out = cert::issue_implicit(
                        &certs[n].0.to_be_signed,
                        &certs[n + 1],
                        &q,
                        &d_issuer,
                        &ru,
                        || k,
                    )
                    .unwrap();
                    assert_eq!(out.private_key_contribution, scalar(g("r")), "chain {id} cert {n}: r");
                    out.certificate
                }
                other => panic!("kind {other}"),
            };
            assert_eq!(
                cert::encode(&mine).unwrap(),
                cert::encode(&certs[n]).unwrap(),
                "chain {id} cert {n}: our certificate is byte-identical to the oracle's"
            );
            reissued += 1;
        }
    }
    // The corpus must exercise what it claims to.
    assert!(valid >= 150 && valid <= 350, "{valid} valid chains");
    assert!(implicit_certs >= 200, "{implicit_certs} implicit issuances");
    assert!(reissued >= 1200, "{reissued} certificates re-issued");
    assert!(spdu_valid >= 150, "{spdu_valid} SPDUs verified");
    for m in [
        "none", "sig-tamper", "tbs-tamper", "implicit-tamper", "issuer-id", "validity", "region",
        "psid", "eetype", "ssp", "chain-length", "min-zero", "untrusted-root", "root-sig",
    ] {
        assert!(mutations_seen.contains(m), "no chain exercises {m}");
    }
}
