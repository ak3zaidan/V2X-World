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
//! * SC-P10.1: `Phase2::provision` runs a joiner's pre-run exchange at 0 instead of
//!   `aged.valid_from.min(now)` — the RA refuses it (`EnrolmentExpired`), the joiner is
//!   counted unprovisioned and
//!   `a_vehicle_joining_after_a_period_boundary_holds_that_periods_pseudonyms` fails.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use v2xw_core::ctx::OwnedRecord;
use v2xw_core::ids::NodeId;
use v2xw_core::time::SimTime;
use v2xw_engine::{Engine, MemoryRecorder, RunRecorder, Scenario};
use v2xw_msg::sec_types::ieee1609_dot2::{CertificateId, CertificateType, IssuerIdentifier};
use v2xw_sec::crypto::{CryptoBackend, SigToken};
use v2xw_sec::envelope::{Envelope, ParsedSigner, SecurityEnvelopeInfo};
use v2xw_world::model::LaneKind;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// A recorder that keeps the frame taps (the SPDU each node actually signed).
struct Tapping {
    inner: MemoryRecorder,
    taps: BTreeMap<u64, (NodeId, Vec<u8>)>,
    /// When each node's first tapped frame went out.
    first_tx: BTreeMap<NodeId, SimTime>,
}

impl RunRecorder for Tapping {
    fn write(&mut self, at: SimTime, record: &OwnedRecord) {
        self.inner.write(at, record);
    }
    fn write_wire_frame(&mut self, frame: &v2xw_record::wire::Frame) {
        self.inner.write_wire_frame(frame);
    }
    fn tap_frame(&mut self, at: SimTime, node: NodeId, msg: u64, spdu: &[u8]) {
        self.first_tx.entry(node).or_insert(at);
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
    first_tx: BTreeMap<NodeId, SimTime>,
    /// Per node (ids are handed out in creation order), how many of its due messages it
    /// could not sign for want of a credential.
    uncredentialed: BTreeMap<NodeId, u64>,
}

fn tapped(s: Scenario) -> Tapped {
    let mut engine = Engine::build(s, "").expect("builds");
    let (aca, scms_aca) = engine.credential_issuer();
    let acas = engine.credential_issuers();
    let wall = engine.wall_clock();
    let mut rec = Tapping {
        inner: MemoryRecorder::new(),
        taps: BTreeMap::new(),
        first_tx: BTreeMap::new(),
    };
    let report = engine.run(&mut rec).expect("runs");
    let uncredentialed = (0..report.nodes_created)
        .map(|k| {
            let id = NodeId::new(u32::try_from(k).expect("fits"));
            (id, engine.node_tx_uncredentialed(id))
        })
        .collect();
    Tapped {
        report,
        first_tx: rec.first_tx,
        uncredentialed,
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

/// `credential-lifecycle.yaml` (60 s i-periods, two held, a top-up when one is left) with
/// `n` vehicles departing at t = 0 and one more departing at `join_s`, each on its own
/// driving lane and bound for the farthest lane its origin can reach. The joiner is the
/// node created last.
fn lifecycle_fleet(n: usize, join_s: f64, duration_s: f64) -> Scenario {
    let mut s = load("credential-lifecycle.yaml", duration_s);
    let world = v2xw_engine::wiring::build_world(&s).expect("world");
    let roads = &world.roads;
    let driving: Vec<_> = roads
        .lanes()
        .iter()
        .filter(|l| l.kind == LaneKind::Driving && l.junction.is_none() && l.length_m > 40.0)
        .collect();

    let mid = |l: &v2xw_world::model::Lane| l.centreline[l.centreline.len() / 2];
    let dist = |a: &v2xw_world::model::Lane, b: &v2xw_world::model::Lane| {
        (mid(a).x - mid(b).x).hypot(mid(a).y - mid(b).y)
    };
    // An origin from which no destination can be reached (an edge lane) is skipped.
    let pairs: Vec<_> = driving
        .iter()
        .filter_map(|o| {
            let mut seen = BTreeSet::from([o.id]);
            let mut queue = VecDeque::from([o.id]);
            while let Some(l) = queue.pop_front() {
                for c in roads.successors(l) {
                    for next in [c.via, Some(c.to_lane)].into_iter().flatten() {
                        if seen.insert(next) {
                            queue.push_back(next);
                        }
                    }
                }
            }
            driving
                .iter()
                .filter(|l| {
                    l.id != o.id && seen.contains(&l.id) && !roads.successors(l.id).is_empty()
                })
                .max_by(|a, b| dist(a, o).total_cmp(&dist(b, o)).then(b.id.cmp(&a.id)))
                .map(|dest| (o.id, dest.id))
        })
        .take(n + 1)
        .collect();
    assert_eq!(pairs.len(), n + 1, "the grid has too few routable lanes");
    let trips: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(i, (o, d))| {
            let depart = if i == n { join_s } else { 0.0 };
            serde_json::json!({"id": i, "depart_s": depart, "origin": o, "destination": d})
        })
        .collect();
    let body = serde_json::json!({
        "schema": "v2xw/trips/1",
        "world_hash": v2xw_world::hash::content_hash_hex(&world),
        "source": {"test": "credential_handoff"},
        "trips": trips,
    })
    .to_string();
    // Named by content and published by rename: the target's temporary directory is
    // shared by this binary's tests and by every worktree building into it.
    let name = format!(
        "credential-handoff-{}.json",
        &v2xw_core::hash::sha256_hex(body.as_bytes())[..16]
    );
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if std::fs::read(&path).ok().as_deref() != Some(body.as_bytes()) {
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&tmp, &body).expect("writes the trip file");
        std::fs::rename(&tmp, &path).expect("publishes the trip file");
    }
    s.actors.vehicles.demand.kind = "mobility/demand/trips".into();
    s.actors.vehicles.demand.rate_veh_per_h = None;
    s.actors.vehicles.demand.params = serde_json::json!({ "path": path });
    s
}

/// Every `(signer digest, certificate)` the run put on the air, and each node's frames as
/// the certificate it signed under (a digest-signed frame resolved through a certificate
/// an earlier frame carried; unresolved ones are left out).
fn frames_by_node(t: &Tapped) -> BTreeMap<NodeId, Vec<v2xw_msg::sec_types::Certificate>> {
    let env = Envelope::ieee1609(t.wall);
    let mut certs: BTreeMap<[u8; 8], v2xw_msg::sec_types::Certificate> = BTreeMap::new();
    let mut out: BTreeMap<NodeId, Vec<_>> = BTreeMap::new();
    for (node, spdu) in t.taps.values() {
        let parsed = env.parse(spdu).expect("every tapped SPDU parses");
        let cert = match &parsed.signer {
            ParsedSigner::Certificate {
                certificate,
                digest,
            } => {
                let c = (**certificate).clone();
                certs.insert(v2xw_sec::hashedid::id8_bytes(digest), c.clone());
                c
            }
            ParsedSigner::Digest(d) => match certs.get(&v2xw_sec::hashedid::id8_bytes(d)) {
                Some(c) => c.clone(),
                None => continue,
            },
            ParsedSigner::SelfSigned => panic!("node {node:?} signed self"),
        };
        out.entry(*node).or_default().push(cert);
    }
    out
}

/// The `cracaId` a factory-issued (bootstrap stand-in) certificate carries: its own ACA's
/// HashedId3, where an SCMS batch names the CRACA (the ICA since WP-B4).
fn factory_cracas(t: &Tapped) -> Vec<v2xw_msg::sec_types::HashedId3> {
    std::iter::once(&t.aca)
        .chain(t.acas.iter())
        .map(|a| {
            let coer = v2xw_sec::cert::encode(a).expect("encodes");
            v2xw_sec::hashed_id3(&coer)
        })
        .collect()
}

/// SC-P10.1: a vehicle that joins two seconds after an i-period boundary (60 s i-periods)
/// holds that period's pseudonyms, signed by its provider's ACA, from its first frame.
///
/// The enrolment lifetime is cut to 60 s so the joiner's enrolment certificate (its age
/// drawn uniformly over the lifetime) began inside the run, after t = 0: its pre-run
/// exchange runs when that certificate began (`Phase2::provision`,
/// `aged.valid_from.min(now)`). At t = 0 the RA refuses the request as signed by a
/// certificate not yet valid and the vehicle joins with no certificate.
///
/// The defect the 2026-09-30 note in `Phase2::provision` names ("vehicles joining a few
/// seconds after a period boundary get no certificate", 4 of 55 at 1,500 veh/h) was the
/// RA's request shuffle: the joiner's pre-run job joined the run's queue, whose timer was
/// armed on the run's kernel. `ScmsRun::preload_at` has run the pool on an empty shuffle
/// since `750e7956`, and in-run top-ups no longer enter that shuffle at all (`ra-maxReqs`
/// = 1: a top-off is `.info` plus a download), so no default run reaches it now:
/// `credential-lifecycle` for 130 s at 3,000 and 6,000 veh/h left 0 vehicles unprovisioned.
#[test]
fn a_vehicle_joining_after_a_period_boundary_holds_that_periods_pseudonyms() {
    let mut s = lifecycle_fleet(6, 62.0, 70.0);
    if let Some(p) = s.security.protocol.as_mut() {
        p.params["enrolment_lifetime_s"] = 60.into();
        p.params["reenrol_lead_s"] = 20.into();
    }
    let t = tapped(s);
    assert!(t.scms_aca, "an SCMS run issues under its own ACA");
    assert_eq!(
        t.report.phase2.vehicles_unprovisioned, 0,
        "a vehicle joined without a certificate"
    );
    // The joiner is the one node whose first frame went out after it joined at 62 s.
    let late: Vec<_> = t
        .first_tx
        .iter()
        .filter(|(_, at)| **at >= 62_000_000_000)
        .collect();
    let [(&joiner, &first)] = late[..] else {
        panic!("one joiner expected, first frames {:?}", t.first_tx);
    };
    assert!(
        first < 65_000_000_000,
        "the joiner first transmitted at {first} ns"
    );
    assert_eq!(
        t.uncredentialed.get(&joiner).copied(),
        Some(0),
        "the joiner had messages it could not sign"
    );
    let factory = factory_cracas(&t);
    let frames = frames_by_node(&t);
    let signed = frames.get(&joiner).map_or(&[][..], Vec::as_slice);
    assert!(
        !signed.is_empty(),
        "no frame of the joiner could be resolved"
    );
    // Period 1 begins at 60 s; the joiner's certificates are that period's, not period
    // 0's (which began at t = 0) and not the stand-in.
    let (lo, hi) = (
        t.wall.time32(59_000_000_000).expect("time32"),
        t.wall.time32(62_000_000_000).expect("time32"),
    );
    for cert in signed {
        assert!(
            matches!(cert.0.to_be_signed.id, CertificateId::linkageData(_)),
            "the joiner signed under {:?}",
            cert.0.to_be_signed.id
        );
        assert!(
            !factory.contains(&cert.0.to_be_signed.craca_id),
            "the joiner signed under a factory (stand-in) certificate"
        );
        let start = cert.0.to_be_signed.validity_period.start.0.0;
        assert!(
            (lo..=hi).contains(&start),
            "the joiner's certificate starts at {start}, not in period 1 [{lo}, {hi}]"
        );
    }
}

/// SC-P10.2: under an SCMS a vehicle the SCMS did not provision cannot sign. It joins with
/// an empty store (no bootstrap stand-in), sends no signed frame and counts its due
/// messages as `tx_uncredentialed` (1609.2: an unprovisioned device has no signing
/// credential).
///
/// The fleet of the SC-P10.1 test with both providers' ACA offline for the whole run. The
/// vehicles on the road from t = 0 hold the pool they were issued before the run (its
/// faults do not apply to it), which lasts past the run's 70 s. The joiner's enrolment
/// began inside the run, so it asked for its pool then, with the ACA down and never back
/// (`ScmsRun::preload_at`): it is the one vehicle left unprovisioned.
#[test]
fn a_vehicle_whose_provisioning_fails_cannot_sign() {
    let mut s = lifecycle_fleet(6, 62.0, 70.0);
    if let Some(p) = s.security.protocol.as_mut() {
        p.params["enrolment_lifetime_s"] = 60.into();
        p.params["reenrol_lead_s"] = 20.into();
    }
    // The fleet is split between the deployment's two providers; the joiner's is either.
    s.actors.backend.failures = ["pca", "provider-b/pca"]
        .into_iter()
        .map(|node| v2xw_engine::scenario::schema::BackendFailureSpec {
            node: node.to_string(),
            mode: "offline".to_string(),
            from_s: 0.0,
            to_s: None,
            extra_ms: None,
        })
        .collect();
    let t = tapped(s);
    assert!(t.scms_aca, "an SCMS run issues under its own ACA");
    assert_eq!(
        t.report.phase2.vehicles_unprovisioned, 1,
        "the joiner, and only it, is left unprovisioned"
    );
    // No pseudonym on the air is a factory-issued stand-in.
    let factory = factory_cracas(&t);
    for (node, certs) in frames_by_node(&t) {
        for cert in certs {
            if matches!(cert.0.to_be_signed.id, CertificateId::linkageData(_)) {
                assert!(
                    !factory.contains(&cert.0.to_be_signed.craca_id),
                    "node {node:?} signed under a factory (stand-in) certificate"
                );
            }
        }
    }
    // The joiner sent nothing: every node that transmitted did so from the start.
    assert!(
        t.first_tx.values().all(|at| *at < 62_000_000_000),
        "a node first transmitted after the join: {:?}",
        t.first_tx
    );
    let unable: Vec<_> = t.uncredentialed.iter().filter(|(_, n)| **n > 0).collect();
    assert_eq!(
        unable.len(),
        1,
        "one node had due messages it could not sign: {unable:?}"
    );
    assert!(
        !t.first_tx.contains_key(unable[0].0),
        "the node that could not sign transmitted"
    );
}
