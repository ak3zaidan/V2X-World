//! The UI's engine requests on the backend's components, answered by the PKI store:
//! U2 the CRL Generator's published and pending
//! entries with each version's signing instant and `nextCrl`, U11 each MA's reports by
//! route (direct or shared by another provider's MA), U12 each case's outcome and
//! decision instant.
//!
//! Each fact is joined from records a run already writes (`pki.state`, `pki.flow`,
//! `pki.case`): a new field on a record would move the G-SCMS digests, which §3 forbids
//! (UI U1 note). Two inputs: the committed two-provider fixture
//! (`ui/apps/studio/test/fixtures/pki/pki-run.jsonl`), checked against counts the test
//! takes from its rows, and short hand-built record streams for what the fixture's slice
//! does not hold (a pending entry, a shared report, a linked or refused case).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};
use v2xw_server::pki::{PkiQuery, PkiStore};

fn fixture_rows() -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ui/apps/studio/test/fixtures/pki/pki-run.jsonl");
    std::fs::read_to_string(path)
        .expect("the fixture")
        .lines()
        .map(|l| serde_json::from_str(l).expect("a JSON line"))
        .collect()
}

fn absorb(store: &mut PkiStore, channel: &str, record: &Value) {
    let bytes = serde_json::to_vec(record).expect("json");
    assert!(store.absorb(channel, &bytes), "{channel} is a PKI channel");
}

fn loaded() -> (PkiStore, Vec<Value>) {
    let rows = fixture_rows();
    let mut store = PkiStore::default();
    for row in &rows {
        absorb(
            &mut store,
            row["channel"].as_str().expect("channel"),
            &row["record"],
        );
    }
    assert!(store.undecodable.is_empty(), "{:?}", store.undecodable);
    (store, rows)
}

fn crls(store: &PkiStore) -> Value {
    store.answer(&PkiQuery::Crls).expect("crls answers")
}

fn cases(store: &PkiStore, view: &str) -> Value {
    store
        .answer(&PkiQuery::Cases { view: view.into() })
        .expect("cases answers")
}

fn trust(store: &PkiStore, t_ns: Option<u64>) -> Value {
    store
        .answer(&PkiQuery::Trust { t_ns })
        .expect("trust answers")
}

/// One component of a snapshot.
fn component(provider: &str, role: &str, counters: Value) -> Value {
    json!({
        "id": format!("{provider}/{role}/0"), "provider": provider, "role": role,
        "instance": 0, "name": role, "status": "online", "node": null, "cert": null,
        "queue": null, "traffic": {"received": 0, "sent": 0, "bytes_in": 0, "bytes_out": 0},
        "ops": {}, "counters": counters,
    })
}

/// A CRL head (`pki_view::CrlHead`).
#[allow(clippy::too_many_arguments)]
fn head(
    provider: &str,
    series: u16,
    version: u32,
    issued_at: u64,
    issue_date: u32,
    next_crl: u32,
    entries: u32,
    bytes: u32,
) -> Value {
    json!({
        "provider": provider, "series": series,
        "kind": if series == 1 { "linkage" } else { "hash" }, "delta": false,
        "version": version, "issue_date": issue_date, "next_crl": next_crl,
        "entries": entries, "bytes": bytes, "issued_at": issued_at, "sla_s": 0,
        "id8": format!("{provider:.10}{series:02x}{version:04x}"),
    })
}

/// A `pki.state` with, per provider, `(label, crlg entries, crl-store entries, MA
/// reports_received)`.
fn state(t: u64, providers: &[(&str, u64, u64, u64)], heads: Vec<Value>) -> Value {
    let providers: Vec<Value> = providers
        .iter()
        .map(|(label, gen_entries, store_entries, received)| {
            json!({
                "label": label,
                "components": [
                    component(label, "ma", json!({"reports_received": received})),
                    component(label, "crlg",
                              json!({"entries": gen_entries, "versions_signed": 2})),
                    component(label, "crl-store",
                              json!({"entries": store_entries, "versions": 1})),
                ],
            })
        })
        .collect();
    json!({
        "schema": 1, "t": t, "time32": 1_000 + t / 1_000_000_000, "governance": [],
        "providers": providers, "ctl": {"roots": []}, "crls": heads, "certificates": {},
    })
}

/// A `pki.flow` message or stage of the report flow.
fn report_flow(kind: &str, name: &str, provider: &str, run: u32, t: u64) -> Value {
    json!({
        "schema": 1, "kind": kind, "id": format!("{provider}/{kind}/{name}/{run}"), "t": t,
        "provider": provider, "flow": "report",
        "instance": format!("{provider}/report/{run}"), "name": name,
    })
}

fn case_row(provider: &str, run: u32, stage: &str, t: u64) -> Value {
    json!({
        "schema": 1, "case": format!("{provider}/case/{run}"), "provider": provider,
        "stage": stage, "t": t,
        "instance": format!("{provider}/linkage-resolution/{run}"), "detail": {},
    })
}

/// **U2, against the fixture.** Per provider, `crlg` splits the Generator's entries into
/// those the CRL Store holds (published) and the rest (pending), and names its newest
/// version's signing instant, `nextCrl` instant and bytes. Expected values come from the
/// newest `pki.state` row's own components and CRL heads.
///
/// Fault: `crlg_at` reports `entries_pending` as 0 -> red (the hand-built test);
/// `signed_ns` taken from `issue_date` -> red.
#[test]
fn the_crl_generator_names_its_published_versions() {
    let (store, rows) = loaded();
    let newest = rows
        .iter()
        .rev()
        .find(|r| r["channel"] == "pki.state")
        .map(|r| &r["record"])
        .expect("a pki.state");
    let mut heads: BTreeMap<String, Value> = BTreeMap::new();
    for r in rows.iter().filter(|r| r["channel"] == "pki.state") {
        for h in r["record"]["crls"].as_array().into_iter().flatten() {
            heads.insert(h["id8"].as_str().unwrap().to_string(), h.clone());
        }
    }
    let got = crls(&store);
    let crlg = got["crlg"].as_array().expect("crlg");
    assert_eq!(crlg.len(), newest["providers"].as_array().unwrap().len());
    for p in newest["providers"].as_array().unwrap() {
        let label = p["label"].as_str().unwrap();
        let comp = |role: &str| {
            p["components"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["role"] == role)
                .unwrap()["counters"]
                .clone()
        };
        let row = crlg.iter().find(|c| c["provider"] == label).expect(label);
        let total = comp("crlg")["entries"].as_u64().unwrap();
        let published = comp("crl-store")["entries"].as_u64().unwrap();
        assert_eq!(row["entries"], total, "{row}");
        assert_eq!(row["entries_published"], published, "{row}");
        assert_eq!(row["entries_pending"], total - published, "{row}");
        let newest_linkage = heads
            .values()
            .filter(|h| h["provider"] == label && h["series"] == 1)
            .max_by_key(|h| h["issued_at"].as_u64().unwrap())
            .expect("a linkage list");
        assert_eq!(row["last_signed_ns"], newest_linkage["issued_at"], "{row}");
        assert_eq!(row["crl_bytes"], newest_linkage["bytes"], "{row}");
        let validity = newest_linkage["next_crl"].as_u64().unwrap()
            - newest_linkage["issue_date"].as_u64().unwrap();
        assert_eq!(
            row["next_crl_ns"].as_u64().unwrap(),
            newest_linkage["issued_at"].as_u64().unwrap() + validity * 1_000_000_000,
            "{row}"
        );
    }
    // Every version, with its own signing and nextCrl instants.
    for h in got["crls"].as_array().unwrap() {
        let raw = &heads[h["id8"].as_str().unwrap()];
        assert_eq!(h["signed_ns"], raw["issued_at"], "{h}");
        let validity = raw["next_crl"].as_u64().unwrap() - raw["issue_date"].as_u64().unwrap();
        assert_eq!(
            h["next_crl_ns"].as_u64().unwrap(),
            raw["issued_at"].as_u64().unwrap() + validity * 1_000_000_000
        );
    }
}

/// **U2, pending entries.** A case's entry appended to the Generator and not yet on the
/// list the Store holds is pending; once the Store holds it, it is published. The trust
/// view carries the split on the Generator's component at each snapshot.
///
/// Fault: `crlg_at` reports `entries_pending` as 0 -> red.
#[test]
fn an_entry_waiting_for_the_publication_is_pending() {
    let mut store = PkiStore::default();
    let h1 = head("provider-a", 1, 1, 0, 1_000, 1_000 + 604_800, 0, 279);
    absorb(
        &mut store,
        "pki.state",
        &state(0, &[("provider-a", 0, 0, 0)], vec![h1.clone()]),
    );
    // Two entries appended (two cases), the cadence publication not yet signed.
    absorb(
        &mut store,
        "pki.state",
        &state(10_000_000_000, &[("provider-a", 2, 0, 0)], vec![h1]),
    );
    let pending = trust(&store, Some(10_000_000_000));
    let gen_counters = &pending["derived"]["provider-a/crlg/0"];
    assert_eq!(gen_counters["entries_published"], 0, "{gen_counters}");
    assert_eq!(gen_counters["entries_pending"], 2, "{gen_counters}");
    assert_eq!(gen_counters["last_signed_ns"], 0, "{gen_counters}");
    // Signed at 20.25 s and received by the Store.
    let h2 = head(
        "provider-a",
        1,
        2,
        20_250_000_000,
        1_020,
        1_020 + 604_800,
        2,
        361,
    );
    absorb(
        &mut store,
        "pki.state",
        &state(21_000_000_000, &[("provider-a", 2, 2, 0)], vec![h2]),
    );
    let row = &crls(&store)["crlg"][0];
    assert_eq!(row["entries_pending"], 0, "{row}");
    assert_eq!(row["entries_published"], 2, "{row}");
    assert_eq!(row["last_signed_ns"], 20_250_000_000u64, "{row}");
    assert_eq!(
        row["next_crl_ns"],
        20_250_000_000u64 + 604_800 * 1_000_000_000,
        "{row}"
    );
    assert_eq!(row["crl_bytes"], 361, "{row}");
    // The earlier snapshot still answers as it was.
    let then = trust(&store, Some(10_000_000_000));
    assert_eq!(then["derived"]["provider-a/crlg/0"]["entries_pending"], 2);
}

/// **U11, against the fixture.** No report in the fixture's slice was shared, so every
/// report an MA received came the direct way: `received_direct` is each MA's own
/// `reports_received` and `received_shared` is 0, on the trust view's MA component and in
/// the cases answer.
///
/// Fault: `ma_received_at` returns a constant 0 direct -> red.
#[test]
fn every_report_of_the_fixture_reached_its_ma_directly() {
    let (store, rows) = loaded();
    let newest = rows
        .iter()
        .rev()
        .find(|r| r["channel"] == "pki.state")
        .map(|r| &r["record"])
        .unwrap();
    let got = cases(&store, "ground-truth");
    let ma = got["ma"].as_array().expect("ma");
    assert_eq!(ma.len(), 2, "{ma:?}");
    let view = trust(&store, None);
    for (k, p) in newest["providers"].as_array().unwrap().iter().enumerate() {
        let label = p["label"].as_str().unwrap();
        let received = p["components"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["role"] == "ma")
            .unwrap()["counters"]["reports_received"]
            .as_u64()
            .unwrap();
        assert!(received > 0, "the fixture's MAs received reports");
        let row = ma.iter().find(|m| m["provider"] == label).unwrap();
        assert_eq!(row["received_direct"], received, "{row}");
        assert_eq!(row["received_shared"], 0, "{row}");
        let derived = &view["derived"][format!("{label}/ma/0")];
        assert_eq!(derived["received_direct"], received, "{derived}");
        assert_eq!(derived["received_shared"], 0, "{derived}");
        // The snapshot itself stays as recorded.
        assert!(
            view["state"]["providers"][k]["components"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c["counters"].get("received_direct").is_none())
        );
    }
    // A component's own view sees only its own MA.
    let own = cases(&store, "provider-b/ma/0");
    let rows: Vec<&Value> = own["ma"].as_array().unwrap().iter().collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["provider"], "provider-b");
}

/// **U11, a shared report.** A report another provider's MA shared opens its run with a
/// `report-shared` message; its `report_received` stamp counts as shared, the rest of
/// the MA's `reports_received` as direct. A shared report the MA refused (no stamp)
/// counts as neither, and the counts are as of each snapshot.
///
/// Fault: `absorb_report` never counts a stamp as shared -> red.
#[test]
fn a_report_another_ma_shared_is_counted_apart() {
    let mut store = PkiStore::default();
    let b = "provider-b";
    absorb(&mut store, "pki.state", &state(0, &[(b, 0, 0, 0)], vec![]));
    // A direct report: the RA's forward, then the MA's stamp at 3 s.
    absorb(
        &mut store,
        "pki.flow",
        &report_flow("msg", "report-forward", b, 7, 2_900_000_000),
    );
    absorb(
        &mut store,
        "pki.flow",
        &report_flow("stage", "report_received", b, 7, 3_000_000_000),
    );
    // A shared one, stamped at 5 s.
    absorb(
        &mut store,
        "pki.flow",
        &report_flow("msg", "report-shared", b, 9, 4_990_000_000),
    );
    absorb(
        &mut store,
        "pki.flow",
        &report_flow("stage", "report_received", b, 9, 5_000_000_000),
    );
    // A shared one the MA refused (its evidence did not verify): no stamp.
    absorb(
        &mut store,
        "pki.flow",
        &report_flow("msg", "report-shared", b, 11, 5_500_000_000),
    );
    absorb(
        &mut store,
        "pki.state",
        &state(4_000_000_000, &[(b, 0, 0, 1)], vec![]),
    );
    absorb(
        &mut store,
        "pki.state",
        &state(6_000_000_000, &[(b, 0, 0, 2)], vec![]),
    );
    let row = &cases(&store, "ground-truth")["ma"][0];
    assert_eq!(row["received_direct"], 1, "{row}");
    assert_eq!(row["received_shared"], 1, "{row}");
    let at4 = trust(&store, Some(4_000_000_000));
    let c = &at4["derived"]["provider-b/ma/0"];
    assert_eq!(
        (c["received_direct"].clone(), c["received_shared"].clone()),
        (json!(1), json!(0))
    );
    let at6 = trust(&store, Some(6_000_000_000));
    let c = &at6["derived"]["provider-b/ma/0"];
    assert_eq!(
        (c["received_direct"].clone(), c["received_shared"].clone()),
        (json!(1), json!(1))
    );
}

/// **U12, against the fixture.** Every case carries its outcome and the MA's decision
/// instant: the fixture's two cases reached a published CRL, so both are `revoked`,
/// decided at their `decision` row and closed at their `crl-published` row.
///
/// Fault: `case_outcome` returns `open` for every case -> red.
#[test]
fn a_published_case_is_revoked_at_its_decision() {
    let (store, rows) = loaded();
    let mut stage_t: BTreeMap<(String, String), u64> = BTreeMap::new();
    for r in rows.iter().filter(|r| r["channel"] == "pki.case") {
        let c = &r["record"];
        stage_t
            .entry((
                c["case"].as_str().unwrap().into(),
                c["stage"].as_str().unwrap().into(),
            ))
            .or_insert(c["t"].as_u64().unwrap());
    }
    let got = cases(&store, "ground-truth");
    let list = got["cases"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    for c in list {
        let id = c["case"].as_str().unwrap().to_string();
        assert_eq!(c["outcome"], "revoked", "{id}");
        assert_eq!(
            c["decided_at_ns"],
            stage_t[&(id.clone(), "decision".into())],
            "{id}"
        );
        assert_eq!(
            c["closed_at_ns"],
            stage_t[&(id.clone(), "crl-published".into())],
            "{id}"
        );
        assert_eq!(c["stage"], "crl-published", "{id}");
    }
    assert_eq!(got["detection_pending"], 0);
    assert_eq!(got["outcomes"]["revoked"], 2);
    // An unrelated component sees no outcome.
    let other = cases(&store, "provider-a/ra/0");
    for c in other["cases"].as_array().unwrap() {
        assert!(c.get("outcome").is_none(), "{c}");
    }
}

/// **U12, every outcome.** A case whose chain the MA had already decided is `linked`, one
/// that ended without an entry `unresolved`, and one still at the LAs `open` (counted in
/// `detection_pending`), each with its decision instant.
///
/// Fault: `case_outcome` ignores the `linked` stage -> red.
#[test]
fn each_case_outcome_is_named() {
    let mut store = PkiStore::default();
    let a = "provider-a";
    absorb(&mut store, "pki.state", &state(0, &[(a, 0, 0, 0)], vec![]));
    for (run, stages) in [
        (
            1,
            vec![
                ("reports-in", 1),
                ("corroborated", 1),
                ("decision", 2),
                ("linked", 4),
            ],
        ),
        (
            2,
            vec![
                ("reports-in", 1),
                ("decision", 3),
                ("la-seeds", 5),
                ("refused", 6),
            ],
        ),
        (
            3,
            vec![
                ("reports-in", 2),
                ("corroborated", 2),
                ("decision", 4),
                ("plv-lookup", 5),
            ],
        ),
    ] {
        for (stage, s) in stages {
            absorb(
                &mut store,
                "pki.case",
                &case_row(a, run, stage, s * 1_000_000_000),
            );
        }
    }
    let got = cases(&store, "provider-a/ma/0");
    let by: BTreeMap<String, &Value> = got["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["case"].as_str().unwrap().to_string(), c))
        .collect();
    let s = 1_000_000_000u64;
    let expect = [
        ("provider-a/case/1", "linked", 2 * s, Some(4 * s)),
        ("provider-a/case/2", "unresolved", 3 * s, Some(6 * s)),
        ("provider-a/case/3", "open", 4 * s, None),
    ];
    for (id, outcome, decided, closed) in expect {
        let c = by[id];
        assert_eq!(c["outcome"], outcome, "{c}");
        assert_eq!(c["decided_at_ns"], decided, "{c}");
        assert_eq!(c["closed_at_ns"], json!(closed), "{c}");
    }
    assert_eq!(by["provider-a/case/3"]["stage"], "plv-lookup");
    assert_eq!(got["detection_pending"], 1);
}
