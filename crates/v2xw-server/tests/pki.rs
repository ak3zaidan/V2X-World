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
// WP-E3: `inspect.pki`, the Vehicles, Cases and Revocation-lists tabs
// ---------------------------------------------------------------------------------------

fn pki(store: &PkiStore, q: PkiQuery) -> Value {
    store.answer(&q).unwrap_or_else(|| panic!("{q:?} unanswered"))
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
    for r in rows.iter().filter(|r| r["channel"] == "pki.knowledge" && r["record"]["role"] == "ma") {
        for c in r["record"]["truth"]["cases"].as_array().expect("truth.cases") {
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
            .map(|c| ((c["i"].as_u64().expect("i"), c["j"].as_u64().expect("j")), c))
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
        assert_eq!(&rebuilt, cells, "{device}: the grid differs from node.credentials");
        // Counts by state, the active certificate and its history.
        for state in ["active", "valid", "downloaded", "expired", "revoked"] {
            let n = cells.iter().filter(|c| c["state"] == state).count() as u64;
            assert_eq!(v["pool"][state].as_u64().unwrap_or(0), n, "{device} {state}");
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
        assert!(own.get("truth").is_none(), "{device}: truth outside ground truth");
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
                la_seeds.iter().any(|d| d["la_ids"] == serde_json::json!([e["la1_id"], e["la2_id"]])
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
        assert_eq!(public["linkage"].as_array().map(Vec::len), Some(linkage.len()));
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
        assert_eq!(c["stages"].as_array().expect("stages"), &recorded[id], "{id}");
        let steps = c["steps"].as_array().expect("steps");
        let names: BTreeSet<&str> = steps.iter().filter_map(|s| s["name"].as_str()).collect();
        for n in ["pca-lookup-request", "seed-request", "seed-response", "crl-append"] {
            assert!(names.contains(n), "{id}: no {n} step in {names:?}");
        }
        assert!(
            steps.windows(2).all(|w| w[0]["t"].as_u64() <= w[1]["t"].as_u64()),
            "{id}: steps out of order"
        );
        let truth = &c["truth"];
        assert!(!truth["accused"].as_array().expect("accused").is_empty(), "{id}");
        assert!(!truth["reporters"].as_array().expect("reporters").is_empty(), "{id}");
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
        assert!(!mine.to_string().contains("\"ee/"), "{la1} sees a device: {mine}");
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
        assert!(!mine.to_string().contains("\"ee/"), "{ma} sees a device: {mine}");
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
            crl.insert(id8, serde_json::json!({"ground-truth": gtv, "other": other}));
        }
    }
    let mut cases = serde_json::Map::new();
    cases.insert("ground-truth".into(), pki(&store, PkiQuery::Cases { view: gt() }));
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
