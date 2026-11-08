//! Health bands: whether a metric's value is healthy, a reason for caution or unhealthy for
//! a real deployment — with the threshold's source.
//!
//! A chart of airtime per node tells a reader the number; it does not tell them whether
//! 12 ms/s is fine. Each [`HealthBand`] says so, in three levels, published with the metric
//! definition ([`crate::MetricDef::health`]) so that every consumer — the Studio's cards and
//! charts, the run summary, an exported catalogue — judges against the same numbers.
//!
//! # Where the numbers come from
//!
//! Every band names its source and its [`Basis`]. Where a standard gives the number, the
//! number is the standard's (the 100 ms basic-safety latency of 3GPP TS 22.185, the 0.68 CBR
//! target of ETSI TS 102 687's adaptive DCC). Where the literature gives a convention rather
//! than a requirement (time-to-collision below 1.5 s is a conflict), the basis says
//! `literature`. Where nothing published sets the line, the basis says `engineering` and the
//! source says why the line is where it is. Several of these match the deterministic run
//! analyst's thresholds (`v2xw_copilot::analyst::Thresholds`) on purpose, so a card and a
//! finding never disagree about the same number.
//!
//! # What a band is not
//!
//! Not a plausibility range: [`crate::MetricDef::min_value`] is the boundary of the
//! *possible*. A band is a judgement about the *acceptable*, and a run outside it is a
//! result, not an error.
//!
//! # Which value is judged
//!
//! [`HealthBand::judged`] names the series a band applies to, as a suffix of the metric's
//! name: `""` for the headline, `".p99"` for a distribution's 99th percentile (the 100 ms
//! requirement is a maximum, so a mean under it proves little), `"[100m]"` for a breakdown
//! value (delivery is a requirement *at a distance*). A consumer that does not have that
//! series does not judge the metric rather than judging a different statistic.

use serde::{Deserialize, Serialize};

/// Which way is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Better {
    /// Smaller is better: a latency, a load, a queue.
    Lower,
    /// Larger is better: a delivery ratio, a time-to-collision.
    Higher,
}

/// How a band's thresholds were arrived at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Basis {
    /// A standard sets the number.
    Standard,
    /// The literature uses the number as a convention.
    Literature,
    /// No published source sets it; the reason is in [`HealthBand::source`].
    Engineering,
}

/// One of the three levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HealthLevel {
    /// Within the healthy band.
    Healthy,
    /// Past the caution threshold, within the unhealthy one.
    Caution,
    /// Past the unhealthy threshold.
    Unhealthy,
}

/// A metric's health band: two thresholds, three one-line explanations, a source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthBand {
    /// Which way is better.
    pub better: Better,
    /// The series judged, as a suffix of the metric's name (`""`, `".p99"`, `"[100m]"`).
    pub judged: String,
    /// The boundary between healthy and caution, in the metric's unit. A value *at* it is
    /// still healthy.
    pub caution: f64,
    /// The boundary between caution and unhealthy, in the metric's unit. A value *at* it
    /// is still caution.
    pub unhealthy: f64,
    /// What a healthy value means, in one line ("below 0.50: congestion control idle").
    pub healthy_note: String,
    /// What a caution value means, in one line.
    pub caution_note: String,
    /// What an unhealthy value means, in one line ("above the 0.68 ETSI congestion target;
    /// vehicles will throttle").
    pub unhealthy_note: String,
    /// Where the thresholds come from.
    pub source: String,
    /// Standard, literature convention or engineering choice.
    pub basis: Basis,
}

impl HealthBand {
    /// The level of a value. `None` for a non-finite value, which is not judged.
    #[must_use]
    pub fn classify(&self, v: f64) -> Option<HealthLevel> {
        if !v.is_finite() {
            return None;
        }
        Some(match self.better {
            Better::Lower if v > self.unhealthy => HealthLevel::Unhealthy,
            Better::Lower if v > self.caution => HealthLevel::Caution,
            Better::Higher if v < self.unhealthy => HealthLevel::Unhealthy,
            Better::Higher if v < self.caution => HealthLevel::Caution,
            _ => HealthLevel::Healthy,
        })
    }

    /// The one-line explanation of a level.
    #[must_use]
    pub fn note(&self, level: HealthLevel) -> &str {
        match level {
            HealthLevel::Healthy => &self.healthy_note,
            HealthLevel::Caution => &self.caution_note,
            HealthLevel::Unhealthy => &self.unhealthy_note,
        }
    }

    /// The problem with the band, if it has one: thresholds out of order or not finite, a
    /// note or the source empty.
    #[must_use]
    pub fn problem(&self) -> Option<String> {
        if !self.caution.is_finite() || !self.unhealthy.is_finite() {
            return Some("a health threshold is not finite".to_string());
        }
        let ordered = match self.better {
            Better::Lower => self.caution <= self.unhealthy,
            Better::Higher => self.caution >= self.unhealthy,
        };
        if !ordered {
            return Some(format!(
                "the caution threshold {} is past the unhealthy threshold {}",
                self.caution, self.unhealthy
            ));
        }
        if self.healthy_note.trim().is_empty()
            || self.caution_note.trim().is_empty()
            || self.unhealthy_note.trim().is_empty()
        {
            return Some("every health level needs its one-line explanation".to_string());
        }
        if self.source.trim().is_empty() {
            return Some("a health band must say where its thresholds come from".to_string());
        }
        if !(self.judged.is_empty()
            || self.judged.starts_with('.')
            || (self.judged.starts_with('[') && self.judged.ends_with(']')))
        {
            return Some(format!(
                "the judged series suffix {:?} is neither empty, `.pNN` nor `[value]`",
                self.judged
            ));
        }
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn band(
    better: Better,
    judged: &str,
    caution: f64,
    unhealthy: f64,
    notes: [&str; 3],
    basis: Basis,
    source: &str,
) -> HealthBand {
    HealthBand {
        better,
        judged: judged.to_string(),
        caution,
        unhealthy,
        healthy_note: notes[0].to_string(),
        caution_note: notes[1].to_string(),
        unhealthy_note: notes[2].to_string(),
        source: source.to_string(),
        basis,
    }
}

/// The 3GPP / SAE basic-safety latency budget, 100 ms (3GPP TS 22.185 §5.1 [R-5.1-002];
/// SAE J2945/1's 100 ms BSM interval).
pub const SAFETY_LATENCY_BUDGET_MS: f64 = 100.0;

/// The ETSI adaptive DCC's target channel busy ratio, 0.68 (ETSI TS 102 687 V1.2.1 §5.4
/// Table 3, `CBR_target`; `v2xw_radio::dcc::AdaptiveParams::TS102687`).
pub const CBR_TARGET_ETSI: f64 = 0.68;

/// SAE J2945/1's `vMinCBP`, 50 %: the channel busy percentage above which its congestion
/// control starts lowering power and rate (Ahmad et al., IEEE TVT 68(3) 2019, Table I).
pub const CBR_J2945_MIN: f64 = 0.50;

/// The health band of a metric, by name, or `None` when no band is published for it.
///
/// One table rather than one band per definition site so the whole set of judgements can
/// be read, reviewed and cited in one place; [`crate::MetricDef::new`] looks every metric up
/// here, so a band reaches the catalogue whichever provider defines the metric.
#[must_use]
pub fn band_for(name: &str) -> Option<HealthBand> {
    use Basis::{Engineering, Literature, Standard};
    use Better::{Higher, Lower};
    Some(match name {
        // ---- Channel -------------------------------------------------------------------
        "cbr" | "channel_load" => band(
            Lower,
            "",
            CBR_J2945_MIN,
            CBR_TARGET_ETSI,
            [
                "below 0.50: congestion control has nothing to do",
                "above 0.50: SAE J2945/1 congestion control is lowering power and message rate",
                "above the 0.68 ETSI congestion target; vehicles will throttle their messages",
            ],
            Standard,
            "ETSI TS 102 687 V1.2.1 §5.4 (adaptive DCC CBR_target 0.68); SAE J2945/1 vMinCBP \
             50 % (Ahmad et al., IEEE TVT 68(3) 2019, Table I)",
        ),
        "airtime_per_node" => band(
            Lower,
            "",
            10.0,
            30.0,
            [
                "within the 1 % duty cycle the relaxed DCC state allows",
                "above 10 ms/s: more than the relaxed DCC state's 1 % duty cycle",
                "above 30 ms/s: past the 3 % per-station duty-cycle ceiling",
            ],
            Standard,
            "ETSI TS 102 687 V1.2.1 reactive DCC Relaxed state (T_on 1 ms per T_off 100 ms, \
             1 %); EN 302 571 / TS 102 687 adaptive δ_max 0.03 (3 % = 30 ms/s)",
        ),
        "mac_queue_depth" => band(
            Lower,
            "",
            1.0,
            5.0,
            [
                "the transmit queue is usually empty",
                "the transmit queue is rarely empty: frames wait behind each other",
                "the transmit queue is backing up; frames will be stale or dropped",
            ],
            Engineering,
            "engineering choice: a mean depth above one frame is a queue that is rarely \
             empty (the run analyst's queue_depth_saturated); five frames is half a second \
             of 10 Hz messages",
        ),
        "mac_access_delay" => band(
            Lower,
            ".p95",
            10.0,
            20.0,
            [
                "p95 channel access within 10 ms",
                "p95 channel access above 10 ms: a tenth of the 100 ms safety budget",
                "p95 channel access above 20 ms: a fifth of the 100 ms safety budget is spent \
                 waiting for the channel",
            ],
            Engineering,
            "engineering choice against the 100 ms budget of 3GPP TS 22.185 §5.1: 20 % is \
             the run analyst's access_budget_fraction",
        ),
        // ---- Delivery ------------------------------------------------------------------
        "pdr" => band(
            Higher,
            "[100m]",
            0.95,
            0.90,
            [
                "at least 95 % of receivers within 100 m decode each message",
                "below 95 % within 100 m: losses at short range, where a link should be clean",
                "below 90 % within 100 m: under the reliability V2V evaluations require",
            ],
            Standard,
            "3GPP TR 37.885 (V2X evaluation methodology reads range at a 0.9 packet reception \
             ratio); 0.95 within near range is the run analyst's engineering choice",
        ),
        "nar" => band(
            Higher,
            "[100m]",
            0.95,
            0.90,
            [
                "vehicles know about at least 95 % of their neighbours within 100 m",
                "awareness of neighbours within 100 m below 95 %",
                "vehicles are unaware of more than 1 in 10 neighbours within 100 m",
            ],
            Engineering,
            "engineering choice matching the 0.9 reception level of 3GPP TR 37.885 (the run \
             analyst's awareness_floor)",
        ),
        "aoi" => band(
            Lower,
            "",
            300.0,
            600.0,
            [
                "what receivers know is under three message intervals old",
                "above 300 ms: receivers' picture is three or more 10 Hz intervals old",
                "above 600 ms: older than the longest gap J2945/1 congestion control allows",
            ],
            Standard,
            "SAE J2945/1 10 Hz BSM and vMaxITT 600 ms (Ahmad et al., IEEE TVT 68(3) 2019, \
             Table I); three intervals is the run analyst's stale threshold",
        ),
        "pir" => band(
            Lower,
            ".p95",
            0.3,
            0.6,
            [
                "p95 gap between messages from one sender under 0.3 s",
                "p95 gap above 0.3 s: three or more 10 Hz messages lost in a row",
                "p95 gap above 0.6 s: longer than J2945/1 ever schedules; messages are lost",
            ],
            Standard,
            "SAE J2945/1 vMaxITT 600 ms (Ahmad et al., IEEE TVT 68(3) 2019, Table I); \
             Martelli et al., IEEE INFOCOM 2012 (packet inter-reception time)",
        ),
        // ---- Latency -------------------------------------------------------------------
        "e2e_latency" => band(
            Lower,
            ".p99",
            50.0,
            SAFETY_LATENCY_BUDGET_MS,
            [
                "p99 latency within half the 100 ms safety budget",
                "p99 latency within the 100 ms safety budget, but above half of it",
                "p99 latency above the 100 ms basic-safety requirement",
            ],
            Standard,
            "3GPP TS 22.185 §5.1 (maximum latency 100 ms) and 3GPP TR 22.886; SAE J2945/1 \
             (100 ms BSM interval). The 50 ms caution line is half the budget, an \
             engineering choice",
        ),
        // ---- Security ------------------------------------------------------------------
        "verify_wait" => band(
            Lower,
            ".p95",
            10.0,
            50.0,
            [
                "p95 verification wait within 10 ms",
                "p95 verification wait above 10 ms: a tenth of the 100 ms safety budget",
                "p95 verification wait above 50 ms: the verifier (HSM) cannot keep up",
            ],
            Engineering,
            "engineering choice against the 100 ms budget of 3GPP TS 22.185 §5.1: 10 % \
             queueing is tolerable, half the budget leaves too little for access and air",
        ),
        "verify_queue_depth" => band(
            Lower,
            "",
            1.0,
            5.0,
            [
                "the verification queue is usually empty",
                "messages queue for verification: the verifier is busy most of the time",
                "the verification queue is backing up: the verifier (HSM) is saturated",
            ],
            Engineering,
            "engineering choice: a mean depth above one is a server rarely idle (the run \
             analyst's queue_depth_saturated)",
        ),
        "unverified_ratio" => band(
            Lower,
            "",
            0.01,
            0.05,
            [
                "almost every delivered message was verified",
                "above 1 % of delivered messages were not verified",
                "above 5 % of delivered messages were not verified: verification is a bottleneck",
            ],
            Engineering,
            "engineering choice; 5 % is the run analyst's unverified_ratio threshold",
        ),
        "cert_pool_valid" => band(
            Higher,
            "",
            5.0,
            1.0,
            [
                "vehicles hold enough valid pseudonym certificates to keep changing them",
                "vehicles are down to a handful of valid pseudonym certificates",
                "vehicles have run out of valid certificates and cannot sign trusted messages",
            ],
            Engineering,
            "engineering choice: the CAMP SCMS design issues 20 certificates valid per week \
             (Whyte et al., IEEE VNC 2013); fewer than 5 leaves little room for a missed \
             top-up, fewer than 1 cannot sign",
        ),
        "revocation_latency_stage" => band(
            Lower,
            ".p95",
            60.0,
            300.0,
            [
                "each revocation stage completes within a minute",
                "a revocation stage takes over a minute",
                "a revocation stage takes over five minutes: a misbehaving vehicle stays trusted",
            ],
            Engineering,
            "engineering choice: no standard sets a revocation deadline (IEEE 1609.2.1 and \
             ETSI TS 102 941 leave CRL timing to policy)",
        ),
        // ---- Misbehaviour and privacy --------------------------------------------------
        "det_recall" => band(
            Higher,
            "",
            0.9,
            0.5,
            [
                "at least 90 % of attackers are detected",
                "between half and 90 % of attackers are detected",
                "fewer than half the attackers are detected",
            ],
            Engineering,
            "engineering choice; no standard sets a detection rate",
        ),
        "det_fpr" => band(
            Lower,
            "",
            0.01,
            0.05,
            [
                "at most 1 % of honest vehicles are wrongly flagged",
                "above 1 % of honest vehicles are wrongly flagged",
                "above 5 % of honest vehicles are wrongly flagged: revocation would hit them",
            ],
            Engineering,
            "engineering choice; no standard sets a false-positive rate",
        ),
        "linkability" => band(
            Lower,
            "",
            0.2,
            0.5,
            [
                "an observer links few pseudonym changes",
                "an observer links more than 1 in 5 pseudonym changes",
                "an observer links most pseudonym changes: pseudonyms are not protecting vehicles",
            ],
            Engineering,
            "engineering choice; 0.5 is the run analyst's linkability threshold",
        ),
        "backend_link_up" => band(
            Higher,
            "",
            0.99,
            0.9,
            [
                "vehicles can reach the credential backend",
                "some vehicles cannot reach the credential backend",
                "more than 1 in 10 vehicles cannot reach the credential backend",
            ],
            Engineering,
            "engineering choice",
        ),
        // ---- Safety --------------------------------------------------------------------
        "ttc_min" => band(
            Higher,
            "",
            3.0,
            1.5,
            [
                "encounters keep at least 3 s to collision",
                "encounters come within 3 s of collision",
                "encounters come within 1.5 s of collision: traffic conflicts",
            ],
            Literature,
            "1.5 s: the conflict threshold of surrogate-safety studies (Hayward 1972; FHWA \
             SSAM default, Gettman et al. 2008); 3 s, twice it, is an engineering choice",
        ),
        "drac" => band(
            Lower,
            ".p95",
            2.0,
            3.35,
            [
                "p95 braking needed to avoid a crash is gentle",
                "p95 braking needed is firm",
                "p95 braking needed is above 3.35 m/s²: critical",
            ],
            Literature,
            "Archer 2005 (DRAC critical above 3.35 m/s²); 2 m/s² is an engineering choice",
        ),
        _ => return None,
    })
}

/// Every metric name the table publishes a band for, in table order.
pub const BANDED: [&str; 21] = [
    "cbr",
    "channel_load",
    "airtime_per_node",
    "mac_queue_depth",
    "mac_access_delay",
    "pdr",
    "nar",
    "aoi",
    "pir",
    "e2e_latency",
    "verify_wait",
    "verify_queue_depth",
    "unverified_ratio",
    "cert_pool_valid",
    "revocation_latency_stage",
    "det_recall",
    "det_fpr",
    "linkability",
    "backend_link_up",
    "ttc_min",
    "drac",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_band_in_the_table_is_well_formed() {
        for name in BANDED {
            let b = band_for(name).unwrap_or_else(|| panic!("{name} has no band"));
            assert_eq!(b.problem(), None, "{name}");
        }
        assert!(band_for("speed").is_none());
    }

    #[test]
    fn the_owner_example_reads_as_unhealthy_with_its_reason() {
        let b = band_for("cbr").unwrap();
        assert_eq!(b.classify(0.71), Some(HealthLevel::Unhealthy));
        assert!(b.note(HealthLevel::Unhealthy).contains("0.68"));
        assert_eq!(b.classify(0.68), Some(HealthLevel::Caution));
        assert_eq!(b.classify(0.50), Some(HealthLevel::Healthy));
        assert_eq!(b.classify(0.51), Some(HealthLevel::Caution));
        assert_eq!(b.classify(f64::NAN), None);
    }

    #[test]
    fn higher_is_better_bands_turn_the_other_way() {
        let b = band_for("pdr").unwrap();
        assert_eq!(b.judged, "[100m]");
        assert_eq!(b.classify(0.97), Some(HealthLevel::Healthy));
        assert_eq!(b.classify(0.92), Some(HealthLevel::Caution));
        assert_eq!(b.classify(0.89), Some(HealthLevel::Unhealthy));
    }

    #[test]
    fn a_band_out_of_order_is_refused() {
        let mut b = band_for("e2e_latency").unwrap();
        b.caution = 200.0;
        assert!(b.problem().is_some());
        let mut b = band_for("pdr").unwrap();
        b.unhealthy = 0.99;
        assert!(b.problem().is_some());
    }
}
