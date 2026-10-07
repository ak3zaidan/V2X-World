//! `traffic_validation` — runs the traffic validation suite and writes its report.
//!
//! ```text
//! cargo run -p v2xw-trafficval --bin traffic_validation -- [--quick] [--only SUBSTR]
//!     [--out docs/validation/traffic.md] [--json FILE] [--list]
//! ```
//!
//! With no `--only`, every check runs and the report is written to
//! `docs/validation/traffic.md` (relative to the workspace root). With `--only`, the
//! report goes to the terminal unless `--out` names a file, so a partial run never
//! overwrites the full report. Exits 1 when a held figure is out of band or a check could
//! not run.

use std::path::PathBuf;

use v2xw_trafficval::{Config, Lab, checks, report, run};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let quick = args.iter().any(|a| a == "--quick");
    let only = value("--only");
    let cfg = Config::new(quick);
    if args.iter().any(|a| a == "--list") {
        for c in checks() {
            println!("{:<44} {}", c.id, c.title);
        }
        return;
    }
    // Scenario files name their worlds relative to the workspace root.
    if let Err(e) = std::env::set_current_dir(&cfg.root) {
        eprintln!("cannot enter the workspace root {}: {e}", cfg.root.display());
        std::process::exit(2);
    }
    let mut lab = Lab::new(cfg.clone());
    let results = run::run_all(&mut lab, only.as_deref());
    let mut command = String::from("cargo run -p v2xw-trafficval --bin traffic_validation --");
    if quick {
        command.push_str(" --quick");
    }
    if let Some(o) = &only {
        command.push_str(&format!(" --only {o}"));
    }
    let md = report::markdown(&results, command.trim_end_matches(" --"), quick);
    let out = value("--out").map(PathBuf::from).or_else(|| {
        only.is_none()
            .then(|| cfg.root.join("docs/validation/traffic.md"))
    });
    match out {
        Some(path) => {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            std::fs::write(&path, &md).expect("write the report");
            eprintln!("wrote {}", path.display());
        }
        None => println!("{md}"),
    }
    if let Some(json) = value("--json") {
        std::fs::write(&json, serde_json::to_string_pretty(&results).expect("json"))
            .expect("write the json");
    }
    let failed: Vec<&str> = results
        .iter()
        .filter(|r| r.failed())
        .map(|r| r.id.as_str())
        .collect();
    if !failed.is_empty() {
        eprintln!("FAILED: {failed:?}");
        std::process::exit(1);
    }
}
