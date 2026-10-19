//! WP-E1: `inspect.pdu`, `inspect.cert` and `inspect.knowledge` on a recorded run.
//!
//! The recording is the committed UI fixture (`ui/apps/studio/test/fixtures/pki/`),
//! written by `v2xw-engine/tests/pki_view.rs` from a two-provider Manhattan run. The
//! server's store reads its records as a live run or a replay delivers them, and every
//! answer must equal the one the engine test computed on the full recording
//! (`pki-answers.json`): the fixture is self-sufficient and the server adds nothing.

use std::collections::{BTreeMap, BTreeSet};
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
        assert!(store.absorb(&channel, &bytes), "{channel} is a PKI channel");
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

/// `inspect.trust`'s `messages` (the Studio's topology draws each along its link): at each
/// snapshot, exactly the recorded `pki.flow` messages after the previous snapshot (the
/// first: in the 2 s before it) and at or before it, in time order, each with its sender,
/// receiver, name, flow, instance and bytes, and nothing else (no SPDU, no plaintext).
#[test]
fn trust_lists_each_window_messages_exactly_as_recorded() {
    let (store, rows) = loaded();
    let states: Vec<u64> = rows
        .iter()
        .filter(|r| r["channel"] == "pki.state")
        .map(|r| r["record"]["t"].as_u64().unwrap())
        .collect();
    let msgs: Vec<&Value> = rows
        .iter()
        .filter(|r| r["channel"] == "pki.flow" && r["record"]["kind"] == "msg")
        .map(|r| &r["record"])
        .collect();
    let mut listed = 0;
    for (i, &t) in states.iter().enumerate() {
        let lo = if i == 0 {
            t.saturating_sub(2_000_000_000)
        } else {
            states[i - 1]
        };
        let mut want: Vec<&Value> = msgs
            .iter()
            .copied()
            .filter(|m| {
                let mt = m["t"].as_u64().unwrap();
                mt > lo && mt <= t
            })
            .collect();
        want.sort_by_key(|m| m["t"].as_u64().unwrap());
        let omitted = want
            .len()
            .saturating_sub(v2xw_server::pki::RECENT_PER_ANSWER);
        let want = &want[omitted..];
        let got = store.answer(&PkiQuery::Trust { t_ns: Some(t) }).unwrap();
        assert_eq!(got["messages_omitted"], omitted, "omitted at {t}");
        assert_eq!(
            got["messages_complete"], true,
            "the log reaches back at {t}"
        );
        let list = got["messages"].as_array().expect("messages");
        assert_eq!(list.len(), want.len(), "messages at {t}");
        for (g, w) in list.iter().zip(want) {
            for key in [
                "id", "t", "from", "to", "name", "flow", "instance", "provider", "bytes",
            ] {
                assert_eq!(g[key], w[key], "{key} of {} at {t}", w["id"]);
            }
            assert!(
                g.get("pdus").is_none() && g.get("hex").is_none(),
                "no SPDU in {g}"
            );
        }
        listed += list.len();
    }
    assert!(
        listed > 20,
        "only {listed} messages listed: the check would pass on nothing"
    );
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

// ---------------------------------------------------------------------------------------
// WP-E3: `inspect.pki`, the Vehicles, Cases and Revocation-lists tabs
// ---------------------------------------------------------------------------------------

fn pki(store: &PkiStore, q: PkiQuery) -> Value {
    store
        .answer(&q)
        .unwrap_or_else(|| panic!("{q:?} unanswered"))
}

fn gt() -> String {
    "ground-truth".to_string()
}

/// The newest `node.credentials` per device in the fixture.
fn last_credentials(rows: &[Value]) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for r in rows.iter().filter(|r| r["channel"] == "node.credentials") {
        out.insert(
            r["record"]["device"].as_str().expect("device").to_string(),
            r["record"].clone(),
        );
    }
    out
}

/// Every case the MA's ground truth accuses in a case that reached a published CRL.
fn accused_of_published_cases(rows: &[Value]) -> BTreeSet<String> {
    let published: BTreeSet<&str> = rows
        .iter()
        .filter(|r| r["channel"] == "pki.case" && r["record"]["stage"] == "crl-published")
        .filter_map(|r| r["record"]["case"].as_str())
        .collect();
    let mut out = BTreeSet::new();
    for r in rows
        .iter()
        .filter(|r| r["channel"] == "pki.knowledge" && r["record"]["role"] == "ma")
    {
        for c in r["record"]["truth"]["cases"]
            .as_array()
            .expect("truth.cases")
        {
            if published.contains(c["case"].as_str().expect("case")) {
                for a in c["accused"].as_array().expect("accused") {
                    out.insert(a.as_str().expect("device").to_string());
                }
            }
        }
    }
    out
}

#[test]
fn the_vehicle_grid_is_the_recorded_node_credentials_cell_for_cell() {
    let (store, rows) = loaded();
    let last = last_credentials(&rows);
    assert!(last.len() >= 4, "{} vehicles", last.len());
    let list = pki(&store, PkiQuery::Vehicles { view: gt() });
    assert_eq!(list["vehicles"].as_array().expect("rows").len(), last.len());
    for (device, rec) in &last {
        let v = pki(
            &store,
            PkiQuery::Vehicle {
                device: device.clone(),
                view: gt(),
            },
        );
        // The grid comes back reduced to the held cells plus its bounds; rebuilding the
        // full grid from that must give the recorded one exactly.
        let grid = &v["grid"];
        let (i_min, i_max, jmax) = (
            grid["i_min"].as_u64().expect("i_min"),
            grid["i_max"].as_u64().expect("i_max"),
            grid["jmax"].as_u64().expect("jmax"),
        );
        let held: BTreeMap<(u64, u64), &Value> = grid["held"]
            .as_array()
            .expect("held")
            .iter()
            .map(|c| {
                (
                    (c["i"].as_u64().expect("i"), c["j"].as_u64().expect("j")),
                    c,
                )
            })
            .collect();
        let cells = rec["cells"].as_array().expect("cells");
        assert_eq!(grid["cells"].as_u64(), Some(cells.len() as u64), "{device}");
        let mut rebuilt = Vec::new();
        for i in i_min..=i_max {
            for j in 0..jmax {
                rebuilt.push(held.get(&(i, j)).map_or_else(
                    || serde_json::json!({"i": i, "j": j, "state": "not-yet-available", "used": false}),
                    |c| (*c).clone(),
                ));
            }
        }
        assert_eq!(
            &rebuilt, cells,
            "{device}: the grid differs from node.credentials"
        );
        // Counts by state, the active certificate and its history.
        for state in ["active", "valid", "downloaded", "expired", "revoked"] {
            let n = cells.iter().filter(|c| c["state"] == state).count() as u64;
            assert_eq!(
                v["pool"][state].as_u64().unwrap_or(0),
                n,
                "{device} {state}"
            );
        }
        assert_eq!(v["active"], rec["active"], "{device}");
        assert_eq!(v["history"], rec["history"], "{device}");
        assert_eq!(v["enrolment"], rec["enrolment"], "{device}");
        assert_eq!(
            v["pseudonyms"][0]["id8"], rec["active"],
            "{device}: the first pseudonym is the one it holds"
        );
        // As the vehicle itself: the same grid, no ground truth; as another, nothing.
        let own = pki(
            &store,
            PkiQuery::Vehicle {
                device: device.clone(),
                view: device.clone(),
            },
        );
        assert_eq!(own["grid"], v["grid"]);
        assert!(
            own.get("truth").is_none(),
            "{device}: truth outside ground truth"
        );
        assert!(
            store
                .answer(&PkiQuery::Vehicle {
                    device: device.clone(),
                    view: "ee/0".into()
                })
                .is_none(),
            "{device} answered to another vehicle"
        );
    }
}

#[test]
fn every_crl_entry_decodes_and_revokes_exactly_the_accused() {
    let (store, rows) = loaded();
    let crls = pki(&store, PkiQuery::Crls);
    let heads = crls["crls"].as_array().expect("crls");
    assert!(!heads.is_empty(), "no CRL");
    // Every version the states listed, with its SLA clock against the newest state.
    let t32 = crls["time32"].as_i64().expect("time32");
    for h in heads {
        assert_eq!(
            h["sla_now_s"].as_i64(),
            Some(h["next_crl"].as_i64().expect("next") - t32),
            "{h}"
        );
    }
    let la_seeds: Vec<&Value> = rows
        .iter()
        .filter(|r| r["channel"] == "pki.case" && r["record"]["stage"] == "la-seeds")
        .map(|r| &r["record"]["detail"])
        .collect();
    let mut owners = BTreeSet::new();
    let mut entries = 0;
    for h in heads.iter().filter(|h| h["decodable"] == true) {
        let id8 = h["id8"].as_str().expect("id8").to_string();
        let v = pki(
            &store,
            PkiQuery::Crl {
                id8: id8.clone(),
                view: gt(),
            },
        );
        let linkage = v["linkage"].as_array().expect("linkage");
        assert_eq!(
            (linkage.len() + v["hash"].as_array().expect("hash").len()) as u64,
            h["entries"].as_u64().expect("entries"),
            "{id8}: the decoder's entries vs the head's count"
        );
        for e in linkage {
            entries += 1;
            // Each entry is one case's seeds: its LA ids and iRev are a case's.
            assert!(
                la_seeds.iter().any(|d| d["la_ids"]
                    == serde_json::json!([e["la1_id"], e["la2_id"]])
                    && d["i_rev"] == e["i_rev"]),
                "{id8}: entry {e} is no case's"
            );
            assert!(e["i_max"].as_u64() >= e["i_rev"].as_u64(), "{e}");
            for m in e["matches"].as_array().expect("matches") {
                let o = &v["owners"][m.as_str().expect("id8")];
                owners.insert(o["device"].as_str().expect("an owner").to_string());
            }
        }
        // Outside ground truth: the entries, never who they match.
        let public = pki(
            &store,
            PkiQuery::Crl {
                id8: id8.clone(),
                view: "provider-a/ra/0".into(),
            },
        );
        assert!(public.get("owners").is_none(), "{id8}");
        assert!(!public.to_string().contains("matches"), "{id8}");
        assert_eq!(
            public["linkage"].as_array().map(Vec::len),
            Some(linkage.len())
        );
    }
    assert!(entries > 0, "no linkage entry");
    let accused = accused_of_published_cases(&rows);
    assert!(!accused.is_empty(), "the MA's truth accuses no one");
    assert_eq!(owners, accused, "CRL matches vs the cases' accused");
    // The Vehicles table marks the same vehicles.
    let list = pki(&store, PkiQuery::Vehicles { view: gt() });
    let marked: BTreeSet<String> = list["vehicles"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|v| v["truth"]["revoked_certs"].as_u64().unwrap_or(0) > 0)
        .map(|v| v["device"].as_str().expect("device").to_string())
        .collect();
    assert_eq!(marked, accused);
}

#[test]
fn the_case_board_is_pki_case_and_each_view_sees_only_its_part() {
    let (store, rows) = loaded();
    let board = pki(&store, PkiQuery::Cases { view: gt() });
    let cases = board["cases"].as_array().expect("cases");
    let mut recorded: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for r in rows.iter().filter(|r| r["channel"] == "pki.case") {
        recorded
            .entry(r["record"]["case"].as_str().expect("case").to_string())
            .or_default()
            .push(r["record"].clone());
    }
    assert_eq!(cases.len(), recorded.len());
    for c in cases {
        let id = c["case"].as_str().expect("case");
        assert_eq!(
            c["stages"].as_array().expect("stages"),
            &recorded[id],
            "{id}"
        );
        let steps = c["steps"].as_array().expect("steps");
        let names: BTreeSet<&str> = steps.iter().filter_map(|s| s["name"].as_str()).collect();
        for n in [
            "pca-lookup-request",
            "seed-request",
            "seed-response",
            "crl-append",
        ] {
            assert!(names.contains(n), "{id}: no {n} step in {names:?}");
        }
        assert!(
            steps
                .windows(2)
                .all(|w| w[0]["t"].as_u64() <= w[1]["t"].as_u64()),
            "{id}: steps out of order"
        );
        let truth = &c["truth"];
        assert!(
            !truth["accused"].as_array().expect("accused").is_empty(),
            "{id}"
        );
        assert!(
            !truth["reporters"].as_array().expect("reporters").is_empty(),
            "{id}"
        );
        // As LA1 of the case's provider: its own steps only, no stage table, no identity.
        let p = c["provider"].as_str().expect("provider");
        let la1 = format!("{p}/la1/0");
        let as_la = pki(&store, PkiQuery::Cases { view: la1.clone() });
        let mine = as_la["cases"]
            .as_array()
            .expect("cases")
            .iter()
            .find(|x| x["case"] == id)
            .unwrap_or_else(|| panic!("{id} invisible to {la1}"));
        assert_eq!(mine["stages"], serde_json::json!([]), "{id}");
        assert!(mine.get("truth").is_none());
        for s in mine["steps"].as_array().expect("steps") {
            assert!(s["from"] == la1.as_str() || s["to"] == la1.as_str(), "{s}");
        }
        assert!(
            !mine.to_string().contains("\"ee/"),
            "{la1} sees a device: {mine}"
        );
        // As the case's MA: its table, still no identity.
        let ma = format!("{p}/ma/0");
        let as_ma = pki(&store, PkiQuery::Cases { view: ma.clone() });
        let mine = as_ma["cases"]
            .as_array()
            .expect("cases")
            .iter()
            .find(|x| x["case"] == id)
            .expect("the MA's own case");
        assert_eq!(mine["stages"].as_array(), Some(&recorded[id]));
        assert!(mine.get("truth").is_none());
        assert!(
            !mine.to_string().contains("\"ee/"),
            "{ma} sees a device: {mine}"
        );
    }
}

/// The answers of `inspect.pki` on the fixture, for the Studio's tests and the mock
/// engine: written with `V2XW_PKI_FIXTURES=<dir>`, otherwise checked against the
/// committed `pki-tabs.json`.
#[test]
fn the_tabs_fixture_is_what_the_server_answers() {
    let (store, rows) = loaded();
    let mut vehicles = serde_json::Map::new();
    for d in last_credentials(&rows).keys() {
        vehicles.insert(
            d.clone(),
            pki(
                &store,
                PkiQuery::Vehicle {
                    device: d.clone(),
                    view: gt(),
                },
            ),
        );
    }
    let crls = pki(&store, PkiQuery::Crls);
    let mut crl = serde_json::Map::new();
    for h in crls["crls"].as_array().expect("crls") {
        let id8 = h["id8"].as_str().expect("id8").to_string();
        if let Some(gtv) = store.answer(&PkiQuery::Crl {
            id8: id8.clone(),
            view: gt(),
        }) {
            let other = pki(
                &store,
                PkiQuery::Crl {
                    id8: id8.clone(),
                    view: "provider-a/ra/0".into(),
                },
            );
            crl.insert(
                id8,
                serde_json::json!({"ground-truth": gtv, "other": other}),
            );
        }
    }
    let mut cases = serde_json::Map::new();
    cases.insert(
        "ground-truth".into(),
        pki(&store, PkiQuery::Cases { view: gt() }),
    );
    for v in ["provider-a/ma/0", "provider-a/la1/0", "provider-b/ma/0"] {
        cases.insert(v.into(), pki(&store, PkiQuery::Cases { view: v.into() }));
    }
    let got = serde_json::json!({
        "vehicles": pki(&store, PkiQuery::Vehicles { view: gt() }),
        "vehicle": vehicles,
        "crls": crls,
        "crl": crl,
        "cases": cases,
    });
    let path = fixtures().join("pki-tabs.json");
    if std::env::var("V2XW_PKI_FIXTURES").is_ok() {
        std::fs::write(&path, serde_json::to_string(&got).expect("json")).expect("write");
        return;
    }
    let want: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("pki-tabs.json"))
        .expect("json");
    assert_eq!(got, want, "regenerate with V2XW_PKI_FIXTURES=1");
}
