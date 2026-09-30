//! The Misbehaviour Authority's decision on **independent evidence over time**:
//! `threat/ma/corroborated`.
//!
//! # What it decides on
//!
//! A misbehaviour report is a witness statement about one message (TS 103 759 §7: a
//! report carries the reported message and the reporter's observation of it; SAE J3287
//! the same). When a vehicle broadcasts one implausible message, every receiver in range
//! may report it — eight reporters of one message are eight witnesses of **one** event, not
//! eight pieces of evidence. The legacy persistence gate ([`crate::ma::LegacyWindow`])
//! counted reporters and distinct seconds, so one 3 s GNSS degradation of one honest
//! vehicle, heard by a dense fleet, met "3 reporters, 4 distinct seconds, 3 s span" by
//! itself: the shipped `revocation-latency` scenario with its attackers removed revoked 21
//! honest vehicles in 300 s at 6,000 veh/h, and `phase2-manhattan` one.
//!
//! This pipeline correlates first and counts second:
//!
//! 1. **Correlation into events.** The evidence about one subject (one pseudonym
//!    certificate: the only identity a report carries) is kept in observation-time order.
//!    It is cut into events greedily: an event opens at its earliest observation and
//!    absorbs every observation within `event_window_s` of that opening. However many
//!    reporters, and however many reports each filed, an event is one piece of evidence.
//! 2. **Corroboration of an event.** An event counts only when at least
//!    `event_min_reporters` distinct *trusted* reporters witnessed it, so one broken or
//!    lying receiver never makes an event on its own.
//! 3. **Independence over time.** The authority revokes when at least `min_events`
//!    corroborated events fall inside the sliding `window_s`, witnessed between them by at
//!    least `min_reporters` distinct trusted reporters. Events are disjoint in time by
//!    construction, so `min_events` events are sustained misbehaviour over at least
//!    `(min_events − 1) · event_window_s` seconds — longer, by construction, than a benign
//!    fault episode lasts.
//!
//! The collusion defence (the reporter budget, the reputation cap and trusted
//! infrastructure) is the legacy pipeline's, unchanged, because it answers a different
//! question — which reporters to believe — from the one this pipeline adds: what counts as
//! more than one piece of evidence.
//!
//! # Why these defaults
//!
//! Neither the SCMS design nor the ETSI standards specify the authority's decision rule:
//! Brecht et al. 2018 (IEEE T-ITS 19(12), §VI) leave "global misbehavior detection" to
//! the MA, TS 103 759 V2.1.1 standardises the report and leaves the reaction out of scope,
//! and SAE J3287 does the same for the US report format. So every threshold here is a
//! **design choice with a stated reason**, marked as such on the card rather than dressed
//! as a citation:
//!
//! * `event_window_s` = 5 s is F2MD's `DELTA_REPORT_TIME` (veins-f2md
//!   `F2MDParameters.h`): the minimum time a reporter lets pass before it reports the same
//!   pseudonym again. One event window is then one report per reporter at most, and the
//!   window outlasts the simulator's benign GNSS degradation burst (3 s,
//!   `mobility/gnss/gauss-markov` `burst_duration_s`) plus the detectors' 1.5 s motion lag.
//! * `event_min_reporters` = 2: an event is corroborated when a second, independent
//!   receiver saw it. The legacy authority's k = 3 is kept as `min_reporters` over the
//!   whole decision.
//! * `min_events` = 4 over `window_s` = 60 s: at least 15 s of sustained, independently
//!   witnessed misbehaviour inside one minute. A benign burst produces at most two events;
//!   revoking an honest vehicle then takes further independent fault episodes of the same
//!   pseudonym inside the minute, each itself corroborated. The window equals the shortest
//!   pseudonym lifetime a shipped scenario runs (60 s i-periods), because the authority's
//!   evidence is about one certificate and a longer window buys nothing across a change.

use std::collections::{BTreeMap, BTreeSet};

use crate::cards::design;
use crate::ctx::{ThreatCtx, ThreatCtxExt};
use crate::ma::{MaAction, MaPipeline};
use crate::records::{MaDecisionRecord, MaReportRecord};
use crate::report::MisbehaviourReport;
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::model::Model;
use v2xw_core::time::{SimTime, secs_to_ns};

/// The model id the pipeline's card and its decisions carry.
pub const MODEL_ID: &str = "threat/ma/corroborated";

/// The operating point.
#[derive(Debug, Clone, PartialEq)]
pub struct CorroborationParams {
    /// Observations of one subject within this long of an event's first observation are
    /// that one event, seconds (F2MD `DELTA_REPORT_TIME`, 5).
    pub event_window_s: f64,
    /// Distinct trusted reporters an event needs to count (2).
    pub event_min_reporters: usize,
    /// Corroborated events needed inside the window (4).
    pub min_events: usize,
    /// The sliding window the events must fall in, seconds (60).
    pub window_s: f64,
    /// Distinct trusted reporters over the counted events (the legacy k, 3).
    pub min_reporters: usize,
    /// Whether the trusted-reporter gate is on (the legacy `ma_defense`, true).
    pub defence: bool,
    /// A reporter itself reported more than this many times is distrusted (legacy, 40).
    pub reputation_max: u32,
    /// A reporter filing more than this many reports is rate-limited (legacy, 30).
    pub report_budget: u32,
}

impl Default for CorroborationParams {
    fn default() -> Self {
        Self {
            event_window_s: 5.0,
            event_min_reporters: 2,
            min_events: 4,
            window_s: 60.0,
            min_reporters: 3,
            defence: true,
            reputation_max: 40,
            report_budget: 30,
        }
    }
}

/// What the authority holds about one subject, as its decision reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EvidenceSummary {
    /// Reports inside the window.
    pub reports: u32,
    /// Events those reports correlate into.
    pub events: u32,
    /// Of those, the events enough trusted reporters witnessed.
    pub corroborated_events: u32,
    /// Distinct trusted reporters over the corroborated events.
    pub trusted_reporters: u32,
}

/// The corroborating authority pipeline.
#[derive(Debug, Clone)]
pub struct CorroboratedMa {
    card: ModelCard,
    params: CorroborationParams,
    /// Per subject: `(observation instant, reporter digest)`, oldest first.
    evidence: BTreeMap<String, Vec<(SimTime, String)>>,
    filed_by: BTreeMap<String, u32>,
    reported: BTreeMap<String, u32>,
    revoked: BTreeSet<String>,
    infrastructure: BTreeSet<String>,
    decisions: Vec<MaAction>,
    /// The most corroborated events any subject reached without being revoked — the
    /// margin an honest fleet leaves to the threshold, for the run report.
    peak_unrevoked: u32,
}

impl CorroboratedMa {
    /// The pipeline at the given operating point.
    #[must_use]
    pub fn new(params: CorroborationParams) -> Self {
        Self {
            card: card(&params),
            params,
            evidence: BTreeMap::new(),
            filed_by: BTreeMap::new(),
            reported: BTreeMap::new(),
            revoked: BTreeSet::new(),
            infrastructure: BTreeSet::new(),
            decisions: Vec::new(),
            peak_unrevoked: 0,
        }
    }

    /// The pipeline at its default operating point.
    #[must_use]
    pub fn defaults() -> Self {
        Self::new(CorroborationParams::default())
    }

    /// The operating point it runs at.
    #[must_use]
    pub fn params(&self) -> &CorroborationParams {
        &self.params
    }

    /// Declares a certificate as trusted infrastructure: never rate-limited or distrusted.
    pub fn trust_infrastructure(&mut self, digest: impl Into<String>) {
        self.infrastructure.insert(digest.into());
    }

    /// Whether the subject has been revoked.
    #[must_use]
    pub fn is_revoked(&self, subject: &str) -> bool {
        self.revoked.contains(subject)
    }

    /// The most corroborated events a subject that was never revoked reached.
    #[must_use]
    pub fn peak_unrevoked_events(&self) -> u32 {
        self.peak_unrevoked
    }

    /// Whether a reporter's evidence counts (the legacy `trusted()`).
    #[must_use]
    pub fn trusted(&self, reporter: &str) -> bool {
        if !self.params.defence || self.infrastructure.contains(reporter) {
            return true;
        }
        !self.revoked.contains(reporter)
            && self.filed_by.get(reporter).copied().unwrap_or(0) <= self.params.report_budget
            && self.reported.get(reporter).copied().unwrap_or(0) < self.params.reputation_max
    }

    /// What the authority holds about `subject` right now, window applied at its newest
    /// observation.
    #[must_use]
    pub fn summary(&self, subject: &str) -> EvidenceSummary {
        let Some(ev) = self.evidence.get(subject) else {
            return EvidenceSummary::default();
        };
        let Some(latest) = ev.last().map(|e| e.0) else {
            return EvidenceSummary::default();
        };
        let cutoff = latest.saturating_sub(secs_to_ns(self.params.window_s));
        let inside: Vec<&(SimTime, String)> = ev.iter().filter(|(t, _)| *t >= cutoff).collect();
        let span = secs_to_ns(self.params.event_window_s).max(1);
        let mut out = EvidenceSummary {
            reports: u32::try_from(inside.len()).unwrap_or(u32::MAX),
            ..EvidenceSummary::default()
        };
        let mut witnesses: BTreeSet<&str> = BTreeSet::new();
        let mut i = 0;
        while i < inside.len() {
            let opened = inside[i].0;
            let mut reporters: BTreeSet<&str> = BTreeSet::new();
            while i < inside.len() && inside[i].0 < opened.saturating_add(span) {
                let r = inside[i].1.as_str();
                if self.trusted(r) {
                    reporters.insert(r);
                }
                i += 1;
            }
            out.events += 1;
            if reporters.len() >= self.params.event_min_reporters {
                out.corroborated_events += 1;
                witnesses.extend(reporters);
            }
        }
        out.trusted_reporters = u32::try_from(witnesses.len()).unwrap_or(u32::MAX);
        out
    }

    /// Correlates the evidence about one subject and decides.
    fn correlate(&mut self, subject: &str) -> Option<MaAction> {
        if self.revoked.contains(subject) {
            return None;
        }
        let s = self.summary(subject);
        if s.corroborated_events as usize >= self.params.min_events
            && s.trusted_reporters as usize >= self.params.min_reporters
        {
            self.revoked.insert(subject.to_string());
            return Some(MaAction::Revoke {
                subject: subject.to_string(),
            });
        }
        self.peak_unrevoked = self.peak_unrevoked.max(s.corroborated_events);
        None
    }

    /// Records one report's evidence, dated at `at` (its observation, not its arrival:
    /// behind the RA's shuffle a batch arrives at one instant carrying a window's worth of
    /// observations), and decides, **emitting nothing**. The same contract as
    /// [`crate::ma::LegacyWindow::ingest_evidence`], so a host can swap one for the other.
    pub fn ingest_evidence(&mut self, r: &MisbehaviourReport, at: SimTime) -> Option<MaAction> {
        *self
            .filed_by
            .entry(r.reporter_cert_digest.clone())
            .or_insert(0) += 1;
        *self
            .reported
            .entry(r.subject_cert_digest.clone())
            .or_insert(0) += 1;
        let window = secs_to_ns(self.params.window_s);
        let ev = self
            .evidence
            .entry(r.subject_cert_digest.clone())
            .or_default();
        let pos = ev.partition_point(|(t, _)| *t <= at);
        ev.insert(pos, (at, r.reporter_cert_digest.clone()));
        // Evidence older than one window before the newest can never count again: the
        // window only moves forward with the newest observation.
        let latest = ev.last().map_or(at, |e| e.0);
        let cutoff = latest.saturating_sub(window);
        let stale = ev.partition_point(|(t, _)| *t < cutoff);
        ev.drain(..stale);
        self.correlate(&r.subject_cert_digest)
    }

    /// [`CorroboratedMa::ingest_evidence`] dated at the report's arrival.
    pub fn ingest(&mut self, r: &MisbehaviourReport) -> Option<MaAction> {
        self.ingest_evidence(r, r.ingest_time)
    }
}

impl Model for CorroboratedMa {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl MaPipeline for CorroboratedMa {
    fn on_report(&mut self, ctx: &mut dyn ThreatCtx, r: &MisbehaviourReport) -> Vec<MaAction> {
        ctx.emit(MaReportRecord {
            t: r.ingest_time,
            reporter: r.reporter,
            subject: r.subject_cert_digest.clone(),
            detector: r.leading_reason().map(str::to_string),
        });
        let mut out = Vec::new();
        if let Some(a) = self.ingest(r) {
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
        // Evidence only grows on a report, and the decision only reads evidence, so a
        // tick with no report cannot change a decision.
        Vec::new()
    }

    fn decisions(&self) -> &[MaAction] {
        &self.decisions
    }
}

fn choice(name: &str, unit: &str, default: serde_json::Value, why: &str) -> Parameter {
    let mut p = Parameter::new(
        name,
        unit,
        default,
        Source {
            kind: SourceKind::TodoCalibrate,
            reference: "design choice: the MA decision rule is unspecified by Brecht et al. \
                        2018 §VI, ETSI TS 103 759 and SAE J3287"
                .to_string(),
            accessed: Some("2026-09-30".to_string()),
            note: Some(why.to_string()),
        },
    );
    p.calibration = Some(
        "measure honest-fleet revocations (none allowed) and attacker recall and latency \
         over the shipped scenarios with `mbd_eval`; keep the smallest thresholds that hold \
         zero honest revocations at 6,000 veh/h"
            .to_string(),
    );
    p
}

/// The model card.
#[must_use]
pub fn card(p: &CorroborationParams) -> ModelCard {
    use serde_json::json;
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::MaPipeline,
        "1.0.0",
        "The Misbehaviour Authority's decision on independent evidence over time: reports \
         about one pseudonym are correlated into events (many witnesses of one message are \
         one piece of evidence), an event counts when two independent trusted reporters \
         witnessed it, and the authority revokes on several such events inside a window.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    card.equations = vec![
        Equation::new(
            "events",
            "E = greedy partition of the subject's observations: an event opens at its \
             earliest observation t₀ and holds every observation in [t₀, t₀ + w_e)",
        ),
        Equation::new(
            "revocation condition",
            "|{e ∈ E in window : |trusted reporters(e)| ≥ k_e}| ≥ n ∧ \
             |∪ trusted reporters of those events| ≥ k",
        ),
    ];
    let mut window = Parameter::new(
        "event_window_s",
        "s",
        json!(p.event_window_s),
        Source {
            kind: SourceKind::Code,
            reference: "veins-f2md src/veins/modules/application/f2md/F2MDParameters.h :: \
                        DELTA_REPORT_TIME = 5"
                .to_string(),
            accessed: Some("2026-09-30".to_string()),
            note: Some(
                "F2MD's minimum interval between two reports by one reporter about one \
                 pseudonym, used here as the span of one event so a reporter contributes \
                 at most one report per event"
                    .to_string(),
            ),
        },
    );
    window.range = Some(vec![json!(0.1), json!(600.0)]);
    card.parameters = vec![
        window,
        choice(
            "event_min_reporters",
            "-",
            json!(p.event_min_reporters),
            "an event is corroborated when a second independent receiver witnessed it",
        ),
        choice(
            "min_events",
            "-",
            json!(p.min_events),
            "four disjoint 5 s events are 15 s or more of sustained misbehaviour, several \
             times the benign GNSS burst of mobility/gnss/gauss-markov (3 s)",
        ),
        choice(
            "window_s",
            "s",
            json!(p.window_s),
            "the shortest pseudonym lifetime a shipped scenario runs (60 s i-periods); the \
             evidence is about one certificate",
        ),
        Parameter::new(
            "min_reporters",
            "-",
            json!(p.min_reporters),
            crate::cards::legacy(crate::cards::LEGACY_PY, "PipelineConfig.report_threshold_k"),
        ),
        Parameter::new(
            "ma_defence",
            "-",
            json!(p.defence),
            crate::cards::legacy(crate::cards::LEGACY_PY, "PipelineConfig.ma_defense"),
        ),
        Parameter::new(
            "reputation_max",
            "-",
            json!(p.reputation_max),
            crate::cards::legacy(crate::cards::LEGACY_PY, "PipelineConfig.reputation_max"),
        ),
        Parameter::new(
            "report_budget",
            "-",
            json!(p.report_budget),
            crate::cards::legacy(crate::cards::LEGACY_PY, "PipelineConfig.report_budget"),
        ),
    ];
    card.sources = vec![
        crate::cards::paper(
            "Brecht et al., A Security Credential Management System for V2X Communications, \
             IEEE T-ITS 19(12), 2018, §VI (misbehavior detection and revocation)",
        ),
        crate::cards::standard(
            "ETSI TS 103 759 V2.1.1 (2023-01): misbehaviour report format; the MA's \
             reaction is out of its scope",
        ),
        crate::cards::standard("SAE J3287: misbehavior reporting format (US)"),
        design("07-threats-and-detection.md §3.2"),
    ];
    card.assumptions = vec![
        "A report names its subject by pseudonym digest only; evidence is correlated per \
         certificate, and linking a device's certificates is the protocol's investigation, \
         not this pipeline's."
            .to_string(),
        "Evidence is dated by observation (the report's detection time), because the RA's \
         shuffle delivers a batch of observations at one instant."
            .to_string(),
    ];
    card.limitations = vec![
        "Events are cut by time only: two different misbehaviours of one subject inside one \
         event window are one event."
            .to_string(),
        "No per-check weighting: a cryptographic failure and a kinematic residual count \
         alike once corroborated."
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
            "ma_corroborate::one_event_heard_by_many_is_one_piece_of_evidence".to_string(),
            "ma_corroborate::sustained_corroborated_misbehaviour_is_revoked".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::CollectingCtx;
    use crate::detect::{DetectorId, Fingerprint, Observation, Verdict};
    use v2xw_core::ids::NodeId;
    use v2xw_core::time::NS_PER_S;

    fn report(reporter: u32, subject: &str, t: SimTime) -> MisbehaviourReport {
        let mut f = Fingerprint::default();
        f.set(DetectorId::PositionSpeedInconsistency, 1.5);
        let v = Verdict {
            subject: subject.to_string(),
            fingerprint: f,
            fired: vec![Observation {
                detector: DetectorId::PositionSpeedInconsistency,
                score: 1.5,
                subject: subject.to_string(),
                at: t,
            }],
        };
        MisbehaviourReport::from_verdict(
            format!("r{reporter}-{t}"),
            NodeId::new(reporter),
            format!("rep{reporter:04}"),
            &v,
            &crate::report::Evidence::at(t, t, 5.0),
        )
        .expect("a fired verdict is a report")
    }

    /// The integrator's question, answered as the PM decided: eight reporters of one
    /// burst — the legacy gate's k = 3, 4 distinct seconds and a 3 s span all met — are
    /// one piece of evidence, and nothing is revoked.
    #[test]
    fn one_event_heard_by_many_is_one_piece_of_evidence() {
        let mut ctx = CollectingCtx::new(1);
        let mut ma = CorroboratedMa::defaults();
        let mut legacy = crate::ma::LegacyWindow::legacy_defaults();
        let mut legacy_revoked = false;
        for tenth in 0..40u64 {
            let t = 257 * NS_PER_S + tenth * NS_PER_S / 10;
            let r = report((tenth % 8) as u32 + 1, "honest", t);
            assert!(ma.on_report(&mut ctx, &r).is_empty());
            legacy_revoked |= legacy.ingest_evidence(&r, t).is_some();
        }
        assert!(
            legacy_revoked,
            "the legacy gate revokes on this burst, which is the defect"
        );
        let s = ma.summary("honest");
        assert_eq!(s.reports, 40);
        assert!(s.corroborated_events <= 1, "{s:?}");
        assert!(!ma.is_revoked("honest"));
    }

    #[test]
    fn sustained_corroborated_misbehaviour_is_revoked() {
        let mut ctx = CollectingCtx::new(1);
        let mut ma = CorroboratedMa::defaults();
        let mut revoked_at = None;
        // Three reporters, one report each a second, for 30 s.
        for s in 0..30u64 {
            for reporter in 1..=3 {
                let r = report(reporter, "liar", 100 * NS_PER_S + s * NS_PER_S);
                if !ma.on_report(&mut ctx, &r).is_empty() && revoked_at.is_none() {
                    revoked_at = Some(s);
                }
            }
        }
        // Four 5 s events: the fourth opens at 15 s.
        assert_eq!(revoked_at, Some(15));
        assert!(ma.is_revoked("liar"));
        assert_eq!(ctx.on_channel("ma.decision").len(), 1);
    }

    #[test]
    fn one_witness_never_corroborates_however_long_it_reports() {
        let mut ctx = CollectingCtx::new(1);
        let mut ma = CorroboratedMa::defaults();
        for s in 0..60u64 {
            let r = report(7, "subject", s * NS_PER_S);
            assert!(ma.on_report(&mut ctx, &r).is_empty());
        }
        assert_eq!(ma.summary("subject").corroborated_events, 0);
    }

    #[test]
    fn events_spread_beyond_the_window_do_not_accumulate() {
        let mut ctx = CollectingCtx::new(1);
        let mut ma = CorroboratedMa::defaults();
        // A corroborated event every 40 s: never four inside one minute.
        for k in 0..10u64 {
            for reporter in 1..=3 {
                let r = report(reporter, "rare", k * 40 * NS_PER_S);
                assert!(ma.on_report(&mut ctx, &r).is_empty());
            }
        }
        assert!(!ma.is_revoked("rare"));
        assert!(ma.peak_unrevoked_events() <= 2);
    }

    #[test]
    fn evidence_arriving_out_of_order_decides_as_in_order() {
        let mut a = CorroboratedMa::defaults();
        let mut b = CorroboratedMa::defaults();
        let mut reports = Vec::new();
        for s in 0..20u64 {
            for reporter in 1..=2 {
                reports.push(report(reporter, "x", s * NS_PER_S));
            }
        }
        let forward: Vec<bool> = reports
            .iter()
            .map(|r| a.ingest_evidence(r, r.detection_time).is_some())
            .collect();
        // A shuffle delivers them in another order.
        let mut shuffled = reports.clone();
        shuffled.reverse();
        let backward: Vec<bool> = shuffled
            .iter()
            .map(|r| b.ingest_evidence(r, r.detection_time).is_some())
            .collect();
        assert_eq!(forward.iter().filter(|x| **x).count(), 1);
        assert_eq!(backward.iter().filter(|x| **x).count(), 1);
        assert_eq!(a.summary("x"), b.summary("x"));
    }

    #[test]
    fn the_card_names_every_parameter() {
        let c = card(&CorroborationParams::default());
        for name in [
            "event_window_s",
            "event_min_reporters",
            "min_events",
            "window_s",
            "min_reporters",
        ] {
            assert!(c.parameters.iter().any(|p| p.name == name), "{name}");
        }
    }
}
