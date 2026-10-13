//! The catalogue-wide sweep: every numeric or choice setting the engine publishes, moved
//! one at a time on a small same-seed scenario, with every metric the run reports compared
//! against the unchanged run.
//!
//! The settings come from [`crate::scenario::publish::fields`], the same index the Studio
//! settings window and the agent read, so a field added to the schema is swept without an
//! edit here. Each setting is routed to a scenario *family* (a DSRC grid, a C-V2X grid, a
//! GeoNetworking grid, a security lifecycle, a traffic grid, pedestrians) by the routing
//! table of a families file (`scenarios/sensitivity/catalogue.yaml`). A setting the
//! family's loader refuses is retried on the next family of its route.
//!
//! The perturbations are the low and high ends of the setting's recommended range (parsed
//! from its guide text, kept inside the validator's bound), else half and twice the
//! default; every other value of a choice; the other value of a switch.
//!
//! Each run is measured into a *fingerprint*: the [`Measured`] metrics of the radio
//! harness, every numeric leaf of the [`RunReport`] (`report.*`), the sample-weighted mean
//! of every `metric.sample` (`metric.*`) and the ground-truth traffic state (`traffic.*`,
//! `vru.*`). A run whose fingerprint equals the base's moved nothing: the setting is
//! **dead** there (not read, or read by something no metric observes). Same seed means an
//! unread setting gives a bit-identical run, so "dead" needs no tolerance.
//!
//! An expectation file (`crates/v2xw-engine/tests/expectations/*.yaml`) names, per setting
//! and end, the metric theory says must move and which way; [`Sweep::contradictions`]
//! lists where the run disagreed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;

use super::{Direction, Measured, Row, SensitivityRecorder, absolutise_world, apply, judge};
use crate::ctx::RunRecorder;
use crate::error::{EngineError, Result, ScenarioError};
use crate::run::{Engine, RunReport};
use crate::scenario::Scenario;

/// One scenario family: a small base scenario and the fields applied to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Family {
    /// The family's name, which routes name.
    pub name: String,
    /// The base scenario, relative to the families file.
    pub base: String,
    /// Fields applied to the base, by dotted path.
    #[serde(default)]
    pub set: BTreeMap<String, Value>,
}

/// A routing rule: settings whose path starts with `prefix` try these families in order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    /// A dotted-path prefix (`""` matches every setting).
    pub prefix: String,
    /// The families to try, first first.
    pub families: Vec<String>,
}

/// A setting the sweep leaves out on purpose, and why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Skip {
    /// A dotted-path prefix, or an exact path.
    pub prefix: String,
    /// Why.
    pub reason: String,
}

/// A families file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Families {
    /// The families.
    pub families: Vec<Family>,
    /// The routing table, most specific prefix first (the first match wins).
    pub routes: Vec<Route>,
    /// Settings left out.
    #[serde(default)]
    pub skip: Vec<Skip>,
    /// Per-setting perturbation values that replace the computed ones (a setting whose
    /// guide text names no usable range), by dotted path.
    #[serde(default)]
    pub values: BTreeMap<String, Vec<Value>>,
    /// The most values a choice is swept at (spread evenly over its options), so a list
    /// of fifty jurisdictions does not cost fifty runs. Default 8.
    #[serde(default)]
    pub max_choices: Option<usize>,
    /// Prerequisites: fields applied to both the base and the changed run of every
    /// setting under a prefix (a Nakagami shape needs `fading: nakagami`), so the setting
    /// is measured where it acts rather than refused. Every matching entry applies, in
    /// order.
    #[serde(default)]
    pub context: Vec<Context>,
}

/// One prerequisite entry of a families file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    /// A dotted-path prefix.
    pub prefix: String,
    /// The fields both runs set.
    pub set: BTreeMap<String, Value>,
}

/// One expectation: moving `setting` to its `change` end moves `metric` `expect`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expectation {
    /// The dotted path.
    pub setting: String,
    /// `low`, `high`, or a literal value (run in addition to the computed ends).
    pub change: Value,
    /// A fingerprint metric name.
    pub metric: String,
    /// Which way.
    pub expect: Direction,
    /// The relative change required (default 0.01).
    #[serde(default)]
    pub min_change: Option<f64>,
    /// The theory, with a citation.
    pub source: String,
}

/// One setting the sweep moves, and the values it moves it to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Setting {
    /// The dotted path.
    pub path: String,
    /// `number`, `integer`, `boolean` or `enum`.
    pub kind: String,
    /// The implementation status the catalogue publishes.
    pub status: String,
    /// The default, when the catalogue publishes one.
    pub default: Option<Value>,
    /// The perturbations: (label, value). Labels are `low`, `high`, or the value.
    pub values: Vec<(String, Value)>,
    /// The families to try, first first.
    pub families: Vec<String>,
    /// Fields both runs set (the families file's `context`).
    #[serde(default)]
    pub context: BTreeMap<String, Value>,
}

/// The fingerprint of one run: every metric the sweep reads, by name.
pub type Fingerprint = BTreeMap<String, f64>;

/// One perturbation's outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    /// The setting.
    pub path: String,
    /// `low`, `high`, or the value.
    pub label: String,
    /// The value it was set to.
    pub value: Value,
    /// The family it ran on, or `None` when every family refused it.
    pub family: Option<String>,
    /// Each refusal on the way, as (family, message).
    pub refusals: Vec<(String, String)>,
    /// The metrics that moved, as (base, changed), for every name in either fingerprint
    /// whose value differs.
    pub moved: BTreeMap<String, (Option<f64>, Option<f64>)>,
    /// A run error (not a refusal), if the run failed.
    pub error: Option<String>,
    /// The key of the base run it is compared with in [`Sweep::bases`]: the family, and
    /// the context when there is one.
    #[serde(default)]
    pub base: String,
}

impl Outcome {
    /// The relative change of `metric`, `(v - b) / |b|`, when both exist.
    #[must_use]
    pub fn rel(&self, metric: &str) -> Option<f64> {
        match self.moved.get(metric)? {
            (Some(b), Some(v)) => Some((v - b) / b.abs().max(1e-12)),
            _ => None,
        }
    }

    /// Whether the run happened and moved nothing.
    #[must_use]
    pub fn dead(&self) -> bool {
        self.family.is_some() && self.error.is_none() && self.moved.is_empty()
    }
}

/// An expectation's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    /// The expectation.
    pub expectation: Expectation,
    /// The base and changed values, when the perturbation ran.
    pub base: Option<f64>,
    /// The changed value.
    pub value: Option<f64>,
    /// Whether it held.
    pub pass: bool,
    /// Why not.
    pub note: String,
}

/// A whole sweep.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Sweep {
    /// The settings planned.
    pub settings: Vec<Setting>,
    /// The settings left out, as (path, reason).
    pub skipped: Vec<(String, String)>,
    /// The outcomes, in plan order.
    pub outcomes: Vec<Outcome>,
    /// Each family's base fingerprint.
    pub bases: BTreeMap<String, Fingerprint>,
    /// The expectation verdicts.
    pub verdicts: Vec<Verdict>,
    /// Wall-clock seconds the runs took.
    pub wall_s: f64,
}

/// Reads a YAML file.
fn read_yaml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text = std::fs::read_to_string(path).map_err(|source| EngineError::Io {
        path: path.display().to_string(),
        source,
    })?;
    serde_yml::from_str(&text).map_err(|e| {
        EngineError::Scenario(ScenarioError::Parse {
            path: path.display().to_string(),
            message: e.to_string(),
        })
    })
}

impl Families {
    /// Reads a families file.
    ///
    /// # Errors
    /// The file cannot be read or parsed.
    pub fn load(path: impl AsRef<Path>) -> Result<Families> {
        read_yaml(path.as_ref())
    }

    /// The families a setting tries, or `None` when no route matches.
    #[must_use]
    pub fn route(&self, path: &str) -> Option<&[String]> {
        self.routes
            .iter()
            .find(|r| prefix_matches(&r.prefix, path))
            .map(|r| r.families.as_slice())
    }

    /// Why `path` is skipped, if it is.
    #[must_use]
    pub fn skip_reason(&self, path: &str) -> Option<&str> {
        self.skip
            .iter()
            .find(|s| prefix_matches(&s.prefix, path))
            .map(|s| s.reason.as_str())
    }

    /// Loads each family's base scenario, with the family's fields applied.
    ///
    /// # Errors
    /// A base cannot be loaded, or a field does not parse into the schema.
    pub fn bases(&self, families_file: &Path) -> Result<BTreeMap<String, Scenario>> {
        let dir = families_file.parent().unwrap_or_else(|| Path::new("."));
        let mut out = BTreeMap::new();
        for f in &self.families {
            let p: PathBuf = dir.join(&f.base);
            let mut s = Scenario::load_unvalidated(&p)?;
            absolutise_world(&mut s, &p);
            out.insert(f.name.clone(), apply(&s, &f.set)?);
        }
        Ok(out)
    }
}

/// Reads every expectation file given (each a YAML list).
///
/// # Errors
/// A file cannot be read or parsed.
pub fn load_expectations(paths: &[PathBuf]) -> Result<Vec<Expectation>> {
    let mut out = Vec::new();
    for p in paths {
        let mut rows: Vec<Expectation> = read_yaml(p)?;
        out.append(&mut rows);
    }
    Ok(out)
}

/// `prefix` matches `path` exactly, as a dotted-path prefix, or always when empty.
fn prefix_matches(prefix: &str, path: &str) -> bool {
    prefix.is_empty()
        || path == prefix
        || (path.starts_with(prefix)
            && (prefix.ends_with('.') || path[prefix.len()..].starts_with('.')))
}

/// The numbers written in a guide's recommended-range text, in order.
#[must_use]
pub fn numbers_in(text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let neg = b[i] == b'-' && i + 1 < b.len() && b[i + 1].is_ascii_digit()
            // "10-20" is a range, not minus twenty.
            && (i == 0 || !b[i - 1].is_ascii_alphanumeric());
        let start = i;
        if neg {
            i += 1;
        }
        if i < b.len() && b[i].is_ascii_digit() && (start == 0 || !b[start - 1].is_ascii_alphabetic()) {
            let mut j = i;
            // "10 000" and "1,000" group thousands in the guide's prose.
            let mut digits = String::new();
            while j < b.len() {
                let c = b[j];
                if c.is_ascii_digit() || c == b'.' {
                    digits.push(c as char);
                    j += 1;
                } else if (c == b',' || c == b' ')
                    && j + 4 <= b.len()
                    && b[j + 1..].iter().take(3).all(u8::is_ascii_digit)
                    && b.get(j + 4).is_none_or(|n| !n.is_ascii_digit())
                    && digits.len() <= 3
                    && !digits.contains('.')
                {
                    j += 1;
                } else {
                    break;
                }
            }
            let digits = digits.trim_end_matches('.');
            // A dotted identifier ("802.11p", "1609.2") is a name, not a number.
            let ident = b.get(j).is_some_and(|c| c.is_ascii_alphabetic() && *c != b'e' && !matches!(*c, b'x' | b's' | b'm' | b'k' | b'h' | b'd' | b'u' | b'n' | b'H' | b'M' | b'G' | b'%'));
            if let Ok(v) = digits.parse::<f64>()
                && !ident
            {
                out.push(if neg { -v } else { v });
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

/// The low and high perturbation of a numeric setting.
fn numeric_ends(row: &Value, integer: bool) -> Vec<(String, Value)> {
    let lo_b = row
        .get("minimum")
        .or_else(|| row.get("exclusiveMinimum"))
        .and_then(Value::as_f64)
        .unwrap_or(f64::NEG_INFINITY);
    let lo_excl = row.get("exclusiveMinimum").is_some();
    let hi_b = row
        .get("maximum")
        .and_then(Value::as_f64)
        .unwrap_or(f64::INFINITY);
    let inside = |v: f64| (if lo_excl { v > lo_b } else { v >= lo_b }) && v <= hi_b;
    let default = row.get("default").and_then(Value::as_f64);
    let rec: Vec<f64> = row
        .get("x-recommended")
        .and_then(Value::as_str)
        .map(numbers_in)
        .unwrap_or_default()
        .into_iter()
        .filter(|v| inside(*v))
        .collect();
    let (mut lo, mut hi) = (None, None);
    if rec.len() >= 2 {
        let a = rec.iter().copied().fold(f64::INFINITY, f64::min);
        let b = rec.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if a < b {
            lo = Some(a);
            hi = Some(b);
        }
    }
    if lo.is_none() {
        match default {
            Some(d) if d != 0.0 => {
                let (a, b) = if d > 0.0 { (d / 2.0, d * 2.0) } else { (d * 2.0, d / 2.0) };
                lo = Some(if inside(a) { a } else { lo_b.max(a) });
                hi = Some(if inside(b) { b } else { hi_b.min(b) });
            }
            Some(_) => {
                hi = Some(if hi_b.is_finite() { hi_b.min(1.0) } else { 1.0 });
                if lo_b.is_finite() && lo_b < 0.0 {
                    lo = Some(lo_b.max(-1.0));
                }
            }
            None => {
                if lo_b.is_finite() && !lo_excl {
                    lo = Some(lo_b);
                }
                if hi_b.is_finite() {
                    hi = Some(hi_b);
                }
            }
        }
    }
    let mut out = Vec::new();
    for (label, v) in [("low", lo), ("high", hi)] {
        let Some(mut v) = v else { continue };
        if integer {
            v = v.round();
        }
        if !inside(v) || default.is_some_and(|d| (d - v).abs() < 1e-12) {
            continue;
        }
        let value = if integer {
            Value::from(v as i64)
        } else {
            serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
        };
        out.push((label.to_string(), value));
    }
    out
}

/// The choices of an enum row, other than the default.
fn choice_values(row: &Value, bare_slot: Option<bool>) -> Vec<(String, Value)> {
    let default = row.get("default");
    let mut values: Vec<String> = Vec::new();
    if let Some(Value::Array(e)) = row.get("enum") {
        values.extend(e.iter().filter_map(Value::as_str).map(str::to_string));
    } else if let Some(Value::Array(o)) = row.get("x-options") {
        values.extend(
            o.iter()
                .filter(|r| r.get("available").is_none_or(|a| a.as_bool() != Some(false)))
                .filter_map(|r| r.get("value").and_then(Value::as_str))
                .map(str::to_string),
        );
    }
    let default_id = default.and_then(|d| d.as_str().or_else(|| d.get("id").and_then(Value::as_str)));
    values
        .into_iter()
        .filter(|v| Some(v.as_str()) != default_id)
        .map(|v| {
            let value = match bare_slot {
                Some(false) => serde_json::json!({"id": v, "params": {}}),
                _ => Value::String(v.clone()),
            };
            (v, value)
        })
        .collect()
}

/// The sweep's plan: every published setting with its perturbations and families, and the
/// settings left out with the reason.
#[must_use]
pub fn plan(families: &Families, expectations: &[Expectation]) -> (Vec<Setting>, Vec<(String, String)>) {
    let fields = crate::scenario::publish::fields();
    let slots = crate::scenario::publish::slots();
    let spelling: BTreeMap<String, bool> = slots
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            Some((
                s.get("path")?.as_str()?.to_string(),
                s.get("spelling")?.as_str()? == "id",
            ))
        })
        .collect();
    let mut settings = Vec::new();
    let mut skipped = Vec::new();
    let mut seen = BTreeSet::new();
    for row in &fields {
        let Some(path) = row.get("x-path").and_then(Value::as_str) else {
            continue;
        };
        if !seen.insert(path.to_string()) {
            continue;
        }
        let kind = row.get("kind").and_then(Value::as_str).unwrap_or("");
        let status = row
            .get("x-status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let skip = |why: &str| (path.to_string(), why.to_string());
        if path.contains('[') || path.contains('*') {
            skipped.push(skip("an element of a list or map: no single field to move"));
            continue;
        }
        if let Some(why) = families.skip_reason(path) {
            skipped.push(skip(why));
            continue;
        }
        if status == "descriptive" {
            skipped.push(skip("descriptive: documentation only by design"));
            continue;
        }
        let closed = row.get("x-options-open").and_then(Value::as_bool) != Some(true);
        let mut values = if let Some(v) = families.values.get(path) {
            // Two numbers are a low and a high end, like the computed ones.
            let ends = v.len() == 2 && v.iter().all(Value::is_number);
            v.iter()
                .enumerate()
                .map(|(i, v)| {
                    let label = if ends {
                        ["low", "high"][i].to_string()
                    } else {
                        v.as_str().map_or_else(|| v.to_string(), str::to_string)
                    };
                    (label, v.clone())
                })
                .collect()
        } else {
            match kind {
                "number" => numeric_ends(row, false),
                "integer" => numeric_ends(row, true),
                "boolean" => {
                    let d = row.get("default").and_then(Value::as_bool).unwrap_or(false);
                    vec![((!d).to_string(), Value::Bool(!d))]
                }
                "enum" if closed || row.get("enum").is_some() => choice_values(row, None),
                "json" if row.get("x-slot").is_some() && row.get("x-options").is_some() => {
                    choice_values(row, Some(spelling.get(path).copied().unwrap_or(false)))
                }
                "enum" => {
                    skipped.push(skip("open choice (free text): no closed set to sweep"));
                    continue;
                }
                "string" => {
                    skipped.push(skip("free text"));
                    continue;
                }
                _ => {
                    skipped.push(skip("structured value (JSON): no scalar to move"));
                    continue;
                }
            }
        };
        let cap = families.max_choices.unwrap_or(8).max(1);
        if values.len() > cap && kind != "number" && kind != "integer" {
            let n = values.len();
            values = (0..cap).map(|i| values[i * n / cap].clone()).collect();
        }
        // Literal values an expectation names run too.
        for e in expectations.iter().filter(|e| e.setting == path) {
            if !matches!(e.change.as_str(), Some("low" | "high"))
                && !values.iter().any(|(_, v)| *v == e.change)
            {
                values.push((label_of(&e.change), e.change.clone()));
            }
        }
        if values.is_empty() {
            skipped.push(skip("no value other than the default inside the bound"));
            continue;
        }
        let Some(fams) = families.route(path) else {
            skipped.push(skip("no family routes it"));
            continue;
        };
        settings.push(Setting {
            path: path.to_string(),
            kind: kind.to_string(),
            status,
            default: row.get("default").cloned(),
            values,
            families: fams.to_vec(),
            context: families
                .context
                .iter()
                .filter(|c| prefix_matches(&c.prefix, path))
                .flat_map(|c| c.set.clone())
                .collect(),
        });
    }
    (settings, skipped)
}

fn label_of(v: &Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_string)
}

/// A recorder that measures the whole fingerprint: the radio harness's metrics, the
/// `metric.sample` means and the ground-truth traffic state.
#[derive(Debug, Default)]
pub struct FingerprintRecorder {
    inner: SensitivityRecorder,
    /// metric -> (weighted sum, weight, is a count)
    samples: BTreeMap<String, (f64, f64, bool)>,
    /// [vehicles, vrus] x (speed sum, samples, stopped samples)
    kin: [(f64, u64, u64); 2],
    actors: [BTreeSet<u64>; 2],
}

fn is_vru(class: Option<&str>) -> bool {
    matches!(
        class,
        Some("pedestrian" | "cyclist" | "bicycle" | "ped" | "wheelchair")
    )
}

impl RunRecorder for FingerprintRecorder {
    fn write(&mut self, at: SimTime, r: &OwnedRecord) {
        self.inner.write(at, r);
        match r.channel {
            "metric.sample" => {
                if let Ok(s) = serde_json::from_slice::<v2xw_metrics::MetricSample>(&r.json)
                    && let Some(p) = s.value.point()
                    && p.is_finite()
                {
                    let count = matches!(s.value, v2xw_metrics::SampleValue::Count { .. });
                    let e = self.samples.entry(s.metric).or_insert((0.0, 0.0, count));
                    if count {
                        e.0 += p;
                        e.1 = 1.0;
                    } else {
                        let n = s.value.n().max(1) as f64;
                        e.0 += p * n;
                        e.1 += n;
                    }
                }
            }
            "gt.kinematics" => {
                if let Ok(k) = v2xw_metrics::channels::decode::<
                    v2xw_metrics::channels::GtKinematicsView,
                >(r)
                {
                    let i = usize::from(is_vru(k.class.as_deref()));
                    let e = &mut self.kin[i];
                    e.0 += k.speed_mps;
                    e.1 += 1;
                    if k.speed_mps < 0.1 {
                        e.2 += 1;
                    }
                    self.actors[i].insert(u64::from(k.actor.index()));
                }
            }
            _ => {}
        }
    }

    fn records_written(&self) -> Option<u64> {
        self.inner.records_written()
    }
}

/// Every numeric leaf of `v`, flattened to dotted names under `prefix`.
fn flatten(prefix: &str, v: &Value, out: &mut Fingerprint) {
    match v {
        Value::Number(n) => {
            if let Some(x) = n.as_f64() {
                out.insert(prefix.to_string(), x);
            }
        }
        Value::Bool(b) => {
            out.insert(prefix.to_string(), f64::from(u8::from(*b)));
        }
        Value::Object(m) => {
            for (k, v) in m {
                flatten(&format!("{prefix}.{k}"), v, out);
            }
        }
        Value::Array(a) => {
            for (i, v) in a.iter().enumerate() {
                flatten(&format!("{prefix}.{i}"), v, out);
            }
        }
        _ => {}
    }
}

/// Report fields that describe the recording rather than the system: they move with
/// anything (every extra record), so they would hide a dead setting behind a live one.
const BOOKKEEPING: &[&str] = &["report.records", "report.records_written", "report.keyframes", "report.deltas", "report.frames_written", "report.records_refused"];

impl FingerprintRecorder {
    /// The fingerprint, joined with the run report.
    #[must_use]
    pub fn finish(&self, report: &RunReport, duration_s: f64) -> Fingerprint {
        let m: Measured = self.inner.finish(report, duration_s);
        let mut f: Fingerprint = m.values.clone();
        for k in ["range_m", "range90_m", "prr_0_200", "prr_200_400", "prr_400_600", "prr_600_1000"] {
            if let Some(v) = m.get(k)
                && !m.prr_bins.is_empty()
            {
                f.insert(k.to_string(), v);
            }
        }
        if let Ok(v) = serde_json::to_value(report) {
            flatten("report", &v, &mut f);
        }
        f.retain(|k, _| !BOOKKEEPING.contains(&k.as_str()));
        for (k, (s, n, _)) in &self.samples {
            f.insert(format!("metric.{k}"), s / n.max(1.0));
        }
        for (i, who) in ["traffic", "vru"].iter().enumerate() {
            let (sum, n, stopped) = self.kin[i];
            if n > 0 {
                f.insert(format!("{who}.speed_mean_mps"), sum / n as f64);
                f.insert(format!("{who}.stopped_share"), stopped as f64 / n as f64);
                f.insert(format!("{who}.actors"), self.actors[i].len() as f64);
            }
        }
        f
    }
}

/// Runs `s` (validated) and fingerprints it.
///
/// # Errors
/// [`EngineError::Scenario`] when the loader refuses it; any build or run error.
pub fn fingerprint(s: Scenario) -> Result<Fingerprint> {
    s.validate()?;
    let duration = s.time.duration_s;
    let mut engine = Engine::build(s, "")?;
    let mut rec = FingerprintRecorder::default();
    let report = engine.run(&mut rec)?;
    Ok(rec.finish(&report, duration))
}

/// The metrics whose values differ between `base` and `run`.
#[must_use]
pub fn diff(base: &Fingerprint, run: &Fingerprint) -> BTreeMap<String, (Option<f64>, Option<f64>)> {
    let names: BTreeSet<&String> = base.keys().chain(run.keys()).collect();
    names
        .into_iter()
        .filter_map(|k| {
            let (b, v) = (base.get(k).copied(), run.get(k).copied());
            let same = match (b, v) {
                (Some(b), Some(v)) => b == v || (b - v).abs() <= 1e-12 * b.abs().max(1.0),
                (None, None) => true,
                _ => false,
            };
            (!same).then(|| (k.clone(), (b, v)))
        })
        .collect()
}

/// Whether an error is the loader refusing the scenario (as opposed to a run failure).
fn is_refusal(e: &EngineError) -> bool {
    matches!(e, EngineError::Scenario(_))
}

/// Runs the plan: each family's base once, then every perturbation, on `jobs` threads.
/// `progress` is called after each perturbation with (done, total, outcome).
///
/// # Errors
/// A family's base is refused or fails to run.
pub fn run(
    settings: Vec<Setting>,
    skipped: Vec<(String, String)>,
    bases: &BTreeMap<String, Scenario>,
    expectations: &[Expectation],
    jobs: usize,
    progress: impl Fn(usize, usize, &Outcome) + Sync,
) -> Result<Sweep> {
    run_cached(settings, skipped, bases, expectations, jobs, None, progress)
}

/// A cache key for one perturbation: the setting, the value, and the families' base
/// scenarios it may run on (so an edited family invalidates its entries).
fn cache_key(s: &Setting, value: &Value, bases: &BTreeMap<String, Scenario>) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for f in &s.families {
        f.hash(&mut h);
        serde_json::to_string(&bases.get(f)).unwrap_or_default().hash(&mut h);
    }
    serde_json::to_string(&s.context).unwrap_or_default().hash(&mut h);
    format!("{}={}#{:016x}", s.path, value, h.finish())
}

/// [`run`], with a JSON-lines cache of finished perturbations at `cache`: an entry whose
/// key matches is reused instead of re-run, and each new outcome is appended as it
/// finishes, so a sweep that is stopped (a memory cap, a closed laptop) resumes where it
/// stopped. Base fingerprints are always re-measured (seven short runs).
///
/// # Errors
/// As [`run`].
pub fn run_cached(
    settings: Vec<Setting>,
    skipped: Vec<(String, String)>,
    bases: &BTreeMap<String, Scenario>,
    expectations: &[Expectation],
    jobs: usize,
    cache: Option<&Path>,
    progress: impl Fn(usize, usize, &Outcome) + Sync,
) -> Result<Sweep> {
    let cached: BTreeMap<String, Outcome> = cache
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| {
            t.lines()
                .filter_map(|l| serde_json::from_str::<(String, Outcome)>(l).ok())
                .collect()
        })
        .unwrap_or_default();
    let sink: Option<Mutex<std::fs::File>> = cache.and_then(|p| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .ok()
            .map(Mutex::new)
    });
    let started = std::time::Instant::now();
    let used: BTreeSet<&String> = settings.iter().flat_map(|s| s.families.iter()).collect();
    let mut base_fp = BTreeMap::new();
    for name in used {
        let Some(s) = bases.get(name) else {
            return Err(EngineError::Scenario(ScenarioError::conflict(
                "families",
                format!("a route names family `{name}`, which the file does not define"),
            )));
        };
        base_fp.insert(name.clone(), Ok(fingerprint(s.clone())?));
    }
    let base_fp: Mutex<BTreeMap<String, std::result::Result<Fingerprint, String>>> =
        Mutex::new(base_fp);
    let work: Vec<(usize, usize)> = settings
        .iter()
        .enumerate()
        .flat_map(|(i, s)| (0..s.values.len()).map(move |j| (i, j)))
        .collect();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<Outcome>>> = Mutex::new(vec![None; work.len()]);
    std::thread::scope(|scope| {
        for _ in 0..jobs.max(1) {
            scope.spawn(|| {
                loop {
                    let k = next.fetch_add(1, Ordering::SeqCst);
                    let Some(&(i, j)) = work.get(k) else { break };
                    let key = cache_key(&settings[i], &settings[i].values[j].1, bases);
                    let o = match cached.get(&key) {
                        Some(o) => o.clone(),
                        None => {
                            let o = perturb(&settings[i], j, bases, &base_fp);
                            if let Some(f) = &sink
                                && let Ok(mut f) = f.lock()
                                && let Ok(line) = serde_json::to_string(&(&key, &o))
                            {
                                use std::io::Write as _;
                                let _ = writeln!(f, "{line}");
                            }
                            o
                        }
                    };
                    let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                    progress(n, work.len(), &o);
                    if let Ok(mut r) = results.lock() {
                        r[k] = Some(o);
                    }
                }
            });
        }
    });
    let outcomes: Vec<Outcome> = results
        .into_inner()
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .collect();
    let mut sweep = Sweep {
        settings,
        skipped,
        outcomes,
        bases: base_fp
            .into_inner()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(k, v)| v.ok().map(|v| (k, v)))
            .collect(),
        verdicts: Vec::new(),
        wall_s: started.elapsed().as_secs_f64(),
    };
    sweep.verdicts = sweep.judge(expectations);
    Ok(sweep)
}

/// One perturbation: the setting's `j`th value on the first family that accepts it.
fn perturb(
    s: &Setting,
    j: usize,
    bases: &BTreeMap<String, Scenario>,
    base_fp: &Mutex<BTreeMap<String, std::result::Result<Fingerprint, String>>>,
) -> Outcome {
    let (label, value) = s.values[j].clone();
    let mut o = Outcome {
        path: s.path.clone(),
        label,
        value: value.clone(),
        family: None,
        refusals: Vec::new(),
        moved: BTreeMap::new(),
        error: None,
        base: String::new(),
    };
    let mut set = s.context.clone();
    set.insert(s.path.clone(), value);
    for fam in &s.families {
        let Some(family_base) = bases.get(fam) else {
            continue;
        };
        // The base this perturbation is compared with: the family with the context.
        let key = if s.context.is_empty() {
            fam.clone()
        } else {
            format!("{fam} + {}", serde_json::to_string(&s.context).unwrap_or_default())
        };
        let known = base_fp.lock().ok().and_then(|m| m.get(&key).cloned());
        let bfp = match known {
            Some(b) => b,
            None => {
                let b = apply(family_base, &s.context)
                    .and_then(fingerprint)
                    .map_err(|e| e.to_string());
                if let Ok(mut m) = base_fp.lock() {
                    m.insert(key.clone(), b.clone());
                }
                b
            }
        };
        let bfp = match bfp {
            Ok(b) => b,
            Err(e) => {
                o.refusals.push((fam.clone(), format!("context refused: {e}")));
                continue;
            }
        };
        o.base = key;
        let changed = match apply(family_base, &set) {
            Ok(c) => c,
            Err(e) => {
                o.refusals.push((fam.clone(), e.to_string()));
                continue;
            }
        };
        match fingerprint(changed) {
            Ok(fp) => {
                o.family = Some(fam.clone());
                o.moved = diff(&bfp, &fp);
                return o;
            }
            Err(e) if is_refusal(&e) => o.refusals.push((fam.clone(), e.to_string())),
            Err(e) => {
                o.family = Some(fam.clone());
                o.error = Some(e.to_string());
                return o;
            }
        }
    }
    o
}

impl Sweep {
    /// The expectations judged against the outcomes.
    #[must_use]
    pub fn judge(&self, expectations: &[Expectation]) -> Vec<Verdict> {
        expectations
            .iter()
            .map(|e| {
                let found = self.outcomes.iter().find(|o| {
                    o.path == e.setting
                        && match e.change.as_str() {
                            Some(l @ ("low" | "high")) => o.label == l,
                            _ => o.value == e.change,
                        }
                });
                let mut v = Verdict {
                    expectation: e.clone(),
                    base: None,
                    value: None,
                    pass: false,
                    note: String::new(),
                };
                let Some(o) = found else {
                    v.note = "not swept (no such setting, or no such end)".into();
                    return v;
                };
                let Some(fam) = &o.family else {
                    v.note = "refused by every family".into();
                    return v;
                };
                if let Some(err) = &o.error {
                    v.note = format!("run failed: {err}");
                    return v;
                }
                let key = if o.base.is_empty() { fam } else { &o.base };
                let base = self.bases.get(key).and_then(|b| b.get(&e.metric)).copied();
                let value = match o.moved.get(&e.metric) {
                    Some((_, after)) => *after,
                    None => base,
                };
                let row = Row {
                    name: e.setting.clone(),
                    set: BTreeMap::new(),
                    base_set: BTreeMap::new(),
                    metric: e.metric.clone(),
                    expect: e.expect,
                    band: None,
                    min_change: e.min_change,
                    source: Some(e.source.clone()),
                };
                let r = judge(&row, base, value);
                v.base = base;
                v.value = value;
                v.pass = r.pass;
                v.note = r.note;
                v
            })
            .collect()
    }

    /// The settings every one of whose runs moved nothing.
    #[must_use]
    pub fn dead(&self) -> Vec<&Setting> {
        self.settings
            .iter()
            .filter(|s| {
                let mine: Vec<&Outcome> = self.outcomes.iter().filter(|o| o.path == s.path).collect();
                !mine.is_empty()
                    && mine.iter().any(|o| o.dead())
                    && mine.iter().all(|o| o.dead() || o.family.is_none())
            })
            .collect()
    }

    /// The verdicts that failed.
    #[must_use]
    pub fn contradictions(&self) -> Vec<&Verdict> {
        self.verdicts.iter().filter(|v| !v.pass).collect()
    }

    /// The perturbations every family refused.
    #[must_use]
    pub fn refused(&self) -> Vec<&Outcome> {
        self.outcomes
            .iter()
            .filter(|o| o.family.is_none())
            .collect()
    }
}

/// The headline metrics the matrix gives a column each.
pub const HEADLINE: &[(&str, &str)] = &[
    ("pdr", "PDR"),
    ("prr", "PRR"),
    ("range90_m", "range90"),
    ("collision_rate", "coll"),
    ("cbr_mean", "CBR"),
    ("e2e_ms_mean", "e2e"),
    ("aoi_ms", "AoI"),
    ("frames_tx", "frames"),
    ("psdu_bytes", "bytes"),
    ("gn_reach", "GN reach"),
    ("traffic.speed_mean_mps", "speed"),
    ("traffic.stopped_share", "stopped"),
    ("vru.speed_mean_mps", "VRU speed"),
];

fn cell(o: &Outcome, metric: &str) -> String {
    match o.moved.get(metric) {
        None => String::new(),
        Some((Some(b), Some(v))) => {
            let rel = (v - b) / b.abs().max(1e-12);
            let arrow = if v > b { "↑" } else { "↓" };
            if b.abs() < 1e-12 {
                format!("{arrow}{v:.3}")
            } else {
                format!("{arrow}{:.0}%", rel.abs() * 100.0)
            }
        }
        Some((None, Some(_))) => "new".into(),
        Some((Some(_), None)) => "gone".into(),
        Some((None, None)) => String::new(),
    }
}

fn short(v: &Value) -> String {
    let s = label_of(v);
    if s.len() > 40 { format!("{}…", &s[..s.char_indices().nth(39).map_or(s.len(), |(i, _)| i)]) } else { s }
}

/// The three lists and the matrix, in Markdown.
#[must_use]
pub fn markdown(sweep: &Sweep, title_note: &str) -> String {
    let mut s = String::new();
    let runs = sweep.outcomes.len();
    let ran = sweep.outcomes.iter().filter(|o| o.family.is_some()).count();
    s.push_str(&format!(
        "{title_note}\n\n**{} settings swept, {} perturbations ({ran} ran, {} refused everywhere), {} settings skipped; {:.0} s wall.**\n\n",
        sweep.settings.len(),
        runs,
        runs - ran,
        sweep.skipped.len(),
        sweep.wall_s
    ));
    s.push_str("Families (base fingerprints):\n\n");
    for (name, fp) in &sweep.bases {
        let pick = |k: &str| fp.get(k).map_or("-".into(), |v| format!("{v:.3}"));
        s.push_str(&format!(
            "- `{name}`: {} metrics; pdr {}, cbr {}, e2e {} ms, speed {} m/s\n",
            fp.len(),
            pick("pdr"),
            pick("cbr_mean"),
            pick("e2e_ms_mean"),
            pick("traffic.speed_mean_mps")
        ));
    }
    // (a)
    let dead = sweep.dead();
    s.push_str(&format!("\n## (a) Dead settings — no metric moved ({})\n\n", dead.len()));
    s.push_str("Same seed, so an unread setting gives a bit-identical run. A setting listed here is either not read on this family, or read by something no reported metric observes.\n\n| Setting | Status | Family | Values tried |\n|---|---|---|---|\n");
    for d in &dead {
        let fams: BTreeSet<String> = sweep
            .outcomes
            .iter()
            .filter(|o| o.path == d.path)
            .filter_map(|o| o.family.clone())
            .collect();
        let vals: Vec<String> = d.values.iter().map(|(_, v)| short(v)).collect();
        s.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            d.path,
            d.status,
            fams.into_iter().collect::<Vec<_>>().join(", "),
            vals.join(", ")
        ));
    }
    // Ends that moved nothing, of settings that are live elsewhere.
    let dead_paths: BTreeSet<&str> = dead.iter().map(|d| d.path.as_str()).collect();
    let quiet: Vec<&Outcome> = sweep
        .outcomes
        .iter()
        .filter(|o| o.dead() && !dead_paths.contains(o.path.as_str()))
        .collect();
    s.push_str(&format!(
        "\n### Values that moved nothing, of settings live at another value ({})\n\nUsually the value equals the model's implicit default (a field with no published default), or the family never exercises it; a choice listed here may also be a value the model treats like the default.\n\n| Setting | Value | Family |\n|---|---|---|\n",
        quiet.len()
    ));
    for o in &quiet {
        s.push_str(&format!(
            "| `{}` | {} | {} |\n",
            o.path,
            short(&o.value),
            o.family.as_deref().unwrap_or("-")
        ));
    }
    // (b)
    let bad = sweep.contradictions();
    s.push_str(&format!(
        "\n## (b) Effects that contradict the expectation table ({} of {})\n\n| Setting | Change | Metric | Expected | Base | Changed | Note | Theory |\n|---|---|---|---|---|---|---|---|\n",
        bad.len(),
        sweep.verdicts.len()
    ));
    let num = |v: Option<f64>| v.map_or("-".into(), |x| format!("{x:.4}"));
    for v in &bad {
        let e = &v.expectation;
        s.push_str(&format!(
            "| `{}` | {} | `{}` | {:?} | {} | {} | {} | {} |\n",
            e.setting,
            label_of(&e.change),
            e.metric,
            e.expect,
            num(v.base),
            num(v.value),
            v.note,
            e.source
        ));
    }
    // (c)
    let refused = sweep.refused();
    s.push_str(&format!(
        "\n## (c) Perturbations refused by every family ({})\n\nA value inside the published recommended range (or bound) that no family's loader accepts: either the family lacks a prerequisite the setting needs (extend the families file), or the loader refuses a combination it should accept.\n\n| Setting | Value | Refusal (first family) |\n|---|---|---|\n",
        refused.len()
    ));
    for o in &refused {
        let why = o
            .refusals
            .first()
            .map_or(String::new(), |(f, m)| format!("{f}: {}", m.replace('|', "/").replace('\n', " ")));
        let why: String = why.chars().take(220).collect();
        s.push_str(&format!("| `{}` | {} | {} |\n", o.path, short(&o.value), why));
    }
    // Expectations that held.
    let good: Vec<&Verdict> = sweep.verdicts.iter().filter(|v| v.pass).collect();
    s.push_str(&format!("\n## Expectations that held ({})\n\n| Setting | Change | Metric | Expected | Base | Changed |\n|---|---|---|---|---|---|\n", good.len()));
    for v in good {
        let e = &v.expectation;
        s.push_str(&format!(
            "| `{}` | {} | `{}` | {:?} | {} | {} |\n",
            e.setting,
            label_of(&e.change),
            e.metric,
            e.expect,
            num(v.base),
            num(v.value)
        ));
    }
    // Matrix.
    s.push_str("\n## Matrix — setting × metric (relative change against the family's base)\n\n`↑12%` / `↓12%`: the metric rose or fell by that share; empty: unchanged (bit-identical). *Moved* counts every fingerprint metric that changed; *largest* names the three largest relative changes outside the headline columns.\n\n| Setting | Value | Family | Moved |");
    for (_, h) in HEADLINE {
        s.push_str(&format!(" {h} |"));
    }
    s.push_str(" Largest other |\n|---|---|---|---|");
    for _ in HEADLINE {
        s.push_str("---|");
    }
    s.push_str("---|\n");
    for o in &sweep.outcomes {
        let fam = match (&o.family, &o.error) {
            (None, _) => "refused".to_string(),
            (Some(f), Some(_)) => format!("{f} (run failed)"),
            (Some(f), None) => f.clone(),
        };
        s.push_str(&format!("| `{}` | {} | {} | {} |", o.path, short(&o.value), fam, o.moved.len()));
        for (k, _) in HEADLINE {
            s.push_str(&format!(" {} |", cell(o, k)));
        }
        let mut others: Vec<(&String, f64)> = o
            .moved
            .iter()
            .filter(|(k, _)| !HEADLINE.iter().any(|(h, _)| h == k))
            .filter_map(|(k, (b, v))| match (b, v) {
                (Some(b), Some(v)) => Some((k, ((v - b) / b.abs().max(1e-9)).abs())),
                _ => Some((k, f64::INFINITY)),
            })
            .collect();
        others.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
        let top: Vec<String> = others
            .iter()
            .take(3)
            .map(|(k, _)| format!("{} {}", k, cell(o, k)))
            .collect();
        s.push_str(&format!(" {} |\n", top.join("; ")));
    }
    // Skipped.
    let mut by_reason: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (p, r) in &sweep.skipped {
        by_reason.entry(r.as_str()).or_default().push(p.as_str());
    }
    s.push_str(&format!("\n## Settings not swept ({})\n\n", sweep.skipped.len()));
    for (r, ps) in by_reason {
        s.push_str(&format!("- **{r}** ({}): ", ps.len()));
        let shown: Vec<String> = ps.iter().take(40).map(|p| format!("`{p}`")).collect();
        s.push_str(&shown.join(", "));
        if ps.len() > 40 {
            s.push_str(&format!(", and {} more", ps.len() - 40));
        }
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_range_text_parser_reads_ranges_not_names() {
        assert_eq!(numbers_in("0 (default) to 0.05."), vec![0.0, 0.05]);
        assert_eq!(numbers_in("10-20 Hz"), vec![10.0, 20.0]);
        assert_eq!(numbers_in("-95 to -85 dBm"), vec![-95.0, -85.0]);
        assert_eq!(numbers_in("10 000 to 50,000 m"), vec![10_000.0, 50_000.0]);
        assert!(numbers_in("IEEE 802.11p").is_empty());
        assert_eq!(numbers_in("3 s"), vec![3.0]);
    }

    #[test]
    fn an_identical_run_is_dead_and_a_moved_one_is_not() {
        let base: Fingerprint = [("pdr".to_string(), 0.9), ("cbr_mean".to_string(), 0.2)]
            .into_iter()
            .collect();
        let mut o = Outcome {
            path: "x".into(),
            label: "high".into(),
            value: Value::from(1),
            family: Some("dsrc".into()),
            refusals: vec![],
            moved: diff(&base, &base.clone()),
            error: None,
            base: "dsrc".into(),
        };
        assert!(o.dead());
        let mut moved = base.clone();
        moved.insert("pdr".into(), 0.8);
        o.moved = diff(&base, &moved);
        assert!(!o.dead());
        assert_eq!(o.moved.len(), 1);
        assert!(o.rel("pdr").is_some_and(|r| r < -0.1));
        // A metric that appears only in the changed run is a move too.
        let mut extra = base.clone();
        extra.insert("gn_reach".into(), 1.0);
        assert_eq!(diff(&base, &extra).len(), 1);
    }

    #[test]
    fn numeric_ends_stay_inside_the_bound_and_skip_the_default() {
        let row = serde_json::json!({"default": 20.0, "minimum": 0.0, "maximum": 33.0,
                                     "x-recommended": "10 to 23 dBm"});
        let v = numeric_ends(&row, false);
        assert_eq!(v, vec![("low".into(), Value::from(10.0)), ("high".into(), Value::from(23.0))]);
        let row = serde_json::json!({"default": 4, "minimum": 1, "maximum": 5});
        let v = numeric_ends(&row, true);
        assert_eq!(v, vec![("low".into(), Value::from(2)), ("high".into(), Value::from(5))]);
    }
}
