//! P2PCD over the engine's radio (00 §1 row 38, WP-C3 follow-up).
//!
//! Four roadside units on `connected-intersections` and its vehicles, split half and half
//! between the two providers by the seeded fleet split (`fleet_share` 0.5 each; since
//! WP-D1 one draw assigns vehicles and units alike). Under
//! `security.scms.chain_file: home-provider` a device's chain file holds only its own
//! provider's CA certificates, so the other provider's units and vehicles name an issuer
//! it does not hold: a vehicle asks for it in its BSMs (`p2pcdLearningRequest`), a
//! P2PCD Entity that holds it answers with an unsecured `Ieee1609dot2Peer2PeerPDU` under
//! PSID 136 once its random back-off expires, and that answer is a radio frame of its own
//! — through the MAC and the PHY, heard or lost like any frame — from which every vehicle
//! that decodes it learns the CA certificates (each validated to the CTL root) and then
//! verifies the other provider's units' SPaT and MAP.
//!
//! What each test proves, and the fault it was shown to fail under (each injected and
//! reverted, see the WP report):
//! * the response is on the air and learned from it (fault: the engine drops the outbox
//!   instead of handing it down);
//! * the response leaves at its back-off instant, not at the node's next periodic step
//!   or a later frame arrival: `response_lag_ns` is zero (fault: no wake for the
//!   back-off, which made the lag positive);
//! * with the default chain file nothing is requested and no response is sent (the
//!   control).

use std::collections::BTreeMap;
use std::path::Path;

use v2xw_engine::{Engine, MemoryRecorder, Scenario};

fn scenario(chain_file: &str) -> Scenario {
    let mut s = Scenario::load(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/connected-intersections.yaml"),
    )
    .expect("loads");
    s.time.duration_s = 12.0;
    s.threats.attackers.clear();
    s.messages.sets = vec!["bsm".into(), "spat".into(), "map".into()];
    s.security.protocol = Some(
        serde_json::from_value(serde_json::json!({"id": "protocol/scms/camp"})).expect("choice"),
    );
    s.security.scms = Some(
        serde_json::from_value(serde_json::json!({
            "deployment": {
                "preset": "us-production-2025",
                "providers": [
                    {"name": "provider-a", "fleet_share": 0.5},
                    {"name": "provider-b", "fleet_share": 0.5}
                ]
            },
            "chain_file": chain_file,
        }))
        .expect("an scms section"),
    );
    s
}

struct Run {
    on_air: u64,
    /// `node.tx` of every P2PCD frame: (node, t_generated, bytes, spdu_bytes).
    frames: Vec<(u32, u64, u64, Option<u64>)>,
    /// Σ over vehicles' `node.p2pcd` of each counter.
    p2pcd: BTreeMap<String, u64>,
    /// `node.rx` fates of P2PCD frames, by "outcome/verification-or-cause".
    rx: BTreeMap<String, u64>,
    /// Node phases (ns) and the mobility step (ns).
    phase: BTreeMap<u32, u64>,
    step_ns: u64,
    /// SPaT and MAP receptions verified by a vehicle.
    spat_verified: u64,
    /// Σ `node.p2pcd` `triggers` by the provider that certifies the node (deployment
    /// order: provider-a, the home provider, first).
    triggers_by_provider: BTreeMap<usize, u64>,
}

fn run(chain_file: &str) -> Run {
    let s = scenario(chain_file);
    let step_ns = s.time.mobility_step().as_nanos();
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    let mut frames = Vec::new();
    let mut p2pcd: BTreeMap<String, u64> = BTreeMap::new();
    let mut rx: BTreeMap<String, u64> = BTreeMap::new();
    let mut spat_verified = 0;
    let mut nodes = std::collections::BTreeSet::new();
    let seed = engine.scenario().seed;
    let provider_of = |node: u64| {
        engine
            .phase2()
            .expect("an SCMS run")
            .deployment()
            .provider_of(seed, node)
    };
    let mut triggers_by_provider: BTreeMap<usize, u64> = BTreeMap::new();
    for (_, r) in rec.records() {
        let v: serde_json::Value = match r.channel {
            "node.tx" | "node.p2pcd" | "node.rx" => serde_json::from_slice(&r.json).expect("json"),
            _ => continue,
        };
        match r.channel {
            "node.tx" if v["msg_type"] == "p2pcd" => {
                let node = v["node"].as_u64().expect("node") as u32;
                nodes.insert(node);
                frames.push((
                    node,
                    v["t_generated"].as_u64().expect("t_generated"),
                    v["bytes_on_wire"].as_u64().unwrap_or(0),
                    v["spdu_bytes"].as_u64(),
                ));
            }
            "node.p2pcd" => {
                if let (Some(node), Some(n)) = (v["node"].as_u64(), v["triggers"].as_u64()) {
                    *triggers_by_provider.entry(provider_of(node)).or_insert(0) += n;
                }
                if let Some(o) = v.as_object() {
                    for (k, x) in o {
                        if let Some(n) = x.as_u64()
                            && !matches!(k.as_str(), "t" | "node")
                        {
                            *p2pcd.entry(k.clone()).or_insert(0) += n;
                        }
                    }
                }
            }
            "node.rx" => {
                let key = format!(
                    "{}/{}",
                    v["outcome"].as_str().unwrap_or("?"),
                    v["verification"]
                        .as_str()
                        .or(v["cause"].as_str())
                        .unwrap_or("-")
                );
                if v["msg_type"] == "p2pcd" {
                    *rx.entry(key).or_insert(0) += 1;
                } else if (v["msg_type"] == "spat" || v["msg_type"] == "map")
                    && key == "delivered/verified"
                {
                    spat_verified += 1;
                }
            }
            _ => {}
        }
    }
    let phase = nodes
        .iter()
        .map(|n| {
            let id = v2xw_core::ids::NodeId::new(*n);
            (*n, engine.node_phase(id).map_or(0, |d| d.as_nanos()))
        })
        .collect();
    let out = Run {
        on_air: report.p2pcd_responses_on_air,
        frames,
        p2pcd,
        rx,
        phase,
        step_ns,
        spat_verified,
        triggers_by_provider,
    };
    eprintln!(
        "{chain_file}: on_air {} frames {} p2pcd {:?} rx {:?} spat/map verified {} triggers by provider {:?}",
        out.on_air,
        out.frames.len(),
        out.p2pcd,
        out.rx,
        out.spat_verified,
        out.triggers_by_provider
    );
    out
}

#[test]
fn an_unknown_provider_ca_is_learned_from_a_response_on_the_air() {
    let r = run("home-provider");
    assert!(
        r.p2pcd.get("requests_sent").copied().unwrap_or(0) > 0,
        "no vehicle asked: the unknown issuer never reached a receiver"
    );
    assert!(r.on_air > 0, "no learning response was put on the air");
    assert_eq!(
        r.frames.len() as u64,
        r.on_air,
        "every response handed down is one node.tx frame"
    );
    // The frame is the PDU itself, unsecured: its SPDU octets are the PDU's and it is
    // bigger than a lone certificate (caCerts carries the ACA and the ICA).
    for (_, _, _, spdu) in &r.frames {
        assert!(spdu.is_some_and(|b| b > 150), "frame {spdu:?}");
    }
    let heard = r.rx.get("delivered/unsecured").copied().unwrap_or(0);
    assert!(
        heard > 0,
        "no receiver took a response off the air: {:?}",
        r.rx
    );
    assert!(
        r.p2pcd.get("learned").copied().unwrap_or(0) > 0,
        "nothing was learned from the responses heard"
    );
    assert!(
        r.p2pcd.get("responses_observed").copied().unwrap_or(0) > 0,
        "responders did not observe each other's responses"
    );
    assert!(
        r.spat_verified > 0,
        "after learning, no vehicle verified a unit's SPaT or MAP"
    );
}

/// R-P2PCD-ASYM (docs/validation/live-scms-2026-10-25.md finding 1): IEEE 1609.2 P2PCD is
/// symmetric, so each provider's devices ask for the other provider's CA certificates.
/// Every vehicle's factory pool used to be issued by the home provider's ACA, which put
/// that ACA (unanchored) in every provider-b device's cache: provider-b devices then never
/// raised a request for provider-a's issuer, and lost its traffic as unknown signers.
/// Fault: `CredentialFactory::pseudonym` issuing from the home ACA again gives provider-b
/// 0 triggers.
#[test]
fn both_providers_devices_ask_for_the_other_providers_issuer() {
    let r = run("home-provider");
    for k in [0usize, 1] {
        assert!(
            r.triggers_by_provider.get(&k).copied().unwrap_or(0) > 0,
            "provider {k}'s devices never asked for the other provider's issuer: {:?}",
            r.triggers_by_provider
        );
    }
}

#[test]
fn a_response_leaves_at_its_backoff_instant_not_at_a_step() {
    let r = run("home-provider");
    let sent = r.p2pcd.get("responses_sent").copied().unwrap_or(0);
    assert!(sent > 0, "no response to time");
    // Each response is handed down at the instant its random back-off expired: the node
    // is woken for it, so the lag past the back-off is exactly zero.
    assert_eq!(
        r.p2pcd.get("response_lag_ns").copied(),
        Some(0),
        "responses left after their back-off expired: the wake was quantised"
    );
    // And the instants are the back-off's own, not the node's periodic step grid.
    let off_grid = r
        .frames
        .iter()
        .filter(|(node, t, ..)| {
            let phase = r.phase.get(node).copied().unwrap_or(0);
            t.wrapping_sub(phase) % r.step_ns != 0
        })
        .count();
    assert!(off_grid > 0, "every response left on its node's step grid");
}

#[test]
fn with_the_global_chain_file_nothing_is_requested_or_sent() {
    let r = run("all-providers");
    assert_eq!(r.on_air, 0);
    assert_eq!(r.p2pcd.get("requests_sent").copied().unwrap_or(0), 0);
    assert!(r.spat_verified > 0, "the control verifies SPaT and MAP");
}
