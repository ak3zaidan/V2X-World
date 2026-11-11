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
//! | Implausible / BrakeAcceleration (3) | brakes on, acceleration not negative | on | [`DetectorId::BrakeAcceleration`] |
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
//! The Implausible type judges "a single BSM" ([SM-MBR] §2.3.1.3.1: "the evidence for this
//! type is a single BSM"), from the fields the frame carried
//! ([`v2xw_core::bsm_fields::BsmExtras`]), so a sender
//! that forges the frame forges them:
//! * **MaxAcceleration** (`max_accel_reading = accel-set`, the default): the BSM's
//!   `|accelSet.long|` above the threshold ("the acceleration in a single BSM",
//!   §2.3.1.3.3). A BSM without `accelSet` falls back to the acceleration two
//!   consecutive speeds imply, `|v₂ − v₁| / Δt`, counted in
//!   [`UsSmMbr::accel_unavailable_fallbacks`]; `speed-difference` keeps that reading for
//!   every BSM.
//! * **BrakeAcceleration** (§2.3.1.3.4): the BSM's `brakes.wheelBrakes` has a wheel bit
//!   set and its acceleration "is not negative, i.e. … the acceleration vector makes an
//!   acute angle with the velocity vector": `accelSet.long > brake_accel_tolerance_mps2`
//!   (0 by default: an acute angle is a strictly positive component) and a velocity to
//!   make the angle with, `speed ≥ speed_resolution_mps` (one J2735 LSB). A vehicle held
//!   at a red light (brakes on, `accelSet.long` 0, speed 0) is therefore not reported.
//!   An `unavailable` brake status or a missing `accelSet` is not judged.
//!
//! Deviations:
//! * **Consecutive** means consecutive *on the air*: the second BSM's `msgCnt` is the
//!   first's plus one (J2735 `MsgCount`, mod 128), and the two are at most `max_gap_s`
//!   apart (ASSUMPTION, 0.5 s: five BSM intervals). Both SM-MBR texts judge "two
//!   consecutive BSMs" (v1.0) "sent 0.1 seconds apart" (ASN.1 v1.1), and `msgCnt` is
//!   the field a receiver tells them by. A receiver that lost the BSMs between two it
//!   heard does not hold a consecutive pair (otherwise every honest `positionJump` at full
//!   density compares BSMs with lost ones between).
//!   `consecutive = received` restores the earlier reading, consecutively *received*
//!   within `max_gap_s`. A BSM without a `msgCnt` (a unit test's) is judged by the gap.
//!
//! # RandomPosition: two normative readings, two clocks (Paper 1 item C)
//!
//! The two texts disagree, so both are
//! implemented and the default is today's:
//!
//! * `reading = v1_0` (default): [SM-MBR] p.6, velocity only, `p₁ + v₁·Δt`;
//! * `reading = asn1_v1_1`: [SM-MBR-ASN] p.8 `Psid20MbrParams1VInconsistentRandomPosition`,
//!   "two BSMs, sent 0.1 seconds apart" and "the speed, acceleration": `p₁ + (v₁·Δt +
//!   ½·a₁·Δt²)` along the first BSM's heading with `a₁` its `accelSet.long`, judged only
//!   on pairs whose Δt is within `spacing_tolerance_s` (ASSUMPTION, 0.05 s, half a BSM
//!   interval) of 0.1 s.
//!
//! Δt is read from the 1609.2 `generationTime` (`time_base = generation_time`, default) or
//! from the BSM's `secMark` (`time_base = secmark`), wrapped at the minute and placed in
//! the minute the generation times say. The gap limit `max_gap_s` is a parameter.
//! Two further receiver clocks a deployed receiver may use:
//!
//! | `time_base` | Δt is | consecutive gap judged on |
//! |---|---|---|
//! | `generation_time` (default) | the difference of the 1609.2 `generationTime`s | Δt |
//! | `secmark` | the difference of the J2735 `secMark`s, minute-wrapped | Δt |
//! | `receive_time` | the difference of the receiver's arrival times of the two BSMs | Δt |
//! | `nominal` | a fixed `nominal_dt_s` (0.1 s, the 10 Hz BSM interval) | the generation-time gap |

use std::collections::BTreeMap;

use crate::cards::{design, standard};
use crate::ctx::{ThreatCtx, ThreatCtxExt};
use crate::detect::{Detector, DetectorCost, DetectorId, Fingerprint, Observation, Verdict};
use crate::obs::{LocalEnvironment, ObservedKind, ObservedMessage, SelfBelief, StationType};
use crate::records::DetObservation;
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
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
    /// Which acceleration MaxAcceleration judges (the BSM's `accelSet`).
    pub max_accel_reading: AccelReading,
    /// `mbr-Psid20-Implausible-BrakeAcceleration` (on).
    pub brake_acceleration: bool,
    /// How far above zero `accelSet.long` must be to count as "not negative" under brakes,
    /// m/s² (0: [SM-MBR] §2.3.1.3.4's acute angle is a strictly positive component).
    pub brake_accel_tolerance_mps2: f64,
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
    /// Which normative text RandomPosition follows (v1.0, velocity only).
    pub reading: RandomPositionReading,
    /// Which clock the consecutive checks read Δt from (generationTime).
    pub time_base: TimeBase,
    /// Under the ASN.1 v1.1 reading, how far from 0.1 s a pair's Δt may be and still be
    /// "sent 0.1 seconds apart", seconds (ASSUMPTION, 0.05).
    pub spacing_tolerance_s: f64,
    /// What makes two BSMs of one certificate consecutive (`msgCnt` successors).
    pub consecutive: Consecutive,
    /// Under `time_base = nominal`, the fixed Δt every pair is judged with, seconds: the
    /// nominal 10 Hz BSM interval, ASN.1 v1.1 "sent 0.1 seconds apart" (0.1).
    pub nominal_dt_s: f64,
}

/// Which acceleration MaxAcceleration judges.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AccelReading {
    /// The BSM's own `accelSet.long` ([SM-MBR] §2.3.1.3.3, "the acceleration in a single
    /// BSM"); a BSM without one falls back to [`Self::SpeedDifference`].
    #[default]
    AccelSet,
    /// `|v₂ − v₁| / Δt` from two consecutive BSMs.
    SpeedDifference,
}

impl AccelReading {
    /// The name the scenario uses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AccelSet => "accel-set",
            Self::SpeedDifference => "speed-difference",
        }
    }
}

/// What makes two BSMs from one certificate a consecutive pair.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Consecutive {
    /// Consecutive on the air: `msgCnt` successors within `max_gap_s`.
    #[default]
    MsgCount,
    /// Consecutively received within `max_gap_s`, BSMs lost between or not.
    Received,
}

impl Consecutive {
    /// The name the scenario uses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MsgCount => "msg_count",
            Self::Received => "received",
        }
    }
}

/// Which normative text RandomPosition follows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RandomPositionReading {
    /// [SM-MBR] v1.0 p.6: `|p₁ + v₁·Δt − p₂| > gpsDrift` (today's behaviour).
    #[default]
    V1_0,
    /// [SM-MBR-ASN] v1.1 p.8: speed and acceleration, two BSMs 0.1 s apart.
    Asn1V1_1,
}

impl RandomPositionReading {
    /// The name the scenario uses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V1_0 => "v1_0",
            Self::Asn1V1_1 => "asn1_v1_1",
        }
    }
}

/// Which clock a consecutive check reads Δt from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TimeBase {
    /// The IEEE 1609.2 header `generationTime` (today's behaviour).
    #[default]
    GenerationTime,
    /// The BSM's own `secMark`, wrapped at the minute.
    SecMark,
    /// The receiver's arrival times of the two BSMs (a receive or log clock).
    ReceiveTime,
    /// A fixed nominal interval, `nominal_dt_s`, whatever the true spacing.
    Nominal,
}

impl TimeBase {
    /// The name the scenario uses.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GenerationTime => "generation_time",
            Self::SecMark => "secmark",
            Self::ReceiveTime => "receive_time",
            Self::Nominal => "nominal",
        }
    }
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
            max_accel_reading: AccelReading::AccelSet,
            brake_acceleration: true,
            brake_accel_tolerance_mps2: 0.0,
            obs_distance: false,
            obs_distance_m: 3000.0,
            obs_gps_drift_m: 10.0,
            max_gap_s: 0.5,
            position_resolution_m: 0.01,
            speed_resolution_mps: 0.02,
            reading: RandomPositionReading::V1_0,
            time_base: TimeBase::GenerationTime,
            spacing_tolerance_s: 0.05,
            consecutive: Consecutive::MsgCount,
            nominal_dt_s: 0.1,
        }
    }
}

/// The parameter names [`UsMbrParams::apply`] reads.
pub const PARAM_KEYS: [&str; 21] = [
    "constant_position",
    "random_position",
    "gps_drift_m",
    "max_speed",
    "max_speed_mps",
    "max_acceleration",
    "max_accel_mps2",
    "max_accel_reading",
    "brake_acceleration",
    "brake_accel_tolerance_mps2",
    "obs_distance",
    "obs_distance_m",
    "obs_gps_drift_m",
    "max_gap_s",
    "position_resolution_m",
    "speed_resolution_mps",
    "reading",
    "time_base",
    "spacing_tolerance_s",
    "consecutive",
    "nominal_dt_s",
];

/// The bounds a numeric parameter must lie in, where the profile states any: the brake
/// tolerance is 0 to 2 m/s² in 0.01 steps (the J2735 `Acceleration` LSB).
#[must_use]
pub fn range(key: &str) -> Option<(f64, f64)> {
    match key {
        "brake_accel_tolerance_mps2" => Some((0.0, 2.0)),
        _ => None,
    }
}

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
            "max_accel_reading" => {
                self.max_accel_reading = if on {
                    AccelReading::SpeedDifference
                } else {
                    AccelReading::AccelSet
                };
            }
            "brake_acceleration" => self.brake_acceleration = on,
            "brake_accel_tolerance_mps2" => self.brake_accel_tolerance_mps2 = v,
            "obs_distance" => self.obs_distance = on,
            "obs_distance_m" => self.obs_distance_m = v,
            "obs_gps_drift_m" => self.obs_gps_drift_m = v,
            "max_gap_s" => self.max_gap_s = v,
            "position_resolution_m" => self.position_resolution_m = v,
            "speed_resolution_mps" => self.speed_resolution_mps = v,
            // A choice by number: 0 is the default, anything else the alternative.
            "reading" => {
                self.reading = if on {
                    RandomPositionReading::Asn1V1_1
                } else {
                    RandomPositionReading::V1_0
                };
            }
            "time_base" => {
                self.time_base = if on {
                    TimeBase::SecMark
                } else {
                    TimeBase::GenerationTime
                };
            }
            "spacing_tolerance_s" => self.spacing_tolerance_s = v,
            "nominal_dt_s" => self.nominal_dt_s = v,
            "consecutive" => {
                self.consecutive = if on {
                    Consecutive::Received
                } else {
                    Consecutive::MsgCount
                };
            }
            _ => return false,
        }
        true
    }

    /// Sets a choice parameter by name (`reading`: `v1_0` | `asn1_v1_1`; `time_base`:
    /// `generation_time` | `secmark` | `receive_time` | `nominal`); false for any other key
    /// or value.
    pub fn apply_str(&mut self, key: &str, v: &str) -> bool {
        match (key, v) {
            ("reading", "v1_0") => self.reading = RandomPositionReading::V1_0,
            ("reading", "asn1_v1_1") => self.reading = RandomPositionReading::Asn1V1_1,
            ("time_base", "generation_time") => self.time_base = TimeBase::GenerationTime,
            ("time_base", "secmark") => self.time_base = TimeBase::SecMark,
            ("time_base", "receive_time") => self.time_base = TimeBase::ReceiveTime,
            ("time_base", "nominal") => self.time_base = TimeBase::Nominal,
            ("consecutive", "msg_count") => self.consecutive = Consecutive::MsgCount,
            ("consecutive", "received") => self.consecutive = Consecutive::Received,
            ("max_accel_reading", "accel-set") => self.max_accel_reading = AccelReading::AccelSet,
            ("max_accel_reading", "speed-difference") => {
                self.max_accel_reading = AccelReading::SpeedDifference;
            }
            _ => return false,
        }
        true
    }

    /// Δt between the previous BSM `r` and `m` on the configured clock, seconds.
    fn delta_t(&self, r: &Last, m: &ObservedMessage) -> f64 {
        let by_generation = ns_to_secs(m.claimed_generation_time.saturating_sub(r.t));
        match (self.time_base, r.sec_mark, m.bsm.sec_mark_ms) {
            (TimeBase::SecMark, Some(a), Some(b)) => {
                // secMark counts milliseconds within the minute: the difference is taken
                // in the minute the generation times put the pair in.
                let gen_ms =
                    (i128::from(m.claimed_generation_time) - i128::from(r.t)).div_euclid(1_000_000);
                let d = i128::from(b) - i128::from(a);
                let k = (gen_ms - d + 30_000).div_euclid(60_000);
                (d + k * 60_000) as f64 * 1e-3
            }
            (TimeBase::ReceiveTime, ..) => ns_to_secs(m.received_at.saturating_sub(r.heard)),
            (TimeBase::Nominal, ..) => self.nominal_dt_s,
            // No secMark on one of the two (not a BSM): the header clock.
            _ => by_generation,
        }
    }

    /// The gap that decides whether `r` and `m` are consecutive (within `max_gap_s`):
    /// Δt itself, except under a nominal Δt, where the generation times say.
    fn gap(&self, r: &Last, m: &ObservedMessage, dt: f64) -> f64 {
        match self.time_base {
            TimeBase::Nominal => ns_to_secs(m.claimed_generation_time.saturating_sub(r.t)),
            _ => dt,
        }
    }

    /// Whether `m` follows `r` on the air: its `msgCnt` is `r`'s plus one, mod 128.
    /// Always true under `consecutive = received`, and when either has no `msgCnt`.
    fn in_sequence(&self, r_cnt: Option<u8>, m: &ObservedMessage) -> bool {
        match (self.consecutive, r_cnt, m.bsm.msg_cnt) {
            (Consecutive::MsgCount, Some(a), Some(b)) => b == (a + 1) % 128,
            _ => true,
        }
    }
}

/// One fired check: its id, the pair it judged (the previous BSM's `msgCnt` and the pair's
/// Δt, s; `None` for a single-BSM check) and the quantity it compared with its threshold.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Hit {
    id: DetectorId,
    pair: Option<(Option<u8>, f64)>,
    residual: f64,
}

impl Hit {
    fn new(id: DetectorId, pair: Option<(Option<u8>, f64)>, residual: f64) -> Self {
        Self { id, pair, residual }
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
    /// When the receiver heard it, on the receiver's own timeline: what eviction reads.
    heard: SimTime,
    /// Its `secMark`, ms within the minute.
    sec_mark: Option<u16>,
    /// Its `accelSet.long`, m/s².
    accel: Option<f64>,
    /// Its `msgCnt`.
    msg_cnt: Option<u8>,
}

/// How long a receiver keeps the previous BSM of a sender it no longer hears: 30 s,
/// ten times the default `max_gap_s` (3 s), after which no consecutive check can use it
/// (an honest sender's claimed generation time tracks the instant it is heard).
/// **ASSUMPTION**, a device limit: a real OBU's per-sender table holds the senders around
/// it, not every pseudonym it has ever heard. Without it a receiver's table grew by every
/// certificate it heard for the rest of the run.
const STALE_SENDER: SimTime = 30_000_000_000;

/// How many inserts between two sweeps of stale senders.
const SWEEP_EVERY: u32 = 1_024;

/// The strict US profile, one per receiver.
#[derive(Debug, Clone)]
pub struct UsSmMbr {
    card: ModelCard,
    params: UsMbrParams,
    last: BTreeMap<[u8; 8], Last>,
    /// Inserts since the last sweep of `last`.
    since_sweep: u32,
    /// MaxAcceleration checks that read two consecutive speeds because the BSM carried no
    /// `accelSet` (under `max_accel_reading = accel-set`).
    accel_unavailable_fallbacks: u64,
}

impl UsSmMbr {
    /// The profile with the given switches and thresholds.
    #[must_use]
    pub fn new(params: UsMbrParams) -> Self {
        Self {
            card: card(&params),
            params,
            last: BTreeMap::new(),
            since_sweep: 0,
            accel_unavailable_fallbacks: 0,
        }
    }

    /// How many MaxAcceleration checks fell back to the speed difference of two
    /// consecutive BSMs because the BSM carried no `accelSet` (`accel-set` reading only).
    #[must_use]
    pub fn accel_unavailable_fallbacks(&self) -> u64 {
        self.accel_unavailable_fallbacks
    }

    /// The thresholds it reads.
    #[must_use]
    pub fn params(&self) -> &UsMbrParams {
        &self.params
    }

    /// The `generationTime` of the BSM the next check of `signer`'s message `generated`
    /// would compare against, when it is consecutive to it (within `max_gap_s`): the
    /// first of the two BSMs a 1VInconsistent report on that message is about, which its
    /// evidence carries ([SM-MBR] §2.3.1.2.1, "a sequence of BSMs signed with the same
    /// certificate"). Read **before** [`Detector::on_message`] judges the message.
    #[must_use]
    pub fn consecutive_before(&self, m: &ObservedMessage) -> Option<SimTime> {
        let r = self.last.get(&m.signer)?;
        let dt = ns_to_secs(m.claimed_generation_time.saturating_sub(r.t));
        (dt > 0.0 && dt <= self.params.max_gap_s && self.params.in_sequence(r.msg_cnt, m))
            .then_some(r.t)
    }

    /// The checks `m` fails against the previous BSM `prev` of its sender (if consecutive)
    /// and on its own.
    fn checks(&mut self, prev: Option<Last>, m: &ObservedMessage, me: &SelfBelief) -> Vec<Hit> {
        let p = &self.params;
        let mut out = Vec::new();
        // MaxAcceleration on the BSM's own accelSet, when that is the reading and the BSM
        // carries one; otherwise on two consecutive speeds below.
        let accel_set = match (p.max_accel_reading, m.bsm.accel_long_mps2) {
            (AccelReading::AccelSet, Some(a)) => Some(a),
            _ => None,
        };
        let mut fell_back = false;
        if let Some(r) = prev {
            // "The time between" two BSMs, on the configured clock (generationTime by
            // default, or secMark).
            let dt = p.delta_t(&r, m);
            let gap = p.gap(&r, m, dt);
            if dt > 0.0 && gap > 0.0 && gap <= p.max_gap_s && p.in_sequence(r.msg_cnt, m) {
                let pair = Some((r.msg_cnt, dt));
                let disp = math::hypot(m.claimed_x_m - r.x, m.claimed_y_m - r.y);
                if p.constant_position
                    && disp < p.position_resolution_m
                    && m.claimed_speed_mps >= p.speed_resolution_mps
                    && r.speed >= p.speed_resolution_mps
                {
                    out.push(Hit::new(DetectorId::ConstantPositionFrozen, pair, disp));
                }
                // v1.0 judges every consecutive pair; the ASN.1 v1.1 only two BSMs "sent
                // 0.1 seconds apart".
                let spaced = match p.reading {
                    RandomPositionReading::V1_0 => true,
                    RandomPositionReading::Asn1V1_1 => (dt - 0.1).abs() <= p.spacing_tolerance_s,
                };
                if p.random_position && spaced {
                    let along = match p.reading {
                        RandomPositionReading::V1_0 => r.speed * dt,
                        // A BSM without accelSet predicts with velocity alone.
                        RandomPositionReading::Asn1V1_1 => {
                            r.speed * dt + 0.5 * r.accel.unwrap_or(0.0) * dt * dt
                        }
                    };
                    // ENU, heading 0 = east, counter-clockwise (crate::obs).
                    let px = r.x + along * v2xw_core::math::cos(r.heading);
                    let py = r.y + along * v2xw_core::math::sin(r.heading);
                    let off = math::hypot(m.claimed_x_m - px, m.claimed_y_m - py);
                    if off > p.gps_drift_m {
                        out.push(Hit::new(DetectorId::PositionJump, pair, off));
                    }
                }
                if p.max_acceleration && accel_set.is_none() {
                    fell_back = p.max_accel_reading == AccelReading::AccelSet;
                    let implied = (m.claimed_speed_mps - r.speed).abs() / dt;
                    if implied > p.max_accel_mps2 {
                        out.push(Hit::new(DetectorId::ImplausibleAcceleration, pair, implied));
                    }
                }
            }
        }
        if p.max_acceleration
            && let Some(a) = accel_set.filter(|a| a.abs() > p.max_accel_mps2)
        {
            out.push(Hit::new(DetectorId::ImplausibleAcceleration, None, a.abs()));
        }
        if p.max_speed && m.claimed_speed_mps > p.max_speed_mps {
            out.push(Hit::new(
                DetectorId::ImplausibleSpeed,
                None,
                m.claimed_speed_mps,
            ));
        }
        // §2.3.1.3.4: braking, and the acceleration makes an acute angle with a velocity.
        if p.brake_acceleration
            && m.bsm.brakes_applied == Some(true)
            && m.bsm
                .accel_long_mps2
                .is_some_and(|a| a > p.brake_accel_tolerance_mps2)
            && m.claimed_speed_mps >= p.speed_resolution_mps
        {
            out.push(Hit::new(
                DetectorId::BrakeAcceleration,
                None,
                m.bsm.accel_long_mps2.unwrap_or(0.0),
            ));
        }
        let range = math::hypot(m.claimed_x_m - me.x_m, m.claimed_y_m - me.y_m);
        if p.obs_distance && range > p.obs_distance_m + p.obs_gps_drift_m {
            out.push(Hit::new(DetectorId::AcceptanceRangeThreshold, None, range));
        }
        if fell_back {
            self.accel_unavailable_fallbacks += 1;
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
                heard: m.received_at,
                sec_mark: m.bsm.sec_mark_ms,
                accel: m.bsm.accel_long_mps2,
                msg_cnt: m.bsm.msg_cnt,
            },
        );
        self.since_sweep += 1;
        if self.since_sweep >= SWEEP_EVERY {
            self.since_sweep = 0;
            let horizon = m.received_at.saturating_sub(STALE_SENDER.max(
                // Never shorter than the configured consecutive gap.
                (self.params.max_gap_s.max(0.0) * 1e9) as SimTime,
            ));
            self.last.retain(|_, l| l.heard >= horizon);
        }
        let mut fired = Vec::with_capacity(fired_ids.len());
        for hit in &fired_ids {
            let d = hit.id;
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
        // One row per fired check, with the pair it judged and its residual, so a
        // per-verdict join (tools/realdata/enginebridge.py, K2) needs no re-derivation.
        for (o, hit) in fired.iter().zip(&fired_ids) {
            ctx.emit(
                DetObservation::new(o.at, me.node, o.detector.as_str(), &o.subject, o.score)
                    .with_pair(
                        m.bsm.msg_cnt,
                        hit.pair.and_then(|(c, _)| c),
                        hit.pair.map(|(_, dt)| dt),
                        Some(hit.residual),
                    ),
            );
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
        "1.1.0",
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
        Equation::new(
            "RandomPosition (ASN.1 v1.1)",
            "|p₁ + (v₁·Δt + ½·a₁·Δt²)·(cos ψ₁, sin ψ₁) − p₂| > gpsDrift, |Δt − 0.1 s| ≤ tol",
        ),
        Equation::new("MaxSpeed", "v > v_max"),
        Equation::new(
            "MaxAcceleration",
            "|accelSet.long| > a_max (accel-set; no accelSet, or speed-difference: |v₂ − v₁| / Δt > a_max)",
        ),
        Equation::new(
            "BrakeAcceleration",
            "wheelBrakes ≠ 0 ∧ accelSet.long > tol ∧ v ≥ 1 LSB",
        ),
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
        param(
            "max_accel_reading",
            "-",
            json!(p.max_accel_reading.as_str()),
            "§2.3.1.3.1 (\"the evidence for this type is a single BSM\") and §2.3.1.3.3 \
             (\"the acceleration in a single BSM\"): accel-set, the BSM's accelSet.long; \
             speed-difference, |v₂ − v₁| / Δt of two consecutive BSMs",
        ),
        param(
            "brake_acceleration",
            "-",
            json!(p.brake_acceleration),
            "§2.3.1.3.4 mbr-Psid20-Implausible-BrakeAcceleration",
        ),
        param(
            "brake_accel_tolerance_mps2",
            "m/s²",
            json!(p.brake_accel_tolerance_mps2),
            "§2.3.1.3.4 (\"the acceleration vector makes an acute angle with the velocity \
             vector\": a strictly positive component, 0; one J2735 LSB is 0.01)",
        ),
        param("obs_distance", "-", json!(p.obs_distance), "§2.3.1.4.2"),
        param("obs_distance_m", "m", json!(p.obs_distance_m), "§2.3.1.4.2"),
        param(
            "obs_gps_drift_m",
            "m",
            json!(p.obs_gps_drift_m),
            "§2.3.1.4.2",
        ),
        Parameter::new(
            "max_gap_s",
            "s",
            json!(p.max_gap_s),
            design("the gap limit between two consecutive BSMs (in neither SM-MBR text)"),
        ),
        Parameter::new(
            "reading",
            "-",
            json!(p.reading.as_str()),
            standard(
                "SM-MBR v1.0 p.6 §2.3.1.2.2 (v1_0) or ASN.1 for Misbehavior Reporting v1.1 \
                 p.8 Psid20MbrParams1VInconsistentRandomPosition (asn1_v1_1)",
            ),
        ),
        Parameter::new(
            "time_base",
            "-",
            json!(p.time_base.as_str()),
            Source::new(
                SourceKind::Code,
                "USDOT ITS JPO CV Pilot BSMs: the receiver's Δt \
                 clock, IEEE 1609.2 generationTime, J2735 secMark, the receiver's arrival \
                 time or a fixed nominal interval",
            ),
        ),
        Parameter::new(
            "consecutive",
            "-",
            json!(p.consecutive.as_str()),
            standard(
                "SM-MBR v1.0 §2.3.1.2 \"two consecutive BSMs\", ASN.1 v1.1 \"sent 0.1 \
                 seconds apart\"; J2735 MsgCount (msg_count) or consecutively received \
                 (received)",
            ),
        ),
        param(
            "nominal_dt_s",
            "s",
            json!(p.nominal_dt_s),
            "ASN.1 v1.1 \"sent 0.1 seconds apart\": the nominal Δt under time_base = nominal",
        ),
        Parameter::new(
            "spacing_tolerance_s",
            "s",
            json!(p.spacing_tolerance_s),
            Source::new(
                SourceKind::Code,
                "ASSUMPTION: half a 100 ms BSM interval around ASN.1 v1.1's 0.1 s \
                 ([SM-MBR] v1.0)",
            ),
        ),
    ];
    card.sources = vec![
        standard(
            "SCMS Manager, Misbehavior Report and Application Specification for Connected \
             Vehicle Pilot Deployment v1.0 (2019-02-14) §2.3",
        ),
        standard("SCMS Manager, ASN.1 for Misbehavior Reporting v1.1 (2019-04-23)"),
        standard(
            "CAMP SCMS PoC EE Requirements and Specifications v1.2.2 §5.2.8.4: report \
             signing, encryption, storage and upload",
        ),
    ];
    card.assumptions = vec![
        "Consecutive means msgCnt successors from one certificate, at most max_gap_s \
         (0.5 s) apart (consecutive = received: consecutively received)."
            .to_string(),
    ];
    card.limitations = vec![
        "MaxAcceleration and BrakeAcceleration read the frame's accelSet.long, the \
         longitudinal component; a vehicle reversing under brakes is judged as if moving \
         forward (the transmission state is not read)."
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
            bsm: Default::default(),
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

    fn on_clock(time_base: TimeBase) -> UsMbrParams {
        UsMbrParams {
            time_base,
            ..UsMbrParams::default()
        }
    }

    #[test]
    fn a_jittered_arrival_trips_random_position_only_on_the_receive_clock() {
        // Generated and secMark-stamped exactly 100 ms apart, heard 160 ms apart.
        let mut a = bsm(0, 0.0, 20.0);
        a.bsm.sec_mark_ms = Some(0);
        let mut b = bsm(100, 2.0, 20.0);
        b.bsm.sec_mark_ms = Some(100);
        b.received_at = 160_000_000;
        let pair = [a, b];
        for quiet in [TimeBase::GenerationTime, TimeBase::SecMark] {
            assert!(run(&pair, on_clock(quiet))[1].is_empty(), "{quiet:?}");
        }
        assert_eq!(
            run(&pair, on_clock(TimeBase::ReceiveTime))[1],
            vec!["positionJump"]
        );
    }

    #[test]
    fn a_nominal_tenth_of_a_second_trips_random_position_on_a_150_ms_gap() {
        // 150 ms apart at 30 m/s: 4.5 m moved, 3.0 m predicted under a nominal 0.1 s.
        let pair = [bsm(0, 0.0, 30.0), bsm(150, 4.5, 30.0)];
        assert!(run(&pair, on_clock(TimeBase::GenerationTime))[1].is_empty());
        assert_eq!(
            run(&pair, on_clock(TimeBase::Nominal))[1],
            vec!["positionJump"]
        );
        // The consecutive gap is still the true one: 0.6 s apart is not a pair.
        let far = [bsm(0, 0.0, 30.0), bsm(600, 99.0, 30.0)];
        assert!(run(&far, on_clock(TimeBase::Nominal))[1].is_empty());
        let mut p = UsMbrParams::default();
        assert!(p.apply_str("time_base", "receive_time"));
        assert_eq!(p.time_base, TimeBase::ReceiveTime);
        assert!(p.apply_str("time_base", "nominal") && p.apply("nominal_dt_s", 0.2));
        assert_eq!((p.time_base, p.nominal_dt_s), (TimeBase::Nominal, 0.2));
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
                &[
                    bsm(0, 5.0, 3.0),
                    {
                        let mut f = bsm(100, 50.0, 3.0);
                        f.verification = state;
                        f
                    },
                    bsm(200, 5.6, 3.0),
                ],
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

    /// A BSM with its brake status and `accelSet.long`.
    fn braking(t_ms: u64, x: f64, speed: f64, brakes: Option<bool>, accel: f64) -> ObservedMessage {
        let mut m = bsm(t_ms, x, speed);
        m.bsm.brakes_applied = brakes;
        m.bsm.accel_long_mps2 = Some(accel);
        m
    }

    /// [SM-MBR] §2.3.1.3.4, brakes on and the acceleration not negative. Brakes on
    /// with +0.5 m/s² at speed fires; brakes on while slowing (−3 m/s²) does not; a
    /// vehicle held at a red light (brakes on, 0 m/s², standstill) does not, because the
    /// rule's acute angle needs a velocity. Fault: drop the speed gate and the standstill
    /// BSM with +0.3 m/s² fires.
    /// A car pulling away from a stop where it held the brake, with the opt-in brake hold
    /// on (`messages.bsm.brake_hold`), reports its brakes on with a non-negative accelSet
    /// during the pedal overlap, and BrakeAcceleration fires on it, as on the Orem CV data;
    /// with the hold off it does not. Fault: drop the hold in
    /// `OwnVehicle::brakes` -> the first assertion is red.
    #[test]
    fn brake_acceleration_fires_on_a_held_brake_car_pulling_away() {
        use v2xw_node::vehicle::{OwnVehicle, VehicleBus};
        let bus = |t_s: f64, v: f64, a: f64| VehicleBus {
            t: (t_s * 1e9) as u64,
            speed_mps: v,
            a_long_mps2: a,
            a_lat_mps2: 0.0,
            yaw_rate_rad_s: 0.0,
            hazard_lights: false,
            low_beam: false,
            intent: None,
            heavy: false,
            speed_limit_mps: 13.4,
            non_urban: None,
            abs_active: false,
        };
        let fired = |m: ObservedMessage| run(&[m], UsMbrParams::default()).remove(0);
        for (hold, want) in [(true, vec!["brakeAcceleration"]), (false, vec![])] {
            let mut v = OwnVehicle::default().with_brake_hold(hold);
            // The stop's draw holds the brake (0 is below any hold probability).
            v.supply_stop_draw(0.0);
            v.set_bus(bus(1.0, 0.0, 0.0));
            // Pulling away gently (0 < a <= the 0.15 m/s² release threshold): the pedal
            // overlap keeps the held brake on.
            v.set_bus(bus(1.1, 1.0, 0.1));
            let on = v.brakes().wheel_brakes != v2xw_msg::j2735::bsm::BrakeAppliedStatus::NONE;
            assert_eq!(
                fired(braking(0, 0.0, 1.0, Some(on), 0.1)),
                want,
                "hold {hold}"
            );
        }
    }

    #[test]
    fn brake_acceleration_fires_on_brakes_with_positive_acceleration() {
        let fired = |m: ObservedMessage| run(&[m], UsMbrParams::default()).remove(0);
        assert_eq!(
            fired(braking(0, 0.0, 10.0, Some(true), 0.5)),
            vec!["brakeAcceleration"]
        );
        assert!(fired(braking(0, 0.0, 10.0, Some(true), -3.0)).is_empty());
        assert!(
            fired(braking(0, 0.0, 0.0, Some(true), 0.0)).is_empty(),
            "a vehicle held at a red light"
        );
        // Standing still with the brakes on and a positive reading (the instant a held
        // vehicle is released, or a sensor offset): no velocity to make the angle with.
        assert!(
            fired(braking(0, 0.0, 0.0, Some(true), 0.3)).is_empty(),
            "a standstill with brakes on and +0.3 m/s²"
        );
        // Brakes off, or a status the sender marked unavailable, is not judged.
        assert!(fired(braking(0, 0.0, 10.0, Some(false), 0.5)).is_empty());
        assert!(fired(braking(0, 0.0, 10.0, None, 0.5)).is_empty());
        // Zero is not an acute angle (the default tolerance, 0); a tolerance of one LSB
        // lets 0.01 m/s² through.
        assert!(fired(braking(0, 0.0, 10.0, Some(true), 0.0)).is_empty());
        let one_lsb = UsMbrParams {
            brake_accel_tolerance_mps2: 0.01,
            ..UsMbrParams::default()
        };
        assert!(run(&[braking(0, 0.0, 10.0, Some(true), 0.01)], one_lsb)[0].is_empty());
        let off = UsMbrParams {
            brake_acceleration: false,
            ..UsMbrParams::default()
        };
        assert!(run(&[braking(0, 0.0, 10.0, Some(true), 0.5)], off)[0].is_empty());
        // US-only: no fingerprint column, so the detnorm_* contract cannot move.
        assert!(!DetectorId::ALL.contains(&DetectorId::BrakeAcceleration));
    }

    /// MaxAcceleration judges the BSM's own `accelSet` under `accel-set` (the
    /// default): 12 m/s² with equal consecutive speeds fires, and the single BSM is
    /// enough. Under `speed-difference` the same pair is quiet. A BSM without `accelSet`
    /// falls back to the speed difference and is counted. Fault: keep the speed-difference
    /// test and the equal-speed pair does not fire.
    #[test]
    fn max_acceleration_reads_accel_set() {
        let pair = [
            braking(0, 0.0, 10.0, Some(false), 12.0),
            braking(100, 1.0, 10.0, Some(false), 12.0),
        ];
        let got = run(&pair, UsMbrParams::default());
        assert_eq!(got[0], vec!["implausibleAcceleration"], "{got:?}");
        assert_eq!(got[1], vec!["implausibleAcceleration"], "{got:?}");
        assert!(
            run(
                &[braking(0, 0.0, 10.0, Some(false), 9.9)],
                UsMbrParams::default()
            )[0]
            .is_empty()
        );
        assert!(
            run(
                &[braking(0, 0.0, 10.0, Some(false), -12.0)],
                UsMbrParams::default()
            )[0]
            .contains(&"implausibleAcceleration")
        );
        let mut by_speed = UsMbrParams::default();
        assert!(by_speed.apply_str("max_accel_reading", "speed-difference"));
        assert!(run(&pair, by_speed).iter().all(Vec::is_empty));
        // No accelSet: two consecutive speeds, counted.
        let mut d = UsSmMbr::new(UsMbrParams::default());
        let mut ctx = CollectingCtx::new(1);
        let mut fired = Vec::new();
        for m in [bsm(0, 0.0, 10.0), bsm(100, 1.0, 11.1)] {
            fired.push(d.on_message(&mut ctx, &me(), &m, &NoMap).fired.len());
        }
        assert_eq!(fired, vec![0, 1]);
        assert_eq!(d.accel_unavailable_fallbacks(), 1);
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

    /// The per-sender table is bounded by the senders a receiver still hears: a sender
    /// not heard for 30 s is evicted at the next sweep, one heard recently is kept, and
    /// the consecutive check on a kept sender still fires.
    #[test]
    fn a_sender_not_heard_for_thirty_seconds_leaves_the_table() {
        let mut d = UsSmMbr::new(UsMbrParams::default());
        let mut ctx = CollectingCtx::new(1);
        let mut feed = |d: &mut UsSmMbr, signer: [u8; 8], t_ms: u64, x: f64| {
            let mut m = bsm(t_ms, x, 20.0);
            m.signer = signer;
            d.on_message(&mut ctx, &me(), &m, &NoMap)
                .fired
                .iter()
                .map(|o| o.detector.as_str())
                .collect::<Vec<_>>()
        };
        feed(&mut d, [1; 8], 0, 0.0);
        // 2,000 other senders, each heard once, 40 s later; sender 2 heard at 39 s.
        feed(&mut d, [2; 8], 39_000, 0.0);
        for k in 0..2_000u32 {
            let mut id = [9u8; 8];
            id[..4].copy_from_slice(&k.to_be_bytes());
            feed(&mut d, id, 40_000, 0.0);
        }
        assert!(
            !d.last.contains_key(&[1; 8]),
            "a 40 s-stale sender was kept"
        );
        assert!(
            d.last.contains_key(&[2; 8]),
            "a sender heard 1 s ago was evicted"
        );
        assert!(d.last.len() <= 2_001, "{}", d.last.len());
        // The kept sender, frozen at x = 0 while claiming 20 m/s: still caught.
        let fired = feed(&mut d, [2; 8], 39_100, 0.0);
        assert!(fired.contains(&"constantPositionFrozen"), "{fired:?}");
    }

    /// `bsm` with its `msgCnt`.
    fn counted(t_ms: u64, x: f64, speed: f64, cnt: u8) -> ObservedMessage {
        let mut m = bsm(t_ms, x, speed);
        m.bsm.msg_cnt = Some(cnt);
        m
    }

    #[test]
    fn only_bsms_consecutive_on_the_air_are_a_pair() {
        // A car accelerating from 10 m/s; the receiver lost the three BSMs between
        // msgCnt 5 and 9, so its straight-line prediction misses by 1.5 m over 0.4 s.
        // Two BSMs with lost ones between are not "two consecutive BSMs": no check.
        let lost = [counted(0, 0.0, 10.0, 5), counted(400, 5.5, 13.0, 9)];
        assert!(
            run(&lost, UsMbrParams::default()).iter().all(Vec::is_empty),
            "a pair with lost BSMs between was judged"
        );
        // The earlier reading (consecutively received) judges it and fires.
        let mut received = UsMbrParams::default();
        assert!(received.apply_str("consecutive", "received"));
        assert_eq!(run(&lost, received)[1], vec!["positionJump"]);
        // msgCnt successors, wrapping at 127 -> 0, are judged as before: 1.5 m off in
        // 0.1 s still fires.
        let next = [counted(0, 0.0, 10.0, 127), counted(100, 2.5, 10.0, 0)];
        assert_eq!(run(&next, UsMbrParams::default())[1], vec!["positionJump"]);
        // The report evidence names the first BSM only for a pair on the air.
        let mut d = UsSmMbr::new(UsMbrParams::default());
        let mut ctx = CollectingCtx::new(1);
        d.on_message(&mut ctx, &me(), &lost[0], &NoMap);
        assert_eq!(d.consecutive_before(&lost[1]), None);
        assert_eq!(d.consecutive_before(&counted(100, 1.0, 10.0, 6)), Some(0));
    }

    /// A creeping car (0.06 m/s north) moves 6 mm in 0.1 s. Its true positions sit 0.3
    /// and 0.84 of a latitude LSB (1e-7°, 1.11 cm) above a grid line, so the BSMs on the
    /// air carry two latitudes one LSB apart. Judged on the unquantised claim the move is
    /// under 1 cm (ConstantPosition); judged on what the receiver decodes it is 1.11 cm.
    #[test]
    fn constant_position_judges_the_decoded_latitude_not_the_unquantised_claim() {
        use v2xw_core::geo::GeoOrigin;
        use v2xw_core::geom::Vec3;
        let origin = GeoOrigin::new(40.75, -73.99, 0.0);
        let per_lsb = origin.metres_per_degree_latitude() * 1e-7;
        let y0 = 1000.3 * per_lsb;
        let y1 = y0 + 0.006;
        let north = std::f64::consts::FRAC_PI_2;
        let at = |t_ms: u64, y: f64, decoded: bool| {
            let (p, s, h) = if decoded {
                v2xw_msg::j2735::bsm::on_air(&origin, Vec3::new(0.0, y, 0.0), 0.06, north)
            } else {
                (Vec3::new(0.0, y, 0.0), 0.06, north)
            };
            let mut m = bsm(t_ms, p.x, s);
            m.claimed_y_m = p.y;
            m.claimed_heading_rad = h;
            m
        };
        let raw = run(
            &[at(0, y0, false), at(100, y1, false)],
            UsMbrParams::default(),
        );
        assert!(raw[1].contains(&"constantPositionFrozen"), "{raw:?}");
        let air = run(
            &[at(0, y0, true), at(100, y1, true)],
            UsMbrParams::default(),
        );
        assert!(air.iter().all(Vec::is_empty), "{air:?}");
    }

    /// Every fired check writes one `det.observation` row naming the pair it judged.
    #[test]
    fn a_fired_check_writes_one_observation_row_with_its_pair() {
        let mut d = UsSmMbr::new(UsMbrParams::default());
        let mut ctx = CollectingCtx::new(1);
        d.on_message(&mut ctx, &me(), &counted(0, 5.0, 3.0, 9), &NoMap);
        assert!(ctx.on_channel("det.observation").is_empty());
        let v = d.on_message(&mut ctx, &me(), &counted(100, 5.0, 3.0, 10), &NoMap);
        let rows = ctx.on_channel("det.observation");
        assert_eq!(rows.len(), v.fired.len());
        assert_eq!(
            rows.len(),
            1,
            "a frozen position fires ConstantPosition alone"
        );
        let r: DetObservation = serde_json::from_slice(&rows[0].json).unwrap();
        assert_eq!(r.detector, "constantPositionFrozen");
        assert_eq!(r.subject, v2xw_core::hash::hex_encode(&[7u8; 8]));
        assert_eq!((r.prev_msg_cnt, r.msg_cnt), (Some(9), Some(10)));
        assert_eq!(r.dt_s, Some(0.1));
        assert_eq!(r.residual, Some(0.0));
        assert_eq!(rows[0].visibility, v2xw_core::ctx::Visibility::Node);
    }
}
