//! The `dataset` exporter end to end: a short run of the live-atk ConstPos scenario with
//! report records, written to an MCAP recording, exported through
//! `export::run_exporters`, and the release layout (docs/dataset/schema.md §3), its row
//! counts and its manifest hashes checked on disk.

use std::path::{Path, PathBuf};

use v2xw_engine::{Engine, MemoryRecorder, Scenario};

const SCENARIO: &str = "\
schema: v2xw/scenario/1
meta: {base: share-constpos-l-10.yaml, name: dataset release test}
time: {duration_s: 8.0}
actors:
  vehicles:
    demand: {fleet_size: 8}
threats:
  attackers:
    - id: threat/attacker/legacy/ConstPos
      fraction: 0.25
      schedule: {from_s: 1.0, to_s: 150.0}
detection:
  ma: {id: threat/ma/legacy-window}
security:
  scms: {report_records: true}
exporters:
  - id: dataset
    opts: {split: test, instrumented_receivers: 3, run_id: itest}
";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
fn a_run_naming_the_dataset_exporter_writes_the_release_layout() {
    let scenario = Scenario::parse(SCENARIO, Some(&root().join("scenarios").join("live-atk")))
        .expect("the test scenario parses");
    assert!(v2xw_engine::export::records_dataset(&scenario.exporters));
    let specs = scenario.exporters.clone();
    let mut engine = Engine::build(scenario, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    engine.run(&mut recorder).expect("runs");

    let dir = std::env::temp_dir().join(format!("v2xw-dataset-release-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("recording.mcap");
    let mut w =
        v2xw_record::RecordingWriter::create(&path, v2xw_record::RecordingOptions::default())
            .expect("recording opens");
    for (t, r) in recorder.records() {
        w.write_record(*t, r).expect("record writes");
    }
    w.finish().expect("recording finishes");
    drop(recorder);

    let exported = v2xw_engine::export::run_exporters(&specs, &path, &dir).expect("exports");
    assert_eq!(exported.len(), 1);
    let ds = dir.join("dataset");
    let part = |t: &str| {
        ds.join(t)
            .join("split=test")
            .join("run=itest")
            .join("part-00000.parquet")
    };
    for t in [
        "local/ego",
        "local/rx",
        "local/checks",
        "local/reports",
        "local/pki",
        "global/reports",
        "global/decisions",
        "global/crl",
        "truth/devices",
        "truth/pseudonyms",
        "truth/transmissions",
        "truth/receptions",
        "truth/reports",
        "truth/timeline",
        "truth/app_effects",
        "ml/local",
        "ml/global",
    ] {
        assert!(part(t).is_file(), "{t} missing");
        assert!(
            ds.join("schema")
                .join(format!("{}.json", t.replace('/', "_")))
                .is_file()
        );
    }
    for f in ["runs.parquet", "manifest.json", "DATASHEET.md"] {
        assert!(ds.join(f).is_file(), "{f} missing");
    }
    assert!(
        ds.join("compat/veremi/itest").is_dir(),
        "compat_veremi defaults on"
    );

    let rows = |t: &str| -> u64 {
        exported[0]
            .files
            .iter()
            .filter(|f| f.path == part(t))
            .map(|f| f.rows as u64)
            .sum()
    };
    for t in [
        "local/ego",
        "local/rx",
        "truth/transmissions",
        "truth/receptions",
        "truth/devices",
        "truth/pseudonyms",
        "ml/local",
    ] {
        assert!(rows(t) > 0, "{t} has no rows");
    }
    assert_eq!(rows("local/rx"), rows("truth/receptions"));
    assert!(
        rows("truth/transmissions") > rows("truth/devices"),
        "every device transmits more than once in 8 s"
    );

    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(ds.join("manifest.json")).unwrap()).unwrap();
    let files = manifest["files"].as_array().expect("files");
    // 18 tables, 18 schema files, the datasheet and at least one VeReMi trace
    assert!(files.len() > 37, "{} files", files.len());
    for f in files {
        let rel = f["path"].as_str().unwrap();
        let bytes = std::fs::read(ds.join(rel)).unwrap();
        assert_eq!(
            f["sha256"].as_str().unwrap(),
            v2xw_core::hash::sha256_hex(&bytes),
            "{rel}'s manifest hash"
        );
    }
    assert!(files.iter().any(|f| {
        f["path"]
            .as_str()
            .unwrap()
            .starts_with("compat/veremi/itest/")
    }));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_validator_refuses_a_bad_dataset_option() {
    let bad = SCENARIO.replace("split: test", "split: holdout");
    // `Scenario::parse` validates
    let err = Scenario::parse(&bad, Some(&root().join("scenarios").join("live-atk")));
    let msg = format!("{:?}", err.expect_err("refused"));
    assert!(msg.contains("exporters[0].opts.split"), "{msg}");
}

/// Exports an existing recording, for measuring the exporter's time and peak memory, or
/// re-exporting a run after a writer fix:
/// `V2XW_DATASET_RECORDING=<recording.mcap> V2XW_DATASET_OUT=<dir>`, optionally
/// `V2XW_DATASET_OPTS=<the exporter's opts as JSON>` (default `{"split": "train"}`), run
/// with `--ignored`.
#[test]
#[ignore = "a measurement over a recording named by the environment"]
fn measure_the_exporter_on_a_recording() {
    let (Ok(rec), Ok(out)) = (
        std::env::var("V2XW_DATASET_RECORDING"),
        std::env::var("V2XW_DATASET_OUT"),
    ) else {
        return;
    };
    let opts: serde_json::Value = std::env::var("V2XW_DATASET_OPTS").map_or_else(
        |_| serde_json::json!({"split": "train"}),
        |j| serde_json::from_str(&j).expect("V2XW_DATASET_OPTS is JSON"),
    );
    let specs: Vec<v2xw_engine::scenario::schema::ExporterSpec> =
        serde_json::from_value(serde_json::json!([{"id": "dataset", "opts": opts}])).unwrap();
    let exported = v2xw_engine::export::run_exporters(&specs, Path::new(&rec), Path::new(&out))
        .expect("exports");
    for f in &exported[0].files {
        if f.rows > 0 {
            println!(
                "{:>9} rows {:>11} bytes {}",
                f.rows,
                f.bytes,
                f.path.display()
            );
        }
    }
}

/// The relay path and the two report poisoners, end to end: 70 % of the cars carry no
/// cellular modem and relay their reports through the grid's roadside unit, which is
/// compromised and replaces every report it relays with a forgery
/// (`PoisonForwardedReports`), and one car is an insider that files forged reports against
/// cars it hears (`ReportPoisoning`).
const POISONING: &str = "\
schema: v2xw/scenario/1
meta: {base: base.yaml, name: dataset poisoning test}
time: {duration_s: 30.0}
actors:
  vehicles:
    demand: {fleet_size: 40}
net:
  uu:
    params: {preset: 4g-east-coast, penetration: 0.3}
threats:
  attackers:
    - {id: threat/attacker/legacy/ConstPos, actor_ids: [5, 9], schedule: {from_s: 1.0, to_s: 30.0}}
    - {id: threat/attacker/insider/ReportPoisoning, actor_ids: [3],
       params: {victim_pct: 1.0, report_prob: 1.0}, schedule: {from_s: 2.0, to_s: 30.0}}
  compromised_rsus: [0]
  compromised_rsu_attack:
    id: threat/attacker/compromised-rsu
    params: {kind: PoisonForwardedReports, poison_prob: 1.0, from_s: 0.0, to_s: 30.0}
detection:
  ma: {id: threat/ma/legacy-window}
security:
  scms: {report_records: true}
exporters:
  - id: dataset
    opts: {split: train, instrumented_receivers: 0, run_id: poison, compat_veremi: false}
";

/// The insider's forged reports are filed and uploaded and are marked in truth
/// (`poisoned`, a false accusation of an honest car, the car falsely accused in the
/// timeline); a relayed report
/// the compromised unit replaced still names the bytes the car sent; and a relaying car
/// keeps one ego row per BSM. Faults, each red here: arm the insider as a plain attacker
/// (no `insider` slot: no poisoned row), drop `Phase2::report_intercepted` (an uploaded
/// row with no report id), let every frame write an ego row (two rows at one instant).
#[test]
fn the_poisoners_and_the_relay_path_leave_the_rows_the_schema_promises() {
    let scenario = Scenario::parse(POISONING, Some(&root().join("scenarios").join("live-atk")))
        .expect("the test scenario parses");
    let specs = scenario.exporters.clone();
    let mut engine = Engine::build(scenario, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    engine.run(&mut recorder).expect("runs");
    let dir = std::env::temp_dir().join(format!("v2xw-dataset-poison-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("recording.mcap");
    let mut w =
        v2xw_record::RecordingWriter::create(&path, v2xw_record::RecordingOptions::default())
            .expect("recording opens");
    for (t, r) in recorder.records() {
        w.write_record(*t, r).expect("record writes");
    }
    w.finish().expect("recording finishes");
    drop(recorder);
    v2xw_engine::export::run_exporters(&specs, &path, &dir).expect("exports");
    let rows = |t: &str| {
        let p = dir
            .join("dataset")
            .join(t)
            .join("split=train/run=poison/part-00000.parquet");
        v2xw_record::dataset::release::read_rows(&p).expect("reads")
    };
    let devices = rows("truth/devices");
    let insider = devices
        .iter()
        .find(|d| d["attack_id"] == "ReportPoisoning")
        .expect("the insider is in truth/devices under its catalogue name");
    assert_eq!(insider["label"], "malicious");
    let insider_id = insider["device_id"].as_i64().unwrap();
    let label_of = |n: i64| {
        devices
            .iter()
            .find(|d| d["device_id"] == n)
            .map(|d| d["label"].as_str().unwrap().to_string())
    };

    let reports = rows("truth/reports");
    let poisoned: Vec<&serde_json::Value> =
        reports.iter().filter(|r| r["poisoned"] == true).collect();
    assert!(!poisoned.is_empty(), "the insider filed nothing");
    for r in &reports {
        let by_insider = r["reporter_device_id"] == insider_id;
        assert_eq!(r["poisoned"] == true, by_insider, "{r}");
    }
    // Forged and filed like any report: stored, then uploaded (the RA's shuffle holds
    // reports a while before the MA sees them; scenarios/dataset/relay-poison.yaml shows
    // them decrypted at the MA).
    assert!(
        poisoned.iter().any(|r| r["fate"] == "uploaded"),
        "no forged report was uploaded"
    );
    let mut framed = std::collections::BTreeSet::new();
    for r in &poisoned {
        let subject = r["subject_device_id"]
            .as_i64()
            .expect("the accused resolves");
        if label_of(subject).as_deref() == Some("benign") {
            assert_eq!(r["verdict"], "false_accusation", "{r}");
            framed.insert(subject);
        }
    }
    assert!(!framed.is_empty(), "no honest car framed");
    let timeline = rows("truth/timeline");
    for s in &framed {
        let t = timeline
            .iter()
            .find(|t| t["device_id"] == *s)
            .expect("a framed car has a timeline row");
        assert_eq!(t["falsely_accused"], true, "{t}");
    }

    let local = rows("local/reports");
    let relayed: Vec<&serde_json::Value> = local
        .iter()
        .filter(|r| r["fate"] == "uploaded" && r["path"] == "rsu_relay")
        .collect();
    println!(
        "{} local reports, {} uploaded, {} relayed; {} truth reports, {} poisoned",
        local.len(),
        local.iter().filter(|r| r["fate"] == "uploaded").count(),
        relayed.len(),
        reports.len(),
        poisoned.len()
    );
    assert!(!relayed.is_empty(), "nothing was relayed");
    for r in local.iter().filter(|r| r["fate"] == "uploaded") {
        assert!(
            r["report_id"].is_string(),
            "an upload without its bytes: {r}"
        );
        assert!(
            r["evidence_spdu_digests"]
                .as_array()
                .is_some_and(|a| !a.is_empty()),
            "an upload without its evidence: {r}"
        );
    }

    let mut last: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
    for e in rows("local/ego") {
        let who = e["receiver_ref"].as_str().unwrap().to_string();
        let t = e["t_ns"].as_i64().unwrap();
        if let Some(prev) = last.insert(who, t) {
            assert!(t > prev, "two ego rows at or out of time order: {e}");
        }
        assert!(e["speed_mps"].is_number(), "an ego row with no speed: {e}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
