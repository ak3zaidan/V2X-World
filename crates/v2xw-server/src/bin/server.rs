//! `v2xw-server` — serve one run over VWP v1.
//!
//! Two modes. `--scenario <path>` runs the real kernel: the scenario is loaded, the world
//! is imported and `v2xw-engine` produces the stream. With no `--scenario`, the synthetic
//! fixture engine runs instead, which is what a client developer points at when they want
//! a conforming stream and no city extract.

use std::net::{IpAddr, SocketAddr};

use v2xw_server::{LiveOptions, ServerOptions, StubOptions, serve_scenario, serve_stub};

const USAGE: &str = "\
v2xw-server — the VWP v1 control surface and stream (docs/protocol/vwp-v1.md)

  v2xw-server --scenario scenarios/phase1-manhattan.yaml
  v2xw-server [fixture options]

  live run (the real engine):
  --scenario <p>  scenario to run; without it the synthetic fixture runs instead
  --record <p>    write the MCAP recording here
  --build-utc <t> manifest build timestamp (the engine may not read a clock)
  --retain <n>    mobility steps of history kept for run.seek (default 36000)
  --retain-mb <n> most memory that history may take, MB (default 1024)
  --lookahead <n> steps the kernel may run ahead of the stream (default 64)
  --resume <p>    open a `v2xw run --checkpoint-every` run where its checkpoint.json
                  stands: the run is computed to that instant and served paused there

  fixture run:
  --actors <n>    actors to drive (default 120)
  --grid <n>      junctions per side of the generated grid (default 8)
  --block <m>     junction spacing, metres (default 150)
  --duration <s>  simulated seconds (default 600)
  --seed <n>      fixture phase seed (default 20260918)

  both:
  --port <n>      TCP port, 0 for an ephemeral one (default 8787)
  --host <addr>   bind address (default 127.0.0.1; a non-loopback bind needs --token)
  --token <t>     bearer token required on the upgrade and every HTTP request
  --speed <x>     multiple of real time; 0 is unthrottled (default 1)
  --paused        start the run paused at t = 0
  --label <s>     human label for Hello.str_run_label
  --quiet         do not print the banner
  --help          this message
";

/// What `--resume` needs from a `v2xw run` checkpoint (`v2xw_cli::checkpoint::Checkpoint`,
/// read here by field so the server does not depend on the command line): the resolved
/// scenario written beside it, the build timestamp and the instant.
fn read_checkpoint(path: &std::path::Path) -> Result<(String, String, u64), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if v.get("format").and_then(serde_json::Value::as_str) != Some("v2xw-checkpoint/1") {
        return Err("not a v2xw-checkpoint/1 file".to_string());
    }
    let t_ns = v
        .get("t_ns")
        .and_then(serde_json::Value::as_u64)
        .ok_or("the checkpoint has no t_ns")?;
    let build_utc = v
        .get("build_utc")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string();
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let scenario = dir.join("scenario.resolved.yaml");
    if !scenario.exists() {
        return Err(format!("{} is missing", scenario.display()));
    }
    Ok((scenario.display().to_string(), build_utc, t_ns))
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let mut options = ServerOptions::default();
    let mut stub = StubOptions::default();
    let mut live = LiveOptions::default();
    let mut scenario: Option<String> = None;
    let mut resume: Option<std::path::PathBuf> = None;
    let mut host: IpAddr = [127, 0, 0, 1].into();
    let mut port: u16 = 8787;
    let mut quiet = false;

    let argv: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        let mut next = || -> Option<String> {
            i += 1;
            argv.get(i).cloned()
        };
        let parsed = match arg {
            "--help" | "-h" => {
                print!("{USAGE}");
                return std::process::ExitCode::SUCCESS;
            }
            "--quiet" => {
                quiet = true;
                Ok(())
            }
            "--paused" => {
                stub.paused = true;
                live.paused = true;
                Ok(())
            }
            "--scenario" => next()
                .map(|v| scenario = Some(v))
                .ok_or("--scenario needs a path"),
            "--resume" => next()
                .map(|v| resume = Some(std::path::PathBuf::from(v)))
                .ok_or("--resume needs the path of a checkpoint.json"),
            "--record" => next()
                .map(|v| live.recording = Some(std::path::PathBuf::from(v)))
                .ok_or("--record needs a path"),
            "--build-utc" => next()
                .map(|v| live.build_utc = v)
                .ok_or("--build-utc needs a timestamp"),
            "--retain" => next()
                .and_then(|v| v.parse().ok())
                .map(|v| live.retain_steps = v)
                .ok_or("--retain needs a number"),
            "--retain-mb" => next()
                .and_then(|v| v.parse::<usize>().ok())
                .map(|v| live.retain_bytes = v.saturating_mul(1 << 20))
                .ok_or("--retain-mb needs a number of megabytes"),
            "--lookahead" => next()
                .and_then(|v| v.parse().ok())
                .map(|v| live.lookahead_steps = v)
                .ok_or("--lookahead needs a number"),
            "--speed" => next()
                .and_then(|v| v.parse().ok())
                .map(|v| {
                    live.speed = v;
                    stub.speed = v;
                })
                .ok_or("--speed needs a number"),
            "--label" => next()
                .map(|v| {
                    stub.label = v.clone();
                    live.label = v;
                })
                .ok_or("--label needs a value"),
            "--actors" => next()
                .and_then(|v| v.parse().ok())
                .map(|v| stub.actors = v)
                .ok_or("--actors needs a number"),
            "--grid" => next()
                .and_then(|v| v.parse().ok())
                .map(|v| stub.grid = v)
                .ok_or("--grid needs a number"),
            "--block" => next()
                .and_then(|v| v.parse().ok())
                .map(|v| stub.block_m = v)
                .ok_or("--block needs a number"),
            "--duration" => next()
                .and_then(|v| v.parse().ok())
                .map(|v| stub.duration_s = v)
                .ok_or("--duration needs a number"),
            "--seed" => next()
                .and_then(|v| v.parse().ok())
                .map(|v| stub.seed = v)
                .ok_or("--seed needs a number"),
            "--port" => next()
                .and_then(|v| v.parse().ok())
                .map(|v| port = v)
                .ok_or("--port needs a number"),
            "--host" => next()
                .and_then(|v| v.parse().ok())
                .map(|v| host = v)
                .ok_or("--host needs an IP address"),
            "--token" => next()
                .map(|v| options.token = Some(v))
                .ok_or("--token needs a value"),
            other => Err(if other.starts_with("--") {
                "unknown option"
            } else {
                "unexpected argument"
            }),
        };
        if let Err(message) = parsed {
            eprintln!("{message}: {arg}");
            return std::process::ExitCode::FAILURE;
        }
        i += 1;
    }
    options.bind = SocketAddr::new(host, port);

    // A checkpoint names its run: the resolved scenario beside it and the build timestamp.
    // The run is served paused at the checkpoint's instant, computed there again (the
    // engine is deterministic, so it is the same run).
    let mut resume_at: Option<u64> = None;
    if let Some(path) = &resume {
        match read_checkpoint(path) {
            Ok((scenario_path, build_utc, t_ns)) => {
                scenario = Some(scenario_path);
                live.build_utc = build_utc;
                live.paused = true;
                resume_at = Some(t_ns);
            }
            Err(message) => {
                eprintln!("v2xw-server: --resume {}: {message}", path.display());
                return std::process::ExitCode::FAILURE;
            }
        }
    }

    let started = match &scenario {
        Some(path) => serve_scenario(options, path, live).await,
        None => serve_stub(options, stub).await,
    };
    let server = match started {
        Ok(s) => s,
        Err(e) => {
            eprintln!("v2xw-server: {}", v2xw_server::error::describe(&e));
            return std::process::ExitCode::FAILURE;
        }
    };
    if let Some(t_ns) = resume_at {
        let run = std::sync::Arc::clone(server.run());
        let target = t_ns.saturating_add(1);
        let reached = tokio::task::spawn_blocking(move || {
            let mut last = 0;
            let mut printed = 0u64;
            loop {
                let reached = run.extend_to(target, std::time::Duration::from_millis(200));
                if reached >= target || (reached == last && reached > 0 && run.extend_to(target, std::time::Duration::from_secs(5)) == reached) {
                    let _ = run.seek(reached.min(target));
                    return reached;
                }
                if !quiet && reached / 3_600_000_000_000 > printed {
                    printed = reached / 3_600_000_000_000;
                    eprintln!(
                        "v2xw-server: computing to the checkpoint: {:.1} of {:.1} simulated hours",
                        reached as f64 / 3.6e12,
                        target as f64 / 3.6e12
                    );
                }
                last = reached;
            }
        })
        .await
        .unwrap_or(0);
        if !quiet {
            eprintln!(
                "v2xw-server: at the checkpoint, {:.1} simulated hours in; paused",
                reached as f64 / 3.6e12
            );
        }
    }
    if !quiet {
        let run = server.run();
        let d = run.descriptor();
        let (actors, nodes) = run.counts();
        let hash = run.world().content_hash_hex();
        println!("v2xw-server listening on {}", server.http_url());
        println!(
            "  stream   {}?compress=none&v=1   (subprotocol vwp.v1)",
            server.ws_url()
        );
        println!("  world    {}/world/{hash}.vwb", server.http_url());
        println!("  json     {}/world/{hash}.json", server.http_url());
        println!(
            "  rpc      POST {}/rpc   schema {}/rpc/schema",
            server.http_url(),
            server.http_url()
        );
        println!("  run      {}", d.run_id);
        println!(
            "  source   {}",
            scenario
                .as_deref()
                .unwrap_or("synthetic fixture (no --scenario)")
        );
        println!("  scenario {}", d.scenario_hash_hex);
        if let Some(path) = &d.recording_path {
            println!("  record   {path}");
        }
        println!("  traffic  {actors} actors, {nodes} nodes");
        // The banner is the readiness signal a test harness waits on, so it must be
        // flushed before the process starts serving quietly.
        use std::io::Write;
        let _ = std::io::stdout().flush();
    }
    let _ = tokio::signal::ctrl_c().await;
    server.stop().await;
    std::process::ExitCode::SUCCESS
}
