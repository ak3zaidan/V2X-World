//! The offline half of the harness suite (`evals/harness.yaml`): what the harness itself
//! owns, checked with no model and no engine.
//!
//! * `resolve` — a settings search with the person's own words (the prompt as typed) finds
//!   every listed key in its first `resolve_top` rows (default 5);
//! * `repair` — a change written the way a model plausibly writes it lands on `want_key` =
//!   `want_value`, and the engine's loader accepts the resulting scenario;
//! * `refuse` — a change the loader refuses is refused with an explanation naming every
//!   listed word.
//!
//! Every key the suite names must be a published setting, so a renamed field turns this red
//! rather than leaving a check that passes by looking at nothing.
//!
//! It prints one line per failure and the pass rate; `V2XW_HARNESS_EVAL_MIN` (default 1.0)
//! is the share of checks that must pass. `V2XW_DUMP_SETTINGS=<file>` also writes the
//! settings index there.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value, json};
use v2xw_copilot::agent::{apply_requested_change, to_pointer};
use v2xw_copilot::catalog;
use v2xw_copilot::scenario::check_document;

#[derive(Debug, Deserialize)]
struct Suite {
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    prompt: String,
    #[serde(default)]
    resolve: Vec<String>,
    #[serde(default)]
    resolve_top: Option<usize>,
    #[serde(default)]
    repair: Vec<Repair>,
    #[serde(default)]
    refuse: Vec<Refuse>,
}

#[derive(Debug, Deserialize)]
struct Repair {
    key: String,
    value: Value,
    want_key: String,
    want_value: Value,
    /// Settings the prompt also asks for, applied first (a truck headway needs trucks).
    #[serde(default)]
    with: std::collections::BTreeMap<String, Value>,
}

#[derive(Debug, Deserialize)]
struct Refuse {
    key: String,
    value: Value,
    #[serde(default)]
    words: Vec<String>,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn base() -> Value {
    let text = std::fs::read_to_string(root().join("../../scenarios/phase1-grid.yaml"))
        .expect("the grid scenario reads");
    serde_yml::from_str(&text).expect("the grid scenario parses")
}

fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() <= 1e-9 * y.abs().max(1.0),
        _ => a == b,
    }
}

/// Whether `key` is a published setting (an array index or a map key matches the index's
/// `[]` / `*`), or a parent of one.
fn published(key: &str) -> bool {
    let norm = |p: &str| -> Vec<String> {
        p.replace("[]", "")
            .split('.')
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    let k = norm(key);
    catalog::settings().iter().any(|s| {
        let p = norm(&s.path);
        p.len() >= k.len()
            && k.iter()
                .zip(&p)
                .all(|(a, b)| a == b || b == "*" || a.chars().all(|c| c.is_ascii_digit()))
    })
}

#[test]
fn the_harness_suite_offline() {
    if let Ok(path) = std::env::var("V2XW_DUMP_SETTINGS") {
        let rows: Vec<Value> = catalog::settings().iter().map(|s| s.to_json()).collect();
        std::fs::write(&path, serde_json::to_string_pretty(&rows).unwrap()).unwrap();
    }
    let suite: Suite = serde_yml::from_str(
        &std::fs::read_to_string(root().join("evals/harness.yaml")).expect("the suite reads"),
    )
    .expect("the suite parses");
    assert!(
        suite.cases.len() >= 40,
        "the suite has {} cases",
        suite.cases.len()
    );

    let mut checks = 0usize;
    let mut failed: Vec<String> = Vec::new();
    for case in &suite.cases {
        for key in case
            .resolve
            .iter()
            .chain(case.repair.iter().map(|r| &r.want_key))
            .chain(case.refuse.iter().map(|r| &r.key))
        {
            assert!(
                published(key),
                "{}: `{key}` is not a published setting",
                case.id
            );
        }
        let top = case.resolve_top.unwrap_or(5);
        if !case.resolve.is_empty() {
            let found = catalog::find_settings(&case.prompt, top);
            let paths: Vec<&str> = found["settings"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|r| r["path"].as_str())
                .collect();
            for key in &case.resolve {
                checks += 1;
                if !paths.contains(&key.as_str()) {
                    failed.push(format!(
                        "{} resolve: `{key}` not in the first {top} for {:?}: {paths:?}",
                        case.id, case.prompt
                    ));
                }
            }
        }
        for r in &case.repair {
            checks += 1;
            let mut doc = base();
            for (k, v) in &r.with {
                apply_requested_change(&mut doc, k, v).expect("a `with` setting applies");
            }
            let outcome = apply_requested_change(&mut doc, &r.key, &r.value);
            let got = doc
                .pointer(&to_pointer(&r.want_key))
                .cloned()
                .unwrap_or(Value::Null);
            let report = check_document(doc);
            let ok = outcome.is_ok() && same(&got, &r.want_value) && report.ok;
            if !ok {
                failed.push(format!(
                    "{} repair: {} = {} gave {} = {got} ({}; loader {})",
                    case.id,
                    r.key,
                    r.value,
                    r.want_key,
                    outcome.map_or_else(|e| e.to_string(), |o| format!("landed on {}", o.0)),
                    report.errors.first().map_or("ok".to_string(), |e| e
                        .message
                        .chars()
                        .take(120)
                        .collect()),
                ));
            }
        }
        for r in &case.refuse {
            checks += 1;
            let mut doc = base();
            let text = match apply_requested_change(&mut doc, &r.key, &r.value) {
                Err(e) => e.to_string(),
                Ok(_) => {
                    let report = check_document(doc);
                    if report.ok {
                        String::new()
                    } else {
                        json!(report.errors).to_string()
                    }
                }
            };
            let missing: Vec<&String> = r
                .words
                .iter()
                .filter(|w| !text.contains(w.as_str()))
                .collect();
            if text.is_empty() || !missing.is_empty() {
                failed.push(format!(
                    "{} refuse: {} = {} → {:?} (missing {missing:?})",
                    case.id,
                    r.key,
                    r.value,
                    text.chars().take(160).collect::<String>()
                ));
            }
        }
    }
    let passed = checks - failed.len();
    let rate = passed as f64 / checks.max(1) as f64;
    for f in &failed {
        println!("FAIL {f}");
    }
    println!(
        "harness suite offline: {passed}/{checks} checks pass ({:.0}%), {} cases, {} settings indexed",
        rate * 100.0,
        suite.cases.len(),
        catalog::settings().len()
    );
    let min: f64 = std::env::var("V2XW_HARNESS_EVAL_MIN")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0);
    assert!(rate >= min, "{passed}/{checks} below {min}");
}

/// Every finding id a live suite expects (`findings:` in `harness.yaml` and `prompts.yaml`)
/// is one the insight engine can emit, so a renamed or invented id turns this red instead
/// of leaving a live check that can never pass (`anomaly-flag` once expected
/// `radio.jammed` and `radio.low-pdr`, which never existed).
#[test]
fn every_expected_finding_id_is_emitted_somewhere() {
    let sources: String = ["src/insight.rs", "src/analyst.rs"]
        .iter()
        .map(|p| std::fs::read_to_string(root().join(p)).expect("the insight source reads"))
        .collect();
    let mut missing = Vec::new();
    for suite in ["evals/harness.yaml", "evals/prompts.yaml"] {
        let text = std::fs::read_to_string(root().join(suite)).expect("the suite reads");
        let doc: Value = serde_yml::from_str(&text).expect("the suite parses");
        for case in doc["cases"].as_array().expect("cases") {
            for id in case["findings"].as_array().into_iter().flatten() {
                let id = id.as_str().expect("a finding id is a string");
                if !sources.contains(&format!("\"{id}\"")) {
                    missing.push(format!("{suite} {}: {id}", case["id"]));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "finding ids no insight emits: {missing:?}"
    );
}
