//! The SCMS effects table (`tests/expectations/scms-effects.yaml`): what each SCMS or
//! environment change did to the metrics, run cost and recorded PKI architecture in real
//! same-seed runs (docs/design/scms-real/testing/effects.md).
//!
//! The runs themselves are minutes each on the real calendar, so they are not repeated
//! here; this checks the table is whole (every row has its expectation, its source, a
//! measurement and a verdict, every run it names is defined, a fixed row names the test
//! that pins the fix) and that the sensitivity sweep's loader passes over it rather than
//! failing on it.
//!
//! Faults that turn it red: a row without a source or verdict; a row naming an undefined
//! run; the loader's `schema` skip removed (`every_row_is_planned` and this test both
//! fail to parse the file).

use std::path::{Path, PathBuf};

use serde_json::Value;

fn table() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/expectations/scms-effects.yaml")
}

#[test]
fn the_effects_table_is_whole() {
    let text = std::fs::read_to_string(table()).expect("the table reads");
    let doc: Value = serde_yml::from_str(&text).expect("the table parses");
    assert_eq!(
        doc["schema"],
        v2xw_engine::sensitivity::sweep::SCMS_RUN_EXPECTATIONS_SCHEMA
    );
    let runs = doc["runs"].as_object().expect("runs");
    assert!(runs.contains_key("L0"), "the base run is defined");
    let rows = doc["rows"].as_array().expect("rows");
    assert!(rows.len() >= 30, "{} rows", rows.len());
    let mut bad = Vec::new();
    for (n, r) in rows.iter().enumerate() {
        for f in ["id", "base", "variant", "area", "measure", "expect", "source", "measured", "verdict"] {
            if r[f].is_null() || r[f].as_str() == Some("") {
                bad.push(format!("row {n}: no {f}"));
            }
        }
        for f in ["base", "variant"] {
            if let Some(name) = r[f].as_str()
                && !runs.contains_key(name)
            {
                bad.push(format!("row {n}: run {name} is not defined"));
            }
        }
        match r["verdict"].as_str() {
            Some("pass" | "wrong-expectation" | "open") => {}
            Some("fixed") => {
                if r["test"].as_str().is_none_or(str::is_empty) {
                    bad.push(format!("row {n}: fixed without the test that pins it"));
                }
            }
            v => bad.push(format!("row {n}: verdict {v:?}")),
        }
        if !matches!(r["area"].as_str(), Some("metrics" | "performance" | "architecture")) {
            bad.push(format!("row {n}: area {:?}", r["area"]));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn the_sweep_loader_passes_over_the_effects_table() {
    let rows = v2xw_engine::sensitivity::sweep::load_expectations(&[table()])
        .expect("the loader reads the file without failing on it");
    assert!(rows.is_empty(), "it is not a sweep table: {} rows", rows.len());
}
