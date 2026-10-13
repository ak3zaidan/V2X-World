//! Per-setting help and sensitivity summaries for the agent.
//!
//! Two tools read this module. `setting_help` answers "what does this setting do, what
//! should I set it to, and does it change anything": the published row (title, unit,
//! range, default, recommended range, status, the guide's explanation), the rows the
//! configuration reference and the parameter inventories give for that path, and a short
//! sensitivity summary. `sensitivity` answers "what moves this metric" or "how sensitive
//! is X to Y" from the checked-in sweeps:
//!
//! * the expectation table (`crates/v2xw-engine/tests/expectations/*.yaml`, read by
//!   `v2xw sensitivity --expect`): what theory says a setting does to a metric, with the
//!   citation;
//! * the regenerated reports (`docs/validation/sensitivity-catalogue.md` from
//!   `v2xw sensitivity --catalogue`, and `sensitivity-expectations.md` from `--expect`):
//!   what the same-seed runs measured, dead settings included.
//!
//! The texts are compiled in, so the answers are the build's own; a sweep re-run and
//! committed is picked up by the next build. Every number returned is quoted from those
//! files, never computed here.

use std::sync::OnceLock;

use serde_json::{Value, json};

use crate::catalog;

const EXPECTATIONS: [&str; 4] = [
    include_str!("../../v2xw-engine/tests/expectations/catalogue.yaml"),
    include_str!("../../v2xw-engine/tests/expectations/radio.yaml"),
    include_str!("../../v2xw-engine/tests/expectations/traffic.yaml"),
    include_str!("../../v2xw-engine/tests/expectations/security-network.yaml"),
];

const REPORTS: [(&str, &str); 2] = [
    (
        "docs/validation/sensitivity-catalogue.md",
        include_str!("../../../docs/validation/sensitivity-catalogue.md"),
    ),
    (
        "docs/validation/sensitivity-expectations.md",
        include_str!("../../../docs/validation/sensitivity-expectations.md"),
    ),
];

const REFERENCES: [(&str, &str); 3] = [
    (
        "docs/guides/configuration-reference.md",
        include_str!("../../../docs/guides/configuration-reference.md"),
    ),
    (
        "docs/guides/config-inventory-traffic.md",
        include_str!("../../../docs/guides/config-inventory-traffic.md"),
    ),
    (
        "docs/guides/config-inventory-network.md",
        include_str!("../../../docs/guides/config-inventory-network.md"),
    ),
];

/// One expectation row, its text squeezed to a line.
#[derive(Debug, Clone)]
struct Expectation {
    setting: String,
    metric: String,
    text: String,
}

fn expectations() -> &'static [Expectation] {
    static CELL: OnceLock<Vec<Expectation>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut out = Vec::new();
        for text in EXPECTATIONS {
            let mut cur = String::new();
            for line in text.lines() {
                let t = line.trim();
                if t.starts_with("- {") {
                    cur = t.trim_start_matches("- ").to_string();
                } else if !cur.is_empty() && !t.starts_with('#') {
                    cur.push(' ');
                    cur.push_str(t);
                } else {
                    continue;
                }
                if cur.ends_with('}') {
                    let field = |name: &str| {
                        cur.split(&format!("{name}:"))
                            .nth(1)
                            .and_then(|r| r.split(',').next())
                            .map(|v| v.trim().trim_matches('"').to_string())
                            .unwrap_or_default()
                    };
                    out.push(Expectation {
                        setting: field("setting"),
                        metric: field("metric"),
                        text: cur.clone(),
                    });
                    cur.clear();
                }
            }
        }
        out
    })
}

/// The table rows of a Markdown text whose first cell names `key` (in backticks), each
/// with the heading of the section it is in.
fn table_rows(doc: &str, key: &str, limit: usize) -> Vec<String> {
    let tick = format!("`{key}`");
    let mut heading = "";
    let mut out = Vec::new();
    for line in doc.lines() {
        if line.starts_with('#') {
            heading = line.trim_start_matches('#').trim();
        } else if line.starts_with('|') {
            let first = line.trim_start_matches('|').split('|').next().unwrap_or("");
            if first.contains(&tick) {
                out.push(format!("[{heading}] {}", catalog::shorten(line, 400)));
                if out.len() >= limit {
                    break;
                }
            }
        }
    }
    out
}

/// The published key a person's words most plausibly name.
fn resolve(query: &str) -> Option<String> {
    if let Ok((k, _)) = crate::repair::resolve_key(query) {
        if crate::repair::setting_for(&k).is_some() {
            return Some(k);
        }
    }
    catalog::find_settings(query, 1)["settings"][0]["path"]
        .as_str()
        .map(str::to_string)
}

/// A setting's sensitivity summary: theory (expectation rows) and measurement (the
/// catalogue and expectation reports).
fn sensitivity_of(key: &str, limit: usize) -> Value {
    let theory: Vec<&str> = expectations()
        .iter()
        .filter(|e| e.setting == key)
        .take(limit)
        .map(|e| e.text.as_str())
        .collect();
    let mut measured = Vec::new();
    for (name, doc) in REPORTS {
        for row in table_rows(doc, key, limit) {
            measured.push(json!({"source": name, "row": row}));
        }
    }
    let dead = measured.iter().any(|m| {
        m["row"]
            .as_str()
            .is_some_and(|r| r.starts_with("[(a) Dead settings"))
    });
    json!({
        "expected_by_theory": theory,
        "measured": measured,
        "dead_in_sweep": dead,
        "how_measured": "same seed, one setting moved to each end of its recommended range on \
                         a small scenario family (v2xw sensitivity --catalogue / --expect); a \
                         setting no metric moved on that family is listed as dead there, which \
                         can mean the family does not exercise it",
    })
}

/// Everything the build knows about one setting, by key or plain words.
#[must_use]
pub fn setting_help(query: &str) -> Value {
    let Some(key) = resolve(query) else {
        return json!({"error": format!("no published setting matches {query:?}; try find_settings")});
    };
    let Some(row) = crate::repair::setting_for(&key) else {
        return json!({"error": format!("{key} is not a published setting; try find_settings")});
    };
    let mut reference = Vec::new();
    for (name, doc) in REFERENCES {
        for r in table_rows(doc, &key, 4) {
            reference.push(json!({"source": name, "row": r}));
        }
    }
    let published = v2xw_engine::scenario::publish::fields()
        .iter()
        .find(|f| f.get("x-path").and_then(Value::as_str) == Some(key.as_str()))
        .map(|f| {
            json!({
                "description": f.get("description"),
                "status_note": f.get("x-status-note"),
                "recommended": f.get("x-recommended"),
                "level": f.get("x-level"),
            })
        });
    json!({
        "setting": row.to_json(),
        "explanation": published,
        "reference": reference,
        "sensitivity": sensitivity_of(&key, 8),
    })
}

/// Sensitivity summaries: by setting (`setting`), by metric (`metric`: which settings
/// theory expects to move it), or both.
#[must_use]
pub fn sensitivity(setting: &str, metric: &str) -> Value {
    let key = if setting.trim().is_empty() {
        None
    } else {
        resolve(setting)
    };
    let metric = metric.trim().to_lowercase();
    let mut out = json!({});
    if let Some(k) = &key {
        let mut s = sensitivity_of(k, 12);
        if !metric.is_empty() {
            if let Some(a) = s["expected_by_theory"].as_array_mut() {
                a.retain(|r| {
                    r.as_str()
                        .is_some_and(|t| t.to_lowercase().contains(&metric))
                });
            }
        }
        out["setting"] = json!(k);
        out["summary"] = s;
    } else if !setting.trim().is_empty() {
        out["error"] = json!(format!("no published setting matches {setting:?}"));
    }
    if key.is_none() && !metric.is_empty() {
        let movers: Vec<&str> = expectations()
            .iter()
            .filter(|e| e.metric.to_lowercase().contains(&metric))
            .take(25)
            .map(|e| e.text.as_str())
            .collect();
        out["metric"] = json!(metric);
        out["settings_expected_to_move_it"] = json!(movers);
    }
    if out.as_object().is_some_and(serde_json::Map::is_empty) {
        out["error"] = json!("give a setting, a metric, or both");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_and_sensitivity_read_the_checked_in_sweeps() {
        assert!(expectations().len() > 100);
        let h = setting_help("radio.devices.obu.tx_power_dbm");
        assert_eq!(h["setting"]["path"], "radio.devices.obu.tx_power_dbm");
        assert!(
            !h["sensitivity"]["expected_by_theory"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(!h["reference"].as_array().unwrap().is_empty());
        let s = sensitivity("", "cbr_mean");
        assert!(
            s["settings_expected_to_move_it"]
                .as_array()
                .is_some_and(|a| !a.is_empty())
        );
        // Plain words resolve through the settings search.
        assert!(setting_help("transmit power")["setting"]["path"].is_string());
    }

    /// The agent's catalogue is the published surface itself: every published path is in
    /// it, so a setting added to the engine is settable by the agent with no edit here.
    #[test]
    fn the_agent_can_set_every_published_setting_including_the_new_ones() {
        let published: std::collections::BTreeSet<String> =
            v2xw_engine::scenario::publish::fields()
                .iter()
                .filter_map(|f| f.get("x-path").and_then(Value::as_str))
                .map(str::to_string)
                .collect();
        let catalogue: std::collections::BTreeSet<String> =
            catalog::settings().iter().map(|s| s.path.clone()).collect();
        assert_eq!(published, catalogue);
        assert!(catalogue.len() >= 250, "{}", catalogue.len());
        for (key, value) in [
            ("actors.vehicles.drivers.politeness", json!(0.8)),
            (
                "actors.vehicles.drivers.lane_change_threshold_mps2",
                json!(0.4),
            ),
            ("weather.intensity", json!(0.9)),
            ("radio.sidelink.resource_keep_probability", json!(0.4)),
            ("traffic.turn_curve_window_m", json!(6.0)),
        ] {
            let r = crate::repair::repair(key, &value).unwrap_or_else(|e| panic!("{key}: {e}"));
            assert_eq!(r.key, key);
        }
        let refused = catalog::refused_attackers();
        assert!(!refused.contains(&"Sybil") && refused.contains(&"DoS"));
    }
}
