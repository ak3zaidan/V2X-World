//! `agent_eval` — runs the agent's prompt suite against a live model and a live engine, and
//! scores every answer.
//!
//! ```text
//! set -a; . ./.env; set +a          # the keys, never printed
//! cargo run -p v2xw-copilot --example agent_eval -- \
//!     --server-bin target/release/v2xw-server --port 8771 \
//!     --out docs/validation/agent-eval.md --label after
//! ```
//!
//! The provider is the one the harness itself picks (`V2XW_AGENT_PROVIDER`, `OPENAI_MODEL`,
//! the Anthropic fallback): `v2xw_copilot::claude::provider_from_env`. The engine is a
//! release `v2xw-server` this program starts (`--server-bin`) or one already running
//! (`--engine`); with neither, an engine is started in this process, which is a debug build
//! here and much slower.
//!
//! Scores per case (see `evals/prompts.yaml` for the vocabulary): configuration, run
//! completed, numbers grounded in the runs' data, conclusions, time to the first thing the
//! person sees, time to the model's first output, total time, model calls, tokens and cost.
//! Results are written as Markdown (`--out`) and JSON (the same path with `.json`), and a
//! previous JSON (`--compare`) is put beside them as "before".
//!
//! This is a measuring program, so unlike the crate it reads the wall clock: every
//! duration below is a reading of `Instant`, and none of it reaches a run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use v2xw_copilot::agent::{Agent, AgentEvent, AgentPolicy, AgentReport, ungrounded_numbers};
use v2xw_copilot::provider::{ChatRequest, Completion, LlmProvider, Usage};
use v2xw_copilot::{CurlPost, HttpRpc};

// ----------------------------------------------------------------------------- the suite

#[derive(Debug, Clone, Deserialize)]
struct Suite {
    cases: Vec<Case>,
}

#[derive(Debug, Clone, Deserialize)]
struct Case {
    id: String,
    category: String,
    prompt: String,
    #[serde(default)]
    follow_up_of: Option<String>,
    #[serde(default)]
    runs: Option<Range>,
    #[serde(default)]
    compares: bool,
    #[serde(default)]
    differ: Vec<String>,
    #[serde(default)]
    expect: Vec<Check>,
    #[serde(default)]
    mentions: Vec<Vec<Value>>,
    #[serde(default)]
    findings: Vec<String>,
    #[serde(default)]
    asks: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct Range {
    #[serde(default)]
    min: usize,
    #[serde(default = "many")]
    max: usize,
}

fn many() -> usize {
    usize::MAX
}

#[derive(Debug, Clone, Deserialize)]
struct Check {
    key: String,
    #[serde(default)]
    equals: Option<Value>,
    #[serde(default)]
    one_of: Option<Vec<Value>>,
    #[serde(default)]
    approx: Option<f64>,
    #[serde(default)]
    tol: Option<f64>,
    #[serde(default)]
    min: Option<f64>,
    #[serde(default)]
    max: Option<f64>,
    #[serde(default)]
    has_id: Option<String>,
    #[serde(default)]
    has_type: Option<String>,
    #[serde(default)]
    present: Option<bool>,
    #[serde(default)]
    all: bool,
}

impl Check {
    fn describe(&self) -> String {
        let what = if let Some(v) = &self.equals {
            format!("= {v}")
        } else if let Some(v) = &self.one_of {
            format!("in {}", Value::Array(v.clone()))
        } else if let Some(x) = self.approx {
            format!("≈ {x}")
        } else if let Some(s) = &self.has_id {
            format!("has {s}")
        } else if let Some(s) = &self.has_type {
            format!("has event {s}")
        } else if self.present.is_some() {
            "set".to_string()
        } else {
            format!("in [{:?}, {:?}]", self.min, self.max)
        };
        format!("{} {what}{}", self.key, if self.all { " (all runs)" } else { "" })
    }

    fn holds(&self, doc: &Value) -> bool {
        let pointer = format!("/{}", self.key.replace('.', "/"));
        let v = doc.pointer(&pointer).cloned().unwrap_or(Value::Null);
        if let Some(e) = &self.equals {
            return loose_eq(&v, e);
        }
        if let Some(set) = &self.one_of {
            return set.iter().any(|e| loose_eq(&v, e));
        }
        if let Some(x) = self.approx {
            let tol = self.tol.unwrap_or(0.02);
            return v.as_f64().is_some_and(|y| (y - x).abs() <= tol * x.abs().max(1e-9));
        }
        if let Some(id) = &self.has_id {
            return v.as_array().is_some_and(|a| {
                a.iter().any(|item| {
                    item.get("id")
                        .and_then(Value::as_str)
                        .or_else(|| item.as_str())
                        .is_some_and(|s| s.starts_with(id.as_str()))
                })
            });
        }
        if let Some(t) = &self.has_type {
            return v.as_array().is_some_and(|a| {
                a.iter().any(|item| item.get("type").and_then(Value::as_str) == Some(t.as_str()))
            });
        }
        if self.present == Some(true) {
            return match &v {
                Value::Null => false,
                Value::Array(a) => !a.is_empty(),
                Value::Object(o) => !o.is_empty(),
                _ => true,
            };
        }
        let y = v.as_f64();
        self.min.is_none_or(|m| y.is_some_and(|y| y >= m))
            && self.max.is_none_or(|m| y.is_some_and(|y| y <= m))
    }
}

fn loose_eq(v: &Value, e: &Value) -> bool {
    match (v.as_f64(), e.as_f64()) {
        (Some(a), Some(b)) => (a - b).abs() <= 1e-9 * b.abs().max(1.0),
        _ => match (v.as_str(), e.as_str()) {
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
            _ => v == e,
        },
    }
}

// ----------------------------------------------------------------------------- metering

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CallStat {
    seconds: f64,
    usage: Option<Usage>,
}

/// The harness's own provider, timed and counted per call.
struct Metered<P: LlmProvider> {
    inner: P,
    calls: Arc<Mutex<Vec<CallStat>>>,
}

impl<P: LlmProvider> LlmProvider for Metered<P> {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn complete(&mut self, request: &ChatRequest) -> v2xw_copilot::Result<Completion> {
        let t = Instant::now();
        let out = self.inner.complete(request);
        let stat = CallStat {
            seconds: t.elapsed().as_secs_f64(),
            usage: self.inner.last_usage(),
        };
        if let Ok(mut c) = self.calls.lock() {
            c.push(stat);
        }
        out
    }

    fn last_usage(&self) -> Option<Usage> {
        self.inner.last_usage()
    }
}

/// List prices, US dollars per million tokens: (input, cached input, output). OpenAI's
/// cached-input price for the 6.x tiers is not published in a source this program can
/// cite, so cached input is charged at the full input price there (an upper bound).
fn price(provider: &str) -> Option<(f64, f64, f64)> {
    let model = provider.split_once(':').map_or(provider, |(_, m)| m);
    Some(match model {
        m if m.contains("sol") => (2.0, 2.0, 10.0),
        m if m.contains("luna") => (0.10, 0.10, 0.50),
        m if m.contains("astra") => (10.0, 10.0, 50.0),
        "claude-opus-5-5" => (4.0, 0.20, 20.0),
        "claude-sonnet-5-5" => (2.0, 0.20, 10.0),
        "claude-fable-5-1" => (10.0, 0.25, 50.0),
        _ => return None,
    })
}

// ----------------------------------------------------------------------------- results

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CaseResult {
    id: String,
    category: String,
    prompt: String,
    error: Option<String>,
    runs: usize,
    config_ok: bool,
    config_notes: Vec<String>,
    completed: bool,
    grounded: bool,
    ungrounded: Vec<String>,
    agent_flagged: Vec<String>,
    conclusions_ok: bool,
    conclusion_notes: Vec<String>,
    first_event_s: Option<f64>,
    first_model_s: Option<f64>,
    total_s: f64,
    model_calls: usize,
    model_s: f64,
    tool_calls: usize,
    tools: Vec<String>,
    input_tokens: u64,
    cached_tokens: u64,
    output_tokens: u64,
    cost_usd: Option<f64>,
    overview: String,
}

impl CaseResult {
    fn score(&self) -> f64 {
        let parts = [self.config_ok, self.completed, self.grounded, self.conclusions_ok];
        parts.iter().filter(|b| **b).count() as f64 / parts.len() as f64
    }

    fn passed(&self) -> bool {
        self.config_ok && self.completed && self.grounded && self.conclusions_ok
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct SuiteResult {
    label: String,
    provider: String,
    cases: Vec<CaseResult>,
}

// ----------------------------------------------------------------------------- the engine

struct Engine {
    child: Option<Child>,
    origin: String,
    rt: Option<tokio::runtime::Runtime>,
    server: Option<v2xw_server::VwpServer>,
}

impl Drop for Engine {
    fn drop(&mut self) {
        if let Some(c) = &mut self.child {
            let _ = c.kill();
            let _ = c.wait();
        }
        if let (Some(rt), Some(s)) = (self.rt.as_ref(), self.server.take()) {
            rt.block_on(s.stop());
        }
    }
}

fn healthy(origin: &str) -> bool {
    Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", &format!("{origin}/healthz")])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "200")
}

fn start_engine(args: &Args) -> Engine {
    if let Some(url) = &args.engine {
        return Engine { child: None, origin: url.trim_end_matches('/').to_string(), rt: None, server: None };
    }
    let scenario = args.scenario.clone();
    if let Some(bin) = &args.server_bin {
        let origin = format!("http://127.0.0.1:{}", args.port);
        let child = Command::new(bin)
            .args(["--scenario", &scenario.display().to_string(), "--port", &args.port.to_string(),
                   "--speed", "0", "--paused", "--retain-mb", "256", "--quiet"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("cannot start {}: {e}", bin.display()));
        let engine = Engine { child: Some(child), origin: origin.clone(), rt: None, server: None };
        for _ in 0..240 {
            if healthy(&origin) {
                return engine;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        panic!("the engine at {origin} did not come up");
    }
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("runtime");
    let server = rt
        .block_on(v2xw_server::serve_scenario(
            v2xw_server::ServerOptions {
                bind: std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
                ..v2xw_server::ServerOptions::default()
            },
            &scenario,
            v2xw_server::LiveOptions { paused: true, speed: 0.0, ..v2xw_server::LiveOptions::default() },
        ))
        .expect("the engine starts");
    let origin = server.http_url();
    Engine { child: None, origin, rt: Some(rt), server: Some(server) }
}

// ----------------------------------------------------------------------------- arguments

struct Args {
    suite: PathBuf,
    engine: Option<String>,
    server_bin: Option<PathBuf>,
    port: u16,
    scenario: PathBuf,
    out: PathBuf,
    compare: Option<PathBuf>,
    label: String,
    only: Vec<String>,
    category: Vec<String>,
    append: bool,
    transcripts: Option<PathBuf>,
}

fn args() -> Args {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut a = Args {
        suite: Path::new(env!("CARGO_MANIFEST_DIR")).join("evals/prompts.yaml"),
        engine: None,
        server_bin: None,
        port: 8771,
        scenario: root.join("scenarios/phase1-grid.yaml"),
        out: root.join("docs/validation/agent-eval.md"),
        compare: None,
        label: "run".to_string(),
        only: Vec::new(),
        category: Vec::new(),
        append: false,
        transcripts: None,
    };
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let next = argv.get(i + 1).cloned().unwrap_or_default();
        if argv[i] == "--append" {
            a.append = true;
            i += 1;
            continue;
        }
        match argv[i].as_str() {
            "--suite" => a.suite = PathBuf::from(&next),
            "--engine" => a.engine = Some(next),
            "--server-bin" => a.server_bin = Some(PathBuf::from(&next)),
            "--port" => a.port = next.parse().expect("--port is a number"),
            "--scenario" => a.scenario = PathBuf::from(&next),
            "--out" => a.out = PathBuf::from(&next),
            "--compare" => a.compare = Some(PathBuf::from(&next)),
            "--label" => a.label = next,
            "--transcripts" => a.transcripts = Some(PathBuf::from(&next)),
            "--only" => a.only = next.split(',').map(str::to_string).collect(),
            "--category" => a.category = next.split(',').map(str::to_string).collect(),
            other => panic!("unknown argument {other}"),
        }
        i += 2;
    }
    a
}

// ----------------------------------------------------------------------------- scoring

fn documents(report: &AgentReport) -> Vec<Value> {
    report
        .runs
        .iter()
        .filter_map(|r| r.prepared.as_ref().map(|p| p.document.clone()))
        .collect()
}

/// The results of the tools that read the build's own registry (model cards, metric
/// definitions, the settings index): a number quoted from a card is sourced, though it is
/// not run data.
fn registry_results(history: &[v2xw_copilot::ChatMessage]) -> String {
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let mut out = String::new();
    for m in history {
        for c in &m.tool_calls {
            names.insert(c.id.clone(), c.name.clone());
        }
        if let Some(id) = &m.tool_call_id {
            if names.get(id).is_some_and(|n| n.starts_with("registry__") || n == "find_settings") {
                out.push_str(m.text());
                out.push('\n');
            }
        }
    }
    out
}

/// `prompts` is the person's words and the registry sources ([`registry_results`]).
fn score(case: &Case, report: &AgentReport, prompts: &str, r: &mut CaseResult) {
    let docs = documents(report);
    r.runs = report.runs.len();
    let mut notes = Vec::new();
    let range = case.runs.clone().unwrap_or(Range {
        min: if case.asks { 0 } else { 1 },
        max: if case.asks { 0 } else { usize::MAX },
    });
    if r.runs < range.min || r.runs > range.max {
        notes.push(format!(
            "{} run(s), expected {}{}",
            r.runs,
            range.min,
            if range.max == usize::MAX { "+".to_string() } else if range.max == range.min { String::new() } else { format!("–{}", range.max) }
        ));
    }
    for c in &case.expect {
        let ok = if c.all {
            !docs.is_empty() && docs.iter().all(|d| c.holds(d))
        } else {
            docs.iter().any(|d| c.holds(d))
        };
        if !ok {
            notes.push(format!("not met: {}", c.describe()));
        }
    }
    for key in &case.differ {
        let pointer = format!("/{}", key.replace('.', "/"));
        let mut seen: Vec<String> = Vec::new();
        for d in &docs {
            let v = d.pointer(&pointer).map(Value::to_string).unwrap_or_default();
            if !seen.contains(&v) {
                seen.push(v);
            }
        }
        if seen.len() < 2 {
            notes.push(format!("{key} does not differ across the runs"));
        }
    }
    if case.compares && report.comparison.is_none() && report.runs.len() < 2 {
        notes.push("no comparison".to_string());
    }
    r.config_ok = notes.is_empty();
    r.config_notes = notes;

    // Runs the agent started must reach their end; analysing a run the engine was already
    // serving (paused part-way, or never started) is not a run that failed to complete.
    r.completed = r.error.is_none()
        && report
            .runs
            .iter()
            .filter(|run| run.prepared.is_some())
            .all(|run| run.analysis.finding("simulation.incomplete").is_none());

    // Strict grounding: the overview against the report's tables (the overview itself cut
    // out) and the person's own words, not against the agent's conversation.
    let data = report.markdown.replacen(report.overview.trim(), "", 1);
    let corpus = format!("{data}\n{prompts}\n{}", serde_json::to_string(&report.runs).unwrap_or_default());
    r.ungrounded = ungrounded_numbers(&report.overview, &corpus);
    r.grounded = r.ungrounded.is_empty();
    r.agent_flagged = report.ungrounded.clone();

    let text = report.overview.to_lowercase();
    let mut cn = Vec::new();
    for group in &case.mentions {
        let words: Vec<String> = group
            .iter()
            .map(|w| match w {
                Value::String(s) => s.to_lowercase(),
                other => other.to_string(),
            })
            .collect();
        if !words.iter().any(|w| text.contains(w.as_str())) {
            cn.push(format!("mentions none of {words:?}"));
        }
    }
    if !case.findings.is_empty() {
        let found = report
            .runs
            .iter()
            .any(|run| case.findings.iter().any(|id| run.analysis.finding(id).is_some()));
        if !found {
            cn.push(format!("no finding among {:?}", case.findings));
        }
    }
    if case.asks {
        let asks = text.contains('?')
            || ["could you", "do you mean", "which ", "not possible", "cannot", "can't",
                "not supported", "isn't supported", "does not", "doesn't", "no ", "unsupported",
                "not available", "unable"]
                .iter()
                .any(|w| text.contains(w));
        if !asks {
            cn.push("neither asks nor explains".to_string());
        }
        if r.runs > 0 {
            cn.push(format!("started {} run(s) instead of asking", r.runs));
        }
    }
    if report.overview.trim().is_empty() {
        cn.push("empty answer".to_string());
    }
    r.conclusions_ok = cn.is_empty();
    r.conclusion_notes = cn;
}

// ----------------------------------------------------------------------------- report

fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

fn pctl(v: &[f64], p: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let idx = ((s.len() as f64 - 1.0) * p).round() as usize;
    Some(s[idx])
}

fn f(v: Option<f64>, digits: usize) -> String {
    v.map_or_else(|| "—".to_string(), |x| format!("{x:.digits$}"))
}

fn summary(s: &SuiteResult) -> BTreeMap<&'static str, String> {
    let n = s.cases.len().max(1) as f64;
    let rate = |p: &dyn Fn(&CaseResult) -> bool| format!("{:.0}%", 100.0 * s.cases.iter().filter(|c| p(c)).count() as f64 / n);
    let firsts: Vec<f64> = s.cases.iter().filter_map(|c| c.first_event_s).collect();
    let models: Vec<f64> = s.cases.iter().filter_map(|c| c.first_model_s).collect();
    let totals: Vec<f64> = s.cases.iter().map(|c| c.total_s).collect();
    let cost: Option<f64> = s.cases.iter().map(|c| c.cost_usd).sum();
    let mut m = BTreeMap::new();
    m.insert("a cases", format!("{}", s.cases.len()));
    m.insert("b passed (all four)", rate(&|c| c.passed()));
    m.insert("c mean score", format!("{:.2}", s.cases.iter().map(CaseResult::score).sum::<f64>() / n));
    m.insert("d configuration", rate(&|c| c.config_ok));
    m.insert("e run completed", rate(&|c| c.completed));
    m.insert("f numbers grounded", rate(&|c| c.grounded));
    m.insert("g conclusions", rate(&|c| c.conclusions_ok));
    m.insert("h first feedback, median s", f(pctl(&firsts, 0.5), 2));
    m.insert("i first model output, median s", f(pctl(&models, 0.5), 1));
    m.insert("j total, median s", f(pctl(&totals, 0.5), 1));
    m.insert("k total, p90 s", f(pctl(&totals, 0.9), 1));
    m.insert("l model calls, mean", f(mean(&s.cases.iter().map(|c| c.model_calls as f64).collect::<Vec<_>>()), 1));
    m.insert("m tokens in, total", format!("{}", s.cases.iter().map(|c| c.input_tokens).sum::<u64>()));
    m.insert("n tokens cached, total", format!("{}", s.cases.iter().map(|c| c.cached_tokens).sum::<u64>()));
    m.insert("o tokens out, total", format!("{}", s.cases.iter().map(|c| c.output_tokens).sum::<u64>()));
    m.insert("p cost, USD", f(cost, 3));
    m
}

fn markdown(s: &SuiteResult, before: Option<&SuiteResult>) -> String {
    let mut md = String::new();
    md.push_str("# Agent evaluation\n\n");
    md.push_str(&format!(
        "Suite `crates/v2xw-copilot/evals/prompts.yaml`, run by `cargo run -p v2xw-copilot --example agent_eval`. \
         Provider `{}`, label `{}`. Each case is scored on four checks: configuration (the scenario changes \
         and run count the prompt implies), run completed, numbers grounded (every number in the answer is in \
         the runs' data) and conclusions (the answer says what the prompt asked about, or asks / explains when \
         it should). Times are wall-clock seconds from the prompt; \"first feedback\" is the first event the \
         person sees, \"first model output\" the model's first plan, tool call or answer.\n\n",
        s.provider, s.label
    ));
    md.push_str("## Summary\n\n");
    let now = summary(s);
    if let Some(b) = before {
        let then = summary(b);
        md.push_str(&format!("| Measure | Before (`{}`) | After (`{}`) |\n|---|---|---|\n", b.label, s.label));
        for (k, v) in &now {
            md.push_str(&format!("| {} | {} | {} |\n", &k[2..], then.get(k).cloned().unwrap_or_default(), v));
        }
    } else {
        md.push_str("| Measure | Value |\n|---|---|\n");
        for (k, v) in &now {
            md.push_str(&format!("| {} | {} |\n", &k[2..], v));
        }
    }
    md.push_str("\n## By category\n\n| Category | Cases | Passed | Mean score |\n|---|---|---|---|\n");
    let mut cats: BTreeMap<&str, Vec<&CaseResult>> = BTreeMap::new();
    for c in &s.cases {
        cats.entry(c.category.as_str()).or_default().push(c);
    }
    for (cat, cs) in &cats {
        md.push_str(&format!(
            "| {cat} | {} | {} | {:.2} |\n",
            cs.len(),
            cs.iter().filter(|c| c.passed()).count(),
            cs.iter().map(|c| c.score()).sum::<f64>() / cs.len() as f64
        ));
    }
    md.push_str("\n## Cases\n\n| Case | Config | Done | Grounded | Concl. | Runs | Calls | First model s | Total s | Tokens in/out | Cost $ | Notes |\n|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    let mark = |b: bool| if b { "yes" } else { "**no**" };
    for c in &s.cases {
        let mut notes: Vec<String> = Vec::new();
        if let Some(e) = &c.error {
            notes.push(format!("error: {}", e.chars().take(160).collect::<String>()));
        }
        notes.extend(c.config_notes.iter().cloned());
        if !c.ungrounded.is_empty() {
            notes.push(format!("ungrounded {}", c.ungrounded.join(", ")));
        }
        notes.extend(c.conclusion_notes.iter().cloned());
        let before_note = before
            .and_then(|b| b.cases.iter().find(|x| x.id == c.id))
            .map(|x| format!(" (before {:.2})", x.score()))
            .unwrap_or_default();
        md.push_str(&format!(
            "| `{}`{} | {} | {} | {} | {} | {} | {} | {} | {:.1} | {}/{} | {} | {} |\n",
            c.id,
            before_note,
            mark(c.config_ok),
            mark(c.completed),
            mark(c.grounded),
            mark(c.conclusions_ok),
            c.runs,
            c.model_calls,
            f(c.first_model_s, 1),
            c.total_s,
            c.input_tokens,
            c.output_tokens,
            f(c.cost_usd, 4),
            notes.join("; ").replace('|', "/")
        ));
    }
    md.push_str("\n## Answers\n\n");
    for c in &s.cases {
        md.push_str(&format!("### `{}`\n\n> {}\n\n", c.id, c.prompt));
        md.push_str(&format!("Tools: {}\n\n", if c.tools.is_empty() { "none".to_string() } else { c.tools.join(", ") }));
        for line in c.overview.lines() {
            md.push_str(&format!("    {line}\n"));
        }
        md.push('\n');
    }
    md
}

// ----------------------------------------------------------------------------- main

fn main() {
    let args = args();
    let suite: Suite = serde_yml::from_str(&std::fs::read_to_string(&args.suite).expect("the suite reads"))
        .expect("the suite parses");
    let before: Option<SuiteResult> = args
        .compare
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok());
    let engine = start_engine(&args);
    eprintln!("engine at {}", engine.origin);

    let mut agents: BTreeMap<String, (Agent<Metered<Box<dyn LlmProvider + Send>>, HttpRpc<CurlPost>>, Arc<Mutex<Vec<CallStat>>>, String)> = BTreeMap::new();
    let json_path = args.out.with_extension("json");
    // `--append`: keep the cases an earlier invocation wrote to the same file and replace
    // only the ones this one runs, so a suite can be run in parts (the machine's lock on
    // heavy commands is released between them).
    let mut result = match args.append.then(|| std::fs::read_to_string(&json_path).ok()).flatten() {
        Some(text) => serde_json::from_str::<SuiteResult>(&text).unwrap_or_default(),
        None => SuiteResult::default(),
    };
    result.label = args.label.clone();
    if let Some(dir) = args.out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }

    for case in &suite.cases {
        let selected = (args.only.is_empty() || args.only.contains(&case.id)
            || case.follow_up_of.as_ref().is_some_and(|p| args.only.contains(p)))
            && (args.category.is_empty() || args.category.contains(&case.category));
        if !selected {
            continue;
        }
        let (mut agent, calls, mut prompts) = match case.follow_up_of.as_ref().and_then(|p| agents.remove(p)) {
            Some(a) => a,
            None => {
                let provider = v2xw_copilot::claude::provider_from_env().unwrap_or_else(|e| panic!("{e}"));
                result.provider = provider.name().to_string();
                let calls = Arc::new(Mutex::new(Vec::new()));
                let metered = Metered { inner: provider, calls: Arc::clone(&calls) };
                let rpc = HttpRpc::new(CurlPost::new().with_timeout_s(60), &engine.origin);
                let agent = Agent::new(Some(metered), rpc)
                    .expect("the agent builds")
                    .with_policy(AgentPolicy { poll_ms: 100, ..AgentPolicy::default() });
                (agent, calls, String::new())
            }
        };
        let calls_before = calls.lock().map(|c| c.len()).unwrap_or(0);
        prompts.push_str(&case.prompt);
        prompts.push('\n');
        eprintln!("\n=== {} ({})\n> {}", case.id, case.category, case.prompt);
        let t0 = Instant::now();
        let mut first_event: Option<f64> = None;
        let mut first_model: Option<f64> = None;
        let mut tools: Vec<String> = Vec::new();
        let outcome = {
            let mut sink = |e: AgentEvent| {
                let t = t0.elapsed().as_secs_f64();
                first_event.get_or_insert(t);
                match &e {
                    AgentEvent::Plan { text } => {
                        first_model.get_or_insert(t);
                        eprintln!("  [{t:6.1}s] plan: {}", text.replace('\n', " / ").chars().take(200).collect::<String>());
                    }
                    AgentEvent::Tool { name, ok } => {
                        first_model.get_or_insert(t);
                        tools.push(if *ok { name.clone() } else { format!("{name}!") });
                        eprintln!("  [{t:6.1}s] tool {name} {}", if *ok { "ok" } else { "FAILED" });
                    }
                    AgentEvent::Changes { valid, changes, .. } => {
                        first_model.get_or_insert(t);
                        eprintln!("  [{t:6.1}s] changes ({}): {}", if *valid { "valid" } else { "INVALID" },
                            changes.iter().map(|c| format!("{}={}", c.key, c.after)).collect::<Vec<_>>().join(", ").chars().take(300).collect::<String>());
                    }
                    AgentEvent::Report { .. } => {
                        first_model.get_or_insert(t);
                    }
                    AgentEvent::Status { text } => eprintln!("  [{t:6.1}s] {}", text.chars().take(160).collect::<String>()),
                    AgentEvent::RunStarted { label, .. } => eprintln!("  [{t:6.1}s] run {label}"),
                    AgentEvent::Error { message } => eprintln!("  [{t:6.1}s] error {message}"),
                    _ => {}
                }
            };
            agent.ask(&case.prompt, &mut sink)
        };
        let total = t0.elapsed().as_secs_f64();
        let stats: Vec<CallStat> = calls.lock().map(|c| c[calls_before..].to_vec()).unwrap_or_default();
        let usage = stats.iter().filter_map(|s| s.usage).fold(Usage::default(), Usage::plus);
        let provider = agent.provider_name();
        let cost = price(&provider).map(|(pi, pc, po)| {
            let uncached = usage.input_tokens.saturating_sub(usage.cached_input_tokens) as f64;
            (uncached * pi + usage.cached_input_tokens as f64 * pc + usage.output_tokens as f64 * po) / 1e6
        });
        let mut r = CaseResult {
            id: case.id.clone(),
            category: case.category.clone(),
            prompt: case.prompt.clone(),
            first_event_s: first_event,
            first_model_s: first_model,
            total_s: total,
            model_calls: stats.len(),
            model_s: stats.iter().map(|s| s.seconds).sum(),
            tool_calls: tools.len(),
            tools: tools.clone(),
            input_tokens: usage.input_tokens,
            cached_tokens: usage.cached_input_tokens,
            output_tokens: usage.output_tokens,
            cost_usd: cost.filter(|_| stats.iter().all(|s| s.usage.is_some())),
            ..CaseResult::default()
        };
        match outcome {
            Ok(report) => {
                r.overview = report.overview.clone();
                score(case, &report, &format!("{prompts}\n{}", registry_results(agent.history())), &mut r);
            }
            Err(e) => {
                r.error = Some(e.to_string());
                let empty = AgentReport {
                    prompt: case.prompt.clone(),
                    provider: provider.clone(),
                    overview: String::new(),
                    ungrounded: Vec::new(),
                    runs: agent.runs().to_vec(),
                    comparison: None,
                    stopped: false,
                    markdown: String::new(),
                };
                score(case, &empty, &prompts, &mut r);
            }
        }
        eprintln!(
            "  => score {:.2} config {} done {} grounded {} concl {} | {:.1}s, {} calls, {} in / {} out tokens{}",
            r.score(), r.config_ok, r.completed, r.grounded, r.conclusions_ok, r.total_s, r.model_calls,
            r.input_tokens, r.output_tokens,
            if r.config_notes.is_empty() && r.conclusion_notes.is_empty() && r.ungrounded.is_empty() { String::new() }
            else { format!("\n     {:?} {:?} {:?}", r.config_notes, r.conclusion_notes, r.ungrounded) }
        );
        if let Some(dir) = &args.transcripts {
            let _ = std::fs::create_dir_all(dir);
            let t: Vec<Value> = agent
                .history()
                .iter()
                .map(|m| json!({"role": m.role, "text": m.content, "calls": m.tool_calls}))
                .collect();
            let _ = std::fs::write(dir.join(format!("{}.json", case.id)), serde_json::to_string_pretty(&t).unwrap_or_default());
        }
        result.cases.retain(|c| c.id != r.id);
        result.cases.push(r);
        let order: Vec<&str> = suite.cases.iter().map(|c| c.id.as_str()).collect();
        result.cases.sort_by_key(|c| order.iter().position(|id| *id == c.id).unwrap_or(usize::MAX));
        agents.insert(case.id.clone(), (agent, calls, prompts));
        let _ = std::fs::write(&json_path, serde_json::to_string_pretty(&result).unwrap_or_default());
        let _ = std::fs::write(&args.out, markdown(&result, before.as_ref()));
    }
    drop(engine);
    let s = summary(&result);
    eprintln!("\n{}", json!(s));
    println!("wrote {} and {}", args.out.display(), json_path.display());
}
