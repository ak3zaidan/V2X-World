//! Running checks: the shipped variant and every fault, and the verdict of each row.

use serde::Serialize;

use crate::{Check, Group, Lab, Outcome, Variant};

/// One fault run of a check.
#[derive(Debug, Clone, Serialize)]
pub struct FaultRun {
    /// What was broken.
    pub fault: String,
    /// What the check saw, or why it could not run.
    pub outcome: Result<Outcome, String>,
}

/// A row's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// Held, in band, and some fault turned it red.
    Pass,
    /// Held, in band, but no fault turned it red: the check is not proved able to fail.
    NotProved,
    /// Held and out of band.
    Fail,
    /// Reported only.
    Reported,
}

impl Verdict {
    /// A stable label.
    pub const fn label(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::NotProved => "pass, not proved",
            Verdict::Fail => "FAIL",
            Verdict::Reported => "reported",
        }
    }
}

/// Everything one check produced.
#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    /// The check's id.
    pub id: String,
    /// Its group.
    pub group: Group,
    /// What it proves.
    pub title: String,
    /// How it is measured.
    pub procedure: String,
    /// The shipped run.
    pub shipped: Result<Outcome, String>,
    /// The fault runs, in the check's order.
    pub faults: Vec<FaultRun>,
}

impl CheckResult {
    /// The faults (1-based labels, as the report prints them) under which row `metric`
    /// left its band.
    pub fn proved_by(&self, metric: &str) -> Vec<usize> {
        self.faults
            .iter()
            .enumerate()
            .filter(|(_, f)| match &f.outcome {
                Ok(o) => o.row(metric).is_some_and(|r| r.tested() && !r.passes()),
                Err(_) => false,
            })
            .map(|(k, _)| k + 1)
            .collect()
    }

    /// Each shipped row with its verdict.
    pub fn verdicts(&self) -> Vec<(&crate::Row, Verdict)> {
        let Ok(o) = &self.shipped else {
            return Vec::new();
        };
        o.rows
            .iter()
            .map(|r| {
                let v = if !r.tested() {
                    Verdict::Reported
                } else if !r.passes() {
                    Verdict::Fail
                } else if self.proved_by(&r.metric).is_empty() {
                    Verdict::NotProved
                } else {
                    Verdict::Pass
                };
                (r, v)
            })
            .collect()
    }

    /// True if the shipped run errored or a held row failed.
    pub fn failed(&self) -> bool {
        self.shipped.is_err() || self.verdicts().iter().any(|(_, v)| *v == Verdict::Fail)
    }
}

/// Runs `check`: shipped first, then each fault.
pub fn run_check(lab: &mut Lab, check: &Check) -> CheckResult {
    eprintln!("check {} — {}", check.id, check.title);
    let shipped = (check.run)(lab, Variant::Shipped);
    match &shipped {
        Ok(o) => {
            for r in &o.rows {
                eprintln!(
                    "    {:<58} {:>12.4} {:<18} {}",
                    r.metric,
                    r.measured,
                    r.band.map(|(a, b)| format!("[{a:.3}, {b:.3}]")).unwrap_or_else(|| "reported".into()),
                    if r.passes() { "" } else { "OUT" }
                );
            }
        }
        Err(e) => eprintln!("    error: {e}"),
    }
    let mut faults = Vec::new();
    for (k, f) in check.faults.iter().enumerate() {
        let outcome = (check.run)(lab, Variant::Fault(k));
        match &outcome {
            Ok(o) => {
                let red: Vec<&str> = o
                    .rows
                    .iter()
                    .filter(|r| r.tested() && !r.passes())
                    .map(|r| r.metric.as_str())
                    .collect();
                eprintln!("    fault {}: {f}\n      red: {red:?}", k + 1);
            }
            Err(e) => eprintln!("    fault {}: error {e}", k + 1),
        }
        faults.push(FaultRun {
            fault: (*f).to_string(),
            outcome,
        });
    }
    CheckResult {
        id: check.id.to_string(),
        group: check.group,
        title: check.title.to_string(),
        procedure: check.procedure.to_string(),
        shipped,
        faults,
    }
}

/// Runs every check whose id contains `filter` (all when `None`).
pub fn run_all(lab: &mut Lab, filter: Option<&str>) -> Vec<CheckResult> {
    crate::checks()
        .iter()
        .filter(|c| filter.is_none_or(|f| c.id.contains(f)))
        .map(|c| run_check(lab, c))
        .collect()
}
