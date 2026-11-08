//! `v2xw compare <A> <B>` — two runs side by side: the settings that differ and every
//! headline metric's change, the same table the Studio's "Compare with previous run" shows
//! (both print [`v2xw_server::compare::Comparison`]).
//!
//! ```text
//! v2xw compare runs/base runs/tx23            # two `v2xw run --out` directories
//! v2xw compare prev.json runs/tx23 --markdown # a digest saved from the Studio / runs.digest
//! v2xw compare runs/a1,runs/a2,runs/a3 runs/b1,runs/b2,runs/b3   # replications
//! ```
//!
//! A side is a run directory (`scenario.resolved.yaml` + `metrics.json`), a digest file
//! (`v2xw/run-digest/1`, what `runs.digest` returns), an experiment output directory (every
//! run under its `runs/` pooled as replications), or several of those separated by commas
//! (pooled as replications, which turns the noise test into a replication-based one).

use std::path::{Path, PathBuf};

use v2xw_server::compare::{self, DigestBuilder, RunDigest};

use crate::error::{CliError, Result};

/// How the comparison is printed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// The terminal table.
    Text,
    /// JSON (`v2xw/run-comparison/1`).
    Json,
    /// A Markdown document.
    Markdown,
    /// CSV, one line per metric then per setting.
    Csv,
}

fn bad(problem: String) -> CliError {
    CliError::BadArgument {
        flag: "run",
        problem,
    }
}

/// Reads one `v2xw run` output directory into a digest.
///
/// # Errors
/// When the directory has no `scenario.resolved.yaml` or `metrics.json`, or they do not parse.
pub fn digest_of_run_dir(dir: &Path) -> Result<RunDigest> {
    let scenario = v2xw_engine::Scenario::load(dir.join("scenario.resolved.yaml"))
        .map_err(|e| bad(format!("{}: {e}", dir.display())))?;
    let samples = v2xw_experiment::reduce::read_samples(&dir.join("metrics.json"))?;
    let mut b = DigestBuilder::new(true);
    for s in &samples {
        b.add(s);
    }
    let json = scenario
        .to_json()
        .map_err(|e| bad(format!("{}: {e}", dir.display())))?;
    let doc: serde_json::Value =
        serde_json::from_str(&json).map_err(|e| bad(format!("{}: {e}", dir.display())))?;
    Ok(RunDigest {
        schema: compare::DIGEST_SCHEMA.to_string(),
        run_id: dir
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        label: scenario.meta.name.clone(),
        seed: scenario.seed,
        city: scenario.current_city().map(|c| c.id.clone()),
        scenario_hash: scenario.content_hash().unwrap_or_default(),
        scenario: doc,
        t_reached_s: b.t_max_s(),
        t_end_s: scenario.time.horizon_ns() as f64 / 1e9,
        source: dir.display().to_string(),
        metrics: b.metrics(None),
    })
}

/// Reads one side: a run directory, a digest file, an experiment directory, or a
/// comma-separated list of those pooled as replications.
///
/// # Errors
/// When a path is none of those.
pub fn load_side(spec: &str) -> Result<RunDigest> {
    let mut runs = Vec::new();
    for part in spec.split(',').filter(|p| !p.is_empty()) {
        let path = PathBuf::from(part);
        if path.is_file() {
            let bytes = std::fs::read(&path).map_err(|e| CliError::io("cannot read", &path, e))?;
            let d: RunDigest = serde_json::from_slice(&bytes).map_err(|e| {
                bad(format!(
                    "{} is not a run digest (v2xw/run-digest/1): {e}",
                    path.display()
                ))
            })?;
            runs.push(d);
        } else if path.join("metrics.json").is_file() {
            runs.push(digest_of_run_dir(&path)?);
        } else if path.join("runs").is_dir() {
            let mut dirs: Vec<PathBuf> = std::fs::read_dir(path.join("runs"))
                .map_err(|e| CliError::io("cannot list", path.join("runs"), e))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.join("metrics.json").is_file())
                .collect();
            dirs.sort();
            for d in dirs {
                runs.push(digest_of_run_dir(&d)?);
            }
        } else {
            return Err(bad(format!(
                "{part} is not a run directory (no metrics.json), an experiment directory \
                 (no runs/) or a digest file"
            )));
        }
    }
    RunDigest::replicated(&runs).ok_or_else(|| bad(format!("`{spec}` names no run")))
}

/// Compares two sides and renders the result.
///
/// # Errors
/// As [`load_side`].
pub fn run(a: &str, b: &str, format: Format, all: bool) -> Result<String> {
    let c = compare::compare(&load_side(a)?, &load_side(b)?);
    Ok(match format {
        Format::Text => compare::text(&c, all),
        Format::Markdown => compare::markdown(&c),
        Format::Csv => compare::csv(&c),
        Format::Json => serde_json::to_string_pretty(&c).map_err(|e| CliError::Json {
            what: "run comparison",
            source: e,
        })?,
    })
}
