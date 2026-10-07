//! The agent: one prompt in; a scenario built, validated, run, watched and analysed; a
//! report out.
//!
//! [`Agent::ask`] runs the loop. The model (Claude by default, [`crate::claude`]) is given
//! a small set of high-level tools — list the shipped scenarios, prepare a scenario from
//! one with the changes the request implies, run it, analyse a run, compare two, look up a
//! finding or a model card — and the agent carries each one out through the server's own
//! JSON-RPC methods ([`AGENT_METHODS`], every one of them a method the Studio uses too).
//! The model plans and writes; it does not compute a single number. Every figure in a
//! report comes from the deterministic analyst ([`crate::analyst`]), and the overview the
//! model writes is checked against the tool results: a number in it that no tool produced
//! is sent back once for a rewrite, and if it survives it is listed in the report as
//! unverified ([`ungrounded_numbers`]).
//!
//! Follow-ups ("now double the density", "compare with LTE-V2X") keep the conversation
//! and the runs: the model prepares from the last run's scenario (`base: "last"`), runs it,
//! and calls `compare_runs`.
//!
//! The person can stop it at any point: [`Agent::cancel_handle`] is checked before every
//! model call, every tool and every poll of a running simulation, and a stopped run is
//! paused, not left running.
//!
//! With no model at all, [`Agent::run_without_model`] and [`Agent::analyse_current`] still
//! run a scenario and analyse it; the report then has a deterministic summary in place of
//! the overview.
//!
//! Progress goes out as [`AgentEvent`]s through a callback, which the Studio's panel and
//! the CLI both render.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::analyst::{self, Analysis, Comparison, Finding, Layer, Severity, Thresholds, fmt};
use crate::error::{CopilotError, Result};
use crate::gather::gather;
use crate::grounding::Grounding;
use crate::provider::{ChatMessage, ChatRequest, LlmProvider};
use crate::scenario::{DraftError, check_document};
use crate::session::{ToolOutcome, local_call};
use crate::transport::RpcTransport;

/// Every server method the agent calls. Each is one of the server's published methods
/// (asserted in the tests), so the agent can do nothing a person in the Studio cannot.
pub const AGENT_METHODS: [&str; 8] = [
    "scenario.list",
    "scenario.load",
    "scenario.get",
    "run.start",
    "run.status",
    "run.pause",
    "metrics.query",
    "inspect.entity",
];

/// One scenario setting the agent changed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Change {
    /// The dotted key, as the settings window names it (`radio.rat`).
    pub key: String,
    /// The value before.
    pub before: Value,
    /// The value after.
    pub after: Value,
    /// Why, in the request's terms.
    pub why: String,
}

/// A scenario prepared and validated, waiting to run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Prepared {
    /// What it was built from: a preset id, `current` or `last`.
    pub base: String,
    /// Why that base.
    pub reason: String,
    /// The changes.
    pub changes: Vec<Change>,
    /// The scenario's content hash, from the engine's loader.
    pub scenario_hash: String,
    /// The document.
    #[serde(skip)]
    pub document: Value,
}

/// One run the agent made or analysed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunRecord {
    /// A short label (`baseline`, `lte-v2x`).
    pub label: String,
    /// What it was built from, and how.
    pub prepared: Option<Prepared>,
    /// The analyst's answer.
    pub analysis: Analysis,
    /// How far it got, simulated seconds.
    pub t_s: f64,
}

/// What the agent reports when it finishes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentReport {
    /// The request.
    pub prompt: String,
    /// The model that wrote the overview, or `none`.
    pub provider: String,
    /// The overview: the model's, or a deterministic summary without one.
    pub overview: String,
    /// Numbers in the overview that no tool result contains.
    pub ungrounded: Vec<String>,
    /// The runs made or analysed in this turn.
    pub runs: Vec<RunRecord>,
    /// The comparison, when the turn made one.
    pub comparison: Option<Comparison>,
    /// Whether the person stopped it.
    pub stopped: bool,
    /// The whole report as Markdown (what `v2xw agent` prints).
    pub markdown: String,
}

/// What the agent is doing, as it does it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AgentEvent {
    /// A line of status.
    Status {
        /// The line.
        text: String,
    },
    /// What the model said it will do (its text beside a tool call).
    Plan {
        /// The text.
        text: String,
    },
    /// A scenario prepared, with every change and whether it validated.
    Changes {
        /// What it was built from.
        base: String,
        /// Why that base.
        reason: String,
        /// The changes.
        changes: Vec<Change>,
        /// Whether the engine's loader accepted it.
        valid: bool,
        /// What it refused.
        errors: Vec<DraftError>,
    },
    /// A run started.
    RunStarted {
        /// Its label.
        label: String,
        /// The run id.
        run_id: String,
        /// Where it ends, simulated seconds.
        t_end_s: f64,
    },
    /// A run's progress.
    Progress {
        /// Its label.
        label: String,
        /// Simulated seconds reached.
        t_s: f64,
        /// Where it ends.
        t_end_s: f64,
        /// Vehicles and other actors on the road now.
        actors: u64,
    },
    /// A run's analysis is ready.
    Analysed {
        /// Its label.
        label: String,
        /// The compact analysis.
        analysis: Value,
    },
    /// A tool finished.
    Tool {
        /// Its name.
        name: String,
        /// Whether it succeeded.
        ok: bool,
    },
    /// The final report.
    Report {
        /// The report.
        report: Box<AgentReport>,
    },
    /// Something failed.
    Error {
        /// What.
        message: String,
    },
}

/// The agent's limits.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentPolicy {
    /// Model calls per prompt.
    pub max_rounds: usize,
    /// Tool calls per prompt.
    pub max_tool_calls: usize,
    /// Wall milliseconds between polls of a running simulation.
    pub poll_ms: u64,
    /// Polls before a run is given up on.
    pub max_polls: usize,
    /// The speed a run is started at: 0 is as fast as the engine goes.
    pub speed: f64,
    /// The longest simulated duration the agent may set without being asked, seconds.
    pub max_duration_s: f64,
}

impl Default for AgentPolicy {
    fn default() -> Self {
        AgentPolicy {
            max_rounds: 16,
            max_tool_calls: 40,
            poll_ms: 500,
            max_polls: 14_400,
            speed: 0.0,
            max_duration_s: 300.0,
        }
    }
}

/// The agent.
#[derive(Debug)]
pub struct Agent<P: LlmProvider, T: RpcTransport> {
    provider: Option<P>,
    rpc: T,
    grounding: Grounding,
    thresholds: Thresholds,
    policy: AgentPolicy,
    history: Vec<ChatMessage>,
    runs: Vec<RunRecord>,
    prepared: Option<Prepared>,
    last_document: Option<Value>,
    cancel: Arc<AtomicBool>,
    corpus: String,
}

/// The tools the model is offered, in the chat-completions shape both providers take.
#[must_use]
pub fn agent_tools() -> Vec<Value> {
    let tool = |name: &str, description: &str, parameters: Value| {
        json!({"type": "function", "function": {
            "name": name, "description": description, "parameters": parameters}})
    };
    vec![
        tool(
            "list_scenarios",
            "The scenarios shipped with the simulator (id, name, description, tags). Pick the \
             one closest to the request as the base.",
            json!({"type": "object", "additionalProperties": false, "properties": {}}),
        ),
        tool(
            "get_scenario",
            "A scenario's settings: a preset id, `current` (what the engine has loaded) or \
             `last` (the last scenario this conversation ran). Returns the document.",
            json!({"type": "object", "additionalProperties": false, "required": ["base"],
                   "properties": {"base": {"type": "string"}}}),
        ),
        tool(
            "prepare_scenario",
            "Build the scenario to run: start from `base` (a preset id, `current` or `last`) \
             and apply only the changes the request implies. Each change is a dotted key \
             (e.g. `radio.rat`, `actors.vehicles.demand.rate_veh_per_h`, `time.duration_s`) \
             with the new value, or {\"multiply\": k} to scale a number, and why. The \
             result is validated by the engine's own loader; fix and retry on errors. \
             `metrics` is set to [all] so every layer is measured.",
            json!({"type": "object", "additionalProperties": false,
                   "required": ["base", "reason", "changes"],
                   "properties": {
                       "base": {"type": "string"},
                       "reason": {"type": "string",
                                  "description": "why this base, in one sentence"},
                       "changes": {"type": "array", "items": {
                           "type": "object", "additionalProperties": false,
                           "required": ["key", "value", "why"],
                           "properties": {
                               "key": {"type": "string"},
                               "value": {},
                               "why": {"type": "string"}}}}}}),
        ),
        tool(
            "run_prepared",
            "Run the prepared scenario to its end, watching it, then analyse it with the \
             deterministic analyst. Returns the analysis: findings by layer with their \
             numbers, where, and references. Takes as long as the simulation takes.",
            json!({"type": "object", "additionalProperties": false, "required": ["label"],
                   "properties": {"label": {"type": "string",
                       "description": "a short name for this run, e.g. baseline, lte-v2x"}}}),
        ),
        tool(
            "analyse_current_run",
            "Analyse the run the engine is serving now, without starting anything.",
            json!({"type": "object", "additionalProperties": false,
                   "properties": {"label": {"type": "string"}}}),
        ),
        tool(
            "compare_runs",
            "Compare two analysed runs of this conversation (default: the last two): the \
             change in every headline number both measured, and the findings each has \
             that the other does not.",
            json!({"type": "object", "additionalProperties": false,
                   "properties": {"a": {"type": "integer"}, "b": {"type": "integer"}}}),
        ),
        tool(
            "finding_detail",
            "One finding of a run in full: detail, every number, where, links.",
            json!({"type": "object", "additionalProperties": false, "required": ["id"],
                   "properties": {"id": {"type": "string"}, "run": {"type": "integer"}}}),
        ),
        tool(
            "registry__metric",
            "A metric's definition: formula, unit, what it does not account for, citation.",
            json!({"type": "object", "additionalProperties": false, "required": ["name"],
                   "properties": {"name": {"type": "string"}}}),
        ),
        tool(
            "registry__model_card",
            "A model's card: purpose, parameters with sources, limitations, validation.",
            json!({"type": "object", "additionalProperties": false, "required": ["id"],
                   "properties": {"id": {"type": "string"}}}),
        ),
    ]
}

/// The agent's instructions. Stable byte for byte, so the prompt cache holds.
#[must_use]
pub fn agent_system_prompt() -> String {
    "You run a V2X world simulator for the person you are talking to: you set up a \
     scenario, run it, and explain how it went. You know nothing about this simulator from \
     training; everything you say about it comes from a tool result in this conversation.\n\
     \n\
     How to work:\n\
     1. Before your first tool call, say in two to four short lines what you will do.\n\
     2. Pick a base with list_scenarios: the shipped scenario closest to the request. \
     Prefer a small, fast one (a procedural grid) unless the request names a city, a \
     credential system, pedestrians or a scale that needs another. Then call \
     prepare_scenario with ONLY the changes the request implies, and say in `reason` and \
     in each `why` what you chose and why. Keep the base's duration unless asked; long \
     runs take minutes.\n\
     3. If prepare_scenario reports errors, fix the change it names and try again. Never \
     run an invalid scenario.\n\
     4. run_prepared runs it and returns the analyst's findings.\n\
     5. Follow-ups (\"now double the density\", \"compare with LTE-V2X\"): prepare from \
     base `last` with the one change, run it with a new label, then compare_runs.\n\
     6. Then write the overview, in Markdown, short: a two-sentence summary; the \
     bottlenecks, most severe first, each naming its finding id in square brackets, e.g. \
     [channel.congested], with where and its numbers; for a comparison, what changed; and \
     which layers were not measured.\n\
     \n\
     The one hard rule: NEVER state a number that is not in a tool result of this \
     conversation, and never compute one (no differences, ratios or percentages of your \
     own: compare_runs gives the changes). Quote numbers with their units exactly as the \
     tools give them. If something was not measured, say so; do not guess.\n\
     \n\
     The person can stop you at any time. Do not start runs the request did not ask for."
        .to_string()
}

impl<P: LlmProvider, T: RpcTransport> Agent<P, T> {
    /// An agent over a transport, with a model or without one.
    ///
    /// # Errors
    /// [`CopilotError::Grounding`] if the model registry cannot be built.
    pub fn new(provider: Option<P>, rpc: T) -> Result<Self> {
        Ok(Agent {
            provider,
            rpc,
            grounding: Grounding::builtin()?,
            thresholds: Thresholds::default(),
            policy: AgentPolicy::default(),
            history: Vec::new(),
            runs: Vec::new(),
            prepared: None,
            last_document: None,
            cancel: Arc::new(AtomicBool::new(false)),
            corpus: String::new(),
        })
    }

    /// The same, with other limits.
    #[must_use]
    pub fn with_policy(mut self, policy: AgentPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// The flag that stops it. Set it from any thread; the agent stops at its next check
    /// and pauses a run it started.
    #[must_use]
    pub fn cancel_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancel)
    }

    /// Whether a model is attached.
    #[must_use]
    pub fn has_model(&self) -> bool {
        self.provider.is_some()
    }

    /// The model's name, or `none`.
    #[must_use]
    pub fn provider_name(&self) -> String {
        self.provider
            .as_ref()
            .map_or_else(|| "none".to_string(), |p| p.name().to_string())
    }

    /// The runs of this conversation.
    #[must_use]
    pub fn runs(&self) -> &[RunRecord] {
        &self.runs
    }

    /// Forgets the conversation and its runs.
    pub fn reset(&mut self) {
        self.history.clear();
        self.runs.clear();
        self.prepared = None;
        self.last_document = None;
        self.corpus.clear();
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// Answers one prompt end to end.
    ///
    /// # Errors
    /// [`CopilotError::MissingApiKey`] with no model attached (use
    /// [`Agent::run_without_model`]), the provider's error, or
    /// [`CopilotError::Budget`] when the turn hits a limit. A tool that fails is not an
    /// error: the model is told, and decides.
    pub fn ask(&mut self, prompt: &str, sink: &mut dyn FnMut(AgentEvent)) -> Result<AgentReport> {
        self.cancel.store(false, Ordering::SeqCst);
        if self.provider.is_none() {
            return Err(CopilotError::MissingApiKey {
                variable: crate::claude::KEY_VARIABLE.to_string(),
            });
        }
        let runs_before = self.runs.len();
        let mut comparison: Option<Comparison> = None;
        self.history.push(ChatMessage::user(prompt));
        self.corpus.push_str(prompt);
        self.corpus.push('\n');
        let tools = agent_tools();
        let system = agent_system_prompt();
        let mut calls = 0usize;
        let mut rewrite_asked = false;

        for _round in 0..self.policy.max_rounds {
            if self.cancelled() {
                return Ok(self.stopped(prompt, runs_before, comparison, sink));
            }
            let mut messages = Vec::with_capacity(self.history.len() + 1);
            messages.push(ChatMessage::system(system.clone()));
            messages.extend(self.history.iter().cloned());
            let provider = self.provider.as_mut().expect("checked above");
            let completion = provider.complete(&ChatRequest {
                messages,
                tools: tools.clone(),
            })?;
            self.history.push(completion.message.clone());

            if completion.message.tool_calls.is_empty() {
                let overview = completion.message.text().trim().to_string();
                let ungrounded = ungrounded_numbers(&overview, &self.corpus);
                if !ungrounded.is_empty() && !rewrite_asked {
                    // Appended, never edited in place: the history is append-only.
                    rewrite_asked = true;
                    let note = format!(
                        "[harness check] Your overview states numbers that no tool result in \
                         this conversation contains: {}. Rewrite the overview using only \
                         numbers that appear in the tool results, exactly as given.",
                        ungrounded.join(", ")
                    );
                    sink(AgentEvent::Status {
                        text: "Checking the overview's numbers against the analysis; asking \
                               for a rewrite."
                            .into(),
                    });
                    self.history.push(ChatMessage::user(note));
                    continue;
                }
                let report = self.report(prompt, runs_before, overview, ungrounded, comparison, false);
                sink(AgentEvent::Report { report: Box::new(report.clone()) });
                return Ok(report);
            }

            let text = completion.message.text().trim().to_string();
            if !text.is_empty() {
                sink(AgentEvent::Plan { text });
            }
            for call in &completion.message.tool_calls {
                if calls >= self.policy.max_tool_calls {
                    return Err(CopilotError::Budget {
                        what: "tool-call",
                        budget: self.policy.max_tool_calls,
                    });
                }
                calls += 1;
                let outcome = if self.cancelled() {
                    ToolOutcome::Refused { reason: "stopped by the person".into() }
                } else {
                    match serde_json::from_str::<Value>(if call.arguments.trim().is_empty() {
                        "{}"
                    } else {
                        &call.arguments
                    }) {
                        Ok(args) if args.is_object() => {
                            self.dispatch(&call.name, &args, sink, &mut comparison)
                        }
                        Ok(_) | Err(_) => ToolOutcome::Failed {
                            message: "the arguments are not a JSON object".into(),
                        },
                    }
                };
                sink(AgentEvent::Tool {
                    name: call.name.clone(),
                    ok: matches!(outcome, ToolOutcome::Ok { .. }),
                });
                let payload = serde_json::to_string(&outcome).unwrap_or_else(|_| {
                    "{\"status\":\"failed\",\"message\":\"unserialisable\"}".into()
                });
                self.corpus.push_str(&payload);
                self.corpus.push('\n');
                self.history.push(ChatMessage::tool(call.id.clone(), payload));
            }
        }
        Err(CopilotError::Budget {
            what: "round",
            budget: self.policy.max_rounds,
        })
    }

    fn stopped(
        &mut self,
        prompt: &str,
        runs_before: usize,
        comparison: Option<Comparison>,
        sink: &mut dyn FnMut(AgentEvent),
    ) -> AgentReport {
        let report = self.report(
            prompt,
            runs_before,
            "Stopped by the person. Whatever ran before the stop is analysed below.".into(),
            Vec::new(),
            comparison,
            true,
        );
        sink(AgentEvent::Report { report: Box::new(report.clone()) });
        report
    }

    /// Runs one tool.
    fn dispatch(
        &mut self,
        tool: &str,
        args: &Value,
        sink: &mut dyn FnMut(AgentEvent),
        comparison: &mut Option<Comparison>,
    ) -> ToolOutcome {
        let s = |k: &str| args.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let result = match tool {
            "list_scenarios" => self
                .rpc
                .call("scenario.list", &json!({"kind": "presets"}))
                .map(|v| {
                    let items: Vec<Value> = v
                        .get("items")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(|i| json!({"id": i["id"], "name": i["name"],
                                        "description": i["description"], "tags": i["tags"]}))
                        .collect();
                    json!({"scenarios": items})
                }),
            "get_scenario" => self.base_document(&s("base")).map(|d| json!({"scenario": d})),
            "prepare_scenario" => {
                let changes = args.get("changes").and_then(Value::as_array).cloned().unwrap_or_default();
                self.prepare(&s("base"), &s("reason"), &changes, sink)
            }
            "run_prepared" => {
                let label = if s("label").is_empty() { format!("run {}", self.runs.len() + 1) } else { s("label") };
                self.run_prepared(&label, sink).map(|r| r.analysis.compact())
            }
            "analyse_current_run" => {
                let label = if s("label").is_empty() { "current run".to_string() } else { s("label") };
                self.analyse_current(&label, sink).map(|r| r.analysis.compact())
            }
            "compare_runs" => {
                let n = self.runs.len();
                let idx = |k: &str, d: usize| args.get(k).and_then(Value::as_u64).map_or(d, |v| v as usize);
                if n < 2 {
                    Err(CopilotError::Rpc { method: "compare_runs".into(),
                        message: "fewer than two runs in this conversation".into() })
                } else {
                    let (a, b) = (idx("a", n - 2), idx("b", n - 1));
                    match (self.runs.get(a), self.runs.get(b)) {
                        (Some(ra), Some(rb)) => {
                            let c = analyst::compare(&ra.analysis, &rb.analysis);
                            *comparison = Some(c.clone());
                            Ok(json!({"a": ra.label, "b": rb.label,
                                      "deltas": c.deltas.iter().map(|d| json!({
                                          "metric": d.metric, "a": fmt(d.a), "b": fmt(d.b),
                                          "change": fmt(d.change)})).collect::<Vec<_>>(),
                                      "only_in_a": c.only_in_a, "only_in_b": c.only_in_b}))
                        }
                        _ => Err(CopilotError::Rpc { method: "compare_runs".into(),
                            message: format!("there are {n} runs, numbered from 0") }),
                    }
                }
            }
            "finding_detail" => {
                let run = args.get("run").and_then(Value::as_u64).map(|v| v as usize)
                    .unwrap_or_else(|| self.runs.len().saturating_sub(1));
                match self.runs.get(run).and_then(|r| r.analysis.finding(&s("id"))) {
                    Some(f) => serde_json::to_value(f).map_err(CopilotError::from),
                    None => Err(CopilotError::Rpc { method: "finding_detail".into(),
                        message: "no such finding in that run".into() }),
                }
            }
            "registry__metric" | "registry__model_card" => {
                return local_call(&self.grounding, tool, args);
            }
            other => {
                return ToolOutcome::Refused {
                    reason: format!("{other:?} is not one of this agent's tools"),
                };
            }
        };
        match result {
            Ok(result) => ToolOutcome::Ok { result },
            Err(e) => ToolOutcome::Failed { message: e.to_string() },
        }
    }

    /// The document a base names.
    fn base_document(&mut self, base: &str) -> Result<Value> {
        match base {
            "" | "current" => self
                .rpc
                .call("scenario.get", &json!({}))
                .map(|v| v.get("scenario").cloned().unwrap_or(Value::Null)),
            "last" => self.last_document.clone().ok_or_else(|| CopilotError::Rpc {
                method: "get_scenario".into(),
                message: "nothing has run in this conversation yet; use a preset id".into(),
            }),
            id => self
                .rpc
                .call("scenario.load", &json!({"path": id}))
                .map(|v| v.get("scenario").cloned().unwrap_or(Value::Null)),
        }
    }

    /// Builds and validates the scenario to run.
    fn prepare(
        &mut self,
        base: &str,
        reason: &str,
        changes: &[Value],
        sink: &mut dyn FnMut(AgentEvent),
    ) -> Result<Value> {
        let mut doc = self.base_document(base)?;
        if !doc.is_object() {
            return Err(CopilotError::Rpc {
                method: "prepare_scenario".into(),
                message: format!("the base {base:?} gave no scenario document"),
            });
        }
        let mut applied = Vec::new();
        for c in changes {
            let key = c.get("key").and_then(Value::as_str).unwrap_or("").to_string();
            let why = c.get("why").and_then(Value::as_str).unwrap_or("").to_string();
            let value = c.get("value").cloned().unwrap_or(Value::Null);
            let pointer = to_pointer(&key);
            let before = doc.pointer(&pointer).cloned().unwrap_or(Value::Null);
            let after = match value.get("multiply").and_then(Value::as_f64) {
                Some(k) if value.as_object().is_some_and(|o| o.len() == 1) => {
                    let Some(b) = before.as_f64() else {
                        return Err(CopilotError::Rpc {
                            method: "prepare_scenario".into(),
                            message: format!("`{key}` is not a number in the base, so it cannot be multiplied"),
                        });
                    };
                    json!(b * k)
                }
                _ => value,
            };
            if let Some(d) = after.as_f64().filter(|_| key == "time.duration_s") {
                if d > self.policy.max_duration_s {
                    return Err(CopilotError::Rpc {
                        method: "prepare_scenario".into(),
                        message: format!(
                            "a duration over {} s takes a long time to simulate; ask the \
                             person first, or keep it shorter",
                            fmt(self.policy.max_duration_s)
                        ),
                    });
                }
            }
            set_pointer(&mut doc, &pointer, after.clone());
            applied.push(Change { key, before, after, why });
        }
        let all = json!(["all"]);
        if doc.get("metrics") != Some(&all) {
            let before = doc.get("metrics").cloned().unwrap_or(Value::Null);
            doc["metrics"] = all.clone();
            applied.push(Change {
                key: "metrics".into(),
                before,
                after: all,
                why: "the analyst reads every layer, so every metric is measured".into(),
            });
        }
        let report = check_document(doc.clone());
        sink(AgentEvent::Changes {
            base: base.to_string(),
            reason: reason.to_string(),
            changes: applied.clone(),
            valid: report.ok,
            errors: report.errors.clone(),
        });
        if !report.ok {
            return Ok(json!({"ok": false, "errors": report.errors,
                             "changes": applied, "note": "nothing was staged; fix and retry"}));
        }
        let hash = report.content_hash.clone().unwrap_or_default();
        self.prepared = Some(Prepared {
            base: base.to_string(),
            reason: reason.to_string(),
            changes: applied.clone(),
            scenario_hash: hash.clone(),
            document: doc,
        });
        Ok(json!({"ok": true, "scenario_hash": hash, "changes": applied,
                  "warnings": report.warnings}))
    }

    /// Runs the prepared scenario to its end and analyses it.
    ///
    /// # Errors
    /// With nothing prepared, when the server refuses the start, or when the run stalls.
    pub fn run_prepared(&mut self, label: &str, sink: &mut dyn FnMut(AgentEvent)) -> Result<RunRecord> {
        let prepared = self.prepared.clone().ok_or_else(|| CopilotError::Rpc {
            method: "run_prepared".into(),
            message: "nothing is prepared; call prepare_scenario first".into(),
        })?;
        // A start is refused while a run is moving; pause first (an error just means it
        // was not running).
        let _ = self.rpc.call("run.pause", &json!({}));
        let started = self.rpc.call(
            "run.start",
            &json!({"scenario": prepared.document, "speed": self.policy.speed, "paused": false}),
        )?;
        let run_id = started.get("run_id").and_then(Value::as_str).unwrap_or("").to_string();
        let t_end_s = started.get("t_end_ns").and_then(Value::as_u64).unwrap_or(0) as f64 / 1e9;
        sink(AgentEvent::RunStarted { label: label.into(), run_id, t_end_s });
        self.last_document = Some(prepared.document.clone());
        let t_s = self.watch(label, sink)?;
        let evidence = gather(&mut self.rpc)?;
        let analysis = analyst::analyse(&evidence, &self.thresholds);
        let record = RunRecord {
            label: label.to_string(),
            prepared: Some(prepared),
            analysis,
            t_s,
        };
        sink(AgentEvent::Analysed { label: label.into(), analysis: record.analysis.compact() });
        self.runs.push(record.clone());
        Ok(record)
    }

    /// Polls the run until it ends, is stopped, or stalls. Returns the simulated time
    /// reached.
    fn watch(&mut self, label: &str, sink: &mut dyn FnMut(AgentEvent)) -> Result<f64> {
        let mut last_t = 0u64;
        let mut still = 0usize;
        for poll in 0..self.policy.max_polls {
            if self.cancelled() {
                let _ = self.rpc.call("run.pause", &json!({}));
                return Err(CopilotError::Rpc {
                    method: "run_prepared".into(),
                    message: "stopped by the person; the run was paused".into(),
                });
            }
            let st = self.rpc.call("run.status", &json!({}))?;
            let t = st.get("t_ns").and_then(Value::as_u64).unwrap_or(0);
            let end = st.get("t_end_ns").and_then(Value::as_u64).unwrap_or(0);
            let state = st.get("state").and_then(Value::as_str).unwrap_or("");
            let finished = st.pointer("/engine/finished").and_then(Value::as_bool).unwrap_or(false);
            let actors = st.get("actors").and_then(Value::as_u64).unwrap_or(0);
            if poll % 2 == 0 || t >= end {
                sink(AgentEvent::Progress {
                    label: label.into(),
                    t_s: t as f64 / 1e9,
                    t_end_s: end as f64 / 1e9,
                    actors,
                });
            }
            if (end > 0 && t >= end) || state == "ended" || state == "stopped" || (finished && t == last_t && poll > 0) {
                return Ok(t as f64 / 1e9);
            }
            if let Some(f) = st.pointer("/engine/failure").filter(|f| !f.is_null()) {
                return Err(CopilotError::Rpc {
                    method: "run_prepared".into(),
                    message: format!("the engine failed: {f}"),
                });
            }
            if t == last_t {
                still += 1;
                // Two minutes without progress while not running is a stall worth naming.
                if still * self.policy.poll_ms as usize > 120_000 && state != "running" {
                    return Err(CopilotError::Rpc {
                        method: "run_prepared".into(),
                        message: format!("the run stopped advancing at {} s (state {state})",
                                         fmt(t as f64 / 1e9)),
                    });
                }
            } else {
                still = 0;
                last_t = t;
            }
            if self.policy.poll_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(self.policy.poll_ms));
            }
        }
        Err(CopilotError::Budget { what: "poll", budget: self.policy.max_polls })
    }

    /// Analyses the run the engine is serving, without starting anything. Needs no model.
    ///
    /// # Errors
    /// When the server cannot be read.
    pub fn analyse_current(&mut self, label: &str, sink: &mut dyn FnMut(AgentEvent)) -> Result<RunRecord> {
        let evidence = gather(&mut self.rpc)?;
        let analysis = analyst::analyse(&evidence, &self.thresholds);
        let record = RunRecord {
            label: label.to_string(),
            prepared: None,
            t_s: evidence.t_reached_s,
            analysis,
        };
        sink(AgentEvent::Analysed { label: label.into(), analysis: record.analysis.compact() });
        self.runs.push(record.clone());
        Ok(record)
    }

    /// Runs a scenario with no model: the base unchanged except that every metric is
    /// measured, then the analysis and a deterministic summary.
    ///
    /// # Errors
    /// As [`Agent::run_prepared`], or when the base does not validate.
    pub fn run_without_model(
        &mut self,
        prompt: &str,
        base: &str,
        sink: &mut dyn FnMut(AgentEvent),
    ) -> Result<AgentReport> {
        self.cancel.store(false, Ordering::SeqCst);
        let runs_before = self.runs.len();
        sink(AgentEvent::Status { text: crate::claude::HOW_TO_ADD_A_KEY.to_string() });
        let prepared = self.prepare(
            base,
            "no model is attached, so the named scenario runs as shipped",
            &[],
            sink,
        )?;
        if prepared.get("ok") != Some(&json!(true)) {
            return Err(CopilotError::Rpc {
                method: "prepare_scenario".into(),
                message: format!("the scenario does not validate: {}", prepared["errors"]),
            });
        }
        let outcome = self.run_prepared(base, sink);
        let stopped = matches!(&outcome, Err(CopilotError::Rpc { message, .. }) if message.starts_with("stopped"));
        if !stopped {
            outcome?;
        }
        let overview = deterministic_summary(self.runs.last().map(|r| &r.analysis));
        let report = self.report(prompt, runs_before, overview, Vec::new(), None, stopped);
        sink(AgentEvent::Report { report: Box::new(report.clone()) });
        Ok(report)
    }

    fn report(
        &self,
        prompt: &str,
        runs_before: usize,
        overview: String,
        ungrounded: Vec<String>,
        comparison: Option<Comparison>,
        stopped: bool,
    ) -> AgentReport {
        let runs: Vec<RunRecord> = self.runs[runs_before.min(self.runs.len())..].to_vec();
        let mut report = AgentReport {
            prompt: prompt.to_string(),
            provider: self.provider_name(),
            overview,
            ungrounded,
            runs,
            comparison,
            stopped,
            markdown: String::new(),
        };
        report.markdown = to_markdown(&report);
        report
    }
}

/// A summary written from the findings alone, for a report with no model.
#[must_use]
pub fn deterministic_summary(analysis: Option<&Analysis>) -> String {
    let Some(a) = analysis else {
        return "No run was analysed.".to_string();
    };
    let top: Vec<&Finding> = a.findings.iter().filter(|f| f.severity >= Severity::Warning).take(5).collect();
    let mut out = String::from("Summary written from the analyst's findings (no model attached).\n\n");
    if top.is_empty() {
        out.push_str("No bottleneck at warning level or above was found in the measured layers.\n");
    } else {
        for f in top {
            out.push_str(&format!("- [{}] {}\n", f.id, f.title));
        }
    }
    if !a.not_measured.is_empty() {
        out.push_str(&format!(
            "\nNot measured: {}.\n",
            a.not_measured.iter().map(|l| l.title()).collect::<Vec<_>>().join(", ")
        ));
    }
    out
}

/// The report as Markdown.
#[must_use]
pub fn to_markdown(r: &AgentReport) -> String {
    let mut md = String::new();
    md.push_str("# Simulation report\n\n");
    md.push_str(&format!("> {}\n\n", r.prompt.replace('\n', " ")));
    if r.stopped {
        md.push_str("**Stopped by the person.**\n\n");
    }
    md.push_str("## Overview\n\n");
    md.push_str(r.overview.trim());
    md.push_str("\n\n");
    if !r.ungrounded.is_empty() {
        md.push_str(&format!(
            "> **Unverified numbers.** The overview states {} that no analysis result \
             contains; trust the tables below over the prose.\n\n",
            r.ungrounded.join(", ")
        ));
    }
    for run in &r.runs {
        md.push_str(&format!("## Run: {}\n\n", run.label));
        md.push_str(&format!(
            "Run `{}`, scenario `{}`, {} s simulated.\n\n",
            run.analysis.run_id,
            short(&run.analysis.scenario_hash),
            fmt(run.t_s)
        ));
        if let Some(p) = &run.prepared {
            md.push_str(&format!("Built from `{}`: {}\n\n", p.base, p.reason));
            if !p.changes.is_empty() {
                md.push_str("| Setting | Before | After | Why |\n|---|---|---|---|\n");
                for c in &p.changes {
                    md.push_str(&format!(
                        "| `{}` | {} | {} | {} |\n",
                        c.key,
                        cell(&c.before),
                        cell(&c.after),
                        c.why.replace('|', "/")
                    ));
                }
                md.push('\n');
            }
        }
        for layer in Layer::ALL {
            let fs = run.analysis.in_layer(layer);
            if fs.is_empty() {
                continue;
            }
            md.push_str(&format!("### {}\n\n", layer.title()));
            for f in fs {
                md.push_str(&format!(
                    "- **{}** `{}` ({:?}): {}\n",
                    f.title,
                    f.id,
                    f.severity,
                    f.detail
                ));
                for e in &f.evidence {
                    md.push_str(&format!(
                        "  - {}: {} {} (`{}`)\n",
                        e.label,
                        fmt(e.value),
                        e.unit,
                        e.source
                    ));
                }
                let w = &f.location;
                let mut wh = Vec::new();
                if !w.nodes.is_empty() {
                    wh.push(format!("nodes {}", w.nodes.join(", ")));
                }
                if !w.regions.is_empty() {
                    wh.push(format!("regions {}", w.regions.join(", ")));
                }
                if !w.entities.is_empty() {
                    wh.push(format!("entities {}", w.entities.join(", ")));
                }
                if !w.distance_bins.is_empty() {
                    wh.push(format!("distance {} m", w.distance_bins.join(", ")));
                }
                if let Some([a, b]) = w.window_s {
                    wh.push(format!("t {}–{} s", fmt(a), fmt(b)));
                }
                if !wh.is_empty() {
                    md.push_str(&format!("  - where: {}\n", wh.join("; ")));
                }
                if let Some(rf) = &f.reference {
                    md.push_str(&format!("  - judged against: {rf}\n"));
                }
                if !f.links.is_empty() {
                    md.push_str(&format!(
                        "  - charts: {}\n",
                        f.links.iter().map(|l| format!("`{}`", l.chart)).collect::<Vec<_>>().join(", ")
                    ));
                }
            }
            md.push('\n');
        }
    }
    if let Some(c) = &r.comparison {
        md.push_str("## Comparison\n\n| Metric | A | B | Change |\n|---|---|---|---|\n");
        for d in &c.deltas {
            md.push_str(&format!("| `{}` | {} | {} | {} |\n", d.metric, fmt(d.a), fmt(d.b), fmt(d.change)));
        }
        if !c.only_in_b.is_empty() {
            md.push_str(&format!("\nNew in B: {}\n", c.only_in_b.join(", ")));
        }
        if !c.only_in_a.is_empty() {
            md.push_str(&format!("\nGone in B: {}\n", c.only_in_a.join(", ")));
        }
        md.push('\n');
    }
    if let Some(run) = r.runs.first() {
        md.push_str("## Thresholds\n\n");
        for (t, src) in &run.analysis.thresholds {
            md.push_str(&format!("- {t}: {src}\n"));
        }
        md.push('\n');
    }
    md.push_str(&format!("_Overview by {}; every number above the overview's is the analyst's._\n", r.provider));
    md
}

fn short(h: &str) -> &str {
    h.get(..12).unwrap_or(h)
}

fn cell(v: &Value) -> String {
    match v {
        Value::Null => "—".into(),
        Value::String(s) => s.clone(),
        Value::Number(n) => n.as_f64().map_or_else(|| n.to_string(), fmt),
        other => other.to_string(),
    }
}

/// A dotted key or a JSON Pointer, as a JSON Pointer.
#[must_use]
pub fn to_pointer(key: &str) -> String {
    if key.starts_with('/') {
        return key.to_string();
    }
    let mut out = String::new();
    for part in key.split('.') {
        out.push('/');
        out.push_str(&part.replace('~', "~0").replace('/', "~1"));
    }
    out
}

/// Sets a value at a JSON Pointer, creating objects on the way.
fn set_pointer(doc: &mut Value, pointer: &str, value: Value) {
    let parts: Vec<String> = pointer
        .split('/')
        .skip(1)
        .map(|p| p.replace("~1", "/").replace("~0", "~"))
        .collect();
    let mut cur = doc;
    for (i, p) in parts.iter().enumerate() {
        let last = i + 1 == parts.len();
        if let Value::Array(a) = cur {
            if let Ok(idx) = p.parse::<usize>() {
                if idx < a.len() {
                    if last {
                        a[idx] = value;
                        return;
                    }
                    cur = &mut a[idx];
                    continue;
                }
            }
            return;
        }
        if !cur.is_object() {
            *cur = Value::Object(Map::new());
        }
        let obj = cur.as_object_mut().expect("made an object");
        if last {
            obj.insert(p.clone(), value);
            return;
        }
        cur = obj.entry(p.clone()).or_insert_with(|| Value::Object(Map::new()));
    }
}

/// Every number in `text`, as written.
fn numbers_in(text: &str, standalone_only: bool) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == ',' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit()
                || chars[i] == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())
            {
                i += 1;
            }
            let before = if start > 0 { Some(chars[start - 1]) } else { None };
            let after = chars.get(i).copied();
            // `802.11p`, `p95`, `J2945/1`, `3GPP`, `node7`, ids and version strings are
            // names, not quantities. A unit glued on (`100ms`, `30s`, `2x`) is not.
            let named = before.is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '/' | '`' | '.'))
                || after.is_some_and(|c| (c.is_alphabetic() && !matches!(c, 's' | 'm' | 'x'))
                    || matches!(c, '/' | '_'));
            if !standalone_only || !named {
                out.push(chars[start..i].iter().collect::<String>().replace(',', ""));
            }
        } else {
            i += 1;
        }
    }
    out
}

/// The numbers of `overview` that appear nowhere in `corpus` (the tool results and the
/// person's own words), allowing a ratio to be written as a percentage and the small
/// integers that prose counts with (`three findings`, `2 runs`).
#[must_use]
pub fn ungrounded_numbers(overview: &str, corpus: &str) -> Vec<String> {
    let known: Vec<f64> = numbers_in(corpus, false)
        .iter()
        .filter_map(|n| n.parse::<f64>().ok())
        .collect();
    let close = |x: f64, y: f64| {
        let scale = x.abs().max(y.abs()).max(1e-9);
        (x - y).abs() / scale < 0.005
    };
    let mut out = Vec::new();
    for n in numbers_in(overview, true) {
        let Ok(x) = n.parse::<f64>() else { continue };
        if x.fract() == 0.0 && x.abs() <= 10.0 {
            continue;
        }
        let ok = known
            .iter()
            .any(|&k| close(x, k) || close(x, k * 100.0) || close(x / 100.0, k) || close(x, k * 1000.0) || close(x / 1000.0, k));
        if !ok && !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_method_the_agent_calls_is_one_the_server_publishes() {
        for m in AGENT_METHODS {
            assert!(v2xw_server::rpc::METHODS.contains(&m), "{m} is not a server method");
        }
    }

    #[test]
    fn dotted_keys_become_pointers_and_set_values() {
        assert_eq!(to_pointer("radio.rat"), "/radio/rat");
        assert_eq!(to_pointer("/time/duration_s"), "/time/duration_s");
        let mut d = json!({"radio": {"rat": "dsrc-80211p"}});
        set_pointer(&mut d, "/radio/rat", json!("lte-v2x-pc5"));
        set_pointer(&mut d, "/actors/vehicles/demand/rate_veh_per_h", json!(600.0));
        assert_eq!(d["radio"]["rat"], "lte-v2x-pc5");
        assert_eq!(d["actors"]["vehicles"]["demand"]["rate_veh_per_h"], 600.0);
    }

    #[test]
    fn a_number_the_tools_did_not_produce_is_caught() {
        let corpus = r#"{"label":"mean channel busy ratio","value":"0.74","unit":"ratio"} 22.185"#;
        assert!(ungrounded_numbers("The CBR was 0.74 [channel.congested].", corpus).is_empty());
        assert!(ungrounded_numbers("The CBR was 74% of airtime.", corpus).is_empty());
        assert!(ungrounded_numbers("Two runs; 802.11p; p95; TS 22.185.", corpus).is_empty());
        assert_eq!(ungrounded_numbers("The CBR was 0.81.", corpus), vec!["0.81".to_string()]);
        assert_eq!(ungrounded_numbers("PDR dropped by 37 %.", corpus), vec!["37".to_string()]);
    }
}
