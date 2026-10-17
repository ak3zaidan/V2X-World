//! Implicit (ECQV) and explicit certificates against official and oracle vectors (WP-A3,
//! docs/design/scms-real/00 §4.2; gap rows 9 and 13).
//!
//! # What each test proves
//!
//! - [`camp_implicit_sec4_algebra`]: the ECQV algebra (`P_U = R_U + kG`, `r = ek + d_CA`,
//!   `d_U = e k_U + r`, `Q_U = e P_U + Q_CA`) against CAMP `implicit.txt`
//!   (`tests/vectors/camp_implicit.json`, transcribed by WP-A0 with citations). CAMP uses
//!   the **SEC 4** hash (`e` = leftmost 255 bits of SHA-256 of `tbs ‖ P_U`), so this test
//!   runs [`ecqv::cert_hash_sec4`]: it checks the algebra, not the 1609.2 profile (02 §6).
//! - [`oracle_ecqv_1609_issuance_and_reconstruction`]: the 1609.2 profile (full 256-bit
//!   `e` over `H(tbs) ‖ H(issuer)`) against the independent oracle
//!   (`tests/vectors/oracle/ecqv_1609.json`). No public 1609.2-profile ECQV vector exists
//!   (02 §14), so the oracle is the reference.
//! - [`oracle_butterfly_implicit_certificates_reconstruct`]: the implicit butterfly
//!   certificates of `oracle/butterfly.json` reconstruct to the key the oracle derived.
//! - [`oracle_explicit_chain`]: explicit certificates, their signed digests, HashedId8/10
//!   and the root HashedId32 against `oracle/chain_explicit.json`.
//!
//! # Faults injected to prove these can fail (each turned the named test red)
//!
//! - SEC 4 truncation (`>> 1`) in `cert_hash_1609`: the oracle ECQV and butterfly tests.
//! - hashing the whole certificate instead of `tbs` in 1609 mode: the same two.
//! - no truncation in `cert_hash_sec4`: the CAMP test.
//! - signer input `""` instead of the issuer certificate in `explicit_cert_digest`:
//!   the explicit-chain test.

use p256::Scalar;
use serde_json::Value;
use v2xw_msg::codec::MsgType;
use v2xw_msg::sec_types::{Certificate, coer};
use v2xw_sec::cert::{self, CertKey};
use v2xw_sec::crypto::ecqv;
use v2xw_sec::ec::{Point, scalar_from_be32, scalar_to_be32};

fn hex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "odd hex length");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn hx(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn scalar(s: &str) -> Scalar {
    let b: [u8; 32] = hex(s).try_into().expect("32 bytes");
    scalar_from_be32(&b).expect("scalar below n")
}

fn load(path: &str) -> Value {
    let full = format!("{}/tests/vectors/{path}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&full).expect("vector file")).expect("json")
}

fn point_xy(x: &str, y: &str) -> Point {
    let mut sec1 = vec![0x04];
    sec1.extend(hex(x));
    sec1.extend(hex(y));
    Point::from_sec1(&sec1).expect("point")
}

fn point_obj(v: &Value) -> Point {
    point_xy(v["x"].as_str().unwrap(), v["y"].as_str().unwrap())
}

fn decode_cert(h: &str) -> Certificate {
    coer::decode(MsgType::Crl, &hex(h)).expect("certificate decodes")
}

fn p256(k: &CertKey) -> Point {
    match k {
        CertKey::P256(p) => *p,
        CertKey::P384(_) => panic!("expected a P-256 key"),
    }
}

#[test]
fn camp_implicit_sec4_algebra() {
    let doc = load("camp_implicit.json");
    let vectors = doc["vectors"].as_array().unwrap();
    assert_eq!(vectors.len(), 2, "implicit.txt has two vectors");
    for v in vectors {
        let id = v["id"].as_str().unwrap();
        let (i, o) = (&v["in"], &v["out"]);
        let s = |k: &str| i[k].as_str().unwrap();
        let ku = scalar(s("ku"));
        let dca = scalar(s("dca"));
        let k = scalar(s("k"));
        let ru = Point::mul_base(&ku);
        let qca = Point::mul_base(&dca);
        let get = |k: &str| o[k].as_str().unwrap();
        assert_eq!(ru, point_xy(get("rux"), get("ruy")), "{id}: R_U");
        assert_eq!(qca, point_xy(get("qcax"), get("qcay")), "{id}: Q_CA");
        let pu = ecqv::reconstruction_value(&ru, &k);
        assert_eq!(pu, point_xy(get("pux"), get("puy")), "{id}: P_U");
        let mut cert_u = hex(s("tbs_cert"));
        cert_u.extend_from_slice(&pu.compressed().unwrap());
        assert_eq!(
            hx(&cert_u),
            get("cert_u"),
            "{id}: CertU = tbs ‖ compressed P_U"
        );
        let e = ecqv::cert_hash_sec4(&cert_u);
        let r = ecqv::private_key_contribution(&e, &k, &dca);
        assert_eq!(hx(&scalar_to_be32(&r)), get("r"), "{id}: r");
        let du = ecqv::subject_private_key(&e, &ku, &r);
        assert_eq!(hx(&scalar_to_be32(&du)), get("du"), "{id}: d_U");
        let qu = ecqv::reconstruct_public_key(&e, &pu, &qca);
        assert_eq!(qu, point_xy(get("qux"), get("quy")), "{id}: Q_U");
        assert!(ecqv::verify_reconstruction(&du, &qu), "{id}: d_U·G = Q_U");
    }
}

#[test]
fn oracle_ecqv_1609_issuance_and_reconstruction() {
    let doc = load("oracle/ecqv_1609.json");
    assert_eq!(doc["edition"], "1609.2-2022");
    let aca = decode_cert(doc["aca_coer"].as_str().unwrap());
    let q_aca = point_obj(&doc["aca_public"]);
    assert_eq!(
        p256(&cert::verification_key(&aca, None).unwrap()),
        q_aca,
        "the ACA's explicit key"
    );
    let vectors = doc["vectors"].as_array().unwrap();
    assert!(vectors.len() >= 12);
    for (n, v) in vectors.iter().enumerate() {
        let g = |k: &str| v[k].as_str().unwrap();
        let oracle_cert = decode_cert(g("cert_coer"));
        assert_eq!(
            hx(&cert::encode(&oracle_cert).unwrap()),
            g("cert_coer"),
            "#{n}: COER round trip"
        );
        assert_eq!(g("issuer_coer"), doc["aca_coer"].as_str().unwrap());

        // Issue the same certificate ourselves from the oracle's inputs.
        let ku = scalar(g("k_u"));
        let k = scalar(g("k"));
        let ru = Point::mul_base(&ku);
        assert_eq!(ru, point_obj(&v["R_U"]), "#{n}: R_U");
        // d_CA is not in the vector, so issuance runs with a stand-in private key and
        // `r` is checked through the public relation r·G = e·(k·G) + Q_CA below.
        let issued = cert::issue_implicit(
            &oracle_cert.0.to_be_signed,
            &aca,
            &q_aca,
            &Scalar::ONE,
            &ru,
            || k,
        )
        .expect("issue");
        assert_eq!(issued.attempts, 1);
        assert_eq!(
            hx(&cert::encode(&issued.certificate).unwrap()),
            g("cert_coer"),
            "#{n}: our implicit certificate is byte-identical to the oracle's"
        );
        assert_eq!(
            hx(&cert::tbs_canonical_coer(&issued.certificate.0.to_be_signed).unwrap()),
            g("tbs_coer"),
            "#{n}: tbs"
        );
        // e: the oracle's value is the full 256-bit hash; ours must equal it bit for bit.
        let e_bytes = ecqv::e_bytes_1609(
            &cert::tbs_canonical_coer(&issued.certificate.0.to_be_signed).unwrap(),
            &cert::canonical_coer(&aca).unwrap(),
        );
        assert_eq!(
            hx(&e_bytes),
            g("e"),
            "#{n}: e (full 256 bits, no SEC 4 truncation)"
        );
        assert_eq!(
            cert::implicit_e(&issued.certificate, &aca).unwrap(),
            issued.e
        );
        let pu = point_obj(&v["P_U"]);
        let r = scalar(g("r"));
        assert_eq!(
            Point::mul_base(&r),
            Point::mul_base(&k).mul(&issued.e).add(&q_aca),
            "#{n}: r·G = e·k·G + Q_CA"
        );
        assert_eq!(
            issued.certificate.0.to_be_signed.verify_key_indicator,
            cert::reconstruction_key_indicator(&pu.compressed().unwrap()).unwrap(),
            "#{n}: P_U"
        );
        let du = ecqv::subject_private_key(&issued.e, &ku, &r);
        assert_eq!(hx(&scalar_to_be32(&du)), g("d_u"), "#{n}: d_U");

        // Reconstruction by a relying party.
        let q = p256(
            &cert::verification_key(&oracle_cert, Some((&aca, &CertKey::P256(q_aca)))).unwrap(),
        );
        assert_eq!(q, point_obj(&v["Q_U"]), "#{n}: Q_U");
        assert_eq!(Point::mul_base(&du), q, "#{n}: d_U·G = Q_U");
        assert_eq!(
            hx(&cert::cert_id8(&oracle_cert).unwrap().0[..]),
            g("cert_hashed_id8"),
            "#{n}: HashedId8"
        );
    }
}

#[test]
fn oracle_butterfly_implicit_certificates_reconstruct() {
    let doc = load("oracle/butterfly.json");
    let aca = decode_cert(doc["aca_coer"].as_str().unwrap());
    let q_aca = CertKey::P256(point_obj(&doc["aca_public"]));
    for (n, v) in doc["vectors"].as_array().unwrap().iter().enumerate() {
        let imp = &v["implicit"];
        let c = decode_cert(imp["cert_coer"].as_str().unwrap());
        let e = cert::implicit_e(&c, &aca).unwrap();
        assert_eq!(e, scalar(imp["e"].as_str().unwrap()), "#{n}: e");
        let q = p256(&cert::verification_key(&c, Some((&aca, &q_aca))).unwrap());
        let d = scalar(imp["d"].as_str().unwrap());
        assert_eq!(Point::mul_base(&d), q, "#{n}: d = e·b + r reconstructs");
        // B as R_U: our d from the vector's b and r.
        let b = scalar(v["b"].as_str().unwrap());
        let r = scalar(imp["r"].as_str().unwrap());
        assert_eq!(
            ecqv::subject_private_key(&e, &b, &r),
            d,
            "#{n}: d = e·b + r"
        );
    }
}

#[test]
fn oracle_explicit_chain() {
    let doc = load("oracle/chain_explicit.json");
    let g = |k: &str| doc[k].as_str().unwrap();
    let root = decode_cert(g("root_coer"));
    let aca = decode_cert(g("aca_coer"));
    assert_eq!(
        hx(&cert::cert_id8(&root).unwrap().0[..]),
        g("root_hashed_id8")
    );
    assert_eq!(
        hx(&cert::hashed_id32(&root).unwrap()),
        g("root_hashed_id32")
    );
    assert_eq!(
        hx(&cert::cert_id8(&aca).unwrap().0[..]),
        g("aca_hashed_id8")
    );
    assert!(
        cert::verify_explicit_cert(&root, None).unwrap(),
        "root self-signature"
    );
    let root_key = cert::verification_key(&root, None).unwrap();
    assert!(
        cert::verify_explicit_cert(&aca, Some((&root, &root_key))).unwrap(),
        "ACA by root"
    );
    let aca_key = cert::verification_key(&aca, None).unwrap();
    // The ACA signature does not verify under the wrong issuer.
    assert!(!cert::verify_explicit_cert(&aca, Some((&aca, &aca_key))).unwrap());
    for (n, v) in doc["vectors"].as_array().unwrap().iter().enumerate() {
        let s = |k: &str| v[k].as_str().unwrap();
        let ee = decode_cert(s("cert_coer"));
        let digest =
            cert::explicit_cert_digest(&ee.0.to_be_signed, Some(&aca), cert::CertHash::Sha256)
                .unwrap();
        assert_eq!(hx(&digest), s("signed_digest"), "#{n}: signed digest");
        assert!(
            cert::verify_explicit_cert(&ee, Some((&aca, &aca_key))).unwrap(),
            "#{n}: verifies"
        );
        assert_eq!(
            hx(&cert::cert_id8(&ee).unwrap().0[..]),
            s("hashed_id8"),
            "#{n}"
        );
        assert_eq!(hx(&cert::cert_id10(&ee).unwrap()), s("hashed_id10"), "#{n}");
        // Re-issue: our own tbs encoding and issuer field reproduce the oracle's bytes
        // apart from the signature, which we produce with our own ECDSA and verify.
        let d = scalar(s("d"));
        let key = cert::verification_key(&ee, None).unwrap();
        assert_eq!(p256(&key), Point::mul_base(&d), "#{n}: EE key");
        let ours = cert::issue_explicit_cert(
            ee.0.to_be_signed.clone(),
            Some((&aca, &aca_key)),
            cert::CertHash::Sha256,
            |dg| {
                let dg: [u8; 32] = dg.try_into().unwrap();
                // A stand-in signer key: only the structure is compared here.
                cert::ecdsa_p256_sign_with_k(&Scalar::from(7u64), &Scalar::from(11u64), &dg)
            },
        )
        .unwrap();
        assert_eq!(ours.0.issuer, ee.0.issuer, "#{n}: issuer field");
        assert_eq!(
            hx(&cert::tbs_canonical_coer(&ours.0.to_be_signed).unwrap()),
            s("tbs_coer"),
            "#{n}: tbs"
        );
        // A one-byte change in the tbs breaks the signature.
        let mut bad = ee.clone();
        bad.0.to_be_signed.crl_series.0.0 ^= 1;
        assert!(
            !cert::verify_explicit_cert(&bad, Some((&aca, &aca_key))).unwrap(),
            "#{n}: tamper"
        );
    }
}

#[test]
fn ecdsa_with_k_matches_the_oracle_signatures() {
    // oracle/ecdsa_1609.json fixes d and k, so our deterministic signer must reproduce
    // r and s exactly, and the crate verifier must accept them.
    let doc = load("oracle/ecdsa_1609.json");
    let mut checked = 0;
    for v in doc["vectors"].as_array().unwrap() {
        if v["curve"] != "P-256" {
            continue;
        }
        let s = |k: &str| v[k].as_str().unwrap();
        let d = scalar(s("d"));
        let k = scalar(s("k"));
        let digest: [u8; 32] = hex(s("digest")).try_into().unwrap();
        let sig = cert::ecdsa_p256_sign_with_k(&d, &k, &digest).unwrap();
        let enc = hx(&coer::encode(MsgType::Crl, &sig).unwrap());
        assert!(enc.ends_with(&format!("{}{}", s("r"), s("s"))), "r ‖ s");
        assert!(cert::ecdsa_verify_digest(
            &CertKey::P256(Point::mul_base(&d)),
            &digest,
            &sig
        ));
        let mut wrong = digest;
        wrong[0] ^= 1;
        assert!(!cert::ecdsa_verify_digest(
            &CertKey::P256(Point::mul_base(&d)),
            &wrong,
            &sig
        ));
        checked += 1;
    }
    assert_eq!(checked, 6);
}

#[test]
fn the_sec4_retry_draws_a_new_k_when_the_key_would_be_infinity() {
    // Force e·P_U + Q_CA = O on the first draw: pick Q_CA = -e·P_U for a fixed tbs and k.
    let doc = load("oracle/ecqv_1609.json");
    let aca = decode_cert(doc["aca_coer"].as_str().unwrap());
    let template = decode_cert(doc["vectors"][0]["cert_coer"].as_str().unwrap());
    let ru = Point::mul_base(&Scalar::from(5u64));
    let k1 = Scalar::from(9u64);
    let pu = ecqv::reconstruction_value(&ru, &k1);
    let mut t = template.0.to_be_signed.clone();
    t.verify_key_indicator = cert::reconstruction_key_indicator(&pu.compressed().unwrap()).unwrap();
    let e = ecqv::cert_hash_1609(
        &cert::tbs_canonical_coer(&t).unwrap(),
        &cert::canonical_coer(&aca).unwrap(),
    );
    let d_bad = -(e * Scalar::from(14u64)); // Q_CA = -(e·14)·G = -e·P_U, since P_U = 14·G
    let q_bad = Point::mul_base(&d_bad);
    let mut draws = vec![Scalar::from(21u64), k1];
    let issued = cert::issue_implicit(&template.0.to_be_signed, &aca, &q_bad, &d_bad, &ru, || {
        draws.pop().unwrap()
    })
    .expect("issues on the second draw");
    assert_eq!(issued.attempts, 2, "the first k must be refused");
    assert_eq!(issued.k, Scalar::from(21u64));
}
