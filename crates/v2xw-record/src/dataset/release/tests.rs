//! The release writer on the synthetic run (`fixture::release_records`), one test per
//! table family plus the layout, identifier, leakage and determinism gates.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::*;
use crate::dataset::fixture::{DatasetShape, release_records};
use crate::reader::RecordedRecord;

fn tmp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("v2xw-release-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn meta() -> RunMeta {
    RunMeta {
        run_id: "fixture-run".into(),
        seed: "0xfixture".into(),
        name: Some("fixture".into()),
        scenario_hash: Some("00ff".into()),
        duration_s: Some(4.0),
        attack_mix: vec![AttackMix {
            id: "ConstPos".into(),
            fraction: Some(0.3),
            count: None,
            from_s: Some(1.0),
            to_s: None,
            params: vec!["magnitude_scale=1".into()],
        }],
        ..Default::default()
    }
}

fn opts() -> ReleaseOptions {
    ReleaseOptions {
        split: "train".into(),
        instrumented_receivers: 3,
        ..Default::default()
    }
}

fn release(tag: &str, shape: &DatasetShape, o: &ReleaseOptions) -> WrittenRelease {
    let recs = release_records(shape);
    let root = tmp(tag);
    write(&mut Records(&recs), &root, o, &meta()).expect("the fixture release writes")
}

fn rows_of(root: &Path, table: &str) -> Vec<Value> {
    let path = if table == "runs" {
        root.join("runs.parquet")
    } else {
        root.join(format!(
            "{table}/split=train/run=fixture-run/part-00000.parquet"
        ))
    };
    let batches = crate::export::parquet::read(&path).expect("reads back");
    let mut w = arrow::json::ArrayWriter::new(Vec::new());
    let refs: Vec<&arrow::array::RecordBatch> = batches.iter().collect();
    w.write_batches(&refs).expect("to json");
    w.finish().expect("finish");
    let bytes = w.into_inner();
    if bytes.is_empty() {
        return Vec::new();
    }
    serde_json::from_slice(&bytes).expect("json rows")
}

fn nonnull(rows: &[Value], col: &str) -> usize {
    rows.iter()
        .filter(|r| r.get(col).is_some_and(|v| !v.is_null()))
        .count()
}

#[test]
fn every_table_the_fixture_populates_has_rows() {
    let w = release("rows", &DatasetShape::default(), &opts());
    for def in all_tables() {
        assert!(w.rows(def.name) > 0, "{} is empty", def.name);
    }
    assert_eq!(w.rows("runs"), 1);
    assert_eq!(w.rows("local/rx"), w.rows("truth/receptions"));
    assert_eq!(w.rows("local/rx"), w.rows("ml/local"));
    assert_eq!(w.rows("global/reports"), w.rows("ml/global"));
}

#[test]
fn the_layout_is_schema_section_3_and_the_manifest_hashes_every_file() {
    let w = release("layout", &DatasetShape::default(), &opts());
    let root = &w.root;
    for rel in [
        "runs.parquet",
        "local/rx/split=train/run=fixture-run/part-00000.parquet",
        "global/crl/split=train/run=fixture-run/part-00000.parquet",
        "truth/timeline/split=train/run=fixture-run/part-00000.parquet",
        "ml/global/split=train/run=fixture-run/part-00000.parquet",
        "schema/local_rx.json",
        "schema/runs.json",
        "DATASHEET.md",
        "manifest.json",
    ] {
        assert!(root.join(rel).is_file(), "{rel} missing");
    }
    let m: Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let files = m["files"].as_array().unwrap();
    assert_eq!(
        files.len() + 1,
        w.files.len(),
        "manifest lists every file but itself"
    );
    for f in files {
        let bytes = std::fs::read(root.join(f["path"].as_str().unwrap())).unwrap();
        assert_eq!(
            f["sha256"].as_str().unwrap(),
            v2xw_core::hash::sha256_hex(&bytes),
            "{f}"
        );
    }
    let rx = files
        .iter()
        .find(|f| f["path"] == "local/rx/split=train/run=fixture-run/part-00000.parquet")
        .unwrap();
    assert_eq!(rx["rows"].as_u64(), Some(w.rows("local/rx")));
    let schema: Value =
        serde_json::from_slice(&std::fs::read(root.join("schema/local_rx.json")).unwrap()).unwrap();
    let pa = schema["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "pos_accuracy")
        .unwrap();
    assert!(
        pa["arrow_type"].as_str().unwrap().starts_with("Struct"),
        "{pa}"
    );
    assert_eq!(schema["visibility"], "NODE");
}

#[test]
fn local_rx_carries_the_security_group_and_native_structs() {
    let w = release("localrx", &DatasetShape::default(), &opts());
    let rows = rows_of(&w.root, "local/rx");
    let n = rows.len();
    for col in [
        "spdu_digest",
        "cert_id",
        "verify_result",
        "verify_latency_us",
        "psid",
        "cbr",
        "msg_cnt",
        "generation_time_us",
        "x_m",
        "signer_kind",
    ] {
        assert_eq!(nonnull(&rows, col), n, "{col} is null somewhere");
    }
    // the receiver context exists once the receiver has sent its own first BSM
    for col in ["range_m", "ego_x_m", "ego_heading_deg"] {
        assert!(nonnull(&rows, col) * 10 >= n * 9, "{col} is mostly null");
    }
    assert!(
        rows.iter()
            .all(|r| r["pos_accuracy"]["semi_major_m"].is_number()),
        "pos_accuracy is not a native struct"
    );
    assert!(rows.iter().all(|r| r["accel"]["long_mps2"].is_number()));
    assert!(
        rows.iter().any(|r| r["verify_result"] == "on_crl"),
        "the revoked attacker's frames read on_crl"
    );
    let ego = rows_of(&w.root, "local/ego");
    assert!(!ego.is_empty() && ego.iter().all(|r| r["own_cert_id"].is_string()));
}

#[test]
fn global_reports_decode_the_mbr_fields() {
    let w = release("global", &DatasetShape::default(), &opts());
    let rows = rows_of(&w.root, "global/reports");
    assert!(!rows.is_empty());
    for r in &rows {
        assert_eq!(r["psid"], 32);
        assert_eq!(r["evidence"].as_array().map(Vec::len), Some(1), "{r}");
        assert!(r["evidence"][0]["spdu_digest"].is_string());
        assert_eq!(r["subtypes"][0]["name"], "ConstantPosition");
        // The wire carries SM-MBR codes only: the engine's check names are truth.
        assert!(r.get("nonstandard_triggers").is_none() && r.get("subtype_names").is_none());
        assert!(r["generation_location"]["lat_deg"].is_number());
        assert_eq!(r["report_id"].as_str().map(str::len), Some(16));
    }
    let dec = rows_of(&w.root, "global/decisions");
    assert!(
        dec.iter().all(|d| d["decision"] == "revoke"
            && d["report_ids"].as_array().is_some_and(|a| !a.is_empty()))
    );
    let crl = rows_of(&w.root, "global/crl");
    assert!(
        crl.iter()
            .all(|c| c["entry_kind"] == "linkage_seed" && c["crl_number"] == 1)
    );
}

/// Each observer table is in its observer's log order (validate_release.py `time`):
/// reports by creation, the MA's by arrival, whatever order the engine's report ids or the
/// recording's batches put them in.
#[test]
fn report_tables_are_in_time_order() {
    let mut recs = release_records(&DatasetShape::default());
    // internal ids that sort against creation time ...
    for r in &mut recs {
        if r.channel == "pki.report_fate" || r.channel == "ma.received" {
            let mut v: Value = serde_json::from_slice(&r.json).unwrap();
            let id = v["report_id"].as_str().unwrap().to_string();
            let t: u64 = id.rsplit('-').next().unwrap().parse().unwrap();
            v["report_id"] = Value::from(format!("r-{:020}", u64::MAX - t));
            r.json = serde_json::to_vec(&v).unwrap();
        }
    }
    // ... and the MA's records in reverse arrival order
    let ma: Vec<usize> = (0..recs.len())
        .filter(|&i| recs[i].channel == "ma.received")
        .collect();
    for k in 0..ma.len() / 2 {
        recs.swap(ma[k], ma[ma.len() - 1 - k]);
    }
    let root = tmp("order");
    let w = write(&mut Records(&recs), &root, &opts(), &meta()).expect("writes");
    for (table, col) in [
        ("local/reports", "t_created_ns"),
        ("local/checks", "t_ns"),
        ("global/reports", "t_received_ns"),
    ] {
        let t: Vec<i64> = rows_of(&w.root, table)
            .iter()
            .filter_map(|r| r[col].as_i64())
            .collect();
        assert!(t.len() >= 2, "{table} has too few rows to order");
        assert!(
            t.windows(2).all(|p| p[0] <= p[1]),
            "{table}.{col} out of time order: {t:?}"
        );
    }
}

#[test]
fn local_reports_checks_and_pki_cover_every_obu() {
    let w = release("localrep", &DatasetShape::default(), &opts());
    let reps = rows_of(&w.root, "local/reports");
    // Every report but the fixture's one quota eviction was uploaded.
    assert_eq!(reps.iter().filter(|r| r["fate"] == "evicted").count(), 1);
    assert!(
        reps.iter()
            .filter(|r| r["fate"] != "evicted")
            .all(|r| r["fate"] == "uploaded"
                && r["path"] == "cellular"
                && r["report_id"].is_string())
    );
    let checks = rows_of(&w.root, "local/checks");
    let suites: BTreeSet<&str> = checks.iter().filter_map(|c| c["suite"].as_str()).collect();
    assert_eq!(suites, BTreeSet::from(["research", "sm-mbr-us"]));
    let pki = rows_of(&w.root, "local/pki");
    let events: BTreeSet<&str> = pki.iter().filter_map(|c| c["event"].as_str()).collect();
    for e in [
        "enrolled",
        "batch_downloaded",
        "pseudonym_change",
        "crl_downloaded",
    ] {
        assert!(events.contains(e), "{e} missing from local/pki");
    }
    let refs: BTreeSet<&str> = pki
        .iter()
        .filter_map(|c| c["receiver_ref"].as_str())
        .collect();
    assert_eq!(
        refs.len(),
        DatasetShape::default().devices as usize,
        "pki covers every OBU, not only the instrumented"
    );
}

/// A report still in the RA's shuffle when the run ends has no `delivered` step and no MA
/// reception: its reporter's row keeps the bytes it uploaded from `node.report_built`
/// (first v1.1 build: 1,083 rows uploaded with no report id and no evidence). Fault: leave
/// `node.report_built` out of the writer's first pass.
#[test]
fn an_upload_no_ma_received_keeps_its_report_id_and_evidence() {
    let recs: Vec<RecordedRecord> = release_records(&DatasetShape::default())
        .into_iter()
        .filter(|r| {
            r.channel != "ma.received"
                && !(r.channel == "pki.report_fate"
                    && serde_json::from_slice::<Value>(&r.json).unwrap()["fate"] == "delivered")
        })
        .collect();
    assert!(recs.iter().any(|r| r.channel == "node.report_built"));
    let root = tmp("in-ra");
    let w = write(&mut Records(&recs), &root, &opts(), &meta()).expect("writes");
    let reps = rows_of(&w.root, "local/reports");
    let up: Vec<&Value> = reps.iter().filter(|r| r["fate"] == "uploaded").collect();
    assert!(!up.is_empty());
    for r in up {
        assert!(r["report_id"].is_string(), "{r}");
        assert!(r["size_bytes"].as_i64().is_some_and(|n| n > 0), "{r}");
        assert_eq!(r["path"], "cellular", "{r}");
        assert!(
            r["evidence_spdu_digests"]
                .as_array()
                .is_some_and(|a| !a.is_empty()),
            "{r}"
        );
    }
}

/// A message two attack actions edited keeps them both: the union of their fields and the
/// first (primary) action's attack (first v1.1 build: the last action replaced the first,
/// and a DoSDisruptive beacon was labelled `[repetitions]` alone). Fault: let the second
/// `gt.attack.action` replace the first.
#[test]
fn a_message_with_two_attack_actions_keeps_both_fields() {
    let mut recs = release_records(&DatasetShape::default());
    let i = recs
        .iter()
        .position(|r| r.channel == "gt.attack.action")
        .expect("an attack");
    let first: Value = serde_json::from_slice(&recs[i].json).unwrap();
    let msg = first["msg"].as_u64().unwrap();
    let mut second = first.clone();
    second["attacker"] = json!("threat/attacker/f2md/DoSDisruptive");
    second["fields"] = json!(["repetitions", "position"]);
    second["magnitude"] = json!(4.0);
    let mut extra = recs[i].clone();
    extra.json = serde_json::to_vec(&second).unwrap();
    recs.insert(i + 1, extra);
    let root = tmp("two-actions");
    let w = write(&mut Records(&recs), &root, &opts(), &meta()).expect("writes");
    let tx = rows_of(&w.root, "truth/transmissions");
    let row = tx
        .iter()
        .find(|r| r["tx_id"].as_u64() == Some(msg))
        .expect("the message");
    assert_eq!(
        row["fields_falsified"],
        json!(["position", "repetitions", "speed"]),
        "{row}"
    );
    assert_eq!(row["attack_id"], "ConstPos", "{row}");
    assert_eq!(row["magnitude"], 1.5, "{row}");
    // Every other falsified message keeps its one action's fields.
    assert!(
        tx.iter()
            .filter(|r| r["label"] == "malicious" && r["tx_id"].as_u64() != Some(msg))
            .all(|r| r["fields_falsified"] == json!(["position", "speed"]))
    );
}

#[test]
fn truth_labels_crosswalk_and_timeline() {
    let shape = DatasetShape::default();
    let w = release("truth", &shape, &opts());
    let tx = rows_of(&w.root, "truth/transmissions");
    let bad: Vec<&Value> = tx.iter().filter(|r| r["label"] == "malicious").collect();
    assert!(!bad.is_empty());
    assert!(bad.iter().all(|r| r["veremi_ext_code"] == 1
        && r["family"] == "position"
        && r["attack_id"] == "ConstPos"));
    assert!(tx.iter().any(|r| r["label"] == "faulty"));
    assert!(
        tx.iter()
            .all(|r| r["gnss_error_m"].is_number() && r["spdu_digest"].is_string())
    );
    let devices = rows_of(&w.root, "truth/devices");
    assert_eq!(
        devices.iter().filter(|d| d["label"] == "malicious").count(),
        shape.attackers as usize
    );
    let tl = rows_of(&w.root, "truth/timeline");
    let a0 = tl
        .iter()
        .find(|r| r["device_id"] == 0)
        .expect("attacker 0 has a timeline");
    for col in [
        "first_falsified_ns",
        "first_report_created_ns",
        "first_report_at_ma_ns",
        "decision_ns",
        "crl_published_ns",
        "crl_first_neighbour_ns",
        "crl_median_neighbour_ns",
        "last_malicious_accepted_ns",
    ] {
        assert!(a0[col].is_number(), "{col} is null: {a0}");
    }
    assert!(a0["first_falsified_ns"].as_i64() <= a0["decision_ns"].as_i64());
    assert!(a0["crl_published_ns"].as_i64() <= a0["crl_first_neighbour_ns"].as_i64());
    // the revoked attacker's frames after the CRL read on_crl, so the last accepted
    // falsified frame precedes the CRL's arrival
    assert!(
        a0["last_malicious_accepted_ns"].as_i64()
            < a0["crl_p95_neighbour_ns"].as_i64().map(|t| t + 200_000_000)
    );
    assert!(
        tl.iter().any(|r| r["falsely_accused"] == true),
        "the framed benign device has a row"
    );
    let reports = rows_of(&w.root, "truth/reports");
    assert!(reports.iter().any(|r| r["verdict"] == "false_accusation"));
    assert!(reports.iter().any(|r| r["verdict"] == "true_accusation"));
    let apps = rows_of(&w.root, "truth/app_effects");
    assert_eq!(apps.len(), 2, "the fixture's two warnings about attacker 0");
    assert!(apps.iter().all(|a| a["subject_label"] == "malicious"));
    let mlg = rows_of(&w.root, "ml/global");
    assert!(mlg.iter().all(|r| r["verdict"].is_string()));
}

#[test]
fn identifiers_are_opaque_and_unique() {
    let shape = DatasetShape::default();
    let w = release("ids", &shape, &opts());
    let rx = rows_of(&w.root, "local/rx");
    let tx = rows_of(&w.root, "truth/transmissions");
    let mut engine_ids: BTreeSet<String> = BTreeSet::new();
    for r in &tx {
        engine_ids.insert(r["tx_id"].to_string());
        engine_ids.insert(r["device_id"].to_string());
        engine_ids.insert(r["cert_id"].as_str().unwrap_or_default().to_string());
    }
    let mut seen = BTreeSet::new();
    for r in &rx {
        let rid = r["rx_id"].as_str().unwrap();
        let rr = r["receiver_ref"].as_str().unwrap();
        assert!(seen.insert(rid.to_string()), "rx_id {rid} repeats");
        assert!(
            !engine_ids.contains(rid) && !engine_ids.contains(rr),
            "{rid}/{rr} equals an engine id"
        );
        assert_eq!(rr.len(), 16);
    }
    let refs: BTreeSet<&str> = rx
        .iter()
        .filter_map(|r| r["receiver_ref"].as_str())
        .collect();
    assert_eq!(refs.len(), 3, "instrumented_receivers = 3");
    assert_eq!(w.instrumented, 3);
}

#[test]
fn instrumented_receivers_are_deterministic_and_zero_means_all() {
    let obus: BTreeSet<i64> = (0..40).collect();
    let a = choose_instrumented("s1", &obus, 5);
    assert_eq!(a, choose_instrumented("s1", &obus, 5));
    assert_eq!(a.len(), 5);
    assert_ne!(
        a,
        choose_instrumented("s2", &obus, 5),
        "a different seed picks differently"
    );
    assert_eq!(choose_instrumented("s1", &obus, 0), obus);
    let all = ReleaseOptions {
        instrumented_receivers: 0,
        ..opts()
    };
    let w = release("all", &DatasetShape::default(), &all);
    assert_eq!(w.instrumented, DatasetShape::default().devices as usize);
}

#[test]
fn two_writes_of_one_run_are_byte_identical() {
    let a = release("det-a", &DatasetShape::default(), &opts());
    let b = release("det-b", &DatasetShape::default(), &opts());
    let ma = std::fs::read(a.root.join("manifest.json")).unwrap();
    let mb = std::fs::read(b.root.join("manifest.json")).unwrap();
    assert_eq!(ma, mb);
}

#[test]
fn a_recording_without_the_contract_fields_still_exports_with_nulls() {
    // the legacy fixture: node.tx / phy.rx / node.verify as the engine wrote them before
    // the record contract, no report channels
    let recs = crate::dataset::fixture::records(&DatasetShape::default());
    let root = tmp("legacy");
    let w = write(&mut Records(&recs), &root, &opts(), &meta()).expect("exports");
    assert!(w.lint.is_clean(), "{}", w.lint.summary());
    assert_eq!(w.rows("runs"), 1);
    assert!(root.join("manifest.json").is_file());
}

#[test]
fn the_clean_release_lints_clean_and_an_injected_truth_column_does_not() {
    let w = release("lint", &DatasetShape::default(), &opts());
    assert!(w.lint.is_clean(), "{}", w.lint.summary());
    assert!(w.lint.files >= 8);
    // inject: a local table that carries the attacker flag, and one with a nested label
    const BAD: TableDef = TableDef {
        name: "local/injected",
        visibility: "NODE",
        doc: "",
        cols: &[
            col("cert_id", Ty::Str, "", ""),
            col("attacker", Ty::Bool, "", ""),
        ],
        extra: &[],
    };
    const NESTED: &[Col] = &[col("label", Ty::Str, "", "")];
    const BAD2: TableDef = TableDef {
        name: "global/injected",
        visibility: "MA",
        doc: "",
        cols: &[col("evidence", Ty::ListStruct(NESTED), "", "")],
        extra: &[],
    };
    for def in [&BAD, &BAD2] {
        let mut p = PartWriter::create(&w.root, format!("{}/part.parquet", def.name), def).unwrap();
        p.push(&Map::new()).unwrap();
        p.finish().unwrap();
    }
    let lint = lint_release(&w.root, &BTreeSet::new()).unwrap();
    let paths: BTreeSet<&str> = lint.violations.iter().map(|v| v.path.as_str()).collect();
    assert!(paths.contains("attacker"), "{}", lint.summary());
    assert!(paths.contains("evidence.label"), "{}", lint.summary());
    // an ml/ table may carry labels: it is the declared exception
    assert!(lint.violations.iter().all(|v| !v.file.starts_with("ml/")));
}

#[test]
fn an_engine_report_id_smuggled_into_a_value_is_caught() {
    let w = release("lintval", &DatasetShape::default(), &opts());
    const SMUGGLE: TableDef = TableDef {
        name: "local/smuggled",
        visibility: "NODE",
        doc: "",
        cols: &[col("note", Ty::Str, "", "")],
        extra: &[],
    };
    let mut p =
        PartWriter::create(&w.root, "local/smuggled/part.parquet".into(), &SMUGGLE).unwrap();
    let mut row = Map::new();
    row.insert("note".into(), "r-3-abc-1".into());
    p.push(&row).unwrap();
    p.finish().unwrap();
    let ids = BTreeSet::from(["r-3-abc-1".to_string()]);
    let lint = lint_release(&w.root, &ids).unwrap();
    assert_eq!(lint.violations.len(), 1, "{}", lint.summary());
    assert_eq!(lint.violations[0].kind, ViolationKind::IdentityValue);
}

#[test]
fn options_have_menus_and_refuse_what_is_not_built() {
    let ok = ReleaseOptions::from_opts(&serde_json::json!({"split": "test", "tier": "mini", "instrumented_receivers": 0, "heldout_attacks": ["threat/attacker/legacy/RandomPos"], "compat_veremi": false})).unwrap();
    assert_eq!(ok.split, "test");
    assert_eq!(ok.heldout_attacks, vec!["RandomPos".to_string()]);
    assert!(!ok.compat_veremi);
    for (bad, field) in [
        (serde_json::json!({"split": "dev"}), "opts.split"),
        (serde_json::json!({"tier": "huge"}), "opts.tier"),
        (serde_json::json!({"wire": true}), "opts.wire"),
        (serde_json::json!({"colour": 1}), "opts.colour"),
        (serde_json::json!({"run_id": "a b"}), "opts.run_id"),
        (
            serde_json::json!({"instrumented_receivers": -1}),
            "opts.instrumented_receivers",
        ),
    ] {
        assert_eq!(ReleaseOptions::from_opts(&bad).unwrap_err().0, field);
    }
    assert_eq!(
        RunMeta::default_run_id(Some("dswriter sample"), Some("17ebb06b2204")),
        "dswriter-sample-17ebb06b"
    );
}

fn null(r: &Value, col: &str) -> bool {
    r.get(col).is_none_or(Value::is_null)
}

/// The MA links a report to a device only through a decision's case (the LAs answer
/// then); every other report's `la_link` is null. The check names live in truth.
#[test]
fn global_reports_link_only_through_a_decision_and_keep_check_names_in_truth() {
    let w = release("linked", &DatasetShape::default(), &opts());
    let dec = rows_of(&w.root, "global/decisions");
    let mut by_report: BTreeMap<String, (Value, Value)> = BTreeMap::new();
    for d in &dec {
        for id in d["report_ids"].as_array().expect("ids") {
            by_report.insert(
                id.as_str().expect("id").into(),
                (d["la_link"].clone(), d["t_ns"].clone()),
            );
        }
    }
    let rows = rows_of(&w.root, "global/reports");
    let (mut linked, mut unlinked) = (0, 0);
    for r in &rows {
        match by_report.get(r["report_id"].as_str().expect("id")) {
            Some((l, t)) => {
                assert_eq!(&r["la_link"], l, "{r}");
                assert_eq!(&r["la_link_t_ns"], t, "{r}");
                linked += 1;
            }
            None => {
                assert!(
                    null(r, "la_link") && null(r, "la_link_t_ns"),
                    "linked with no case: {r}"
                );
                unlinked += 1;
            }
        }
    }
    assert!(
        linked > 0 && unlinked > 0,
        "{linked} linked, {unlinked} not"
    );
    let truth = rows_of(&w.root, "truth/reports");
    assert!(
        truth
            .iter()
            .any(|t| t["reporter_checks"][1] == "staleOrReplay")
    );
    assert!(
        truth
            .iter()
            .any(|t| t["nonstandard_triggers"][0] == "staleOrReplay")
    );
}

/// The issuer a receiver reads from the signer's certificate, and the MA from the
/// subject's: the ACA's HashedId8, never an engine provider name.
#[test]
fn issuers_are_the_certificates_hashedid8s() {
    let w = release("issuer", &DatasetShape::default(), &opts());
    let rx = rows_of(&w.root, "local/rx");
    let issuers = [
        crate::dataset::fixture::FIXTURE_ISSUER_A,
        crate::dataset::fixture::FIXTURE_ISSUER_B,
    ];
    assert!(!rx.is_empty());
    for r in &rx {
        assert!(
            issuers.contains(&r["issuer_id"].as_str().unwrap_or("none")),
            "{r}"
        );
    }
    for r in rows_of(&w.root, "global/reports") {
        assert!(
            issuers.contains(&r["subject_issuer"].as_str().unwrap_or("none")),
            "{r}"
        );
    }
}

/// The reporter's own checks are in local/checks from the report's creation, whether or
/// not the MA ever saw it; a check names the reception it ran on when that reception is
/// in the reporter's local/rx; a quota eviction reads `evicted`.
#[test]
fn local_checks_come_from_the_reporter_and_name_their_reception() {
    let o = ReleaseOptions {
        instrumented_receivers: 0,
        ..opts()
    };
    let w = release("checks", &DatasetShape::default(), &o);
    let reports = rows_of(&w.root, "local/reports");
    let evicted: Vec<&Value> = reports.iter().filter(|r| r["fate"] == "evicted").collect();
    assert_eq!(evicted.len(), 1, "the evicted report: {reports:?}");
    assert!(null(evicted[0], "report_id"), "never built, so no digest");
    assert_eq!(evicted[0]["subtypes"], json!(["ConstantPosition"]));
    let checks = rows_of(&w.root, "local/checks");
    let t = &evicted[0]["t_created_ns"];
    assert!(
        checks
            .iter()
            .any(|c| &c["t_ns"] == t && c["check"] == "ConstantPosition"),
        "the evicted report's check is missing"
    );
    let rx: BTreeMap<String, String> = rows_of(&w.root, "local/rx")
        .iter()
        .map(|r| {
            (
                r["rx_id"].as_str().unwrap().to_string(),
                r["receiver_ref"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    let named: Vec<&Value> = checks.iter().filter(|c| !null(c, "rx_id")).collect();
    assert!(!named.is_empty(), "no check names its reception");
    for c in named {
        assert_eq!(
            rx.get(c["rx_id"].as_str().unwrap()),
            c["receiver_ref"].as_str().map(str::to_string).as_ref(),
            "{c}"
        );
    }
}

/// A check names the reception its report was created from, when the same SPDU reaches
/// the reporter again later (a replay or a flood repeats an SPDU byte for byte: the v1.1
/// r08 DoSDisruptive copies; first v1.1 recording: the check named the last copy, received
/// after the check ran). Fault: keep only the last reception per SPDU and the check's
/// reception comes after it.
#[test]
fn a_check_names_the_reception_before_its_report_when_the_spdu_repeats() {
    let o = ReleaseOptions {
        instrumented_receivers: 0,
        ..opts()
    };
    let mut recs = release_records(&DatasetShape::default());
    let w = write(&mut Records(&recs), &tmp("repeat0"), &o, &meta()).expect("writes");
    let check = rows_of(&w.root, "local/checks")
        .into_iter()
        .find(|c| !null(c, "rx_id"))
        .expect("a check names its reception");
    let rcp = rows_of(&w.root, "truth/receptions")
        .into_iter()
        .find(|r| r["rx_id"] == check["rx_id"])
        .expect("its truth row");
    let (msg, rx) = (
        rcp["tx_id"].as_i64().unwrap(),
        rcp["rx_device_id"].as_i64().unwrap(),
    );
    let t_check = check["t_ns"].as_u64().unwrap();
    // The same SPDU again, as a new transmission received after the check.
    let copy = 9_000_000_i64;
    let first_t = recs
        .iter()
        .filter(|r| r.channel == "node.tx")
        .find(|r| serde_json::from_slice::<Value>(&r.json).unwrap()["msg"] == msg)
        .map(|r| r.sim_time)
        .expect("the message's air record");
    let shift = t_check - first_t + 50_000_000;
    let mut extra: Vec<RecordedRecord> = recs
        .iter()
        .filter(|r| {
            let v: Value = serde_json::from_slice(&r.json).unwrap();
            ["node.tx", "gt.tx_truth", "phy.rx", "node.rx_sec"].contains(&r.channel.as_str())
                && v["msg"] == msg
                && (!["phy.rx", "node.rx_sec"].contains(&r.channel.as_str()) || v["rx"] == rx)
        })
        .cloned()
        .collect();
    for r in &mut extra {
        r.sim_time += shift;
        let mut v: Value = serde_json::from_slice(&r.json).unwrap();
        v["msg"] = copy.into();
        for k in ["t", "t_start", "t_end"] {
            if let Some(t) = v[k].as_u64() {
                v[k] = (t + shift).into();
            }
        }
        r.json = serde_json::to_vec(&v).unwrap();
    }
    assert_eq!(extra.len(), 4, "tx, truth, rx and its security record");
    for r in extra {
        let at = recs.partition_point(|x| x.sim_time <= r.sim_time);
        recs.insert(at, r);
    }
    let w = write(&mut Records(&recs), &tmp("repeat1"), &o, &meta()).expect("writes");
    let t_rx: BTreeMap<String, u64> = rows_of(&w.root, "local/rx")
        .iter()
        .map(|r| {
            (
                r["rx_id"].as_str().unwrap().to_string(),
                r["t_rx_ns"].as_u64().unwrap(),
            )
        })
        .collect();
    assert!(
        t_rx.contains_key(&rx_id("0xfixture", rx, copy)),
        "the copy is in local/rx"
    );
    let mut named = 0;
    for c in rows_of(&w.root, "local/checks")
        .iter()
        .filter(|c| !null(c, "rx_id"))
    {
        let t = t_rx[c["rx_id"].as_str().unwrap()];
        assert!(
            t <= c["t_ns"].as_u64().unwrap(),
            "check before its reception {t}: {c}"
        );
        named += 1;
    }
    assert!(named > 0);
}

/// An attacker's parameters are the scenario's, as `name=value` strings (no JSON text);
/// an application warning that does not name its message gets the sender's last one.
#[test]
fn attack_params_are_strings_and_every_app_effect_names_its_message() {
    let w = release("params", &DatasetShape::default(), &opts());
    let dev = rows_of(&w.root, "truth/devices");
    let atk: Vec<&Value> = dev
        .iter()
        .filter(|d| d["attack_id"] == "ConstPos")
        .collect();
    assert!(!atk.is_empty());
    for d in atk {
        assert_eq!(d["attack_params"], json!(["magnitude_scale=1"]), "{d}");
    }
    let tx: BTreeSet<i64> = rows_of(&w.root, "truth/transmissions")
        .iter()
        .filter_map(|t| t["tx_id"].as_i64())
        .collect();
    let fx = rows_of(&w.root, "truth/app_effects");
    assert!(fx.len() >= 2, "{fx:?}");
    for a in &fx {
        assert!(a["tx_id"].as_i64().is_some_and(|t| tx.contains(&t)), "{a}");
    }
}

/// The timeline's first falsified message is the attacker's first falsified transmission
/// in truth/transmissions (the air time), not the attack model's earlier log entry.
#[test]
fn the_timeline_starts_at_the_first_falsified_transmission() {
    let w = release("firstfalse", &DatasetShape::default(), &opts());
    let tx = rows_of(&w.root, "truth/transmissions");
    let tl = rows_of(&w.root, "truth/timeline");
    let mut checked = 0;
    for r in tl.iter().filter(|r| r["label"] == "malicious") {
        let d = r["device_id"].as_i64();
        let first = tx
            .iter()
            .filter(|t| t["device_id"].as_i64() == d && t["label"] == "malicious")
            .filter_map(|t| t["t_ns"].as_i64())
            .min();
        assert_eq!(r["first_falsified_ns"].as_i64(), first, "{r}");
        checked += 1;
    }
    assert!(checked > 0);
}

/// An attacker that joins mid-run is labelled on its node even when its first
/// falsification is logged before any kinematics record maps its actor id to that node
/// (an actor id is not a node id).
#[test]
fn a_late_attackers_falsifications_land_on_its_node() {
    let mut recs = release_records(&DatasetShape::default());
    // Actor 0 (node 0, an attacker) gets actor id 1000 ...
    for r in &mut recs {
        if matches!(
            r.channel.as_str(),
            "gt.attack.action" | "gt.kinematics" | "gt.subject"
        ) {
            let mut v: Value = serde_json::from_slice(&r.json).unwrap();
            if v["actor"] == 0 {
                v["actor"] = 1000.into();
                r.json = serde_json::to_vec(&v).unwrap();
            }
        }
    }
    // ... and its kinematics records come after its first falsification.
    let first = recs
        .iter()
        .position(|r| {
            r.channel == "gt.attack.action"
                && serde_json::from_slice::<Value>(&r.json).unwrap()["actor"] == 1000
        })
        .expect("actor 0 attacks");
    let (late, keep): (Vec<_>, Vec<_>) = recs.drain(..first).partition(|r| {
        r.channel == "gt.kinematics"
            && serde_json::from_slice::<Value>(&r.json).unwrap()["actor"] == 1000
    });
    assert!(!late.is_empty());
    let mut reordered = keep;
    let atk = recs.remove(0);
    reordered.push(atk);
    reordered.extend(late);
    reordered.extend(recs);
    let root = tmp("lateattacker");
    let w = write(&mut Records(&reordered), &root, &opts(), &meta()).expect("writes");
    let dev = rows_of(&w.root, "truth/devices");
    assert!(
        dev.iter().all(|d| d["device_id"] != 1000),
        "an actor id became a device"
    );
    let d0 = dev.iter().find(|d| d["device_id"] == 0).expect("device 0");
    assert_eq!(d0["label"], "malicious", "{d0}");
}

/// The reporter's own copy of the evidence it attached: the engine writes the digests on
/// the fate that follows the report's build (`delivered`), after `stored` and `uploaded`
/// rows that carry an empty list (v1.0: `local/reports.evidence_spdu_digests` was `[]` in
/// every row). Fault: keep the first list seen (`if r.evidence.is_null()`) and every row
/// is empty.
#[test]
fn local_reports_carry_the_reporters_copy_of_the_evidence() {
    let w = release("localev", &DatasetShape::default(), &opts());
    let global: BTreeMap<String, Vec<Value>> = rows_of(&w.root, "global/reports")
        .iter()
        .map(|r| {
            let ev = r["evidence"].as_array().unwrap();
            (
                r["report_id"].as_str().unwrap().to_string(),
                ev.iter().map(|e| e["spdu_digest"].clone()).collect(),
            )
        })
        .collect();
    let mut joined = 0;
    for r in rows_of(&w.root, "local/reports") {
        let own = r["evidence_spdu_digests"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        match r["report_id"].as_str().and_then(|id| global.get(id)) {
            Some(g) => {
                assert_eq!(&own, g, "{r}");
                joined += 1;
            }
            // Never built for an MA (the quota eviction): nothing attached.
            None => assert!(own.is_empty(), "{r}"),
        }
    }
    assert!(joined > 0, "no local report joined to the MA's");
}

/// The fixture's records with message `msg`'s air record (and its receptions) delayed by
/// `delay` ns, as a queued message is: falsified at generation, on the air later.
fn delay_message(recs: &mut Vec<RecordedRecord>, msg: i64, delay: u64) {
    let moved_channels = ["node.tx", "gt.tx_truth", "phy.rx", "node.rx_sec"];
    let (mut moved, keep): (Vec<RecordedRecord>, Vec<RecordedRecord>) =
        std::mem::take(recs).into_iter().partition(|r| {
            moved_channels.contains(&r.channel.as_str())
                && serde_json::from_slice::<Value>(&r.json).unwrap()["msg"] == msg
        });
    assert!(!moved.is_empty());
    *recs = keep;
    for r in &mut moved {
        r.sim_time += delay;
        let mut v: Value = serde_json::from_slice(&r.json).unwrap();
        for k in ["t", "t_start", "t_end"] {
            if let Some(t) = v[k].as_u64() {
                v[k] = (t + delay).into();
            }
        }
        r.json = serde_json::to_vec(&v).unwrap();
    }
    for r in moved {
        let at = recs.partition_point(|x| x.sim_time <= r.sim_time);
        recs.insert(at, r);
    }
}

/// A falsified message that reaches the air seconds after its falsification (4.6 s for a
/// NotYetValid attacker in the v1.0 sample) is one `truth/transmissions` row with its air
/// record and the attack's label, and its receptions are labelled malicious (v1.0: a
/// second row under the same `tx_id`, the air row and its receptions benign). The VeReMi
/// traces label the same messages. Fault: write a falsification without its air record
/// when the 3 s window closes (`ATK_WAIT_NS` unused) and the first assertion fails.
#[test]
fn a_falsified_message_late_to_the_air_is_one_row_with_its_label() {
    let shape = DatasetShape {
        steps: 80,
        ..DatasetShape::default()
    };
    let mut recs = release_records(&shape);
    let first = recs
        .iter()
        .find(|r| r.channel == "gt.attack.action")
        .map(|r| serde_json::from_slice::<Value>(&r.json).unwrap())
        .expect("the fixture attacks");
    let msg = first["msg"].as_i64().unwrap();
    delay_message(&mut recs, msg, 4_600_000_000);
    let root = tmp("lateair");
    let o = ReleaseOptions {
        instrumented_receivers: 0,
        ..opts()
    };
    let w = write(&mut Records(&recs), &root, &o, &meta()).expect("writes");
    let tx = rows_of(&w.root, "truth/transmissions");
    let rows: Vec<&Value> = tx.iter().filter(|r| r["tx_id"] == msg).collect();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["label"], "malicious", "{}", rows[0]);
    assert!(rows[0]["t_ns"].is_number() && rows[0]["spdu_digest"].is_string());
    let mut ids = BTreeSet::new();
    assert!(
        tx.iter().all(|r| ids.insert(r["tx_id"].as_i64())),
        "a tx_id twice"
    );
    let rcp: Vec<Value> = rows_of(&w.root, "truth/receptions")
        .into_iter()
        .filter(|r| r["tx_id"] == msg)
        .collect();
    assert!(!rcp.is_empty());
    assert!(rcp.iter().all(|r| r["label"] == "malicious"), "{rcp:?}");
    // Every reception's label is its transmission's.
    let label: BTreeMap<i64, Value> = tx
        .iter()
        .map(|r| (r["tx_id"].as_i64().unwrap(), r["label"].clone()))
        .collect();
    for r in rows_of(&w.root, "truth/receptions") {
        assert_eq!(label[&r["tx_id"].as_i64().unwrap()], r["label"], "{r}");
    }
    // The VeReMi traces from the same records: attackerType non-zero exactly on the
    // malicious messages. (The release fixture's kinematics carry positions only; the
    // VeReMi view also reads the speed.)
    let mut vrecs = recs.clone();
    for r in vrecs.iter_mut().filter(|r| r.channel == "gt.kinematics") {
        let mut k: Value = serde_json::from_slice(&r.json).unwrap();
        if k.get("speed_mps").is_none() {
            k["speed_mps"] = 12.0.into();
        }
        r.json = serde_json::to_vec(&k).unwrap();
    }
    let v = crate::dataset::veremi::from_records_with(
        &vrecs,
        &crate::dataset::veremi::VeremiOptions::default(),
    )
    .expect("the VeReMi export");
    assert!(!v.truth.is_empty());
    for t in &v.truth {
        let id = i64::try_from(t.message_id).unwrap();
        assert_eq!(
            t.attacker_type != 0,
            label.get(&id) == Some(&Value::from("malicious")),
            "message {id}: attackerType {} against truth {:?}",
            t.attacker_type,
            label.get(&id)
        );
    }
}

/// A falsified message on the air 1.5 s after its falsification (inside the 3 s window,
/// as 16 queued NotYetValid messages in the v1.1 r03 recording) keeps its label on every
/// reception: the window restarts at the air record, so the receptions (written 2 s after
/// the air) still find the falsification (first v1.1 recording: the entry was written
/// 3 s after the falsification, before its receptions, and they were labelled benign).
/// Fault: drop the restart in `on_tx` (and its skip in `drain`) and the receptions read benign.
#[test]
fn a_falsified_message_queued_inside_the_window_labels_its_receptions() {
    let shape = DatasetShape {
        steps: 80,
        ..DatasetShape::default()
    };
    let mut recs = release_records(&shape);
    let first = recs
        .iter()
        .find(|r| r.channel == "gt.attack.action")
        .map(|r| serde_json::from_slice::<Value>(&r.json).unwrap())
        .expect("the fixture attacks");
    let msg = first["msg"].as_i64().unwrap();
    delay_message(&mut recs, msg, 1_500_000_000);
    let o = ReleaseOptions {
        instrumented_receivers: 0,
        ..opts()
    };
    let w = write(&mut Records(&recs), &tmp("queuedair"), &o, &meta()).expect("writes");
    let tx = rows_of(&w.root, "truth/transmissions");
    let rows: Vec<&Value> = tx.iter().filter(|r| r["tx_id"] == msg).collect();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["label"], "malicious", "{}", rows[0]);
    let rcp: Vec<Value> = rows_of(&w.root, "truth/receptions")
        .into_iter()
        .filter(|r| r["tx_id"] == msg)
        .collect();
    assert!(!rcp.is_empty());
    assert!(rcp.iter().all(|r| r["label"] == "malicious"), "{rcp:?}");
    let label: BTreeMap<i64, Value> = tx
        .iter()
        .map(|r| (r["tx_id"].as_i64().unwrap(), r["label"].clone()))
        .collect();
    for r in rows_of(&w.root, "truth/receptions") {
        assert_eq!(label[&r["tx_id"].as_i64().unwrap()], r["label"], "{r}");
    }
    // The receivers' own rows of it carry the air record's fields.
    let ids: BTreeSet<&str> = rcp.iter().filter_map(|r| r["rx_id"].as_str()).collect();
    let lrx: Vec<Value> = rows_of(&w.root, "local/rx")
        .into_iter()
        .filter(|r| r["rx_id"].as_str().is_some_and(|i| ids.contains(i)))
        .collect();
    assert_eq!(lrx.len(), rcp.len());
    assert!(lrx.iter().all(|r| r["speed_mps"].is_number()), "{lrx:?}");
}

/// An attack the VeReMi codebooks have no code for is null in the truth and ML tables
/// (v1.0: the trace files' 1024 sentinel); a benign message is 0. Fault: write the
/// codebook's answer as is and the first assertion reads 1024.
#[test]
fn an_attack_without_a_veremi_code_is_null_not_1024() {
    let mut recs = release_records(&DatasetShape::default());
    for r in &mut recs {
        if r.channel == "gt.attack.action" {
            let mut v: Value = serde_json::from_slice(&r.json).unwrap();
            v["attacker"] = "threat/attacker/f2md/AccelerationMultiplication".into();
            r.json = serde_json::to_vec(&v).unwrap();
        }
    }
    let w = write(&mut Records(&recs), &tmp("nullcode"), &opts(), &meta()).expect("writes");
    let tx = rows_of(&w.root, "truth/transmissions");
    let bad: Vec<&Value> = tx.iter().filter(|r| r["label"] == "malicious").collect();
    assert!(!bad.is_empty());
    for r in &bad {
        assert!(null(r, "veremi_ext_code") && null(r, "veremi1_code"), "{r}");
        assert_eq!(r["attack_id"], "AccelerationMultiplication");
    }
    assert!(
        tx.iter()
            .filter(|r| r["label"] == "benign")
            .all(|r| r["veremi_ext_code"] == 0 && r["veremi1_code"] == 0)
    );
    let ml = rows_of(&w.root, "ml/local");
    assert!(
        ml.iter()
            .filter(|r| r["label"] == "malicious")
            .all(|r| null(r, "veremi_ext_code"))
    );
    assert!(
        ml.iter()
            .filter(|r| r["label"] == "benign")
            .all(|r| r["veremi_ext_code"] == 0)
    );
}

/// A device's attack window covers the air time of each of its falsified messages, which
/// follows the falsification by the signing and queueing delay (v1.0: windows ended ~9 ms
/// before the last falsified air time). Fault: write `last_atk` as the end and the
/// fixture's 0.1 ms lag fails it.
#[test]
fn the_attack_window_covers_every_falsified_air_time() {
    let w = release("window", &DatasetShape::default(), &opts());
    let tx = rows_of(&w.root, "truth/transmissions");
    let mut checked = 0;
    for d in rows_of(&w.root, "truth/devices")
        .iter()
        .filter(|d| d["label"] == "malicious")
    {
        let times: Vec<i64> = tx
            .iter()
            .filter(|t| t["device_id"] == d["device_id"] && t["label"] == "malicious")
            .filter_map(|t| t["t_ns"].as_i64())
            .collect();
        let (lo, hi) = (times.iter().min().unwrap(), times.iter().max().unwrap());
        assert!(d["attack_start_ns"].as_i64().unwrap() <= *lo, "{d}");
        assert!(d["attack_end_ns"].as_i64().unwrap() >= *hi, "{d}");
        checked += 1;
    }
    assert!(checked > 0);
}

/// An attacker that leaves the run before its attack window opens logs no
/// `gt.attack.action`, so its device row was `malicious` with a null `attack_id` (v1.1
/// sample: a RandomSpeed vehicle scheduled from 25 s left at 21.2 s). The engine names the
/// scheduled attack on `gt.device`, and the row takes it when nothing was logged. Fault:
/// drop the fallback in `resolve_attacks` and the row's `attack_id` is null again.
#[test]
fn an_attacker_that_leaves_before_its_window_still_names_its_attack() {
    let mut recs = release_records(&DatasetShape::default());
    // Node 0 is an attacker: it never gets to falsify anything ...
    recs.retain(|r| {
        !(r.channel == "gt.attack.action"
            && serde_json::from_slice::<Value>(&r.json).unwrap()["actor"] == 0)
    });
    // ... and its device record names the attack it was given.
    let mut named = 0;
    for r in &mut recs {
        if r.channel == "gt.device" {
            let mut v: Value = serde_json::from_slice(&r.json).unwrap();
            if v["node"] == 0 {
                v["scheduled_attack"] = "ConstPos".into();
                r.json = serde_json::to_vec(&v).unwrap();
                named += 1;
            }
        }
    }
    assert_eq!(named, 1);
    let root = tmp("leftearly");
    let w = write(&mut Records(&recs), &root, &opts(), &meta()).expect("writes");
    let dev = rows_of(&w.root, "truth/devices");
    let d0 = dev.iter().find(|d| d["device_id"] == 0).expect("device 0");
    assert_eq!(d0["label"], "malicious", "{d0}");
    assert_eq!(d0["attack_id"], "ConstPos", "{d0}");
    // A device that did falsify keeps the attack its log names, not the scheduled one.
    let tx = rows_of(&w.root, "truth/transmissions");
    assert!(
        tx.iter()
            .all(|t| t["device_id"] != 0 || t["label"] != "malicious"),
        "node 0 logged nothing, so none of its messages is malicious"
    );
}

/// A car without a cellular modem sends its misbehaviour report over the air to a
/// roadside unit at the same instant as one of its BSMs: `local/ego` keeps one row per own
/// BSM (relay repro run: a second ego row at that instant with no speed or fix, so the
/// per-receiver time order broke). Fault: let every `node.tx` of an instrumented device
/// write an ego row (`states_own_kinematics` always true) and both assertions fail.
#[test]
fn a_relayed_report_frame_writes_no_ego_row() {
    let mut recs = release_records(&DatasetShape::default());
    let w0 = write(&mut Records(&recs), &tmp("ego-0"), &opts(), &meta()).expect("writes");
    let ego0 = rows_of(&w0.root, "local/ego").len();
    // Every instrumented car's BSMs each get a report frame beside them, same node, same
    // time, its own message id, no kinematic content.
    let mut next_msg = 9_000_000_i64;
    let mut extra = Vec::new();
    for r in recs.iter().filter(|r| r.channel == "node.tx") {
        let v: Value = serde_json::from_slice(&r.json).unwrap();
        if v["msg_type"] != "bsm" || v["content"].is_null() {
            continue;
        }
        next_msg += 1;
        let frame = json!({"t": v["t"], "node": v["node"], "msg": next_msg, "msg_type": "mbr",
                           "bytes_on_wire": 900, "signer": "certificate",
                           "pseudonym": v["pseudonym"], "report_id": "r-relayed"});
        let mut c = r.clone();
        c.json = serde_json::to_vec(&frame).unwrap();
        extra.push(c);
    }
    assert!(!extra.is_empty());
    for r in extra {
        let at = recs.partition_point(|x| x.sim_time <= r.sim_time);
        recs.insert(at, r);
    }
    let w = write(&mut Records(&recs), &tmp("ego-1"), &opts(), &meta()).expect("writes");
    let ego = rows_of(&w.root, "local/ego");
    assert_eq!(ego.len(), ego0, "a report frame wrote ego rows");
    let mut last: BTreeMap<String, i64> = BTreeMap::new();
    for r in &ego {
        let who = r["receiver_ref"].as_str().unwrap().to_string();
        let t = r["t_ns"].as_i64().unwrap();
        if let Some(prev) = last.insert(who, t) {
            assert!(t > prev, "two ego rows at or out of time order: {r}");
        }
        assert!(
            r["speed_mps"].is_number(),
            "an ego row without its speed: {r}"
        );
    }
}
