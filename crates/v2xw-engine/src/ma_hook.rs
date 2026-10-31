//! An MA decision rule the host program supplies (08-settings-ux-and-gaps.md §7, piece B4).
//!
//! `security.scms.ma_policy` covers every rule the settings can say. A researcher with a
//! rule they cannot say writes it in code instead: the Python bindings adapt a Python
//! object to [`MaPolicyHook`] (`v2xw.run(..., ma_policy=obj)`), and [`crate::Engine`]
//! hands it the deployment's MA decisions through [`crate::Engine::attach_ma_policy`].
//! A scenario file never names code, so a hook exists only in a run its host program
//! started, and the manifest pins it (`ma_policy:<id>` with the hook's content hash).
//!
//! # What a hook sees and returns
//!
//! Each step in which a deciding MA receives new evidence, the hook gets one call with
//! that evidence as [`MaHookRow`]s, in the order the MA ingests it: the `ma.received`
//! columns (what the MA reads from the report and can learn through the linkage
//! authorities, never ground truth), the report's observation instant (the instant a
//! built-in policy dates its evidence by) and whether the reporter is trusted
//! infrastructure. Evidence the MA sets aside (an unverifiable signature, an implausible
//! evidence time) never reaches it, as it never reaches a built-in policy. The hook
//! returns [`MaVerdict`]s: a pseudonym to revoke, and the row whose evidence decided it.
//! A revoke is the MA's recommendation; the run carries it out exactly as a built-in
//! policy's, through the provider's decision mode (operator review or automatic), the
//! linkage authorities' "one case per device" and the CRL.
//!
//! # Which MAs it decides for
//!
//! Every provider's MA that would run the deployment-wide rule. A provider with its own
//! `ma_policy` keeps it. A scenario that already chooses the deployment-wide rule
//! (`detection.ma` or `security.scms.ma_policy`) is refused: two rules cannot both decide.
//!
//! # Determinism
//!
//! The hook is called at the same instants with the same rows on every run of a seed,
//! so a hook that is a function of its calls keeps the run reproducible. A hook that
//! fails returns an error; the MA then decides nothing for that call, the run goes on,
//! and [`MaHookStatus::error`] keeps the first failure for the host to raise, so a broken
//! hook cannot pass for a policy that decided nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use v2xw_core::manifest::{Manifest, PluginPin};
use v2xw_core::time::SimTime;

use crate::error::{EngineError, Result, ScenarioError};
use crate::report_records::MaReceived;
use crate::scenario::Scenario;

/// The prefix of a hook's id in the run report and the manifest's plug-in pins.
pub const ID_PREFIX: &str = "ma_policy:";

/// One report as a hook reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct MaHookRow {
    /// The `ma.received` row (`set_aside` always `None`).
    pub view: MaReceived,
    /// The report's observation instant (its detection time): the instant a built-in
    /// policy dates the evidence by, rather than its arrival `view.t`.
    pub observed: SimTime,
    /// Whether the reporter is trusted infrastructure (a roadside unit the MA trusts).
    pub reporter_infrastructure: bool,
}

/// One call: a deciding MA's new evidence of one step.
#[derive(Debug, Clone, Copy)]
pub struct MaHookCall<'a> {
    /// The step's instant.
    pub t: SimTime,
    /// The deciding MA's provider index, in deployment order.
    pub ma: u32,
    /// Its provider's name.
    pub provider: &'a str,
    /// The evidence, in ingest order.
    pub rows: &'a [MaHookRow],
}

/// One decision a hook returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaVerdict {
    /// The pseudonym certificate digest decided about.
    pub subject: String,
    /// `true` revokes; `false` is an explicit "not yet" and changes nothing.
    pub revoke: bool,
    /// The row whose evidence decided it. `None` takes the last row about `subject`.
    pub row: Option<usize>,
}

/// A decision rule supplied by the host program.
pub trait MaPolicyHook: Send {
    /// What the manifest pins: its id (`python:<module>.<class>`), version and content
    /// hash (the SHA-256 of its source).
    fn pin(&self) -> PluginPin;

    /// Decides over one call's evidence.
    ///
    /// # Errors
    /// Why the rule could not decide (for a Python rule, its traceback).
    fn decide(&mut self, call: &MaHookCall<'_>) -> std::result::Result<Vec<MaVerdict>, String>;
}

/// What the host can ask about an attached hook after the run.
#[derive(Debug, Clone, Default)]
pub struct MaHookStatus {
    error: Arc<Mutex<Option<String>>>,
    calls: Arc<Mutex<u64>>,
}

impl MaHookStatus {
    /// The first failure the hook returned, if any.
    #[must_use]
    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|g| g.clone())
    }

    /// How many calls the hook received.
    #[must_use]
    pub fn calls(&self) -> u64 {
        self.calls.lock().map_or(0, |g| *g)
    }

    fn fail(&self, e: String) {
        if let Ok(mut g) = self.error.lock()
            && g.is_none()
        {
            *g = Some(e);
        }
    }
}

/// The hook as one provider's MA hosts it (`phase2::MaHost::External`).
#[derive(Clone)]
pub(crate) struct ExternalMa {
    hook: Arc<Mutex<Box<dyn MaPolicyHook>>>,
    id: String,
    ma: u32,
    provider: String,
    status: MaHookStatus,
    /// Report id → the subject this step's call decided on it.
    decided: BTreeMap<String, String>,
    subjects: BTreeSet<String>,
    infrastructure: BTreeSet<String>,
}

impl std::fmt::Debug for ExternalMa {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalMa")
            .field("id", &self.id)
            .field("ma", &self.ma)
            .field("provider", &self.provider)
            .finish_non_exhaustive()
    }
}

impl ExternalMa {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn subjects(&self) -> usize {
        self.subjects.len()
    }

    pub(crate) fn trust_infrastructure(&mut self, digest: String) {
        self.infrastructure.insert(digest);
    }

    /// Whether the MA trusts `reporter` as infrastructure.
    pub(crate) fn is_infrastructure(&self, reporter: &str) -> bool {
        self.infrastructure.contains(reporter)
    }

    /// One step's evidence through the hook; [`ExternalMa::ingest_evidence`] then hands
    /// each decision out on the report it was taken on.
    pub(crate) fn decide(&mut self, t: SimTime, rows: &[MaHookRow]) {
        self.decided.clear();
        if rows.is_empty() {
            return;
        }
        for r in rows {
            self.subjects.insert(r.view.subject_cert.clone());
        }
        if let Ok(mut c) = self.status.calls.lock() {
            *c += 1;
        }
        let call = MaHookCall {
            t,
            ma: self.ma,
            provider: &self.provider,
            rows,
        };
        let verdicts = match self.hook.lock() {
            Ok(mut h) => h.decide(&call),
            Err(_) => Err("the MA policy hook panicked in an earlier call".to_string()),
        };
        let verdicts = match verdicts {
            Ok(v) => v,
            Err(e) => {
                self.status.fail(e);
                return;
            }
        };
        for v in verdicts.into_iter().filter(|v| v.revoke) {
            let row = match v.row {
                Some(i) => rows.get(i).filter(|r| r.view.subject_cert == v.subject),
                None => rows.iter().rev().find(|r| r.view.subject_cert == v.subject),
            };
            let Some(row) = row else {
                self.status.fail(format!(
                    "the MA policy hook revoked {} on {}, which is not a row about it in this \
                     call ({} rows)",
                    v.subject,
                    v.row
                        .map_or("its last row".to_string(), |i| format!("row {i}")),
                    rows.len()
                ));
                continue;
            };
            self.decided
                .entry(row.view.report_id.clone())
                .or_insert(v.subject);
        }
    }

    /// The decision this step's call took on report `r`, once.
    pub(crate) fn ingest_evidence(&mut self, report_id: &str) -> Option<v2xw_threat::MaAction> {
        self.decided
            .remove(report_id)
            .map(|subject| v2xw_threat::MaAction::Revoke { subject })
    }
}

/// Hands every provider MA that runs the deployment-wide rule to `hook`, and pins it in
/// the manifest.
///
/// # Errors
/// A run with no SCMS Misbehaviour Authority, a scenario that already chooses the
/// deployment-wide rule, or a deployment whose every provider has its own policy.
pub(crate) fn attach(
    scenario: &Scenario,
    phase2: Option<&mut crate::phase2::Phase2>,
    manifest: &mut Manifest,
    hook: Box<dyn MaPolicyHook>,
) -> Result<MaHookStatus> {
    let refuse =
        |field: &str, why: &str| Err(EngineError::Scenario(ScenarioError::conflict(field, why)));
    let Some(phase2) = phase2 else {
        return refuse(
            "security.scms",
            "the run has no SCMS deployment, so there is no Misbehaviour Authority for an MA \
             policy hook to decide for",
        );
    };
    if scenario.detection.ma.is_some() {
        return refuse(
            "detection.ma",
            "and the MA policy hook both choose the MA's decision rule; remove detection.ma",
        );
    }
    if scenario
        .security
        .scms
        .as_ref()
        .is_some_and(|s| s.ma_policy.is_some())
    {
        return refuse(
            "security.scms.ma_policy",
            "and the MA policy hook both choose the MA's decision rule; remove ma_policy",
        );
    }
    let own: BTreeSet<String> = scenario
        .security
        .scms
        .as_ref()
        .and_then(|s| s.deployment.as_ref())
        .map(|d| {
            d.providers
                .iter()
                .filter(|p| p.ma_policy.is_some())
                .map(|p| p.name.clone())
                .collect()
        })
        .unwrap_or_default();
    let pin = hook.pin();
    let id = format!("{ID_PREFIX}{}", pin.id);
    let status = MaHookStatus::default();
    let shared = Arc::new(Mutex::new(hook));
    let names = phase2.provider_names();
    let mut hosted = 0;
    for (k, host) in phase2.ma_hosts_mut().iter_mut().enumerate() {
        let name = names
            .get(k)
            .cloned()
            .unwrap_or_else(|| format!("provider-{k}"));
        if own.contains(&name) {
            continue;
        }
        *host = crate::phase2::MaHost::External(ExternalMa {
            hook: Arc::clone(&shared),
            id: id.clone(),
            ma: u32::try_from(k).unwrap_or(u32::MAX),
            provider: name,
            status: status.clone(),
            decided: BTreeMap::new(),
            subjects: BTreeSet::new(),
            infrastructure: BTreeSet::new(),
        });
        hosted += 1;
    }
    if hosted == 0 {
        return refuse(
            "security.scms.deployment.providers",
            "every provider runs its own ma_policy, so the MA policy hook would decide for \
             no Misbehaviour Authority",
        );
    }
    manifest
        .plugins
        .push(PluginPin::new(id, pin.version, pin.content_hash));
    manifest.plugins.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    type Step = std::result::Result<Vec<MaVerdict>, String>;

    /// Returns its script, one step per call.
    struct Scripted(Vec<Step>);

    impl MaPolicyHook for Scripted {
        fn pin(&self) -> PluginPin {
            PluginPin::new("rust:scripted", "1.0.0", "00")
        }
        fn decide(&mut self, _call: &MaHookCall<'_>) -> Step {
            self.0.remove(0)
        }
    }

    fn row(id: &str, subject: &str) -> MaHookRow {
        MaHookRow {
            view: MaReceived {
                t: 1,
                report_id: id.to_string(),
                ma: 0,
                reporter_cert: "rep".to_string(),
                subject_cert: subject.to_string(),
                subtypes: Vec::new(),
                generation_time: 0,
                evidence_start: 0,
                evidence_end: 0,
                excess: 1.0,
                pos_accuracy_m: 1.0,
                secmark_phase_ms: 0,
                issuer: String::new(),
                la_link: String::new(),
                set_aside: None,
            },
            observed: 0,
            reporter_infrastructure: false,
        }
    }

    fn host(script: Vec<Step>) -> ExternalMa {
        ExternalMa {
            hook: Arc::new(Mutex::new(Box::new(Scripted(script)))),
            id: "ma_policy:rust:scripted".to_string(),
            ma: 0,
            provider: "provider-a".to_string(),
            status: MaHookStatus::default(),
            decided: BTreeMap::new(),
            subjects: BTreeSet::new(),
            infrastructure: BTreeSet::new(),
        }
    }

    fn verdict(subject: &str, revoke: bool, row: Option<usize>) -> MaVerdict {
        MaVerdict {
            subject: subject.to_string(),
            revoke,
            row,
        }
    }

    fn revoked(subject: &str) -> Option<v2xw_threat::MaAction> {
        Some(v2xw_threat::MaAction::Revoke {
            subject: subject.to_string(),
        })
    }

    /// A decision lands on the report it names (or the last about its subject), once; a
    /// "not yet" changes nothing.
    #[test]
    fn a_verdict_lands_on_its_row_once() {
        let rows = [row("r0", "a"), row("r1", "b"), row("r2", "a")];
        let mut m = host(vec![Ok(vec![
            verdict("b", true, Some(1)),
            verdict("a", true, None),
            verdict("b", false, None),
        ])]);
        m.decide(10, &rows);
        assert_eq!(m.status.error(), None);
        assert_eq!(m.ingest_evidence("r0"), None, "not the last row about a");
        assert_eq!(m.ingest_evidence("r1"), revoked("b"));
        assert_eq!(m.ingest_evidence("r2"), revoked("a"));
        assert_eq!(m.ingest_evidence("r2"), None, "once");
        assert_eq!(m.subjects(), 2);
        assert_eq!(m.status.calls(), 1);
    }

    /// A failure, or a verdict about no row of the call, is kept for the host and decides
    /// nothing; a step with no evidence makes no call.
    #[test]
    fn a_failure_is_kept_and_decides_nothing() {
        let rows = [row("r0", "a")];
        let mut m = host(vec![
            Err("boom".to_string()),
            Ok(vec![verdict("z", true, None), verdict("a", true, Some(5))]),
        ]);
        m.decide(10, &[]);
        assert_eq!(m.status.calls(), 0, "no evidence, no call");
        m.decide(10, &rows);
        assert_eq!(m.status.error().as_deref(), Some("boom"));
        assert_eq!(m.ingest_evidence("r0"), None);
        m.decide(20, &rows);
        assert_eq!(m.ingest_evidence("r0"), None);
        assert_eq!(
            m.status.error().as_deref(),
            Some("boom"),
            "the first failure"
        );
    }
}
