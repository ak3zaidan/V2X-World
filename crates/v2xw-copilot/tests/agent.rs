//! The agent loop end to end, with no network: a scripted model drives it through
//! choosing a scenario, changing it, validating it with the engine's real loader, running
//! it against a fake engine, analysing the run and writing an overview; then a follow-up
//! that changes one setting, runs again and compares.
//!
//! The fake engine answers the same JSON-RPC methods the real server does, with numbers
//! that depend on the scenario it was started with (more demand, busier channel), so the
//! analysis the agent reports is a function of the change the model asked for.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use v2xw_copilot::agent::{Agent, AgentEvent, AgentPolicy};
use v2xw_copilot::provider::ScriptedProvider;
use v2xw_copilot::{ChatMessage, Completion, CopilotError, Result, RpcTransport, ToolCall};

fn base_scenario() -> Value {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/phase1-grid.yaml"),
    )
    .expect("the shipped grid scenario reads");
    serde_yml::from_str(&text).expect("it is YAML")
}

/// A stand-in for the server: stateful, and answering by method.
struct FakeEngine {
    running: Value,
    t_ns: u64,
    t_end_ns: u64,
    runs: usize,
    pub calls: Arc<Mutex<Vec<(String, Value)>>>,
    /// Set on the n-th `run.status` (a person pressing Stop mid-run).
    stop_at_status: Option<(usize, Arc<Mutex<Option<Arc<AtomicBool>>>>)>,
    statuses: AtomicUsize,
}

impl FakeEngine {
    fn new() -> Self {
        FakeEngine {
            running: base_scenario(),
            t_ns: 0,
            t_end_ns: 60_000_000_000,
            runs: 0,
            calls: Arc::new(Mutex::new(Vec::new())),
            stop_at_status: None,
            statuses: AtomicUsize::new(0),
        }
    }

    fn demand(&self) -> f64 {
        self.running
            .pointer("/actors/vehicles/demand/rate_veh_per_h")
            .and_then(Value::as_f64)
            .unwrap_or(30.0)
    }

    fn lte(&self) -> bool {
        self.running.pointer("/radio/rat").and_then(Value::as_str) == Some("lte-v2x-pc5")
    }

    /// The channel busy ratio this fake reports: grows with demand, lower on the sidelink.
    fn cbr(&self) -> f64 {
        let base = 0.45 * self.demand() / 30.0;
        if self.lte() { base * 0.5 } else { base }
    }
}

impl RpcTransport for FakeEngine {
    fn endpoint(&self) -> &str {
        "<fake engine>"
    }

    fn call(&mut self, method: &str, params: &Value) -> Result<Value> {
        self.calls.lock().unwrap().push((method.to_string(), params.clone()));
        let metric = params
            .get("metrics")
            .and_then(|m| m.get(0))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        Ok(match method {
            "scenario.list" => json!({"items": [
                {"id": "phase1-grid", "name": "phase1-grid", "description": "procedural grid",
                 "tags": ["procedural"]},
                {"id": "manhattan-5min", "name": "manhattan-5min", "description": "Midtown",
                 "tags": ["manhattan"]}]}),
            "scenario.load" => json!({"scenario": base_scenario(), "valid": true}),
            "scenario.get" => json!({"scenario": self.running}),
            "run.pause" => json!({"state": "paused"}),
            "run.start" => {
                self.running = params["scenario"].clone();
                self.runs += 1;
                self.t_ns = 0;
                let d = self.running.pointer("/time/duration_s").and_then(Value::as_f64).unwrap_or(60.0);
                self.t_end_ns = (d * 1e9) as u64;
                json!({"run_id": format!("run-{}", self.runs), "state": "running",
                       "t_end_ns": self.t_end_ns})
            }
            "run.status" => {
                let n = self.statuses.fetch_add(1, Ordering::SeqCst) + 1;
                if let Some((at, slot)) = &self.stop_at_status {
                    if n == *at {
                        if let Some(flag) = slot.lock().unwrap().as_ref() {
                            flag.store(true, Ordering::SeqCst);
                        }
                    }
                }
                self.t_ns = (self.t_ns + 20_000_000_000).min(self.t_end_ns);
                json!({"run_id": format!("run-{}", self.runs), "t_ns": self.t_ns,
                       "t_end_ns": self.t_end_ns, "actors": 12,
                       "state": if self.t_ns >= self.t_end_ns { "paused" } else { "running" },
                       "scenario_hash": "abc"})
            }
            "metrics.query" if params.get("metrics").is_none() => json!({"catalogue": [
                {"name": "cbr", "unit": "ratio", "dims": ["t", "channel"], "base": "cbr"},
                {"name": "pdr", "unit": "ratio", "dims": ["t", "radius", "dist_bin"], "base": "pdr"},
                {"name": "mac_access_delay.p95", "unit": "ms", "dims": ["t"], "base": "mac_access_delay"},
                {"name": "mean_speed", "unit": "m/s", "dims": ["t", "region"], "base": "mean_speed"},
                {"name": "e2e_latency.p95", "unit": "ms", "dims": ["t"], "base": "e2e_latency"}]}),
            "metrics.query" if params.get("group_by").is_some() => {
                // pdr by distance: falls off sooner on a busier channel.
                let k = self.cbr();
                let rows: Vec<Value> = (0..8)
                    .map(|i| {
                        let lo = i * 20;
                        let v = (1.0 - k * 0.15 * f64::from(i)).max(0.0);
                        json!([format!("{lo}-{}", lo + 20), (v * 1000.0).round() / 1000.0, null, null, 100])
                    })
                    .collect();
                json!({"rows": rows})
            }
            "metrics.query" => {
                let v = match metric.as_str() {
                    "cbr" => self.cbr(),
                    "pdr" => 0.9,
                    "mac_access_delay.p95" => 0.6,
                    "mean_speed" => 9.5,
                    "e2e_latency.p95" => 8.0,
                    _ => 0.0,
                };
                json!({"rows": [[0, (v * 1000.0).round() / 1000.0]]})
            }
            other => {
                return Err(CopilotError::Rpc {
                    method: other.to_string(),
                    message: "not in this fake".into(),
                });
            }
        })
    }
}

fn call(id: &str, name: &str, args: Value) -> ToolCall {
    ToolCall { id: id.to_string(), name: name.to_string(), arguments: args.to_string() }
}

fn tools(text: Option<&str>, calls: Vec<ToolCall>) -> Completion {
    Completion {
        message: ChatMessage::calls(text.map(str::to_string), calls),
        finish_reason: "tool_calls".into(),
    }
}

fn say(text: &str) -> Completion {
    Completion { message: ChatMessage::assistant(text), finish_reason: "stop".into() }
}

fn quick() -> AgentPolicy {
    AgentPolicy { poll_ms: 0, ..AgentPolicy::default() }
}

#[test]
fn one_prompt_configures_runs_analyses_and_reports_then_a_follow_up_compares() {
    let provider = ScriptedProvider::new(vec![
        tools(Some("I will start from the grid and double its demand."),
              vec![call("c1", "list_scenarios", json!({}))]),
        tools(None, vec![call("c2", "prepare_scenario", json!({
            "base": "phase1-grid",
            "reason": "the request names no city; the grid is the closest small scenario",
            "changes": [{"key": "actors.vehicles.demand.rate_veh_per_h",
                         "value": {"multiply": 2}, "why": "double the density"}]}))]),
        tools(None, vec![call("c3", "run_prepared", json!({"label": "double density"}))]),
        say("The channel ran above its target [channel.congested]: mean busy ratio 0.9 \
             against 0.8. Delivery fell below 0.9 from 20 m [delivery.range]."),
        // The follow-up.
        tools(None, vec![call("c4", "prepare_scenario", json!({
            "base": "last", "reason": "same scenario, other radio",
            "changes": [{"key": "radio.rat", "value": "lte-v2x-pc5", "why": "compare with LTE-V2X"}]}))]),
        tools(None, vec![call("c5", "run_prepared", json!({"label": "lte-v2x"}))]),
        tools(None, vec![call("c6", "compare_runs", json!({}))]),
        say("On LTE-V2X the busy ratio fell from 0.9 to 0.45 [channel.congested] is gone."),
    ]);
    let engine = FakeEngine::new();
    let calls = Arc::clone(&engine.calls);
    let mut agent = Agent::new(Some(provider), engine).expect("builds").with_policy(quick());
    let mut events: Vec<AgentEvent> = Vec::new();
    let report = agent
        .ask("Run the grid at double the density and tell me how the channel holds up.",
             &mut |e| events.push(e))
        .expect("the turn completes");

    // The plan, the change (with its before and after), the run, its progress, the report.
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Plan { text } if text.contains("double"))));
    let changes = events.iter().find_map(|e| match e {
        AgentEvent::Changes { changes, valid, .. } => Some((changes.clone(), *valid)),
        _ => None,
    }).expect("a Changes event");
    assert!(changes.1, "the engine's loader accepted the scenario");
    let demand = changes.0.iter().find(|c| c.key == "actors.vehicles.demand.rate_veh_per_h").expect("demand changed");
    assert_eq!(demand.before, json!(30.0));
    assert_eq!(demand.after, json!(60.0));
    assert!(changes.0.iter().any(|c| c.key == "metrics" && c.after == json!(["all"])));
    assert!(events.iter().any(|e| matches!(e, AgentEvent::RunStarted { .. })));
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Progress { t_s, .. } if *t_s >= 60.0)));
    assert!(events.iter().any(|e| matches!(e, AgentEvent::Report { .. })));

    // The run was started with the changed scenario, through the server's own method.
    let started = calls.lock().unwrap().iter().find(|(m, _)| m == "run.start").cloned().expect("started");
    assert_eq!(started.1["scenario"]["actors"]["vehicles"]["demand"]["rate_veh_per_h"], json!(60.0));

    assert_eq!(report.runs.len(), 1);
    let a = &report.runs[0].analysis;
    assert!(a.finding("channel.congested").is_some(), "{:#?}", a.findings);
    assert!(report.ungrounded.is_empty(), "{:?}", report.ungrounded);
    assert!(report.markdown.contains("# Simulation report"));
    assert!(report.markdown.contains("channel.congested"));
    assert!(report.markdown.contains("actors.vehicles.demand.rate_veh_per_h"));

    // The follow-up re-runs with the one change and compares.
    let follow = agent.ask("Now compare with LTE-V2X.", &mut |_| {}).expect("the follow-up completes");
    assert_eq!(follow.runs.len(), 1, "only the follow-up's run is in its report");
    let c = follow.comparison.expect("a comparison");
    let cbr = c.deltas.iter().find(|d| d.metric == "cbr").expect("cbr compared");
    assert!(cbr.b < cbr.a, "LTE-V2X's fake channel is quieter");
    assert!(c.only_in_a.contains(&"channel.congested".to_string()));
    let second = calls.lock().unwrap().iter().filter(|(m, _)| m == "run.start").nth(1).cloned().expect("second start");
    assert_eq!(second.1["scenario"]["radio"]["rat"], json!("lte-v2x-pc5"));
    assert_eq!(second.1["scenario"]["actors"]["vehicles"]["demand"]["rate_veh_per_h"], json!(60.0),
               "built from the last run, so the doubled demand stays");
}

#[test]
fn a_number_the_analyst_did_not_produce_is_sent_back_and_then_flagged() {
    let provider = ScriptedProvider::new(vec![
        tools(None, vec![call("c1", "prepare_scenario", json!({
            "base": "phase1-grid", "reason": "as asked", "changes": []}))]),
        tools(None, vec![call("c2", "run_prepared", json!({"label": "baseline"}))]),
        say("The busy ratio was 0.83."),          // not produced by any tool
        say("The busy ratio was 0.4123 still."),  // still not
    ]);
    let mut agent = Agent::new(Some(provider), FakeEngine::new()).expect("builds").with_policy(quick());
    let report = agent.ask("Run the grid.", &mut |_| {}).expect("completes");
    assert_eq!(report.ungrounded, vec!["0.4123".to_string()]);
    assert!(report.markdown.contains("Unverified numbers"));
}

#[test]
fn an_invalid_change_is_refused_by_the_loader_and_nothing_runs() {
    let provider = ScriptedProvider::new(vec![
        tools(None, vec![call("c1", "prepare_scenario", json!({
            "base": "phase1-grid", "reason": "r",
            "changes": [{"key": "radio.rat", "value": "carrier-pigeon", "why": "w"}]}))]),
        tools(None, vec![call("c2", "run_prepared", json!({"label": "x"}))]),
        say("The scenario could not be validated."),
    ]);
    let engine = FakeEngine::new();
    let calls = Arc::clone(&engine.calls);
    let mut agent = Agent::new(Some(provider), engine).expect("builds").with_policy(quick());
    let mut invalid = false;
    let report = agent
        .ask("Use a carrier pigeon radio.", &mut |e| {
            if let AgentEvent::Changes { valid: false, errors, .. } = &e {
                invalid = !errors.is_empty();
            }
        })
        .expect("completes");
    assert!(invalid, "the loader's errors reached the panel");
    assert!(report.runs.is_empty());
    assert!(!calls.lock().unwrap().iter().any(|(m, _)| m == "run.start"), "nothing invalid ran");
}

#[test]
fn stopping_mid_run_pauses_the_engine_and_reports_stopped() {
    let provider = ScriptedProvider::new(vec![
        tools(None, vec![call("c1", "prepare_scenario", json!({
            "base": "phase1-grid", "reason": "r", "changes": []}))]),
        tools(None, vec![call("c2", "run_prepared", json!({"label": "x"}))]),
        say("unreachable"),
    ]);
    let slot: Arc<Mutex<Option<Arc<AtomicBool>>>> = Arc::new(Mutex::new(None));
    let mut engine = FakeEngine::new();
    engine.stop_at_status = Some((2, Arc::clone(&slot)));
    let calls = Arc::clone(&engine.calls);
    let mut agent = Agent::new(Some(provider), engine).expect("builds").with_policy(quick());
    *slot.lock().unwrap() = Some(agent.cancel_handle());
    let report = agent.ask("Run the grid.", &mut |_| {}).expect("a stopped turn is not an error");
    assert!(report.stopped);
    let log = calls.lock().unwrap();
    let start = log.iter().position(|(m, _)| m == "run.start").expect("started");
    assert!(log[start..].iter().any(|(m, _)| m == "run.pause"), "the run was paused after the stop");
}

#[test]
fn with_no_model_a_scenario_still_runs_and_is_analysed() {
    let mut agent: Agent<ScriptedProvider, FakeEngine> =
        Agent::new(None, FakeEngine::new()).expect("builds").with_policy(quick());
    assert!(matches!(agent.ask("anything", &mut |_| {}), Err(CopilotError::MissingApiKey { .. })));
    let report = agent.run_without_model("Run the grid", "phase1-grid", &mut |_| {}).expect("runs");
    assert_eq!(report.provider, "none");
    assert_eq!(report.runs.len(), 1);
    assert!(report.overview.contains("no model attached"));
    assert!(report.markdown.contains("## Run: phase1-grid"));
}
