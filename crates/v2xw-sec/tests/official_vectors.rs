//! WP-A4: butterfly key expansion and linkage values against official and independent
//! vectors (docs/design/scms-real/00 §4.2 WP-A4; gap rows 10 and 11; 02 §9, §10).
//!
//! The legacy self-referential vectors stay in `legacy_vectors.rs` as a regression; this
//! file is the acceptance test. Every value below is read from the transcribed corpus in
//! `tests/vectors/` (nothing is fetched at test time; provenance and sha256 in
//! docs/design/scms-real/vectors-provenance.md):
//!
//! - `camp_bfkeyexp.json`: CAMP `bfkeyexp.txt`, signing and encryption, every intermediate.
//! - `openscms_bfkeyexp.json`: OpenSCMS (Apache-2.0) encryption (i = 7590, j = 0) and
//!   signing (i = 7596, j = 0) unit-test vectors.
//! - `camp_lv.json`, `camp_glv.json`: CAMP `lv.txt` (i = 0, 1) and `glv.txt` (i = 0, 1).
//! - `oracle/butterfly.json`, `oracle/linkage.json`: emitted by the WP-A0 independent
//!   oracle (`tools/oracle`), which was written without reading this Rust: cocoon keys
//!   B and J, h_ij, the explicit (U, u) and implicit (P_U, e, r, d) ACA steps.
//! - `wpa4_butterfly_response.json`: the original-variant response encryption, composed
//!   from the oracle's ECIES, CCM and asn1tools COER by `tests/data/gen_butterfly_response_vectors.py`
//!   (which also re-opens each one with the oracle).
//!
//! Mutation proof (00 §4.1), run by WP-A4 and reported: swapping the 0x00000000 /
//! 0xFFFFFFFF prefixes, dropping the 0^112 seed pad, and a 32-bit la_id each turn tests
//! here red.

use p256::Scalar;
use serde_json::Value;
use v2xw_core::hash::sha256;
use v2xw_sec::butterfly::{self, ButterflyVariant, Caterpillar, response};
use v2xw_sec::ec::{self, Point};
use v2xw_sec::hashedid;
use v2xw_sec::linkage::{self, DeviceLinkageContext, GroupLinkageContext, LaId, LinkageSeed};
use v2xw_sec::{aes128, crypto::ecqv};

fn load(name: &str) -> Value {
    let path = format!("{}/tests/vectors/{name}", env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&raw).expect("corpus JSON")
}

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str()
        .unwrap_or_else(|| panic!("missing string `{k}`"))
}

fn u(v: &Value, k: &str) -> u32 {
    u32::try_from(v[k].as_u64().unwrap_or_else(|| panic!("missing int `{k}`"))).expect("u32")
}

fn hex(s: &str) -> Vec<u8> {
    assert!(s.len() % 2 == 0, "odd hex length");
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

fn arr<const N: usize>(s: &str) -> [u8; N] {
    hex(s).try_into().expect("width")
}

fn hx(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn scalar(s: &str) -> Scalar {
    ec::scalar_from_be32(&arr::<32>(s)).expect("scalar below n")
}

fn sh(k: &Scalar) -> String {
    hx(&ec::scalar_to_be32(k))
}

fn point_xy(x: &str, y: &str) -> Point {
    Point::from_sec1(&[vec![0x04], hex(x), hex(y)].concat()).expect("point")
}

fn opoint(v: &Value) -> Point {
    point_xy(s(v, "x"), s(v, "y"))
}

fn xy(p: &Point) -> (String, String) {
    let (x, y) = p.xy().expect("finite point");
    (hx(&x), hx(&y))
}

fn comp(p: &Point) -> String {
    hx(&p.compressed().expect("finite point"))
}

// --------------------------------------------------------------------------------------
// Butterfly expansion: CAMP bfkeyexp.txt
// --------------------------------------------------------------------------------------

#[test]
fn camp_bfkeyexp_signing_and_encryption_reproduce_every_intermediate() {
    let doc = load("camp_bfkeyexp.json");
    let vs = doc["vectors"].as_array().expect("vectors");
    assert_eq!(vs.len(), 2);
    let mut kinds = Vec::new();
    for v in vs {
        let (i, o) = (&v["in"], &v["out"]);
        let (pi, pj) = (u(i, "i"), u(i, "j"));
        let key: butterfly::ExpansionKey = arr(s(i, "key"));
        let signing = s(v, "kind") == "signing";
        kinds.push(signing);
        let x = if signing {
            butterfly::x_sign(pi, pj)
        } else {
            butterfly::x_enc(pi, pj)
        };
        assert_eq!(hx(&x), s(o, "x"), "{}: x", s(v, "id"));
        // AES_k(x+m) itself, before the Davies-Meyer XOR.
        let base = u128::from_be_bytes(x);
        for m in 1..=3u128 {
            let xm = base.wrapping_add(m).to_be_bytes();
            assert_eq!(hx(&xm), s(o, &format!("x{m}")));
            assert_eq!(
                hx(&aes128::encrypt_block(&key, &xm)),
                s(o, &format!("aes{m}")),
                "{}: aes{m}",
                s(v, "id")
            );
        }
        assert_eq!(
            hx(&butterfly::f_int(&key, &x)),
            s(o, "f_int"),
            "{}: f_int",
            s(v, "id")
        );
        let f = if signing {
            butterfly::f1(&key, pi, pj)
        } else {
            butterfly::f2(&key, pi, pj)
        };
        assert_eq!(sh(&f), s(o, "f"), "{}: f", s(v, "id"));

        let seed = scalar(s(i, "seed_private"));
        let seed_pub = point_xy(s(i, "seed_public_x"), s(i, "seed_public_y"));
        assert_eq!(Point::mul_base(&seed), seed_pub, "seed key pair");
        let want_pub = (
            s(o, "expanded_public_x").to_string(),
            s(o, "expanded_public_y").to_string(),
        );
        let unused = [0u8; 16];
        if signing {
            let cat = Caterpillar::from_parts(seed, Scalar::ONE, key, unused);
            assert_eq!(
                sh(&cat.cocoon_signing_private(pi, pj)),
                s(o, "expanded_private")
            );
            assert_eq!(
                xy(&butterfly::ra_cocoon_signing_key(&seed_pub, &key, pi, pj)),
                want_pub
            );
            // The legacy entry point agrees.
            let (b, _) =
                butterfly::ra_cocoon_keys(&seed_pub, &Point::GENERATOR, &key, &unused, pi, pj);
            assert_eq!(xy(&b), want_pub);
        } else {
            let cat = Caterpillar::from_parts(Scalar::ONE, seed, unused, key);
            assert_eq!(
                sh(&cat.encryption_private(pi, pj)),
                s(o, "expanded_private")
            );
            assert_eq!(
                xy(&butterfly::ra_cocoon_encryption_key(
                    &seed_pub, &key, pi, pj
                )),
                want_pub
            );
            let (_, q) =
                butterfly::ra_cocoon_keys(&Point::GENERATOR, &seed_pub, &unused, &key, pi, pj);
            assert_eq!(xy(&q), want_pub);
        }
    }
    assert_eq!(
        kinds,
        vec![true, false],
        "one signing and one encryption vector"
    );
}

// --------------------------------------------------------------------------------------
// Butterfly expansion: OpenSCMS
// --------------------------------------------------------------------------------------

#[test]
fn openscms_expansion_vectors_reproduce() {
    let doc = load("openscms_bfkeyexp.json");
    let vs = doc["vectors"].as_array().expect("vectors");
    assert_eq!(vs.len(), 2);
    for v in vs {
        let (i, o) = (&v["in"], &v["out"]);
        let (pi, pj) = (u(i, "i"), u(i, "j"));
        let key: butterfly::ExpansionKey = arr(s(i, "key"));
        let cat_pub = Point::from_sec1(&hex(s(i, "caterpillar_uncompressed"))).expect("point");
        let (x, f, cocoon) = if s(v, "kind") == "signing" {
            (
                butterfly::x_sign(pi, pj),
                butterfly::f1(&key, pi, pj),
                butterfly::ra_cocoon_signing_key(&cat_pub, &key, pi, pj),
            )
        } else {
            (
                butterfly::x_enc(pi, pj),
                butterfly::f2(&key, pi, pj),
                butterfly::ra_cocoon_encryption_key(&cat_pub, &key, pi, pj),
            )
        };
        if let Some(fi) = o["f_int"].as_str() {
            assert_eq!(hx(&butterfly::f_int(&key, &x)), fi, "{}: f_int", s(v, "id"));
        }
        assert_eq!(
            comp(&Point::mul_base(&f)),
            s(o, "f_point_compressed"),
            "{}",
            s(v, "id")
        );
        assert_eq!(comp(&cocoon), s(o, "cocoon_compressed"), "{}", s(v, "id"));
    }
}

// --------------------------------------------------------------------------------------
// Butterfly issuance: the independent oracle
// --------------------------------------------------------------------------------------

#[test]
fn oracle_butterfly_cocoon_keys_and_both_aca_variants_reproduce() {
    let doc = load("oracle/butterfly.json");
    let vs = doc["vectors"].as_array().expect("vectors");
    assert_eq!(vs.len(), 8);
    let aca_coer = hex(s(&doc, "aca_coer"));
    let q_ca = opoint(&doc["aca_public"]);
    let aca_id8 = hashedid::id8_bytes(&hashedid::hashed_id8(&aca_coer));
    for (n, v) in vs.iter().enumerate() {
        let (pi, pj) = (u(v, "i"), u(v, "j"));
        let (a, h) = (scalar(s(v, "a")), scalar(s(v, "h")));
        let (ck, ek): (butterfly::ExpansionKey, butterfly::ExpansionKey) =
            (arr(s(v, "ck")), arr(s(v, "ek")));
        let cat = Caterpillar::from_parts(a, h, ck, ek);
        assert_eq!(cat.signing_public(), opoint(&v["A"]), "#{n} A");
        assert_eq!(cat.encryption_public(), opoint(&v["H"]), "#{n} H");
        assert_eq!(hx(&butterfly::x_sign(pi, pj)), s(v, "x_sign"), "#{n}");
        assert_eq!(hx(&butterfly::x_enc(pi, pj)), s(v, "x_enc"), "#{n}");
        assert_eq!(
            sh(&butterfly::f1(&ck, pi, pj)),
            s(v, "f_sign"),
            "#{n} f_sign"
        );
        assert_eq!(sh(&butterfly::f2(&ek, pi, pj)), s(v, "f_enc"), "#{n} f_enc");

        // RA: cocoon keys. EE: their private halves.
        let b = cat.cocoon_signing_private(pi, pj);
        let big_b = butterfly::ra_cocoon_signing_key(&cat.signing_public(), &ck, pi, pj);
        assert_eq!(sh(&b), s(v, "b"), "#{n} b");
        assert_eq!(big_b, opoint(&v["B"]), "#{n} B");
        let h_ij = cat.encryption_private(pi, pj);
        let big_j = butterfly::ra_cocoon_encryption_key(&cat.encryption_public(), &ek, pi, pj);
        assert_eq!(sh(&h_ij), s(v, "h_ij"), "#{n} h_ij");
        assert_eq!(big_j, opoint(&v["J"]), "#{n} J");
        let original = (cat.encryption_public(), ek);
        let j_by_variant = butterfly::response_encryption_key(
            ButterflyVariant::Original,
            Some((&original.0, &original.1)),
            pi,
            pj,
        )
        .expect("original variant");
        assert_eq!(j_by_variant, big_j, "#{n} response key");

        // Explicit: U = B + c·G, u = b + c.
        let ex = &v["explicit"];
        let c = scalar(s(ex, "c"));
        let (big_u, _) = butterfly::pca_certify_explicit(&big_b, &c);
        assert_eq!(big_u, opoint(&ex["U"]), "#{n} U");
        let u_priv = cat.signing_private(pi, pj, &c);
        assert_eq!(sh(&u_priv), s(ex, "u"), "#{n} u");
        assert!(butterfly::derived_key_matches(&u_priv, &big_u));

        // Implicit: B plays R_U. P_U = B + k·G; e over (tbs, issuer); d = e·b + r.
        let im = &v["implicit"];
        let k = scalar(s(im, "k"));
        let p_u = butterfly::aca_certify_implicit(&big_b, &k);
        assert_eq!(p_u, opoint(&im["P_U"]), "#{n} P_U");
        let cert = hex(s(im, "cert_coer"));
        // Certificate header: preamble 0x00, version 3, type implicit, issuer
        // sha256AndDigest (0x80) + HashedId8(ACA). What follows is the COER tbs.
        assert_eq!(cert[..4], [0x00, 0x03, 0x01, 0x80], "#{n} header");
        assert_eq!(cert[4..12], aca_id8, "#{n} issuer");
        let tbs = &cert[12..];
        let e_digest = sha256(&[sha256(tbs), sha256(&aca_coer)].concat());
        let e = ecqv::hash_to_scalar(&e_digest);
        assert_eq!(sh(&e), s(im, "e"), "#{n} e");
        let r = scalar(s(im, "r"));
        let d = cat.signing_private_implicit(pi, pj, &e, &r);
        assert_eq!(sh(&d), s(im, "d"), "#{n} d");
        assert_eq!(
            Point::mul_base(&d),
            ecqv::reconstruct_public_key(&e, &p_u, &q_ca),
            "#{n} Q_U"
        );
    }
}

#[test]
fn the_unified_variants_are_refused_until_their_key_is_confirmed() {
    let (h, ek) = (Point::GENERATOR, [7u8; 16]);
    for v in [ButterflyVariant::Unified, ButterflyVariant::CompactUnified] {
        assert!(!v.is_confirmed());
        assert_eq!(
            butterfly::response_encryption_key(v, Some((&h, &ek)), 1, 2),
            Err(response::ResponseError::Unconfirmed(v))
        );
    }
    assert!(ButterflyVariant::default().is_confirmed());
    assert_eq!(
        butterfly::response_encryption_key(ButterflyVariant::Original, None, 1, 2),
        Err(response::ResponseError::MissingOriginalParams)
    );
}

// --------------------------------------------------------------------------------------
// Original-variant response encryption (oracle-composed)
// --------------------------------------------------------------------------------------

#[test]
fn the_original_variant_response_is_bit_exact_and_opens_only_with_h_ij() {
    let doc = load("wpa4_butterfly_response.json");
    let vs = doc["vectors"].as_array().expect("vectors");
    assert_eq!(vs.len(), 8);
    for (n, v) in vs.iter().enumerate() {
        let big_j = opoint(&v["J"]);
        let h_ij = scalar(s(v, "h_ij"));
        assert_eq!(Point::mul_base(&h_ij), big_j);
        assert_eq!(
            hx(&response::public_encryption_key_coer(&big_j).expect("pek")),
            s(v, "public_encryption_key_coer"),
            "#{n} PublicEncryptionKey"
        );
        assert_eq!(
            hx(&response::rek_recipient_id(&big_j).expect("id")),
            s(v, "recipient_id"),
            "#{n}"
        );
        let plaintext = hex(s(v, "plaintext"));
        let spdu = response::aca_encrypt(
            &big_j,
            &plaintext,
            &arr(s(v, "data_key")),
            &scalar(s(v, "ecies_v")),
            &arr(s(v, "nonce")),
        )
        .expect("encrypt");
        assert_eq!(hx(&spdu), s(v, "spdu_coer"), "#{n} encrypted SPDU bytes");
        let oracle_spdu = hex(s(v, "spdu_coer"));
        assert_eq!(
            response::ee_decrypt(&h_ij, &oracle_spdu).expect("decrypt"),
            plaintext,
            "#{n}"
        );
        // A different key is not a recipient; a tampered ciphertext fails CCM.
        assert_eq!(
            response::ee_decrypt(&(h_ij + Scalar::ONE), &oracle_spdu),
            Err(response::ResponseError::NoRecipient)
        );
        let mut bad = oracle_spdu.clone();
        let last = bad.len() - 1;
        bad[last] ^= 1;
        assert!(matches!(
            response::ee_decrypt(&h_ij, &bad),
            Err(response::ResponseError::Ccm(_))
        ));
    }
}

// --------------------------------------------------------------------------------------
// Linkage values: CAMP lv.txt and the oracle
// --------------------------------------------------------------------------------------

fn la(v: &Value, k: &str) -> LaId {
    LaId(u16::from_be_bytes(arr(s(v, k))))
}

fn seed(v: &Value, k: &str) -> LinkageSeed {
    LinkageSeed::new(arr(s(v, k)))
}

#[test]
fn camp_lv_seed_chain_plv_and_lv_reproduce() {
    let doc = load("camp_lv.json");
    let vs = doc["vectors"].as_array().expect("vectors");
    assert_eq!(vs.len(), 2);
    let mut periods = Vec::new();
    for v in vs {
        let (i, o) = (&v["in"], &v["out"]);
        let (la1, la2) = (la(i, "la_id1"), la(i, "la_id2"));
        let (pi, pj) = (u(i, "i"), u(i, "j"));
        periods.push(pi);
        let ls1 = linkage::linkage_seed_at(la1, seed(i, "ls1_0"), pi);
        let ls2 = linkage::linkage_seed_at(la2, seed(i, "ls2_0"), pi);
        assert_eq!(hx(ls1.as_bytes()), s(o, "ls1_i"), "{}", s(v, "id"));
        assert_eq!(hx(ls2.as_bytes()), s(o, "ls2_i"), "{}", s(v, "id"));
        if let Some(sha_out) = o["sha_out1"].as_str() {
            // The full digest CAMP prints, of which ls is the first 16 bytes.
            assert_eq!(sha_out[..32], *s(o, "ls1_i"));
            assert_eq!(s(o, "sha_out2")[..32], *s(o, "ls2_i"));
        }
        let plv1 = linkage::pre_linkage_value(la1, ls1, pj);
        let plv2 = linkage::pre_linkage_value(la2, ls2, pj);
        assert_eq!(hx(plv1.as_bytes()), s(o, "plv1"), "{}", s(v, "id"));
        assert_eq!(hx(plv2.as_bytes()), s(o, "plv2"), "{}", s(v, "id"));
        let lv = linkage::linkage_value(plv1, plv2);
        assert_eq!(hx(lv.as_bytes()), s(o, "lv"), "{}", s(v, "id"));
        let ctx = DeviceLinkageContext::new(la1, la2, seed(i, "ls1_0"), seed(i, "ls2_0"));
        assert_eq!(ctx.linkage_value_for(pi, pj), lv);
    }
    assert_eq!(periods, vec![0, 1], "both CAMP periods");
    // The headline value of gap row 11.
    assert_eq!(s(&vs[0]["out"], "lv"), "688acf70e25ec01f32");
}

#[test]
fn oracle_linkage_values_reproduce() {
    let doc = load("oracle/linkage.json");
    let vs = doc["vectors"].as_array().expect("vectors");
    assert_eq!(vs.len(), 10);
    let mut checked = 0;
    for (n, v) in vs.iter().enumerate() {
        let to_la = |k: &str| LaId(u16::try_from(v[k].as_u64().expect("la id")).expect("u16"));
        let (la1, la2) = (to_la("la_id1"), to_la("la_id2"));
        let pi = u(v, "i");
        let ctx = DeviceLinkageContext::new(la1, la2, seed(v, "ls1_0"), seed(v, "ls2_0"));
        assert_eq!(
            hx(linkage::linkage_seed_at(la1, ctx.ls1_0, pi).as_bytes()),
            s(v, "ls1_i"),
            "#{n}"
        );
        assert_eq!(
            hx(linkage::linkage_seed_at(la2, ctx.ls2_0, pi).as_bytes()),
            s(v, "ls2_i"),
            "#{n}"
        );
        for (j, want) in v["lv"].as_object().expect("lv map") {
            let j: u32 = j.parse().expect("j");
            assert_eq!(
                hx(ctx.linkage_value_for(pi, j).as_bytes()),
                want.as_str().expect("hex"),
                "#{n} j={j}"
            );
            checked += 1;
        }
    }
    assert!(checked >= 10, "only {checked} oracle lv values");
}

// --------------------------------------------------------------------------------------
// Group linkage: CAMP glv.txt
// --------------------------------------------------------------------------------------

#[test]
fn camp_glv_group_seeds_gplv_glv_and_encrypted_index_reproduce() {
    let doc = load("camp_glv.json");
    let vs = doc["vectors"].as_array().expect("vectors");
    assert_eq!(vs.len(), 2);
    for v in vs {
        let (i, o) = (&v["in"], &v["out"]);
        let ctx = GroupLinkageContext {
            la_id1: la(i, "la_id1"),
            la_id2: la(i, "la_id2"),
            gs1_0: seed(i, "gs1_0"),
            gs2_0: seed(i, "gs2_0"),
            ik: arr(s(i, "ik")),
        };
        let (pi, pj, pk) = (u(i, "i"), u(i, "j"), u(i, "k"));
        let (g1, g2) = ctx.group_seeds_at(pi);
        assert_eq!(hx(g1.as_bytes()), s(o, "gs1_i"), "{}", s(v, "id"));
        assert_eq!(hx(g2.as_bytes()), s(o, "gs2_i"), "{}", s(v, "id"));
        let p1 = linkage::group_pre_linkage_value(ctx.la_id1, g1, pj);
        let p2 = linkage::group_pre_linkage_value(ctx.la_id2, g2, pj);
        assert_eq!(hx(p1.as_bytes()), s(o, "gplv1"), "{}", s(v, "id"));
        assert_eq!(hx(p2.as_bytes()), s(o, "gplv2"), "{}", s(v, "id"));
        let glv = ctx.group_linkage_value_for(pi, pj);
        assert_eq!(hx(glv.as_bytes()), s(o, "glv"), "{}", s(v, "id"));
        assert_eq!(
            hx(&ctx.encrypted_index(pk, pj)),
            s(o, "ei"),
            "{}: ei",
            s(v, "id")
        );
        assert_eq!(
            s(o, "ei_in")[..16],
            format!("{pk:08x}{pj:08x}"),
            "printed order k || j"
        );
        // The certificate field carries the 9-byte value.
        let asn = linkage::GroupLinkageValue {
            j_value: pj.to_be_bytes(),
            value: glv,
        }
        .to_asn1();
        assert_eq!(hx(&asn.value), s(o, "glv"));
    }
    assert!(
        !linkage::GROUP_LINKAGE_DEFAULT,
        "group linkage is off by default (00 §2)"
    );
}
