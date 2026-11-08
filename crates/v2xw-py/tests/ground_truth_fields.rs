//! `v2xw_record::export::schema::ground_truth_fields` checked against the records the
//! engine really writes.
//!
//! The ground-truth projection (`TableSchema::without_ground_truth`, which
//! `Recording.arrow(..., ground_truth=False)` and the node-only exporters use) and the
//! channel-tagged leakage lint both match columns by exact name. A declared name that no
//! record carries therefore withholds nothing and fails nothing: `phy.rx`'s list once
//! named `tx_node`, `distance_m` and `los_class` while the record carries `tx` and
//! `dist_m`, and a `ground_truth=False` export shipped the true transmitter id and the
//! true distance with every check green.
//!
//! The tests live here, not in `v2xw-record`, because this is the first crate that sees
//! the engine's record types (`v2xw-record` sits below the engine). Each sample below is
//! the real record type serialised, so its keys are the field names a recording holds.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::{Value, json};
use v2xw_core::Visibility;
use v2xw_engine::app_truth::AppOutcome;
use v2xw_engine::records::{NodeRx, PhyRx};
use v2xw_engine::report_records::ReportFate;
use v2xw_engine::sec_records::PkiKnowledgeRecord;
use v2xw_metrics::latency::LatencyTrace;
use v2xw_record::channels::CHANNELS;
use v2xw_record::export::schema::ground_truth_fields;

/// The top-level JSON keys a record serialises with.
fn keys<T: Serialize>(record: &T) -> BTreeSet<String> {
    match serde_json::to_value(record).expect("the record serialises") {
        Value::Object(m) => m.keys().cloned().collect(),
        other => panic!("a record must serialise as a JSON object, got {other}"),
    }
}

fn from<T: serde::de::DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v).expect("the sample deserialises into the record type")
}

/// One record of every node-and-ground-truth channel, built as the engine's own type with
/// every optional field present, keyed by channel.
fn engine_records() -> BTreeMap<&'static str, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    out.insert(
        "phy.rx",
        keys(&PhyRx(from(json!({
            "t_start": 1, "t_end": 2, "tx": 0, "rx": 1, "msg": 7, "rssi_dbm": -70.0,
            "sinr_db": 20.0, "outcome": "ok", "dist_m": 12.5, "candidate": true,
            "payload_bytes": 39, "focus": "inside", "copies": 2, "sdu": 7,
        })))),
    );
    out.insert(
        "node.rx",
        keys(&NodeRx(from(json!({
            "t": 2, "rx": 1, "tx": 0, "msg": 7, "msg_type": "bsm", "outcome": "lost",
            "cause": "below-sensitivity", "dist_m": 12.5, "t_handoff": 1,
        })))),
    );
    out.insert(
        "msg.latency",
        keys(&from::<LatencyTrace>(json!({
            "flow": "v2v", "msg_type": "bsm", "msg": 7, "origin": 0, "dest": 1,
            "t_origin": 0, "spans": [{"stage": "sign", "hop": 0, "start": 0, "end": 1}],
        }))),
    );
    out.insert(
        "app.outcome",
        keys(&AppOutcome {
            t: 3,
            node: from(json!(1)),
            app: "fcw".to_string(),
            subject: Some(from(json!(0))),
            outcome: "true",
            ttc_truth_s: Some(2.1),
            delay_s: Some(0.1),
        }),
    );
    out.insert(
        "pki.report_fate",
        keys(&from::<ReportFate>(json!({
            "t": 4, "report_id": "r-1", "reporter": 1, "fate": "stored", "weight": 1.0,
            "subject": "ffaf5eca9bc1755e",
            "truth": {"subject_node": 0, "subject_attacker": true},
        }))),
    );
    out.insert(
        "pki.knowledge",
        keys(&PkiKnowledgeRecord(from(json!({
            "schema": 1, "t": 0, "entity": "provider-a/ra/0", "role": "ra",
            "holds": {}, "cannot_see": [], "truth": {},
        })))),
    );
    out
}

/// Every name a channel declares ground truth is a field its real record carries.
#[test]
fn every_declared_ground_truth_field_is_a_field_of_its_channels_real_record() {
    let records = engine_records();
    let mut problems = Vec::new();
    for spec in CHANNELS {
        let declared = ground_truth_fields(spec.name);
        if declared.is_empty() {
            continue;
        }
        let Some(real) = records.get(spec.name) else {
            problems.push(format!(
                "{} declares ground-truth fields {declared:?} but this test has no record of \
                 the channel to check them against; add the engine's record type above",
                spec.name
            ));
            continue;
        };
        for name in declared {
            if !real.contains(*name) {
                problems.push(format!(
                    "{}.{name} is declared ground truth, but the record the engine writes has \
                     no such field (it has {real:?}), so the projection withholds nothing",
                    spec.name
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// A channel tagged node-and-ground-truth declares which of its columns are the ground
/// truth; with none declared, a node-only export of it would carry them all.
#[test]
fn every_node_and_ground_truth_channel_declares_its_ground_truth_columns() {
    let missing: Vec<&str> = CHANNELS
        .iter()
        .filter(|c| c.visibility == Visibility::NodeAndGt)
        .filter(|c| ground_truth_fields(c.name).is_empty())
        .map(|c| c.name)
        .collect();
    assert!(
        missing.is_empty(),
        "node-and-ground-truth channels with no declared ground-truth column: {missing:?}"
    );
    // The check above is not vacuous: the registry does tag channels this way.
    assert!(
        CHANNELS
            .iter()
            .any(|c| c.name == "phy.rx" && c.visibility == Visibility::NodeAndGt)
    );
}
