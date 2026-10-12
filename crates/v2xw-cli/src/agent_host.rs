//! The agent's HTTP endpoints, served beside the engine's (`v2xw serve`) or in front of a
//! running engine (`v2xw serve --attach`).
//!
//! | Endpoint | What it does |
//! |---|---|
//! | `GET  /agent/status` | whether a model is attached, which one, where its key comes from (named, never shown), and how to add one |
//! | `POST /agent/ask {prompt}` | starts the agent on a prompt; follow-ups continue the same conversation |
//! | `POST /agent/run {base}` | runs a shipped scenario and analyses it, no model needed |
//! | `POST /agent/analyse` | analyses the run on screen, no model needed |
//! | `GET  /agent/events?job=&after=` | the job's events from `after` on: plan, changes, progress, analysis, report |
//! | `POST /agent/stop` | stops the job at its next check, pausing a run it started |
//! | `POST /agent/reset` | forgets the conversation |
//! | `GET  /agent/watch?after=` | the always-on watcher: what it noticed about the runs since `after`, and where it is |
//! | `GET  /agent/summary` | the last finished run's summary: verdict, layers, problems, overview, analysis |
//! | `POST /agent/compare-signatures {document?, scenario?, schemes?, duration_s?}` | runs one scenario under several signature schemes on a thread (`crate::sigcompare`) |
//! | `GET  /agent/compare-signatures` | where that comparison is, and its result once finished |
//!
//! # Always on
//!
//! [`AgentHost::start_watcher`] starts a thread that follows every run the engine serves,
//! with no request from anyone: it reads `run.status` once a second and, while a run
//! moves, the run's evidence every few seconds, and hands both to
//! [`v2xw_copilot::watch::Watcher`], which says in plain words when something notable
//! happens and writes the run's summary when it ends. With a model key the summary's
//! overview is the model's, checked number by number against the analysis; without one
//! it is the analyst's, and the status says how to add a key. The watcher only reads; it
//! never pauses, steps or changes the run, and the run never waits for it.
//!
//! The agent runs on a thread of its own and reaches the engine the way every other
//! client does, JSON-RPC over HTTP to the engine's origin, so it has exactly the authority
//! of a person in the Studio. The API key lives in this process (`ANTHROPIC_API_KEY` or a
//! key file outside the repository, read by `v2xw_copilot::claude`); nothing here puts it
//! in a response, and the events carry the agent's words and the analyst's numbers only.
//!
//! The polling design is deliberate: an `EventSource` would need a second long-lived
//! connection through the Studio's dev proxy and the engine's isolation headers, and a
//! job's events are few (a progress line a second at most), so a poll every half second
//! costs nothing and survives a reload — the page reads the job back from event 0.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use axum::Json;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::{Value, json};
use v2xw_copilot::agent::{Agent, AgentEvent};
use v2xw_copilot::watch::{Next, WatchCadence, Watcher, model_overview, summary_markdown};
use v2xw_copilot::{CurlPost, HttpRpc, LlmProvider, RpcTransport, Secret};

type HostAgent = Agent<Box<dyn LlmProvider + Send>, HttpRpc<CurlPost>>;

/// One job's record.
#[derive(Debug, Default)]
struct Job {
    id: u64,
    kind: String,
    events: Vec<Value>,
    running: bool,
}

/// The host's state.
#[derive(Clone)]
pub struct AgentHost {
    origin: Arc<OnceLock<String>>,
    token: Option<Arc<String>>,
    agent: Arc<Mutex<Option<HostAgent>>>,
    cancel: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    job: Arc<Mutex<Job>>,
    provider_error: Arc<Mutex<Option<String>>>,
    /// `(model attached, provider name, runs)`, refreshed after every job, so a status
    /// request never waits on the agent while a job holds it.
    info: Arc<Mutex<(bool, String, usize)>>,
    /// The always-on watcher's state.
    watch: Arc<Mutex<WatchState>>,
    watching: Arc<AtomicBool>,
}

/// The watcher and what the host knows around it.
#[derive(Debug, Default)]
struct WatchState {
    watcher: Watcher,
    /// The engine could not be read on the last try.
    unreachable: Option<String>,
    /// The model is writing the last summary's overview.
    overview_pending: bool,
}

impl AgentHost {
    /// A host whose engine origin is set later, once the engine has bound its port.
    #[must_use]
    pub fn new(token: Option<String>) -> Self {
        AgentHost {
            origin: Arc::new(OnceLock::new()),
            token: token.map(Arc::new),
            agent: Arc::new(Mutex::new(None)),
            cancel: Arc::new(Mutex::new(None)),
            job: Arc::new(Mutex::new(Job::default())),
            provider_error: Arc::new(Mutex::new(None)),
            info: Arc::new(Mutex::new((false, "none".to_string(), 0))),
            watch: Arc::new(Mutex::new(WatchState::default())),
            watching: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The engine transport, as every client of the engine gets it.
    fn rpc(&self, origin: &str) -> HttpRpc<CurlPost> {
        let mut rpc = HttpRpc::new(CurlPost::new().with_timeout_s(60), origin);
        if let Some(t) = &self.token {
            rpc = rpc.with_token(Secret::new(t.as_str()));
        }
        rpc
    }

    /// Starts the always-on watcher, once. It waits for the engine's origin, then follows
    /// every run until the process ends.
    pub fn start_watcher(&self, cadence: WatchCadence) {
        if self.watching.swap(true, Ordering::SeqCst) {
            return;
        }
        let host = self.clone();
        std::thread::spawn(move || host.watch_loop(cadence));
    }

    fn watch_loop(&self, cadence: WatchCadence) {
        let pause = std::time::Duration::from_millis(cadence.status_ms.max(100));
        let origin = loop {
            if let Some(o) = self.origin.get() {
                break o.clone();
            }
            std::thread::sleep(pause);
        };
        let mut rpc = self.rpc(&origin);
        // Evidence is read every `every` status reads while a run moves; counting reads
        // keeps the host off a wall clock it does not need.
        let every = (cadence.gather_ms / cadence.status_ms.max(1)).max(1);
        let mut since_gather = every;
        loop {
            std::thread::sleep(pause);
            let status = match rpc.call("run.status", &json!({})) {
                Ok(s) => s,
                Err(e) => {
                    if let Ok(mut w) = self.watch.lock() {
                        w.unreachable = Some(e.to_string());
                    }
                    continue;
                }
            };
            let next = match self.watch.lock() {
                Ok(mut w) => {
                    w.unreachable = None;
                    w.watcher.on_status(&status)
                }
                Err(_) => return,
            };
            match next {
                Next::Idle => since_gather = every,
                Next::Gather => {
                    since_gather += 1;
                    if since_gather < every {
                        continue;
                    }
                    since_gather = 0;
                    if let Ok(evidence) = v2xw_copilot::gather::gather(&mut rpc) {
                        if let Ok(mut w) = self.watch.lock() {
                            let _ = w.watcher.on_evidence(&evidence);
                        }
                    }
                }
                Next::Finish => {
                    since_gather = every;
                    let Ok(evidence) = v2xw_copilot::gather::gather(&mut rpc) else {
                        continue;
                    };
                    let summary = match self.watch.lock() {
                        Ok(mut w) => {
                            let s = w.watcher.finish(&evidence);
                            w.overview_pending = v2xw_copilot::claude::key_source().is_some();
                            s
                        }
                        Err(_) => return,
                    };
                    self.write_overview(&summary);
                }
            }
        }
    }

    /// The model's overview of a finished run, when a model is attached. Runs on the
    /// watcher's thread, after the analyst's summary is already readable.
    fn write_overview(&self, summary: &v2xw_copilot::RunSummary) {
        let (overview, by, note) = match v2xw_copilot::claude::provider_from_env() {
            Err(_) => (String::new(), String::new(), None),
            Ok(mut provider) => {
                let name = provider.name().to_string();
                match model_overview(&mut provider, summary) {
                    Ok(Ok(text)) => (text, name, None),
                    Ok(Err(why)) => (String::new(), String::new(), Some(why)),
                    Err(e) => (
                        String::new(),
                        String::new(),
                        Some(format!(
                            "the model could not write the overview ({e}); the analyst's is shown"
                        )),
                    ),
                }
            }
        };
        if let Ok(mut w) = self.watch.lock() {
            w.watcher.set_overview(&summary.run_id, overview, by, note);
            w.overview_pending = false;
        }
    }

    /// Points the agent at the engine, e.g. `http://127.0.0.1:8787`. Only the first call
    /// counts.
    pub fn set_origin(&self, origin: impl Into<String>) {
        let _ = self.origin.set(origin.into());
    }

    /// The routes.
    #[must_use]
    pub fn router(&self) -> Router {
        Router::new()
            .route("/agent/status", get(status))
            .route("/agent/ask", post(ask))
            .route("/agent/run", post(run))
            .route("/agent/analyse", post(analyse))
            .route("/agent/events", get(events))
            .route("/agent/stop", post(stop))
            .route("/agent/reset", post(reset))
            .route("/agent/undo", post(undo))
            .route("/agent/watch", get(watch))
            .route("/agent/summary", get(summary))
            .route(
                "/agent/compare-signatures",
                get(compare_status).post(compare_start),
            )
            .with_state(self.clone())
    }

    /// The one route an engine started with `--no-agent` serves: `GET /agent/status` answers
    /// 200 `{"available": false, ...}`, so the Studio learns once that there is no agent and
    /// stops asking, instead of polling a 404 (which every browser logs as a console error).
    #[must_use]
    pub fn off_router() -> Router {
        const REASON: &str = "The engine was started with --no-agent, so it serves no agent \
            (`v2xw-server` and `v2xw serve` serve one by default). Restart it without --no-agent \
            (./run-sim.sh <scenario>) and reload this page.";
        Router::new()
            .route(
                "/agent/status",
                get(|| async {
                    reply(
                        StatusCode::OK,
                        json!({ "available": false, "reason": REASON }),
                    )
                }),
            )
            // The Studio's signature-comparison pane reads any job already running on mount.
            .route(
                "/agent/compare-signatures",
                get(|| async { reply(StatusCode::OK, json!({ "job": null })) }).post(|| async {
                    reply(StatusCode::SERVICE_UNAVAILABLE, json!({ "error": REASON }))
                }),
            )
    }

    /// The agent, built on first use so the engine's origin is known.
    fn with_agent<R>(&self, f: impl FnOnce(&mut HostAgent) -> R) -> Result<R, String> {
        let mut slot = self
            .agent
            .lock()
            .map_err(|_| "the agent's lock is poisoned".to_string())?;
        if slot.is_none() {
            let origin = self
                .origin
                .get()
                .cloned()
                .ok_or_else(|| "the engine's address is not known yet".to_string())?;
            let rpc = self.rpc(&origin);
            let provider = match v2xw_copilot::claude::provider_from_env() {
                Ok(p) => Some(p),
                Err(why) => {
                    if let Ok(mut e) = self.provider_error.lock() {
                        *e = Some(why);
                    }
                    None
                }
            };
            let agent = Agent::new(provider, rpc).map_err(|e| e.to_string())?;
            *slot = Some(agent);
        }
        let agent = slot.as_mut().expect("built above");
        let out = f(agent);
        if let Ok(mut i) = self.info.lock() {
            *i = (agent.has_model(), agent.provider_name(), agent.runs().len());
        }
        Ok(out)
    }

    /// The watcher's warnings and criticals about the run it follows, one line each, or
    /// `None` when it flagged nothing.
    fn alerts_note(&self) -> Option<String> {
        let w = self.watch.lock().ok()?;
        let (run, _) = w.watcher.run()?;
        let lines: Vec<String> = w
            .watcher
            .feed()
            .iter()
            .filter(|o| o.run_id == run && o.severity >= v2xw_copilot::Severity::Warning)
            .rev()
            .take(8)
            .map(|o| format!("- t={} s: {}", v2xw_copilot::analyst::fmt(o.t_s), o.text))
            .collect();
        if lines.is_empty() {
            return None;
        }
        Some(format!(
            "The watcher flagged these in run {run} (newest first):\n{}",
            lines.join("\n")
        ))
    }

    fn authorized(&self, headers: &HeaderMap) -> bool {
        let Some(expected) = &self.token else {
            return true;
        };
        headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            == Some(expected.as_str())
    }

    /// Starts a job on a thread, unless one is running.
    fn start(
        &self,
        kind: &str,
        work: impl FnOnce(&mut HostAgent, &mut dyn FnMut(AgentEvent)) -> Result<(), String>
        + Send
        + 'static,
    ) -> Response {
        let id = {
            let Ok(mut job) = self.job.lock() else {
                return reply(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({"error": "lock poisoned"}),
                );
            };
            if job.running {
                return reply(
                    StatusCode::CONFLICT,
                    json!({"error": "the agent is busy with another request; stop it or wait",
                           "job": job.id}),
                );
            }
            job.id += 1;
            job.kind = kind.to_string();
            job.events.clear();
            job.running = true;
            job.id
        };
        let host = self.clone();
        std::thread::spawn(move || {
            let jobs = Arc::clone(&host.job);
            let push = move |v: Value| {
                if let Ok(mut j) = jobs.lock() {
                    j.events.push(v);
                }
            };
            let outcome = host.with_agent(|agent| {
                if let Ok(mut c) = host.cancel.lock() {
                    *c = Some(agent.cancel_handle());
                }
                let mut sink =
                    |e: AgentEvent| push(serde_json::to_value(&e).unwrap_or(Value::Null));
                work(agent, &mut sink)
            });
            let error = match outcome {
                Ok(Ok(())) => None,
                Ok(Err(e)) | Err(e) => Some(e),
            };
            if let Ok(mut j) = host.job.lock() {
                if let Some(message) = error {
                    j.events.push(json!({"kind": "error", "message": message}));
                }
                j.events.push(json!({"kind": "done"}));
                j.running = false;
            }
        });
        reply(StatusCode::ACCEPTED, json!({"job": id}))
    }
}

/// The isolation headers every engine response carries (vwp-v1 §1.1), so the Studio's
/// cross-origin-isolated page can read these replies too.
fn reply(status: StatusCode, body: Value) -> Response {
    let mut r = (status, Json(body)).into_response();
    let h = r.headers_mut();
    h.insert(
        "cross-origin-opener-policy",
        HeaderValue::from_static("same-origin"),
    );
    h.insert(
        "cross-origin-embedder-policy",
        HeaderValue::from_static("require-corp"),
    );
    h.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r
}

fn unauthorized() -> Response {
    reply(StatusCode::UNAUTHORIZED, json!({"error": "unauthorized"}))
}

async fn status(State(host): State<AgentHost>, headers: HeaderMap) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    let busy = host.job.lock().map(|j| j.running).unwrap_or(true);
    // While a job runs it holds the agent; the cached line answers instead of waiting.
    let built = if busy {
        Ok(())
    } else {
        host.with_agent(|_| ())
    };
    let (model, provider, runs) =
        host.info
            .lock()
            .map(|i| i.clone())
            .unwrap_or((false, "none".into(), 0));
    let why = host.provider_error.lock().ok().and_then(|e| e.clone());
    let (job, running, kind) = host
        .job
        .lock()
        .map(|j| (j.id, j.running, j.kind.clone()))
        .unwrap_or((0, false, String::new()));
    reply(
        StatusCode::OK,
        json!({
            "available": true,
            "model": model,
            "provider": provider,
            "key_source": v2xw_copilot::claude::key_source(),
            "how_to_add_key": if model { Value::Null } else {
                json!(why.unwrap_or_else(|| v2xw_copilot::claude::HOW_TO_ADD_A_KEY.to_string()))
            },
            "runs": runs,
            "job": job,
            "running": running,
            "kind": kind,
            "engine": host.origin.get(),
            "error": built.err(),
        }),
    )
}

#[derive(Debug, Deserialize)]
struct AskBody {
    prompt: String,
}

async fn ask(
    State(host): State<AgentHost>,
    headers: HeaderMap,
    Json(body): Json<AskBody>,
) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    let prompt = body.prompt.trim().to_string();
    if prompt.is_empty() {
        return reply(
            StatusCode::BAD_REQUEST,
            json!({"error": "the prompt is empty"}),
        );
    }
    // What the watcher flagged in the run on screen, so a question about it is answered
    // with the alerts the person saw beside the chat.
    let alerts = host.alerts_note();
    host.start("ask", move |agent, sink| {
        if !agent.has_model() {
            return Err(v2xw_copilot::claude::HOW_TO_ADD_A_KEY.to_string());
        }
        if let Some(n) = alerts {
            agent.note(n);
        }
        agent
            .ask(&prompt, sink)
            .map(|_| ())
            .map_err(|e| e.to_string())
    })
}

#[derive(Debug, Deserialize, Default)]
struct RunBody {
    #[serde(default)]
    base: Option<String>,
}

async fn run(
    State(host): State<AgentHost>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    // An empty body means the current scenario.
    let base = serde_json::from_slice::<RunBody>(&body)
        .ok()
        .and_then(|b| b.base)
        .filter(|b| !b.trim().is_empty())
        .unwrap_or_else(|| "current".to_string());
    host.start("run", move |agent, sink| {
        agent
            .run_without_model(&format!("Run `{base}` and analyse it"), &base, sink)
            .map(|_| ())
            .map_err(|e| e.to_string())
    })
}

async fn analyse(State(host): State<AgentHost>, headers: HeaderMap) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    host.start("analyse", move |agent, sink| {
        let record = agent
            .analyse_current("current run", sink)
            .map_err(|e| e.to_string())?;
        let summary = v2xw_copilot::agent::deterministic_summary(Some(&record.analysis));
        let mut report = v2xw_copilot::agent::AgentReport {
            prompt: "Analyse the current run".into(),
            provider: "none".into(),
            overview: summary,
            ungrounded: Vec::new(),
            runs: vec![record],
            comparison: None,
            stopped: false,
            markdown: String::new(),
        };
        report.markdown = v2xw_copilot::agent::to_markdown(&report);
        sink(AgentEvent::Report {
            report: Box::new(report),
        });
        Ok(())
    })
}

#[derive(Debug, Deserialize)]
struct EventsQuery {
    #[serde(default)]
    job: Option<u64>,
    #[serde(default)]
    after: Option<usize>,
}

async fn events(
    State(host): State<AgentHost>,
    headers: HeaderMap,
    Query(q): Query<EventsQuery>,
) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    let Ok(job) = host.job.lock() else {
        return reply(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": "lock poisoned"}),
        );
    };
    if q.job.is_some_and(|id| id != job.id) {
        return reply(
            StatusCode::GONE,
            json!({"error": "that job was replaced", "job": job.id}),
        );
    }
    let after = q.after.unwrap_or(0).min(job.events.len());
    reply(
        StatusCode::OK,
        json!({
            "job": job.id,
            "kind": job.kind,
            "events": &job.events[after..],
            "next": job.events.len(),
            "running": job.running,
        }),
    )
}

async fn stop(State(host): State<AgentHost>, headers: HeaderMap) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    let flagged = host
        .cancel
        .lock()
        .ok()
        .and_then(|c| c.clone())
        .map(|c| c.store(true, Ordering::SeqCst))
        .is_some();
    reply(StatusCode::OK, json!({"stopping": flagged}))
}

async fn reset(State(host): State<AgentHost>, headers: HeaderMap) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    if host.job.lock().map(|j| j.running).unwrap_or(true) {
        return reply(
            StatusCode::CONFLICT,
            json!({"error": "stop the running request first"}),
        );
    }
    let _ = host.with_agent(HostAgent::reset);
    reply(StatusCode::OK, json!({"reset": true}))
}

#[derive(Debug, Deserialize)]
struct UndoBody {
    setup: usize,
}

/// `POST /agent/undo {setup}`: the Settings window gets back what it held before that
/// setup.
async fn undo(
    State(host): State<AgentHost>,
    headers: HeaderMap,
    Json(body): Json<UndoBody>,
) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    if host.job.lock().map(|j| j.running).unwrap_or(true) {
        return reply(
            StatusCode::CONFLICT,
            json!({"error": "the agent is busy; stop it first"}),
        );
    }
    // The restore is an RPC to this same server: off the async workers.
    let h = host.clone();
    let out = tokio::task::spawn_blocking(move || {
        h.with_agent(|a| a.undo_setup(body.setup).map_err(|e| e.to_string()))
    })
    .await;
    match out {
        Ok(Ok(Ok(()))) => reply(StatusCode::OK, json!({"undone": body.setup})),
        Ok(Ok(Err(e)) | Err(e)) => reply(StatusCode::CONFLICT, json!({"error": e})),
        Err(e) => reply(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": e.to_string()}),
        ),
    }
}

#[derive(Debug, Deserialize)]
struct WatchQuery {
    #[serde(default)]
    after: Option<usize>,
}

async fn watch(
    State(host): State<AgentHost>,
    headers: HeaderMap,
    Query(q): Query<WatchQuery>,
) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    let Ok(w) = host.watch.lock() else {
        return reply(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": "lock poisoned"}),
        );
    };
    let feed = w.watcher.feed();
    let after = q.after.unwrap_or(0).min(feed.len());
    let (t_s, t_end_s) = w.watcher.progress();
    let model = v2xw_copilot::claude::key_source().is_some();
    reply(
        StatusCode::OK,
        json!({
            "watching": host.watching.load(Ordering::SeqCst),
            "phase": w.watcher.phase(),
            "run_id": w.watcher.run().map(|r| r.0),
            "generation": w.watcher.run().map(|r| r.1),
            "t_s": t_s,
            "t_end_s": t_end_s,
            "observations": &feed[after..],
            "next": feed.len(),
            "summary_run_id": w.watcher.summary().map(|s| s.run_id.clone()),
            "summary_generation": w.watcher.summary().map(|s| s.generation),
            "overview_pending": w.overview_pending,
            "model": model,
            "how_to_add_key": if model { Value::Null } else {
                json!(v2xw_copilot::claude::HOW_TO_ADD_A_KEY)
            },
            "unreachable": w.unreachable,
        }),
    )
}

async fn summary(State(host): State<AgentHost>, headers: HeaderMap) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    let Ok(w) = host.watch.lock() else {
        return reply(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": "lock poisoned"}),
        );
    };
    match w.watcher.summary() {
        Some(s) => reply(
            StatusCode::OK,
            json!({"summary": s, "markdown": summary_markdown(s),
                   "overview_pending": w.overview_pending}),
        ),
        None => reply(
            StatusCode::OK,
            json!({"summary": Value::Null, "phase": w.watcher.phase(),
                   "note": "no run has finished since the engine started"}),
        ),
    }
}

async fn compare_start(
    State(host): State<AgentHost>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    let req =
        serde_json::from_slice::<crate::sigcompare::CompareRequest>(&body).unwrap_or_default();
    match tokio::task::spawn_blocking(move || crate::sigcompare::start(req)).await {
        Ok(Ok(job)) => reply(StatusCode::ACCEPTED, json!(job)),
        Ok(Err(e)) => reply(StatusCode::CONFLICT, json!({"error": e})),
        Err(e) => reply(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": e.to_string()}),
        ),
    }
}

async fn compare_status(State(host): State<AgentHost>, headers: HeaderMap) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    reply(StatusCode::OK, json!({"job": crate::sigcompare::job()}))
}
