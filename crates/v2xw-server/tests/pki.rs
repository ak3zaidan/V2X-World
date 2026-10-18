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

// ---------------------------------------------------------------------------------------
// WP-E2: the Studio's reads of pki.state and pki.flow
// ---------------------------------------------------------------------------------------

fn components(state: &Value) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut lists = vec![&state["governance"]];
    for p in state["providers"].as_array().expect("providers") {
        lists.push(&p["components"]);
    }
    for list in lists {
        for c in list.as_array().expect("components") {
            let t = &c["traffic"];
            let n = t["sent"].as_u64().unwrap() + t["received"].as_u64().unwrap();
            out.push((c["id"].as_str().unwrap().to_string(), n));
        }
    }
    out
}

#[test]
fn trust_answers_each_snapshot_at_its_instant_with_its_activity() {
    let (store, rows) = loaded();
    let states: Vec<&Value> = rows
        .iter()
        .filter(|r| r["channel"] == "pki.state")
        .map(|r| &r["record"])
        .collect();
    assert!(states.len() >= 3, "{} states", states.len());
    let mut lit = 0;
    for (i, s) in states.iter().enumerate() {
        let t = s["t"].as_u64().unwrap();
        let got = store
            .answer(&PkiQuery::Trust { t_ns: Some(t + 1) })
            .expect("a state");
        assert_eq!(got["t_ns"], t);
        // The state as recorded, less what the store does not keep for this read.
        let mut want = (*s).clone();
        want.as_object_mut().unwrap().remove("certificates");
        for c in want["crls"].as_array_mut().unwrap() {
            c.as_object_mut().unwrap().remove("hex");
        }
        assert_eq!(got["state"], want, "state at {t}");
        assert!(got["state"].get("certificates").is_none());
        // Activity: every component whose message count grew since the newest earlier
        // snapshot at least 2 s older (here every gap is), and only those.
        let act = got["activity"].as_object().expect("activity");
        if i == 0 {
            assert!(
                act.is_empty() && got["window_ns"].is_null(),
                "the first has no base"
            );
            continue;
        }
        let before: std::collections::BTreeMap<String, u64> =
            components(states[i - 1]).into_iter().collect();
        assert_eq!(got["window_ns"], t - states[i - 1]["t"].as_u64().unwrap());
        for (id, n) in components(s) {
            let grew = n.saturating_sub(before.get(&id).copied().unwrap_or(0));
            match act.get(&id) {
                Some(a) => {
                    assert!(grew > 0, "{id} at {t} is lit but carried nothing");
                    assert_eq!(a["messages"], grew, "{id} at {t}");
                }
                None => assert_eq!(grew, 0, "{id} at {t} grew but is not lit"),
            }
            lit += usize::from(grew > 0);
        }
    }
    assert!(
        lit > 10,
        "only {lit} lit components: the check would pass on nothing"
    );
    // Before the first snapshot: the first. With no instant: the newest.
    let first = store.answer(&PkiQuery::Trust { t_ns: Some(0) }).unwrap();
    assert_eq!(first["t_ns"], states[0]["t"]);
    let newest = store.answer(&PkiQuery::Trust { t_ns: None }).unwrap();
    assert_eq!(newest["t_ns"], states.last().unwrap()["t"]);
}

fn sequence_store() -> (PkiStore, Vec<Value>) {
    let text = std::fs::read_to_string(fixtures().join("pki-sequence.jsonl")).expect("the fixture");
    let mut store = PkiStore::default();
    let mut flows = Vec::new();
    for line in text.lines() {
        let row: Value = serde_json::from_str(line).expect("a JSON line");
        let channel = row["channel"].as_str().unwrap();
        if channel == "pki.case" {
            continue;
        }
        store.absorb(channel, &serde_json::to_vec(&row["record"]).unwrap());
        if channel == "pki.flow" {
            flows.push(row["record"].clone());
        }
    }
    (store, flows)
}

#[test]
fn flows_file_every_record_once_in_time_order_without_bytes_or_plaintext() {
    for (store, flows) in [sequence_store(), {
        let (s, rows) = loaded();
        let f = rows
            .into_iter()
            .filter(|r| r["channel"] == "pki.flow")
            .map(|r| r["record"].clone())
            .collect();
        (s, f)
    }] {
        let list = store
            .answer(&PkiQuery::Flows {
                flow: None,
                device: None,
                provider: None,
                limit: 5_000,
                t_ns: None,
            })
            .unwrap();
        let instances = list["instances"].as_array().unwrap();
        let distinct: std::collections::BTreeSet<&str> = flows
            .iter()
            .map(|f| f["instance"].as_str().unwrap())
            .collect();
        assert_eq!(instances.len(), distinct.len());
        assert_eq!(list["total"], distinct.len());
        let firsts: Vec<u64> = instances
            .iter()
            .map(|i| i["first_t"].as_u64().unwrap())
            .collect();
        assert!(
            firsts.windows(2).all(|w| w[0] >= w[1]),
            "newest first: {firsts:?}"
        );
        let mut seen = 0;
        for row in instances {
            let id = row["instance"].as_str().unwrap();
            let got = store
                .answer(&PkiQuery::Flow {
                    instance: id.to_string(),
                    t_ns: None,
                })
                .unwrap();
            let recs = got["records"].as_array().unwrap();
            let mut want: Vec<&Value> = flows.iter().filter(|f| f["instance"] == id).collect();
            want.sort_by_key(|f| f["t"].as_u64().unwrap());
            assert_eq!(recs.len(), want.len(), "{id}");
            for (g, w) in recs.iter().zip(&want) {
                assert_eq!(g["id"], w["id"], "{id}: order");
                assert_eq!(g["t"], w["t"]);
                assert_eq!(g["from"], w["from"]);
                assert_eq!(g["to"], w["to"]);
                assert_eq!(g["name"], w["name"]);
                let gp = g["pdus"].as_array().map_or(0, Vec::len);
                assert_eq!(gp, w["pdus"].as_array().map_or(0, Vec::len));
            }
            let text = serde_json::to_string(&got).unwrap();
            assert!(!text.contains("\"hex\""), "{id}: bytes or plaintext leaked");
            seen += recs.len();
            assert_eq!(
                row["messages"].as_u64().unwrap(),
                want.iter().filter(|w| w["kind"] == "msg").count() as u64
            );
        }
        assert_eq!(seen, flows.len());
        let any_hex = flows.iter().any(|f| f.to_string().contains("\"hex\""));
        assert!(
            any_hex,
            "the fixture carries bytes, so the leak check means something"
        );
    }
}

#[test]
fn flows_filter_by_flow_device_provider_and_instant() {
    let (store, _) = sequence_store();
    let q = |flow: Option<&str>, device: Option<&str>, provider: Option<&str>, t: Option<u64>| {
        store
            .answer(&PkiQuery::Flows {
                flow: flow.map(str::to_string),
                device: device.map(str::to_string),
                provider: provider.map(str::to_string),
                limit: 100,
                t_ns: t,
            })
            .unwrap()
    };
    let prov = q(Some("provisioning"), None, None, None);
    assert_eq!(prov["total"], 2);
    assert_eq!(prov["flows"]["provisioning"], 2);
    assert_eq!(prov["flows"]["enrolment"], 2);
    let mine = q(None, Some("1000"), None, None);
    let ids: Vec<&str> = mine["instances"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["instance"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["provider-a/provisioning/2", "provider-a/enrolment/1"]);
    assert_eq!(q(None, Some("ee/1000"), None, None)["total"], 2);
    assert_eq!(q(None, None, Some("provider-b"), None)["total"], 0);
    // At 1 s only the first enrolment and provisioning had started.
    assert_eq!(q(None, None, None, Some(1_000_000_000))["total"], 2);
    // One instance up to an instant: provisioning stops at the shuffle.
    let early = store
        .answer(&PkiQuery::Flow {
            instance: "provider-a/provisioning/2".into(),
            t_ns: Some(1_000_000_000),
        })
        .unwrap();
    let last = early["records"].as_array().unwrap().last().unwrap().clone();
    assert!(last["t"].as_u64().unwrap() <= 1_000_000_000);
    let whole = store
        .answer(&PkiQuery::Flow {
            instance: "provider-a/provisioning/2".into(),
            t_ns: None,
        })
        .unwrap();
    let stages: Vec<&str> = whole["records"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == "stage")
        .map(|r| r["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        stages,
        [
            "requested",
            "proxy_forwarded",
            "acknowledged",
            "expanded",
            "pre_linkage_ready",
            "shuffled",
            "certified",
            "batch_ready",
            "downloaded",
            "installed"
        ]
    );
    assert!(
        store
            .answer(&PkiQuery::Flow {
                instance: "provider-a/nope/1".into(),
                t_ns: None
            })
            .is_none()
    );
}

/// The views the Studio bundle answers `inspect.pdu` and `inspect.knowledge` in.
const STUDIO_VIEWS: [&str; 6] = [
    "ground-truth",
    "provider-a/ra/0",
    "provider-a/aca/0",
    "provider-a/la1/0",
    "provider-a/la2/0",
    "provider-a/ma/0",
];

/// What the server answers the Studio's PKI view on one fixture: `inspect.trust` at each
/// snapshot and with no instant, `inspect.flows` unfiltered and per flow, `inspect.flow`
/// for every instance and, when `pdus`, `inspect.pdu` and `inspect.knowledge` in each of
/// [`STUDIO_VIEWS`].
fn studio_answers(store: &PkiStore, rows: &[Value], pdus: bool) -> Value {
    let mut trust = serde_json::Map::new();
    for r in rows.iter().filter(|r| r["channel"] == "pki.state") {
        let t = r["record"]["t"].as_u64().unwrap();
        trust.insert(
            t.to_string(),
            store.answer(&PkiQuery::Trust { t_ns: Some(t) }).unwrap(),
        );
    }
    trust.insert(
        "newest".into(),
        store.answer(&PkiQuery::Trust { t_ns: None }).unwrap(),
    );
    let list = |flow: Option<&str>| {
        store
            .answer(&PkiQuery::Flows {
                flow: flow.map(str::to_string),
                device: None,
                provider: None,
                limit: 300,
                t_ns: None,
            })
            .unwrap()
    };
    let all = list(None);
    let mut flows = serde_json::Map::new();
    for f in all["flows"].as_object().unwrap().keys() {
        flows.insert(f.clone(), list(Some(f)));
    }
    flows.insert(String::new(), all.clone());
    let mut detail = serde_json::Map::new();
    for i in all["instances"].as_array().unwrap() {
        let id = i["instance"].as_str().unwrap();
        detail.insert(
            id.into(),
            store
                .answer(&PkiQuery::Flow {
                    instance: id.into(),
                    t_ns: None,
                })
                .unwrap(),
        );
    }
    let mut out = serde_json::json!({
        "inspect.trust": trust,
        "inspect.flows": flows,
        "inspect.flow": detail,
    });
    if pdus {
        let mut answers = serde_json::Map::new();
        let mut know = serde_json::Map::new();
        for r in rows.iter().filter(|r| r["channel"] == "pki.flow") {
            for p in r["record"]["pdus"].as_array().into_iter().flatten() {
                let id = p["id"].as_str().unwrap();
                let mut views = serde_json::Map::new();
                for v in STUDIO_VIEWS {
                    let q = PkiQuery::Pdu {
                        id: id.into(),
                        view: v.into(),
                    };
                    views.insert(v.into(), store.answer(&q).unwrap());
                }
                answers.insert(id.into(), Value::Object(views));
            }
        }
        for e in STUDIO_VIEWS.iter().skip(1) {
            let mut views = serde_json::Map::new();
            for v in ["ground-truth", e] {
                let q = PkiQuery::Knowledge {
                    entity: (*e).into(),
                    view: v.into(),
                };
                if let Some(a) = store.answer(&q) {
                    views.insert(v.into(), a);
                }
            }
            know.insert((*e).into(), Value::Object(views));
        }
        out["inspect.pdu"] = Value::Object(answers);
        out["inspect.knowledge"] = Value::Object(know);
    }
    out
}

/// `pki-studio.json`: the server's answers to the Studio's PKI queries on both fixtures,
/// which the Studio's tests (vitest and Playwright) serve in place of an engine. Regenerate
/// with `V2XW_PKI_STUDIO=1 cargo test -p v2xw-server --test pki studio_bundle`; otherwise
/// the committed bundle must be what the server answers now.
#[test]
fn studio_bundle_is_what_the_server_answers() {
    let (run_store, run_rows) = loaded();
    let text = std::fs::read_to_string(fixtures().join("pki-sequence.jsonl")).expect("the fixture");
    let seq_rows: Vec<Value> = text
        .lines()
        .map(|l| serde_json::from_str(l).expect("json"))
        .collect();
    let (seq_store, _) = sequence_store();
    let bundle = serde_json::json!({
        "about": "WP-E2: v2xw-server's answers to inspect.trust, inspect.flows, inspect.flow \
                  (both fixtures) and inspect.pdu, inspect.knowledge (sequence) on the WP-E1 \
                  fixtures; written and checked by crates/v2xw-server/tests/pki.rs",
        "run": studio_answers(&run_store, &run_rows, false),
        "sequence": studio_answers(&seq_store, &seq_rows, true),
    });
    let path = fixtures().join("pki-studio.json");
    if std::env::var_os("V2XW_PKI_STUDIO").is_some() {
        std::fs::write(&path, serde_json::to_string(&bundle).unwrap()).expect("write");
        return;
    }
    let committed: Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .expect("pki-studio.json: regenerate it (V2XW_PKI_STUDIO=1)"),
    )
    .expect("json");
    assert!(
        committed == bundle,
        "pki-studio.json is stale: regenerate it (V2XW_PKI_STUDIO=1)"
    );
}
