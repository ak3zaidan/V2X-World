//! The MA policy family report: a sweep over `security.scms.ma_policy.preset` writes
//! `ma-policy-family.csv` and `ma-policy-family.md`, one row per (policy, seed) and a mean
//! with a 95 % interval per policy.
//!
//! Every point is a full run, so the comparison carries the feedback an offline replay
//! (`v2xw ma-replay --policies`) cannot show: a revoked device stops sending, the CRL's
//! delays apply and reporters' reputations change.
//!
//! **Ground truth is joined here and only here**, after every run decided:
//! `gt.device_exposure` (who is an attacker, when its attack began, how long each device
//! was in the run) and the truth join of `pki.report_fate` (which device stood behind an
//! accused pseudonym). A decision is about a device: the engine resolves the decided
//! pseudonym to its device, so the honest/attacker count joins through the device, never
//! through the pseudonym.
//!
//! The interval is Student's t over the seeds (`mean ± t₀.₉₇₅,ₙ₋₁ · s/√n`): with two or
//! three seeds the normal approximation's 1.96 would claim several times the precision
//! the sample has.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use v2xw_core::ids::NodeId;
use v2xw_core::time::SimTime;
use v2xw_engine::report_records::{DeviceExposure, ReportFate};

use crate::error::{ExperimentError, Result};

/// The sweep axis that makes a sweep an MA policy comparison.
pub const SWEEP_AXIS: &str = v2xw_engine::ma_policy::SWEEP_AXIS;
/// The per-run and per-policy table.
pub const CSV_FILE: &str = "ma-policy-family.csv";
/// The same, as a Markdown summary.
pub const MD_FILE: &str = "ma-policy-family.md";

/// What one run's recording says, as the report reads it.
#[derive(Debug, Clone, Default)]
pub struct RunFacts {
    /// `gt.device_exposure`: every SCMS device (ground truth).
    pub devices: Vec<DeviceExposure>,
    /// From `pki.report_fate`'s truth: the device behind each accused pseudonym.
    pub subject_device: BTreeMap<String, NodeId>,
    /// `ma.decision` revocations: `(instant, pseudonym)`.
    pub decisions: Vec<(SimTime, String)>,
    /// `proto.revocation`: per revocation (its pseudonym), each stage's first instant.
    pub stages: BTreeMap<String, BTreeMap<String, SimTime>>,
    /// `ma.received` rows the MA accepted as evidence.
    pub reports_processed: u64,
    /// The run's last record instant: an open case is open until then.
    pub end: SimTime,
}

fn parse(channel: &'static str, json: &[u8]) -> Result<serde_json::Value> {
    serde_json::from_slice(json).map_err(|e| ExperimentError::json(channel, e))
}

impl RunFacts {
    /// Takes one record.
    ///
    /// # Errors
    /// A record of a channel it reads does not parse.
    pub fn push(&mut self, channel: &str, at: SimTime, json: &[u8]) -> Result<()> {
        self.end = self.end.max(at);
        match channel {
            "gt.device_exposure" => {
                let v = parse("gt.device_exposure", json)?;
                self.devices.push(
                    serde_json::from_value(v)
                        .map_err(|e| ExperimentError::json("gt.device_exposure", e))?,
                );
            }
            "pki.report_fate" => {
                let v = parse("pki.report_fate", json)?;
                let f: ReportFate = serde_json::from_value(v)
                    .map_err(|e| ExperimentError::json("pki.report_fate", e))?;
                if let Some(n) = f.truth.subject_node {
                    self.subject_device.insert(f.subject, n);
                }
            }
            "ma.decision" => {
                let v = parse("ma.decision", json)?;
                if v["decision"] == "revoke"
                    && let Some(s) = v["subject"].as_str()
                {
                    self.decisions.push((at, s.to_string()));
                }
            }
            "proto.revocation" => {
                let v = parse("proto.revocation", json)?;
                if let (Some(id), Some(stage)) = (v["id"].as_str(), v["stage"].as_str()) {
                    let t = v["t"].as_u64().unwrap_or(at);
                    let e = self
                        .stages
                        .entry(id.to_string())
                        .or_default()
                        .entry(stage.to_string())
                        .or_insert(t);
                    *e = (*e).min(t);
                }
            }
            "ma.received" => {
                let v = parse("ma.received", json)?;
                if v["set_aside"].is_null() {
                    self.reports_processed += 1;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Reads a run directory's `recording.mcap`.
    ///
    /// # Errors
    /// No recording (a sweep run with `--no-recording`), or one that does
    /// not read.
    pub fn read(dir: &Path) -> Result<RunFacts> {
        let path = dir.join("recording.mcap");
        if !path.exists() {
            return Err(ExperimentError::Run {
                run_id: dir.display().to_string(),
                message: "the MA policy family report reads each run's recording; run the \
                          sweep without `--no-recording`"
                    .to_string(),
            });
        }
        let mut reader = v2xw_record::reader::Reader::open(&path)?;
        let mut facts = RunFacts::default();
        let mut bad = None;
        // One streamed pass, keeping only what the report reads: a run's recording is
        // hundreds of megabytes, most of it frames and radio records.
        reader.for_each_message(
            |_| Ok(()),
            |r| {
                if bad.is_none()
                    && let Err(e) = facts.push(&r.channel, r.sim_time, &r.json)
                {
                    bad = Some(e);
                }
                Ok(())
            },
        )?;
        match bad {
            Some(e) => Err(e),
            None => Ok(facts),
        }
    }
}

/// One (policy, seed) row.
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyRow {
    /// The policy: a preset id or a key of `security.scms.ma_policies`.
    pub policy: String,
    /// The run's seed, hex.
    pub seed: String,
    /// Attackers in the run.
    pub attackers: usize,
    /// Of them revoked.
    pub attackers_revoked: usize,
    /// Honest devices in the run.
    pub honest_devices: usize,
    /// Their summed time in the run, h.
    pub honest_device_hours: f64,
    /// Distinct honest devices revoked.
    pub honest_revoked: usize,
    /// Per 10⁶ honest device-hours.
    pub honest_blocked_per_1e6_dh: f64,
    /// Attack start to the MA's decision, per revoked attacker, s: median and p90.
    pub to_decision_s: (Option<f64>, Option<f64>),
    /// The decision to the CRL's publication, s: median and p90.
    pub to_published_s: (Option<f64>, Option<f64>),
    /// The publication to the first receiver refusing, s: median and p90.
    pub to_refused_s: (Option<f64>, Option<f64>),
    /// Reports the MA accepted as evidence.
    pub reports_processed: u64,
    /// The most revocation cases open at once (decision to the CRL entry's issue).
    pub peak_open_cases: usize,
}

/// The median and the 90th percentile (nearest rank) of `v`.
fn median_p90(mut v: Vec<f64>) -> (Option<f64>, Option<f64>) {
    if v.is_empty() {
        return (None, None);
    }
    v2xw_core::math::sort_total_order(&mut v);
    let rank = |q: f64| {
        let i = (q * v.len() as f64).ceil() as usize;
        v[i.clamp(1, v.len()) - 1]
    };
    (Some(rank(0.5)), Some(rank(0.9)))
}

const S: f64 = 1e9;

/// The row of one run.
#[must_use]
pub fn row(policy: &str, seed: &str, f: &RunFacts) -> PolicyRow {
    // The first decision about each device, through the truth join.
    let mut first: BTreeMap<NodeId, (SimTime, &str)> = BTreeMap::new();
    for (t, subject) in &f.decisions {
        if let Some(n) = f.subject_device.get(subject) {
            let e = first.entry(*n).or_insert((*t, subject.as_str()));
            if *t < e.0 {
                *e = (*t, subject.as_str());
            }
        }
    }
    let honest: Vec<&DeviceExposure> = f.devices.iter().filter(|d| !d.attacker).collect();
    let hours: f64 = honest
        .iter()
        .map(|d| d.left.saturating_sub(d.joined) as f64 / 3.6e12)
        .sum();
    let honest_revoked = honest
        .iter()
        .filter(|d| first.contains_key(&d.node))
        .count();
    let attackers: Vec<&DeviceExposure> = f.devices.iter().filter(|d| d.attacker).collect();
    let (mut to_decision, mut to_published, mut to_refused) = (Vec::new(), Vec::new(), Vec::new());
    let mut attackers_revoked = 0;
    for a in &attackers {
        let Some((t, subject)) = first.get(&a.node) else {
            continue;
        };
        attackers_revoked += 1;
        let start = a.attack_start.unwrap_or(a.joined);
        to_decision.push(t.saturating_sub(start) as f64 / S);
        let stages = f.stages.get(*subject);
        let published = stages.and_then(|s| s.get("published")).copied();
        if let Some(p) = published {
            to_published.push(p.saturating_sub(*t) as f64 / S);
            if let Some(e) = stages.and_then(|s| s.get("enforced")) {
                to_refused.push(e.saturating_sub(p) as f64 / S);
            }
        }
    }
    // Open cases: each revocation from its decision to its CRL entry's issue.
    let mut edges: Vec<(SimTime, i32)> = Vec::new();
    for s in f.stages.values() {
        if let Some(d) = s.get("decision") {
            edges.push((*d, 1));
            edges.push((s.get("issued").copied().unwrap_or(f.end.max(*d) + 1), -1));
        }
    }
    edges.sort_unstable_by_key(|e| (e.0, e.1));
    let (mut open, mut peak) = (0_i32, 0_i32);
    for (_, d) in edges {
        open += d;
        peak = peak.max(open);
    }
    PolicyRow {
        policy: policy.to_string(),
        seed: seed.to_string(),
        attackers: attackers.len(),
        attackers_revoked,
        honest_devices: honest.len(),
        honest_device_hours: hours,
        honest_revoked,
        honest_blocked_per_1e6_dh: if hours > 0.0 {
            honest_revoked as f64 / hours * 1e6
        } else {
            0.0
        },
        to_decision_s: median_p90(to_decision),
        to_published_s: median_p90(to_published),
        to_refused_s: median_p90(to_refused),
        reports_processed: f.reports_processed,
        peak_open_cases: usize::try_from(peak).unwrap_or(0),
    }
}

/// The numeric columns, in table order, with each row's value.
fn columns(r: &PolicyRow) -> Vec<(&'static str, Option<f64>)> {
    vec![
        ("attackers", Some(r.attackers as f64)),
        ("attackers_revoked", Some(r.attackers_revoked as f64)),
        ("honest_devices", Some(r.honest_devices as f64)),
        ("honest_device_hours", Some(r.honest_device_hours)),
        ("honest_revoked", Some(r.honest_revoked as f64)),
        (
            "honest_blocked_per_1e6_dh",
            Some(r.honest_blocked_per_1e6_dh),
        ),
        ("to_decision_median_s", r.to_decision_s.0),
        ("to_decision_p90_s", r.to_decision_s.1),
        ("to_published_median_s", r.to_published_s.0),
        ("to_published_p90_s", r.to_published_s.1),
        ("to_refused_median_s", r.to_refused_s.0),
        ("to_refused_p90_s", r.to_refused_s.1),
        ("reports_processed", Some(r.reports_processed as f64)),
        ("peak_open_cases", Some(r.peak_open_cases as f64)),
    ]
}

/// Student's t, two-sided 95 %, for `df` degrees of freedom (NIST/SEMATECH e-Handbook
/// 1.3.6.7.2); 1.96 beyond 30.
fn t975(df: usize) -> f64 {
    const T: [f64; 30] = [
        12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179, 2.160,
        2.145, 2.131, 2.120, 2.110, 2.101, 2.093, 2.086, 2.080, 2.074, 2.069, 2.064, 2.060, 2.056,
        2.052, 2.048, 2.045, 2.042,
    ];
    T.get(df.wrapping_sub(1)).copied().unwrap_or(1.96)
}

/// One policy's summary of a column: `(mean, 95 % half-width)`; the half-width is `None`
/// below two values.
fn summarise(values: &[f64]) -> Option<(f64, Option<f64>)> {
    if values.is_empty() {
        return None;
    }
    let n = values.len() as f64;
    let mut v = values.to_vec();
    v2xw_core::math::sort_total_order(&mut v);
    let mean = v2xw_core::math::sum_ordered(v.iter().copied()) / n;
    if values.len() < 2 {
        return Some((mean, None));
    }
    let mut sq: Vec<f64> = v.iter().map(|x| (x - mean) * (x - mean)).collect();
    v2xw_core::math::sort_total_order(&mut sq);
    let var = v2xw_core::math::sum_ordered(sq.iter().copied()) / (n - 1.0);
    let half = t975(values.len() - 1) * v2xw_core::math::sqrt(var / n);
    Some((mean, Some(half)))
}

/// The policies in first-seen order, each with its rows.
fn by_policy(rows: &[PolicyRow]) -> Vec<(&str, Vec<&PolicyRow>)> {
    let mut out: Vec<(&str, Vec<&PolicyRow>)> = Vec::new();
    for r in rows {
        match out.iter_mut().find(|(p, _)| *p == r.policy) {
            Some((_, v)) => v.push(r),
            None => out.push((r.policy.as_str(), vec![r])),
        }
    }
    out
}

fn cell(x: Option<f64>) -> String {
    x.map_or(String::new(), |v| format!("{v:.3}"))
}

/// The CSV: per policy, its runs (`kind` `run`), then its `mean` and `ci95` rows (the
/// half-width of Student's t interval over the seeds).
#[must_use]
pub fn csv(rows: &[PolicyRow]) -> String {
    let mut s = String::from("policy,kind,seed");
    if let Some(r) = rows.first() {
        for (name, _) in columns(r) {
            let _ = write!(s, ",{name}");
        }
    }
    s.push('\n');
    for (policy, runs) in by_policy(rows) {
        for r in &runs {
            let _ = write!(s, "{policy},run,{}", r.seed);
            for (_, v) in columns(r) {
                let _ = write!(s, ",{}", cell(v));
            }
            s.push('\n');
        }
        let width = columns(runs[0]).len();
        let mut means = Vec::with_capacity(width);
        let mut halves = Vec::with_capacity(width);
        for i in 0..width {
            let vals: Vec<f64> = runs.iter().filter_map(|r| columns(r)[i].1).collect();
            let sm = summarise(&vals);
            means.push(sm.map(|x| x.0));
            halves.push(sm.and_then(|x| x.1));
        }
        let _ = write!(s, "{policy},mean,");
        for m in &means {
            let _ = write!(s, ",{}", cell(*m));
        }
        s.push('\n');
        let _ = write!(s, "{policy},ci95,");
        for h in &halves {
            let _ = write!(s, ",{}", cell(*h));
        }
        s.push('\n');
    }
    s
}

/// The Markdown summary: one line per policy, mean ± 95 % half-width.
#[must_use]
pub fn markdown(rows: &[PolicyRow]) -> String {
    let mut s = String::from(
        "# MA policy family\n\n\
         One full run per policy per seed: feedback included (a revoked device stops \
         sending, CRL delays apply, reputations change), unlike `v2xw ma-replay \
         --policies`, which replays one recorded run. Every point ran with \
         `security.scms.report_records: true`, set by the sweep. Ground truth \
         (`gt.device_exposure`, the truth join of `pki.report_fate`) is joined in this \
         report only, never by a policy. Intervals: Student's t, 95 %, over the seeds.\n\n\
         | policy | runs | attackers revoked | honest revoked | honest blocked / 10⁶ dev-h \
         | attack → decision median s | decision → CRL median s | CRL → first refusal \
         median s | reports processed | peak open cases |\n\
         |---|---|---|---|---|---|---|---|---|---|\n",
    );
    let pm = |vals: Vec<f64>| match summarise(&vals) {
        None => "–".to_string(),
        Some((m, None)) => format!("{m:.2}"),
        Some((m, Some(h))) => format!("{m:.2} ± {h:.2}"),
    };
    for (policy, runs) in by_policy(rows) {
        let col =
            |f: &dyn Fn(&PolicyRow) -> Option<f64>| pm(runs.iter().filter_map(|r| f(r)).collect());
        let _ = writeln!(
            s,
            "| {policy} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            runs.len(),
            col(&|r| Some(r.attackers_revoked as f64)),
            col(&|r| Some(r.honest_revoked as f64)),
            col(&|r| Some(r.honest_blocked_per_1e6_dh)),
            col(&|r| r.to_decision_s.0),
            col(&|r| r.to_published_s.0),
            col(&|r| r.to_refused_s.0),
            col(&|r| Some(r.reports_processed as f64)),
            col(&|r| Some(r.peak_open_cases as f64)),
        );
    }
    s
}

/// Whether `plan` is an MA policy comparison.
#[must_use]
pub fn is_family_sweep(plan: &crate::plan::ExperimentPlan) -> bool {
    plan.runs
        .iter()
        .any(|r| r.cell.values.contains_key(SWEEP_AXIS))
}

/// Builds the report from the finished runs of `plan` under `out` and writes both files.
/// Returns their names.
///
/// # Errors
/// A run's recording is missing or does not read, or a file cannot be written.
pub fn write(
    plan: &crate::plan::ExperimentPlan,
    done: &BTreeSet<String>,
    out: &Path,
) -> Result<Vec<String>> {
    let mut rows = Vec::new();
    for run in plan.runs.iter().filter(|r| done.contains(&r.run_id)) {
        let Some(policy) = run.cell.values.get(SWEEP_AXIS).and_then(|v| v.as_str()) else {
            continue;
        };
        let dir = out.join(crate::runner::RUNS_DIR).join(&run.run_id);
        let facts = RunFacts::read(&dir)?;
        rows.push(row(policy, &run.seed_hex(), &facts));
    }
    for (name, body) in [(CSV_FILE, csv(&rows)), (MD_FILE, markdown(&rows))] {
        let path = out.join(name);
        std::fs::write(&path, body)
            .map_err(|e| ExperimentError::io("cannot write the MA policy report", &path, e))?;
    }
    Ok(vec![CSV_FILE.to_string(), MD_FILE.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(node: u32, attacker: bool) -> DeviceExposure {
        DeviceExposure {
            t: 100_000_000_000,
            node: NodeId::new(node),
            la_link: format!("l{node}"),
            joined: 0,
            left: 3_600_000_000_000,
            attacker,
            attack_start: attacker.then_some(10_000_000_000),
        }
    }

    /// Decisions join through the device: two pseudonyms of one honest device are one
    /// honest revocation, an attacker's decision times its stages, and the open-case peak
    /// counts overlapping revocations.
    #[test]
    fn decisions_join_through_the_device() {
        let mut f = RunFacts {
            devices: vec![device(1, false), device(2, false), device(3, true)],
            ..RunFacts::default()
        };
        for (p, n) in [("pa", 1), ("pb", 1), ("pc", 3)] {
            f.subject_device.insert(p.to_string(), NodeId::new(n));
        }
        f.decisions = vec![
            (20_000_000_000, "pa".into()),
            (25_000_000_000, "pb".into()),
            (40_000_000_000, "pc".into()),
        ];
        for (id, stage, t) in [
            ("pa", "decision", 20),
            ("pb", "decision", 25),
            ("pc", "decision", 40),
            ("pa", "issued", 50),
            ("pc", "published", 70),
            ("pc", "enforced", 100),
        ] {
            f.stages
                .entry(id.into())
                .or_default()
                .insert(stage.into(), t * 1_000_000_000);
        }
        f.end = 200_000_000_000;
        let r = row("p", "s", &f);
        assert_eq!((r.honest_revoked, r.attackers_revoked), (1, 1));
        assert_eq!(r.to_decision_s.0, Some(30.0));
        assert_eq!(r.to_published_s.0, Some(30.0));
        assert_eq!(r.to_refused_s.0, Some(30.0));
        assert_eq!(r.peak_open_cases, 3);
        assert!((r.honest_device_hours - 2.0).abs() < 1e-9);
    }

    /// The CSV has one `run` row per (policy, seed) and one `mean` and one `ci95` row per
    /// policy; the interval is Student's.
    #[test]
    fn the_csv_summarises_each_policy() {
        let mut rows = Vec::new();
        for p in ["a", "b"] {
            for (seed, revoked) in [("1", 1.0), ("2", 3.0)] {
                let mut r = row(p, seed, &RunFacts::default());
                r.honest_blocked_per_1e6_dh = revoked;
                rows.push(r);
            }
        }
        let text = csv(&rows);
        let kind = |k: &str| {
            text.lines()
                .filter(|l| l.split(',').nth(1) == Some(k))
                .count()
        };
        assert_eq!((kind("run"), kind("mean"), kind("ci95")), (4, 2, 2));
        // mean 2, s = √2, half-width 12.706 · √2 / √2 = 12.706.
        let ci = text
            .lines()
            .find(|l| l.starts_with("a,ci95"))
            .expect("a ci95 row");
        assert!(ci.contains(",12.706,"), "{ci}");
        assert!(markdown(&rows).contains("| a | 2 |"));
    }
}
