//! WP-E1: `inspect.pdu`, `inspect.cert` and `inspect.knowledge` on a recorded run.
//!
//! The recording is the committed UI fixture (`ui/apps/studio/test/fixtures/pki/`),
//! written by `v2xw-engine/tests/pki_view.rs` from a two-provider Manhattan run. The
//! server's store reads its records as a live run or a replay delivers them, and every
//! answer must equal the one the engine test computed on the full recording
//! (`pki-answers.json`): the fixture is self-sufficient and the server adds nothing.

use std::path::PathBuf;

use serde_json::Value;
use v2xw_server::pki::{PkiQuery, PkiStore};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/apps/studio/test/fixtures/pki")
}

fn loaded() -> (PkiStore, Vec<Value>) {
    let text = std::fs::read_to_string(fixtures().join("pki-run.jsonl")).expect("the fixture");
    let mut store = PkiStore::default();
    let mut rows = Vec::new();
    for line in text.lines() {
        let row: Value = serde_json::from_str(line).expect("a JSON line");
        let channel = row["channel"].as_str().expect("channel").to_string();
        let bytes = serde_json::to_vec(&row["record"]).expect("json");
        if channel != "pki.case" {
            assert!(store.absorb(&channel, &bytes), "{channel} is a PKI channel");
        }
        rows.push(row);
    }
    assert!(store.undecodable.is_empty(), "{:?}", store.undecodable);
    (store, rows)
}

fn answers() -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(fixtures().join("pki-answers.json")).expect("answers"),
    )
    .expect("json")
}

#[test]
fn every_recorded_pdu_answers_as_the_engine_answered_it() {
    let (store, _) = loaded();
    let a = answers();
    let pdus = a["inspect.pdu"].as_object().expect("pdus");
    assert!(pdus.len() > 50, "only {} PDUs in the fixture", pdus.len());
    for (id, views) in pdus {
        for (view, expected) in views.as_object().expect("views") {
            let view = if view == "other" {
                "provider-a/la1/0"
            } else {
                view
            };
            let got = store
                .answer(&PkiQuery::Pdu {
                    id: id.clone(),
                    view: view.to_string(),
                })
                .unwrap_or_else(|| panic!("{id} unknown"));
            assert_eq!(&got, expected, "{id} as {view}");
        }
    }
    assert!(
        store
            .answer(&PkiQuery::Pdu {
                id: "provider-a/pdu/999999999".into(),
                view: "ground-truth".into()
            })
            .is_none()
    );
}

#[test]
fn every_vehicle_certificate_answers_as_the_engine_answered_it() {
    let (store, _) = loaded();
    let a = answers();
    let certs = a["inspect.cert"].as_object().expect("certs");
    assert!(certs.len() > 10, "only {} certificates", certs.len());
    let mut valid = 0;
    for (id8, expected) in certs {
        let mut got = store
            .answer(&PkiQuery::Cert {
                id8: id8.clone(),
                t_ns: None,
            })
            .unwrap_or_else(|| panic!("{id8} unknown"));
        got.as_object_mut().expect("object").remove("t_ns");
        assert_eq!(&got, expected, "{id8}");
        valid += usize::from(got["valid"] == true);
    }
    assert!(valid > 0, "no certificate validates");
}

#[test]
fn knowledge_answers_hide_ground_truth_outside_it() {
    let (store, _) = loaded();
    let a = answers();
    for (entity, expected) in a["inspect.knowledge"].as_object().expect("knowledge") {
        for (view, key) in [
            ("ground-truth", "ground-truth"),
            (entity.as_str(), "entity"),
        ] {
            let got = store
                .answer(&PkiQuery::Knowledge {
                    entity: entity.clone(),
                    view: view.to_string(),
                })
                .expect("known");
            assert_eq!(got, expected[key], "{entity} as {view}");
            assert_eq!(
                got.get("truth").is_some(),
                view == "ground-truth",
                "{entity} as {view}"
            );
        }
    }
}
