//! Reading a run's evidence from a server, for the analyst.
//!
//! Everything comes through [`RpcTransport`], the copilot's one path to a run, with the same
//! read methods the Studio's metrics panel uses: `run.status`, `scenario.get`,
//! `metrics.query` (the catalogue, whole-run values, binned series, breakdowns) and
//! `inspect.entity {entity: "backend"}`. Nothing is computed here; the numbers are the
//! server's, quantised at its writer, and the analyst only compares them.
//!
//! Each metric is asked for on its own: a combined query returns as many rows as its
//! shortest series, so one metric with no samples would blank all the others.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::analyst::{
    BREAKDOWNS, Group, HEADLINE_METRICS, MetricEvidence, RunEvidence, SERIES_METRICS,
    ScenarioFacts,
};
use crate::error::Result;
use crate::transport::RpcTransport;

/// One catalogue row: what the run can be asked for.
#[derive(Debug, Clone)]
struct Row {
    unit: String,
    dims: Vec<String>,
    base: String,
}

/// The series kept for the analyst beyond [`SERIES_METRICS`]: a minimum and a peak are
/// read off these.
const EXTRA_SERIES: [&str; 3] = ["cert_pool_valid", "ttc_conflicts", "nar"];

/// Reads everything the analyst uses about the run the server is serving.
///
/// # Errors
/// Whatever `run.status` or `metrics.query` returns when the server cannot be reached; a
/// single metric that fails is left out rather than failing the whole read, and the
/// backend view is optional.
pub fn gather<T: RpcTransport>(rpc: &mut T) -> Result<RunEvidence> {
    let status = rpc.call("run.status", &json!({}))?;
    let t_ns = status.get("t_ns").and_then(Value::as_u64).unwrap_or(0);
    let t_end_ns = status.get("t_end_ns").and_then(Value::as_u64).unwrap_or(0);
    let scenario_doc = rpc
        .call("scenario.get", &json!({}))
        .ok()
        .and_then(|v| v.get("scenario").cloned())
        .unwrap_or(Value::Null);
    let facts = ScenarioFacts::from_document(&scenario_doc);

    let catalogue = rpc.call("metrics.query", &json!({}))?;
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    for r in catalogue
        .get("catalogue")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(name) = r.get("name").and_then(Value::as_str) else { continue };
        rows.insert(
            name.to_string(),
            Row {
                unit: r.get("unit").and_then(Value::as_str).unwrap_or("").to_string(),
                dims: r
                    .get("dims")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(|d| d.as_str().map(str::to_string)).collect())
                    .unwrap_or_default(),
                base: r.get("base").and_then(Value::as_str).unwrap_or(name).to_string(),
            },
        );
    }

    let mut metrics: BTreeMap<String, MetricEvidence> = BTreeMap::new();
    let whole = t_ns.max(1).saturating_add(1);
    for name in HEADLINE_METRICS {
        let Some(row) = rows.get(name) else { continue };
        let value = rpc
            .call(
                "metrics.query",
                &json!({"metrics": [name], "t_from_ns": 0, "t_to_ns": t_ns, "bin_ns": whole}),
            )
            .ok()
            .and_then(|v| last_value(&v));
        metrics.insert(
            name.to_string(),
            MetricEvidence {
                unit: row.unit.clone(),
                dims: row.dims.clone(),
                value,
                ..MetricEvidence::default()
            },
        );
    }

    // Binned: about thirty bins over the run, never finer than a second.
    let bin = (t_ns / 30).max(1_000_000_000);
    for name in SERIES_METRICS.iter().chain(EXTRA_SERIES.iter()) {
        let Some(row) = rows.get(*name) else { continue };
        if let Ok(v) = rpc.call(
            "metrics.query",
            &json!({"metrics": [name], "t_from_ns": 0, "t_to_ns": t_ns, "bin_ns": bin}),
        ) {
            let series = series_of(&v);
            let entry = metrics.entry((*name).to_string()).or_insert_with(|| MetricEvidence {
                unit: row.unit.clone(),
                dims: row.dims.clone(),
                ..MetricEvidence::default()
            });
            entry.series = series;
        }
    }

    for (name, dim) in BREAKDOWNS {
        let Some(row) = rows
            .get(name)
            .or_else(|| rows.values().find(|r| r.base == name))
        else {
            continue;
        };
        if !row.dims.iter().any(|d| d == dim) {
            continue;
        }
        if let Ok(v) = rpc.call(
            "metrics.query",
            &json!({"metrics": [name], "group_by": [dim], "t_from_ns": 0, "t_to_ns": t_ns}),
        ) {
            let groups = groups_of(&v);
            if groups.is_empty() {
                continue;
            }
            let entry = metrics.entry(name.to_string()).or_insert_with(|| MetricEvidence {
                unit: row.unit.clone(),
                dims: row.dims.clone(),
                ..MetricEvidence::default()
            });
            entry.groups.insert(dim.to_string(), groups);
        }
    }

    let backend = if facts.security_protocol.is_some() {
        rpc.call("inspect.entity", &json!({"entity": "backend"}))
            .ok()
            .and_then(|v| v.get("state").cloned())
            .filter(Value::is_object)
    } else {
        None
    };

    Ok(RunEvidence {
        run_id: status
            .get("run_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        scenario_hash: status
            .get("scenario_hash")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        scenario: facts,
        t_reached_s: t_ns as f64 / 1e9,
        t_end_s: t_end_ns as f64 / 1e9,
        metrics,
        backend,
        engine: status.get("engine").cloned(),
    })
}

/// The last non-null value of the first metric column of a `metrics.query` answer.
fn last_value(v: &Value) -> Option<f64> {
    v.get("rows")?
        .as_array()?
        .iter()
        .rev()
        .find_map(|r| r.get(1).and_then(Value::as_f64))
}

/// `(t_s, value)` pairs of a binned answer.
fn series_of(v: &Value) -> Vec<(f64, Option<f64>)> {
    v.get("rows")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    let t = r.get(0).and_then(Value::as_f64)? / 1e9;
                    Some((t, r.get(1).and_then(Value::as_f64)))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The groups of a grouped answer: rows `[key, value, lo, hi, n]`.
fn groups_of(v: &Value) -> Vec<Group> {
    v.get("rows")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    let key = match r.get(0)? {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    Some(Group {
                        key,
                        value: r.get(1).and_then(Value::as_f64),
                        n: r.get(4).and_then(Value::as_u64).unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::ScriptedRpc;

    #[test]
    fn evidence_is_read_from_the_servers_own_answers() {
        // ScriptedRpc answers by method, so every metrics.query gets the same reply here;
        // what is checked is the shape handling, and that a metric missing from the
        // catalogue is not asked for.
        let mut rpc = ScriptedRpc::new()
            .with("run.status", json!({"run_id": "r", "t_ns": 30_000_000_000u64,
                                        "t_end_ns": 60_000_000_000u64, "scenario_hash": "h"}))
            .with("scenario.get", json!({"scenario": {"meta": {"name": "n"},
                                          "radio": {"rat": "lte-v2x-pc5"}}}))
            .with("metrics.query", json!({
                "catalogue": [{"name": "cbr", "unit": "ratio", "dims": ["t", "node"], "base": "cbr"}],
                "rows": [[0, 0.42]]
            }));
        let e = gather(&mut rpc).expect("gathers");
        assert_eq!(e.run_id, "r");
        assert_eq!(e.scenario.rat, "lte-v2x-pc5");
        assert!((e.t_reached_s - 30.0).abs() < 1e-9);
        assert_eq!(e.metrics["cbr"].value, Some(0.42));
        assert!(!e.metrics.contains_key("pdr"), "pdr is not in this run's catalogue");
        assert!(e.backend.is_none(), "no security protocol, no backend read");
        assert!(
            rpc.seen.iter().all(|(m, p)| m != "metrics.query"
                || p.get("metrics").is_none()
                || p["metrics"][0] == "cbr"),
            "only catalogue metrics are queried"
        );
    }
}
