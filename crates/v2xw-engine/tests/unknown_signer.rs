//! A strict verification policy holds when the signer's certificate is unknown.
//!
//! IEEE 1609.2-2022 §5.2.4.2 / SAE J2945/1-2020 §6.3.8: a receiver that cannot resolve a
//! `digest` signer identifier has not verified the message; it requests the certificate
//! through P2PCD and does not pass the message on as if it had been checked. Under
//! `verify-all` a message reaching an application is a verified message, so an unknown
//! signer is a loss (`unknown-signer`), not an `unverified` delivery.

use std::collections::BTreeMap;
use std::path::Path;

use v2xw_engine::{Engine, MemoryRecorder, Scenario};

fn fates(policy: &str) -> BTreeMap<String, u64> {
    let mut s = Scenario::load(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/phase1-grid.yaml"),
    )
    .expect("the shipped grid loads");
    s.time.duration_s = 20.0;
    s.actors.vehicles.demand.rate_veh_per_h = Some(900.0);
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = serde_json::json!(5);
        params["rows"] = serde_json::json!(8);
    }
    s.security.verification_policy = policy.to_string();
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    engine.run(&mut rec).expect("runs");
    let mut out = BTreeMap::new();
    for (_, r) in rec.records() {
        if r.channel != "node.rx" {
            continue;
        }
        let v: serde_json::Value = serde_json::from_slice(&r.json).expect("json");
        let key = format!(
            "{}/{}",
            v["outcome"].as_str().unwrap_or("?"),
            v["verification"]
                .as_str()
                .or(v["cause"].as_str())
                .unwrap_or("-")
        );
        *out.entry(key).or_insert(0) += 1;
    }
    println!("{policy}: {out:?}");
    out
}

#[test]
fn verify_all_delivers_nothing_unverified_and_counts_the_unknown_signers() {
    let f = fates("verify-all");
    let delivered_unverified = f.get("delivered/unverified").copied().unwrap_or(0);
    assert_eq!(
        delivered_unverified, 0,
        "verify-all passed an unchecked message on"
    );
    assert!(
        f.get("lost/unknown-signer").copied().unwrap_or(0) > 0,
        "no message from an unknown signer: the check proves nothing"
    );
    assert!(f.get("delivered/verified").copied().unwrap_or(0) > 0);
}
