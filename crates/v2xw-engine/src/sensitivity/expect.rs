//! Expectation checks: the expectation table run on its own (`v2xw sensitivity --expect`,
//! `tests/sensitivity_expectations.rs`).
//!
//! Two kinds of row:
//!
//! - **One setting** ([`super::sweep::Expectation`], `tests/expectations/*.yaml`): moving a
//!   setting to an end of its range moves a metric a known way, judged on the catalogue
//!   sweep's perturbation of that setting. [`expect_plan`] plans only the perturbations the
//!   table names, so the table runs without the whole sweep.
//! - **Two settings together** ([`Interaction`], `tests/expectations/interactions/*.yaml`):
//!   a 2 × 2 factorial on one family, all four runs on the same seed: the base, the
//!   *factor* alone, the *condition* alone, and both. The factor's effect without the
//!   condition (`A − base`) is compared with its effect under it (`AB − B`), in the
//!   metric's own units (Montgomery, *Design and Analysis of Experiments* 9e, 5.2: the
//!   interaction is the difference between the simple effects). DCC on a congested channel
//!   should *dampen* the CBR rise that density brings; a larger signature should
//!   *amplify* the airtime a higher message rate costs; and so on.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Direction;
use super::apply;
use super::sweep::{
    Expectation, Families, Fingerprint, Setting, Sweep, Verdict, fingerprint, plan, read_yaml,
    yaml_files,
};
use crate::error::Result;
use crate::scenario::Scenario;

/// How the condition changes the factor's effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Interplay {
    /// The effect under the condition is smaller in size (it may vanish).
    Dampens,
    /// The effect under the condition is larger in size, the same way.
    Amplifies,
    /// The effect keeps its sign under the condition (a robust effect).
    Holds,
    /// The effect changes sign under the condition.
    Reverses,
}

/// One interaction check: a 2 × 2 factorial on `family`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interaction {
    /// A short label (`density x DCC`).
    pub name: String,
    /// The family (a name in the families file) all four runs start from.
    pub family: String,
    /// Fields applied to all four runs.
    #[serde(default)]
    pub base_set: BTreeMap<String, Value>,
    /// The factor whose effect is measured (A).
    pub factor: BTreeMap<String, Value>,
    /// The condition it is measured under (B).
    pub condition: BTreeMap<String, Value>,
    /// A fingerprint metric.
    pub metric: String,
    /// How B changes A's effect.
    pub expect: Interplay,
    /// The direction of A's effect without B, when theory fixes it (checked too).
    #[serde(default)]
    pub direction: Option<Direction>,
    /// The relative change A alone must reach for `direction` (default 0.01).
    #[serde(default)]
    pub min_effect: Option<f64>,
    /// The relative margin: `dampens` needs |AB − B| ≤ (1 − m)·|A − base|, `amplifies`
    /// |AB − B| ≥ (1 + m)·|A − base|. Default 0.05.
    #[serde(default)]
    pub min_change: Option<f64>,
    /// The theory, with a citation.
    pub source: String,
}

/// An interaction's verdict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionVerdict {
    /// The check.
    pub interaction: Interaction,
    /// The metric in the four runs: base, A, B, A and B.
    pub values: [Option<f64>; 4],
    /// A − base.
    pub effect: Option<f64>,
    /// AB − B.
    pub effect_under: Option<f64>,
    /// Whether it held.
    pub pass: bool,
    /// Why not, or a run error.
    pub note: String,
}

/// Reads every interaction file `paths` names (files, or directories of `*.yaml`).
///
/// # Errors
/// A file cannot be read or parsed.
pub fn load_interactions(paths: &[PathBuf]) -> Result<Vec<Interaction>> {
    let mut out = Vec::new();
    for p in yaml_files(paths)? {
        let mut rows: Vec<Interaction> = read_yaml(&p)?;
        out.append(&mut rows);
    }
    Ok(out)
}

/// The catalogue plan cut down to the perturbations `expectations` name: only settings
/// with a row, only the ends (or literal values) a row names.
#[must_use]
pub fn expect_plan(
    families: &Families,
    expectations: &[Expectation],
) -> (Vec<Setting>, Vec<String>) {
    let (mut settings, skipped) = plan(families, expectations);
    let wanted: BTreeSet<&str> = expectations.iter().map(|e| e.setting.as_str()).collect();
    settings.retain(|s| wanted.contains(s.path.as_str()));
    for s in &mut settings {
        let rows: Vec<&Expectation> = expectations
            .iter()
            .filter(|e| e.setting == s.path)
            .collect();
        s.values.retain(|(label, v)| {
            rows.iter().any(|e| match e.change.as_str() {
                Some(end @ ("low" | "high")) => label == end,
                _ => *v == e.change,
            })
        });
    }
    // Rows whose setting the plan skips or does not publish: reported, not silently dropped.
    let planned: BTreeSet<&str> = settings.iter().map(|s| s.path.as_str()).collect();
    let mut unplanned: Vec<String> = wanted
        .iter()
        .filter(|w| !planned.contains(*w))
        .map(|w| {
            let why = skipped
                .iter()
                .find(|(p, _)| p == w)
                .map_or("not a published setting", |(_, r)| r.as_str());
            format!("{w}: {why}")
        })
        .collect();
    unplanned.sort();
    (settings, unplanned)
}

fn cache_key(family: &str, set: &BTreeMap<String, Value>, base: &Scenario) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(base).unwrap_or_default().hash(&mut h);
    format!(
        "fp:{family}:{}#{:016x}",
        serde_json::to_string(set).unwrap_or_default(),
        h.finish()
    )
}

/// Runs every interaction (four runs each, shared runs once) on `jobs` threads, reusing
/// and appending to the JSON-lines `cache` (the sweep's cache file works: its lines are
/// told apart by shape).
///
/// # Errors
/// None at present: a refused or failed run fails its rows with the error as the note.
pub fn run_interactions(
    interactions: &[Interaction],
    bases: &BTreeMap<String, Scenario>,
    jobs: usize,
    cache: Option<&Path>,
    progress: impl Fn(usize, usize, &str) + Sync,
) -> Result<Vec<InteractionVerdict>> {
    let cached: BTreeMap<String, Fingerprint> = cache
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| {
            t.lines()
                .filter(|l| l.starts_with("[\"fp:"))
                .filter_map(|l| serde_json::from_str::<(String, Fingerprint)>(l).ok())
                .collect()
        })
        .unwrap_or_default();
    let sink: Option<Mutex<std::fs::File>> = cache.and_then(|p| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .ok()
            .map(Mutex::new)
    });
    // The four runs of each check, as (family, fields).
    let quad = |i: &Interaction| -> [BTreeMap<String, Value>; 4] {
        let mut b = i.base_set.clone();
        let base = b.clone();
        let mut a = b.clone();
        a.extend(i.factor.clone());
        b.extend(i.condition.clone());
        let mut ab = b.clone();
        ab.extend(i.factor.clone());
        [base, a, b, ab]
    };
    let mut runs: Vec<(String, BTreeMap<String, Value>)> = Vec::new();
    for i in interactions {
        for set in quad(i) {
            let r = (i.family.clone(), set);
            if !runs.contains(&r) {
                runs.push(r);
            }
        }
    }
    let results: Mutex<BTreeMap<usize, std::result::Result<Fingerprint, String>>> =
        Mutex::new(BTreeMap::new());
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..jobs.max(1) {
            scope.spawn(|| {
                loop {
                    let k = next.fetch_add(1, Ordering::SeqCst);
                    let Some((fam, set)) = runs.get(k) else { break };
                    let label = format!("{fam} {}", serde_json::to_string(set).unwrap_or_default());
                    let r = match bases.get(fam) {
                        None => Err(format!("no family `{fam}` in the families file")),
                        Some(base) => {
                            let key = cache_key(fam, set, base);
                            if let Some(fp) = cached.get(&key) {
                                Ok(fp.clone())
                            } else {
                                let r = apply(base, set)
                                    .and_then(fingerprint)
                                    .map_err(|e| e.to_string());
                                if let (Ok(fp), Some(f)) = (&r, &sink)
                                    && let Ok(mut f) = f.lock()
                                    && let Ok(line) = serde_json::to_string(&(&key, fp))
                                {
                                    use std::io::Write as _;
                                    let _ = writeln!(f, "{line}");
                                }
                                r
                            }
                        }
                    };
                    let n = done.fetch_add(1, Ordering::SeqCst) + 1;
                    progress(n, runs.len(), &label);
                    if let Ok(mut m) = results.lock() {
                        m.insert(k, r);
                    }
                }
            });
        }
    });
    let results = results.into_inner().unwrap_or_default();
    let get = |fam: &str, set: &BTreeMap<String, Value>| {
        runs.iter()
            .position(|(f, s)| f == fam && s == set)
            .and_then(|k| results.get(&k).cloned())
            .unwrap_or_else(|| Err("not run".into()))
    };
    Ok(interactions
        .iter()
        .map(|i| {
            let fps: Vec<_> = quad(i).iter().map(|s| get(&i.family, s)).collect();
            if let Some(e) = fps.iter().find_map(|r| r.as_ref().err()) {
                return InteractionVerdict {
                    interaction: i.clone(),
                    values: [None; 4],
                    effect: None,
                    effect_under: None,
                    pass: false,
                    note: format!("run refused or failed: {e}"),
                };
            }
            let v: Vec<Option<f64>> = fps
                .iter()
                .map(|r| r.as_ref().ok().and_then(|fp| fp.get(&i.metric).copied()))
                .collect();
            judge_interaction(i, [v[0], v[1], v[2], v[3]])
        })
        .collect())
}

/// Judges one interaction from its four values (base, A, B, AB).
#[must_use]
pub fn judge_interaction(i: &Interaction, values: [Option<f64>; 4]) -> InteractionVerdict {
    let mut out = InteractionVerdict {
        interaction: i.clone(),
        values,
        effect: None,
        effect_under: None,
        pass: false,
        note: String::new(),
    };
    let [Some(b0), Some(a), Some(b1), Some(ab)] = values else {
        out.note = format!("`{}` was not measured in all four runs", i.metric);
        return out;
    };
    let (d0, d1) = (a - b0, ab - b1);
    out.effect = Some(d0);
    out.effect_under = Some(d1);
    let tiny = 1e-12 * b0.abs().max(b1.abs()).max(1.0);
    if let Some(dir) = i.direction {
        let rel = d0 / b0.abs().max(1e-12);
        let tol = i.min_effect.unwrap_or(0.01);
        let ok = match dir {
            Direction::Up => d0 > tiny && rel >= tol,
            Direction::Down => d0 < -tiny && -rel >= tol,
            Direction::Same => rel.abs() <= tol,
            Direction::Changes => rel.abs() >= tol,
        };
        if !ok {
            out.note = format!(
                "the factor alone was expected {dir:?}, moved {:+.1} %",
                rel * 100.0
            );
            return out;
        }
    }
    if d0.abs() <= tiny {
        out.note = "the factor alone moved nothing: no effect to compare".into();
        return out;
    }
    let m = i.min_change.unwrap_or(0.05);
    let same_sign = d0.signum() == d1.signum() && d1.abs() > tiny;
    let ok = match i.expect {
        Interplay::Dampens => d1.abs() <= (1.0 - m) * d0.abs(),
        Interplay::Amplifies => same_sign && d1.abs() >= (1.0 + m) * d0.abs(),
        Interplay::Holds => same_sign,
        Interplay::Reverses => d1.abs() > tiny && d0.signum() != d1.signum(),
    };
    if !ok {
        out.note = format!(
            "expected {:?}: effect {d0:+.4} alone, {d1:+.4} under the condition",
            i.expect
        );
        return out;
    }
    out.pass = true;
    out
}

/// A row's status in the report.
#[must_use]
pub fn status(sweep: &Sweep, v: &Verdict) -> &'static str {
    if v.pass {
        return "PASS";
    }
    let o = sweep.outcomes.iter().find(|o| {
        o.path == v.expectation.setting
            && match v.expectation.change.as_str() {
                Some(l @ ("low" | "high")) => o.label == l,
                _ => o.value == v.expectation.change,
            }
    });
    match o {
        None => "UNPLANNED",
        Some(o) if o.family.is_none() => "REFUSED",
        Some(o) if o.error.is_some() => "ERROR",
        Some(o) if o.dead() => "DEAD",
        Some(_) => "FAIL",
    }
}

fn num(v: Option<f64>) -> String {
    v.map_or_else(|| "-".into(), |x| format!("{x:.4}"))
}

/// The plain-text pass/fail report: one line per row, failures first, then a total.
#[must_use]
pub fn report(sweep: &Sweep, interactions: &[InteractionVerdict], unplanned: &[String]) -> String {
    let mut lines: Vec<(bool, String)> = sweep
        .verdicts
        .iter()
        .map(|v| {
            let e = &v.expectation;
            let st = status(sweep, v);
            let rel = match (v.base, v.value) {
                (Some(b), Some(x)) => format!("{:+.1}%", (x - b) / b.abs().max(1e-12) * 100.0),
                _ => "-".into(),
            };
            (
                v.pass,
                format!(
                    "{st:<9} {}={} {} {:?} ({} -> {}, {rel}){}",
                    e.setting,
                    super::sweep::label(&e.change),
                    e.metric,
                    e.expect,
                    num(v.base),
                    num(v.value),
                    if v.pass {
                        String::new()
                    } else {
                        format!(": {}", v.note)
                    }
                ),
            )
        })
        .collect();
    lines.extend(interactions.iter().map(|v| {
        (
            v.pass,
            format!(
                "{:<9} [x] {} {} {:?} (alone {}, under {}){}",
                if v.pass { "PASS" } else { "FAIL" },
                v.interaction.name,
                v.interaction.metric,
                v.interaction.expect,
                num(v.effect),
                num(v.effect_under),
                if v.pass {
                    String::new()
                } else {
                    format!(": {}", v.note)
                }
            ),
        )
    }));
    lines.sort_by_key(|(pass, _)| *pass);
    let mut s = String::new();
    for (_, l) in &lines {
        s.push_str(l);
        s.push('\n');
    }
    for u in unplanned {
        s.push_str(&format!("UNPLANNED {u}\n"));
    }
    let pass = lines.iter().filter(|(p, _)| *p).count();
    s.push_str(&format!(
        "{pass}/{} rows pass ({} single-setting, {} interactions), {} unplanned, {:.0} s\n",
        lines.len(),
        sweep.verdicts.len(),
        interactions.len(),
        unplanned.len(),
        sweep.wall_s
    ));
    s
}

/// The report as Markdown, for docs/validation.
#[must_use]
pub fn markdown(sweep: &Sweep, interactions: &[InteractionVerdict], note: &str) -> String {
    let mut s = String::new();
    let single_pass = sweep.verdicts.iter().filter(|v| v.pass).count();
    let x_pass = interactions.iter().filter(|v| v.pass).count();
    s.push_str(&format!(
        "{note}\n\n**{single_pass}/{} single-setting rows and {x_pass}/{} interaction checks pass.**\n\n",
        sweep.verdicts.len(),
        interactions.len()
    ));
    s.push_str("## Single-setting rows\n\n| status | setting | change | metric | expect | base | changed | Δ | source |\n|---|---|---|---|---|---|---|---|---|\n");
    for v in &sweep.verdicts {
        let e = &v.expectation;
        let rel = match (v.base, v.value) {
            (Some(b), Some(x)) => format!("{:+.1}%", (x - b) / b.abs().max(1e-12) * 100.0),
            _ => "-".into(),
        };
        s.push_str(&format!(
            "| {} | `{}` | {} | `{}` | {:?}{} | {} | {} | {rel} | {} |\n",
            status(sweep, v),
            e.setting,
            super::sweep::label(&e.change),
            e.metric,
            e.expect,
            e.band
                .map_or_else(String::new, |[lo, hi]| format!(" [{lo}, {hi}]")),
            num(v.base),
            num(v.value),
            e.source.replace('|', "/")
        ));
    }
    s.push_str("\n## Interaction checks (2 × 2, same seed)\n\n| status | check | metric | expect | base | A | B | A+B | A alone | A under B | source |\n|---|---|---|---|---|---|---|---|---|---|---|\n");
    for v in interactions {
        let i = &v.interaction;
        s.push_str(&format!(
            "| {} | {} | `{}` | {:?} | {} | {} | {} | {} | {} | {} | {} |\n",
            if v.pass { "PASS" } else { "FAIL" },
            i.name,
            i.metric,
            i.expect,
            num(v.values[0]),
            num(v.values[1]),
            num(v.values[2]),
            num(v.values[3]),
            num(v.effect),
            num(v.effect_under),
            i.source.replace('|', "/")
        ));
    }
    let failing: Vec<String> = sweep
        .verdicts
        .iter()
        .filter(|v| !v.pass)
        .map(|v| {
            format!(
                "- `{}` = {} on `{}`: {} ({})",
                v.expectation.setting,
                super::sweep::label(&v.expectation.change),
                v.expectation.metric,
                status(sweep, v),
                v.note
            )
        })
        .chain(interactions.iter().filter(|v| !v.pass).map(|v| {
            format!(
                "- {} on `{}`: {}",
                v.interaction.name, v.interaction.metric, v.note
            )
        }))
        .collect();
    if !failing.is_empty() {
        s.push_str("\n## Failing\n\n");
        s.push_str(&failing.join("\n"));
        s.push('\n');
    }
    s
}
