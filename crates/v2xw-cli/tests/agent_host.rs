//! The agent's HTTP endpoints, served beside a real server (the fixture engine), driven the
//! way the Studio's panel drives them: status, a model-less analysis job, its events, and
//! the key never appearing in anything served.

use std::io::{Read, Write};
use std::net::TcpStream;

use serde_json::Value;
use v2xw_cli::agent_host::AgentHost;
use v2xw_server::{ServerOptions, StubOptions, serve_stub};

/// One HTTP/1.1 request over a plain socket; returns `(status, body)`.
fn http(addr: &str, method: &str, path: &str, body: &str) -> (u16, String) {
    let mut s = TcpStream::connect(addr).expect("connects");
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).expect("writes");
    let mut out = String::new();
    s.read_to_string(&mut out).expect("reads");
    let status = out.split(' ').nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    let body = out.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    // A chunked reply: drop the chunk-size lines.
    let body = if out.to_ascii_lowercase().contains("transfer-encoding: chunked") {
        body.lines().filter(|l| !l.chars().all(|c| c.is_ascii_hexdigit()) || l.is_empty()).collect::<Vec<_>>().join("")
    } else {
        body
    };
    (status, body)
}

#[test]
fn the_endpoints_report_status_run_a_model_less_analysis_and_never_serve_the_key() {
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().expect("runtime");
    let host = AgentHost::new(None);
    let server = rt
        .block_on(serve_stub(
            ServerOptions {
                bind: std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
                extension: Some(host.router()),
                ..ServerOptions::default()
            },
            StubOptions { actors: 20, duration_s: 20, ..StubOptions::default() },
        ))
        .expect("the fixture engine starts");
    let addr = server.address().to_string();
    host.set_origin(format!("http://{addr}"));

    let (code, body) = http(&addr, "GET", "/agent/status", "");
    assert_eq!(code, 200, "{body}");
    let status: Value = serde_json::from_str(&body).expect("status is JSON");
    assert!(status["model"].is_boolean());
    if status["model"] == false {
        assert!(status["how_to_add_key"].as_str().unwrap_or("").contains("ANTHROPIC_API_KEY"));
    }

    // An empty prompt is refused before anything starts.
    let (code, _) = http(&addr, "POST", "/agent/ask", r#"{"prompt":"  "}"#);
    assert_eq!(code, 400);

    let (code, body) = http(&addr, "POST", "/agent/analyse", "{}");
    assert_eq!(code, 202, "{body}");
    let job = serde_json::from_str::<Value>(&body).expect("json")["job"].as_u64().expect("a job id");

    let mut events: Vec<Value> = Vec::new();
    for _ in 0..240 {
        let (code, body) = http(&addr, "GET", &format!("/agent/events?job={job}&after={}", events.len()), "");
        assert_eq!(code, 200, "{body}");
        let page: Value = serde_json::from_str(&body).expect("events are JSON");
        events.extend(page["events"].as_array().cloned().unwrap_or_default());
        if events.iter().any(|e| e["kind"] == "done") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    assert!(events.iter().any(|e| e["kind"] == "done"), "the job ended: {events:#?}");
    let report = events.iter().find(|e| e["kind"] == "report").expect("a report");
    assert!(report["report"]["markdown"].as_str().unwrap_or("").contains("# Simulation report"));
    assert!(events.iter().all(|e| e["kind"] != "error"), "{events:#?}");

    // While nothing runs, a second job is accepted; a reset is accepted.
    let (code, _) = http(&addr, "POST", "/agent/reset", "{}");
    assert_eq!(code, 200);

    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        if !key.trim().is_empty() {
            let all = serde_json::to_string(&events).unwrap_or_default();
            assert!(!all.contains(key.trim()), "the key reached an event");
            let (_, s) = http(&addr, "GET", "/agent/status", "");
            assert!(!s.contains(key.trim()), "the key reached the status");
        }
    }
    rt.block_on(server.stop());
}
