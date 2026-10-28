//! A vehicle's node id, and every draw keyed by it, does not depend on how many roadside
//! units the scenario declares (P1-E6's RSU-density confound, 2026-10-27).
//!
//! Before the fix `Engine::create_rsus` took the vehicle counter's first values, so adding
//! a mast renumbered every vehicle and re-keyed its BSM clock, timing class, GNSS and
//! hand-off draws: an RSU-count sweep compared different fleets. The vehicles are matched
//! here by spawn rank (both runs spawn the same actors in the same order), so the test
//! reads the same vehicle in both runs with or without the fix, and the fault (units on
//! the vehicle counter) turns it red.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};
use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::{Engine, RunRecorder, Scenario};

/// Per sender node: `(t_generated, secMark-relevant generation instant, payload bytes)`
/// of every BSM it put on the air.
#[derive(Default)]
struct Out {
    bsm: BTreeMap<u32, Vec<(SimTime, Option<u64>)>>,
}

impl RunRecorder for Out {
    fn write(&mut self, _at: SimTime, r: &OwnedRecord) {
        if &*r.channel != "node.tx" {
            return;
        }
        let v: Value = serde_json::from_slice(&r.json).expect("node.tx parses");
        if v["msg_type"].as_str() != Some("bsm") {
            return;
        }
        let node = u32::try_from(v["node"].as_u64().expect("node")).expect("u32");
        let generated = v["t_generated"].as_u64().or_else(|| v["t"].as_u64()).expect("t");
        self.bsm
            .entry(node)
            .or_default()
            .push((generated, v["payload_bytes"].as_u64()));
    }
}

fn run(extra_rsus: usize) -> Out {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let s = Scenario::load(root.join("scenarios").join("credential-lifecycle.yaml"))
        .expect("the shipped scenario loads");
    let mut d = serde_json::to_value(&s).expect("serialises");
    d["time"]["duration_s"] = json!(40.0);
    // The shipped attacker starts at 60 s, past this run's end, which validation refuses;
    // the BSM sequences compared here are the honest vehicles' own.
    d["threats"]["attackers"] = json!([]);
    // Two device classes, so the class draw (keyed by the node) is exercised.
    d["actors"]["vehicles"]["bsm_timing"] = json!({"enabled": true, "locked_share": 0.5});
    let first = d["actors"]["rsus"][0].clone();
    for k in 0..extra_rsus {
        let mut u = first.clone();
        u["position_m"] = json!([148.0 + 200.0 * k as f64, 80.0, 0.0]);
        d["actors"]["rsus"].as_array_mut().expect("rsus").push(u);
    }
    let s = Scenario::from_document(d).expect("deserialises");
    let errors = v2xw_engine::scenario::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut out = Out::default();
    engine.run(&mut out).expect("runs");
    out
}

#[test]
fn a_roadside_count_change_leaves_every_vehicle_draw_byte_identical() {
    let one = run(0);
    let three = run(2);
    // Units send no BSM, so the BSM senders are the vehicles, in node-id order = spawn order.
    let a: Vec<_> = one.bsm.values().collect();
    let b: Vec<_> = three.bsm.values().collect();
    assert!(a.len() >= 10, "too few vehicles sent a BSM: {}", a.len());
    let n = a.len().min(b.len());
    let same = (0..n).filter(|&i| a[i] == b[i]).count();
    println!("{} / {} vehicles' BSM sequences identical", same, n);
    assert_eq!(a.len(), b.len(), "the RSU count changed how many vehicles transmit");
    assert_eq!(same, n, "an RSU-count change moved a vehicle's own draws");
    // And the node ids themselves: the first vehicle is node 0 in both.
    assert_eq!(one.bsm.keys().next(), Some(&0));
    assert_eq!(one.bsm.keys().collect::<Vec<_>>(), three.bsm.keys().collect::<Vec<_>>());
}
