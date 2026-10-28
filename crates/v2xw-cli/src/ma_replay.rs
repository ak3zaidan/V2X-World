//! `v2xw ma-replay`: an offline Misbehaviour Authority rule family over a finished run's
//! recorded reports, without re-simulating (paper plan 2026-10 §1.4 item E, experiment
//! P1-E7; shared later with Paper 3).
//!
//! # What a rule sees
//!
//! A rule reads [`MaView`] rows only: the `ma.received` channel (NODE), field for field
//! what the deciding MA read from each report it received or can learn through the
//! protocol (the reporter's and the accused's pseudonym certificates, the subtypes, the
//! evidence window and its secMark phase, the accused's PositionalAccuracy, the issuing
//! provider, and the LAs' "same device" link token). Rows the MA set aside (unverifiable
//! or time-implausible evidence) are dropped before any rule runs, as the MA drops them.
//! Ground truth (`gt.device_exposure` and the truth join of `pki.report_fate`) is read
//! into [`Truth`] and touched only by [`score`], after every rule has decided.
//! [`check_blind`] is the guard: renaming every identifier in the MA view through a
//! bijection must rename a rule's decisions the same way, which a rule that was handed
//! ground truth (keyed by the real identifiers) fails.
//!
//! # The family
//!
//! * `pseudonym-k`: revoke a pseudonym certificate once `k` distinct reporters accused it.
//! * `device-k`: revoke a device (by its LA link) once `k` distinct reporters accused any
//!   of its pseudonyms within a sliding window `w`.
//! * `catch`: accuracy-aware weighting in the spirit of CaTch (Kamel, Ben Jemaa, Kaiser,
//!   Urien, "CaTch: a confidence range tolerant misbehavior detection approach", IEEE
//!   WCNC 2019; re-implemented from the paper's idea, no code): each report counts the
//!   share of its position deviation that the accused's own claimed confidence cannot
//!   explain, `clamp(1 − 2a / (excess · τ), 0, 1)` with `a` the PositionalAccuracy and
//!   `τ` = 1 m (SM-MBR gpsDrift); a non-position check counts 1. Revoke a device when its
//!   distinct reporters' factors sum to `θ` within `w`.
//! * `cluster`: cross-vehicle clustering on fields the MA already receives. Each report
//!   falls in a class (secMark phase locked within 1 ms of the 50 ms epoch or free;
//!   accuracy band; issuer). When `c` or more distinct devices of one class are accused
//!   within `w`, the class looks like a device population with a systematic benign
//!   trigger, and a device in it needs `m·k` reporters instead of `k`.
//!
//! A parameterisation is `sound` when it needs corroboration (`k ≥ 2`, `θ ≥ 2`): a single
//! report never revokes. `pseudonym-k` with `k = 1` is kept as the unsound baseline.
//!
//! # Output
//!
//! Per rule and parameter: honest devices blocked per 10⁶ device-hours and the attacker's
//! time to act (attack start to revocation decision), as `ma-replay.csv` in the run
//! directory and a short table.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::path::Path;

use v2xw_core::ids::NodeId;
use v2xw_core::time::SimTime;
use v2xw_engine::report_records::{DeviceExposure, MaReceived, ReportFate};

use crate::{CliError, Result};

/// One report as the MA holds it: [`MaReceived`] with no set-aside reason.
pub type MaView = MaReceived;

/// What a rule revokes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    /// One pseudonym certificate (its digest).
    Pseudonym(String),
    /// A device, by its LA link token.
    Device(String),
}

/// One revocation decision.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Revocation {
    /// The decision's instant (the report that tipped it).
    pub t: SimTime,
    /// What is revoked.
    pub target: Target,
}

/// An MA decision rule: MA-visible rows in, revocations out.
pub trait MaRule {
    /// The family name.
    fn name(&self) -> &'static str;
    /// The parameters, as `key=value` words.
    fn params(&self) -> String;
    /// Whether it needs corroboration (no single report revokes).
    fn sound(&self) -> bool;
    /// The decisions on `log` (accepted rows, in arrival order).
    fn decide(&self, log: &[MaView]) -> Vec<Revocation>;
}

/// Distinct reporters within a sliding window, per key.
#[derive(Default)]
struct Window {
    rows: VecDeque<(SimTime, String, f64)>,
}

impl Window {
    /// Adds a row and returns the summed weight of distinct reporters inside `w`
    /// (each reporter's largest weight).
    fn add(&mut self, t: SimTime, reporter: &str, weight: f64, w: SimTime) -> f64 {
        self.rows.push_back((t, reporter.to_string(), weight));
        while self
            .rows
            .front()
            .is_some_and(|(s, ..)| t.saturating_sub(*s) > w)
        {
            self.rows.pop_front();
        }
        let mut best: BTreeMap<&str, f64> = BTreeMap::new();
        for (_, r, x) in &self.rows {
            let e = best.entry(r.as_str()).or_insert(0.0);
            *e = e.max(*x);
        }
        best.values().sum()
    }
}

const S: SimTime = 1_000_000_000;

/// `pseudonym-k`.
pub struct PseudonymK {
    /// Distinct reporters needed.
    pub k: usize,
}

impl MaRule for PseudonymK {
    fn name(&self) -> &'static str {
        "pseudonym-k"
    }
    fn params(&self) -> String {
        format!("k={}", self.k)
    }
    fn sound(&self) -> bool {
        self.k >= 2
    }
    fn decide(&self, log: &[MaView]) -> Vec<Revocation> {
        let mut seen: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        let mut out = Vec::new();
        let mut done = BTreeSet::new();
        for r in log {
            let s = seen.entry(r.subject_cert.as_str()).or_default();
            s.insert(r.reporter_cert.as_str());
            if s.len() >= self.k && done.insert(r.subject_cert.clone()) {
                out.push(Revocation {
                    t: r.t,
                    target: Target::Pseudonym(r.subject_cert.clone()),
                });
            }
        }
        out
    }
}

/// `device-k`.
pub struct DeviceK {
    /// Distinct reporters needed.
    pub k: usize,
    /// The window, s.
    pub w_s: u64,
}

impl MaRule for DeviceK {
    fn name(&self) -> &'static str {
        "device-k"
    }
    fn params(&self) -> String {
        format!("k={} w={}s", self.k, self.w_s)
    }
    fn sound(&self) -> bool {
        self.k >= 2
    }
    fn decide(&self, log: &[MaView]) -> Vec<Revocation> {
        weighted(log, self.w_s, self.k as f64, |_| 1.0)
    }
}

/// Revokes a device once the distinct reporters' weights within `w_s` reach `need`.
fn weighted(
    log: &[MaView],
    w_s: u64,
    need: f64,
    weight: impl Fn(&MaView) -> f64,
) -> Vec<Revocation> {
    let mut win: BTreeMap<&str, Window> = BTreeMap::new();
    let mut done = BTreeSet::new();
    let mut out = Vec::new();
    for r in log {
        let sum = win.entry(r.la_link.as_str()).or_default().add(
            r.t,
            &r.reporter_cert,
            weight(r),
            w_s * S,
        );
        if sum + 1e-9 >= need && done.insert(r.la_link.clone()) {
            out.push(Revocation {
                t: r.t,
                target: Target::Device(r.la_link.clone()),
            });
        }
    }
    out
}

/// The CaTch-style confidence factor of one report (see the module header).
#[must_use]
pub fn catch_factor(r: &MaView) -> f64 {
    let positional = r
        .subtypes
        .first()
        .is_some_and(|c| c.starts_with("position"));
    if !positional {
        return 1.0;
    }
    // The deviation in metres: the excess over SM-MBR's gpsDrift τ = 1 m.
    let tau_m = 1.0;
    let d = r.excess.max(0.0) * tau_m;
    if d <= 0.0 {
        return 0.0;
    }
    (1.0 - 2.0 * r.pos_accuracy_m.max(0.0) / d).clamp(0.0, 1.0)
}

/// `catch`.
pub struct Catch {
    /// The summed confidence needed.
    pub theta: f64,
    /// The window, s.
    pub w_s: u64,
}

impl MaRule for Catch {
    fn name(&self) -> &'static str {
        "catch"
    }
    fn params(&self) -> String {
        format!("theta={} w={}s", self.theta, self.w_s)
    }
    fn sound(&self) -> bool {
        self.theta >= 2.0
    }
    fn decide(&self, log: &[MaView]) -> Vec<Revocation> {
        weighted(log, self.w_s, self.theta, catch_factor)
    }
}

/// A report's class for [`Cluster`]: secMark phase, accuracy band and issuer.
#[must_use]
pub fn class_of(r: &MaView) -> String {
    let phase = if r.secmark_phase_ms <= 1 || r.secmark_phase_ms >= 49 {
        "locked"
    } else {
        "free"
    };
    let band = match r.pos_accuracy_m {
        a if a < 1.0 => "acc<1m",
        a if a < 3.0 => "acc1-3m",
        a if a < 10.0 => "acc3-10m",
        _ => "acc>=10m",
    };
    format!("{phase}/{band}/{}", r.issuer)
}

/// `cluster`.
pub struct Cluster {
    /// Distinct reporters needed outside a crowded class.
    pub k: usize,
    /// Distinct accused devices that make a class crowded.
    pub c: usize,
    /// The multiplier on `k` inside a crowded class.
    pub m: usize,
    /// The window, s.
    pub w_s: u64,
}

impl MaRule for Cluster {
    fn name(&self) -> &'static str {
        "cluster"
    }
    fn params(&self) -> String {
        format!("k={} c={} m={} w={}s", self.k, self.c, self.m, self.w_s)
    }
    fn sound(&self) -> bool {
        self.k >= 2
    }
    fn decide(&self, log: &[MaView]) -> Vec<Revocation> {
        let w = self.w_s * S;
        let mut classes: BTreeMap<String, VecDeque<(SimTime, String)>> = BTreeMap::new();
        let mut win: BTreeMap<&str, Window> = BTreeMap::new();
        let mut done = BTreeSet::new();
        let mut out = Vec::new();
        for r in log {
            let q = classes.entry(class_of(r)).or_default();
            q.push_back((r.t, r.la_link.clone()));
            while q.front().is_some_and(|(s, _)| r.t.saturating_sub(*s) > w) {
                q.pop_front();
            }
            let crowd: BTreeSet<&str> = q.iter().map(|(_, l)| l.as_str()).collect();
            let need = if crowd.len() >= self.c {
                self.k * self.m
            } else {
                self.k
            };
            let n = win
                .entry(r.la_link.as_str())
                .or_default()
                .add(r.t, &r.reporter_cert, 1.0, w);
            if n + 1e-9 >= need as f64 && done.insert(r.la_link.clone()) {
                out.push(Revocation {
                    t: r.t,
                    target: Target::Device(r.la_link.clone()),
                });
            }
        }
        out
    }
}

/// The rule family and its parameter grid.
#[must_use]
pub fn family() -> Vec<Box<dyn MaRule>> {
    let mut v: Vec<Box<dyn MaRule>> = Vec::new();
    for k in [1, 2, 3, 5, 10] {
        v.push(Box::new(PseudonymK { k }));
    }
    for w_s in [60, 600, 3_600, 86_400] {
        for k in [2, 3, 5, 10, 20] {
            v.push(Box::new(DeviceK { k, w_s }));
        }
    }
    for w_s in [600, 3_600] {
        for theta in [2.0, 3.0, 5.0, 10.0] {
            v.push(Box::new(Catch { theta, w_s }));
        }
    }
    for k in [3, 5] {
        for c in [3, 10] {
            v.push(Box::new(Cluster {
                k,
                c,
                m: 3,
                w_s: 3_600,
            }));
        }
    }
    v
}

/// Ground truth, for the scoring step only.
#[derive(Debug, Clone, Default)]
pub struct Truth {
    /// Every SCMS vehicle's exposure.
    pub devices: Vec<DeviceExposure>,
    /// Which device stands behind each accused pseudonym digest.
    pub subject_device: BTreeMap<String, NodeId>,
}

/// One rule-and-parameter row of the result.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Row {
    /// The family.
    pub rule: String,
    /// Its parameters.
    pub params: String,
    /// Whether it needs corroboration.
    pub sound: bool,
    /// Revocations decided.
    pub decisions: usize,
    /// Honest devices in the run.
    pub honest_devices: usize,
    /// Their summed time in the run, h.
    pub honest_device_hours: f64,
    /// Distinct honest devices revoked.
    pub honest_blocked: usize,
    /// Per 10⁶ honest device-hours.
    pub honest_blocked_per_1e6_dh: f64,
    /// Attackers in the run.
    pub attackers: usize,
    /// Of them revoked.
    pub attackers_revoked: usize,
    /// Median attack start to revocation, s (`None`: none revoked).
    pub tta_median_s: Option<f64>,
    /// Longest, s.
    pub tta_max_s: Option<f64>,
    /// The attack budget: summed over attackers, the seconds from attack start to the
    /// first revocation decision, or to the attacker's exit (run end) when none came
    /// (P1-E8). Censored at the exit, so a lower bound for an unrevoked attacker.
    pub attack_budget_s: f64,
    /// Per attacker (by its opaque `la_link`, which pairs across runs of one seed): attack
    /// start, decision (`None`: not revoked) and budget, s.
    pub per_attacker: Vec<(String, f64, Option<f64>, f64)>,
}

/// Scores one rule's decisions against ground truth.
#[must_use]
pub fn score(rule: &dyn MaRule, decisions: &[Revocation], truth: &Truth) -> Row {
    let by_link: BTreeMap<&str, &DeviceExposure> = truth
        .devices
        .iter()
        .map(|d| (d.la_link.as_str(), d))
        .collect();
    let by_node: BTreeMap<NodeId, &DeviceExposure> =
        truth.devices.iter().map(|d| (d.node, d)).collect();
    let mut first: BTreeMap<NodeId, SimTime> = BTreeMap::new();
    for d in decisions {
        let node = match &d.target {
            Target::Device(l) => by_link.get(l.as_str()).map(|e| e.node),
            Target::Pseudonym(p) => truth.subject_device.get(p).copied(),
        };
        if let Some(n) = node {
            let e = first.entry(n).or_insert(d.t);
            *e = (*e).min(d.t);
        }
    }
    let honest: Vec<&DeviceExposure> = truth.devices.iter().filter(|d| !d.attacker).collect();
    let hours: f64 = honest
        .iter()
        .map(|d| d.left.saturating_sub(d.joined) as f64 / 3.6e12)
        .sum();
    let blocked = first
        .keys()
        .filter(|n| by_node.get(n).is_some_and(|d| !d.attacker))
        .count();
    let mut tta: Vec<f64> = first
        .iter()
        .filter_map(|(n, t)| {
            let d = by_node.get(n)?;
            let start = d.attack_start.filter(|_| d.attacker)?;
            Some((*t as f64 - start as f64) / 1e9)
        })
        .collect();
    tta.sort_by(f64::total_cmp);
    let attackers = truth.devices.iter().filter(|d| d.attacker).count();
    let per_attacker: Vec<(String, f64, Option<f64>, f64)> = truth
        .devices
        .iter()
        .filter(|d| d.attacker)
        .map(|d| {
            let start = d.attack_start.unwrap_or(d.joined);
            let decided = first.get(&d.node).copied();
            let end = decided.unwrap_or(d.left).min(d.left).max(start);
            (
                d.la_link.clone(),
                start as f64 / 1e9,
                decided.map(|t| t as f64 / 1e9),
                (end - start) as f64 / 1e9,
            )
        })
        .collect();
    Row {
        rule: rule.name().to_string(),
        params: rule.params(),
        sound: rule.sound(),
        decisions: decisions.len(),
        honest_devices: honest.len(),
        honest_device_hours: hours,
        honest_blocked: blocked,
        honest_blocked_per_1e6_dh: if hours > 0.0 {
            blocked as f64 / hours * 1e6
        } else {
            0.0
        },
        attackers,
        attackers_revoked: tta.len(),
        tta_median_s: (!tta.is_empty()).then(|| tta[tta.len() / 2]),
        tta_max_s: tta.last().copied(),
        attack_budget_s: per_attacker.iter().map(|a| a.3).sum(),
        per_attacker,
    }
}

/// Every rule of `rules` decided on `log`, then scored against `truth`.
#[must_use]
pub fn replay(rules: &[Box<dyn MaRule>], log: &[MaView], truth: &Truth) -> Vec<Row> {
    let accepted: Vec<MaView> = log
        .iter()
        .filter(|r| r.set_aside.is_none())
        .cloned()
        .collect();
    rules
        .iter()
        .map(|r| {
            let d = r.decide(&accepted);
            score(r.as_ref(), &d, truth)
        })
        .collect()
}

/// A keyed renaming of one identifier: a bijection on the identifiers in a run.
fn rename(s: &str) -> String {
    v2xw_core::hash::sha256_hex(format!("v2xw/ma-replay/decoy/{s}").as_bytes())[..16].to_string()
}

/// The blindness guard: renames every identifier of the MA view (reports, reporters,
/// accused certificates, LA links) and checks each rule's decisions are renamed the same
/// way. A rule that decides from MA-visible fields only passes; one that was handed
/// ground truth keyed by the real identifiers decides differently on the renamed log.
/// Returns the rules that failed, with what differed.
#[must_use]
pub fn check_blind(rules: &[Box<dyn MaRule>], log: &[MaView]) -> Vec<String> {
    let decoy: Vec<MaView> = log
        .iter()
        .map(|r| MaView {
            report_id: rename(&r.report_id),
            reporter_cert: rename(&r.reporter_cert),
            subject_cert: rename(&r.subject_cert),
            la_link: rename(&r.la_link),
            ..r.clone()
        })
        .collect();
    let mut bad = Vec::new();
    for rule in rules {
        let mut want: Vec<Revocation> = rule
            .decide(log)
            .into_iter()
            .map(|d| Revocation {
                t: d.t,
                target: match d.target {
                    Target::Pseudonym(p) => Target::Pseudonym(rename(&p)),
                    Target::Device(l) => Target::Device(rename(&l)),
                },
            })
            .collect();
        let mut got = rule.decide(&decoy);
        want.sort();
        got.sort();
        if want != got {
            bad.push(format!(
                "{} {}: {} decisions on the log, {} on the renamed log",
                rule.name(),
                rule.params(),
                want.len(),
                got.len()
            ));
        }
    }
    bad
}

/// Reads the MA view and the truth from a run directory's `recording.mcap`.
///
/// # Errors
/// The recording cannot be opened or a record does not parse.
pub fn load(dir: &Path) -> Result<(Vec<MaView>, Truth)> {
    let path = dir.join("recording.mcap");
    let mut reader = v2xw_record::reader::Reader::open(&path)?;
    let parse = |what: &'static str, j: &[u8]| -> Result<serde_json::Value> {
        serde_json::from_slice(j).map_err(|source| CliError::Json { what, source })
    };
    let mut log: Vec<MaView> = Vec::new();
    for r in reader.records(Some("ma.received"))? {
        let v = parse("ma.received", &r.json)?;
        log.push(serde_json::from_value(v).map_err(|source| CliError::Json {
            what: "ma.received",
            source,
        })?);
    }
    let mut truth = Truth::default();
    for r in reader.records(Some("gt.device_exposure"))? {
        let v = parse("gt.device_exposure", &r.json)?;
        truth
            .devices
            .push(serde_json::from_value(v).map_err(|source| CliError::Json {
                what: "gt.device_exposure",
                source,
            })?);
    }
    for r in reader.records(Some("pki.report_fate"))? {
        let v = parse("pki.report_fate", &r.json)?;
        let f: ReportFate = serde_json::from_value(v).map_err(|source| CliError::Json {
            what: "pki.report_fate",
            source,
        })?;
        if let Some(n) = f.truth.subject_node {
            truth.subject_device.insert(f.subject, n);
        }
    }
    log.sort_by(|a, b| (a.t, &a.report_id).cmp(&(b.t, &b.report_id)));
    Ok((log, truth))
}

/// The CSV of `rows`.
#[must_use]
pub fn csv(rows: &[Row]) -> String {
    let mut s = String::from(
        "rule,params,sound,decisions,honest_devices,honest_device_hours,honest_blocked,\
         honest_blocked_per_1e6_dh,attackers,attackers_revoked,tta_median_s,tta_max_s,\
         attack_budget_s\n",
    );
    let o = |x: Option<f64>| x.map_or(String::new(), |v| format!("{v:.1}"));
    for r in rows {
        let _ = writeln!(
            s,
            "{},{},{},{},{},{:.3},{},{:.1},{},{},{},{},{:.1}",
            r.rule,
            r.params,
            r.sound,
            r.decisions,
            r.honest_devices,
            r.honest_device_hours,
            r.honest_blocked,
            r.honest_blocked_per_1e6_dh,
            r.attackers,
            r.attackers_revoked,
            o(r.tta_median_s),
            o(r.tta_max_s),
            r.attack_budget_s
        );
    }
    s
}

/// The per-attacker CSV of `rows` (`ma-replay-attackers.csv`): one line per rule,
/// parameter and attacker, for paired comparisons across runs of one seed (P1-E8).
#[must_use]
pub fn attackers_csv(rows: &[Row]) -> String {
    let mut s = String::from("rule,params,attacker,attack_start_s,revoked_s,budget_s\n");
    for r in rows {
        for (a, start, decided, budget) in &r.per_attacker {
            let _ = writeln!(
                s,
                "{},{},{a},{start:.1},{},{budget:.1}",
                r.rule,
                r.params,
                decided.map_or(String::new(), |t| format!("{t:.1}"))
            );
        }
    }
    s
}

/// A short table of `rows`.
#[must_use]
pub fn table(rows: &[Row], received: usize, accepted: usize) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "MA replay: {received} reports received, {accepted} accepted as evidence"
    );
    let _ = writeln!(
        s,
        "{:<12} {:<22} {:>5} {:>9} {:>14} {:>9} {:>9}",
        "rule", "params", "sound", "honest", "per 1e6 dev-h", "attackers", "tta med s"
    );
    for r in rows {
        let _ = writeln!(
            s,
            "{:<12} {:<22} {:>5} {:>9} {:>14.1} {:>9} {:>9}",
            r.rule,
            r.params,
            if r.sound { "yes" } else { "no" },
            r.honest_blocked,
            r.honest_blocked_per_1e6_dh,
            format!("{}/{}", r.attackers_revoked, r.attackers),
            r.tta_median_s
                .map_or("-".to_string(), |v| format!("{v:.0}"))
        );
    }
    s
}

/// `v2xw ma-replay <dir>`: loads, checks blindness, replays the family, writes
/// `ma-replay.csv` beside the recording and returns the table.
///
/// # Errors
/// The recording cannot be read, the CSV cannot be written, or a rule is not blind.
pub fn run(dir: &Path) -> Result<String> {
    let (log, truth) = load(dir)?;
    let rules = family();
    let accepted: Vec<MaView> = log
        .iter()
        .filter(|r| r.set_aside.is_none())
        .cloned()
        .collect();
    let bad = check_blind(&rules, &accepted);
    if !bad.is_empty() {
        return Err(CliError::Agent(format!(
            "ma-replay: rules not blind to ground truth: {bad:?}"
        )));
    }
    let rows = replay(&rules, &log, &truth);
    let out = dir.join("ma-replay.csv");
    std::fs::write(&out, csv(&rows)).map_err(|source| CliError::Io {
        what: "cannot write",
        path: out.display().to_string(),
        source,
    })?;
    let out = dir.join("ma-replay-attackers.csv");
    std::fs::write(&out, attackers_csv(&rows)).map_err(|source| CliError::Io {
        what: "cannot write",
        path: out.display().to_string(),
        source,
    })?;
    Ok(table(&rows, log.len(), accepted.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(t_s: u64, reporter: &str, subject: &str, link: &str) -> MaView {
        MaView {
            t: t_s * S,
            report_id: format!("{reporter}-{subject}-{t_s}"),
            ma: 0,
            reporter_cert: reporter.into(),
            subject_cert: subject.into(),
            subtypes: vec!["positionJump".into()],
            generation_time: t_s * S,
            evidence_start: t_s * S,
            evidence_end: t_s * S,
            excess: 3.0,
            pos_accuracy_m: 0.5,
            secmark_phase_ms: 0,
            issuer: "provider-0".into(),
            la_link: link.into(),
            set_aside: None,
        }
    }

    fn device(node: u32, attacker: bool) -> DeviceExposure {
        DeviceExposure {
            t: 3_600 * S,
            node: NodeId::new(node),
            la_link: format!("dev{node}"),
            joined: 0,
            left: 3_600 * S,
            attacker,
            attack_start: attacker.then_some(100 * S),
        }
    }

    /// Honest devices only, with the sparse isolated false reports an epoch-locked
    /// fleet gives: each device accused at most once an hour, by one reporter.
    fn honest_world() -> (Vec<MaView>, Truth) {
        let mut log = Vec::new();
        let mut truth = Truth::default();
        for n in 0..50u32 {
            truth.devices.push(device(n, false));
            if n % 3 == 0 {
                let p = format!("p{n}");
                log.push(row(
                    60 + u64::from(n) * 7,
                    &format!("r{}", (n + 1) % 50),
                    &p,
                    &format!("dev{n}"),
                ));
                truth.subject_device.insert(p, NodeId::new(n));
            }
        }
        (log, truth)
    }

    #[test]
    fn with_honest_devices_only_every_sound_rule_blocks_no_one() {
        let (log, truth) = honest_world();
        let rows = replay(&family(), &log, &truth);
        for r in &rows {
            if r.sound {
                assert_eq!(
                    r.honest_blocked, 0,
                    "{} {} blocked honest devices",
                    r.rule, r.params
                );
            }
        }
        // The unsound baseline does block: the check can go red.
        let k1 = rows
            .iter()
            .find(|r| r.rule == "pseudonym-k" && r.params == "k=1")
            .unwrap();
        assert!(k1.honest_blocked > 0);
        assert!(rows.iter().all(|r| r.honest_device_hours > 49.0));
    }

    #[test]
    fn one_injected_attacker_is_revoked_by_the_k_threshold() {
        let (mut log, mut truth) = honest_world();
        truth.devices.push(device(99, true));
        truth.subject_device.insert("pa".into(), NodeId::new(99));
        for (i, rep) in ["r1", "r2", "r3", "r4", "r5"].iter().enumerate() {
            log.push(row(200 + i as u64 * 10, rep, "pa", "dev99"));
        }
        log.sort_by_key(|r| r.t);
        let rows = replay(&family(), &log, &truth);
        let k3 = rows
            .iter()
            .find(|r| r.rule == "pseudonym-k" && r.params == "k=3")
            .unwrap();
        assert_eq!(
            (k3.attackers, k3.attackers_revoked, k3.honest_blocked),
            (1, 1, 0)
        );
        // Attack start 100 s, third reporter at 220 s.
        assert_eq!(k3.tta_median_s, Some(120.0));
        let dk = rows
            .iter()
            .find(|r| r.rule == "device-k" && r.params == "k=5 w=600s")
            .unwrap();
        assert_eq!(dk.attackers_revoked, 1);
        assert_eq!(dk.tta_median_s, Some(140.0));
    }

    #[test]
    fn the_attack_budget_runs_to_the_decision_or_the_exit() {
        let (mut log, mut truth) = honest_world();
        truth.devices.push(device(99, true));
        truth.subject_device.insert("pa".into(), NodeId::new(99));
        for (i, rep) in ["r1", "r2", "r3", "r4", "r5"].iter().enumerate() {
            log.push(row(200 + i as u64 * 10, rep, "pa", "dev99"));
        }
        log.sort_by_key(|r| r.t);
        let rows = replay(&family(), &log, &truth);
        let get = |rule: &str, params: &str| {
            rows.iter()
                .find(|r| r.rule == rule && r.params == params)
                .unwrap()
        };
        // Revoked at the third reporter (220 s): 120 s after the 100 s attack start.
        assert_eq!(get("pseudonym-k", "k=3").attack_budget_s, 120.0);
        // Never revoked: the whole exposure after the start, censored at the exit.
        let k10 = get("pseudonym-k", "k=10");
        assert_eq!(k10.attack_budget_s, 3_500.0);
        assert_eq!(k10.per_attacker[0].2, None);
        assert!(attackers_csv(&rows).contains("pseudonym-k,k=3,dev99,100.0,220.0,120.0"));
    }

    /// A free class crowded with honest accused devices, and one attacker accused by five
    /// reporters: locked, it is outside the crowd and `cluster` (k = 3) revokes it; under
    /// a free-class secMark phase it is inside, needs m·k = 9 and is not revoked.
    #[test]
    fn a_free_class_phase_buys_the_crowd_multiplier_under_cluster() {
        let budget = |phase: u32| {
            let mut log = Vec::new();
            let mut truth = Truth::default();
            for n in 0..6u32 {
                truth.devices.push(device(n, false));
                let mut r = row(
                    50 + u64::from(n),
                    &format!("h{n}"),
                    &format!("p{n}"),
                    &format!("dev{n}"),
                );
                r.secmark_phase_ms = 23;
                log.push(r);
            }
            truth.devices.push(device(99, true));
            for (i, rep) in ["r1", "r2", "r3", "r4", "r5"].iter().enumerate() {
                let mut r = row(200 + i as u64 * 10, rep, "pa", "dev99");
                r.secmark_phase_ms = phase;
                log.push(r);
            }
            let rows = replay(&family(), &log, &truth);
            let c = rows
                .iter()
                .find(|r| r.rule == "cluster" && r.params == "k=3 c=3 m=3 w=3600s")
                .unwrap();
            assert_eq!(c.honest_blocked, 0);
            c.attack_budget_s
        };
        assert_eq!(budget(0), 120.0);
        assert_eq!(budget(25), 3_500.0);
    }

    #[test]
    fn claimed_poor_accuracy_discounts_a_report_under_catch() {
        let mut r = row(1, "a", "b", "c");
        assert!((catch_factor(&r) - (1.0 - 1.0 / 3.0)).abs() < 1e-9);
        r.pos_accuracy_m = 5.0;
        assert_eq!(catch_factor(&r), 0.0);
        r.subtypes = vec!["implausibleSpeed".into()];
        assert_eq!(catch_factor(&r), 1.0);
    }

    /// A rule handed ground truth: it revokes whatever the truth says is an attacker,
    /// at its first report. It is what `check_blind` exists to catch.
    struct Peeking {
        attackers: BTreeSet<String>,
    }

    impl MaRule for Peeking {
        fn name(&self) -> &'static str {
            "peeking"
        }
        fn params(&self) -> String {
            String::new()
        }
        fn sound(&self) -> bool {
            true
        }
        fn decide(&self, log: &[MaView]) -> Vec<Revocation> {
            let mut done = BTreeSet::new();
            log.iter()
                .filter(|r| self.attackers.contains(&r.la_link) && done.insert(r.la_link.clone()))
                .map(|r| Revocation {
                    t: r.t,
                    target: Target::Device(r.la_link.clone()),
                })
                .collect()
        }
    }

    #[test]
    fn a_rule_fed_ground_truth_is_caught() {
        let (mut log, mut truth) = honest_world();
        truth.devices.push(device(99, true));
        for (i, rep) in ["r1", "r2", "r3"].iter().enumerate() {
            log.push(row(200 + i as u64, rep, "pa", "dev99"));
        }
        assert!(
            check_blind(&family(), &log).is_empty(),
            "a family rule is not blind"
        );
        let attackers = truth
            .devices
            .iter()
            .filter(|d| d.attacker)
            .map(|d| d.la_link.clone())
            .collect();
        let peeking: Vec<Box<dyn MaRule>> = vec![Box::new(Peeking { attackers })];
        let bad = check_blind(&peeking, &log);
        assert_eq!(bad.len(), 1, "the peeking rule passed: {bad:?}");
    }

    #[test]
    fn set_aside_reports_are_not_evidence() {
        let mut log = Vec::new();
        let mut truth = Truth::default();
        truth.devices.push(device(1, false));
        for rep in ["r1", "r2", "r3", "r4"] {
            let mut r = row(10, rep, "p1", "dev1");
            r.set_aside = Some("evidence-unverifiable".into());
            log.push(r);
        }
        truth.subject_device.insert("p1".into(), NodeId::new(1));
        let rows = replay(&family(), &log, &truth);
        assert!(rows.iter().all(|r| r.decisions == 0));
    }
}
