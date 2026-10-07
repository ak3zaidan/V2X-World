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
use v2xw_copilot::{CurlPost, HttpRpc, LlmProvider, Secret};

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
            .with_state(self.clone())
    }

    /// The agent, built on first use so the engine's origin is known.
    fn with_agent<R>(&self, f: impl FnOnce(&mut HostAgent) -> R) -> Result<R, String> {
        let mut slot = self.agent.lock().map_err(|_| "the agent's lock is poisoned".to_string())?;
        if slot.is_none() {
            let origin = self
                .origin
                .get()
                .cloned()
                .ok_or_else(|| "the engine's address is not known yet".to_string())?;
            let mut rpc = HttpRpc::new(CurlPost::new().with_timeout_s(60), &origin);
            if let Some(t) = &self.token {
                rpc = rpc.with_token(Secret::new(t.as_str()));
            }
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

    fn authorized(&self, headers: &HeaderMap) -> bool {
        let Some(expected) = &self.token else { return true };
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
        work: impl FnOnce(&mut HostAgent, &mut dyn FnMut(AgentEvent)) -> Result<(), String> + Send + 'static,
    ) -> Response {
        let id = {
            let Ok(mut job) = self.job.lock() else {
                return reply(StatusCode::INTERNAL_SERVER_ERROR, json!({"error": "lock poisoned"}));
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
                let mut sink = |e: AgentEvent| push(serde_json::to_value(&e).unwrap_or(Value::Null));
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
    h.insert("cross-origin-opener-policy", HeaderValue::from_static("same-origin"));
    h.insert("cross-origin-embedder-policy", HeaderValue::from_static("require-corp"));
    h.insert("cross-origin-resource-policy", HeaderValue::from_static("same-origin"));
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
    let built = if busy { Ok(()) } else { host.with_agent(|_| ()) };
    let (model, provider, runs) = host
        .info
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

async fn ask(State(host): State<AgentHost>, headers: HeaderMap, Json(body): Json<AskBody>) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    let prompt = body.prompt.trim().to_string();
    if prompt.is_empty() {
        return reply(StatusCode::BAD_REQUEST, json!({"error": "the prompt is empty"}));
    }
    host.start("ask", move |agent, sink| {
        if !agent.has_model() {
            return Err(v2xw_copilot::claude::HOW_TO_ADD_A_KEY.to_string());
        }
        agent.ask(&prompt, sink).map(|_| ()).map_err(|e| e.to_string())
    })
}

#[derive(Debug, Deserialize, Default)]
struct RunBody {
    #[serde(default)]
    base: Option<String>,
}

async fn run(State(host): State<AgentHost>, headers: HeaderMap, body: Option<Json<RunBody>>) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    let base = body.and_then(|b| b.0.base).unwrap_or_else(|| "current".to_string());
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
        let record = agent.analyse_current("current run", sink).map_err(|e| e.to_string())?;
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
        sink(AgentEvent::Report { report: Box::new(report) });
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

async fn events(State(host): State<AgentHost>, headers: HeaderMap, Query(q): Query<EventsQuery>) -> Response {
    if !host.authorized(&headers) {
        return unauthorized();
    }
    let Ok(job) = host.job.lock() else {
        return reply(StatusCode::INTERNAL_SERVER_ERROR, json!({"error": "lock poisoned"}));
    };
    if q.job.is_some_and(|id| id != job.id) {
        return reply(StatusCode::GONE, json!({"error": "that job was replaced", "job": job.id}));
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
        return reply(StatusCode::CONFLICT, json!({"error": "stop the running request first"}));
    }
    let _ = host.with_agent(HostAgent::reset);
    reply(StatusCode::OK, json!({"reset": true}))
}
