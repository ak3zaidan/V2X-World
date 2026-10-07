//! `v2xw agent "<prompt>"` and `v2xw serve`.
//!
//! `v2xw agent` drives the same agent the Studio's panel drives and prints its report as
//! Markdown. With `--engine` it talks to a running engine; without, it starts one in this
//! process on a free loopback port with `--scenario` (the procedural grid by default) and
//! stops it when the report is written. Progress goes to stderr, the report to stdout, so
//! `v2xw agent "…" > report.md` keeps the report clean.
//!
//! `v2xw serve` is the engine server with the agent's endpoints on the same origin
//! ([`crate::agent_host`]), which is what the Studio's agent panel talks to. With
//! `--attach <url>` it serves only the agent's endpoints, in front of an engine that is
//! already running (`v2xw-server`), for the Studio's dev proxy to route `/agent` to.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

use v2xw_copilot::agent::{Agent, AgentEvent, AgentReport};
use v2xw_copilot::analyst::fmt;
use v2xw_copilot::{CurlPost, HttpRpc, LlmProvider, Secret};
use v2xw_server::{LiveOptions, ServerOptions, StubOptions, serve_scenario, serve_stub};

use crate::agent_host::AgentHost;
use crate::error::{CliError, Result};

/// What `v2xw agent` was asked to do.
#[derive(Debug, Clone)]
pub struct AgentOptions {
    /// The request. Empty with `analyse_only`.
    pub prompt: String,
    /// A running engine's origin; `None` starts one here.
    pub engine: Option<String>,
    /// The scenario an in-process engine starts with.
    pub scenario: PathBuf,
    /// Run without a model even when a key is set.
    pub no_model: bool,
    /// Only analyse the run the engine is serving.
    pub analyse_only: bool,
    /// The base a model-less run uses (`current` or a preset id).
    pub base: String,
    /// The engine's bearer token.
    pub token: Option<String>,
    /// Print the report as JSON rather than Markdown.
    pub json: bool,
}

/// What `v2xw serve` was asked to do.
#[derive(Debug, Clone)]
pub struct ServeOptions {
    /// The scenario; `None` serves the synthetic fixture.
    pub scenario: Option<PathBuf>,
    /// An engine to stand in front of, serving only the agent's endpoints.
    pub attach: Option<String>,
    /// Bind address.
    pub host: IpAddr,
    /// Port.
    pub port: u16,
    /// Bearer token (required off loopback).
    pub token: Option<String>,
    /// Start paused.
    pub paused: bool,
    /// Speed, multiple of real time; 0 is unthrottled.
    pub speed: f64,
    /// The recording path.
    pub record: Option<PathBuf>,
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| CliError::Agent(format!("cannot start the async runtime: {e}")))
}

fn progress(e: &AgentEvent) {
    let line = match e {
        AgentEvent::Status { text } => text.clone(),
        AgentEvent::Plan { text } => format!("plan: {}", text.replace('\n', " / ")),
        AgentEvent::Changes { base, changes, valid, errors, .. } => format!(
            "scenario from `{base}`: {} change(s){}{}",
            changes.len(),
            changes
                .iter()
                .map(|c| format!("\n    {} = {} ({})", c.key, c.after, c.why))
                .collect::<String>(),
            if *valid {
                String::new()
            } else {
                format!("\n    INVALID: {}", errors.iter().map(|e| e.message.clone()).collect::<Vec<_>>().join("; "))
            }
        ),
        AgentEvent::RunStarted { label, run_id, t_end_s } => {
            format!("run `{label}` started ({run_id}), {} s to simulate", fmt(*t_end_s))
        }
        AgentEvent::Progress { label, t_s, t_end_s, actors } => {
            format!("  {label}: {} / {} s, {actors} actors", fmt(*t_s), fmt(*t_end_s))
        }
        AgentEvent::Analysed { label, .. } => format!("run `{label}` analysed"),
        AgentEvent::Tool { name, ok } => format!("  tool {name}: {}", if *ok { "ok" } else { "failed" }),
        AgentEvent::Report { .. } => "report ready".to_string(),
        AgentEvent::Error { message } => format!("error: {message}"),
    };
    eprintln!("{line}");
}

/// Runs `v2xw agent`. Returns the report as Markdown (or JSON).
///
/// # Errors
/// When the engine cannot be started or reached, or the agent fails.
pub fn agent(opts: &AgentOptions) -> Result<String> {
    let rt = runtime()?;
    let mut server = None;
    let origin = match &opts.engine {
        Some(url) => url.trim_end_matches('/').to_string(),
        None => {
            eprintln!("starting an engine with {} …", opts.scenario.display());
            let live = LiveOptions {
                paused: true,
                speed: 0.0,
                ..LiveOptions::default()
            };
            let options = ServerOptions {
                bind: SocketAddr::from(([127, 0, 0, 1], 0)),
                ..ServerOptions::default()
            };
            let s = rt
                .block_on(serve_scenario(options, &opts.scenario, live))
                .map_err(|e| CliError::Agent(format!("the engine did not start: {e}")))?;
            let url = s.http_url();
            server = Some(s);
            url
        }
    };
    let mut rpc = HttpRpc::new(CurlPost::new().with_timeout_s(60), &origin);
    if let Some(t) = &opts.token {
        rpc = rpc.with_token(Secret::new(t.as_str()));
    }
    let provider: Option<Box<dyn LlmProvider + Send>> = if opts.no_model || opts.analyse_only {
        None
    } else {
        match v2xw_copilot::claude::provider_from_env() {
            Ok(p) => Some(p),
            Err(why) => {
                eprintln!("{why}");
                None
            }
        }
    };
    let mut agent = Agent::new(provider, rpc).map_err(|e| CliError::Agent(e.to_string()))?;
    let mut sink = |e: AgentEvent| progress(&e);
    let outcome: std::result::Result<AgentReport, String> = if opts.analyse_only {
        agent
            .analyse_current("current run", &mut sink)
            .map(|record| {
                let mut r = AgentReport {
                    prompt: "Analyse the current run".into(),
                    provider: "none".into(),
                    overview: v2xw_copilot::agent::deterministic_summary(Some(&record.analysis)),
                    ungrounded: Vec::new(),
                    runs: vec![record],
                    comparison: None,
                    stopped: false,
                    markdown: String::new(),
                };
                r.markdown = v2xw_copilot::agent::to_markdown(&r);
                r
            })
            .map_err(|e| e.to_string())
    } else if agent.has_model() {
        agent.ask(&opts.prompt, &mut sink).map_err(|e| e.to_string())
    } else {
        agent
            .run_without_model(&opts.prompt, &opts.base, &mut sink)
            .map_err(|e| e.to_string())
    };
    if let Some(s) = server {
        rt.block_on(s.stop());
    }
    let report = outcome.map_err(CliError::Agent)?;
    if opts.json {
        serde_json::to_string_pretty(&report)
            .map(|mut s| {
                s.push('\n');
                s
            })
            .map_err(|source| CliError::Json { what: "agent report", source })
    } else {
        Ok(report.markdown)
    }
}

/// Runs `v2xw serve` until interrupted.
///
/// # Errors
/// When the engine cannot be started or the port bound.
pub fn serve(opts: &ServeOptions) -> Result<String> {
    let rt = runtime()?;
    let host = AgentHost::new(opts.token.clone());
    let bind = SocketAddr::new(opts.host, opts.port);
    if let Some(engine) = &opts.attach {
        host.set_origin(engine.trim_end_matches('/'));
        let router = host.router();
        rt.block_on(async move {
            let listener = tokio::net::TcpListener::bind(bind)
                .await
                .map_err(|e| CliError::Agent(format!("cannot bind {bind}: {e}")))?;
            println!("v2xw agent endpoints on http://{} for the engine at {engine}", bind);
            print_key_line();
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = tokio::signal::ctrl_c().await;
                })
                .await
                .map_err(|e| CliError::Agent(e.to_string()))
        })?;
        return Ok(String::new());
    }
    let options = ServerOptions {
        bind,
        token: opts.token.clone(),
        extension: Some(host.router()),
        ..ServerOptions::default()
    };
    rt.block_on(async {
        let server = match &opts.scenario {
            Some(path) => {
                let live = LiveOptions {
                    paused: opts.paused,
                    speed: opts.speed,
                    recording: opts.record.clone(),
                    ..LiveOptions::default()
                };
                serve_scenario(options, path, live).await
            }
            None => {
                let stub = StubOptions {
                    paused: opts.paused,
                    speed: opts.speed,
                    ..StubOptions::default()
                };
                serve_stub(options, stub).await
            }
        }
        .map_err(|e| CliError::Agent(format!("the engine did not start: {e}")))?;
        // The agent reaches the engine over loopback even when it is bound wider.
        host.set_origin(format!("http://127.0.0.1:{}", server.address().port()));
        println!("v2xw serve listening on {}", server.http_url());
        println!("  rpc      POST {}/rpc", server.http_url());
        println!("  agent    {}/agent/status", server.http_url());
        print_key_line();
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let _ = tokio::signal::ctrl_c().await;
        server.stop().await;
        Ok::<(), CliError>(())
    })?;
    Ok(String::new())
}

fn print_key_line() {
    match v2xw_copilot::claude::key_source() {
        Some(src) => println!("  model    Anthropic key from {src}"),
        None => println!("  model    none: set ANTHROPIC_API_KEY to let the agent plan and write"),
    }
}
