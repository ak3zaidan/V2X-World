//! Experiments: two or more runs of one scenario that differ in one stated way, compared
//! metric by metric, with the differences that matter called out and tied to the analyst's
//! findings.
//!
//! Pure: everything here is arithmetic on numbers the server already produced. The change
//! and the relative change of a metric are the only numbers it derives, and both are
//! printed with the values they came from, so a reader can check them.
//!
//! # What "significant" means here
//!
//! The variants of an experiment share the seed (the harness runs them so), so a
//! difference between them is the change's own effect on that one realisation, not noise
//! between two draws. It is still one realisation: no confidence interval is claimed, and
//! [`Experiment::caveats`] says so. A difference is called out when either
//!
//! * the metric crosses a health band (healthy / caution / unhealthy, [`HealthBand`]), or
//! * it moves by at least [`RELATIVE_FLOOR`] of the baseline **and** by more than the
//!   metric's own resolution ([`absolute_floor`]: a hundredth of a ratio, a millisecond, a
//!   tenth of a second, one count), so a jump from 0.001 to 0.002 is not news.
//!
//! # Health bands
//!
//! A band comes from the metric catalogue the server publishes (`metrics.query {}`, each
//! row's `health`) when the build has them; otherwise from [`fallback_band`], which reads
//! the deterministic analyst's own thresholds ([`crate::analyst::Thresholds`]) so a band and
//! a finding never disagree about the same number.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::analyst::{Analysis, Severity, Thresholds, fmt};

/// The relative change below which a metric is not called out, 10 %.
pub const RELATIVE_FLOOR: f64 = 0.10;

/// Which way is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Better {
    /// Smaller is better.
    Lower,
    /// Larger is better.
    Higher,
}

/// Two thresholds and where they come from: the shape of `MetricDef::health` in the
/// metric catalogue, read from JSON so this crate works with a server that publishes them
/// and one that does not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthBand {
    /// Which way is better.
    pub better: Better,
    /// The series judged, as a suffix of the metric's name (`""`, `".p95"`).
    #[serde(default)]
    pub judged: String,
    /// Healthy up to (or down to) this.
    pub caution: f64,
    /// Caution up to (or down to) this; unhealthy past it.
    pub unhealthy: f64,
    /// Where the thresholds come from.
    #[serde(default)]
    pub source: String,
}

impl HealthBand {
    /// `healthy`, `caution` or `unhealthy`.
    #[must_use]
    pub fn level(&self, v: f64) -> Option<&'static str> {
        if !v.is_finite() {
            return None;
        }
        Some(match self.better {
            Better::Lower if v > self.unhealthy => "unhealthy",
            Better::Lower if v > self.caution => "caution",
            Better::Higher if v < self.unhealthy => "unhealthy",
            Better::Higher if v < self.caution => "caution",
            _ => "healthy",
        })
    }
}

/// The bands a server's catalogue publishes, by the series name they judge
/// (`name + judged`).
#[must_use]
pub fn bands_from_catalogue(catalogue: &Value) -> BTreeMap<String, HealthBand> {
    let mut out = BTreeMap::new();
    for row in catalogue.get("catalogue").and_then(Value::as_array).into_iter().flatten() {
        let (Some(name), Some(h)) = (row.get("name").and_then(Value::as_str), row.get("health")) else {
            continue;
        };
        if let Ok(band) = serde_json::from_value::<HealthBand>(h.clone()) {
            out.insert(format!("{name}{}", band.judged), band);
        }
    }
    out
}

/// A band from the analyst's thresholds, for a server whose catalogue has none.
#[must_use]
pub fn fallback_band(series: &str, th: &Thresholds) -> Option<HealthBand> {
    let b = |better, caution, unhealthy, source: &str| HealthBand {
        better,
        judged: String::new(),
        caution,
        unhealthy,
        source: source.to_string(),
    };
    Some(match series {
        "cbr" | "cbr.p95" => b(Better::Lower, 0.5, th.cbr_target_etsi,
            "ETSI TS 102 687 target CBR 0.68; 0.5 is where DCC leaves its relaxed state"),
        "e2e_latency.p95" | "latency.p95" => b(Better::Lower, th.latency_budget_ms / 2.0, th.latency_budget_ms,
            "3GPP TS 22.185 §5.1 100 ms; caution at half the budget (engineering choice)"),
        "pdr[100m]" => b(Better::Higher, th.near_range_prr, th.prr_range_level,
            "3GPP TR 37.885 range read at PRR 0.9; 0.95 near range (engineering choice)"),
        "nar" | "awareness" => b(Better::Higher, 0.95, th.awareness_floor,
            "analyst awareness floor 0.9 (engineering choice)"),
        "unverified_ratio" => b(Better::Lower, 0.01, th.unverified_ratio,
            "analyst: 5 % unverified is a verification bottleneck (engineering choice)"),
        "aoi.p95" => b(Better::Lower, 2.0 * th.generation_interval_ms, th.aoi_intervals * th.generation_interval_ms,
            "3 generation intervals of 10 Hz BSMs (engineering choice)"),
        "mac_queue_depth" => b(Better::Lower, 0.5, th.queue_depth_saturated,
            "analyst: a mean depth of one frame is a saturating queue (engineering choice)"),
        "linkability" => b(Better::Lower, th.linkability / 2.0, th.linkability,
            "analyst: linkability 0.5 means pseudonym change is not protecting (engineering choice)"),
        "ttc_conflicts" => b(Better::Lower, 0.0, 5.0,
            "conflicts below the 1.5 s TTC of Hayward 1972 / FHWA SSAM; counts are engineering choices"),
        _ => return None,
    })
}

/// The resolution below which a change in this unit is not a change.
#[must_use]
pub fn absolute_floor(metric: &str, unit: &str) -> f64 {
    let u = unit.to_lowercase();
    match u.as_str() {
        "ratio" | "fraction" | "" if metric.contains("pdr") || metric.contains("ratio") || metric == "cbr" || metric.starts_with("cbr.") || metric.starts_with("det_") => 0.01,
        "ratio" | "fraction" => 0.01,
        "ms" => 1.0,
        "s" => 0.1,
        "m/s" => 0.2,
        "count" | "events" | "frames" | "messages" | "certs" | "vehicles" => 1.0,
        "b" | "bytes" | "byte" => 1.0,
        _ => 1e-6,
    }
}

/// One run of an experiment, as the comparison reads it.
#[derive(Debug, Clone, Copy)]
pub struct Arm<'a> {
    /// Its label.
    pub label: &'a str,
    /// The analyst's answer.
    pub analysis: &'a Analysis,
    /// Whole-run values beyond the analyst's headline (detection, overhead, privacy, …).
    pub extra: &'a BTreeMap<String, f64>,
    /// Units by metric.
    pub units: &'a BTreeMap<String, String>,
    /// The run's seed.
    pub seed: &'a str,
}

/// One metric across the arms.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Row {
    /// The metric.
    pub metric: String,
    /// Its unit.
    pub unit: String,
    /// Its value in each arm, in order; `None` where not measured.
    pub values: Vec<Option<f64>>,
    /// Each arm's change from the first (`None` for the first).
    pub changes: Vec<Option<f64>>,
    /// Each arm's relative change from the first, as a fraction.
    pub relative: Vec<Option<f64>>,
    /// Each arm's health level, when a band covers the metric.
    pub health: Vec<Option<String>>,
    /// Whether any arm differs from the first by the rule in the module notes.
    pub significant: bool,
    /// The findings that explain it: `id@label`, most severe first.
    pub explained_by: Vec<String>,
}

/// The findings one arm has that the first does not, and the reverse.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FindingChange {
    /// The arm.
    pub label: String,
    /// Finding ids (severity notice and up) it has and the first does not.
    pub new: Vec<String>,
    /// Finding ids the first has and it does not.
    pub gone: Vec<String>,
}

/// An experiment's comparison.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Experiment {
    /// The arms' labels; the first is the baseline.
    pub labels: Vec<String>,
    /// Every metric measured in the baseline and at least one other arm, significant ones
    /// first.
    pub rows: Vec<Row>,
    /// What changed in the findings.
    pub findings: Vec<FindingChange>,
    /// What the comparison does not establish.
    pub caveats: Vec<String>,
}

fn base(metric: &str) -> &str {
    metric.split(['.', '[']).next().unwrap_or(metric)
}

/// Compares the arms against the first.
#[must_use]
pub fn compare(arms: &[Arm<'_>], bands: &BTreeMap<String, HealthBand>, th: &Thresholds) -> Experiment {
    let labels: Vec<String> = arms.iter().map(|a| a.label.to_string()).collect();
    let mut caveats = vec![
        "One seed per arm: the differences are this realisation's, with no confidence interval."
            .to_string(),
    ];
    if arms.iter().any(|a| a.seed != arms[0].seed) {
        caveats.push(
            "The arms ran with different seeds, so part of each difference is a different draw."
                .to_string(),
        );
    }
    let value = |a: &Arm<'_>, m: &str| a.analysis.headline.get(m).or_else(|| a.extra.get(m)).copied();
    let mut names: BTreeSet<String> = BTreeSet::new();
    if let Some(first) = arms.first() {
        names.extend(first.analysis.headline.keys().cloned());
        names.extend(first.extra.keys().cloned());
    }
    let mut rows = Vec::new();
    for m in names {
        let values: Vec<Option<f64>> = arms.iter().map(|a| value(a, &m)).collect();
        if values.iter().skip(1).all(Option::is_none) {
            continue;
        }
        let unit = arms
            .iter()
            .find_map(|a| a.units.get(&m).or_else(|| a.units.get(base(&m))).cloned())
            .unwrap_or_default();
        let band = bands.get(&m).cloned().or_else(|| fallback_band(&m, th));
        let v0 = values[0];
        let floor = absolute_floor(&m, &unit);
        let mut changes = vec![None];
        let mut relative = vec![None];
        let mut significant = false;
        let health: Vec<Option<String>> = values
            .iter()
            .map(|v| v.and_then(|v| band.as_ref().and_then(|b| b.level(v))).map(str::to_string))
            .collect();
        for (i, v) in values.iter().enumerate().skip(1) {
            let (Some(a), Some(b)) = (v0, *v) else {
                changes.push(None);
                relative.push(None);
                continue;
            };
            let d = b - a;
            let rel = (a.abs() > 1e-12).then(|| d / a.abs());
            changes.push(Some(d));
            relative.push(rel);
            let moved = d.abs() > floor && rel.is_none_or(|r| r.abs() >= RELATIVE_FLOOR);
            let crossed = health[0].is_some() && health[i].is_some() && health[0] != health[i];
            significant |= moved || crossed;
        }
        let explained_by = if significant { explain(&m, arms) } else { Vec::new() };
        rows.push(Row { metric: m, unit, values, changes, relative, health, significant, explained_by });
    }
    rows.sort_by(|a, b| b.significant.cmp(&a.significant).then_with(|| a.metric.cmp(&b.metric)));
    let ids = |a: &Analysis| -> BTreeSet<String> {
        a.findings.iter().filter(|f| f.severity >= Severity::Notice).map(|f| f.id.clone()).collect()
    };
    let first = arms.first().map(|a| ids(a.analysis)).unwrap_or_default();
    let findings = arms
        .iter()
        .skip(1)
        .map(|a| {
            let mine = ids(a.analysis);
            FindingChange {
                label: a.label.to_string(),
                new: mine.difference(&first).cloned().collect(),
                gone: first.difference(&mine).cloned().collect(),
            }
        })
        .collect();
    Experiment { labels, rows, findings, caveats }
}

/// The findings, in any arm, whose evidence or charts name this metric: those an arm has
/// and the baseline lacks first, then by severity.
fn explain(metric: &str, arms: &[Arm<'_>]) -> Vec<String> {
    let b = base(metric);
    let first: BTreeSet<&str> = arms
        .first()
        .map(|a| a.analysis.findings.iter().map(|f| f.id.as_str()).collect())
        .unwrap_or_default();
    let mut hits: Vec<(bool, Severity, String)> = Vec::new();
    for (i, a) in arms.iter().enumerate() {
        for f in &a.analysis.findings {
            if f.severity < Severity::Notice {
                continue;
            }
            let names = f.links.iter().any(|l| base(&l.metric) == b)
                || f.evidence.iter().any(|e| base(&e.source) == b);
            if !names {
                continue;
            }
            let tag = format!("{}@{}", f.id, a.label);
            let new = i > 0 && !first.contains(f.id.as_str());
            if !hits.iter().any(|(_, _, t)| *t == tag) {
                hits.push((new, f.severity, tag));
            }
        }
    }
    hits.sort_by(|x, y| y.0.cmp(&x.0).then(y.1.cmp(&x.1)).then_with(|| x.2.cmp(&y.2)));
    hits.into_iter().take(3).map(|(_, _, t)| t).collect()
}

fn signed(v: f64) -> String {
    if v > 0.0 { format!("+{}", fmt(v)) } else { fmt(v) }
}

impl Experiment {
    /// The compact form a model reads: significant rows in full, the rest by name only.
    #[must_use]
    pub fn compact(&self) -> Value {
        let opt = |v: &Option<f64>| v.map_or_else(|| "—".to_string(), fmt);
        let sig: Vec<Value> = self
            .rows
            .iter()
            .filter(|r| r.significant)
            .map(|r| {
                let mut o = json!({
                    "metric": r.metric,
                    "values": r.values.iter().map(opt).collect::<Vec<_>>(),
                    "change": r.changes.iter().skip(1).map(|c| c.map_or_else(|| "—".to_string(), signed)).collect::<Vec<_>>(),
                    "pct": r.relative.iter().skip(1).map(|c| c.map_or_else(|| "—".to_string(), |x| format!("{}%", signed(100.0 * x)))).collect::<Vec<_>>(),
                });
                if !r.unit.is_empty() {
                    o["unit"] = json!(r.unit);
                }
                if r.health.iter().any(Option::is_some) {
                    o["health"] = json!(r.health.iter().map(|h| h.clone().unwrap_or_else(|| "—".into())).collect::<Vec<_>>());
                }
                if !r.explained_by.is_empty() {
                    o["why"] = json!(r.explained_by);
                }
                o
            })
            .collect();
        let unchanged: Vec<&str> = self.rows.iter().filter(|r| !r.significant).map(|r| r.metric.as_str()).collect();
        json!({
            "arms": self.labels,
            "significant": sig,
            "unchanged": unchanged,
            "findings": self.findings,
            "caveats": self.caveats,
        })
    }

    /// The comparison as Markdown, for the report.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut md = String::from("## Experiment\n\n");
        md.push_str(&format!("| Metric | Unit | {} | Significant | Explained by |\n|---|---|", self.labels.join(" | ")));
        for _ in &self.labels {
            md.push_str("---|");
        }
        md.push_str("---|---|\n");
        for r in &self.rows {
            let cells: Vec<String> = r
                .values
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let mut s = v.map_or_else(|| "—".to_string(), fmt);
                    match (r.changes.get(i), r.relative.get(i)) {
                        (Some(Some(d)), Some(Some(rel))) => {
                            s.push_str(&format!(" ({}, {}%)", signed(*d), signed(100.0 * rel)));
                        }
                        (Some(Some(d)), _) => s.push_str(&format!(" ({})", signed(*d))),
                        _ => {}
                    }
                    if let Some(Some(h)) = r.health.get(i) {
                        s.push_str(&format!(" {h}"));
                    }
                    s
                })
                .collect();
            md.push_str(&format!(
                "| `{}` | {} | {} | {} | {} |\n",
                r.metric,
                r.unit,
                cells.join(" | "),
                if r.significant { "yes" } else { "" },
                r.explained_by.join(", ")
            ));
        }
        for f in &self.findings {
            if !f.new.is_empty() {
                md.push_str(&format!("\nNew in {}: {}\n", f.label, f.new.join(", ")));
            }
            if !f.gone.is_empty() {
                md.push_str(&format!("\nGone in {}: {}\n", f.label, f.gone.join(", ")));
            }
        }
        for c in &self.caveats {
            md.push_str(&format!("\n_{c}_\n"));
        }
        md.push('\n');
        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyst::{Evidence, Finding, Layer, Link, Location};

    fn analysis(headline: &[(&str, f64)], findings: Vec<Finding>) -> Analysis {
        Analysis {
            run_id: "r".into(),
            scenario_hash: "h".into(),
            findings,
            headline: headline.iter().map(|(k, v)| ((*k).to_string(), *v)).collect(),
            not_measured: Vec::new(),
            thresholds: Vec::new(),
        }
    }

    fn finding(id: &str, metric: &str) -> Finding {
        Finding {
            id: id.into(),
            layer: Layer::Channel,
            severity: Severity::Warning,
            title: id.into(),
            detail: String::new(),
            evidence: vec![Evidence { label: "x".into(), value: 0.7, unit: "ratio".into(), source: metric.into() }],
            location: Location::default(),
            reference: None,
            links: vec![Link { metric: metric.into(), chart: metric.into(), query: Value::Null }],
        }
    }

    #[test]
    fn a_large_move_and_a_band_crossing_are_called_out_and_explained_and_a_tiny_one_is_not() {
        let a = analysis(&[("cbr", 0.30), ("pdr", 0.950), ("mac_drops", 0.0)], vec![]);
        let b = analysis(&[("cbr", 0.72), ("pdr", 0.952), ("mac_drops", 0.0)], vec![finding("channel.congested", "cbr")]);
        let none = BTreeMap::new();
        let units: BTreeMap<String, String> = [("cbr", "ratio"), ("pdr", "ratio")].iter().map(|(k, v)| ((*k).into(), (*v).into())).collect();
        let arms = [
            Arm { label: "light", analysis: &a, extra: &none, units: &units, seed: "1" },
            Arm { label: "rush", analysis: &b, extra: &none, units: &units, seed: "1" },
        ];
        let x = compare(&arms, &BTreeMap::new(), &Thresholds::default());
        let cbr = x.rows.iter().find(|r| r.metric == "cbr").expect("cbr");
        assert!(cbr.significant);
        assert_eq!(cbr.health, vec![Some("healthy".into()), Some("unhealthy".into())]);
        assert_eq!(cbr.explained_by, vec!["channel.congested@rush".to_string()]);
        assert!((cbr.relative[1].expect("rel") - 1.4).abs() < 1e-9);
        let pdr = x.rows.iter().find(|r| r.metric == "pdr").expect("pdr");
        assert!(!pdr.significant, "a 0.2 % move is not news");
        assert_eq!(x.rows[0].metric, "cbr", "significant rows come first");
        assert_eq!(x.findings[0].new, vec!["channel.congested".to_string()]);
        assert_eq!(x.caveats.len(), 1, "same seed: one caveat");
        let c = x.compact();
        assert_eq!(c["significant"][0]["pct"][0], "+140%");
        assert!(c["unchanged"].as_array().is_some_and(|u| u.iter().any(|m| m == "pdr")));
    }

    #[test]
    fn a_band_from_the_catalogue_wins_over_the_fallback() {
        let cat = json!({"catalogue": [{"name": "cbr", "health": {"better": "lower", "judged": "",
            "caution": 0.2, "unhealthy": 0.4, "source": "s"}}]});
        let bands = bands_from_catalogue(&cat);
        let a = analysis(&[("cbr", 0.30)], vec![]);
        let b = analysis(&[("cbr", 0.31)], vec![]);
        let none = BTreeMap::new();
        let no_units = BTreeMap::new();
        let arms = [
            Arm { label: "a", analysis: &a, extra: &none, units: &no_units, seed: "1" },
            Arm { label: "b", analysis: &b, extra: &none, units: &no_units, seed: "2" },
        ];
        let x = compare(&arms, &bands, &Thresholds::default());
        assert_eq!(x.rows[0].health[0].as_deref(), Some("caution"));
        assert_eq!(x.caveats.len(), 2, "different seeds are a caveat");
    }
}
