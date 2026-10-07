//! Runs every network validation check in both modes and writes
//! `docs/validation/network.md`.
//!
//! `--quick` leaves out the checks that run the engine. `--only <ID,...>` runs the named
//! checks and prints them without writing the report. The exit status is non-zero when
//! any check fails or cannot fail.

use std::path::Path;
use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let quick = args.iter().any(|a| a == "--quick");
    let only: Option<Vec<String>> = args
        .iter()
        .position(|a| a == "--only")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.split(',').map(str::to_string).collect());
    let checks: Vec<v2xw_netval::Check> = v2xw_netval::all()
        .into_iter()
        .filter(|c| !quick || c.cost == v2xw_netval::Cost::Fast)
        .filter(|c| only.as_ref().is_none_or(|o| o.iter().any(|id| id == c.id)))
        .collect();
    let mut results = Vec::new();
    for c in checks {
        let start = std::time::Instant::now();
        let v = v2xw_netval::run(&c);
        eprintln!(
            "{:<7} {:<11} {:>6.1}s  {}\n        faulted: {:?} {}",
            c.id,
            v.label(),
            start.elapsed().as_secs_f64(),
            v.honest.measured,
            v.faulted.status,
            v.faulted.measured
        );
        results.push((c, v));
    }
    let bad = results.iter().filter(|r| !r.1.valid() && !r.1.skipped()).count();
    if only.is_none() {
        let commit = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let doc = v2xw_netval::report::render(&results, &commit, quick);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/validation/network.md");
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(&path, doc).expect("the report is writable");
        eprintln!("wrote {}", path.display());
    }
    if bad > 0 {
        eprintln!("{bad} checks failed or cannot fail");
        std::process::exit(1);
    }
}
