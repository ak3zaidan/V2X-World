//! The records this crate writes, on the channels `v2xw-metrics` already reads.
//!
//! # No second scoring scheme
//!
//! `v2xw_metrics::detection` builds its confusion matrices from four channels —
//! `det.observation`, `ma.report`, `ma.decision` and `gt.attack.action` — and its
//! reader-side views (`v2xw_metrics::channels::DetObservationView` and friends) fix the
//! field names. The types below are the writer side of exactly those views, field for
//! field, so a detector firing in this crate lands in the metrics crate's matrix without
//! anything in between deciding what a true positive is. The detection metrics need the
//! confusion-matrix inputs; these are they, and there is no other scorer.
//!
//! The one thing this crate does **not** supply is the subject-to-actor link. A report
//! names its subject by pseudonym digest, and whether that digest belongs to an attacker
//! is ground truth: the run declares it to the metrics provider
//! (`DetectionProvider::declare_subject`) from the identity map, exactly as the legacy
//! `gt_idmap` did. A detector that could resolve a digest to an actor would be reading
//! through the firewall.
//!
//! # Quantisation
//!
//! Every float here is quantised at construction to [`SCORE_Q`] (no
//! float reaches a recorded artefact in raw IEEE-754 form). The quantum is the legacy
//! engine's own `round(x, 3)`, so a ported detector's score serialises to the same digits
//! the legacy corpus carries.

use serde::{Deserialize, Serialize};
use v2xw_core::ctx::{Record, Visibility};
use v2xw_core::ids::{ActorId, NodeId};
use v2xw_core::math::quantize_to;
use v2xw_core::time::SimTime;

/// The quantum every score and magnitude in this module is rounded to: 1e-3.
///
/// The legacy engine wrote every detector score as `round(x, 3)`,
/// and 1e-3 is the legacy quantum. Keeping it means a ported score and a legacy score are
/// comparable digit for digit.
pub const SCORE_Q: f64 = v2xw_core::math::LEGACY_QUANTUM;

/// Rounds a score to [`SCORE_Q`].
#[must_use]
pub fn q(x: f64) -> f64 {
    quantize_to(x, SCORE_Q)
}

/// `det.observation` — one local detector firing at one node (NODE).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetObservation {
    /// The instant.
    pub t: SimTime,
    /// The observing node.
    pub node: NodeId,
    /// The detector's id, e.g. `positionSpeedInconsistency`.
    pub detector: String,
    /// The subject's pseudonym digest, lowercase hex — what the node can see, not who it
    /// really is.
    pub subject: String,
    /// The detector's normalised score, `≈ 1` at its firing threshold.
    #[serde(default)]
    pub score: Option<f64>,
    /// The judged message's `msgCnt`, when the detector reads one (the US profile).
    /// Absent otherwise, so every other detector's row is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msg_cnt: Option<u8>,
    /// The `msgCnt` of the previous BSM a pair check compared it with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_msg_cnt: Option<u8>,
    /// The pair's Δt on the detector's configured clock, s (pair checks only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dt_s: Option<f64>,
    /// The quantity the check compared with its threshold, in the check's own unit:
    /// metres for a position check, m/s for a speed, m/s² for an acceleration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub residual: Option<f64>,
}

impl Record for DetObservation {
    const CHANNEL: &'static str = "det.observation";
    const VISIBILITY: Visibility = Visibility::Node;
}

impl DetObservation {
    /// A record with its score quantised to [`SCORE_Q`].
    #[must_use]
    pub fn new(t: SimTime, node: NodeId, detector: &str, subject: &str, score: f64) -> Self {
        Self {
            t,
            node,
            detector: detector.to_string(),
            subject: subject.to_string(),
            score: Some(q(score)),
            msg_cnt: None,
            prev_msg_cnt: None,
            dt_s: None,
            residual: None,
        }
    }

    /// The same row with the pair a check judged: the message's and its predecessor's
    /// `msgCnt`, the pair's Δt (quantised to 1e-3 s) and the check's residual (quantised
    /// to 1e-6 in its unit).
    #[must_use]
    pub fn with_pair(
        mut self,
        msg_cnt: Option<u8>,
        prev_msg_cnt: Option<u8>,
        dt_s: Option<f64>,
        residual: Option<f64>,
    ) -> Self {
        self.msg_cnt = msg_cnt;
        self.prev_msg_cnt = prev_msg_cnt;
        self.dt_s = dt_s.map(|d| quantize_to(d, 1e-3));
        self.residual = residual.map(|r| quantize_to(r, 1e-6));
        self
    }
}

/// `ma.report` — a misbehaviour report as it reached the authority (NODE).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaReportRecord {
    /// The instant the report reached the authority.
    pub t: SimTime,
    /// The reporting node.
    #[serde(default)]
    pub reporter: Option<NodeId>,
    /// The subject, as the reporter could name it: a pseudonym digest in hex.
    pub subject: String,
    /// The detector that produced the report — the highest-scoring fired check.
    #[serde(default)]
    pub detector: Option<String>,
}

impl Record for MaReportRecord {
    const CHANNEL: &'static str = "ma.report";
    const VISIBILITY: Visibility = Visibility::Node;
}

/// `ma.decision` — the authority's decision about a subject (NODE).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaDecisionRecord {
    /// The instant of the decision.
    pub t: SimTime,
    /// The subject.
    pub subject: String,
    /// The decision: `revoke`, `dismiss` or `investigate`.
    pub decision: String,
    /// What the dataset release adds (the deciding MA, its rule, the reports it acted
    /// on), only when the scenario names the dataset exporter; absent from the JSON
    /// otherwise.
    #[serde(flatten)]
    pub dataset: Option<MaDecisionDataset>,
}

/// The dataset's view of one MA decision (`ma.decision`, NODE: all of it is the MA's own
/// knowledge).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaDecisionDataset {
    /// The provider index of the deciding MA.
    pub ma: u32,
    /// The MA rule that decided (the scenario's `detection.ma.id`).
    pub rule: String,
    /// `pseudonym` (the decision names a certificate) or `device` (it names a linked
    /// chain).
    pub subject_kind: String,
    /// The MA's `la_link` token of the subject, when it asked the LAs.
    pub la_link: Option<String>,
    /// The digests of the reports about the subject the MA held at the decision.
    pub report_digests: Vec<String>,
    /// How many reports about the subject the MA held.
    pub n_reports: u64,
    /// How many distinct reporter certificates they came from.
    pub n_distinct_reporters: u64,
}

impl Record for MaDecisionRecord {
    const CHANNEL: &'static str = "ma.decision";
    const VISIBILITY: Visibility = Visibility::Node;
}

/// `gt.attack.action` — an attacker's action with the true actor behind it (GT).
///
/// Invariant I-T3: every action that changes bytes on the air is logged here, with the
/// true actor id, on a ground-truth channel only. This is the record the per-message
/// `falsified` label is derived from, and it is the reason an attacker
/// may not compute that label itself: the attacker does not know which actor it is.
///
/// The [`Visibility::Gt`] tag is what keeps it out of every NODE export and away from
/// every detector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GtAttackAction {
    /// The instant.
    pub t: SimTime,
    /// The true actor behind the action — the field I-T3 requires.
    pub actor: ActorId,
    /// The attacker model's id, e.g. `threat/attacker/legacy-catalog`.
    pub attacker: String,
    /// The action, as [`crate::attack::AttackAction::name`] spells it.
    pub action: String,
    /// The fields the action changed, sorted, e.g. `["heading", "position"]`.
    #[serde(default)]
    pub fields: Vec<String>,
    /// Whether the action changed bytes on the air.
    #[serde(default = "crate::records::yes")]
    pub changed_bytes_on_air: bool,
    /// The message id the action produced, for joining to `node.tx`.
    #[serde(default)]
    pub msg: Option<u64>,
    /// The magnitude of the falsification, in the units of the field it changed:
    /// metres for a position edit, m/s for a speed edit, degrees for a heading edit,
    /// seconds for a timing edit, a count for a repetition edit.
    ///
    /// Not in the reader-side view the metrics crate decodes today, which ignores unknown
    /// fields; it is here because the ground-truth label asks the channel to carry "the fields
    /// changed **and the magnitude**", and because the per-message `falsified` label is a
    /// threshold on exactly this number.
    #[serde(default)]
    pub magnitude: Option<f64>,
}

impl Record for GtAttackAction {
    const CHANNEL: &'static str = "gt.attack.action";
    const VISIBILITY: Visibility = Visibility::Gt;
}

/// `serde` default for [`GtAttackAction::changed_bytes_on_air`], matching the reader-side
/// view's own default.
#[must_use]
pub const fn yes() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_detection_record_lands_on_the_channel_the_metrics_crate_reads() {
        assert_eq!(DetObservation::CHANNEL, "det.observation");
        assert_eq!(MaReportRecord::CHANNEL, "ma.report");
        assert_eq!(MaDecisionRecord::CHANNEL, "ma.decision");
        assert_eq!(GtAttackAction::CHANNEL, "gt.attack.action");
    }

    /// The firewall as a unit test: the attack channel is ground-truth tainted and may
    /// not be written to a NODE channel, and every detection channel may.
    #[test]
    fn the_attack_channel_is_ground_truth_and_the_detection_channels_are_not() {
        assert!(GtAttackAction::VISIBILITY.is_gt_tainted());
        assert!(!GtAttackAction::VISIBILITY.allowed_on_node_channel());
        for v in [
            DetObservation::VISIBILITY,
            MaReportRecord::VISIBILITY,
            MaDecisionRecord::VISIBILITY,
        ] {
            assert!(!v.is_gt_tainted());
            assert!(v.allowed_on_node_channel());
        }
    }

    #[test]
    fn scores_are_quantised_at_the_writer() {
        let r = DetObservation::new(1, NodeId::new(1), "positionJump", "ab", 1.234_567_8);
        assert_eq!(r.score, Some(1.235));
        assert!(v2xw_core::math::is_on_grid(r.score.unwrap(), SCORE_Q));
    }

    /// The field names are the contract with `v2xw_metrics::channels`; a rename here
    /// would silently stop every detection metric counting.
    #[test]
    fn the_serialised_field_names_match_the_reader_side_view() {
        let json = serde_json::to_value(DetObservation::new(
            7,
            NodeId::new(2),
            "sybilCoLocation",
            "ff00",
            2.0,
        ))
        .unwrap();
        for k in ["t", "node", "detector", "subject", "score"] {
            assert!(json.get(k).is_some(), "missing {k}");
        }
        let gt = serde_json::to_value(GtAttackAction {
            t: 1,
            actor: ActorId::new(4),
            attacker: "threat/attacker/legacy-catalog".to_string(),
            action: "FalsifyOutgoing".to_string(),
            fields: vec!["position".to_string()],
            changed_bytes_on_air: true,
            msg: Some(9),
            magnitude: Some(25.0),
        })
        .unwrap();
        for k in [
            "t",
            "actor",
            "attacker",
            "action",
            "fields",
            "changed_bytes_on_air",
            "msg",
        ] {
            assert!(gt.get(k).is_some(), "missing {k}");
        }
    }
}

/// `ma.case` — one authority case: what the resolution stage concluded and why (NODE).
///
/// The catalogue lists `ma.case` alongside `ma.report` and `ma.decision`, and this
/// is the record that fills it. It exists so report poisoning is *measurable*: a forged
/// report that the two-authority resolution threw out at ingestion, and an honest subject
/// whose case stalled because only one authority could resolve its pseudonym, are
/// different outcomes and neither is visible on `ma.decision`, which only carries the
/// decisions that were actually taken.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaCaseRecord {
    /// The instant the case reached this outcome.
    pub t: SimTime,
    /// The case's id, unique within the run.
    pub case_id: String,
    /// The subject, by pseudonym digest.
    pub subject: String,
    /// The outcome: `ingested`, `dismissed-no-evidence`, `dismissed-below-threshold`,
    /// `unresolved-single-authority`, `resolved`, or `revoked`.
    pub outcome: String,
    /// How many distinct reporter certificates the case holds.
    #[serde(default)]
    pub reporters: u32,
    /// How many of those the authority trusts.
    #[serde(default)]
    pub trusted_reporters: u32,
    /// How many authorities could resolve the subject's pseudonym.
    #[serde(default)]
    pub authorities_resolved: u32,
    /// How many the protocol requires before an identity is resolved.
    #[serde(default)]
    pub authorities_required: u32,
}

impl Record for MaCaseRecord {
    const CHANNEL: &'static str = "ma.case";
    const VISIBILITY: Visibility = Visibility::Node;
}

/// `privacy.link` — one linkage the passive observer claimed across a pseudonym change
/// (NODE).
///
/// # Why this is a NODE record
///
/// Every field is something the observer computed from its own receptions: two pseudonym
/// digests, its own posterior over the candidates it could see, and the entropy of that
/// posterior. **Whether the link is correct is not here**, because the observer cannot
/// know — resolving a digest to a vehicle is exactly what the pseudonym scheme prevents.
/// The run declares the digest-to-actor map on the ground-truth side, as
/// `v2xw_metrics::detection::DetectionProvider::declare_subject` already does for reports,
/// and `linkability_rate` is then correct links over
/// pseudonym changes, the denominator coming from the `sec.cert` `change` events.
///
/// An observer that *declined* to link emits a record too, with an empty
/// [`Self::predecessor`] and `method = "unlinked"`, so the denominator of a linkage rate
/// is not silently the numerator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrivacyLinkClaim {
    /// The instant the observer made the claim, on its own clock.
    pub t: SimTime,
    /// The observing node.
    pub observer: NodeId,
    /// The predecessor pseudonym digest the observer linked from, hex. Empty when it
    /// declined to link.
    pub predecessor: String,
    /// The new pseudonym digest, hex.
    pub successor: String,
    /// The observer's posterior probability for this candidate, in `[0, 1]`.
    pub posterior: f64,
    /// How many candidate predecessors were inside the observer's gate.
    pub candidates: u32,
    /// |Ψ|: the anonymity set at the change — the candidates plus the target
    /// (ETSI TR 103 415 §5.1.2).
    pub anonymity_set_size: u32,
    /// S = −Σ p·log₂ p over the observer's posterior: the effective anonymity-set size in
    /// bits.
    pub effective_anonymity_set_bits: f64,
    /// d = S / log₂|Ψ|, the degree of anonymity; `0` when |Ψ| ≤ 1, where the quantity is
    /// undefined and the target has none.
    pub degree_of_anonymity: f64,
    /// How the link was made: the observer's tracker id, or `unlinked`.
    pub method: String,
}

impl Record for PrivacyLinkClaim {
    const CHANNEL: &'static str = "privacy.link";
    const VISIBILITY: Visibility = Visibility::Node;
}

/// `privacy.track` — one chain of pseudonyms the observer followed, and for how long
/// (NODE).
///
/// `tracking_duration` (the Wiedersheim et al. 2010
/// method) is the mean of [`Self::duration_s`] over the chains the ground-truth join
/// confirms were one vehicle. As with [`PrivacyLinkClaim`], the confirmation is not in the
/// record: the chain is what the observer *believed* it was following.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrivacyTrackSegment {
    /// The instant the chain ended — the observer gave it up or the run did.
    pub t: SimTime,
    /// The observing node.
    pub observer: NodeId,
    /// The first pseudonym digest of the chain, hex.
    pub origin: String,
    /// The last pseudonym digest of the chain, hex.
    pub last: String,
    /// How many pseudonym changes the observer linked across. `0` is a chain it never
    /// extended, which is the scheme working.
    pub links: u32,
    /// How long the chain lasted, seconds: last reception minus first reception.
    pub duration_s: f64,
    /// How many receptions the chain is built from.
    pub fixes: u32,
    /// Why it ended: `silence` (nothing heard for longer than the observer's gate) or
    /// `run-end`.
    pub closed_reason: String,
}

impl Record for PrivacyTrackSegment {
    const CHANNEL: &'static str = "privacy.track";
    const VISIBILITY: Visibility = Visibility::Node;
}

#[cfg(test)]
mod new_channel_tests {
    use super::*;

    #[test]
    fn the_new_channels_are_node_visible_and_never_ground_truth() {
        assert_eq!(MaCaseRecord::CHANNEL, "ma.case");
        assert_eq!(PrivacyLinkClaim::CHANNEL, "privacy.link");
        assert_eq!(PrivacyTrackSegment::CHANNEL, "privacy.track");
        for v in [
            MaCaseRecord::VISIBILITY,
            PrivacyLinkClaim::VISIBILITY,
            PrivacyTrackSegment::VISIBILITY,
        ] {
            assert!(!v.is_gt_tainted());
            assert!(v.allowed_on_node_channel());
        }
    }

    /// A linkage claim names two digests and a posterior, and nothing that resolves either
    /// digest to a vehicle: the correctness join is the run's, not the observer's.
    #[test]
    fn a_link_claim_carries_digests_and_a_posterior_and_no_actor() {
        let r = PrivacyLinkClaim {
            t: 5_000_000_000,
            observer: NodeId::new(9),
            predecessor: "aabb".to_string(),
            successor: "ccdd".to_string(),
            posterior: q(0.812_345),
            candidates: 3,
            anonymity_set_size: 4,
            effective_anonymity_set_bits: q(1.234_5),
            degree_of_anonymity: q(0.617_25),
            method: "kinematic-mht".to_string(),
        };
        let json = serde_json::to_value(&r).unwrap();
        assert!(json.get("actor").is_none());
        assert_eq!(r.posterior, 0.812);
        assert!(v2xw_core::math::is_on_grid(r.posterior, SCORE_Q));
        assert!(v2xw_core::math::is_on_grid(
            r.effective_anonymity_set_bits,
            SCORE_Q
        ));
        for k in [
            "t",
            "observer",
            "predecessor",
            "successor",
            "posterior",
            "candidates",
            "anonymity_set_size",
            "effective_anonymity_set_bits",
            "degree_of_anonymity",
            "method",
        ] {
            assert!(json.get(k).is_some(), "missing {k}");
        }
    }

    #[test]
    fn a_case_record_distinguishes_a_dismissal_from_a_stalled_resolution() {
        let dismissed = MaCaseRecord {
            t: 1,
            case_id: "case_00001".to_string(),
            subject: "beef".to_string(),
            outcome: "dismissed-no-evidence".to_string(),
            reporters: 1,
            trusted_reporters: 0,
            authorities_resolved: 0,
            authorities_required: 2,
        };
        let stalled = MaCaseRecord {
            outcome: "unresolved-single-authority".to_string(),
            authorities_resolved: 1,
            ..dismissed.clone()
        };
        assert_ne!(dismissed.outcome, stalled.outcome);
        let json = serde_json::to_value(&stalled).unwrap();
        for k in [
            "t",
            "case_id",
            "subject",
            "outcome",
            "reporters",
            "trusted_reporters",
            "authorities_resolved",
            "authorities_required",
        ] {
            assert!(json.get(k).is_some(), "missing {k}");
        }
    }
}
