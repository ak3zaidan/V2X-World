//! The agent's control tools: everything the simulator does through its API, in as few
//! round trips and as few tokens as the task allows.
//!
//! [`super::agent_tools`] offers these beside the original nine; [`dispatch`] carries them
//! out. Every one goes through a published server method ([`CONTROL_METHODS`]) or reads a
//! table compiled into this build ([`crate::catalog`]), so the agent can do nothing a
//! person in the Studio cannot.
//!
//! | Tool | What it does | Server methods |
//! |---|---|---|
//! | `simulate` | build one or more variants of a base (same seed), validate all, run each to the end, analyse, compare — one call for a run, an A/B, or a sweep | `scenario.*`, `run.start`, `run.status`, `metrics.query`, `inspect.entity` |
//! | `find_settings` | search every published setting (about 360) by plain words or synonyms: path, type, allowed values, unit, range, default, recommended range, whether the engine acts on it | none (the build's own schema) |
//! | `run_control` | start / pause / resume / step / seek / run to a time / speed / stop / status | `run.*`, `scenario.get` |
//! | `live_metrics` | the current run's values so far, with units and health bands | `metrics.query` |
//! | `query_metric` | one metric over time, or broken down by a dimension | `metrics.query` |
//! | `inspect` | a node, a link, a backend entity, or a metric's provenance | `inspect.*`, `explain` |
//! | `list_runs` | this conversation's runs and what each changed | none |
//! | `compare_runs` | any two or more of them, metric by metric ([`crate::experiment`]) | none |
//!
//! # Compact by construction
//!
//! A tool result is read by the model on every later round of the turn, so its size is
//! paid many times. Results here are summaries: the analysis without its info-level
//! findings, an experiment's significant rows in full and the rest by name, arrays cut to
//! a stated length with the count of what was cut, numbers formatted the one way the
//! analyst formats them ([`crate::analyst::fmt`]) so the grounding check matches them.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value, json};

use super::{Agent, AgentEvent, Change, Prepared, RunRecord, set_pointer, to_pointer};
use crate::analyst::{self, Analysis, Comparison, Severity, fmt};
use crate::catalog;
use crate::error::{CopilotError, Result};
use crate::experiment::{self, Arm, Experiment, HealthBand};
use crate::provider::LlmProvider;
use crate::scenario::check_document;
use crate::session::ToolOutcome;
use crate::transport::RpcTransport;

/// Every server method the control tools call, beyond [`super::AGENT_METHODS`].
pub const CONTROL_METHODS: [&str; 9] = [
    "run.resume",
    "run.step",
    "run.seek",
    "run.speed",
    "run.stop",
    "inspect.node",
    "inspect.link",
    "explain",
    "scenario.validate",
];

/// Whole-run metrics read after every run beyond the analyst's headline set, when the
/// run's catalogue has them: detection, security overhead, privacy, pedestrians,
/// applications. A name ending in `_` is a prefix.
const EXTRA_METRICS: [&str; 30] = [
    "det_recall",
    "det_precision",
    "det_f1",
    "det_fpr",
    "det_tp",
    "det_fp",
    "det_fn",
    "time_to_decision",
    "envelope_overhead",
    "cert_bytes_share",
    "full_cert_share",
    "air_bytes_per_payload_byte",
    "bytes_air",
    "bytes_total",
    "bytes_per_vehicle_hour",
    "verify_cost",
    "crl_entries",
    "crl_bytes",
    "loss_rate",
    "airtime_per_node",
    "channel_occupancy",
    "pdr_by_cause[jammed]",
    "pdr_by_cause[collision]",
    "pdr_by_cause[below-sensitivity]",
    "ped_",
    "apps_",
    "warnings",
    "priority_",
    "travel_time",
    "acceleration",
];

/// The most extra metrics read after one run, so a catalogue with hundreds of rows does not
/// turn the read into hundreds of requests.
const EXTRA_LIMIT: usize = 40;

/// What the control tools remember across a conversation.
#[derive(Debug, Default)]
pub(super) struct Harness {
    /// The shipped scenarios, compact, read once.
    scenarios: Option<Value>,
    /// Per run (by index in `Agent::runs`): extra metrics, units, seed.
    meta: BTreeMap<usize, RunMeta>,
    /// Health bands from the latest catalogue.
    bands: BTreeMap<String, HealthBand>,
    /// The experiments of this conversation, newest last, with the turn they belong to.
    pub(super) experiments: Vec<(usize, Experiment)>,
    /// Which turn this is (`ask` calls so far).
    pub(super) turn: usize,
    /// Per `setup` (id = index + 1): what the Settings window held before it, as
    /// `scenario.get` answered, so Undo can put it back.
    pub(super) setups: Vec<Option<Value>>,
}

#[derive(Debug, Default, Clone)]
struct RunMeta {
    extra: BTreeMap<String, f64>,
    units: BTreeMap<String, String>,
    seed: String,
    /// Every other metric the run's catalogue has, as `base[dim|dim]` (its breakdown
    /// dimensions), so the model asks query_metric for a name that exists.
    catalogue: Vec<String>,
}

impl Harness {
    /// Forgets everything (a new conversation).
    pub(super) fn reset(&mut self) {
        let scenarios = self.scenarios.take();
        *self = Harness::default();
        self.scenarios = scenarios;
    }

    /// The extra metrics of runs `from..to`, as Markdown: run data the analyst's sections do
    /// not show (detection, overhead, loss counts, pedestrians, applications).
    pub(super) fn extras_markdown(&self, runs: &[RunRecord], from: usize) -> String {
        let mut md = String::new();
        for (i, r) in runs.iter().enumerate().skip(from) {
            let Some(m) = self.meta.get(&i).filter(|m| !m.extra.is_empty()) else {
                continue;
            };
            md.push_str(&format!(
                "## Other measured values: {}\n\n| Metric | Value | Unit |\n|---|---|---|\n",
                r.label
            ));
            for (k, v) in &m.extra {
                md.push_str(&format!(
                    "| `{k}` | {} | {} |\n",
                    fmt(*v),
                    m.units.get(k).map_or("", String::as_str)
                ));
            }
            md.push('\n');
        }
        md
    }

    /// The experiments of one turn, as Markdown.
    pub(super) fn markdown_for_turn(&self, turn: usize) -> String {
        self.experiments
            .iter()
            .filter(|(t, _)| *t == turn)
            .map(|(_, e)| e.to_markdown())
            .collect()
    }
}

/// The control tools, in the chat-completions shape both providers take.
#[must_use]
pub fn tools() -> Vec<Value> {
    let tool = |name: &str, description: &str, parameters: Value| {
        json!({"type": "function", "function": {
            "name": name, "description": description, "parameters": parameters}})
    };
    let change = json!({"type": "object", "additionalProperties": false,
        "required": ["key", "value"],
        "properties": {
            "key": {"type": "string", "description": "dotted key, e.g. radio.rat"},
            "value": {"description": "new value, or {multiply: k} | {append: item} | {remove: true}"},
            "why": {"type": "string"}}});
    vec![
        tool(
            "setup",
            "Set up the person's next run without running it: start from `base` (a \
             scenario id, or `current` = what the Settings window shows now, the default), \
             apply the changes, validate with the engine's loader, and put the result in \
             the Settings window as the draft their Run button starts. Use it for every \
             'set up / configure / use X' request. The person sees each change as a chip \
             with Undo. Returns ok, or the loader's errors to fix and retry.",
            json!({"type": "object", "additionalProperties": false,
                   "required": ["changes"],
                   "properties": {
                       "base": {"type": "string", "description": "a scenario id, or `current` (default)"},
                       "reason": {"type": "string"},
                       "changes": {"type": "array", "items": change.clone()}}}),
        ),
        tool(
            "list_cities",
            "The city library: id, short name, area, and whether its map is downloaded. \
             Check here before saying a city is not available.",
            json!({"type": "object", "additionalProperties": false,
                   "properties": {"query": {"type": "string", "description": "a name, id or state; empty lists all"}}}),
        ),
        tool(
            "simulate",
            "Build, validate, run to the end and analyse one or more variants of a base \
             scenario, all with the base's seed, and compare them. One call for a single run \
             (no variants), an A/B or a sweep (variants, the first is the baseline). Returns \
             the analyst's findings and, for 2+ arms, every metric's change with the \
             significant ones called out, their health and the findings that explain them. \
             A single run also lists `other_metrics` (name[breakdown dims]) for \
             query_metric. Nothing runs if any variant is invalid; the errors come back.",
            json!({"type": "object", "additionalProperties": false,
                   "required": ["base", "reason"],
                   "properties": {
                       "base": {"type": "string", "description": "a scenario id from the list, `current`, or `last`"},
                       "reason": {"type": "string", "description": "why this base, one sentence"},
                       "changes": {"type": "array", "items": change.clone(),
                                   "description": "applied to every variant"},
                       "variants": {"type": "array", "items": {
                           "type": "object", "additionalProperties": false, "required": ["label"],
                           "properties": {"label": {"type": "string"},
                                          "changes": {"type": "array", "items": change.clone()}}}},
                       "label": {"type": "string", "description": "name of a single run"},
                       "duration_s": {"type": "number", "description": "simulated seconds for every arm"},
                       "compare_to_last": {"type": "boolean",
                           "description": "use this conversation's last run as the baseline arm (follow-ups)"}}}),
        ),
        tool(
            "find_settings",
            "Search every published scenario setting by plain words or synonyms (\"weaker \
             radio\", \"C-V2X\", \"truck headway\", \"jaywalking\"): path, type, allowed \
             values, unit, range, default, recommended range, and whether the engine acts on \
             it. Attack and jam searches also list the attacker kinds and jammer models. \
             Never guess a key: search first.",
            json!({"type": "object", "additionalProperties": false, "required": ["query"],
                   "properties": {"query": {"type": "string"}}}),
        ),
        tool(
            "run_control",
            "Drive the run the engine is serving: start (the prepared scenario, or the \
             current one), pause, resume, step {seconds}, seek {t_s} (backwards re-runs from \
             0 with the same seed), run_to {t_s} (omit t_s for the end), speed {speed}, stop, \
             status. Returns the run's state and time.",
            json!({"type": "object", "additionalProperties": false, "required": ["action"],
                   "properties": {
                       "action": {"type": "string", "enum": ["start", "pause", "resume", "step",
                                  "seek", "run_to", "speed", "stop", "status"]},
                       "t_s": {"type": "number"}, "seconds": {"type": "number"},
                       "speed": {"type": "number"}, "paused": {"type": "boolean"}}}),
        ),
        tool(
            "live_metrics",
            "The current run's metrics from 0 to now (live or finished), each with unit and \
             health band (healthy, caution, unhealthy). Omit `metrics` for the headline set.",
            json!({"type": "object", "additionalProperties": false,
                   "properties": {"metrics": {"type": "array", "items": {"type": "string"}}}}),
        ),
        tool(
            "query_metric",
            "One metric of the current run over time (`bins`) or broken down by a dimension \
             (`group_by`: node, dist_bin, cause, stage, region, …), optionally filtered \
             (`where`).",
            json!({"type": "object", "additionalProperties": false, "required": ["metric"],
                   "properties": {"metric": {"type": "string"}, "group_by": {"type": "string"},
                                  "bins": {"type": "integer"}, "from_s": {"type": "number"},
                                  "to_s": {"type": "number"}, "where": {"type": "object"}}}),
        ),
        tool(
            "inspect",
            "Look inside the current run: a node (telemetry, queues, neighbours), a link \
             (tx, rx), the credential backend (entity `backend`, which lists its entities), \
             or a headline metric's provenance (explain). Only for a question the \
             simulate result does not answer.",
            json!({"type": "object", "additionalProperties": false, "required": ["target"],
                   "properties": {"target": {"type": "string", "enum": ["node", "link", "entity", "explain"]},
                                  "node": {"type": "integer"}, "tx": {"type": "integer"},
                                  "rx": {"type": "integer"}, "entity": {"type": "string"},
                                  "metric": {"type": "string"}, "t_s": {"type": "number"}}}),
        ),
        tool(
            "list_runs",
            "This conversation's runs: index, label, what changed, run id.",
            json!({"type": "object", "additionalProperties": false, "properties": {}}),
        ),
    ]
}

/// Carries out a control tool, or `None` when `tool` is not one.
pub(super) fn dispatch<P: LlmProvider, T: RpcTransport>(
    agent: &mut Agent<P, T>,
    tool: &str,
    args: &Value,
    sink: &mut dyn FnMut(AgentEvent),
    comparison: &mut Option<Comparison>,
) -> Option<ToolOutcome> {
    let result = match tool {
        "simulate" => simulate(agent, args, sink, comparison),
        "find_settings" => Ok(catalog::find_settings(
            args.get("query").and_then(Value::as_str).unwrap_or(""),
            12,
        )),
        "setup" => setup(agent, args, sink),
        "list_cities" => Ok(catalog::cities(
            args.get("query").and_then(Value::as_str).unwrap_or(""),
        )),
        "run_control" => run_control(agent, args, sink),
        "live_metrics" => live_metrics(agent, args),
        "query_metric" => query_metric(agent, args),
        "inspect" => inspect(agent, args),
        "list_runs" => Ok(list_runs(agent)),
        _ => return None,
    };
    Some(match result {
        Ok(result) => ToolOutcome::Ok { result },
        Err(e) => ToolOutcome::Failed {
            message: e.to_string(),
        },
    })
}

fn err(method: &str, message: impl Into<String>) -> CopilotError {
    CopilotError::Rpc {
        method: method.into(),
        message: message.into(),
    }
}

// ----------------------------------------------------------------------------- scenarios

/// The shipped scenarios, compact (`id: description`), read once per agent.
pub(super) fn scenario_list<P: LlmProvider, T: RpcTransport>(agent: &mut Agent<P, T>) -> Value {
    if let Some(v) = &agent.harness.scenarios {
        return v.clone();
    }
    let v = agent
        .rpc
        .call("scenario.list", &json!({"kind": "presets"}))
        .map(|v| {
            Value::Array(
                v.get("items")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(|i| {
                        let id = i.get("id").and_then(Value::as_str).unwrap_or("");
                        let d = i.get("description").and_then(Value::as_str).unwrap_or("");
                        json!(format!("{id}: {}", catalog::shorten(d, 150)))
                    })
                    .collect(),
            )
        })
        .unwrap_or(Value::Null);
    if !v.is_null() {
        agent.harness.scenarios = Some(v.clone());
    }
    v
}

/// One requested change after [`crate::repair`] and application.
pub(super) struct Applied {
    /// The published key it landed on.
    pub key: String,
    pub before: Value,
    pub after: Value,
    /// What was read differently from how it was written, and any companion set.
    pub notes: Vec<String>,
}

/// Repairs one change as the model wrote it ([`crate::repair::repair`]: the key, the
/// value's unit or synonym, a companion setting) and applies it.
///
/// # Errors
/// When the key could mean several settings, or the change cannot be applied.
pub(super) fn apply_repaired(doc: &mut Value, key: &str, value: &Value) -> Result<Applied> {
    let r = crate::repair::repair(key, value).map_err(|m| err("prepare_scenario", m))?;
    let (before, after) = apply_change(doc, &r.key, &r.value)?;
    let mut notes = r.notes;
    for (k, v) in &r.companions {
        let (_, a) = apply_change(doc, k, v)?;
        notes.push(format!("also set {k} = {a}"));
    }
    // A post-quantum or hybrid signature needs an on-board unit that publishes its costs;
    // when the base's does not, the loader names it, and the one the shipped
    // `scenarios/pq-signatures.yaml` runs every scheme on is taken instead.
    if r.key == "security.signature"
        && after.as_str().is_some_and(|s| !s.starts_with("ecdsa"))
        && check_document(doc.clone())
            .errors
            .iter()
            .any(|e| e.field.as_deref() == Some("nodes.default_obu"))
    {
        let obu = json!("obu/generic-automotive-soc-no-hsm");
        apply_change(doc, "nodes.default_obu", &obu)?;
        notes.push(format!(
            "the base's on-board unit cannot price {after}, so nodes.default_obu = {obu} \
             (as scenarios/pq-signatures.yaml)"
        ));
    }
    Ok(Applied {
        key: r.key,
        before,
        after,
        notes,
    })
}

/// A tool's `changes` argument in the one shape the tools apply, `[{key, value, why}]`,
/// from the shapes models also write: a map `{"radio.rat": "lte-v2x-pc5"}`, a single change
/// object, a change whose key is under `path` / `setting` / `name` and whose value is under
/// `to` / `set`. Anything else is passed through for the tool to refuse.
pub(super) fn changes_of(v: Option<&Value>) -> Vec<Value> {
    let one = |c: &Value| -> Value {
        let Some(o) = c.as_object() else {
            return c.clone();
        };
        if o.contains_key("key") && o.contains_key("value") {
            return c.clone();
        }
        let key = ["key", "path", "setting", "name", "field"]
            .iter()
            .find_map(|k| o.get(*k).filter(|v| v.is_string()));
        let value = ["value", "to", "set", "new_value", "new"]
            .iter()
            .find_map(|k| o.get(*k));
        match (key, value) {
            (Some(k), Some(v)) => {
                json!({"key": k, "value": v, "why": o.get("why").cloned().unwrap_or(json!(""))})
            }
            _ => c.clone(),
        }
    };
    match v {
        Some(Value::Array(a)) => a.iter().map(one).collect(),
        Some(Value::Object(o)) if o.contains_key("key") || o.contains_key("path") => {
            vec![one(&Value::Object(o.clone()))]
        }
        Some(Value::Object(o)) => o
            .iter()
            .map(|(k, v)| json!({"key": k, "value": v, "why": ""}))
            .collect(),
        Some(Value::String(s)) => serde_json::from_str::<Value>(s)
            .ok()
            .filter(|p| p.is_array() || p.is_object())
            .map(|p| changes_of(Some(&p)))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// `why`, with the repair notes after it.
pub(super) fn why_with(why: String, notes: &[String]) -> String {
    if notes.is_empty() {
        return why;
    }
    let n = format!("(read: {})", notes.join("; "));
    if why.is_empty() {
        n
    } else {
        format!("{why} {n}")
    }
}

/// Applies one change to a document. Returns the value before and after.
///
/// # Errors
/// When a `multiply` meets a non-number, an `append` a non-list, or a jammer's `"centre"`
/// a world whose middle is not known.
pub(super) fn apply_change(doc: &mut Value, key: &str, value: &Value) -> Result<(Value, Value)> {
    let pointer = to_pointer(key);
    let before = doc.pointer(&pointer).cloned().unwrap_or(Value::Null);
    let op = value
        .as_object()
        .filter(|o| o.len() == 1)
        .and_then(|o| o.iter().next());
    let mut after = match op {
        Some((k, v)) if k == "multiply" => {
            let (Some(b), Some(m)) = (before.as_f64(), v.as_f64()) else {
                return Err(err(
                    "prepare_scenario",
                    format!("`{key}` is not a number in the base, so it cannot be multiplied"),
                ));
            };
            json!(b * m)
        }
        Some((k, v)) if k == "add" && v.is_number() => {
            let (Some(b), Some(m)) = (before.as_f64(), v.as_f64()) else {
                return Err(err(
                    "prepare_scenario",
                    format!("`{key}` is not a number in the base"),
                ));
            };
            json!(b + m)
        }
        Some((k, item)) if k == "append" => {
            let mut list = match &before {
                Value::Array(a) => a.clone(),
                Value::Null => Vec::new(),
                _ => {
                    return Err(err(
                        "prepare_scenario",
                        format!("`{key}` is not a list, so nothing can be appended to it"),
                    ));
                }
            };
            match item {
                Value::Array(items) => list.extend(items.iter().cloned()),
                other => list.push(other.clone()),
            }
            Value::Array(list)
        }
        Some((k, v)) if k == "remove" && v.as_bool() == Some(true) => {
            remove_pointer(doc, &pointer);
            return Ok((before, Value::Null));
        }
        _ => value.clone(),
    };
    if key
        .trim_start_matches('/')
        .replace('/', ".")
        .starts_with("threats.jammers")
    {
        resolve_centre(doc, &mut after)?;
    }
    set_pointer(doc, &pointer, after.clone());
    Ok((before, after))
}

fn remove_pointer(doc: &mut Value, pointer: &str) {
    let Some((parent, last)) = pointer.rsplit_once('/') else {
        return;
    };
    let last = last.replace("~1", "/").replace("~0", "~");
    match doc.pointer_mut(parent) {
        Some(Value::Object(o)) => {
            o.remove(&last);
        }
        Some(Value::Array(a)) => {
            if let Ok(i) = last.parse::<usize>() {
                if i < a.len() {
                    a.remove(i);
                }
            }
        }
        _ => {}
    }
}

/// Replaces `position_m: "centre"` in jammer parameters with the middle of a procedural
/// grid: junction `(i, j)` sits at `(half + i·block_x, half + j·block_y)` with
/// `half = lanes_per_direction · lane_width` (`v2xw_world::procedural`).
fn resolve_centre(doc: &Value, v: &mut Value) -> Result<()> {
    let items: Vec<&mut Value> = match v {
        Value::Array(a) => a.iter_mut().collect(),
        Value::Object(_) => vec![v],
        _ => return Ok(()),
    };
    for item in items {
        let wants = item
            .pointer("/params/position_m")
            .and_then(Value::as_str)
            .is_some_and(|s| matches!(s.to_lowercase().as_str(), "centre" | "center" | "middle"));
        let unplaced = item.get("params").is_none_or(|p| {
            p.get("position_m").is_none()
                && p.get("follow_node").is_none()
                && p.get("path_m").is_none()
        });
        if !wants && !unplaced {
            continue;
        }
        let p = |k: &str| {
            doc.pointer(&format!("/world/source/params/{k}"))
                .and_then(Value::as_f64)
        };
        let generator = doc
            .pointer("/world/source/generator")
            .and_then(Value::as_str)
            .unwrap_or("");
        let centre = match (
            generator.contains("procedural-grid"),
            p("cols"),
            p("rows"),
            p("block_x_m"),
            p("block_y_m"),
        ) {
            (true, Some(c), Some(r), Some(bx), Some(by)) => {
                let half =
                    p("lanes_per_direction").unwrap_or(1.0) * p("lane_width_m").unwrap_or(3.5);
                [half + (c - 1.0) * bx / 2.0, half + (r - 1.0) * by / 2.0]
            }
            _ => {
                return Err(err(
                    "prepare_scenario",
                    "the middle of the world is known only for a procedural grid; give the \
                     jammer `position_m: [x, y]` in metres or `follow_node: <node id>`",
                ));
            }
        };
        if let Some(o) = item.as_object_mut() {
            let params = o.entry("params").or_insert_with(|| json!({}));
            if let Some(pm) = params.as_object_mut() {
                pm.insert("position_m".into(), json!([centre[0], centre[1]]));
                pm.entry("power_dbm").or_insert(json!(23.0));
            }
        }
    }
    Ok(())
}

/// One variant's document and its changes. `metrics` is set to `[all]`.
fn build(
    base_doc: &Value,
    lists: &[&[Value]],
    duration_s: Option<f64>,
    max_duration_s: f64,
) -> Result<(Value, Vec<Change>)> {
    let mut doc = base_doc.clone();
    let mut applied = Vec::new();
    for list in lists {
        for c in *list {
            let key = c
                .get("key")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if key.is_empty() {
                return Err(err("simulate", "a change has no key"));
            }
            let why = c
                .get("why")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let value = c.get("value").cloned().unwrap_or(Value::Null);
            let a = apply_repaired(&mut doc, &key, &value)?;
            applied.push(Change {
                key: a.key,
                before: a.before,
                after: a.after,
                why: why_with(why, &a.notes),
            });
        }
    }
    if let Some(d) = duration_s {
        let before = doc
            .pointer("/time/duration_s")
            .cloned()
            .unwrap_or(Value::Null);
        set_pointer(&mut doc, "/time/duration_s", json!(d));
        applied.retain(|c| c.key != "time.duration_s");
        applied.push(Change {
            key: "time.duration_s".into(),
            before,
            after: json!(d),
            why: "duration_s".into(),
        });
    }
    if let Some(d) = doc.pointer("/time/duration_s").and_then(Value::as_f64) {
        if d > max_duration_s {
            return Err(err(
                "simulate",
                format!(
                    "{} s is over the {} s the agent may simulate without asking; ask the person, or keep it shorter",
                    fmt(d),
                    fmt(max_duration_s)
                ),
            ));
        }
    }
    let all = json!(["all"]);
    if doc.get("metrics") != Some(&all) {
        let before = doc.get("metrics").cloned().unwrap_or(Value::Null);
        doc["metrics"] = all.clone();
        applied.push(Change {
            key: "metrics".into(),
            before,
            after: all,
            why: "the analyst reads every layer".into(),
        });
    }
    Ok((doc, applied))
}

// ----------------------------------------------------------------------------- setup

/// One applied change as the panel shows it: `City → Portland, OR`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Chip {
    /// The dotted key (opens the setting in the Settings window).
    pub key: String,
    /// The setting's plain-language label.
    pub label: String,
    /// The new value in words.
    pub value: String,
}

/// The changes as chips. The analyst's own `metrics: all` is left out: the person did not
/// ask for it and it changes nothing they would set.
#[must_use]
pub fn chips(changes: &[Change]) -> Vec<Chip> {
    changes
        .iter()
        .filter(|c| c.key != "metrics")
        .map(|c| Chip {
            key: c.key.clone(),
            label: catalog::label_of(&c.key),
            value: value_words(&c.key, &c.before, &c.after),
        })
        .collect()
}

/// The new value of a change in words.
fn value_words(key: &str, before: &Value, after: &Value) -> String {
    let path = key.trim_start_matches('/').replace('/', ".");
    let pct = |v: f64| format!("{}%", fmt(v * 100.0));
    let item = |v: &Value| -> String {
        let id = v.get("id").and_then(Value::as_str).unwrap_or("");
        let name = v2xw_engine::scenario::options::humanise_id(id);
        match (
            v.get("fraction").and_then(Value::as_f64),
            v.get("count").and_then(Value::as_u64),
        ) {
            (Some(f), _) => format!("{} {name}", pct(f)),
            (None, Some(n)) => format!("{n} {name}"),
            _ => name,
        }
    };
    match (path.as_str(), after) {
        (_, Value::Null) => "removed".into(),
        ("city", Value::String(id)) => catalog::city_short_name(id).unwrap_or_else(|| id.clone()),
        (_, Value::Array(items)) => {
            // An append shows what was added, not the whole list.
            let had = before.as_array().map_or(0, Vec::len);
            let added: Vec<String> = items
                .iter()
                .skip(had.min(items.len()))
                .map(|v| {
                    if v.is_object() {
                        item(v)
                    } else {
                        scalar_words(&path, v)
                    }
                })
                .collect();
            if added.is_empty() {
                format!("{} items", items.len())
            } else {
                added.join(", ")
            }
        }
        ("actors.vehicles.classes", Value::Object(m)) => m
            .iter()
            .map(|(k, v)| {
                let f = v.get("fraction").and_then(Value::as_f64).unwrap_or(0.0);
                format!("{k} {}", pct(f))
            })
            .collect::<Vec<_>>()
            .join(", "),
        (_, Value::Object(_)) if after.get("id").is_some() => item(after),
        (_, Value::Object(m)) => m
            .iter()
            .map(|(k, v)| format!("{k} {}", scalar_words("", v)))
            .collect::<Vec<_>>()
            .join(", "),
        (_, v) => scalar_words(&path, v),
    }
}

fn scalar_words(path: &str, v: &Value) -> String {
    match v {
        Value::String(s) => {
            let row = v2xw_engine::scenario::options::option_row(path, s, "");
            row.get("label")
                .and_then(Value::as_str)
                .filter(|l| !l.is_empty())
                .map_or_else(|| s.clone(), str::to_string)
        }
        Value::Number(n) => {
            let x = n.as_f64().unwrap_or(0.0);
            let unit = catalog::settings()
                .iter()
                .find(|s| s.path == path)
                .map(|s| s.unit.clone())
                .unwrap_or_default();
            match unit.as_str() {
                "" | "ratio" | "count" | "1" | "dimensionless" => fmt(x),
                u => format!("{} {u}", fmt(x)),
            }
        }
        Value::Bool(b) => if *b { "on" } else { "off" }.into(),
        other => other.to_string(),
    }
}

/// `setup`: builds and validates the draft, then stages it in the engine so the Settings
/// window shows it and the person's Run button starts it.
fn setup<P: LlmProvider, T: RpcTransport>(
    agent: &mut Agent<P, T>,
    args: &Value,
    sink: &mut dyn FnMut(AgentEvent),
) -> Result<Value> {
    let base = args
        .get("base")
        .and_then(Value::as_str)
        .filter(|b| !b.is_empty())
        .unwrap_or("current")
        .to_string();
    let reason = args
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let changes = changes_of(args.get("changes"));
    let before = agent.rpc.call("scenario.get", &json!({})).ok();
    let prepared = agent.prepare_with(&base, &reason, &changes, false, sink)?;
    if prepared.get("ok") != Some(&Value::Bool(true)) {
        return Ok(prepared);
    }
    let Some(p) = agent.prepared.clone() else {
        return Err(err("setup", "nothing was prepared"));
    };
    agent
        .rpc
        .call("scenario.set", &json!({"scenario": p.document}))?;
    agent.harness.setups.push(before);
    let id = agent.harness.setups.len();
    let chips = chips(&p.changes);
    sink(AgentEvent::Staged {
        setup: id,
        base: base.clone(),
        chips: chips.clone(),
    });
    Ok(json!({
        "ok": true,
        "staged": true,
        "applied": chips.iter().map(|c| format!("{} → {}", c.label, c.value)).collect::<Vec<_>>(),
        "warnings": prepared.get("warnings").cloned().unwrap_or(Value::Null),
        "next": "It is in the Settings window now. Reply in one line; the person presses Run. \
                 The watcher flags problems while it runs.",
    }))
}

/// Puts back what the Settings window held before setup `id`.
///
/// # Errors
/// When there is no such setup, or the engine refuses the restore.
pub(super) fn undo_setup<P: LlmProvider, T: RpcTransport>(
    agent: &mut Agent<P, T>,
    id: usize,
) -> Result<()> {
    let Some(before) = id
        .checked_sub(1)
        .and_then(|i| agent.harness.setups.get(i))
        .cloned()
    else {
        return Err(err("undo", format!("no setup {id} in this conversation")));
    };
    let Some(b) = before else {
        return Err(err("undo", "the engine's draft before it was not read"));
    };
    // Loading the running scenario's hash withdraws a staged one, but only while that run is
    // still the running one: once the person pressed Run on the setup, the engine knows only the
    // new run's hash and refused the old one (409 "scenario.load"), so Undo did nothing. Then
    // the settings from before are staged instead, for the next Run.
    let running_now = agent
        .rpc
        .call("scenario.get", &json!({}))
        .ok()
        .and_then(|v| {
            v.get("running_hash")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    let running_before = b.get("running_hash").and_then(Value::as_str);
    let staged_before = b.get("staged").is_some_and(|s| !s.is_null());
    match running_before {
        Some(h) if !staged_before && running_now.as_deref() == Some(h) => {
            agent.rpc.call("scenario.load", &json!({"path": h}))?;
        }
        _ => {
            agent.rpc.call(
                "scenario.set",
                &json!({"scenario": b.get("scenario"), "validate": false}),
            )?;
        }
    }
    Ok(())
}

// ----------------------------------------------------------------------------- simulate

fn simulate<P: LlmProvider, T: RpcTransport>(
    agent: &mut Agent<P, T>,
    args: &Value,
    sink: &mut dyn FnMut(AgentEvent),
    comparison: &mut Option<Comparison>,
) -> Result<Value> {
    let s = |k: &str| {
        args.get(k)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let base = if s("base").is_empty() {
        "current".to_string()
    } else {
        s("base")
    };
    let reason = s("reason");
    let common: Vec<Value> = changes_of(args.get("changes"));
    let mut variants: Vec<(String, Vec<Value>)> = args
        .get("variants")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(i, v)| {
            let label = v
                .get("label")
                .and_then(Value::as_str)
                .map_or_else(|| format!("arm {}", i + 1), str::to_string);
            (label, changes_of(v.get("changes")))
        })
        .collect();
    if variants.is_empty() {
        let label = if s("label").is_empty() {
            format!("run {}", agent.runs.len() + 1)
        } else {
            s("label")
        };
        variants.push((label, Vec::new()));
    }
    if variants.len() > 6 {
        return Err(err(
            "simulate",
            "at most 6 variants per call; split the sweep",
        ));
    }
    let compare_to_last = args
        .get("compare_to_last")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if compare_to_last && agent.runs.is_empty() {
        return Err(err(
            "simulate",
            "compare_to_last: nothing has run in this conversation yet",
        ));
    }
    let duration_s = args.get("duration_s").and_then(Value::as_f64);
    let base_doc = agent.base_document(&base)?;
    if !base_doc.is_object() {
        return Err(err(
            "simulate",
            format!("the base {base:?} gave no scenario document"),
        ));
    }

    // Build and validate every arm before running any.
    let mut built: Vec<(String, Value, Vec<Change>, String)> = Vec::new();
    let mut invalid: Vec<Value> = Vec::new();
    for (label, changes) in &variants {
        let (doc, applied) = match build(
            &base_doc,
            &[&common, changes],
            duration_s,
            agent.policy.max_duration_s,
        ) {
            Ok(x) => x,
            Err(e) => {
                invalid.push(json!({"label": label, "errors": [e.to_string()]}));
                continue;
            }
        };
        let report = check_document(doc.clone());
        sink(AgentEvent::Changes {
            base: base.clone(),
            reason: reason.clone(),
            changes: applied.clone(),
            valid: report.ok,
            errors: report.errors.clone(),
        });
        if report.ok {
            built.push((
                label.clone(),
                doc,
                applied,
                report.content_hash.clone().unwrap_or_default(),
            ));
        } else {
            invalid.push(json!({"label": label, "errors": report.errors.iter()
                .map(|e| match e.field.as_deref().and_then(crate::repair::allowed) {
                    Some(a) => json!({"field": e.field, "message": e.message, "allowed": a}),
                    None => json!({"field": e.field, "message": e.message}),
                }).collect::<Vec<_>>()}));
        }
    }
    if !invalid.is_empty() {
        return Ok(json!({"ok": false, "invalid": invalid,
                         "note": "nothing ran; fix the named fields and call simulate again"}));
    }

    let mut arms: Vec<usize> = Vec::new();
    if compare_to_last {
        arms.push(agent.runs.len() - 1);
    }
    let first_new = agent.runs.len();
    for (label, doc, applied, hash) in built {
        agent.prepared = Some(Prepared {
            base: base.clone(),
            reason: reason.clone(),
            changes: applied,
            scenario_hash: hash,
            document: doc,
        });
        if let Err(e) = agent.run_prepared(&label, sink) {
            let done: Vec<String> = agent.runs[first_new..]
                .iter()
                .map(|r| r.label.clone())
                .collect();
            return Err(err(
                "simulate",
                format!("arm {label:?} failed: {e}; finished before it: {done:?}"),
            ));
        }
        arms.push(agent.runs.len() - 1);
    }

    if arms.len() >= 2 {
        let x = experiment_of(agent, &arms);
        if let (Some(a), Some(b)) = (arms.first(), arms.last()) {
            *comparison = Some(analyst::compare(
                &agent.runs[*a].analysis,
                &agent.runs[*b].analysis,
            ));
        }
        let out = json!({
            "ok": true,
            "runs": arms.iter().map(|i| brief(&agent.runs[*i], *i, false)).collect::<Vec<_>>(),
            "experiment": x.compact(),
        });
        agent.harness.experiments.push((agent.harness.turn, x));
        Ok(out)
    } else {
        let mut out = run_summary(agent, arms[0]);
        out["ok"] = json!(true);
        Ok(out)
    }
}

/// One run for the model: its findings with their numbers, the headline and extra metrics,
/// and every metric outside its healthy band.
pub(super) fn run_summary<P: LlmProvider, T: RpcTransport>(agent: &Agent<P, T>, i: usize) -> Value {
    let Some(run) = agent.runs.get(i) else {
        return Value::Null;
    };
    let mut out = brief(run, i, true);
    let empty = RunMeta::default();
    let m = agent.harness.meta.get(&i).unwrap_or(&empty);
    if !m.extra.is_empty() {
        out["extra"] = json!(
            m.extra
                .iter()
                .map(|(k, v)| (k.clone(), json!(fmt(*v))))
                .collect::<Map<_, _>>()
        );
    }
    let health: BTreeMap<String, String> =
        health_of(&run.analysis.headline, &m.extra, &agent.harness.bands)
            .into_iter()
            .filter(|(_, l)| l != "healthy")
            .collect();
    if !health.is_empty() {
        out["out_of_band"] = json!(health);
    }
    if !m.catalogue.is_empty() {
        out["other_metrics"] = json!(m.catalogue);
    }
    out
}

/// The experiment over these runs of the conversation.
fn experiment_of<P: LlmProvider, T: RpcTransport>(
    agent: &Agent<P, T>,
    idx: &[usize],
) -> Experiment {
    let empty = RunMeta::default();
    let metas: Vec<&RunMeta> = idx
        .iter()
        .map(|i| agent.harness.meta.get(i).unwrap_or(&empty))
        .collect();
    let arms: Vec<Arm<'_>> = idx
        .iter()
        .zip(&metas)
        .map(|(i, m)| Arm {
            label: &agent.runs[*i].label,
            analysis: &agent.runs[*i].analysis,
            extra: &m.extra,
            units: &m.units,
            seed: &m.seed,
        })
        .collect();
    experiment::compare(&arms, &agent.harness.bands, &agent.thresholds)
}

/// `compare_runs` over any runs of the conversation: `runs: [i, j, …]`, or `a` and `b`, or
/// the last two.
pub(super) fn compare_runs<P: LlmProvider, T: RpcTransport>(
    agent: &mut Agent<P, T>,
    args: &Value,
    comparison: &mut Option<Comparison>,
) -> Result<Value> {
    let n = agent.runs.len();
    if n < 2 {
        return Err(err(
            "compare_runs",
            "fewer than two runs in this conversation",
        ));
    }
    let idx: Vec<usize> = match args.get("runs").and_then(Value::as_array) {
        Some(list) => list
            .iter()
            .filter_map(Value::as_u64)
            .map(|v| v as usize)
            .collect(),
        None => {
            let pick = |k: &str, d: usize| {
                args.get(k)
                    .and_then(Value::as_u64)
                    .map_or(d, |v| v as usize)
            };
            vec![pick("a", n - 2), pick("b", n - 1)]
        }
    };
    if idx.len() < 2 || idx.iter().any(|i| *i >= n) {
        return Err(err(
            "compare_runs",
            format!("give two or more run indexes below {n} (list_runs numbers them)"),
        ));
    }
    let x = experiment_of(agent, &idx);
    *comparison = Some(analyst::compare(
        &agent.runs[idx[0]].analysis,
        &agent.runs[idx[idx.len() - 1]].analysis,
    ));
    let out = x.compact();
    agent.harness.experiments.push((agent.harness.turn, x));
    Ok(out)
}

/// A run, compact: findings at notice and above with their numbers (`full`: the headline
/// numbers too), what was not measured.
fn brief(r: &RunRecord, index: usize, full: bool) -> Value {
    let a: &Analysis = &r.analysis;
    let findings: Vec<Value> = a
        .findings
        .iter()
        .filter(|f| f.severity >= Severity::Notice)
        .map(|f| {
            let mut o = json!({"id": f.id, "severity": f.severity, "title": f.title});
            if full || f.severity >= Severity::Warning {
                o["evidence"] = json!(
                    f.evidence
                        .iter()
                        .map(|e| format!("{}: {} {}", e.label, fmt(e.value), e.unit)
                            .trim()
                            .to_string())
                        .collect::<Vec<_>>()
                );
                let w = &f.location;
                let mut wh = Vec::new();
                if !w.nodes.is_empty() {
                    wh.push(format!(
                        "nodes {}",
                        w.nodes
                            .iter()
                            .take(3)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(",")
                    ));
                }
                if !w.regions.is_empty() {
                    wh.push(format!(
                        "regions {}",
                        w.regions
                            .iter()
                            .take(3)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(",")
                    ));
                }
                if !w.entities.is_empty() {
                    wh.push(format!("entities {}", w.entities.join(",")));
                }
                if !w.distance_bins.is_empty() {
                    wh.push(format!("distance {} m", w.distance_bins.join(",")));
                }
                if let Some([x, y]) = w.window_s {
                    wh.push(format!("t {}-{} s", fmt(x), fmt(y)));
                }
                if !wh.is_empty() {
                    o["where"] = json!(wh.join("; "));
                }
                if let Some(rf) = &f.reference {
                    o["reference"] = json!(rf);
                }
            }
            o
        })
        .collect();
    let mut out = json!({
        "index": index,
        "label": r.label,
        "t_s": fmt(r.t_s),
        "findings": findings,
        "not_measured": a.not_measured,
    });
    if full {
        out["headline"] = json!(
            a.headline
                .iter()
                .map(|(k, v)| (k.clone(), json!(fmt(*v))))
                .collect::<Map<_, _>>()
        );
    }
    out
}

/// The headline and extra metrics outside their healthy band, with the level.
fn health_of(
    headline: &BTreeMap<String, f64>,
    extra: &BTreeMap<String, f64>,
    bands: &BTreeMap<String, HealthBand>,
) -> BTreeMap<String, String> {
    let th = analyst::Thresholds::default();
    headline
        .iter()
        .chain(extra.iter())
        .filter_map(|(k, v)| {
            let band = bands
                .get(k)
                .cloned()
                .or_else(|| experiment::fallback_band(k, &th))?;
            band.level(*v).map(|l| (k.clone(), l.to_string()))
        })
        .collect()
}

/// Reads the extra metrics, units, health bands and seed of the run the engine just
/// finished, for run `index`.
pub(super) fn record_meta<P: LlmProvider, T: RpcTransport>(agent: &mut Agent<P, T>, index: usize) {
    let seed = agent.runs[index]
        .prepared
        .as_ref()
        .and_then(|p| p.document.get("seed"))
        .map(|s| s.as_str().map_or_else(|| s.to_string(), str::to_string))
        .unwrap_or_default();
    let mut meta = RunMeta {
        seed,
        ..RunMeta::default()
    };
    let t_ns = (agent.runs[index].t_s * 1e9).round() as u64;
    if let Ok(cat) = agent.rpc.call("metrics.query", &json!({})) {
        let bands = experiment::bands_from_catalogue(&cat);
        if !bands.is_empty() {
            agent.harness.bands = bands;
        }
        let rows: Vec<(String, String)> = cat
            .get("catalogue")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|r| {
                Some((
                    r.get("name")?.as_str()?.to_string(),
                    r.get("unit")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                ))
            })
            .collect();
        for r in cat
            .get("catalogue")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let base = r
                .get("base")
                .or_else(|| r.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if base.is_empty() || agent.runs[index].analysis.headline.contains_key(base) {
                continue;
            }
            let dims: Vec<&str> = r
                .get("dims")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|d| *d != "t")
                .collect();
            let entry = if dims.is_empty() {
                base.to_string()
            } else {
                format!("{base}[{}]", dims.join("|"))
            };
            if !meta.catalogue.contains(&entry) && meta.catalogue.len() < 120 {
                meta.catalogue.push(entry);
            }
        }
        for (name, unit) in &rows {
            meta.units.insert(name.clone(), unit.clone());
        }
        let headline = &agent.runs[index].analysis.headline;
        let wanted: Vec<&String> = rows
            .iter()
            .map(|(n, _)| n)
            .filter(|n| !headline.contains_key(*n))
            .filter(|n| {
                EXTRA_METRICS.iter().any(|p| {
                    if p.ends_with('_') {
                        n.starts_with(p)
                    } else {
                        n == p
                    }
                })
            })
            .take(EXTRA_LIMIT)
            .collect();
        for name in wanted {
            let v = agent
                .rpc
                .call(
                    "metrics.query",
                    &json!({"metrics": [name], "t_from_ns": 0, "t_to_ns": t_ns,
                                               "bin_ns": t_ns.max(1) + 1}),
                )
                .ok()
                .and_then(|v| {
                    v.get("rows")?
                        .as_array()?
                        .iter()
                        .rev()
                        .find_map(|r| r.get(1).and_then(Value::as_f64))
                });
            if let Some(v) = v {
                meta.extra.insert(name.clone(), v);
            }
        }
        // Lost frames by cause, as counts: the grouped answer's sample count is the number
        // of losses with that cause ("how many frames did the jammer kill").
        if rows.iter().any(|(n, _)| n.starts_with("pdr_by_cause")) {
            if let Ok(v) = agent.rpc.call(
                "metrics.query",
                &json!({"metrics": ["pdr_by_cause"], "group_by": ["cause"], "t_from_ns": 0, "t_to_ns": t_ns}),
            ) {
                for r in v.get("rows").and_then(Value::as_array).into_iter().flatten() {
                    if let (Some(cause), Some(n)) = (r.get(0).and_then(Value::as_str), r.get(4).and_then(Value::as_u64)) {
                        let key = format!("lost_frames[{cause}]");
                        meta.units.insert(key.clone(), "frames".into());
                        meta.extra.insert(key, n as f64);
                    }
                }
            }
        }
    }
    // The findings the extra metrics support (detection, overhead, pedestrians, apps),
    // added to the run's analysis so reports, briefs and experiments explain with them.
    let attackers = agent.runs[index]
        .prepared
        .as_ref()
        .and_then(|p| p.document.pointer("/threats/attackers"))
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty());
    let more = crate::insight::findings(
        &meta.extra,
        &agent.runs[index].analysis.headline,
        &meta.units,
        attackers,
    );
    if !more.is_empty() {
        // The analyst's own finding wins where both have one (its jamming finding carries
        // the whole loss breakdown).
        let findings = &mut agent.runs[index].analysis.findings;
        let fresh: Vec<_> = more
            .into_iter()
            .filter(|m| !findings.iter().any(|f| f.id == m.id))
            .collect();
        findings.extend(fresh);
        findings.sort_by(|a, b| b.severity.cmp(&a.severity));
    }
    agent.harness.meta.insert(index, meta);
}

fn list_runs<P: LlmProvider, T: RpcTransport>(agent: &Agent<P, T>) -> Value {
    json!(
        agent
            .runs
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let changed: Vec<String> = r
                    .prepared
                    .as_ref()
                    .map(|p| {
                        p.changes
                            .iter()
                            .filter(|c| c.key != "metrics")
                            .map(|c| format!("{}={}", c.key, compact_value(&c.after)))
                            .collect()
                    })
                    .unwrap_or_default();
                json!({"index": i, "label": r.label, "run_id": r.analysis.run_id,
                   "base": r.prepared.as_ref().map(|p| p.base.clone()), "changed": changed,
                   "t_s": fmt(r.t_s)})
            })
            .collect::<Vec<_>>()
    )
}

fn compact_value(v: &Value) -> String {
    let s = match v {
        Value::Number(n) => n.as_f64().map_or_else(|| n.to_string(), fmt),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    catalog::shorten(&s, 80)
}

// ----------------------------------------------------------------------------- run control

fn status_brief(st: &Value) -> Value {
    let ns = |k: &str| st.get(k).and_then(Value::as_u64).unwrap_or(0) as f64 / 1e9;
    json!({
        "run_id": st.get("run_id"),
        "state": st.get("state"),
        "t_s": fmt(ns("t_ns")),
        "t_end_s": fmt(ns("t_end_ns")),
        "actors": st.get("actors"),
        "nodes": st.get("nodes"),
        "speed": st.get("speed"),
    })
}

fn run_control<P: LlmProvider, T: RpcTransport>(
    agent: &mut Agent<P, T>,
    args: &Value,
    sink: &mut dyn FnMut(AgentEvent),
) -> Result<Value> {
    let raw = args
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("status")
        .trim()
        .to_ascii_lowercase();
    // The words a model also uses for an action, and a number written with its unit
    // ("4x", "30 s").
    let mut args = args.clone();
    let action = match raw.as_str() {
        "play" | "run" | "go" | "begin" => "start",
        "unpause" | "continue" => "resume",
        "halt" | "end" | "cancel" => "stop",
        "restart" | "rewind" | "reset" => {
            args["t_s"] = json!(0.0);
            "seek"
        }
        "set_speed" | "speed_up" | "slow_down" => "speed",
        "jump" | "goto" | "go_to" => "seek",
        "run_until" | "advance_to" => "run_to",
        "state" | "get_status" => "status",
        other => other,
    }
    .to_string();
    let action = action.as_str();
    let num = |k: &str| {
        args.get(k).and_then(|v| {
            v.as_f64().or_else(|| {
                v.as_str().and_then(|s| {
                    let t: String = s
                        .trim()
                        .trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '×' || c == ' ')
                        .to_string();
                    t.parse().ok()
                })
            })
        })
    };
    match action {
        "status" => {}
        "pause" | "resume" => {
            agent.rpc.call(&format!("run.{action}"), &json!({}))?;
        }
        "stop" => {
            agent.rpc.call("run.stop", &json!({}))?;
        }
        "speed" => {
            let v = num("speed").ok_or_else(|| {
                err(
                    "run_control",
                    "speed needs `speed` (0 is as fast as possible)",
                )
            })?;
            agent.rpc.call("run.speed", &json!({"speed": v}))?;
        }
        "start" => {
            // "Start the run" means it plays, at the speed the person has set (the
            // server keeps the current speed when none is sent) unless one is given.
            let paused = args.get("paused").and_then(Value::as_bool).unwrap_or(false);
            let _ = agent.rpc.call("run.pause", &json!({}));
            let doc = match &agent.prepared {
                Some(p) => p.document.clone(),
                None => agent.base_document("current")?,
            };
            let mut params = json!({"scenario": &doc, "paused": paused});
            if let Some(s) = num("speed") {
                params["speed"] = json!(s);
            }
            let started = agent.rpc.call("run.start", &params)?;
            agent.last_document = Some(doc);
            sink(AgentEvent::RunStarted {
                label: "manual".into(),
                run_id: started
                    .get("run_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                t_end_s: started.get("t_end_ns").and_then(Value::as_u64).unwrap_or(0) as f64 / 1e9,
            });
        }
        "step" => {
            let secs = num("seconds").unwrap_or(1.0).max(0.1);
            let _ = agent.rpc.call("run.pause", &json!({}));
            agent.rpc.call(
                "run.step",
                &json!({"unit": "second", "count": secs.round().max(1.0)}),
            )?;
        }
        "seek" | "run_to" => {
            let st = agent.rpc.call("run.status", &json!({}))?;
            let t_now = st.get("t_ns").and_then(Value::as_u64).unwrap_or(0);
            let t_end = st.get("t_end_ns").and_then(Value::as_u64).unwrap_or(0);
            let target =
                num("t_s").map_or(t_end, |t| ((t * 1e9).round().max(0.0) as u64).min(t_end));
            if action == "seek" && target < t_now {
                // Over HTTP the server cannot rewind (`run.seek` streams frames over the
                // socket); a run is a function of its scenario and seed, so starting it
                // again and stepping to the target reaches the same state.
                let doc = agent.base_document("current")?;
                let _ = agent.rpc.call("run.pause", &json!({}));
                agent.rpc.call(
                    "run.start",
                    &json!({"scenario": &doc, "speed": agent.policy.speed, "paused": true}),
                )?;
            }
            let st = agent.rpc.call("run.status", &json!({}))?;
            let now = st.get("t_ns").and_then(Value::as_u64).unwrap_or(0);
            if target > now {
                if action == "seek" {
                    let _ = agent.rpc.call("run.pause", &json!({}));
                    let secs = ((target - now) as f64 / 1e9).ceil().max(1.0);
                    agent
                        .rpc
                        .call("run.step", &json!({"unit": "second", "count": secs}))?;
                } else {
                    let _ = agent
                        .rpc
                        .call("run.speed", &json!({"speed": agent.policy.speed}));
                    let _ = agent.rpc.call("run.resume", &json!({}));
                    wait_until(agent, target)?;
                    if target < t_end {
                        let _ = agent.rpc.call("run.pause", &json!({}));
                    }
                }
            }
        }
        other => return Err(err("run_control", format!("unknown action {other:?}"))),
    }
    let st = agent.rpc.call("run.status", &json!({}))?;
    Ok(status_brief(&st))
}

/// Polls until the run reaches `target_ns`, ends, or is stopped by the person.
fn wait_until<P: LlmProvider, T: RpcTransport>(
    agent: &mut Agent<P, T>,
    target_ns: u64,
) -> Result<()> {
    for poll in 0..agent.policy.max_polls {
        if agent.cancelled() {
            let _ = agent.rpc.call("run.pause", &json!({}));
            return Err(err(
                "run_control",
                "stopped by the person; the run was paused",
            ));
        }
        let st = agent.rpc.call("run.status", &json!({}))?;
        let t = st.get("t_ns").and_then(Value::as_u64).unwrap_or(0);
        let state = st.get("state").and_then(Value::as_str).unwrap_or("");
        if t >= target_ns || matches!(state, "finished" | "error" | "stopped") {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(super::poll_delay_ms(
            agent.policy.poll_ms,
            poll,
        )));
    }
    Err(CopilotError::Budget {
        what: "poll",
        budget: agent.policy.max_polls,
    })
}

// ----------------------------------------------------------------------------- reading

/// The metrics a `live_metrics` call with no list reads.
const LIVE_DEFAULT: [&str; 12] = [
    "pdr",
    "pdr[100m]",
    "cbr",
    "cbr.p95",
    "e2e_latency.p95",
    "aoi.p95",
    "nar",
    "mac_queue_depth",
    "unverified_ratio",
    "mean_speed",
    "ttc_conflicts",
    "linkability",
];

fn live_metrics<P: LlmProvider, T: RpcTransport>(
    agent: &mut Agent<P, T>,
    args: &Value,
) -> Result<Value> {
    let st = agent.rpc.call("run.status", &json!({}))?;
    let t_ns = st.get("t_ns").and_then(Value::as_u64).unwrap_or(0);
    let cat = agent.rpc.call("metrics.query", &json!({}))?;
    let bands = experiment::bands_from_catalogue(&cat);
    if !bands.is_empty() {
        agent.harness.bands = bands;
    }
    let units: BTreeMap<String, String> = cat
        .get("catalogue")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| {
            Some((
                r.get("name")?.as_str()?.to_string(),
                r.get("unit")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            ))
        })
        .collect();
    let mut asked: Vec<String> = args
        .get("metrics")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    // Nothing, or "all", is the headline set plus every banded metric the run has.
    if asked.is_empty() || asked.iter().any(|m| m == "all") {
        asked = LIVE_DEFAULT.iter().map(|s| (*s).to_string()).collect();
        for k in agent.harness.bands.keys() {
            if !asked.contains(k) {
                asked.push(k.clone());
            }
        }
    }
    let th = analyst::Thresholds::default();
    let mut rows = Vec::new();
    let mut missing = Vec::new();
    for name in asked.iter().take(30) {
        if !units.contains_key(name) {
            missing.push(name.clone());
            continue;
        }
        let v = agent
            .rpc
            .call("metrics.query", &json!({"metrics": [name], "t_from_ns": 0, "t_to_ns": t_ns, "bin_ns": t_ns.max(1) + 1}))
            .ok()
            .and_then(|v| v.get("rows")?.as_array()?.iter().rev().find_map(|r| r.get(1).and_then(Value::as_f64)));
        let Some(v) = v else {
            missing.push(name.clone());
            continue;
        };
        let band = agent
            .harness
            .bands
            .get(name)
            .cloned()
            .or_else(|| experiment::fallback_band(name, &th));
        let mut row = json!({"metric": name, "value": fmt(v)});
        if let Some(u) = units.get(name).filter(|u| !u.is_empty()) {
            row["unit"] = json!(u);
        }
        if let Some(b) = band {
            row["health"] = json!(b.level(v));
        }
        rows.push(row);
    }
    Ok(json!({"run": status_brief(&st), "metrics": rows, "not_available": missing}))
}

fn query_metric<P: LlmProvider, T: RpcTransport>(
    agent: &mut Agent<P, T>,
    args: &Value,
) -> Result<Value> {
    let metric = args
        .get("metric")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let st = agent.rpc.call("run.status", &json!({}))?;
    let t_now = st.get("t_ns").and_then(Value::as_u64).unwrap_or(0);
    let ns = |k: &str, d: u64| {
        args.get(k)
            .and_then(Value::as_f64)
            .map_or(d, |s| (s * 1e9).round().max(0.0) as u64)
    };
    let (from, to) = (ns("from_s", 0), ns("to_s", t_now).min(t_now.max(1)));
    let mut params = json!({"metrics": [metric], "t_from_ns": from, "t_to_ns": to});
    let grouped = args
        .get("group_by")
        .and_then(Value::as_str)
        .filter(|g| !g.is_empty());
    if let Some(g) = grouped {
        params["group_by"] = json!([g]);
    } else {
        let bins = args
            .get("bins")
            .and_then(Value::as_u64)
            .unwrap_or(10)
            .clamp(1, 40);
        params["bin_ns"] = json!(((to - from) / bins).max(1_000_000_000));
    }
    if let Some(w) = args.get("where").filter(|w| w.is_object()) {
        params["where"] = w.clone();
    }
    let v = agent.rpc.call("metrics.query", &params)?;
    let rows: Vec<Value> = v
        .get("rows")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(40)
        .map(|r| {
            let key = match r.get(0) {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Number(n)) if grouped.is_none() => fmt(n.as_f64().unwrap_or(0.0) / 1e9),
                Some(other) => other.to_string(),
                None => String::new(),
            };
            let val = r
                .get(1)
                .and_then(Value::as_f64)
                .map_or_else(|| "insufficient".to_string(), fmt);
            let mut o = json!([key, val]);
            if grouped.is_some() {
                if let Some(n) = r.get(4) {
                    o.as_array_mut().map(|a| a.push(n.clone()));
                }
            }
            o
        })
        .collect();
    let total = v.get("rows").and_then(Value::as_array).map_or(0, Vec::len);
    Ok(json!({
        "metric": metric,
        "columns": if grouped.is_some() { json!([grouped, "value", "n"]) } else { json!(["t_s", "value"]) },
        "rows": rows,
        "rows_omitted": total.saturating_sub(40),
    }))
}

/// Cuts arrays to 8 items and strings to 200 characters, recursively, noting what was cut.
fn trim(v: &Value, depth: usize) -> Value {
    match v {
        Value::Array(a) => {
            let mut out: Vec<Value> = a.iter().take(8).map(|x| trim(x, depth + 1)).collect();
            if a.len() > 8 {
                out.push(json!(format!("… {} more", a.len() - 8)));
            }
            Value::Array(out)
        }
        Value::Object(o) if depth < 6 => Value::Object(
            o.iter()
                .map(|(k, x)| (k.clone(), trim(x, depth + 1)))
                .collect(),
        ),
        Value::Object(_) => json!("{…}"),
        Value::String(s) => json!(catalog::shorten(s, 200)),
        Value::Number(n) => n.as_f64().map_or_else(
            || v.clone(),
            |f| {
                if f.fract() == 0.0 {
                    v.clone()
                } else {
                    json!(fmt(f))
                }
            },
        ),
        other => other.clone(),
    }
}

fn inspect<P: LlmProvider, T: RpcTransport>(
    agent: &mut Agent<P, T>,
    args: &Value,
) -> Result<Value> {
    let target = args.get("target").and_then(Value::as_str).unwrap_or("");
    let t_ns = args
        .get("t_s")
        .and_then(Value::as_f64)
        .map(|t| (t * 1e9).round() as u64);
    let mut p = Map::new();
    if let Some(t) = t_ns {
        p.insert("t_ns".into(), json!(t));
    }
    let (method, extra) = match target {
        "node" => (
            "inspect.node",
            json!({"node": args.get("node").cloned().unwrap_or(json!(1)), "limit": 8}),
        ),
        "link" => (
            "inspect.link",
            json!({"tx": args.get("tx"), "rx": args.get("rx")}),
        ),
        "entity" => (
            "inspect.entity",
            json!({"entity": args.get("entity").and_then(Value::as_str).unwrap_or("backend"), "limit": 8}),
        ),
        "explain" => (
            "explain",
            json!({"subject": {"kind": "metric",
            "id": args.get("metric").and_then(Value::as_str).unwrap_or("pdr")}}),
        ),
        other => return Err(err("inspect", format!("unknown target {other:?}"))),
    };
    if let Some(o) = extra.as_object() {
        for (k, v) in o {
            p.insert(k.clone(), v.clone());
        }
    }
    let v = agent.rpc.call(method, &Value::Object(p))?;
    Ok(trim(&v, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_written_in_other_shapes_are_read_as_the_one_shape() {
        let want = vec![json!({"key": "radio.rat", "value": "lte-v2x-pc5", "why": ""})];
        assert_eq!(changes_of(Some(&json!({"radio.rat": "lte-v2x-pc5"}))), want);
        assert_eq!(
            changes_of(Some(&json!([{"path": "radio.rat", "to": "lte-v2x-pc5"}]))),
            want
        );
        assert_eq!(
            changes_of(Some(&json!(
                "[{\"key\":\"radio.rat\",\"value\":\"lte-v2x-pc5\",\"why\":\"\"}]"
            ))),
            want
        );
        assert!(changes_of(None).is_empty());
    }

    #[test]
    fn a_setups_changes_read_as_short_chips() {
        let ch = |key: &str, before: Value, after: Value| Change {
            key: key.into(),
            before,
            after,
            why: String::new(),
        };
        let c = chips(&[
            ch("city", Value::Null, json!("portland")),
            ch("radio.rat", json!("lte-v2x-pc5"), json!("dsrc-80211p")),
            ch(
                "threats.attackers",
                json!([]),
                json!([{"id": "threat/attacker/legacy/ConstPos", "fraction": 0.05}]),
            ),
            ch(
                "actors.vehicles.classes",
                Value::Null,
                json!({"passenger": {"fraction": 0.9}, "truck": {"fraction": 0.1}}),
            ),
            ch("metrics", Value::Null, json!(["all"])),
        ]);
        let line: Vec<String> = c
            .iter()
            .map(|c| format!("{} → {}", c.label, c.value))
            .collect();
        assert_eq!(
            c.len(),
            4,
            "the analyst's own metrics change is not a chip: {line:?}"
        );
        assert_eq!(line[0], "City → Portland, OR");
        assert!(
            line[1].starts_with("Radio") && !line[1].contains("dsrc-80211p")
                || line[1].contains("DSRC"),
            "{line:?}"
        );
        assert_eq!(line[2], "Attackers → 5% Constant position");
        assert_eq!(line[3], "Vehicle mix → passenger 90%, truck 10%");
    }

    #[test]
    fn changes_set_scale_append_and_remove() {
        let mut d = json!({"actors": {"vehicles": {"demand": {"rate_veh_per_h": 1500.0}}},
                           "threats": {"attackers": []}, "weather": {"intensity": 0.2}});
        let (b, a) = apply_change(
            &mut d,
            "actors.vehicles.demand.rate_veh_per_h",
            &json!({"multiply": 2}),
        )
        .expect("scales");
        assert_eq!((b, a), (json!(1500.0), json!(3000.0)));
        apply_change(
            &mut d,
            "threats.attackers",
            &json!({"append": {"id": "threat/attacker/legacy/ConstPos", "fraction": 0.05}}),
        )
        .expect("appends");
        assert_eq!(d["threats"]["attackers"][0]["fraction"], json!(0.05));
        apply_change(&mut d, "threats.attackers.0.fraction", &json!(0.1)).expect("indexes");
        assert_eq!(d["threats"]["attackers"][0]["fraction"], json!(0.1));
        apply_change(&mut d, "weather.intensity", &json!({"remove": true})).expect("removes");
        assert!(d["weather"].get("intensity").is_none());
        assert!(
            apply_change(&mut d, "weather", &json!({"multiply": 2})).is_err(),
            "an object cannot be scaled"
        );
    }

    #[test]
    fn a_jammer_at_the_centre_of_a_grid_is_placed_at_its_middle_junction() {
        let mut d = json!({"world": {"source": {"generator": "world/source/procedural-grid", "params": {
            "cols": 13, "rows": 34, "block_x_m": 274.0, "block_y_m": 80.0,
            "lanes_per_direction": 2, "lane_width_m": 3.5}}}, "threats": {}});
        apply_change(
            &mut d,
            "threats.jammers",
            &json!({"append": {"id": "attacker/jammer/constant",
            "params": {"position_m": "centre"}}}),
        )
        .expect("placed");
        assert_eq!(
            d["threats"]["jammers"][0]["params"]["position_m"],
            json!([7.0 + 6.0 * 274.0, 7.0 + 16.5 * 80.0])
        );
        assert_eq!(
            d["threats"]["jammers"][0]["params"]["power_dbm"],
            json!(23.0)
        );
        let mut city = json!({"world": {"source": {"generator": "world/source/osm"}}});
        assert!(
            apply_change(
                &mut city,
                "threats.jammers",
                &json!({"append": {"id": "attacker/jammer/constant"}})
            )
            .is_err()
        );
    }

    #[test]
    fn a_variant_is_built_from_common_and_own_changes_with_every_metric_and_a_capped_duration() {
        let base = json!({"time": {"duration_s": 60.0}, "radio": {"rat": "dsrc-80211p"}, "metrics": ["pdr"]});
        let common = vec![json!({"key": "time.duration_s", "value": 30})];
        let own = vec![json!({"key": "radio.rat", "value": "lte-v2x-pc5", "why": "w"})];
        let (doc, changes) = build(&base, &[&common, &own], None, 300.0).expect("builds");
        assert_eq!(doc["radio"]["rat"], "lte-v2x-pc5");
        assert_eq!(doc["metrics"], json!(["all"]));
        assert!(changes.iter().any(|c| c.key == "time.duration_s"));
        assert!(
            build(&base, &[], Some(900.0), 300.0).is_err(),
            "over the cap"
        );
    }

    #[test]
    fn every_method_the_control_tools_call_is_published() {
        for m in CONTROL_METHODS {
            assert!(v2xw_server::rpc::METHODS.contains(&m), "{m}");
        }
    }

    #[test]
    fn trimming_keeps_a_note_of_what_it_cut() {
        let v = trim(
            &json!({"list": (0..20).collect::<Vec<_>>(), "x": 0.123456}),
            0,
        );
        assert_eq!(v["list"].as_array().map(Vec::len), Some(9));
        assert_eq!(v["list"][8], "… 12 more");
        assert_eq!(v["x"], "0.123");
    }
}
