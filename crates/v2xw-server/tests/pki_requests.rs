//! The UI's engine requests answered by the PKI store (wave D, `10-completion-specs.md`
//! §3): U1 a held CRL's provider and id8, U4 a vehicle's `in_run`/`left_at_ns` and its
//! enrolment certificate's end, U6 a roadside unit's device, U9 authority certificates
//! kept apart from the bounded device store.
//!
//! The recording is the committed two-provider SCMS fixture
//! (`ui/apps/studio/test/fixtures/pki/pki-run.jsonl`, written by
//! `v2xw-engine/tests/pki_view.rs`). Each test checks a field against a count taken from
//! the records themselves, not from the store's own code.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};
use v2xw_server::pki::{CERT_CAP, PkiQuery, PkiStore};

fn fixture_rows() -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ui/apps/studio/test/fixtures/pki/pki-run.jsonl");
    std::fs::read_to_string(path)
        .expect("the fixture")
        .lines()
        .map(|l| serde_json::from_str(l).expect("a JSON line"))
        .collect()
}

fn loaded() -> (PkiStore, Vec<Value>) {
    let rows = fixture_rows();
    let mut store = PkiStore::default();
    for row in &rows {
        let channel = row["channel"].as_str().expect("channel");
        let bytes = serde_json::to_vec(&row["record"]).expect("json");
        assert!(store.absorb(channel, &bytes), "{channel} is a PKI channel");
    }
    (store, rows)
}

fn gt() -> String {
    "ground-truth".into()
}

fn vehicle(store: &PkiStore, device: &str) -> Value {
    store
        .answer(&PkiQuery::Vehicle {
            device: device.into(),
            view: gt(),
        })
        .expect("the vehicle answers")
}

/// The newest `node.credentials` per device.
fn last_credentials(rows: &[Value]) -> BTreeMap<String, Value> {
    rows.iter()
        .filter(|r| r["channel"] == "node.credentials")
        .map(|r| {
            (
                r["record"]["device"].as_str().unwrap().to_string(),
                r["record"].clone(),
            )
        })
        .collect()
}

/// Every CRL version the fixture published, with its bytes.
fn published_crls(rows: &[Value]) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    for r in rows.iter().filter(|r| r["channel"] == "pki.state") {
        for h in r["record"]["crls"].as_array().into_iter().flatten() {
            if h["hex"].is_string() && !out.iter().any(|x| x["id8"] == h["id8"]) {
                out.push(h.clone());
            }
        }
    }
    out
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

/// **U1.** A `node.security` row's held CRLs come back named by the list's provider and
/// id8. The rows are what a device's receiver files a CRL under (`HeldCrlView`), built
/// from each published list's own bytes decoded here; the provider and the id8 to expect
/// are the publishing `pki.state` head's. A list nobody published names nothing.
///
/// Fault: `named_crls` leaves `crl_id8` null -> red.
#[test]
fn a_held_crl_is_named_by_its_provider_and_id8() {
    use v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::TypeSpecificCrlContents as T;
    let (mut store, rows) = loaded();
    let heads = published_crls(&rows);
    assert!(heads.len() >= 2, "the fixture publishes CRLs");
    let mut held = Vec::new();
    for h in &heads {
        let opened =
            v2xw_sec::crl::decode_crl(&unhex(h["hex"].as_str().unwrap())).expect("decodes");
        let c = &opened.contents;
        let counter = match &c.type_specific {
            T::fullHashCrl(x) | T::deltaHashCrl(x) => x.crl_serial.0,
            T::fullLinkedCrl(l) | T::deltaLinkedCrl(l) => {
                u32::from(l.i_rev.0.0) * 256 + u32::from(l.index_within_i.0)
            }
            _ => 0,
        };
        let craca: String = c.crl_craca.0.iter().map(|b| format!("{b:02x}")).collect();
        held.push(json!({
            "craca": craca, "series": c.crl_series.0.0, "counter": counter,
            "issue_date": c.issue_date.0.0, "next_crl": c.next_crl.0.0,
        }));
    }
    // One list no `pki.state` ever carried.
    held.push(
        json!({"craca": "00000000000000aa", "series": 1, "counter": 7,
                     "issue_date": 1, "next_crl": 2}),
    );
    let (device, creds) = last_credentials(&rows)
        .into_iter()
        .next()
        .expect("a vehicle");
    store.absorb_security(&json!({"node": creds["node"], "t": creds["t"], "crls": held}));

    let row = vehicle(&store, &device);
    let got = row["security"]["crls"].as_array().expect("security.crls");
    assert_eq!(got.len(), heads.len() + 1);
    for (h, g) in heads.iter().zip(got) {
        assert_eq!(g["crl_id8"], h["id8"], "the held list's id8: {g}");
        assert_eq!(
            g["provider"], h["provider"],
            "the held list's provider: {g}"
        );
    }
    let unknown = got.last().unwrap();
    assert!(
        unknown["crl_id8"].is_null() && unknown["provider"].is_null(),
        "{unknown}"
    );
}

/// **U4.** Every vehicle row says whether it is still in the run and when it left. The
/// fixture's vehicles all stay; a last `node.credentials` that says `departed` takes one
/// out at that record's instant.
///
/// Fault: `in_run` always true -> red.
#[test]
fn a_departed_vehicle_is_out_of_the_run_from_its_last_record() {
    let (mut store, rows) = loaded();
    let last = last_credentials(&rows);
    let staying = last
        .values()
        .filter(|c| c["departed"] != json!(true))
        .count();
    let table = store.answer(&PkiQuery::Vehicles { view: gt() }).unwrap();
    let in_run = table["vehicles"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["in_run"] == json!(true) && v["left_at_ns"].is_null())
        .count();
    assert_eq!(
        in_run, staying,
        "every vehicle whose last record did not depart"
    );

    let (device, mut gone) = last.into_iter().next().unwrap();
    let left = gone["t"].as_u64().unwrap() + 5_000_000_000;
    gone["t"] = json!(left);
    gone["departed"] = json!(true);
    gone["active"] = Value::Null;
    gone["certificates"] = json!({});
    assert!(store.absorb("node.credentials", &serde_json::to_vec(&gone).unwrap()));
    let row = vehicle(&store, &device);
    assert_eq!(row["in_run"], json!(false), "{device} left");
    assert_eq!(row["left_at_ns"], json!(left));
}

/// **U4.** Every vehicle row carries its enrolment certificate's end, Time32 and run
/// instant, as `start + duration` of the certificate's own `validityPeriod` read from
/// `inspect.cert`'s decoded tree.
///
/// Fault: `enrolment_valid_until` emitted as 0 -> red.
#[test]
fn every_vehicle_row_carries_its_enrolment_certificate_end() {
    let (store, rows) = loaded();
    let state = rows
        .iter()
        .rev()
        .find(|r| r["channel"] == "pki.state")
        .map(|r| &r["record"])
        .unwrap();
    let (t, now32) = (
        state["t"].as_u64().unwrap(),
        state["time32"].as_i64().unwrap(),
    );
    let mut checked = 0;
    for (device, c) in last_credentials(&rows) {
        let Some(ec) = c["enrolment"].as_str() else {
            continue;
        };
        let cert = store
            .answer(&PkiQuery::Cert {
                id8: ec.into(),
                t_ns: None,
            })
            .expect("the enrolment certificate answers");
        let v = &cert["tree"]["toBeSigned"]["validityPeriod"];
        let start = v["start"].as_u64().expect("start");
        let (unit, n) = v["duration"].as_object().unwrap().iter().next().unwrap();
        let n = n.as_u64().unwrap();
        let secs = match unit.as_str() {
            "seconds" => n,
            "minutes" => n * 60,
            "hours" => n * 3_600,
            "sixtyHours" => n * 216_000,
            "years" => n * 31_556_952,
            other => panic!("unit {other}"),
        };
        let end = start + secs;
        let row = vehicle(&store, &device);
        assert_eq!(row["enrolment_valid_until"], json!(end), "{device}");
        let ns = i128::from(t) + (i128::from(end) - i128::from(now32)) * 1_000_000_000;
        assert_eq!(
            row["enrolment_valid_until_ns"],
            json!(u64::try_from(ns).unwrap())
        );
        checked += 1;
    }
    assert!(checked > 0, "the fixture's vehicles are enrolled");
}

/// **U6.** A device id names its kind and its node: every vehicle's is the node its own
/// `node.credentials` gives, and a roadside unit's (`ee/<SCMS_DEVICE_BASE + node>` with
/// the node in the roadside range) is `kind: rsu` with that node and its unit index.
///
/// Fault: `device_ref` calls every device a vehicle -> red.
#[test]
fn a_roadside_device_is_named_as_its_unit() {
    use v2xw_engine::phase2::SCMS_DEVICE_BASE;
    use v2xw_engine::run::roadside_node;
    let (mut store, rows) = loaded();
    let flow = rows
        .iter()
        .find(|r| {
            r["channel"] == "pki.flow"
                && r["record"]["to"]
                    .as_str()
                    .is_some_and(|t| t.starts_with("ee/"))
        })
        .map(|r| r["record"].clone())
        .expect("a message to a device");
    let rsu = roadside_node(3).index();
    let rsu_device = format!("ee/{}", SCMS_DEVICE_BASE + rsu);
    let mut f = flow.clone();
    f["instance"] = json!("u6-roadside");
    f["to"] = json!(rsu_device);
    f["pdus"] = json!([]);
    assert!(store.absorb("pki.flow", &serde_json::to_vec(&f).unwrap()));

    let detail = store
        .answer(&PkiQuery::Flow {
            instance: "u6-roadside".into(),
            t_ns: None,
        })
        .unwrap();
    assert_eq!(
        detail["device_kinds"][&rsu_device],
        json!({"kind": "rsu", "node": rsu, "rsu": 3}),
        "{detail}"
    );
    let list = store
        .answer(&PkiQuery::Flows {
            flow: None,
            device: Some(rsu_device.clone()),
            provider: None,
            limit: 10,
            t_ns: None,
        })
        .unwrap();
    assert_eq!(
        list["instances"][0]["device_kinds"][&rsu_device]["kind"],
        "rsu"
    );

    let mut vehicles = 0;
    for device in last_credentials(&rows).keys() {
        let row = vehicle(&store, device);
        assert_eq!(row["kind"], "vehicle", "{device}");
        vehicles += 1;
    }
    // Every vehicle device in any flow names its own node.
    for r in rows.iter().filter(|r| r["channel"] == "node.credentials") {
        let d = r["record"]["device"].as_str().unwrap().to_string();
        let list = store
            .answer(&PkiQuery::Flows {
                flow: None,
                device: Some(d.clone()),
                provider: None,
                limit: 1,
                t_ns: None,
            })
            .unwrap();
        if let Some(i) = list["instances"].as_array().unwrap().first() {
            let id = i["instance"].as_str().unwrap();
            let detail = store
                .answer(&PkiQuery::Flow {
                    instance: id.into(),
                    t_ns: None,
                })
                .unwrap();
            assert_eq!(
                detail["device_kinds"][&d],
                json!({"kind": "vehicle", "node": r["record"]["node"]}),
                "{d}"
            );
        }
    }
    assert!(vehicles > 0);
}

/// **U9.** Authority certificates are kept apart from the bounded device store: after
/// more device certificates than the store holds, every certificate a `pki.state`
/// carried (roots, ICAs, the components) still opens.
///
/// Fault: `absorb_state` files the authority certificates in the device ring -> red.
#[test]
fn authority_certificates_outlive_the_device_ring() {
    let (mut store, rows) = loaded();
    let authority: Vec<String> = rows
        .iter()
        .filter(|r| r["channel"] == "pki.state")
        .flat_map(|r| {
            r["record"]["certificates"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
        })
        .collect();
    assert!(
        authority.len() >= 10,
        "the fixture carries the authority certificates"
    );
    let creds = last_credentials(&rows);
    let (_, mut flood) = creds.into_iter().next().unwrap();
    let mut certs = serde_json::Map::new();
    for k in 0..=CERT_CAP as u64 {
        certs.insert(format!("ff{k:014x}"), json!("00"));
    }
    flood["certificates"] = Value::Object(certs);
    flood["cells"] = json!([]);
    assert!(store.absorb("node.credentials", &serde_json::to_vec(&flood).unwrap()));
    for id8 in &authority {
        assert!(
            store
                .answer(&PkiQuery::Cert {
                    id8: id8.clone(),
                    t_ns: None,
                })
                .is_some(),
            "authority certificate {id8} was evicted by device certificates"
        );
    }
}
