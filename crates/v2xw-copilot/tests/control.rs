//! The control tools end to end, with no network: a scripted model asks for an experiment
//! (two variants, one call), a run controlled step by step (start paused, run to a time,
//! seek back, live metrics with health), an attack run with detection, and an invalid
//! variant; a fake engine answers the server's methods with numbers that depend on the
//! scenario it was started with.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use v2xw_copilot::agent::{Agent, AgentEvent, AgentPolicy, poll_delay_ms};
use v2xw_copilot::provider::ScriptedProvider;
use v2xw_copilot::{ChatMessage, Completion, CopilotError, Result, RpcTransport, ToolCall};

fn base_scenario() -> Value {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/phase1-grid.yaml"),
    )
    .expect("the shipped grid scenario reads");
    serde_yml::from_str(&text).expect("it is YAML")
}

type Log = Arc<Mutex<Vec<(String, Value)>>>;

struct Fake {
    running: Value,
    t_ns: u64,
    t_end_ns: u64,
    runs: usize,
    paused: bool,
    log: Log,
    staged: Option<Value>,
}

impl Fake {
    fn new() -> Self {
        Fake {
            running: base_scenario(),
            t_ns: 0,
            t_end_ns: 60_000_000_000,
            runs: 0,
            paused: true,
            log: Arc::new(Mutex::new(Vec::new())),
            staged: None,
        }
    }

    fn f(&self, p: &str) -> Option<f64> {
        self.running.pointer(p).and_then(Value::as_f64)
    }

    fn lte(&self) -> bool {
        self.running.pointer("/radio/rat").and_then(Value::as_str) == Some("lte-v2x-pc5")
    }

    fn attackers(&self) -> bool {
        self.running
            .pointer("/threats/attackers")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty())
    }

    fn value(&self, metric: &str) -> Option<f64> {
        let demand = self
            .f("/actors/vehicles/demand/rate_veh_per_h")
            .unwrap_or(30.0);
        let cbr = (0.3 * demand / 1500.0).min(0.95) * if self.lte() { 0.5 } else { 1.0 };
        Some(match metric {
            "cbr" => cbr,
            "pdr" => 0.95 - cbr * 0.2,
            "e2e_latency.p95" => 8.0,
            "mean_speed" => 9.5,
            "det_recall" if self.attackers() => 0.3,
            "det_fpr" if self.attackers() => 0.0,
            _ => return None,
        })
    }
}

impl RpcTransport for Fake {
    fn endpoint(&self) -> &str {
        "<fake>"
    }

    fn call(&mut self, method: &str, params: &Value) -> Result<Value> {
        self.log
            .lock()
            .unwrap()
            .push((method.to_string(), params.clone()));
        let metric = params
            .pointer("/metrics/0")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let status = |s: &Fake| {
            json!({"run_id": format!("run-{}", s.runs), "t_ns": s.t_ns,
            "t_end_ns": s.t_end_ns, "actors": 12, "nodes": 12, "scenario_hash": "abc",
            "state": if s.t_ns >= s.t_end_ns { "finished" } else if s.paused { "paused" } else { "running" }})
        };
        Ok(match method {
            "scenario.list" => json!({"items": [
                {"id": "scenarios/phase1-grid.yaml", "description": "procedural grid"}]}),
            "scenario.load" => {
                // Loading the running scenario's hash withdraws a staged draft.
                if params["path"] == "running-hash" {
                    self.staged = None;
                }
                json!({"scenario": base_scenario()})
            }
            "scenario.get" => match &self.staged {
                Some(doc) => json!({"scenario": doc, "running_hash": "running-hash",
                                    "staged": {"hash": "staged-hash"}}),
                None => json!({"scenario": self.running, "running_hash": "running-hash",
                               "staged": null}),
            },
            "scenario.set" => {
                self.staged = Some(params["scenario"].clone());
                json!({"hash": "staged-hash", "valid": true})
            }
            "run.pause" => {
                self.paused = true;
                json!({"state": "paused"})
            }
            "run.resume" => {
                self.paused = false;
                json!({"state": "running"})
            }
            "run.speed" => json!({"speed": params["speed"]}),
            "run.start" => {
                self.running = params["scenario"].clone();
                self.runs += 1;
                self.t_ns = 0;
                self.paused = params["paused"].as_bool().unwrap_or(false);
                self.t_end_ns = (self.f("/time/duration_s").unwrap_or(60.0) * 1e9) as u64;
                json!({"run_id": format!("run-{}", self.runs), "state": "running", "t_end_ns": self.t_end_ns})
            }
            "run.step" => {
                let secs = params["count"].as_f64().unwrap_or(1.0);
                self.t_ns = (self.t_ns + (secs * 1e9) as u64).min(self.t_end_ns);
                json!({"t_ns": self.t_ns})
            }
            "run.status" => {
                if !self.paused {
                    self.t_ns = (self.t_ns + 10_000_000_000).min(self.t_end_ns);
                }
                status(self)
            }
            "metrics.query" if params.get("metrics").is_none() => {
                let mut rows = vec![
                    json!({"name": "cbr", "unit": "ratio", "dims": ["t"], "base": "cbr"}),
                    json!({"name": "pdr", "unit": "ratio", "dims": ["t"], "base": "pdr"}),
                    json!({"name": "e2e_latency.p95", "unit": "ms", "dims": ["t"], "base": "e2e_latency"}),
                    json!({"name": "mean_speed", "unit": "m/s", "dims": ["t"], "base": "mean_speed"}),
                ];
                if self.attackers() {
                    rows.push(json!({"name": "det_recall", "unit": "ratio", "dims": ["t"], "base": "det_recall"}));
                    rows.push(json!({"name": "det_fpr", "unit": "ratio", "dims": ["t"], "base": "det_fpr"}));
                }
                json!({"catalogue": rows})
            }
            "metrics.query" if params.get("group_by").is_some() => json!({"rows": []}),
            "metrics.query" => match self.value(&metric) {
                Some(v) => json!({"rows": [[0, (v * 1000.0).round() / 1000.0]]}),
                None => json!({"rows": []}),
            },
            "inspect.node" => json!({"node": params["node"], "kind": "obu",
                "neighbors": (0..20).map(|i| json!({"node": i, "rssi_dbm": -90.123456})).collect::<Vec<_>>()}),
            other => {
                return Err(CopilotError::Rpc {
                    method: other.into(),
                    message: "not in this fake".into(),
                });
            }
        })
    }
}

fn call(id: &str, name: &str, args: Value) -> ToolCall {
    ToolCall {
        id: id.into(),
        name: name.into(),
        arguments: args.to_string(),
    }
}

fn tools(calls: Vec<ToolCall>) -> Completion {
    Completion {
        message: ChatMessage::calls(Some("Plan: one call.".into()), calls),
        finish_reason: "tool_calls".into(),
    }
}

fn say(text: &str) -> Completion {
    Completion {
        message: ChatMessage::assistant(text),
        finish_reason: "stop".into(),
    }
}

fn quick() -> AgentPolicy {
    AgentPolicy {
        poll_ms: 0,
        ..AgentPolicy::default()
    }
}

fn tool_result(provider_seen: &[v2xw_copilot::ChatRequest], call_id: &str) -> Value {
    provider_seen
        .iter()
        .flat_map(|r| r.messages.iter())
        .find(|m| m.tool_call_id.as_deref() == Some(call_id))
        .and_then(|m| serde_json::from_str(m.text()).ok())
        .expect("the tool result reached the model")
}

#[test]
fn one_simulate_call_runs_both_arms_with_one_seed_and_explains_the_difference() {
    let provider = ScriptedProvider::new(vec![
        tools(vec![call(
            "c1",
            "simulate",
            json!({
            "base": "scenarios/phase1-grid.yaml", "reason": "the grid",
            "changes": [{"key": "actors.vehicles.demand.rate_veh_per_h", "value": 6000}],
            "variants": [{"label": "dsrc"},
                         {"label": "lte", "changes": [{"key": "radio.rat", "value": "lte-v2x-pc5"}]}],
            "duration_s": 30}),
        )]),
        say("LTE-V2X halved the busy ratio [channel.congested]."),
    ]);
    let engine = Fake::new();
    let log = Arc::clone(&engine.log);
    let mut agent = Agent::new(Some(provider), engine)
        .expect("builds")
        .with_policy(quick());
    let mut events = Vec::new();
    let report = agent
        .ask("DSRC vs LTE at 6000 veh/h", &mut |e| events.push(e))
        .expect("completes");

    assert!(
        matches!(events.first(), Some(AgentEvent::Status { .. })),
        "something on screen before the model answers"
    );
    let starts: Vec<Value> = log
        .lock()
        .unwrap()
        .iter()
        .filter(|(m, _)| m == "run.start")
        .map(|(_, p)| p.clone())
        .collect();
    assert_eq!(starts.len(), 2, "both arms ran, from one tool call");
    assert_eq!(
        starts[0]["scenario"]["seed"], starts[1]["scenario"]["seed"],
        "one seed"
    );
    assert_eq!(starts[1]["scenario"]["radio"]["rat"], "lte-v2x-pc5");
    assert_eq!(starts[0]["scenario"]["time"]["duration_s"], json!(30.0));
    assert_eq!(
        starts[0]["scenario"]["actors"]["vehicles"]["demand"]["rate_veh_per_h"],
        json!(6000)
    );
    assert_eq!(report.runs.len(), 2);
    assert!(report.comparison.is_some());
    assert!(
        report.markdown.contains("## Experiment"),
        "{}",
        report.markdown
    );

    let seen = &agent_provider_seen(&agent);
    let result = tool_result(seen, "c1");
    let sig = result["result"]["experiment"]["significant"]
        .as_array()
        .expect("significant rows");
    let cbr = sig
        .iter()
        .find(|r| r["metric"] == "cbr")
        .expect("cbr is significant");
    assert_eq!(cbr["values"], json!(["0.95", "0.475"]));
    assert_eq!(cbr["pct"], json!(["-50%"]));
    assert_eq!(cbr["health"], json!(["unhealthy", "healthy"]));
    assert!(
        cbr["why"].as_array().is_some_and(|w| w
            .iter()
            .any(|x| x.as_str().is_some_and(|s| s.starts_with("channel.")))),
        "{cbr}"
    );
    assert!(report.ungrounded.is_empty());
}

/// The provider's requests, through the agent (the scripted provider records them).
fn agent_provider_seen<T: RpcTransport>(
    agent: &Agent<ScriptedProvider, T>,
) -> Vec<v2xw_copilot::ChatRequest> {
    agent.provider().map(|p| p.seen.clone()).unwrap_or_default()
}

#[test]
fn an_invalid_variant_stops_every_arm_before_anything_runs() {
    let provider = ScriptedProvider::new(vec![
        tools(vec![call(
            "c1",
            "simulate",
            json!({
            "base": "scenarios/phase1-grid.yaml", "reason": "r",
            "variants": [{"label": "ok"},
                         {"label": "bad", "changes": [{"key": "radio.rat", "value": "carrier-pigeon"}]}]}),
        )]),
        say("The second variant is not valid."),
    ]);
    let engine = Fake::new();
    let log = Arc::clone(&engine.log);
    let mut agent = Agent::new(Some(provider), engine)
        .expect("builds")
        .with_policy(quick());
    let report = agent.ask("x", &mut |_| {}).expect("completes");
    assert!(report.runs.is_empty());
    assert!(!log.lock().unwrap().iter().any(|(m, _)| m == "run.start"));
    let r = tool_result(&agent_provider_seen(&agent), "c1");
    assert_eq!(r["result"]["ok"], false);
    assert_eq!(r["result"]["invalid"][0]["label"], "bad");
}

#[test]
fn a_run_is_driven_step_by_step_and_read_live_with_health() {
    let provider = ScriptedProvider::new(vec![
        tools(vec![call(
            "c0",
            "prepare_scenario",
            json!({"base": "scenarios/phase1-grid.yaml", "reason": "r",
            "changes": [{"key": "actors.vehicles.demand.rate_veh_per_h", "value": 4500, "why": "w"}]}),
        )]),
        tools(vec![call(
            "c1",
            "run_control",
            json!({"action": "start", "paused": true}),
        )]),
        tools(vec![call(
            "c2",
            "run_control",
            json!({"action": "run_to", "t_s": 30}),
        )]),
        tools(vec![call(
            "c3",
            "run_control",
            json!({"action": "seek", "t_s": 10}),
        )]),
        tools(vec![call(
            "c4",
            "live_metrics",
            json!({"metrics": ["cbr", "pdr", "nope"]}),
        )]),
        tools(vec![call(
            "c5",
            "inspect",
            json!({"target": "node", "node": 3}),
        )]),
        say("At 10 s the busy ratio was 0.9 [cbr]."),
    ]);
    let engine = Fake::new();
    let log = Arc::clone(&engine.log);
    let mut agent = Agent::new(Some(provider), engine)
        .expect("builds")
        .with_policy(quick());
    agent
        .ask(
            "start paused, run to 30 s, go back to 10 s, read the channel",
            &mut |_| {},
        )
        .expect("completes");
    let seen = agent_provider_seen(&agent);
    assert_eq!(tool_result(&seen, "c1")["result"]["state"], "paused");
    assert_eq!(tool_result(&seen, "c2")["result"]["t_s"], "30");
    let back = tool_result(&seen, "c3");
    assert_eq!(
        back["result"]["t_s"], "10",
        "seeking back re-ran from 0 and stepped: {back}"
    );
    let calls = log.lock().unwrap();
    assert_eq!(
        calls.iter().filter(|(m, _)| m == "run.start").count(),
        2,
        "the seek back restarted the run"
    );
    let live = tool_result(&seen, "c4");
    let rows = live["result"]["metrics"].as_array().expect("rows");
    assert_eq!(rows[0]["metric"], "cbr");
    assert_eq!(rows[0]["health"], "unhealthy");
    assert_eq!(live["result"]["not_available"], json!(["nope"]));
    let node = tool_result(&seen, "c5");
    assert_eq!(
        node["result"]["neighbors"].as_array().map(Vec::len),
        Some(9),
        "cut to 8 and a note"
    );
}

#[test]
fn an_attack_run_carries_a_detection_finding_and_a_jammer_lands_in_the_middle() {
    let provider = ScriptedProvider::new(vec![
        tools(vec![call(
            "c1",
            "simulate",
            json!({
            "base": "scenarios/phase1-grid.yaml", "reason": "r", "label": "attack",
            "changes": [
                {"key": "threats.attackers", "value": {"append": {"id": "threat/attacker/legacy/ConstPos", "fraction": 0.1}}},
                {"key": "threats.jammers", "value": {"append": {"id": "attacker/jammer/constant", "params": {"position_m": "centre"}}}}]}),
        )]),
        say("Detection caught few [security.detection]."),
    ]);
    let engine = Fake::new();
    let log = Arc::clone(&engine.log);
    let mut agent = Agent::new(Some(provider), engine)
        .expect("builds")
        .with_policy(quick());
    let report = agent.ask("attack it", &mut |_| {}).expect("completes");
    let f = report.runs[0]
        .analysis
        .finding("security.detection")
        .expect("a detection finding");
    assert!(
        f.evidence
            .iter()
            .any(|e| e.source == "det_recall" && (e.value - 0.3).abs() < 1e-9)
    );
    let start = log
        .lock()
        .unwrap()
        .iter()
        .find(|(m, _)| m == "run.start")
        .cloned()
        .expect("started");
    let pos = &start.1["scenario"]["threats"]["jammers"][0]["params"]["position_m"];
    assert!(pos[0].as_f64().is_some_and(|x| x > 1000.0), "{pos}");
}

#[test]
fn polling_starts_fast_and_settles_at_the_policy_interval() {
    assert_eq!(poll_delay_ms(500, 0), 25);
    assert_eq!(poll_delay_ms(500, 2), 100);
    assert_eq!(poll_delay_ms(500, 10), 500);
    assert_eq!(poll_delay_ms(0, 3), 0);
}

/// The Portland request: one `setup` call stages a draft on Portland (nothing runs), the
/// panel gets its chips, a follow-up setup builds on the draft, and Undo puts back what the
/// Settings window held before.
#[test]
fn setup_stages_a_portland_draft_with_chips_and_undo_restores_the_window() {
    let provider = ScriptedProvider::new(vec![
        tools(vec![call(
            "s1",
            "setup",
            json!({"changes": [
                {"key": "city", "value": "portland"},
                {"key": "radio.rat", "value": "dsrc-80211p"},
                {"key": "threats.attackers", "value": {"append":
                    {"id": "threat/attacker/legacy/ConstPos", "fraction": 0.05}}}]}),
        )]),
        say("Ready in Settings; press Run."),
        tools(vec![call(
            "s2",
            "setup",
            json!({"changes": [{"key": "time.duration_s", "value": 120}]}),
        )]),
        say("Duration set."),
    ]);
    let engine = Fake::new();
    let log = Arc::clone(&engine.log);
    let mut agent = Agent::new(Some(provider), engine)
        .expect("builds")
        .with_policy(quick());
    let mut events = Vec::new();
    agent
        .ask("Set up busy traffic on Portland with DSRC and attackers", &mut |e| {
            events.push(e)
        })
        .expect("completes");

    let calls: Vec<(String, Value)> = log.lock().unwrap().clone();
    assert!(
        !calls.iter().any(|(m, _)| m == "run.start"),
        "a setup never runs anything"
    );
    let set = calls
        .iter()
        .find(|(m, _)| m == "scenario.set")
        .expect("the draft was staged");
    assert_eq!(set.1["scenario"]["city"], "portland");
    assert_eq!(set.1["scenario"]["radio"]["rat"], "dsrc-80211p");
    let chips = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::Staged { setup, chips, .. } => Some((*setup, chips.clone())),
            _ => None,
        })
        .expect("a staged event");
    assert_eq!(chips.0, 1);
    let line: Vec<String> = chips
        .1
        .iter()
        .map(|c| format!("{} → {}", c.label, c.value))
        .collect();
    assert_eq!(line[0], "City → Portland, OR", "{line:?}");
    assert!(line.iter().any(|l| l == "Attackers → 5% Constant position"), "{line:?}");
    assert!(!line.iter().any(|l| l.starts_with("Metrics")), "{line:?}");

    // A follow-up builds on the draft, not on the running scenario.
    agent
        .ask("make it two minutes", &mut |_| {})
        .expect("completes");
    let last_set = log
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|(m, _)| m == "scenario.set")
        .cloned()
        .expect("staged again");
    assert_eq!(last_set.1["scenario"]["city"], "portland", "kept the first setup");
    assert_eq!(last_set.1["scenario"]["time"]["duration_s"], json!(120));

    // Undo of the second setup puts the first one's draft back.
    agent.undo_setup(2).expect("undoes");
    let restored = log
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find(|(m, _)| m == "scenario.set")
        .cloned()
        .expect("restored");
    assert_eq!(restored.1["validate"], json!(false));
    assert_eq!(restored.1["scenario"]["city"], "portland");
    assert_ne!(restored.1["scenario"]["time"]["duration_s"], json!(120));
    // Undo of the first withdraws the draft: the running scenario's hash is loaded.
    agent.undo_setup(1).expect("undoes");
    assert!(
        log.lock()
            .unwrap()
            .iter()
            .any(|(m, p)| m == "scenario.load" && p["path"] == "running-hash")
    );
    assert!(agent.undo_setup(9).is_err(), "no such setup");
}
