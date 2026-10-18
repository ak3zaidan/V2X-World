//! WP-C1: the credentials a node signs with are the ones the SCMS issued.
//!
//! # Faults injected to prove these checks can fail
//!
//! * A `fn device_linkage` (or a `struct Issuer`, or an `RxFrame.claimed_linkage`) added
//!   back to `v2xw-node/src/secure.rs` — `no_stand_in_issuer_or_side_channel_remains`
//!   fails naming the file.
//! * `ImplicitKey::new` in `creds_of_device` given `imp.r` in place of `d` — every
//!   installation is refused, `credentials_refused > 0` and
//!   `vehicles_sign_under_the_runs_aca` fails.
//! * `Real::import_reconstructed` (the independent verifier) given the wrong issuer — the
//!   signature check in the same test fails (see the sec crate's own equivalence test).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use v2xw_core::ctx::OwnedRecord;
use v2xw_core::ids::NodeId;
use v2xw_core::time::SimTime;
use v2xw_engine::{Engine, MemoryRecorder, RunRecorder, Scenario};
use v2xw_msg::sec_types::ieee1609_dot2::{CertificateId, CertificateType, IssuerIdentifier};
use v2xw_sec::crypto::{CryptoBackend, SigToken};
use v2xw_sec::envelope::{Envelope, ParsedSigner, SecurityEnvelopeInfo};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// A recorder that keeps the frame taps (the SPDU each node actually signed).
struct Tapping {
    inner: MemoryRecorder,
    taps: BTreeMap<u64, (NodeId, Vec<u8>)>,
}

impl RunRecorder for Tapping {
    fn write(&mut self, at: SimTime, record: &OwnedRecord) {
        self.inner.write(at, record);
    }
    fn write_wire_frame(&mut self, frame: &v2xw_record::wire::Frame) {
        self.inner.write_wire_frame(frame);
    }
    fn tap_frame(&mut self, _at: SimTime, node: NodeId, msg: u64, spdu: &[u8]) {
        self.taps.insert(msg, (node, spdu.to_vec()));
    }
}

fn load(name: &str, duration_s: f64) -> Scenario {
    let mut s = Scenario::load(root().join("scenarios").join(name)).expect("loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root().join(&*path).to_string_lossy().into_owned();
    }
    s.time.duration_s = duration_s;
    s.threats.attackers.clear();
    s
}

struct Tapped {
    report: v2xw_engine::RunReport,
    taps: BTreeMap<u64, (NodeId, Vec<u8>)>,
    aca: std::sync::Arc<v2xw_msg::sec_types::Certificate>,
    /// Every provider's ACA (WP-D1: each provider certifies its share of the fleet).
    acas: Vec<std::sync::Arc<v2xw_msg::sec_types::Certificate>>,
    scms_aca: bool,
    wall: v2xw_core::time::WallClock,
}

fn tapped(s: Scenario) -> Tapped {
    let mut engine = Engine::build(s, "").expect("builds");
    let (aca, scms_aca) = engine.credential_issuer();
    let acas = engine.credential_issuers();
    let wall = engine.wall_clock();
    let mut rec = Tapping {
        inner: MemoryRecorder::new(),
        taps: BTreeMap::new(),
    };
    let report = engine.run(&mut rec).expect("runs");
    Tapped {
        report,
        taps: rec.taps,
        aca,
        acas,
        scms_aca,
        wall,
    }
}

/// The acceptance grep: the per-node stand-in issuer, the node-id linkage construction
/// and the engine's linkage side channel are gone from the node and the engine.
#[test]
fn no_stand_in_issuer_or_side_channel_remains() {
    let forbidden = [
        "struct Issuer",
        "fn device_linkage",
        "device_linkage(",
        "claimed_linkage",
        "claimed_cert_period",
        "fn provision_all",
    ];
    let mut hits = Vec::new();
    for dir in ["crates/v2xw-node/src", "crates/v2xw-engine/src"] {
        let mut stack = vec![root().join(dir)];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).expect("reads") {
                let p = e.expect("entry").path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let text = std::fs::read_to_string(&p).expect("reads");
                    for f in forbidden {
                        if text.contains(f) {
                            hits.push(format!("{}: {f}", p.display()));
                        }
                    }
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "stand-in credential code remains: {hits:#?}"
    );
}

/// Every vehicle frame of an SCMS run is signed under an implicit certificate its provider's
/// ACA issued, with a linkage value, and its signature verifies under the key an
/// *independent* verifier reconstructs from that certificate and the ACA certificate
/// alone. Every roadside unit signs under an RSU application certificate (no linkage
/// data, CRL series 3) from the same ACA, when one transmits inside the window.
#[test]
fn vehicles_sign_under_the_runs_aca() {
    // Real crypto: the signatures on the air are ECDSA, so a verifier outside the run
    // can check them.
    let mut s = load("phase2-manhattan.yaml", 4.0);
    s.security.crypto_mode = serde_json::from_value(serde_json::json!("real")).expect("mode");
    let t = tapped(s);
    assert!(t.scms_aca, "an SCMS run issues under its own ACA");
    assert_eq!(t.report.credentials_refused, 0);
    assert!(!t.taps.is_empty(), "the run transmitted nothing");
    let env = Envelope::ieee1609(t.wall);
    // Each provider's ACA, by its HashedId8 (WP-D1: the fleet is split between the
    // deployment's providers, and each vehicle signs under its own provider's ACA).
    let acas: Vec<_> = std::iter::once(&t.aca)
        .chain(t.acas.iter())
        .map(|a| {
            let coer = v2xw_sec::cert::encode(a).expect("encodes");
            (v2xw_sec::hashed_id8(&coer), coer, std::sync::Arc::clone(a))
        })
        .collect();
    let mut certs: BTreeMap<[u8; 8], v2xw_msg::sec_types::Certificate> = BTreeMap::new();
    let mut verifier = v2xw_sec::Real::new();
    let mut ctx = v2xw_sec::testctx::TestCtx::new(1);
    let mut keys = BTreeMap::new();
    let (mut checked, mut pseudonyms, mut rsu_certs) = (0u32, BTreeSet::new(), BTreeSet::new());
    for (node, spdu) in t.taps.values() {
        let parsed = env.parse(spdu).expect("every tapped SPDU parses");
        let (digest, cert) = match &parsed.signer {
            ParsedSigner::Certificate {
                certificate,
                digest,
            } => {
                let c = (**certificate).clone();
                certs.insert(v2xw_sec::hashedid::id8_bytes(digest), c.clone());
                (v2xw_sec::hashedid::id8_bytes(digest), c)
            }
            ParsedSigner::Digest(d) => match certs.get(&v2xw_sec::hashedid::id8_bytes(d)) {
                Some(c) => (v2xw_sec::hashedid::id8_bytes(d), c.clone()),
                None => continue,
            },
            ParsedSigner::SelfSigned => panic!("node {node:?} signed self"),
        };
        assert_eq!(cert.0.r_type, CertificateType::implicit, "node {node:?}");
        let (aca_coer, aca) = match &cert.0.issuer {
            IssuerIdentifier::sha256AndDigest(d) => acas
                .iter()
                .find(|(id, ..)| id == d)
                .map(|(_, coer, aca)| (coer, aca))
                .unwrap_or_else(|| panic!("issuer {d:?} is none of the run's ACAs")),
            other => panic!("issuer {other:?}"),
        };
        match &cert.0.to_be_signed.id {
            CertificateId::linkageData(_) => {
                assert_eq!(cert.0.to_be_signed.crl_series.0.0, 1);
                // The protocol's certificate, not a factory one: the ACA put the CRACA's
                // cracaId in it (the ICA since WP-B4), where factory issuance puts the
                // ACA's own.
                // (Every vehicle of this run is provisioned before it drives.)
                assert_ne!(
                    cert.0.to_be_signed.craca_id,
                    v2xw_sec::hashed_id3(aca_coer),
                    "node {node:?} signs under a factory certificate, not its SCMS batch"
                );
                pseudonyms.insert(digest);
            }
            CertificateId::none(()) => {
                assert_eq!(cert.0.to_be_signed.crl_series.0.0, 3, "RSU application");
                rsu_certs.insert(digest);
            }
            other => panic!("certificate id {other:?}"),
        }
        // An independent verifier: its own backend, the ACA certificate and the SPDU.
        if checked < 300 || !keys.contains_key(&digest) {
            let pk = *keys.entry(digest).or_insert_with(|| {
                verifier
                    .import_reconstructed(
                        v2xw_sec::PrimitiveId::ECDSA_P256_SHA256,
                        NodeId::new(0),
                        &cert,
                        aca,
                    )
                    .expect("reconstructs")
            });
            let token = SigToken {
                primitive: v2xw_sec::PrimitiveId::ECDSA_P256_SHA256,
                bytes: parsed.signature.clone(),
            };
            assert!(
                verifier.verify_prehashed(
                    &mut ctx,
                    &pk,
                    &parsed.signing_digest(&v2xw_sec::cert::encode(&cert).expect("encodes")),
                    &token
                ),
                "node {node:?}: signature does not verify under e·P_U + Q_ACA"
            );
            checked += 1;
        }
    }
    let sizes: BTreeSet<usize> = certs
        .values()
        .map(|c| v2xw_sec::cert::encode(c).expect("encodes").len())
        .collect();
    println!(
        "{checked} SPDUs verified independently; {} pseudonym and {} RSU certificates on \
         the air; encoded sizes {sizes:?} B",
        pseudonyms.len(),
        rsu_certs.len()
    );
    assert!(checked > 0 && !pseudonyms.is_empty(), "{checked} checked");
}

/// I-S1 with the hand-off in place: the same run in `real` and `modelled` crypto puts
/// the same number of octets on the air for every message, and its receivers reach the
/// same verdicts.
#[test]
fn real_and_modelled_runs_agree_with_scms_credentials() {
    let run = |mode: &str| {
        let mut s = load("phase1-grid.yaml", 2.0);
        s.security.crypto_mode = serde_json::from_value(serde_json::json!(mode)).expect("mode");
        tapped(s)
    };
    let real = run("real");
    let modelled = run("modeled");
    assert!(
        !real.scms_aca && !modelled.scms_aca,
        "no SCMS in this scenario"
    );
    let sizes = |t: &Tapped| -> Vec<(u64, usize)> {
        t.taps.iter().map(|(m, (_, b))| (*m, b.len())).collect()
    };
    assert!(!real.taps.is_empty());
    assert_eq!(sizes(&real), sizes(&modelled));
    assert_eq!(real.report.credentials_refused, 0);
    // Every count the run reports, receptions and verdicts included, is mode-independent.
    assert_eq!(real.report, modelled.report);
}
