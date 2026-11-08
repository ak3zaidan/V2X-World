//! SAE J2945/1's critical event condition is the event a critical event flag stands for
//! (3.1.2-3.1.3): the hard-braking, ABS, traction-control and stability-control flags of
//! `DE_VehicleEventFlags`. Whatever sets one of those flags on a unit's BSM also makes the
//! transmission decision (6.3.8.5) see a critical event, which sends the next BSM at once
//! and at `vRPMax`.
//!
//! The congestion control used to raise the critical event from the deceleration alone, at
//! a light vehicle's 0.4 g. A heavy vehicle's hard-braking flag is set at 0.2 g (J2735) and
//! the ABS flag while the anti-lock brakes regulate, which on a slippery road is below
//! 0.4 g; both set the flag, so the BSM carried the full certificate, but neither was a
//! critical event to the schedule, so neither BSM went out early. Fault that turns it red:
//! `Engine::update_dcc` reading the critical event from the deceleration again (no
//! critical-event BSM from a truck braking at 0.3 g, or from a car whose ABS regulates on
//! snow).

use std::path::Path;

use serde_json::{Value, json};
use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::{Engine, RunRecorder, Scenario};

use v2xw_msg::j2735::bsm::VehicleEventFlags;

/// The hard-braking and ABS flags as `node.tx` records the BSM's `VehicleEventFlags`.
const HARD_BRAKING: u64 = VehicleEventFlags::HARD_BRAKING.0 as u64;
const ABS_ACTIVATED: u64 = VehicleEventFlags::ABS_ACTIVATED.0 as u64;

#[derive(Default)]
struct Tx(Vec<Value>);

impl RunRecorder for Tx {
    fn write(&mut self, _at: SimTime, r: &OwnedRecord) {
        if r.channel == "node.tx" {
            let v: Value = serde_json::from_slice(&r.json).expect("a record is JSON");
            if v["msg_type"] == "bsm" {
                self.0.push(v);
            }
        }
    }
}

/// A fleet of `class` vehicles on `surface`, nodes 2-7 scripted to brake hard at 4 s at
/// `decel_mps2` (capped by the road's grip): the BSMs it sent.
fn run(class: &str, surface: Option<&str>, decel_mps2: f64) -> Vec<Value> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let s = Scenario::load(
        root.join("scenarios")
            .join("live-atk")
            .join("ma-legacy.yaml"),
    )
    .expect("the shipped scenario loads");
    let mut d = serde_json::to_value(&s).expect("serialises");
    d["time"]["duration_s"] = json!(8.0);
    d["actors"]["vehicles"]["demand"]["fleet_size"] = json!(12);
    d["actors"]["vehicles"]["classes"] = json!({ class: {"fraction": 1.0} });
    d["threats"]["attackers"] = json!([]);
    // The dataset gate records each BSM's content as the encoder put it on the air, its
    // event flags among it.
    d["exporters"] = json!([{"id": "dataset"}]);
    if let Some(surface) = surface {
        d["weather"]["surface"] = json!(surface);
    }
    d["events"] = json!(
        (2..8)
            .map(|n| json!({
                "t": 4.0, "type": "safety.hard-brake", "decel_mps2": decel_mps2, "target": n
            }))
            .collect::<Vec<_>>()
    );
    let s = Scenario::from_document(d).expect("deserialises");
    let errors = v2xw_engine::scenario::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    assert!(
        v2xw_engine::wiring::j2945_schedule(&s).is_some(),
        "the fleet runs SAE J2945/1's BSM schedule"
    );
    let mut engine = Engine::build(s, "").expect("builds");
    let mut tx = Tx::default();
    engine.run(&mut tx).expect("runs");
    tx.0
}

/// The BSMs that carry `flag`, and of those how many the congestion control sent for a
/// critical event (`event=critical` in the frame's congestion-control state).
fn flagged(bsms: &[Value], flag: u64) -> (usize, usize) {
    let with: Vec<&Value> = bsms
        .iter()
        .filter(|v| {
            v["content"]["event_flags"]
                .as_u64()
                .is_some_and(|f| f & flag != 0)
        })
        .collect();
    let critical = with
        .iter()
        .filter(|v| {
            v["dcc_state"]
                .as_str()
                .is_some_and(|s| s.contains("event=critical"))
        })
        .count();
    (with.len(), critical)
}

#[test]
fn every_critical_event_flag_is_a_critical_event_to_the_bsm_schedule() {
    // A truck braking at 0.3 g: past the heavy vehicle's 0.2 g, short of a car's 0.4 g.
    let trucks = run("truck", None, 3.0);
    let (hard, hard_critical) = flagged(&trucks, HARD_BRAKING);
    // A car braking as hard as it can on snow (grip 0.25: 2.45 m/s²): its ABS regulates.
    let cars = run("passenger", Some("snow"), 8.0);
    let (abs, abs_critical) = flagged(&cars, ABS_ACTIVATED);
    println!(
        "trucks: {} BSMs, {hard} with the hard-braking flag, {hard_critical} of them \
         critical-event BSMs; cars on snow: {} BSMs, {abs} with the ABS flag, {abs_critical} \
         of them critical-event BSMs",
        trucks.len(),
        cars.len()
    );
    assert!(hard > 0, "no truck's BSM carried the hard-braking flag");
    assert!(
        hard_critical > 0,
        "a truck braking at 0.3 g set its hard-braking flag but sent no critical-event BSM"
    );
    assert!(abs > 0, "no car's BSM on snow carried the ABS flag");
    assert!(
        abs_critical > 0,
        "a car whose ABS regulated set its flag but sent no critical-event BSM"
    );
    // No BSM went for a critical event without a critical event flag, except the one
    // after the event ends: the flag is the vehicle bus's at the BSM's generation, the
    // congestion control's critical event that of the unit's last transmission decision,
    // which may come up to one 100 ms interval after the bus last read the braking.
    for (what, bsms) in [("truck", &trucks), ("car", &cars)] {
        let critical_event_flag = |v: &Value| {
            v["content"]["event_flags"]
                .as_u64()
                .is_some_and(|f| f & (HARD_BRAKING | ABS_ACTIVATED) != 0)
        };
        let mut last: std::collections::BTreeMap<u64, &Value> = std::collections::BTreeMap::new();
        let mut unflagged = Vec::new();
        for v in bsms.iter() {
            let node = v["node"].as_u64().expect("a node");
            let sent_for_one = v["dcc_state"]
                .as_str()
                .is_some_and(|s| s.contains("event=critical"));
            if sent_for_one
                && !critical_event_flag(v)
                && !last
                    .get(&node)
                    .is_some_and(|prev| critical_event_flag(prev))
            {
                unflagged.push(v["t_generated"].clone());
            }
            last.insert(node, v);
        }
        assert!(
            unflagged.is_empty(),
            "{} {what} critical-event BSMs neither flagged nor right after a flagged one, \
             generated at {unflagged:?} ns",
            unflagged.len()
        );
    }
}
