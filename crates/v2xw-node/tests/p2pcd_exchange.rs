//! WP-C3 acceptance: a new ACA certificate propagates through a fleet by P2PCD
//! (00 §1 row 38: "count the frames and the time to learn it").
//!
//! The PKI is the run's real one (`ScmsPki`). A second ACA (`aca.provider-b`) is issued
//! under the ICA after the fleet's chain files were made, so no fleet member holds it. A
//! newcomer whose pseudonym that ACA issued joins and sends BSMs at 10 Hz, attaching its
//! certificate on the J2945/1 450 ms cadence. Every station is a `NodeSecurity` with the
//! receiver validation of WP-C2 installed; the channel is a 1 ms-step broadcast medium in
//! which every station hears every frame (one collision domain, no loss), so the numbers
//! are the protocol's own and not the radio's.
//!
//! Out-of-band (the US default): the fleet asks with `p2pcdLearningRequest`, the newcomer
//! answers with a learning-response PDU after its back-off, everyone learns the ACA from
//! it (validated to the root), and the newcomer's next certificate-bearing BSM verifies.
//! Inline (ETSI): the request rides in `inlineP2pcdRequest`, the answer in the newcomer's
//! next BSM's `requestedCertificate`. A forged response is refused and leaves the request
//! running.

use std::collections::BTreeMap;
use std::sync::Arc;

use v2xw_core::ids::NodeId;
use v2xw_core::rng::RngRegistry;
use v2xw_core::time::{NS_PER_MS, WallClock};
use v2xw_msg::MsgType;
use v2xw_msg::sec_types::{Certificate, HashedId3};
use v2xw_node::ctx::NodeRuntimeCtx;
use v2xw_node::secure::{CryptoMode, NodeSecurity, PSID_SAFETY, SpduVerdict};
use v2xw_proto::scms::pdu::{Authority, ScmsPki};
use v2xw_sec::cert::{self, CertSpec, HolderId};
use v2xw_sec::credential::{ImplicitIssuer, ImplicitKey};
use v2xw_sec::envelope::{EnvelopeProfile, ParsedSigner, SignerIdChoice};
use v2xw_sec::linkage::{DeviceLinkageContext, LaId, LinkageSeed};
use v2xw_sec::p2pcd::{P2pcdFlavor, P2pcdParams};
use v2xw_sec::{ec, hashedid};

const UNIX0: i64 = 1_767_225_600;

fn wall() -> WallClock {
    WallClock::new(UNIX0)
}

fn t0() -> u32 {
    wall().time32(0).expect("time32")
}

fn device() -> DeviceLinkageContext {
    DeviceLinkageContext::new(
        LaId(1),
        LaId(2),
        LinkageSeed::new([0x11; 16]),
        LinkageSeed::new([0x22; 16]),
    )
}

fn pseudonym(pki: &ScmsPki, aca: &Authority, seed: u8) -> ImplicitKey {
    let iss =
        ImplicitIssuer::new(aca.cert.clone(), aca.private.expect("P-256 CA")).expect("issuer");
    let id8 = hashedid::hashed_id8(&cert::encode(&aca.cert).expect("enc"));
    let mut spec = CertSpec::pseudonym(
        id8,
        HolderId::Linkage {
            i_cert: 0,
            linkage_value: device().linkage_value_for(0, u32::from(seed)),
        },
        t0() - 60,
        PSID_SAFETY,
    );
    spec.craca_id = HashedId3(rasn::types::FixedOctetString::from(pki.craca_id()));
    spec.crl_series = 1;
    spec.valid_for = v2xw_msg::sec_types::Ieee1609Duration::hours(
        v2xw_msg::sec_types::ieee1609_dot2_base_types::Uint16(169),
    );
    let mut n = 0u8;
    iss.issue_spec(&spec, &ec::scalar_from_be_mod_n(&[seed; 32]), || {
        n += 1;
        let mut k = [seed ^ 0x5A; 32];
        k[1] = n;
        ec::scalar_from_be_mod_n(&k)
    })
    .expect("issues")
}

/// The PKI with the new ACA (`extra[0]`), issued after the fleet's chain files.
fn pki_with_new_aca() -> ScmsPki {
    let mut pki = ScmsPki::new(7, t0() - 31_556_952).expect("PKI");
    let ssp = v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::SecurityMgmtSsp::aca(
        v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::AcaSsp::new(
            v2xw_msg::sec_types::ieee1609_dot2_base_types::Uint8(2),
        ),
    );
    pki.issue_component(11, "aca.provider-b", &ssp, false, t0() - 31_556_952)
        .expect("new ACA");
    pki
}

struct Station {
    id: NodeId,
    sec: NodeSecurity,
    phase_ms: u64,
    peers: BTreeMap<[u8; 8], Arc<Certificate>>,
}

fn station(
    mode: CryptoMode,
    pki: &ScmsPki,
    n: u8,
    aca: &Authority,
    flavor: P2pcdFlavor,
    with_new_aca: bool,
) -> Station {
    let profile = if flavor == P2pcdFlavor::Inline {
        EnvelopeProfile::EtsiTs103097
    } else {
        EnvelopeProfile::Ieee1609Dot2
    };
    let mut sec = NodeSecurity::new(wall(), mode, PSID_SAFETY)
        .with_profile(profile, wall())
        .with_p2pcd(P2pcdParams::recommended(flavor));
    let mut cas: Vec<Arc<Certificate>> =
        pki.all().iter().map(|a| Arc::new(a.cert.clone())).collect();
    if with_new_aca {
        cas.push(Arc::new(pki.extra[0].cert.clone()));
    }
    sec.install_trust(&[Arc::new(pki.root.cert.clone())], &cas);
    let id = NodeId::new(u32::from(n) + 1);
    sec.install(id, 0, 0, &pseudonym(pki, aca, n + 1))
        .expect("installs");
    assert!(sec.set_active(0, 0));
    Station {
        id,
        sec,
        // Spread the 10 Hz phases over the 100 ms period.
        phase_ms: (u64::from(n) * 37) % 100,
        peers: BTreeMap::new(),
    }
}

#[derive(Debug, Default)]
struct Outcome {
    /// Signed BSMs on the air.
    bsms: u64,
    /// Of which carried a P2PCD request field.
    request_frames: u64,
    /// Of which carried `requestedCertificate`.
    inline_answers: u64,
    /// Learning-response PDUs on the air.
    response_pdus: u64,
    /// Octets of those PDUs.
    response_octets: u64,
    /// ms from the newcomer's first BSM to each fleet member's first verified BSM from it.
    learn_ms: Vec<u64>,
    /// Newcomer BSMs the fleet could not verify (unknown issuer).
    unverifiable: u64,
}

/// Runs the fleet (`fleet` members holding the old ACA, one newcomer under the new one)
/// for `ms` milliseconds. `forge` broadcasts a forged learning response at 5 ms.
fn run(
    mode: CryptoMode,
    flavor: P2pcdFlavor,
    fleet: u8,
    ms: u64,
    forge: bool,
) -> (Outcome, Vec<Station>) {
    run_n(mode, flavor, fleet, 1, ms, forge)
}

/// [`run`] with `newcomers` stations under the new ACA (all potential responders).
fn run_n(
    mode: CryptoMode,
    flavor: P2pcdFlavor,
    fleet: u8,
    newcomers: u8,
    ms: u64,
    forge: bool,
) -> (Outcome, Vec<Station>) {
    let pki = pki_with_new_aca();
    let mut st: Vec<Station> = (0..fleet)
        .map(|n| station(mode, &pki, n, &pki.aca, flavor, false))
        .collect();
    for k in 0..newcomers {
        st.push(station(mode, &pki, fleet + k, &pki.extra[0], flavor, true));
    }
    let newcomer = usize::from(fleet);
    // The newcomers have been driving (and signing) elsewhere before they arrive, so the
    // chain of the certificate they sign with is "currently used" (§8.2.4.2.2 a)1)) from
    // the start; these warm-up SPDUs are not heard by the fleet.
    let warm = RngRegistry::new(41);
    for s in st.iter_mut().skip(newcomer) {
        let mut ctx = NodeRuntimeCtx::new(0, &warm);
        let _ = s
            .sec
            .sign(
                &mut ctx,
                MsgType::Bsm,
                b"elsewhere",
                SignerIdChoice::Digest,
                None,
            )
            .expect("signs");
    }
    let reg = RngRegistry::new(42);
    let mut out = Outcome::default();
    let mut first_tx: Option<u64> = None;
    let mut learned_at: BTreeMap<usize, u64> = BTreeMap::new();
    for t in 0..ms {
        let now = t * NS_PER_MS;
        if forge && t == 5 {
            // A CA certificate that does not chain: the new ACA's tbs re-signed by nobody
            // (its signature bytes altered), so the ICA's signature fails.
            let mut fake = pki.extra[0].cert.clone();
            if let Some(v2xw_msg::sec_types::Signature::ecdsaNistP256Signature(s)) =
                fake.0.signature.as_mut()
            {
                let mut b = s.s_sig.to_vec();
                b[0] ^= 1;
                s.s_sig = b.into();
            }
            let pdu = v2xw_sec::p2pcd::encode_learning_response(&[fake]).expect("enc");
            for s in st.iter_mut() {
                let _ = s.sec.p2pcd_receive_response(&pdu);
            }
        }
        for i in 0..st.len() {
            if (t + st[i].phase_ms) % 100 != 0 {
                continue;
            }
            let mut ctx = NodeRuntimeCtx::new(now, &reg);
            let sid = st[i].sec.signer_id_for(MsgType::Bsm, now);
            let (frame, pdu) = st[i]
                .sec
                .sign(&mut ctx, MsgType::Bsm, b"bsm-payload", sid, None)
                .expect("signs");
            if pdu.signer_id == SignerIdChoice::Certificate {
                st[i].sec.note_certificate_attached(MsgType::Bsm, now);
            }
            if i >= newcomer && first_tx.is_none() {
                first_tx = Some(t);
            }
            out.bsms += 1;
            for j in 0..st.len() {
                if j == i {
                    continue;
                }
                let rx = &mut st[j];
                let from_new = i >= newcomer;
                let to_new = j >= newcomer;
                let parsed = rx.sec.parse(&frame.spdu).expect("parses");
                if j == 0 {
                    if parsed.p2pcd_learning_request.is_some()
                        || !parsed.inline_p2pcd_request.is_empty()
                    {
                        out.request_frames += 1;
                    }
                    if parsed.requested_certificate.is_some() {
                        out.inline_answers += 1;
                    }
                } else if i == 0 {
                    if parsed.p2pcd_learning_request.is_some()
                        || !parsed.inline_p2pcd_request.is_empty()
                    {
                        out.request_frames += 1;
                    }
                }
                let cert = match &parsed.signer {
                    ParsedSigner::Certificate {
                        certificate,
                        digest,
                    } => {
                        let c = Arc::new((**certificate).clone());
                        rx.peers.insert(hashedid::id8_bytes(digest), Arc::clone(&c));
                        Some(c)
                    }
                    ParsedSigner::Digest(d) => rx.peers.get(&hashedid::id8_bytes(d)).cloned(),
                    ParsedSigner::SelfSigned => None,
                };
                let known = cert.is_some();
                rx.sec.p2pcd_preprocess(&mut ctx, rx.id, &parsed, known);
                if let Some(c) = cert {
                    let v = rx.sec.verify_parsed(&mut ctx, &parsed, &c, rx.id);
                    if from_new && !to_new {
                        match v {
                            SpduVerdict::Valid => {
                                learned_at.entry(j).or_insert(t);
                            }
                            SpduVerdict::Unverifiable => out.unverifiable += 1,
                            other => panic!("newcomer BSM at {t} ms: {other:?}"),
                        }
                    } else if !from_new || to_new {
                        assert_eq!(v, SpduVerdict::Valid, "fleet BSMs always verify");
                    }
                }
            }
        }
        for i in 0..st.len() {
            let pdus = st[i].sec.p2pcd_due(now);
            for pdu in pdus {
                out.response_pdus += 1;
                out.response_octets += pdu.len() as u64;
                for (j, s) in st.iter_mut().enumerate() {
                    if j != i {
                        s.sec
                            .p2pcd_receive_response(&pdu)
                            .expect("a learning response");
                    }
                }
            }
        }
    }
    let t1 = first_tx.expect("newcomer sent");
    out.learn_ms = (0..newcomer)
        .map(|j| learned_at.get(&j).map_or(u64::MAX, |t| t - t1))
        .collect();
    (out, st)
}

#[test]
fn a_new_aca_certificate_propagates_out_of_band() {
    for mode in [CryptoMode::Modeled, CryptoMode::Real] {
        let (o, st) = run(mode, P2pcdFlavor::OutOfBand, 8, 1_500, false);
        let total = |f: &dyn Fn(&v2xw_node::p2pcd::P2pcdStats) -> u64| -> u64 {
            st.iter().map(|s| f(&s.sec.p2pcd().stats)).sum()
        };
        eprintln!(
            "OOB {mode:?}: bsms {} learn_ms {:?} requests {} suppressed(active {}, observed {}) \
             responses {} ({} B) triggered {} threshold-suppressed {} learned {} unverifiable {}",
            o.bsms,
            o.learn_ms,
            total(&|s| s.requests_sent),
            total(&|s| s.requests_suppressed_active),
            total(&|s| s.requests_suppressed_observed),
            o.response_pdus,
            o.response_octets,
            total(&|s| s.responses_triggered),
            total(&|s| s.responses_suppressed_threshold),
            total(&|s| s.learned),
            o.unverifiable,
        );
        // Every fleet member learned the new ACA from the exchange and then verified the
        // newcomer: the first certificate-bearing BSM is unverifiable, the next verifies.
        assert!(o.learn_ms.iter().all(|&t| t < 1_000), "{:?}", o.learn_ms);
        assert_eq!(total(&|s| s.learned), 8, "each member learns the ACA once");
        assert!(o.unverifiable >= 8, "the trigger SPDU taught nobody");
        // Only the newcomer holds the ACA in a chain it signs with, so it alone answers.
        assert_eq!(st[8].sec.p2pcd().stats.responses_sent, o.response_pdus);
        assert!(o.response_pdus >= 1);
        // caCerts = [new ACA, ICA]: the ICA is already held, so it is "known", not learned.
        assert_eq!(total(&|s| s.learn_known), 8 * o.response_pdus);
        // Rate limits: one request goes out, the rest of the fleet hears it and holds back.
        let sent = total(&|s| s.requests_sent);
        assert!(sent >= 1 && sent <= 3, "{sent} requests");
        assert!(total(&|s| s.requests_suppressed_observed) >= 1);
        assert_eq!(o.request_frames > 0, true);
    }
}

#[test]
fn a_new_aca_certificate_propagates_inline() {
    let (o, st) = run(CryptoMode::Modeled, P2pcdFlavor::Inline, 8, 1_500, false);
    let total = |f: &dyn Fn(&v2xw_node::p2pcd::P2pcdStats) -> u64| -> u64 {
        st.iter().map(|s| f(&s.sec.p2pcd().stats)).sum()
    };
    eprintln!(
        "inline: bsms {} learn_ms {:?} inline requests {} entries {} ca answers {} forced {} learned {}",
        o.bsms,
        o.learn_ms,
        total(&|s| s.inline_requests_sent),
        total(&|s| s.inline_entries_sent),
        total(&|s| s.inline_ca_sent),
        total(&|s| s.inline_cert_forced),
        total(&|s| s.learned),
    );
    assert!(o.learn_ms.iter().all(|&t| t < 1_000), "{:?}", o.learn_ms);
    assert_eq!(total(&|s| s.learned), 8);
    assert_eq!(o.response_pdus, 0, "inline sends no separate PDU");
    assert!(total(&|s| s.inline_ca_sent) >= 1);
    assert!(o.inline_answers >= 1);
}

/// Five stations hold the new ACA, so five back-off timers start on one request; the
/// response-count threshold (3) stops the late ones (1609.2a-2017 §8.2.1, Figure 14 c)).
#[test]
fn several_responders_back_off_and_stop_at_the_threshold() {
    let (o, st) = run_n(
        CryptoMode::Modeled,
        P2pcdFlavor::OutOfBand,
        6,
        5,
        1_500,
        false,
    );
    let total = |f: &dyn Fn(&v2xw_node::p2pcd::P2pcdStats) -> u64| -> u64 {
        st.iter().map(|s| f(&s.sec.p2pcd().stats)).sum()
    };
    let triggered = total(&|s| s.responses_triggered);
    let sent = total(&|s| s.responses_sent);
    let held = total(&|s| s.responses_suppressed_threshold);
    eprintln!(
        "5 responders: requests {} triggered {triggered} sent {sent} threshold-held {held} learn_ms {:?}",
        total(&|s| s.requests_sent),
        o.learn_ms
    );
    assert_eq!(sent, o.response_pdus);
    assert_eq!(
        triggered,
        sent + held,
        "every process ends in one of the two"
    );
    assert!(triggered >= 5, "every holder starts a back-off");
    let requests = total(&|s| s.requests_sent);
    assert!(sent <= 3 * requests, "at most the threshold per request");
    assert!(held >= 1, "the threshold silenced someone");
    assert!(o.learn_ms.iter().all(|&t| t < 1_000), "{:?}", o.learn_ms);
}

#[test]
fn a_forged_response_is_refused_and_the_request_goes_on() {
    let (o, st) = run(CryptoMode::Modeled, P2pcdFlavor::OutOfBand, 4, 1_500, true);
    for s in &st[..4] {
        assert_eq!(
            s.sec.p2pcd().stats.learn_refused,
            1,
            "the forged ACA is refused"
        );
    }
    assert!(o.learn_ms.iter().all(|&t| t < 1_000), "{:?}", o.learn_ms);
}

// -------------------------------------------------------------------------------------
// The same exchange through `ObuRuntime`: the receive path preprocesses every SPDU for
// P2PCD, the generated BSM carries the request, and the runtime's P2P entity answers.
// -------------------------------------------------------------------------------------

fn obu(
    pki: &ScmsPki,
    aca: &Authority,
    n: u8,
    with_new_aca: bool,
) -> v2xw_node::runtime::ObuRuntime {
    use v2xw_node::runtime::{NodeConfig, ObuRuntime};
    let node = NodeId::new(100 + u32::from(n));
    let mut rt = ObuRuntime::new(
        node,
        v2xw_node::profiles::get("obu/unex-obu-301-craton2")
            .expect("profile")
            .clone(),
        Box::new(v2xw_node::policy::VerifyAll::new()),
        NodeConfig {
            services: v2xw_node::generate::ServiceSet::SAE,
            wall: wall(),
            ..NodeConfig::default()
        },
        0,
    );
    let mut cas: Vec<Arc<Certificate>> =
        pki.all().iter().map(|a| Arc::new(a.cert.clone())).collect();
    if with_new_aca {
        cas.push(Arc::new(pki.extra[0].cert.clone()));
    }
    rt.security_mut()
        .install_trust(&[Arc::new(pki.root.cert.clone())], &cas);
    let key = pseudonym(pki, aca, 50 + n);
    let handle = v2xw_node::stores::CredentialHandle {
        digest: v2xw_node::stores::pseudo_signer(node, 0),
        cert_coer: Vec::new(),
        key: v2xw_sec::KeyId(1),
        i_period: 0,
        j_index: 0,
        valid_from: 0,
        valid_until: 10_000 * v2xw_core::time::NS_PER_S,
        state: v2xw_node::stores::CredState::Active,
    };
    rt.install_credential(handle, &key).expect("installs");
    rt.stores_mut().crl.set_period(0);
    let mut b = v2xw_core::belief::PositionEstimate::no_fix(0);
    b.semi_major_m = 5.0;
    b.semi_minor_m = 3.0;
    b.fix = v2xw_core::belief::FixQuality::ThreeD;
    rt.set_belief(b);
    rt
}

fn frames_of(out: &v2xw_node::runtime::StepOutcome) -> Vec<v2xw_node::runtime::RxFrame> {
    out.transmissions
        .iter()
        .filter_map(|t| {
            let s = t.signed.as_ref()?;
            Some(v2xw_node::runtime::RxFrame {
                signer: Some(t.signer.clone()),
                msg_type: t.msg_type,
                bytes: t.bytes,
                claimed_pos: None,
                claimed_speed_mps: 0.0,
                claimed_heading_rad: 0.0,
                claimed_generation_time: t.generation_time,
                full_certificate: t.full_certificate,
                signature_valid: true,
                spdu: Some(s.spdu.clone()),
                signer_certificate: None,
            })
        })
        .collect()
}

#[test]
fn the_runtime_requests_answers_and_learns() {
    let pki = pki_with_new_aca();
    let mut asker = obu(&pki, &pki.aca, 1, false);
    let mut newcomer = obu(&pki, &pki.extra[0], 2, true);
    let reg = RngRegistry::new(9);
    let mut to_asker = Vec::new();
    let mut to_new = Vec::new();
    let mut verified_new = None;
    let mut recorded: Vec<serde_json::Value> = Vec::new();
    for k in 0..30u64 {
        let now = k * 100 * NS_PER_MS;
        let mut ctx = NodeRuntimeCtx::new(now, &reg);
        let a = asker.step(&mut ctx, core::mem::take(&mut to_asker), 0.0);
        recorded.extend(
            ctx.emitted()
                .iter()
                .filter(|r| r.channel == "node.p2pcd")
                .map(|r| serde_json::from_slice(&r.json).expect("json")),
        );
        let mut ctx = NodeRuntimeCtx::new(now, &reg);
        let b = newcomer.step(&mut ctx, core::mem::take(&mut to_new), 0.0);
        let _ = &b;
        to_new.extend(frames_of(&a));
        to_asker.extend(frames_of(&b));
        for (_, pdu) in newcomer.take_p2pcd_responses() {
            asker.receive_p2pcd_response(&pdu).expect("a response");
        }
        if verified_new.is_none()
            && a.delivered
                .iter()
                .any(|m| m.verification == v2xw_node::stores::VerificationState::Verified)
        {
            verified_new = Some(k);
        }
    }
    let sa = &asker.security().p2pcd().stats;
    let sb = &newcomer.security().p2pcd().stats;
    eprintln!("runtime: asker {sa:?}\nnewcomer {sb:?}\nverified at step {verified_new:?}");
    assert!(sa.requests_sent >= 1, "the asker's BSM carried the request");
    assert!(sb.responses_sent >= 1, "the newcomer's P2P entity answered");
    assert_eq!(sa.learned, 1, "the asker learned the new ACA");
    assert!(verified_new.is_some(), "and then verified the newcomer");
    // Recorded on `node.p2pcd`: the request the asker sent, as a delta.
    let sum = |f: &str| -> u64 { recorded.iter().map(|r| r[f].as_u64().unwrap_or(0)).sum() };
    assert_eq!(sum("requests_sent"), sa.requests_sent, "{recorded:?}");
    assert_eq!(sum("triggers"), sa.triggers);
    assert!(recorded.iter().all(|r| r["flavor"] == "out-of-band"));
}
