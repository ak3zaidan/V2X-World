//! A moped carries a vehicle OBU (`wiring::is_vru_class` gives a VRU device to a
//! pedestrian and a cyclist only), so its J2945/1 BSMs carry what every OBU reads off its
//! own vehicle, among them `accelSet.long`.
//!
//! The bus used to be built only for classes outside the mobility's wider `is_vru`, which
//! also counts the moped and the scooter. A moped's BSM then went without `accelSet`,
//! brakes or event flags, while its J2945/1 schedule (which reads the deceleration from
//! the vehicle's motion) still sent a new critical event's BSM at once, a few ms after the
//! one before; an SM-MBR receiver with no `accelSet` to read took the speed difference
//! over those few ms as the acceleration and filed honest MaxAcceleration reports
//! (0.5 m/s in 3.9 ms, 130 m/s², at full density). Fault: put the `is_vru` gate back on
//! the bus and every moped BSM here has a null `accel_long_mps2`.

use std::path::Path;

use serde_json::{Value, json};
use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::{Engine, RunRecorder, Scenario};

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

#[test]
fn a_mopeds_bsm_carries_its_own_acceleration() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let s = Scenario::load(
        root.join("scenarios")
            .join("live-atk")
            .join("ma-legacy.yaml"),
    )
    .expect("the shipped scenario loads");
    let mut d = serde_json::to_value(&s).expect("serialises");
    d["time"]["duration_s"] = json!(8.0);
    d["actors"]["vehicles"]["demand"]["fleet_size"] = json!(8);
    d["actors"]["vehicles"]["classes"] = json!({"moped": {"fraction": 1.0}});
    d["threats"]["attackers"] = json!([]);
    // The dataset gate records each BSM's `accelSet.long` as the encoder put it on the air.
    d["exporters"] = json!([{"id": "dataset"}]);
    let s = Scenario::from_document(d).expect("deserialises");
    let errors = v2xw_engine::scenario::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut tx = Tx::default();
    engine.run(&mut tx).expect("runs");
    assert!(tx.0.len() > 100, "{} BSMs", tx.0.len());
    let without: Vec<&Value> =
        tx.0.iter()
            .filter(|r| r["content"]["accel_long_mps2"].is_null())
            .collect();
    assert!(
        without.is_empty(),
        "{} of {} moped BSMs without accelSet, e.g. {}",
        without.len(),
        tx.0.len(),
        without[0]
    );
}
