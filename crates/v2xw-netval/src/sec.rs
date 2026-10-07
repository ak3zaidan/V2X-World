//! Layer 4: the IEEE 1609.2 envelope against an independent parser and OpenSSL, ECDSA
//! interoperability, certificate-chain and revocation decisions, and butterfly key
//! expansion against independent curve arithmetic.

use std::sync::Arc;

use serde_json::{Value, json};
use v2xw_core::ids::NodeId;
use v2xw_core::time::WallClock;
use v2xw_msg::codec::MsgType;
use v2xw_msg::sec_types::Certificate;
use v2xw_sec::butterfly::{self, Caterpillar};
use v2xw_sec::cert::{self, CertSpec, HolderId};
use v2xw_sec::crypto::{CryptoBackend, CryptoBackendInfo, KeyHandle, Real, SigToken};
use v2xw_sec::ec;
use v2xw_sec::envelope::{
    CrlStore, Envelope, HeaderInfoSpec, PeerCertCache, PlanOutcome, RejectReason,
    SecurityEnvelope, SecurityEnvelopeInfo, SignerHandle, SignerIdChoice, TrustStore,
};
use v2xw_sec::hashedid::certificate_digest;
use v2xw_sec::linkage::{CrlLinkageEntry, DeviceLinkageContext, LaId, LinkageSeed};
use v2xw_sec::primitive::PrimitiveId;
use v2xw_sec::testctx::TestCtx;

use crate::{Check, Cost, Layer, Mode, Outcome};

/// 2026-01-01T00:00:00Z.
const T0_UNIX: i64 = 1_767_225_600;
const T0_1609: u32 = (T0_UNIX - v2xw_core::time::IEEE1609_EPOCH_UNIX_S) as u32;
const PSID_BSM: u64 = 0x20;

/// The checks of this layer.
#[must_use]
pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "SEC-01",
            layer: Layer::Security,
            title: "ECDSA P-256 interoperates with OpenSSL in both directions",
            reference: "FIPS 186-4 ECDSA over P-256 with SHA-256 (IEEE 1609.2 §5.3.1); OpenSSL through Python `cryptography` as the independent implementation: 32 Rust signatures verified by OpenSSL, 32 OpenSSL signatures verified by the Rust backend, 32 tampered digests rejected by both",
            tolerance: "every verdict as expected",
            fault: "The signature's r and s halves swapped on export",
            cost: Cost::Fast,
            run: ecdsa_interop,
        },
        Check {
            id: "SEC-02",
            layer: Layer::Security,
            title: "A signed BSM's IEEE 1609.2 SignedData decodes field by field under an independent COER parser, and its signature verifies under OpenSSL",
            reference: "IEEE 1609.2-2016 §6.3 (Ieee1609Dot2Data, SignedData, HeaderInfo, SignerIdentifier, EcdsaP256Signature x-only), §5.3.1 (signature over H(H(tbsData) ‖ H(signer certificate))), §6.3.9 (HashedId8 = low-order 8 octets of SHA-256); ITU-T X.696 COER; reference/security.py",
            tolerance: "every field equal; signature valid; no trailing octets; 12 SPDUs",
            fault: "The signer handle pairs one vehicle's certificate with another vehicle's key",
            cost: Cost::Fast,
            run: spdu_independent,
        },
        Check {
            id: "SEC-03",
            layer: Layer::Security,
            title: "Receiver decisions: a valid chain verifies, a revoked pseudonym is rejected in its period and every later one, another device is not, an expired certificate is rejected",
            reference: "IEEE 1609.2 §5.1.2.3 and §6.4.8 (validity); IEEE 1609.2.1 §5.1.3.4 / CAMP linkage-based revocation (lv(i,j) from (la_id1, ls1), (la_id2, ls2); the CRL carries the seeds at period i, which a receiver steps forward); pseudonym validity 10,140 min (CAMP: one week plus one hour of overlap)",
            tolerance: "exact decisions on 6 cases",
            fault: "The CRL entry built with the two linkage authorities' identifiers swapped",
            cost: Cost::Fast,
            run: receiver_decisions,
        },
        Check {
            id: "SEC-04",
            layer: Layer::Security,
            title: "Butterfly key expansion: cocoon key B = A + f·G, certified key = B + c·G, and the vehicle's private key d = a + f + c satisfies d·G = certified, for 12 (i, j)",
            reference: "IEEE 1609.2.1-2020 §9.3 (butterfly key expansion, explicit certificates); independent pure-Python P-256 arithmetic over the FIPS 186-4 D.1.2.3 curve (reference/security.py)",
            tolerance: "exact point equality",
            fault: "The PCA's randomiser c omitted from the vehicle's private key",
            cost: Cost::Fast,
            run: butterfly_identity,
        },
    ]
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap_or(0)).collect()
}

struct Entity {
    key: KeyHandle,
    certificate: Arc<Certificate>,
    signer: SignerHandle,
    linkage: DeviceLinkageContext,
}

struct Pki {
    root: Arc<Certificate>,
    pca: Arc<Certificate>,
    entities: Vec<Entity>,
}

fn device_linkage(device: u8) -> DeviceLinkageContext {
    DeviceLinkageContext::new(
        LaId(0x0001),
        LaId(0x0002),
        LinkageSeed::new([device; 16]),
        LinkageSeed::new([device.wrapping_add(0x80); 16]),
    )
}

/// A root, a PCA under it, and `count` pseudonym certificates under that, for period
/// `i_cert`.
fn build_pki(ctx: &mut TestCtx, backend: &mut Real, count: u32, i_cert: u16) -> Pki {
    let p = PrimitiveId::ECDSA_P256_SHA256;
    let root_key = backend.keygen(ctx, p, NodeId::new(900)).expect("root keygen");
    let root_material = backend.public_material(&backend.public_of(&root_key).expect("pub")).expect("material");
    let root = Arc::new(cert::trust_anchor(&CertSpec::authority(T0_1609, PSID_BSM), &root_material).expect("root"));
    let root_coer = cert::encode(&root).expect("root encodes");
    let root_digest = certificate_digest(&root).expect("digest");
    let pca_key = backend.keygen(ctx, p, NodeId::new(901)).expect("pca keygen");
    let pca_material = backend.public_material(&backend.public_of(&pca_key).expect("pub")).expect("material");
    let mut pca_spec = CertSpec::authority(T0_1609, PSID_BSM);
    pca_spec.issuer = Some(root_digest);
    let pca = Arc::new(
        cert::issue_explicit(&pca_spec, &pca_material, &root_coer, |digest| {
            backend.sign_prehashed(ctx, &root_key, digest)?.to_ieee1609_signature()
        })
        .expect("pca"),
    );
    let pca_coer = cert::encode(&pca).expect("pca encodes");
    let pca_digest = certificate_digest(&pca).expect("digest");
    let mut entities = Vec::new();
    for n in 0..count {
        let node = NodeId::new(n);
        let key = backend.keygen(ctx, p, node).expect("keygen");
        let material = backend.public_material(&backend.public_of(&key).expect("pub")).expect("material");
        let linkage = device_linkage((n + 1) as u8);
        let spec = CertSpec::pseudonym(
            pca_digest.clone(),
            HolderId::Linkage { i_cert: i_cert, linkage_value: linkage.linkage_value_for(u32::from(i_cert), n) },
            T0_1609,
            PSID_BSM,
        );
        let certificate = Arc::new(
            cert::issue_explicit(&spec, &material, &pca_coer, |digest| {
                backend.sign_prehashed(ctx, &pca_key, digest)?.to_ieee1609_signature()
            })
            .expect("ee"),
        );
        let signer = SignerHandle::new(node, key, certificate.clone()).expect("signer");
        entities.push(Entity { key, certificate, signer, linkage });
    }
    Pki { root, pca, entities }
}

fn digest_of(i: u32) -> [u8; 32] {
    v2xw_core::hash::sha256(format!("netval digest {i}").as_bytes())
}

fn ecdsa_interop(mode: Mode) -> Outcome {
    let mut ctx = TestCtx::new(11);
    let mut backend = Real::new();
    let p = PrimitiveId::ECDSA_P256_SHA256;
    let mut verify = Vec::new();
    for i in 0..32u32 {
        let key = backend.keygen(&mut ctx, p, NodeId::new(i)).expect("keygen");
        let pubm = backend.public_material(&backend.public_of(&key).expect("pub")).expect("material");
        let d = digest_of(i);
        let sig = backend.sign_prehashed(&mut ctx, &key, &d).expect("signs");
        let (mut r, mut s) = (sig.r().expect("r"), sig.s().expect("s"));
        if mode.faulted() {
            core::mem::swap(&mut r, &mut s);
        }
        verify.push(json!({"pub": hex(&pubm), "digest": hex(&d), "r": hex(&r), "s": hex(&s)}));
        // The same signature against a tampered digest must be refused.
        let mut t = d;
        t[0] ^= 1;
        verify.push(json!({"pub": hex(&pubm), "digest": hex(&t), "r": hex(&r), "s": hex(&s)}));
    }
    let sign: Vec<Value> = (100..132u32).map(|i| Value::String(hex(&digest_of(i)))).collect();
    let out = match crate::python("security.py", &json!({"op": "ecdsa", "verify": verify, "sign": sign})) {
        Ok(v) => v,
        Err(e) => return Outcome::skip(e),
    };
    let verdicts: Vec<bool> = out["verify"].as_array().map(|a| a.iter().map(|v| v.as_bool() == Some(true)).collect()).unwrap_or_default();
    let openssl_ok = verdicts.iter().step_by(2).filter(|v| **v).count();
    let openssl_tamper_rejected = verdicts.iter().skip(1).step_by(2).filter(|v| !**v).count();
    let mut rust_ok = 0;
    let mut rust_tamper_rejected = 0;
    for (k, item) in out["signed"].as_array().into_iter().flatten().enumerate() {
        let pubm = unhex(item["pub"].as_str().unwrap_or(""));
        let Ok(pk) = backend.import_public(p, NodeId::new(500 + k as u32), &pubm) else {
            continue;
        };
        let mut bytes = unhex(item["r"].as_str().unwrap_or(""));
        bytes.extend(unhex(item["s"].as_str().unwrap_or("")));
        let token = SigToken { primitive: p, bytes };
        let d: [u8; 32] = unhex(item["digest"].as_str().unwrap_or("")).try_into().unwrap_or([0; 32]);
        if backend.verify_prehashed(&mut ctx, &pk, &d, &token) {
            rust_ok += 1;
        }
        let mut t = d;
        t[31] ^= 0x80;
        if !backend.verify_prehashed(&mut ctx, &pk, &t, &token) {
            rust_tamper_rejected += 1;
        }
    }
    Outcome::judge(
        openssl_ok == 32 && openssl_tamper_rejected == 32 && rust_ok == 32 && rust_tamper_rejected == 32,
        format!("OpenSSL accepted {openssl_ok}/32 Rust signatures and rejected {openssl_tamper_rejected}/32 tampered; Rust accepted {rust_ok}/32 OpenSSL signatures and rejected {rust_tamper_rejected}/32 tampered"),
    )
}

fn spdu_independent(mode: Mode) -> Outcome {
    let mut ctx = TestCtx::new(12);
    let mut backend = Real::new();
    let pki = build_pki(&mut ctx, &mut backend, 3, 7);
    let envelope = Envelope::ieee1609(WallClock::new(T0_UNIX));
    let mut items = Vec::new();
    let mut expected = Vec::new();
    for k in 0..12u32 {
        let e = &pki.entities[(k % 3) as usize];
        let other = &pki.entities[((k + 1) % 3) as usize];
        let signer = if mode.faulted() {
            SignerHandle::new(NodeId::new(k % 3), other.key, e.certificate.clone()).expect("signer")
        } else {
            e.signer.clone()
        };
        ctx.set_now(u64::from(k) * 100_000_000);
        let payload: Vec<u8> = (0..(20 + 13 * k)).map(|b| (b * 7 + k) as u8).collect();
        let hdr = HeaderInfoSpec { psid: PSID_BSM, msg_type: Some(MsgType::Bsm), ..HeaderInfoSpec::default() };
        let pdu = envelope.sign(&mut ctx, &mut backend, &signer, &payload, &hdr, SignerIdChoice::Digest).expect("signs");
        let parsed = envelope.parse(pdu.bytes()).expect("parses");
        let pubm = backend.public_material(&backend.public_of(&e.key).expect("pub")).expect("material");
        items.push(json!({"spdu": hex(pdu.bytes()), "cert": hex(e.signer.cert_coer()), "pub": hex(&pubm)}));
        expected.push(json!({
            "protocolVersion": 3, "content": 1, "hashId": 0, "innerVersion": 3,
            "payload": hex(&payload), "psid": PSID_BSM, "generationTime": parsed.generation_time,
            "signerChoice": 0, "signatureValid": true, "trailing": 0,
        }));
    }
    let out = match crate::python("security.py", &json!({"op": "spdu", "items": items})) {
        Ok(v) => v,
        Err(e) => return Outcome::skip(e),
    };
    let mut wrong = Vec::new();
    for (i, (got, want)) in out["items"].as_array().into_iter().flatten().zip(&expected).enumerate() {
        if let Some(err) = got.get("error") {
            wrong.push(format!("SPDU {i}: {err}"));
            continue;
        }
        for (k, v) in want.as_object().expect("object") {
            if got.get(k) != Some(v) {
                wrong.push(format!("SPDU {i} {k}: {} vs {v}", got.get(k).cloned().unwrap_or(Value::Null)));
            }
        }
        if got.get("signerDigest") != got.get("expectedDigest") {
            wrong.push(format!("SPDU {i}: signer digest is not the low-order 8 octets of SHA-256(cert)"));
        }
    }
    Outcome::judge(
        wrong.is_empty() && expected.len() == 12,
        if wrong.is_empty() { "12 SPDUs: every field equal, HashedId8 right, OpenSSL verifies every signature".to_string() } else { format!("{} problems; first: {}", wrong.len(), wrong[0]) },
    )
}

fn receiver_decisions(mode: Mode) -> Outcome {
    let mut ctx = TestCtx::new(13);
    let mut backend = Real::new();
    let pki = build_pki(&mut ctx, &mut backend, 2, 7);
    let pki8 = build_pki(&mut TestCtx::new(14), &mut Real::new(), 1, 8);
    let envelope = Envelope::ieee1609(WallClock::new(T0_UNIX));
    let mut anchors = TrustStore::new();
    anchors.insert(pki.root.clone()).expect("root");
    let mut cache = PeerCertCache::new();
    cache.insert(pki.pca.clone(), true).expect("pca");
    let mut anchors8 = TrustStore::new();
    anchors8.insert(pki8.root.clone()).expect("root");
    let mut cache8 = PeerCertCache::new();
    cache8.insert(pki8.pca.clone(), true).expect("pca");
    let hdr = HeaderInfoSpec { psid: PSID_BSM, msg_type: Some(MsgType::Bsm), ..HeaderInfoSpec::default() };
    let mut sign = |ctx: &mut TestCtx, backend: &mut Real, signer: &SignerHandle, at: u64| {
        ctx.set_now(at);
        let pdu = envelope.sign(ctx, backend, signer, b"bsm", &hdr, SignerIdChoice::Certificate).expect("signs");
        envelope.parse(pdu.bytes()).expect("parses")
    };
    // The CRL: device 1 (entity 0) revoked from period 7. The fault swaps the LA ids.
    let dev = &pki.entities[0].linkage;
    let crl_ctx = if mode.faulted() {
        DeviceLinkageContext::new(LaId(0x0002), LaId(0x0001), LinkageSeed::new([1; 16]), LinkageSeed::new([0x81; 16]))
    } else {
        dev.clone()
    };
    let mut crl = CrlStore::new();
    crl.add_linkage_entry(CrlLinkageEntry::from_device(&crl_ctx, 7, 20));
    let empty = CrlStore::new();
    let mut parts = Vec::new();
    let p0 = sign(&mut ctx, &mut backend, &pki.entities[0].signer, 1_000_000_000);
    let p1 = sign(&mut ctx, &mut backend, &pki.entities[1].signer, 1_000_000_000);
    let plan = |p, c: &PeerCertCache, a: &TrustStore, crl: &CrlStore| envelope.verify_plan(p, c, a, crl).outcome;
    let o = plan(&p0, &cache, &anchors, &empty);
    parts.push((o == PlanOutcome::Verifiable, format!("valid chain: {o:?}")));
    let o = plan(&p0, &cache, &anchors, &crl);
    parts.push((o == PlanOutcome::Reject(RejectReason::Revoked), format!("revoked device, its period: {o:?}")));
    let o = plan(&p1, &cache, &anchors, &crl);
    parts.push((o == PlanOutcome::Verifiable, format!("other device under the same CRL: {o:?}")));
    // Period 8: the same device's seeds stepped forward one period.
    let p8 = sign(&mut ctx, &mut Real::new(), &pki8.entities[0].signer, 1_000_000_000);
    let o = plan(&p8, &cache8, &anchors8, &crl);
    parts.push((o == PlanOutcome::Reject(RejectReason::Revoked), format!("revoked device, next period: {o:?}")));
    let lv = dev.linkage_value_for(9, 3);
    let later = crl.revokes_linkage(9, 3, lv);
    parts.push((later, format!("linkage value of period 9 matched: {later}")));
    // Expired: generated after the 10,140-minute validity.
    let p_late = sign(&mut ctx, &mut backend, &pki.entities[1].signer, 10_141 * 60 * 1_000_000_000);
    let o = plan(&p_late, &cache, &anchors, &empty);
    parts.push((o == PlanOutcome::Reject(RejectReason::OutsideValidityPeriod), format!("expired: {o:?}")));
    Outcome::all(parts)
}

fn butterfly_identity(mode: Mode) -> Outcome {
    let seed: Vec<u8> = (0..64u8).map(|b| b.wrapping_mul(37).wrapping_add(11)).collect();
    let cat = Caterpillar::from_seed(&seed).expect("64 bytes");
    let a_pub = cat.signing_public();
    let mut items = Vec::new();
    for (i, j) in [(0u32, 0u32), (0, 1), (1, 0), (3, 7), (7, 19), (52, 0), (100, 3), (155, 19), (2, 2), (9, 9), (40, 11), (77, 5)] {
        let f = butterfly::f1(cat.ck(), i, j);
        let (b, _q) = butterfly::ra_cocoon_keys(&a_pub, &cat.encryption_public(), cat.ck(), cat.ek(), i, j);
        let c = ec::scalar_from_be_mod_n(&v2xw_core::hash::sha256(format!("c {i} {j}").as_bytes()));
        let (certified, _big_c) = butterfly::pca_certify_explicit(&b, &c);
        let d = if mode.faulted() {
            // a + f only.
            *cat.a() + f
        } else {
            cat.signing_private(i, j, &c)
        };
        items.push(json!({
            "A": hex(&a_pub.compressed().expect("point")),
            "f": hex(&ec::scalar_to_be32(&f)),
            "B": hex(&b.compressed().expect("point")),
            "c": hex(&ec::scalar_to_be32(&c)),
            "certified": hex(&certified.compressed().expect("point")),
            "d": hex(&ec::scalar_to_be32(&d)),
        }));
    }
    let out = match crate::python("security.py", &json!({"op": "butterfly", "items": items})) {
        Ok(v) => v,
        Err(e) => return Outcome::skip(e),
    };
    let rows = out["items"].as_array().cloned().unwrap_or_default();
    let mut bad = Vec::new();
    for (k, r) in rows.iter().enumerate() {
        for key in ["on_curve", "cocoon", "certified", "private"] {
            if r.get(key).and_then(Value::as_bool) != Some(true) {
                bad.push(format!("row {k}: {key}"));
            }
        }
    }
    Outcome::judge(
        bad.is_empty() && rows.len() == 12,
        if bad.is_empty() { "12 expansions: B = A + f·G, certified = B + c·G, d·G = certified".to_string() } else { format!("{} failures: {}", bad.len(), bad.iter().take(4).cloned().collect::<Vec<_>>().join(", ")) },
    )
}
