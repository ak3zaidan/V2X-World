//! A small evaluation set for the agent with a real model: prompts, the scenario change each
//! one should produce, and what its report must contain.
//!
//! Runs only when `ANTHROPIC_API_KEY` is set (and `V2XW_AGENT_EVAL` is not `off`); without a
//! key it prints that it was skipped and passes, so the suite stays offline by default. With
//! a key it starts a real engine in this process on the procedural grid, points the agent at
//! it over HTTP exactly as `v2xw serve` does, and checks each case:
//!
//! * the scenario change the agent made (the `run.start` document, read back from the run),
//! * that every run reached its end and was analysed,
//! * that the overview states no number the analyst did not produce.
//!
//! A model is not deterministic, so a failure here is a finding about the prompt or the
//! tools, to be read, not a flaky test to retry blindly.

use serde_json::Value;
use v2xw_copilot::agent::{Agent, AgentPolicy};
use v2xw_copilot::{CurlPost, HttpRpc};

struct Case {
    prompt: &'static str,
    /// `(dotted key, check on the value the last run used)`.
    expect: Vec<(&'static str, fn(&Value) -> bool)>,
    runs_at_least: usize,
    compares: bool,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            prompt: "Run the procedural grid scenario with twice its traffic demand, 20 seconds long, and tell me how the channel holds up.",
            expect: vec![
                ("actors.vehicles.demand.rate_veh_per_h", |v| v.as_f64() == Some(60.0)),
                ("time.duration_s", |v| v.as_f64() == Some(20.0)),
            ],
            runs_at_least: 1,
            compares: false,
        },
        Case {
            prompt: "On the grid scenario, 20 seconds long, compare DSRC with LTE-V2X.",
            expect: vec![("radio.rat", |v| v.as_str() == Some("lte-v2x-pc5") || v.as_str() == Some("dsrc-80211p"))],
            runs_at_least: 2,
            compares: true,
        },
    ]
}

fn pointer(key: &str) -> String {
    format!("/{}", key.replace('.', "/"))
}

#[test]
fn the_agent_meets_its_evaluation_set_with_a_real_model() {
    if std::env::var("ANTHROPIC_API_KEY").map(|k| k.trim().is_empty()).unwrap_or(true)
        || std::env::var("V2XW_AGENT_EVAL").is_ok_and(|v| v == "off")
    {
        eprintln!("skipped: ANTHROPIC_API_KEY is not set, so the evaluation set needs no network and runs nothing");
        return;
    }
    let scenario = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/phase1-grid.yaml");
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
    let mut failures = Vec::new();
    for case in cases() {
        let provider = v2xw_copilot::claude::provider_from_env().expect("a key is set");
        let rpc = HttpRpc::new(CurlPost::new().with_timeout_s(60), &origin);
        let mut agent = Agent::new(Some(provider), rpc)
            .expect("builds")
            .with_policy(AgentPolicy { poll_ms: 250, ..AgentPolicy::default() });
        match agent.ask(case.prompt, &mut |_| {}) {
            Err(e) => failures.push(format!("{:?}: {e}", case.prompt)),
            Ok(report) => {
                if report.runs.len() < case.runs_at_least {
                    failures.push(format!("{:?}: {} run(s), expected {}", case.prompt, report.runs.len(), case.runs_at_least));
                }
                if case.compares && report.comparison.is_none() {
                    failures.push(format!("{:?}: no comparison", case.prompt));
                }
                if !report.ungrounded.is_empty() {
                    failures.push(format!("{:?}: unverified numbers {:?}", case.prompt, report.ungrounded));
                }
                for run in &report.runs {
                    if run.analysis.finding("simulation.incomplete").is_some() {
                        failures.push(format!("{:?}: run {} did not reach its end", case.prompt, run.label));
                    }
                }
                let changed: Vec<_> = report.runs.iter().filter_map(|r| r.prepared.as_ref()).flat_map(|p| p.changes.clone()).collect();
                for (key, ok) in &case.expect {
                    let hit = changed.iter().any(|c| c.key == *key && ok(&c.after));
                    if !hit {
                        failures.push(format!("{:?}: expected a change to {key} ({}), got {:?}", case.prompt, pointer(key),
                            changed.iter().map(|c| (&c.key, &c.after)).collect::<Vec<_>>()));
                    }
                }
                eprintln!("---- {}\n{}", case.prompt, report.markdown);
            }
        }
    }
    rt.block_on(server.stop());
    assert!(failures.is_empty(), "{failures:#?}");
}
