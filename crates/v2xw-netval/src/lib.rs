//! `v2xw-netval` — the network validation suite.
//!
//! Every layer of the network simulation, held to its reference from outside the crate
//! that implements it: the physical layer to its published equations, distributions and
//! receiver tables; the access layer to IEEE 802.11 / ETSI EN 302 663, ETSI TS 102 687,
//! SAE J2945/1 and the 3GPP sidelink procedures; the network layer and the messages to
//! their header tables and to an independent decoder; the security envelope to OpenSSL;
//! and whole runs to conservation, metamorphic and determinism properties and to an
//! independent reference implementation.
//!
//! # The contract every check keeps
//!
//! A [`Check`] runs twice. In [`Mode::Honest`] it measures the model as shipped and must
//! pass. In [`Mode::Faulted`] it injects a named, realistic fault — a rounded constant, a
//! misread table row, a backoff counter that restarts instead of freezing — and must
//! **fail**. A check that passes with the fault in place cannot tell a right model from a
//! wrong one, and the suite reports it as such rather than as a pass. That is run on every
//! invocation, not once, so a check that loses its teeth in a later edit shows up in the
//! next report.
//!
//! The faults are injected at the boundary between the model and the check (a model built
//! with a wrong parameter, a value perturbed the way the named defect would perturb it),
//! not by editing the crates under test, so the suite can prove its own sensitivity
//! without a build per fault.
//!
//! `cargo run -p v2xw-netval --bin netval-report` writes `docs/validation/network.md`.

#![forbid(unsafe_code)]

pub mod access;
pub mod ctx;
pub mod differential;
pub mod e2e;
pub mod net;
pub mod phy;
pub mod report;
pub mod sec;
pub mod stats;

use std::panic::{AssertUnwindSafe, catch_unwind};

/// The layer a check belongs to, in report order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Layer {
    /// Propagation, fading, error models, noise and link-budget arithmetic.
    Physical,
    /// 802.11p EDCA, congestion control, the sidelink schedulers.
    Access,
    /// WSMP, GeoNetworking, fragmentation, the messages and their identifiers.
    Network,
    /// IEEE 1609.2 structures, signatures, certificates, revocation.
    Security,
    /// Whole runs on the shipped scenarios.
    EndToEnd,
}

impl Layer {
    /// Every layer, in report order.
    pub const ALL: [Layer; 5] = [
        Layer::Physical,
        Layer::Access,
        Layer::Network,
        Layer::Security,
        Layer::EndToEnd,
    ];

    /// The heading the report uses.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Layer::Physical => "1. Physical layer",
            Layer::Access => "2. Access layer",
            Layer::Network => "3. Network, transport and messages",
            Layer::Security => "4. Security",
            Layer::EndToEnd => "5. End to end",
        }
    }
}

/// Which of its two runs a check is making.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The model as shipped.
    Honest,
    /// The model with the check's named fault injected.
    Faulted,
}

impl Mode {
    /// True in the faulted run.
    #[must_use]
    pub const fn faulted(self) -> bool {
        matches!(self, Mode::Faulted)
    }
}

/// What one run of a check found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Within tolerance of the reference.
    Pass,
    /// Outside it.
    Fail,
    /// Could not run (a tool missing), with the reason in the measurement.
    Skip,
}

/// One run's result: the status and what was measured, in words and numbers.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// Pass, fail or skip.
    pub status: Status,
    /// The measurement, for the report.
    pub measured: String,
}

impl Outcome {
    /// Pass when `pass`, fail otherwise.
    pub fn judge(pass: bool, measured: impl Into<String>) -> Self {
        Self {
            status: if pass { Status::Pass } else { Status::Fail },
            measured: measured.into(),
        }
    }

    /// The check could not run.
    pub fn skip(reason: impl Into<String>) -> Self {
        Self {
            status: Status::Skip,
            measured: reason.into(),
        }
    }

    /// A list of sub-results folded into one: fails if any failed, and the measurement
    /// names the worst.
    pub fn all(parts: Vec<(bool, String)>) -> Self {
        let failed: Vec<&String> = parts.iter().filter(|p| !p.0).map(|p| &p.1).collect();
        if failed.is_empty() {
            let summary = parts
                .iter()
                .map(|p| p.1.as_str())
                .collect::<Vec<_>>()
                .join("; ");
            Outcome::judge(true, summary)
        } else {
            Outcome::judge(
                false,
                format!(
                    "{} of {} failed: {}",
                    failed.len(),
                    parts.len(),
                    failed
                        .iter()
                        .take(4)
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
            )
        }
    }
}

/// How long a check takes, so a quick run can leave out the whole-run checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cost {
    /// Milliseconds to a few seconds.
    Fast,
    /// Runs the engine.
    Slow,
}

/// One validation check.
#[derive(Clone, Copy)]
pub struct Check {
    /// Stable identifier, `LAYER-NN`.
    pub id: &'static str,
    /// The layer.
    pub layer: Layer,
    /// What is checked, in one line.
    pub title: &'static str,
    /// The reference it is held to: standard and clause, paper and equation, or the
    /// independent implementation.
    pub reference: &'static str,
    /// The tolerance, and why it is that wide.
    pub tolerance: &'static str,
    /// The fault the faulted run injects.
    pub fault: &'static str,
    /// Fast or slow.
    pub cost: Cost,
    /// The check itself.
    pub run: fn(Mode) -> Outcome,
}

/// A check's two runs and what they mean together.
#[derive(Debug, Clone)]
pub struct Verdict {
    /// The check's id.
    pub id: &'static str,
    /// The honest run.
    pub honest: Outcome,
    /// The faulted run.
    pub faulted: Outcome,
}

impl Verdict {
    /// The honest run passed and the faulted run failed: the model meets its reference
    /// and the check is able to say when it does not.
    #[must_use]
    pub fn valid(&self) -> bool {
        self.honest.status == Status::Pass && self.faulted.status == Status::Fail
    }

    /// The model does not meet its reference.
    #[must_use]
    pub fn defect(&self) -> bool {
        self.honest.status == Status::Fail
    }

    /// The check passed with its fault injected: it cannot fail, so it proves nothing.
    #[must_use]
    pub fn toothless(&self) -> bool {
        self.honest.status == Status::Pass && self.faulted.status == Status::Pass
    }

    /// Either run was skipped.
    #[must_use]
    pub fn skipped(&self) -> bool {
        self.honest.status == Status::Skip || self.faulted.status == Status::Skip
    }

    /// One word for the report.
    #[must_use]
    pub fn label(&self) -> &'static str {
        if self.skipped() {
            "SKIP"
        } else if self.defect() {
            "FAIL"
        } else if self.toothless() {
            "CANNOT FAIL"
        } else {
            "PASS"
        }
    }
}

/// Runs one mode of a check, turning a panic into a failure with its message.
fn run_mode(check: &Check, mode: Mode) -> Outcome {
    match catch_unwind(AssertUnwindSafe(|| (check.run)(mode))) {
        Ok(o) => o,
        Err(e) => {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_else(|| "panic".to_string());
            Outcome::judge(false, format!("panicked: {msg}"))
        }
    }
}

/// Runs a check in both modes.
#[must_use]
pub fn run(check: &Check) -> Verdict {
    Verdict {
        id: check.id,
        honest: run_mode(check, Mode::Honest),
        faulted: run_mode(check, Mode::Faulted),
    }
}

/// Every check, in report order.
#[must_use]
pub fn all() -> Vec<Check> {
    let mut v = Vec::new();
    v.extend(phy::checks());
    v.extend(access::checks());
    v.extend(net::checks());
    v.extend(sec::checks());
    v.extend(e2e::checks());
    v.extend(differential::checks());
    v
}

/// The checks of one layer.
#[must_use]
pub fn of_layer(layer: Layer) -> Vec<Check> {
    all().into_iter().filter(|c| c.layer == layer).collect()
}

/// Runs one of the independent reference implementations in `reference/` with `python3 -I`,
/// feeding `input` as JSON on stdin and parsing JSON from stdout.
///
/// # Errors
/// A message when Python is not installed, the script fails, or its output is not JSON.
/// Checks turn it into [`Status::Skip`], never into a pass.
pub fn python(script: &str, input: &serde_json::Value) -> Result<serde_json::Value, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("reference")
        .join(script);
    let mut child = Command::new("python3")
        .arg("-I")
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("python3 not runnable: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("no stdin")?
        .write_all(input.to_string().as_bytes())
        .map_err(|e| format!("writing to {script}: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("waiting for {script}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{script} failed: {}",
            String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("")
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("{script} output: {e}"))
}

/// Relative difference `|a − b| / max(|b|, tiny)`.
#[must_use]
pub fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}
