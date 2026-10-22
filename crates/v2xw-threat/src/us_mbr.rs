//! The strict US misbehaviour profile: the PSID-0x20 (BSM) report subtypes of the SCMS
//! Manager's *Misbehavior Report and Application Specification for Connected Vehicle Pilot
//! Deployment* v1.0 ([SM-MBR] §2.3) and its *ASN.1 for Misbehavior Reporting* v1.1
//! ([SM-MBR-ASN]), with their default thresholds, and nothing else.
//!
//! | Type / subtype (ASN.1 number) | Rule ([SM-MBR] §2.3) | Default | Check here |
//! |---|---|---|---|
//! | 1VInconsistent / ConstantPosition (1) | velocity non-zero but the position has not changed | on | [`DetectorId::ConstantPositionFrozen`] |
//! | 1VInconsistent / RandomPosition (2) | two consecutive BSMs: `|p₁ + v₁·Δt − p₂| > gpsDrift` | on, 1 m | [`DetectorId::PositionJump`] |
//! | Implausible / MaxSpeed (1) | speed > threshold | on, 90 m/s | [`DetectorId::ImplausibleSpeed`] |
//! | Implausible / MaxAcceleration (2) | acceleration > threshold | on, 10 m/s² | [`DetectorId::ImplausibleAcceleration`] |
//! | Implausible / BrakeAcceleration (3) | brakes on, acceleration not negative | on | not modelled (below) |
//! | ObsImplausible / Distance (1) | distance to the observer > threshold + gpsDrift | **off**, 3000 m + 10 m | [`DetectorId::AcceptanceRangeThreshold`] |
//!
//! What the profile does that the research suite ([`crate::detect::Legacy12`]) does not:
//! it checks every received BSM against the one before it from the same certificate
//! ("two consecutive BSMs"), with no streak gate, no stated-accuracy tolerance and no
//! map; and it checks nothing else (no heading, staleness, Sybil, map or frequency check).
//! A check fires the moment its rule holds, as a deployed OBU would generate the report.
//!
//! It judges only BSMs this receiver **verified**. The evidence of every PSID-0x20 report
//! is "a sequence of BSMs signed with the same certificate" ([SM-MBR] §2.3.1.2.1 and §2.3.1.3.1;
//! [SM-MBR-ASN] `SignedBsmsWithCertificate`), so a BSM whose signature was not checked (a
//! verify-on-demand policy deferred it) or failed is not evidence about that certificate:
//! anyone can put a certificate on a frame with a forged signature and invented
//! kinematics. Such a BSM is neither checked nor kept as the "previous" BSM of its
//! certificate. Under `on-demand` verification that leaves the profile nearly blind, which
//! the loader says in a warning (`scenario::detection_policy_warnings`).
//!
//! Deviations, each named in `docs/design/scms-real/testing/realism.md`:
//! * **MaxAcceleration** reads the acceleration implied by two consecutive speeds,
//!   `|v₂ − v₁| / Δt`: the receiver's observation does not carry the BSM's `accelSet`.
//! * **BrakeAcceleration** never fires: the observation does not carry `brakes`. No
//!   attack of the catalogue falsifies the brake status, so nothing is missed today.
//! * **Consecutive** means consecutively *received* from one certificate, at most
//!   `max_gap_s` apart (ASSUMPTION, 0.5 s: five BSM intervals). Beyond that the first
//!   BSM's velocity no longer predicts the second's position to within 1 m.

use std::collections::BTreeMap;

use crate::cards::{design, standard};
use crate::ctx::{ThreatCtx, ThreatCtxExt};
use crate::detect::{Detector, DetectorCost, DetectorId, Fingerprint, Observation, Verdict};
use crate::obs::{LocalEnvironment, ObservedKind, ObservedMessage, SelfBelief, StationType};
use crate::records::DetObservation;
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Tier, Validation, ValidationStatus,
};
use v2xw_core::math;
use v2xw_core::model::Model;
use v2xw_core::time::{SimTime, ns_to_secs};

/// The model id.
pub const MODEL_ID: &str = "threat/detector/us-sm-mbr";

/// The score a fired check reports (any value ≥ 1 fires; the profile has no gradation).
pub const FIRED_SCORE: f64 = 1.5;

/// The profile's switches and thresholds, [SM-MBR] §2.3 defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct UsMbrParams {
    /// `mbr-Psid20-1VInconsistent-ConstantPosition` (on).
    pub constant_position: bool,
    /// `mbr-Psid20-1VInconsistent-RandomPosition` (on).
    pub random_position: bool,
    /// `…-RandomPosition-gpsDrift`, metres (1).
    pub gps_drift_m: f64,
    /// `mbr-Psid20-Implausible-MaxSpeed` (on).
    pub max_speed: bool,
    /// `…-MaxSpeed-Threshold`, m/s (90, "approx. 200 mph").
    pub max_speed_mps: f64,
    /// `mbr-Psid20-Implausible-MaxAcceleration` (on).
    pub max_acceleration: bool,
    /// `…-MaxAcceleration-Threshold`, m/s² (10).
    pub max_accel_mps2: f64,
    /// `mbr-Psid20-ObsImplausible-Distance` (**off**: a relay makes an innocent sender
    /// look far away).
    pub obs_distance: bool,
    /// `…-Distance-Threshold`, metres (3000).
    pub obs_distance_m: f64,
    /// `…-Distance-gpsDrift`, metres (10).
    pub obs_gps_drift_m: f64,
    /// The longest gap between two BSMs that still counts as consecutive, seconds
    /// (ASSUMPTION, 0.5).
    pub max_gap_s: f64,
    /// The smallest displacement that counts as a changed position, metres: the J2735
    /// position LSB, 1e-7 degree of latitude, is 1.1 cm (0.01).
    pub position_resolution_m: f64,
    /// The smallest non-zero speed, m/s: the J2735 `Speed` LSB (0.02).
    pub speed_resolution_mps: f64,
}

impl Default for UsMbrParams {
    fn default() -> Self {
        Self {
            constant_position: true,
            random_position: true,
            gps_drift_m: 1.0,
            max_speed: true,
            max_speed_mps: 90.0,
            max_acceleration: true,
            max_accel_mps2: 10.0,
            obs_distance: false,
            obs_distance_m: 3000.0,
            obs_gps_drift_m: 10.0,
            max_gap_s: 0.5,
            position_resolution_m: 0.01,
            speed_resolution_mps: 0.02,
        }
    }
}

/// The parameter names [`UsMbrParams::apply`] reads.
pub const PARAM_KEYS: [&str; 13] = [
    "constant_position",
    "random_position",
    "gps_drift_m",
    "max_speed",
    "max_speed_mps",
    "max_acceleration",
    "max_accel_mps2",
    "obs_distance",
    "obs_distance_m",
    "obs_gps_drift_m",
    "max_gap_s",
    "position_resolution_m",
    "speed_resolution_mps",
];

impl UsMbrParams {
    /// Sets one parameter by name (a switch is on when `v` is non-zero); false for a name
    /// the profile does not read.
    pub fn apply(&mut self, key: &str, v: f64) -> bool {
        let on = v != 0.0;
        match key {
            "constant_position" => self.constant_position = on,
            "random_position" => self.random_position = on,
            "gps_drift_m" => self.gps_drift_m = v,
            "max_speed" => self.max_speed = on,
            "max_speed_mps" => self.max_speed_mps = v,
            "max_acceleration" => self.max_acceleration = on,
            "max_accel_mps2" => self.max_accel_mps2 = v,
            "obs_distance" => self.obs_distance = on,
            "obs_distance_m" => self.obs_distance_m = v,
            "obs_gps_drift_m" => self.obs_gps_drift_m = v,
            "max_gap_s" => self.max_gap_s = v,
            "position_resolution_m" => self.position_resolution_m = v,
            "speed_resolution_mps" => self.speed_resolution_mps = v,
            _ => return false,
        }
        true
    }
}

/// The previous BSM heard from one certificate.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Last {
    x: f64,
    y: f64,
    speed: f64,
    heading: f64,
    t: SimTime,
}

/// The strict US profile, one per receiver.
#[derive(Debug, Clone)]
pub struct UsSmMbr {
    card: ModelCard,
    params: UsMbrParams,
    last: BTreeMap<[u8; 8], Last>,
}

impl UsSmMbr {
    /// The profile with the given switches and thresholds.
    #[must_use]
    pub fn new(params: UsMbrParams) -> Self {
        Self {
            card: card(&params),
            params,
            last: BTreeMap::new(),
        }
    }

    /// The thresholds it reads.
    #[must_use]
    pub fn params(&self) -> &UsMbrParams {
        &self.params
    }

    /// The checks `m` fails against the previous BSM `prev` of its sender (if consecutive)
    /// and on its own.
    fn checks(&self, prev: Option<Last>, m: &ObservedMessage, me: &SelfBelief) -> Vec<DetectorId> {
        let p = &self.params;
        let mut out = Vec::new();
        if let Some(r) = prev {
            // "The time between" two BSMs is between their generation times.
            let dt = ns_to_secs(m.claimed_generation_time.saturating_sub(r.t));
            if dt > 0.0 && dt <= p.max_gap_s {
                let disp = math::hypot(m.claimed_x_m - r.x, m.claimed_y_m - r.y);
                if p.constant_position
                    && disp < p.position_resolution_m
                    && m.claimed_speed_mps >= p.speed_resolution_mps
                    && r.speed >= p.speed_resolution_mps
                {
                    out.push(DetectorId::ConstantPositionFrozen);
                }
                if p.random_position {
                    // ENU, heading 0 = east, counter-clockwise (crate::obs).
                    let px = r.x + r.speed * r.heading.cos() * dt;
                    let py = r.y + r.speed * r.heading.sin() * dt;
                    if math::hypot(m.claimed_x_m - px, m.claimed_y_m - py) > p.gps_drift_m {
                        out.push(DetectorId::PositionJump);
                    }
                }
                if p.max_acceleration
                    && (m.claimed_speed_mps - r.speed).abs() / dt > p.max_accel_mps2
                {
                    out.push(DetectorId::ImplausibleAcceleration);
                }
            }
        }
        if p.max_speed && m.claimed_speed_mps > p.max_speed_mps {
            out.push(DetectorId::ImplausibleSpeed);
        }
        if p.obs_distance
            && math::hypot(m.claimed_x_m - me.x_m, m.claimed_y_m - me.y_m)
                > p.obs_distance_m + p.obs_gps_drift_m
        {
            out.push(DetectorId::AcceptanceRangeThreshold);
        }
        out
    }
}

impl Model for UsSmMbr {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl Detector for UsSmMbr {
    fn on_message(
        &mut self,
        ctx: &mut dyn ThreatCtx,
        me: &SelfBelief,
        m: &ObservedMessage,
        _env: &dyn LocalEnvironment,
    ) -> Verdict {
        let subject = m.signer_hex();
        let mut fingerprint = Fingerprint::default();
        // [SM-MBR] defines reports for PSID 0x20 only: a vehicle's BSM. A PSM (a handset's
        // own declaration) and an event message are not checked.
        if !matches!(m.kind, ObservedKind::Beacon)
            || m.station_type != StationType::Vehicle
            || !m.verification.is_valid()
        {
            return Verdict {
                subject,
                fingerprint,
                fired: Vec::new(),
            };
        }
        let prev = self.last.get(&m.signer).copied();
        let fired_ids = self.checks(prev, m, me);
        self.last.insert(
            m.signer,
            Last {
                x: m.claimed_x_m,
                y: m.claimed_y_m,
                speed: m.claimed_speed_mps,
                heading: m.claimed_heading_rad,
                t: m.claimed_generation_time,
            },
        );
        let mut fired = Vec::with_capacity(fired_ids.len());
        for d in fired_ids {
            // The fingerprint keeps the research suite's frozen columns
            // (DetectorId::ALL); a US-only check has no column.
            if DetectorId::ALL.contains(&d) {
                fingerprint.set(d, FIRED_SCORE);
            }
            fired.push(Observation {
                detector: d,
                score: FIRED_SCORE,
                subject: subject.clone(),
                at: m.received_at,
            });
        }
        for o in &fired {
            ctx.emit(DetObservation::new(
                o.at,
                me.node,
                o.detector.as_str(),
                &o.subject,
                o.score,
            ));
        }
        Verdict {
            subject,
            fingerprint,
            fired,
        }
    }

    fn cost(&self) -> DetectorCost {
        DetectorCost {
            per_message_us: 5.0,
        }
    }
}

fn param(name: &str, unit: &str, default: serde_json::Value, clause: &str) -> Parameter {
    Parameter::new(
        name,
        unit,
        default,
        standard(&format!(
            "SCMS Manager, Misbehavior Report and Application Specification for CV Pilot \
             Deployment v1.0 (2019), {clause}"
        )),
    )
}

/// The model card for the profile.
#[must_use]
pub fn card(p: &UsMbrParams) -> ModelCard {
    use serde_json::json;
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Detector,
        "1.0.0",
        "The strict US misbehaviour profile: the SCMS Manager PSID-0x20 report subtypes \
         (ConstantPosition, RandomPosition, MaxSpeed, MaxAcceleration, the ObsImplausible \
         distance check off) with their published default thresholds, every received BSM \
         checked against the previous one from its certificate, no streak gate.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    card.equations = vec![
        Equation::new("ConstantPosition", "|p₂ − p₁| < 1 LSB and v₁, v₂ ≥ 1 LSB"),
        Equation::new(
            "RandomPosition",
            "|p₁ + v₁·(cos ψ₁, sin ψ₁)·Δt − p₂| > gpsDrift",
        ),
        Equation::new("MaxSpeed", "v > v_max"),
        Equation::new("MaxAcceleration", "|v₂ − v₁| / Δt > a_max"),
        Equation::new("ObsImplausible Distance", "|p − p_self| > d_max + gpsDrift"),
    ];
    card.parameters = vec![
        param("gps_drift_m", "m", json!(p.gps_drift_m), "§2.3.1.2.2"),
        param("max_speed_mps", "m/s", json!(p.max_speed_mps), "§2.3.1.3.2"),
        param(
            "max_accel_mps2",
            "m/s²",
            json!(p.max_accel_mps2),
            "§2.3.1.3.3",
        ),
        param("obs_distance", "-", json!(p.obs_distance), "§2.3.1.4.2"),
        param("obs_distance_m", "m", json!(p.obs_distance_m), "§2.3.1.4.2"),
        param(
            "obs_gps_drift_m",
            "m",
            json!(p.obs_gps_drift_m),
            "§2.3.1.4.2",
        ),
    ];
    card.sources = vec![
        standard(
            "SCMS Manager, Misbehavior Report and Application Specification for Connected \
             Vehicle Pilot Deployment v1.0 (2019-02-14) §2.3",
        ),
        standard("SCMS Manager, ASN.1 for Misbehavior Reporting v1.1 (2019-04-23)"),
        design("scms-real/03-protocols-and-operations.md §8.1"),
    ];
    card.assumptions = vec![
        "Consecutive means consecutively received from one certificate, at most \
         max_gap_s (0.5 s) apart."
            .to_string(),
    ];
    card.limitations = vec![
        "MaxAcceleration reads the acceleration two consecutive speeds imply: the \
         observation does not carry the BSM accelSet."
            .to_string(),
        "BrakeAcceleration is not checked: the observation does not carry the brake \
         status."
            .to_string(),
    ];
    card.determinism = Determinism {
        uses_rng: false,
        rng_domains: Vec::new(),
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: vec![standard("[SM-MBR] §2.3 default thresholds")],
        tests: vec!["us_mbr::tests".to_string()],
    };
    card.cost = Some(v2xw_core::card::CostClass {
        per_call_us: Some(5.0),
        notes: Some("Placeholder: four scalar checks. Not measured.".to_string()),
    });
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::CollectingCtx;
    use crate::obs::{NoMap, VerificationState};
    use v2xw_core::ids::NodeId;

    fn me() -> SelfBelief {
        SelfBelief {
            node: NodeId::new(0),
            believed_time: 0,
            x_m: 0.0,
            y_m: 0.0,
            radio_range_m: 500.0,
        }
    }

    fn bsm(t_ms: u64, x: f64, speed: f64) -> ObservedMessage {
        ObservedMessage {
            signer: [7; 8],
            kind: ObservedKind::Beacon,
            received_at: t_ms * 1_000_000,
            claimed_generation_time: t_ms * 1_000_000,
            claimed_x_m: x,
            claimed_y_m: 0.0,
            claimed_speed_mps: speed,
            claimed_heading_rad: 0.0,
            claimed_pos_confidence_m: 1.5,
            repetitions: 1,
            cert_valid_from: 0,
            cert_valid_to: SimTime::MAX,
            station_type: StationType::Vehicle,
            verification: VerificationState::Valid,
        }
    }

    fn run(msgs: &[ObservedMessage], p: UsMbrParams) -> Vec<Vec<&'static str>> {
        let mut d = UsSmMbr::new(p);
        let mut ctx = CollectingCtx::new(1);
        msgs.iter()
            .map(|m| {
                d.on_message(&mut ctx, &me(), m, &NoMap)
                    .fired
                    .iter()
                    .map(|o| o.detector.as_str())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn an_honest_constant_speed_track_raises_nothing() {
        // 20 m/s east at 10 Hz: each BSM 2 m on, exactly where the last one's velocity put it.
        let msgs: Vec<_> = (0..20)
            .map(|k| bsm(k * 100, 2.0 * k as f64, 20.0))
            .collect();
        assert!(run(&msgs, UsMbrParams::default()).iter().all(Vec::is_empty));
    }

    #[test]
    fn random_position_fires_just_past_the_one_metre_drift() {
        // 0.95 m off the prediction stays quiet, 1.05 m fires: the [SM-MBR] 1 m gpsDrift.
        let ok = run(
            &[bsm(0, 0.0, 10.0), bsm(100, 1.0 + 0.95, 10.0)],
            UsMbrParams::default(),
        );
        assert!(ok[1].is_empty(), "{ok:?}");
        let bad = run(
            &[bsm(0, 0.0, 10.0), bsm(100, 1.0 + 1.05, 10.0)],
            UsMbrParams::default(),
        );
        assert_eq!(bad[1], vec!["positionJump"]);
    }

    #[test]
    fn only_verified_bsms_are_judged_or_remembered() {
        // A frozen position with speed fires only when both BSMs verified: a deferred
        // (on-demand) or failed signature is not evidence about the certificate.
        for state in [
            VerificationState::Unverified,
            VerificationState::BadSignature,
            VerificationState::UnknownCertificate,
        ] {
            let mut second = bsm(100, 5.0, 3.0);
            second.verification = state;
            let v = run(&[bsm(0, 5.0, 3.0), second], UsMbrParams::default());
            assert!(v[1].is_empty(), "{state:?} judged: {v:?}");
            // Nor does an unverified BSM become the "previous" one: a verified BSM after
            // it is compared with the last verified one (200 ms before, 0.6 m on at 3 m/s:
            // where it should be), and raises nothing.
            let v = run(
                &[bsm(0, 5.0, 3.0), {
                    let mut f = bsm(100, 50.0, 3.0);
                    f.verification = state;
                    f
                }, bsm(200, 5.6, 3.0)],
                UsMbrParams::default(),
            );
            assert!(v.iter().all(Vec::is_empty), "{state:?}: {v:?}");
        }
    }

    #[test]
    fn a_frozen_position_with_speed_is_constant_position() {
        let v = run(
            &[bsm(0, 5.0, 3.0), bsm(100, 5.0, 3.0)],
            UsMbrParams::default(),
        );
        assert!(v[1].contains(&"constantPositionFrozen"), "{v:?}");
        // Stopped: the same position at zero speed is honest.
        let s = run(
            &[bsm(0, 5.0, 0.0), bsm(100, 5.0, 0.0)],
            UsMbrParams::default(),
        );
        assert!(s[1].is_empty(), "{s:?}");
    }

    #[test]
    fn max_speed_and_max_acceleration_use_the_published_thresholds() {
        let fast = run(&[bsm(0, 0.0, 90.5)], UsMbrParams::default());
        assert_eq!(fast[0], vec!["implausibleSpeed"]);
        let limit = run(&[bsm(0, 0.0, 89.9)], UsMbrParams::default());
        assert!(limit[0].is_empty());
        // 1.1 m/s in 0.1 s is 11 m/s² (> 10); 0.9 m/s is 9 m/s². Positions follow the
        // first BSM's velocity, so RandomPosition stays quiet.
        let hard = run(
            &[bsm(0, 0.0, 10.0), bsm(100, 1.0, 11.1)],
            UsMbrParams::default(),
        );
        assert_eq!(hard[1], vec!["implausibleAcceleration"]);
        let soft = run(
            &[bsm(0, 0.0, 10.0), bsm(100, 1.0, 10.9)],
            UsMbrParams::default(),
        );
        assert!(soft[1].is_empty(), "{soft:?}");
    }

    #[test]
    fn the_distance_check_is_off_by_default_and_a_gap_breaks_consecutiveness() {
        let far = run(&[bsm(0, 5000.0, 0.0)], UsMbrParams::default());
        assert!(far[0].is_empty());
        let on = UsMbrParams {
            obs_distance: true,
            ..UsMbrParams::default()
        };
        assert_eq!(
            run(&[bsm(0, 5000.0, 0.0)], on)[0],
            vec!["acceptanceRangeThreshold"]
        );
        // 0.6 s apart is not consecutive: a 30 m jump is not compared.
        let gap = run(
            &[bsm(0, 0.0, 10.0), bsm(600, 36.0, 10.0)],
            UsMbrParams::default(),
        );
        assert!(gap[1].is_empty(), "{gap:?}");
    }
}
