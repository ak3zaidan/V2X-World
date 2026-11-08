//! `dump_metrics` — prints every metric a run can measure, with its definition, as JSON.
//!
//! ```text
//! cargo run -p v2xw-engine --example dump_metrics -- --out metrics.json
//! ```
//!
//! The catalogue comes from the engine's own provider selection
//! ([`v2xw_engine::wiring::build_metrics`]), asked once per way a scenario can select
//! metrics: `metrics: [all]` alone, `all` on a run with a security path, `all` with
//! applications enabled, and each opt-in group (`signing`, `traffic`, `detection-scores`,
//! `ssm`, an `energy:` section). Each metric is tagged with the first selection that
//! installs it. The runtime diagnostics of `v2xw_metrics::runtime` are listed apart: the
//! engine does not install them in a run.
//!
//! The documentation site's metric catalogue is generated from this output by
//! `tools/docs/gen_metric_catalogue.py`, so the catalogue cannot drift from the code.

use std::collections::BTreeSet;
use std::io::Write;

use serde_json::{Value, json};
use v2xw_core::registry::Registry;
use v2xw_engine::scenario::{ModelChoice, Scenario};
use v2xw_metrics::MetricProvider;

fn catalogue(
    scenario: &Scenario,
) -> Result<Vec<v2xw_metrics::MetricDef>, Box<dyn std::error::Error>> {
    let mut registry = Registry::new();
    let set = v2xw_engine::wiring::build_metrics(scenario, &mut registry)?;
    Ok(set.catalog())
}

fn with_metrics(names: &[&str]) -> Scenario {
    let mut s = Scenario::minimal();
    s.metrics = names.iter().map(|n| (*n).to_string()).collect();
    s.apps.enabled.clear();
    s
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let all = with_metrics(&["all"]);
    let mut security = with_metrics(&["all"]);
    security
        .detection
        .local
        .push(ModelChoice::new("detect/legacy-12"));
    let mut apps = with_metrics(&["all"]);
    apps.apps.enabled = ["fcw", "eebl", "ima", "lta", "bsw", "pcw", "rlvw", "glosa"]
        .iter()
        .map(|a| (*a).to_string())
        .collect();
    let mut energy = with_metrics(&[]);
    energy.energy = Some(v2xw_energy::EnergySpec::default());

    let selections: Vec<(&str, Scenario)> = vec![
        ("all", all),
        ("all, on a run with a security path", security),
        ("all, with apps.enabled", apps),
        ("an energy section", energy),
        ("signing", with_metrics(&["signing"])),
        ("traffic", with_metrics(&["traffic"])),
        ("detection-scores", with_metrics(&["detection-scores"])),
        ("ssm", with_metrics(&["ssm"])),
    ];

    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (selected_by, scenario) in &selections {
        for def in catalogue(scenario)? {
            if seen.insert(def.name.clone()) {
                let mut v = serde_json::to_value(&def)?;
                v["selected_by"] = json!(selected_by);
                out.push(v);
            }
        }
    }
    for def in v2xw_metrics::runtime::RuntimeProvider::new().defs() {
        if seen.insert(def.name.clone()) {
            let mut v = serde_json::to_value(&def)?;
            v["selected_by"] = Value::Null;
            out.push(v);
        }
    }

    let text = serde_json::to_string_pretty(&json!({ "metrics": out }))?;
    match args.as_slice() {
        [] => {
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(text.as_bytes())?;
            stdout.write_all(b"\n")?;
        }
        [flag, path] if flag == "--out" => std::fs::write(path, text + "\n")?,
        _ => return Err("usage: dump_metrics [--out <file.json>]".into()),
    }
    Ok(())
}
