//! The Misbehaviour Authority's decision rule as **one declarative policy**:
//! `threat/ma/policy` (docs/design/scms-real/08-settings-ux-and-gaps.md §7).
//!
//! # Why a family, not one rule
//!
//! The US deployment publishes the report format and the device-side store weights
//! (SCMS Manager SM-MBR v1.0), not the MA's decision rule. The CV pilots ran a "minimum
//! viable" MA in which an operator reviews reports and adds devices to the CRL (CAMP
//! SCMS CAMP122 §4.2-4.3), and ETSI TS 103 759 and SAE J3287 leave the reaction out of
//! scope. So no single rule can be called the deployed one. This module states the rule
//! as data: a preset fills every field, and a field written beside it overrides the
//! preset's. The presets are the engine's two pipelines and the `v2xw ma-replay` family,
//! with the same parameters.
//!
//! # One evaluator
//!
//! Every preset but `legacy-window` runs the corroborating evaluator, which is
//! [`crate::ma_corroborate::CorroboratedMa`] generalised field by field:
//!
//! 1. A report's **score** is the largest [`MaPolicyParams::scores`] entry among the checks
//!    it lists (1 when none is set). A score of 0 means the MA ignores the report: it is
//!    not evidence at all. Under `accuracy: catch` a position report's score is multiplied
//!    by the CaTch factor `clamp(1 − 2a / (excess · τ), 0, 1)` (Kamel et al., IEEE WCNC
//!    2019; the `ma-replay` `catch` rule).
//! 2. The evidence about one **case** (one pseudonym certificate, or under
//!    `identity: device` every certificate the linkage authorities join into one device)
//!    is cut into events of `event_window_s`, and an event counts when
//!    `event_min_reporters` distinct trusted reporters witnessed it.
//! 3. The case is decided when, inside `window_s`, it holds `min_events` counted events
//!    witnessed by `min_distinct_reporters` distinct trusted reporters (times `m` in a
//!    crowded class under the population guard), and, when set, a summed score of
//!    `min_score` (each reporter's best report, decayed) and `min_reports` reports.
//!
//! With the `cv-pilot-manual` fields every extension is inert, and the evaluator is
//! [`CorroboratedMa`](crate::ma_corroborate::CorroboratedMa) decision for decision; the
//! unit test `cv_pilot_manual_is_the_corroborating_ma` holds that on a seeded stream.
//! `legacy-window` delegates to [`crate::ma::LegacyWindow`], whose distinct-seconds and
//! span gate has no field here.
//!
//! # What the policy reads
//!
//! [`PolicyEvidence`]: only what the deciding MA reads from a report or learns through
//! the protocol (reporter and subject certificates, the checks, the re-derived excess, the
//! accused's broadcast accuracy, the evidence's secMark phase, the issuer and, through the
//! LAs, a device link). Never ground truth.

use std::collections::{BTreeMap, BTreeSet};

use crate::ctx::{ThreatCtx, ThreatCtxExt};
use crate::ma::{LegacyWindow, MaAction, MaParams, MaPipeline};
use crate::records::{MaDecisionRecord, MaReportRecord};
use crate::report::MisbehaviourReport;
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::model::Model;
use v2xw_core::time::{NS_PER_S, SimTime, secs_to_ns};

/// The model id the policy's card and its decisions carry.
pub const MODEL_ID: &str = "threat/ma/policy";

/// The preset a policy starts from when it names none: today's MA.
pub const DEFAULT_PRESET: &str = "cv-pilot-manual";

/// The family's presets (08 §7 table). The middle four are `v2xw ma-replay`'s rules.
pub const PRESET_IDS: [&str; 8] = [
    "cv-pilot-manual",
    "corroborated-auto",
    "legacy-window",
    "pseudonym-k",
    "device-k",
    "catch",
    "cluster",
    "custom",
];

/// The keys of [`MaPolicyParams::scores`]: the five SM-MBR v1.0 report subtypes, then
/// every engine check as `detect/<check id>` ([`crate::detect::DetectorId`]). A check's own
/// key wins over its subtype's.
pub const RULE_KEYS: [&str; 21] = [
    "RandomPosition",
    "ConstantPosition",
    "MaxSpeed",
    "MaxAcceleration",
    "BrakeAcceleration",
    "detect/positionSpeedInconsistency",
    "detect/positionJump",
    "detect/headingInconsistency",
    "detect/staleOrReplay",
    "detect/constantPositionFrozen",
    "detect/implausibleAcceleration",
    "detect/sybilCoLocation",
    "detect/acceptanceRangeThreshold",
    "detect/beaconFrequency",
    "detect/signatureVerification",
    "detect/certValidity",
    "detect/mapOffRoad",
    "detect/vruImpersonation",
    "detect/denmPlausibility",
    "detect/kalmanConsistency",
    "detect/implausibleSpeed",
];

/// The SM-MBR v1.0 subtype an engine check files as (`us_mbr` module table).
/// `BrakeAcceleration` has no check (not modelled), so its score never applies.
#[must_use]
pub fn sm_mbr_subtype(check: &str) -> Option<&'static str> {
    match check {
        "positionJump" => Some("RandomPosition"),
        "constantPositionFrozen" => Some("ConstantPosition"),
        "implausibleSpeed" => Some("MaxSpeed"),
        "implausibleAcceleration" => Some("MaxAcceleration"),
        _ => None,
    }
}

/// What one case is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identity {
    /// One pseudonym certificate, as the report names it.
    Pseudonym,
    /// One device: every certificate the linkage authorities join.
    Device,
}

/// How a report's score allows for the accused's stated accuracy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Accuracy {
    /// Every report counts its full score.
    None,
    /// CaTch: a position report counts the share its accuracy cannot explain.
    Catch {
        /// The deviation one unit of excess stands for, m (SM-MBR gpsDrift, 1).
        tau_m: f64,
    },
}

/// How old evidence loses score.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decay {
    /// Full score until it leaves the window.
    None,
    /// Multiplied by `factor` at every `hour_et`:00 US Eastern (SM-MBR v1.0's store).
    DailyMultiplier {
        /// The daily factor.
        factor: f64,
        /// The Eastern hour.
        hour_et: u32,
    },
    /// Halved every `half_life_s`.
    HalfLife {
        /// The half-life, s.
        half_life_s: f64,
    },
}

/// The guard against revoking a device population that shares one benign trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopulationGuard {
    /// Every case needs the same evidence.
    Off,
    /// When `c` cases of one class are accused inside the window, a case in it needs `m`
    /// times the reporters (the `ma-replay` `cluster` rule).
    Cluster {
        /// Accused cases that crowd a class.
        c: usize,
        /// The reporter multiplier inside a crowded class.
        m: usize,
    },
}

/// Who takes the final decision. The evaluator recommends; the host carries this out
/// (the SCMS run's `MaMode`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Decision {
    /// An operator reviews each case for `review_s` (the CV pilots).
    ManualReview {
        /// The review, s.
        review_s: f64,
    },
    /// The rule revokes at once.
    Automatic,
}

/// Which evaluator runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evaluator {
    /// Events, corroboration and the extensions above.
    Corroborating,
    /// [`LegacyWindow`]: distinct reporters, distinct seconds and a span.
    LegacyWindow,
}

/// One fully resolved policy.
#[derive(Debug, Clone, PartialEq)]
pub struct MaPolicyParams {
    /// The preset it started from.
    pub preset: String,
    /// The evaluator.
    pub evaluator: Evaluator,
    /// What one case is about.
    pub identity: Identity,
    /// Per [`RULE_KEYS`] key, the score one report adds (absent: 1; 0: ignored).
    pub scores: BTreeMap<String, f64>,
    /// Accuracy weighting.
    pub accuracy: Accuracy,
    /// An event's span, s.
    pub event_window_s: f64,
    /// Distinct trusted reporters an event needs.
    pub event_min_reporters: usize,
    /// Counted events that decide.
    pub min_events: usize,
    /// Summed score that decides (0: not used).
    pub min_score: f64,
    /// Counted reports that decide (0: not used).
    pub min_reports: usize,
    /// Distinct trusted reporters that decide.
    pub min_distinct_reporters: usize,
    /// The sliding window, s.
    pub window_s: f64,
    /// Score decay.
    pub decay: Decay,
    /// Population guard.
    pub guard: PopulationGuard,
    /// Whether only trusted reporters count.
    pub defence: bool,
    /// A reporter itself accused this often inside the window is untrusted.
    pub reputation_max: u32,
    /// A reporter filing about more than this inside the window is rate-limited.
    pub report_budget: u32,
    /// Who decides.
    pub decision: Decision,
}

/// The operator's review under `cv-pilot-manual`, s (ASSUMPTION: one 8 h shift; the
/// engine's `ma_review_s` default).
pub const CV_PILOT_REVIEW_S: f64 = 28_800.0;

impl MaPolicyParams {
    /// The preset `id`'s fields, or `None` for an id outside [`PRESET_IDS`].
    ///
    /// The corroborating presets are `CorroborationParams::default()`; the replay rules
    /// take the middle of `ma-replay`'s grid: k = 3 (the legacy k), a 1 h window, θ = 3,
    /// c = 3 and m = 3 (ASSUMPTION each; no source fixes them, 08 §9).
    #[must_use]
    pub fn preset(id: &str) -> Option<MaPolicyParams> {
        let d = crate::ma_corroborate::CorroborationParams::default();
        let base = MaPolicyParams {
            preset: id.to_string(),
            evaluator: Evaluator::Corroborating,
            identity: Identity::Pseudonym,
            scores: BTreeMap::new(),
            accuracy: Accuracy::None,
            event_window_s: d.event_window_s,
            event_min_reporters: d.event_min_reporters,
            min_events: d.min_events,
            min_score: 0.0,
            min_reports: 0,
            min_distinct_reporters: d.min_reporters,
            window_s: d.window_s,
            decay: Decay::None,
            guard: PopulationGuard::Off,
            defence: d.defence,
            reputation_max: d.reputation_max,
            report_budget: d.report_budget,
            decision: Decision::ManualReview {
                review_s: CV_PILOT_REVIEW_S,
            },
        };
        // The replay rules: every report is its own counted evidence, no trust gate.
        let replay = |identity, window_s| MaPolicyParams {
            identity,
            event_min_reporters: 1,
            min_events: 1,
            window_s,
            defence: false,
            decision: Decision::Automatic,
            ..base.clone()
        };
        Some(match id {
            "cv-pilot-manual" | "custom" => base,
            "corroborated-auto" => MaPolicyParams {
                decision: Decision::Automatic,
                ..base
            },
            "legacy-window" => {
                let l = MaParams::default();
                MaPolicyParams {
                    evaluator: Evaluator::LegacyWindow,
                    min_distinct_reporters: l.report_threshold_k,
                    window_s: l.revoke_window_s,
                    defence: l.defence,
                    reputation_max: l.reputation_max,
                    report_budget: l.report_budget,
                    decision: Decision::Automatic,
                    ..base
                }
            }
            // No window: `ma-replay`'s pseudonym-k counts every reporter ever; 30 days is
            // the window's top, longer than any run.
            "pseudonym-k" => replay(Identity::Pseudonym, 2_592_000.0),
            "device-k" => replay(Identity::Device, 3_600.0),
            "catch" => MaPolicyParams {
                accuracy: Accuracy::Catch { tau_m: 1.0 },
                min_distinct_reporters: 1,
                min_score: 3.0,
                ..replay(Identity::Device, 3_600.0)
            },
            "cluster" => MaPolicyParams {
                guard: PopulationGuard::Cluster { c: 3, m: 3 },
                ..replay(Identity::Device, 3_600.0)
            },
            _ => return None,
        })
    }

    /// The fields (relative to a policy's root) that mean something under this
    /// policy's evaluator. Under `legacy-window` the events, scores, decay, guard and
    /// device identity do not exist; a host refuses them when written.
    #[must_use]
    pub fn applies(&self, field: &str) -> bool {
        match self.evaluator {
            Evaluator::Corroborating => true,
            Evaluator::LegacyWindow => matches!(
                field,
                "preset"
                    | "threshold.min_distinct_reporters"
                    | "threshold.window_s"
                    | "defence.trusted_reporters"
                    | "defence.reputation_max"
                    | "defence.report_budget"
                    | "decision.mode"
                    | "decision.review_s"
            ),
        }
    }

    /// The score of a report that lists `checks`: the largest set score among them, a
    /// check's own `detect/<id>` key before its SM-MBR subtype's; 1 when none is set.
    #[must_use]
    pub fn score_of(&self, checks: &[String]) -> f64 {
        if self.scores.is_empty() {
            return 1.0;
        }
        let mut best: Option<f64> = None;
        for c in checks {
            let own = self.scores.get(&format!("detect/{c}"));
            let sub = sm_mbr_subtype(c).and_then(|s| self.scores.get(s));
            let v = own.or(sub).copied().unwrap_or(1.0);
            best = Some(best.map_or(v, |b: f64| b.max(v)));
        }
        best.unwrap_or(1.0)
    }

    /// Whether one reporter can never decide a case alone: the soundness `ma-replay`
    /// marks (k ≥ 2, θ ≥ 2).
    #[must_use]
    pub fn sound(&self) -> bool {
        let one = self
            .scores
            .values()
            .copied()
            .fold(1.0_f64, f64::max)
            .max(if self.scores.is_empty() { 1.0 } else { 0.0 });
        self.min_distinct_reporters >= 2
            || (self.evaluator == Evaluator::Corroborating
                && (self.event_min_reporters >= 2 || self.min_score > one))
    }
}

/// One report as the policy reads it.
#[derive(Debug, Clone, Copy)]
pub struct PolicyEvidence<'a> {
    /// The evidence's instant: the report's observation, not its arrival (behind the RA's
    /// shuffle a batch arrives at once).
    pub at: SimTime,
    /// The reporter's pseudonym certificate digest.
    pub reporter: &'a str,
    /// The accused's pseudonym certificate digest.
    pub subject: &'a str,
    /// The accused's device link from the linkage authorities, when the MA asked them;
    /// `None` makes the pseudonym its own device.
    pub device: Option<&'a str>,
    /// The checks the report lists, leading first.
    pub checks: &'a [String],
    /// How far the leading check exceeds its threshold (≈ 1 at it).
    pub excess: f64,
    /// The accused's broadcast positional accuracy, m.
    pub pos_accuracy_m: f64,
    /// The judged BSM's secMark modulo 50 ms, ms.
    pub secmark_phase_ms: u32,
    /// The provider whose ACA issued the accused certificate (`""`: unknown).
    pub issuer: &'a str,
}

impl<'a> PolicyEvidence<'a> {
    /// A report's evidence dated `at`, with what the report itself carries: no device
    /// link (the pseudonym stands for its device), the evidence end's phase and no issuer.
    #[must_use]
    pub fn of_report(r: &'a MisbehaviourReport, at: SimTime) -> PolicyEvidence<'a> {
        PolicyEvidence {
            at,
            reporter: &r.reporter_cert_digest,
            subject: &r.subject_cert_digest,
            device: None,
            checks: &r.reason_codes,
            excess: crate::records::q(r.detector_score),
            pos_accuracy_m: crate::records::q(r.subject_pos_confidence),
            secmark_phase_ms: ((r.st_tend / 1_000_000) % 50) as u32,
            issuer: "",
        }
    }
}

/// The CaTch factor of one report (the `ma-replay` `catch` rule): a non-position check
/// counts 1; a position check counts `clamp(1 − 2a / (excess · τ), 0, 1)`.
#[must_use]
pub fn catch_factor(checks: &[String], excess: f64, pos_accuracy_m: f64, tau_m: f64) -> f64 {
    let positional = checks.first().is_some_and(|c| c.starts_with("position"));
    if !positional {
        return 1.0;
    }
    let d = excess.max(0.0) * tau_m;
    if d <= 0.0 {
        return 0.0;
    }
    (1.0 - 2.0 * pos_accuracy_m.max(0.0) / d).clamp(0.0, 1.0)
}

/// A report's class for the population guard: secMark phase (locked within 1 ms of the
/// 50 ms epoch, or free), accuracy band and issuer (the `ma-replay` `cluster` rule).
#[must_use]
pub fn class_of(secmark_phase_ms: u32, pos_accuracy_m: f64, issuer: &str) -> String {
    let phase = if secmark_phase_ms <= 1 || secmark_phase_ms >= 49 {
        "locked"
    } else {
        "free"
    };
    let band = match pos_accuracy_m {
        a if a < 1.0 => "acc<1m",
        a if a < 3.0 => "acc1-3m",
        a if a < 10.0 => "acc3-10m",
        _ => "acc>=10m",
    };
    format!("{phase}/{band}/{issuer}")
}

/// How many US Eastern `hour:00` instants lie in `(from, to]`, unix seconds. The policy
/// takes the deployment calendar's (daylight saving included) from its host through
/// [`MaPolicy::with_calendar`]; this fixed-EST count is the stand-alone fallback.
pub type EasternCount = fn(i64, i64, u32, u32) -> u32;

fn fixed_est_count(from: i64, to: i64, hh: u32, mm: u32) -> u32 {
    let off = 5 * 3_600 + i64::from(hh) * 3_600 + i64::from(mm) * 60;
    let day = |t: i64| (t - off).div_euclid(86_400);
    u32::try_from((day(to) - day(from)).max(0)).unwrap_or(u32::MAX)
}

/// What the policy holds about one case, as its decision reads it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PolicySummary {
    /// Reports inside the window.
    pub reports: u32,
    /// Events those reports cut into.
    pub events: u32,
    /// Of them, the events enough trusted reporters witnessed.
    pub corroborated_events: u32,
    /// Distinct trusted reporters over the counted events.
    pub trusted_reporters: u32,
    /// Trusted reports in the counted events.
    pub counted_reports: u32,
    /// Summed over those reporters, each one's best decayed score.
    pub score: f64,
}

/// One decision: the pseudonym whose report tipped it and the case it decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDecision {
    /// The tipping report's subject certificate.
    pub subject: String,
    /// The case: the pseudonym, or the device link under `identity: device`.
    pub case: String,
}

#[derive(Debug, Clone)]
struct Item {
    at: SimTime,
    reporter: String,
    /// Score times the CaTch factor, before decay.
    weight: f64,
}

/// The corroborating evaluator's state ([`crate::ma_corroborate::CorroboratedMa`]'s, per
/// case rather than per pseudonym).
#[derive(Debug, Clone, Default)]
struct Corroborating {
    evidence: BTreeMap<String, Vec<Item>>,
    filed_by: BTreeMap<String, BTreeSet<(u64, String)>>,
    reported: BTreeMap<String, BTreeSet<(u64, String)>>,
    /// Pseudonym → case, under `identity: device`.
    case_of: BTreeMap<String, String>,
    revoked_cases: BTreeSet<String>,
    /// Per guard class: `(instant, case)`, oldest first.
    classes: BTreeMap<String, Vec<(SimTime, String)>>,
    peak_unrevoked: u32,
}

#[derive(Debug, Clone)]
enum State {
    Corroborating(Corroborating),
    Legacy(Box<LegacyWindow>),
}

/// The policy evaluator.
#[derive(Debug, Clone)]
pub struct MaPolicy {
    card: ModelCard,
    params: MaPolicyParams,
    state: State,
    infrastructure: BTreeSet<String>,
    decisions: Vec<MaAction>,
    t0_unix_s: i64,
    eastern: EasternCount,
}

impl MaPolicy {
    /// The policy with the given fields.
    #[must_use]
    pub fn new(params: MaPolicyParams) -> MaPolicy {
        let state = match params.evaluator {
            Evaluator::Corroborating => State::Corroborating(Corroborating::default()),
            Evaluator::LegacyWindow => State::Legacy(Box::new(LegacyWindow::new(MaParams {
                report_threshold_k: params.min_distinct_reporters,
                revoke_window_s: params.window_s,
                defence: params.defence,
                reputation_max: params.reputation_max,
                report_budget: params.report_budget,
                ..MaParams::default()
            }))),
        };
        MaPolicy {
            card: card(&params),
            params,
            state,
            infrastructure: BTreeSet::new(),
            decisions: Vec::new(),
            t0_unix_s: 0,
            eastern: fixed_est_count,
        }
    }

    /// The deployment calendar the daily decay reads: the run's start in unix seconds,
    /// and its Eastern-instant counter.
    #[must_use]
    pub fn with_calendar(mut self, t0_unix_s: i64, eastern: EasternCount) -> MaPolicy {
        self.t0_unix_s = t0_unix_s;
        self.eastern = eastern;
        self
    }

    /// The fields it runs.
    #[must_use]
    pub fn params(&self) -> &MaPolicyParams {
        &self.params
    }

    /// Declares a certificate as trusted infrastructure: never rate-limited or distrusted.
    pub fn trust_infrastructure(&mut self, digest: impl Into<String>) {
        let d = digest.into();
        if let State::Legacy(l) = &mut self.state {
            l.trust_infrastructure(d.clone());
        }
        self.infrastructure.insert(d);
    }

    /// How many cases the policy holds evidence about.
    #[must_use]
    pub fn subjects(&self) -> usize {
        match &self.state {
            State::Corroborating(c) => c.evidence.len(),
            State::Legacy(l) => l.subjects(),
        }
    }

    /// The most counted events a case that was never decided reached (`None` under
    /// `legacy-window`, which has no events).
    #[must_use]
    pub fn peak_unrevoked_events(&self) -> Option<u32> {
        match &self.state {
            State::Corroborating(c) => Some(c.peak_unrevoked),
            State::Legacy(_) => None,
        }
    }

    /// What the policy holds about the case of pseudonym `subject` now.
    #[must_use]
    pub fn summary(&self, subject: &str) -> PolicySummary {
        match &self.state {
            State::Corroborating(c) => {
                let case = c.case_of.get(subject).map_or(subject, String::as_str);
                self.summarise(c, case)
            }
            State::Legacy(_) => PolicySummary::default(),
        }
    }

    /// The counted events about pseudonym `subject`'s case (`None` under `legacy-window`).
    #[must_use]
    pub fn corroborated_events(&self, subject: &str) -> Option<u32> {
        match &self.state {
            State::Corroborating(_) => Some(self.summary(subject).corroborated_events),
            State::Legacy(_) => None,
        }
    }

    fn slot_ns(&self) -> u64 {
        secs_to_ns(self.params.event_window_s).max(1)
    }

    fn slot_range(&self, at: SimTime) -> (u64, u64) {
        let slot = self.slot_ns();
        let from = at.saturating_sub(secs_to_ns(self.params.window_s)) / slot;
        (from, at / slot)
    }

    fn trusted_at(&self, c: &Corroborating, reporter: &str, at: SimTime) -> bool {
        if !self.params.defence || self.infrastructure.contains(reporter) {
            return true;
        }
        let case = c.case_of.get(reporter).map_or(reporter, String::as_str);
        if c.revoked_cases.contains(case) {
            return false;
        }
        let (from, to) = self.slot_range(at);
        let in_window = |set: Option<&BTreeSet<(u64, String)>>| {
            set.map_or(0, |s| {
                s.iter()
                    .filter(|(slot, _)| *slot >= from && *slot <= to)
                    .count()
            })
        };
        in_window(c.filed_by.get(reporter)) <= self.params.report_budget as usize
            && in_window(c.reported.get(reporter)) < self.params.reputation_max as usize
    }

    /// The decay factor of evidence dated `at`, read at `now`.
    fn decay(&self, at: SimTime, now: SimTime) -> f64 {
        match self.params.decay {
            Decay::None => 1.0,
            Decay::HalfLife { half_life_s } => {
                let age_s = now.saturating_sub(at) as f64 / NS_PER_S as f64;
                v2xw_core::math::exp2(-age_s / half_life_s)
            }
            Decay::DailyMultiplier { factor, hour_et } => {
                let unix = |t: SimTime| self.t0_unix_s + i64::try_from(t / NS_PER_S).unwrap_or(0);
                let n = (self.eastern)(unix(at), unix(now), hour_et, 0);
                let mut w = 1.0;
                for _ in 0..n.min(10_000) {
                    w *= factor;
                }
                w
            }
        }
    }

    fn summarise(&self, c: &Corroborating, case: &str) -> PolicySummary {
        let Some(ev) = c.evidence.get(case) else {
            return PolicySummary::default();
        };
        let Some(latest) = ev.last().map(|e| e.at) else {
            return PolicySummary::default();
        };
        let cutoff = latest.saturating_sub(secs_to_ns(self.params.window_s));
        let inside: Vec<&Item> = ev.iter().filter(|e| e.at >= cutoff).collect();
        let span = secs_to_ns(self.params.event_window_s).max(1);
        let mut out = PolicySummary {
            reports: u32::try_from(inside.len()).unwrap_or(u32::MAX),
            ..PolicySummary::default()
        };
        let mut witnesses: BTreeSet<&str> = BTreeSet::new();
        let mut best: BTreeMap<&str, f64> = BTreeMap::new();
        let mut i = 0;
        while i < inside.len() {
            let opened = inside[i].at;
            let mut reporters: BTreeSet<&str> = BTreeSet::new();
            let mut members: Vec<&Item> = Vec::new();
            while i < inside.len() && inside[i].at < opened.saturating_add(span) {
                let r = inside[i].reporter.as_str();
                if self.trusted_at(c, r, latest) {
                    reporters.insert(r);
                    members.push(inside[i]);
                }
                i += 1;
            }
            out.events += 1;
            if reporters.len() >= self.params.event_min_reporters {
                out.corroborated_events += 1;
                witnesses.extend(reporters);
                for m in members {
                    out.counted_reports += 1;
                    let w = m.weight * self.decay(m.at, latest);
                    let b = best.entry(m.reporter.as_str()).or_insert(0.0);
                    *b = b.max(w);
                }
            }
        }
        out.trusted_reporters = u32::try_from(witnesses.len()).unwrap_or(u32::MAX);
        out.score = best.values().sum();
        out
    }

    /// Records one report's evidence and decides, **emitting nothing** (the contract of
    /// [`crate::ma_corroborate::CorroboratedMa::ingest_evidence`]).
    pub fn ingest(&mut self, e: &PolicyEvidence<'_>) -> Option<PolicyDecision> {
        let score = self.params.score_of(e.checks);
        if score <= 0.0 {
            return None;
        }
        if let State::Legacy(l) = &mut self.state {
            return l
                .ingest_pair(e.reporter, e.subject, e.at)
                .map(|a| PolicyDecision {
                    subject: a.subject().to_string(),
                    case: a.subject().to_string(),
                });
        }
        let weight = match self.params.accuracy {
            Accuracy::None => score,
            Accuracy::Catch { tau_m } => {
                score * catch_factor(e.checks, e.excess, e.pos_accuracy_m, tau_m)
            }
        };
        let slot = e.at / self.slot_ns();
        let keep_from = self
            .slot_range(e.at.saturating_sub(secs_to_ns(self.params.window_s)))
            .0;
        let window = secs_to_ns(self.params.window_s);
        let case = match (self.params.identity, e.device) {
            (Identity::Device, Some(d)) => d.to_string(),
            _ => e.subject.to_string(),
        };
        let guard = self.params.guard;
        let State::Corroborating(c) = &mut self.state else {
            return None;
        };
        if case != e.subject {
            c.case_of.insert(e.subject.to_string(), case.clone());
        }
        let filed = c.filed_by.entry(e.reporter.to_string()).or_default();
        filed.insert((slot, e.subject.to_string()));
        filed.retain(|(s, _)| *s >= keep_from);
        let about = c.reported.entry(e.subject.to_string()).or_default();
        about.insert((slot, e.reporter.to_string()));
        about.retain(|(s, _)| *s >= keep_from);
        let ev = c.evidence.entry(case.clone()).or_default();
        let pos = ev.partition_point(|x| x.at <= e.at);
        ev.insert(
            pos,
            Item {
                at: e.at,
                reporter: e.reporter.to_string(),
                weight,
            },
        );
        let latest = ev.last().map_or(e.at, |x| x.at);
        let cutoff = latest.saturating_sub(window);
        let stale = ev.partition_point(|x| x.at < cutoff);
        ev.drain(..stale);
        let need = match guard {
            PopulationGuard::Off => self.params.min_distinct_reporters,
            PopulationGuard::Cluster { c: crowd_at, m } => {
                let q = c
                    .classes
                    .entry(class_of(e.secmark_phase_ms, e.pos_accuracy_m, e.issuer))
                    .or_default();
                let pos = q.partition_point(|x| x.0 <= e.at);
                q.insert(pos, (e.at, case.clone()));
                let newest = q.last().map_or(e.at, |x| x.0);
                let stale = q.partition_point(|x| x.0 < newest.saturating_sub(window));
                q.drain(..stale);
                let from = e.at.saturating_sub(window);
                let crowd: BTreeSet<&str> = q
                    .iter()
                    .filter(|x| x.0 >= from)
                    .map(|x| x.1.as_str())
                    .collect();
                if crowd.len() >= crowd_at {
                    self.params.min_distinct_reporters.saturating_mul(m)
                } else {
                    self.params.min_distinct_reporters
                }
            }
        };
        self.correlate(&case, need).map(|()| PolicyDecision {
            subject: e.subject.to_string(),
            case,
        })
    }

    fn correlate(&mut self, case: &str, need: usize) -> Option<()> {
        let State::Corroborating(c) = &self.state else {
            return None;
        };
        if c.revoked_cases.contains(case) {
            return None;
        }
        let s = self.summarise(c, case);
        let p = &self.params;
        let decided = s.corroborated_events as usize >= p.min_events
            && s.trusted_reporters as usize >= need
            && (p.min_score <= 0.0 || s.score + 1e-9 >= p.min_score)
            && (p.min_reports == 0 || s.counted_reports as usize >= p.min_reports);
        let State::Corroborating(c) = &mut self.state else {
            return None;
        };
        if decided {
            c.revoked_cases.insert(case.to_string());
            return Some(());
        }
        c.peak_unrevoked = c.peak_unrevoked.max(s.corroborated_events);
        None
    }

    /// [`MaPolicy::ingest`] on a report, dated `at`, with what the report carries
    /// ([`PolicyEvidence::of_report`]).
    pub fn ingest_evidence(&mut self, r: &MisbehaviourReport, at: SimTime) -> Option<MaAction> {
        self.ingest(&PolicyEvidence::of_report(r, at))
            .map(|d| MaAction::Revoke { subject: d.subject })
    }
}

impl Model for MaPolicy {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl MaPipeline for MaPolicy {
    fn on_report(&mut self, ctx: &mut dyn ThreatCtx, r: &MisbehaviourReport) -> Vec<MaAction> {
        ctx.emit(MaReportRecord {
            t: r.ingest_time,
            reporter: r.reporter,
            subject: r.subject_cert_digest.clone(),
            detector: r.leading_reason().map(str::to_string),
        });
        let mut out = Vec::new();
        if let Some(a) = self.ingest_evidence(r, r.ingest_time) {
            ctx.emit(MaDecisionRecord {
                t: r.ingest_time,
                subject: a.subject().to_string(),
                decision: a.as_str().to_string(),
            });
            self.decisions.push(a.clone());
            out.push(a);
        }
        out
    }

    fn on_tick(&mut self, _ctx: &mut dyn ThreatCtx, _t: SimTime) -> Vec<MaAction> {
        // Evidence only grows on a report, and the decision only reads evidence.
        Vec::new()
    }

    fn decisions(&self) -> &[MaAction] {
        &self.decisions
    }
}

/// The model card.
#[must_use]
pub fn card(p: &MaPolicyParams) -> ModelCard {
    use serde_json::json;
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::MaPipeline,
        "1.0.0",
        "The Misbehaviour Authority's decision rule as one declarative policy: a preset \
         (the CV-pilot operator review over the corroborating rule, the legacy gate, or a \
         ma-replay rule) with any field overridden: what a case is (pseudonym or device), \
         each report subtype's score, accuracy weighting, events, thresholds, decay, a \
         population guard, the reporter defences and who decides.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    card.equations = vec![Equation::new(
        "decision",
        "|counted events in window| ≥ n ∧ |∪ trusted reporters| ≥ k·(m if crowded) ∧ \
         (θ = 0 ∨ Σ_reporters max decayed score ≥ θ) ∧ (r = 0 ∨ |counted reports| ≥ r)",
    )];
    let assumption = |name: &str, unit: &str, v: serde_json::Value, why: &str| {
        Parameter::new(
            name,
            unit,
            v,
            Source {
                kind: SourceKind::TodoCalibrate,
                reference: "design choice: the MA decision rule is unspecified by CAMP122 \
                            §4.2-4.3, ETSI TS 103 759 and SAE J3287 (08 §7)"
                    .to_string(),
                accessed: Some("2026-10-29".to_string()),
                note: Some(why.to_string()),
            },
        )
    };
    card.parameters = vec![
        assumption(
            "preset",
            "-",
            json!(p.preset),
            "the family member the fields start from",
        ),
        assumption(
            "min_distinct_reporters",
            "-",
            json!(p.min_distinct_reporters),
            "the legacy k",
        ),
        assumption("window_s", "s", json!(p.window_s), "the sliding window"),
        assumption(
            "min_score",
            "-",
            json!(p.min_score),
            "0 is not used; catch takes θ = 3",
        ),
    ];
    card.sources = vec![
        crate::cards::standard(
            "SCMS Manager, Misbehavior Reporting (SM-MBR) v1.0: report subtypes and the \
             device store's daily multiplier",
        ),
        crate::cards::paper(
            "Kamel, Ben Jemaa, Kaiser, Urien, CaTch: a confidence range tolerant \
             misbehavior detection approach, IEEE WCNC 2019",
        ),
        crate::cards::design("scms-real/08-settings-ux-and-gaps.md §7"),
    ];
    card.assumptions = vec![
        "The default preset is the CV pilots' operator-reviewed MA; its evidence rule is \
         the engine's corroborating rule, an ASSUMPTION."
            .to_string(),
    ];
    card.limitations = vec![
        "Under identity device the case joins certificates only when the host passes the \
         linkage authorities' link; a report alone stands for its pseudonym."
            .to_string(),
    ];
    card.determinism = Determinism {
        uses_rng: false,
        rng_domains: Vec::new(),
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec![
            "ma_policy::cv_pilot_manual_is_the_corroborating_ma".to_string(),
            "ma_policy::legacy_window_is_the_legacy_gate".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ma_corroborate::CorroboratedMa;

    /// A seeded stream of reports: `n` reports over `subjects` pseudonyms from
    /// `reporters` reporters, observation times drifting forward with jitter (and so
    /// sometimes out of order, as behind the RA's shuffle).
    fn stream(seed: u64, n: usize, subjects: u64, reporters: u64) -> Vec<(String, String, u64)> {
        let mut x = seed;
        let mut next = move || {
            x = x
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            x >> 33
        };
        let mut t: u64 = 10 * NS_PER_S;
        (0..n)
            .map(|_| {
                t += next() % (2 * NS_PER_S);
                let at = t.saturating_sub(next() % (3 * NS_PER_S));
                let s = format!("subj{:02}", next() % subjects);
                let r = format!("rep{:03}", next() % reporters);
                (r, s, at)
            })
            .collect()
    }

    fn ev<'a>(r: &'a str, s: &'a str, at: SimTime, checks: &'a [String]) -> PolicyEvidence<'a> {
        PolicyEvidence {
            at,
            reporter: r,
            subject: s,
            device: None,
            checks,
            excess: 1.0,
            pos_accuracy_m: 1.0,
            secmark_phase_ms: 0,
            issuer: "",
        }
    }

    /// `cv-pilot-manual` (and `custom`, which starts from it) is `CorroboratedMa` decision
    /// for decision: the same revocations in the same order, the same counted events per
    /// subject after every report, and the same peak.
    #[test]
    fn cv_pilot_manual_is_the_corroborating_ma() {
        let checks = vec!["positionJump".to_string()];
        let mut revoked = 0;
        for preset in ["cv-pilot-manual", "corroborated-auto", "custom"] {
            for seed in 1..=6_u64 {
                let mut a = CorroboratedMa::defaults();
                let mut b = MaPolicy::new(MaPolicyParams::preset(preset).expect("a preset"));
                a.trust_infrastructure("rep000");
                b.trust_infrastructure("rep000");
                for (r, s, at) in stream(seed, 1_500, 6, 9) {
                    let x = a.ingest_pair(&r, &s, at).map(|d| d.subject().to_string());
                    let y = b.ingest(&ev(&r, &s, at, &checks)).map(|d| d.subject);
                    assert_eq!(x, y, "{preset} seed {seed}: decision at {at} differs");
                    revoked += usize::from(x.is_some());
                    assert_eq!(
                        a.summary(&s).corroborated_events,
                        b.summary(&s).corroborated_events,
                        "{preset} seed {seed}: events of {s} at {at}"
                    );
                }
                assert_eq!(Some(a.peak_unrevoked_events()), b.peak_unrevoked_events());
                assert_eq!(a.subjects(), b.subjects());
            }
        }
        assert!(
            revoked > 10,
            "the stream must exercise decisions: {revoked}"
        );
    }

    /// `legacy-window` is `LegacyWindow` at the legacy operating point.
    #[test]
    fn legacy_window_is_the_legacy_gate() {
        let checks = vec!["positionJump".to_string()];
        let mut revoked = 0;
        for seed in 1..=4_u64 {
            let mut a = LegacyWindow::legacy_defaults();
            let mut b = MaPolicy::new(MaPolicyParams::preset("legacy-window").expect("preset"));
            for (r, s, at) in stream(seed, 1_000, 5, 7) {
                let x = a.ingest_pair(&r, &s, at).map(|d| d.subject().to_string());
                let y = b.ingest(&ev(&r, &s, at, &checks)).map(|d| d.subject);
                assert_eq!(x, y, "seed {seed} at {at}");
                revoked += usize::from(x.is_some());
            }
        }
        assert!(revoked > 5, "{revoked}");
    }

    /// `pseudonym-k` revokes at the k-th distinct reporter of one pseudonym, whatever
    /// the gap; `device-k` pools a device's pseudonyms; k = 1 revokes on one report and
    /// is marked unsound.
    #[test]
    fn replay_rules_count_reporters() {
        let c = vec!["implausibleSpeed".to_string()];
        let mut p = MaPolicy::new(MaPolicyParams::preset("pseudonym-k").expect("preset"));
        assert!(p.ingest(&ev("r1", "s", NS_PER_S, &c)).is_none());
        assert!(p.ingest(&ev("r1", "s", 2 * NS_PER_S, &c)).is_none());
        assert!(p.ingest(&ev("r2", "s", 3_000 * NS_PER_S, &c)).is_none());
        let d = p.ingest(&ev("r3", "s", 9_000 * NS_PER_S, &c));
        assert_eq!(d.map(|d| d.case).as_deref(), Some("s"));

        let mut d = MaPolicy::new(MaPolicyParams::preset("device-k").expect("preset"));
        let on = |r: &'static str, s: &'static str, t: u64| PolicyEvidence {
            device: Some("dev-1"),
            ..ev(r, s, t * NS_PER_S, &c)
        };
        assert!(d.ingest(&on("r1", "p1", 10)).is_none());
        assert!(d.ingest(&on("r2", "p2", 20)).is_none());
        let got = d
            .ingest(&on("r3", "p3", 30))
            .expect("three reporters over the device");
        assert_eq!((got.subject.as_str(), got.case.as_str()), ("p3", "dev-1"));

        let one = MaPolicyParams {
            min_distinct_reporters: 1,
            ..MaPolicyParams::preset("pseudonym-k").expect("preset")
        };
        assert!(!one.sound());
        assert!(
            MaPolicyParams::preset("pseudonym-k")
                .expect("preset")
                .sound()
        );
        assert!(MaPolicyParams::preset("catch").expect("preset").sound());
        let mut p = MaPolicy::new(one);
        assert!(p.ingest(&ev("r1", "s", NS_PER_S, &c)).is_some());
    }

    /// A subtype scored 0 is not evidence; the score threshold sums each reporter's best
    /// report; CaTch discounts what the stated accuracy explains; the half-life fades it.
    #[test]
    fn scores_accuracy_and_decay_move_the_decision() {
        let speed = vec!["implausibleSpeed".to_string()];
        let mut ignore = MaPolicyParams::preset("pseudonym-k").expect("preset");
        ignore.scores.insert("MaxSpeed".to_string(), 0.0);
        let mut p = MaPolicy::new(ignore.clone());
        for (i, r) in ["r1", "r2", "r3", "r4"].iter().enumerate() {
            assert!(
                p.ingest(&ev(r, "s", (i as u64 + 1) * NS_PER_S, &speed))
                    .is_none()
            );
        }
        assert_eq!(p.subjects(), 0, "an ignored subtype is not evidence");
        // The check's own key wins over its subtype's.
        ignore
            .scores
            .insert("detect/implausibleSpeed".to_string(), 2.0);
        assert!((ignore.score_of(&speed) - 2.0).abs() < 1e-12);

        let jump = vec!["positionJump".to_string()];
        let mut catch = MaPolicy::new(MaPolicyParams::preset("catch").expect("preset"));
        // Accuracy 1 m, excess 4: factor 1 − 2/4 = 0.5 per reporter; θ = 3 needs six.
        let acc = |r: &'static str, t: u64| PolicyEvidence {
            excess: 4.0,
            ..ev(r, "s", t * NS_PER_S, &jump)
        };
        for (i, r) in ["r1", "r2", "r3", "r4", "r5"].iter().enumerate() {
            assert!(catch.ingest(&acc(r, i as u64 + 1)).is_none(), "{r}");
        }
        assert!(catch.ingest(&acc("r6", 6)).is_some());

        let mut half = MaPolicyParams::preset("pseudonym-k").expect("preset");
        half.min_distinct_reporters = 1;
        half.min_score = 1.5;
        half.decay = Decay::HalfLife { half_life_s: 10.0 };
        let mut p = MaPolicy::new(half.clone());
        assert!(p.ingest(&ev("r1", "s", NS_PER_S, &speed)).is_none());
        // 100 s later the first report is worth 2⁻¹⁰: 1 + 0.001 < 1.5.
        assert!(p.ingest(&ev("r2", "s", 101 * NS_PER_S, &speed)).is_none());
        let mut p = MaPolicy::new(half);
        assert!(p.ingest(&ev("r1", "s", NS_PER_S, &speed)).is_none());
        assert!(p.ingest(&ev("r2", "s", 2 * NS_PER_S, &speed)).is_some());
    }

    /// The daily multiplier applies once per Eastern `hour:00` crossed (fixed EST
    /// stand-alone): 05:00 EST is 10:00 UTC.
    #[test]
    fn the_daily_multiplier_counts_eastern_mornings() {
        let t0 = 1_767_225_600_i64; // 2026-01-01 00:00 UTC
        let p = MaPolicy::new(MaPolicyParams {
            decay: Decay::DailyMultiplier {
                factor: 0.5,
                hour_et: 5,
            },
            ..MaPolicyParams::preset("pseudonym-k").expect("preset")
        })
        .with_calendar(t0, fixed_est_count);
        let h = |x: u64| x * 3_600 * NS_PER_S;
        assert!((p.decay(h(9), h(10) - 1) - 1.0).abs() < 1e-12);
        assert!((p.decay(h(9), h(10)) - 0.5).abs() < 1e-12);
        assert!((p.decay(h(9), h(58)) - 0.125).abs() < 1e-12);
    }

    /// The cluster guard: once `c` devices of one class are accused, a device in it
    /// needs `m·k` reporters.
    #[test]
    fn the_cluster_guard_raises_the_bar_in_a_crowded_class() {
        let c = vec!["implausibleSpeed".to_string()];
        let mut p = MaPolicy::new(MaPolicyParams::preset("cluster").expect("preset"));
        let on = |r: &'static str, dev: &'static str, t: u64| PolicyEvidence {
            device: Some(dev),
            ..ev(r, dev, t * NS_PER_S, &c)
        };
        // Three devices accused once each: the class is crowded.
        for (i, dev) in ["d1", "d2", "d3"].iter().enumerate() {
            assert!(p.ingest(&on("r1", dev, i as u64 + 1)).is_none());
        }
        // d1 reaches k = 3 reporters but needs 9 now.
        assert!(p.ingest(&on("r2", "d1", 10)).is_none());
        assert!(p.ingest(&on("r3", "d1", 11)).is_none());
        let mut off = MaPolicy::new(MaPolicyParams {
            guard: PopulationGuard::Off,
            ..MaPolicyParams::preset("cluster").expect("preset")
        });
        for (i, dev) in ["d1", "d2", "d3"].iter().enumerate() {
            assert!(off.ingest(&on("r1", dev, i as u64 + 1)).is_none());
        }
        assert!(off.ingest(&on("r2", "d1", 10)).is_none());
        assert!(off.ingest(&on("r3", "d1", 11)).is_some());
    }

    /// The rule keys are the five SM-MBR subtypes and every engine check.
    #[test]
    fn rule_keys_cover_every_check() {
        let mut want: Vec<String> = ["RandomPosition", "ConstantPosition", "MaxSpeed"]
            .iter()
            .chain(["MaxAcceleration", "BrakeAcceleration"].iter())
            .map(ToString::to_string)
            .collect();
        for d in crate::detect::DetectorId::ALL {
            want.push(format!("detect/{}", d.as_str()));
        }
        want.push(format!(
            "detect/{}",
            crate::detect::DetectorId::ImplausibleSpeed.as_str()
        ));
        assert_eq!(RULE_KEYS.to_vec(), want);
        for id in PRESET_IDS {
            assert!(MaPolicyParams::preset(id).is_some(), "{id}");
        }
        assert!(MaPolicyParams::preset("nonesuch").is_none());
    }
}
