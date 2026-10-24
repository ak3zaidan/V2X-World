//! The ETSI TS 103 759 observation classes 1–5, the F2MD-style checks, and the perception
//! cross-check — the detector families 07-threats-and-detection.md §3.1 asks for
//! *alongside* the legacy twelve rather than in place of them.
//!
//! # Why alongside
//!
//! The legacy suite's formulas and normalisation are frozen because the machine-learning
//! contract's `detnorm_*` columns are those signals (07-threats-and-detection.md §3.1). A
//! new family that changed them would invalidate the corpus. So this is a second suite
//! with its own model card, its own thresholds — every one of them cited to 04-models.md
//! §14 — and its own check ids, and a run may host both.
//!
//! # The five classes
//!
//! | Class | TS 103 759 | Checks here |
//! |---|---|---|
//! | 1 | implausible values | range, speed, acceleration, position confidence, oversized message |
//! | 2 | inconsistency with previous messages from the same station | position, position/speed, speed, position/heading, beacon frequency, generation-time order, sudden appearance |
//! | 3 | inconsistency with the local environment and the LDM | map position plausibility, certificate region |
//! | 4 | inconsistency with on-board sensors | the perception cross-check |
//! | 5 | inconsistency with other stations' messages | physical intersection, CPM consistency |
//!
//! Two envelope-level checks sit outside the classes, because a signature that did not
//! verify is not an *observation* about the sender's behaviour.
//!
//! # A check that was not run is not a check that passed
//!
//! [`TsVerdict::score`] returns `Option<f64>`, and a check whose input this receiver does
//! not have is **absent from the verdict** rather than scored zero. Four things are
//! commonly absent, and each is counted so that a run can see it:
//!
//! * no perception ([`crate::obs::NoPerception`]) — class 4 cannot run at all;
//! * a claim outside the sensor's coverage — class 4 must not run, because the absence of
//!   a sensed object in a blind spot is the blind spot and not evidence;
//! * no measured payload size ([`crate::obs::EnvelopeExtras`]) — the oversized-message
//!   check has no input;
//! * no declared own region — the certificate-region check has nothing to compare against.
//!
//! A suite that scored those zero would report a class-4 recall for a fleet with no
//! sensors, and nobody would notice.
//!
//! # Why this suite does not implement [`crate::detect::Detector`]
//!
//! That trait returns a [`crate::detect::Verdict`], whose observations are typed on the
//! [`crate::detect::DetectorId`] enum — and that enum is not a list of detectors, it is the
//! `detnorm_*` **column order of the legacy corpus**. Adding a variant to it changes the
//! on-disk feature table; making [`crate::detect::Observation::detector`] a string changes
//! every reader of it, including the engine and the metrics crate. Neither is this crate's
//! to do unilaterally, so this suite has its own [`TsVerdict`] and its own entry point, and
//! reports through [`crate::report::MisbehaviourReport::from_named_checks`] — same wire
//! shape, same quantisation, its own check ids. What the interface of 03-interfaces.md §9
//! needs, and what the engine will want, is a string-typed observation; that is a finding
//! for whoever owns `detect` and the engine's detector host, not something to paper over
//! by mapping a TS 103 759 class onto a legacy check id it is not.
//!
//! # The four verification states are four different things
//!
//! This is the defect that produced a 98 % false-positive rate on honest traffic in the
//! legacy suite's first port, and [`Ts103759Suite::check`] matches all four explicitly
//! with no catch-all arm:
//!
//! * [`VerificationState::Valid`] — every check runs.
//! * [`VerificationState::BadSignature`] — the receiver *tested* the signature and it
//!   failed. [`Ts103759Check::SignatureInvalid`] fires; no content check runs, because the
//!   content of a message whose signature failed is not evidence of anything.
//! * [`VerificationState::UnknownCertificate`] — the receiver could not build a chain.
//!   [`Ts103759Check::CertificateUnknown`] fires. This is a *different* finding from a bad
//!   signature: it is a statement about this receiver's trust store, and a report citing
//!   it names a different misbehaviour (P2PCD learning, an unknown authority) than one
//!   citing a forged signature.
//! * [`VerificationState::Unverified`] — the receiver's policy **deferred** the check.
//!   Nothing fires, nothing is scored, and [`Ts103759Suite::deferred`] counts it. An
//!   absence of evidence is not evidence of misbehaviour.

use std::collections::BTreeMap;

use crate::capability::{angle_diff_rad, bearing_rad};
use crate::cards::{LEGACY_PY, design, legacy_param, standard};
use crate::ctx::{ThreatCtx, ThreatCtxExt};
use crate::detect::DetectorCost;
use crate::obs::{
    EnvelopeExtras, LocalEnvironment, LocalPerception, NoPerception, ObservedKind, ObservedMessage,
    PerceivedObject, RegionId, SelfBelief, VerificationState,
};
use crate::records::DetObservation;
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::math;
use v2xw_core::model::Model;
use v2xw_core::time::{SimTime, ns_to_secs};

/// The model id this suite's card and every `det.observation` it writes carry.
pub const MODEL_ID: &str = "threat/detector/ts103759-observations";

/// A TS 103 759 misbehaviour observation class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObservationClass {
    /// Class 1: implausible values.
    ImplausibleValues,
    /// Class 2: inconsistency with previous messages from the same station.
    PreviousMessages,
    /// Class 3: inconsistency with the local environment or the LDM.
    LocalEnvironment,
    /// Class 4: inconsistency with on-board sensors.
    OnBoardSensors,
    /// Class 5: inconsistency with other stations' messages.
    OtherStations,
    /// Not an observation class: an envelope-level cryptographic conclusion.
    Envelope,
}

impl ObservationClass {
    /// Every class, 1 to 5, then the envelope group.
    pub const ALL: [ObservationClass; 6] = [
        ObservationClass::ImplausibleValues,
        ObservationClass::PreviousMessages,
        ObservationClass::LocalEnvironment,
        ObservationClass::OnBoardSensors,
        ObservationClass::OtherStations,
        ObservationClass::Envelope,
    ];

    /// The class number TS 103 759 gives it; `0` for the envelope group.
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            ObservationClass::ImplausibleValues => 1,
            ObservationClass::PreviousMessages => 2,
            ObservationClass::LocalEnvironment => 3,
            ObservationClass::OnBoardSensors => 4,
            ObservationClass::OtherStations => 5,
            ObservationClass::Envelope => 0,
        }
    }
}

/// One check in this suite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ts103759Check {
    /// The claimed position is further from the receiver than any link could reach.
    RangePlausibility,
    /// The claimed speed is above anything a vehicle does.
    SpeedPlausibility,
    /// The implied acceleration is above anything a vehicle does.
    AccelerationPlausibility,
    /// The broadcast position confidence is outside its plausible bound.
    ConfidencePlausibility,
    /// The message is larger than the largest conforming payload.
    OversizedMessage,
    /// The claimed displacement is further than the plausible maximum speed allows.
    PositionConsistency,
    /// The claimed displacement disagrees with the claimed speed over the interval.
    PositionSpeedConsistency,
    /// The claimed speed changed by more than the interval allows, asymmetrically for
    /// acceleration and braking.
    SpeedConsistency,
    /// The claimed heading disagrees with the bearing of the claimed motion.
    PositionHeadingConsistency,
    /// The sender is transmitting more often than the minimum inter-beacon time.
    BeaconFrequency,
    /// The generation time did not advance.
    GenerationTimeOrder,
    /// A station was first heard implausibly close.
    SuddenAppearance,
    /// The claimed position is not on any road the node's map knows.
    PositionPlausibility,
    /// The certificate states a region other than the receiver's own.
    ForeignRegion,
    /// No object this node's own sensors hold corroborates a claim inside their coverage.
    PerceptionCrossCheck,
    /// Two stations claim to occupy the same space.
    Intersection,
    /// A collective-perception message's objects are not corroborated by this node's own
    /// perception, inside the part of the region this node can see.
    CpmConsistency,
    /// The signature was tested and did not verify.
    SignatureInvalid,
    /// The signer's certificate chain could not be built.
    CertificateUnknown,
}

impl Ts103759Check {
    /// Every check, grouped by class.
    pub const ALL: [Ts103759Check; 19] = [
        Ts103759Check::RangePlausibility,
        Ts103759Check::SpeedPlausibility,
        Ts103759Check::AccelerationPlausibility,
        Ts103759Check::ConfidencePlausibility,
        Ts103759Check::OversizedMessage,
        Ts103759Check::PositionConsistency,
        Ts103759Check::PositionSpeedConsistency,
        Ts103759Check::SpeedConsistency,
        Ts103759Check::PositionHeadingConsistency,
        Ts103759Check::BeaconFrequency,
        Ts103759Check::GenerationTimeOrder,
        Ts103759Check::SuddenAppearance,
        Ts103759Check::PositionPlausibility,
        Ts103759Check::ForeignRegion,
        Ts103759Check::PerceptionCrossCheck,
        Ts103759Check::Intersection,
        Ts103759Check::CpmConsistency,
        Ts103759Check::SignatureInvalid,
        Ts103759Check::CertificateUnknown,
    ];

    /// The check's id, as a `det.observation` record and a report's reason code spell it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Ts103759Check::RangePlausibility => "rangePlausibility",
            Ts103759Check::SpeedPlausibility => "speedPlausibility",
            Ts103759Check::AccelerationPlausibility => "accelerationPlausibility",
            Ts103759Check::ConfidencePlausibility => "confidencePlausibility",
            Ts103759Check::OversizedMessage => "oversizedMessage",
            Ts103759Check::PositionConsistency => "positionConsistency",
            Ts103759Check::PositionSpeedConsistency => "positionSpeedConsistencyTs",
            Ts103759Check::SpeedConsistency => "speedConsistency",
            Ts103759Check::PositionHeadingConsistency => "positionHeadingConsistency",
            Ts103759Check::BeaconFrequency => "beaconFrequencyTs",
            Ts103759Check::GenerationTimeOrder => "generationTimeOrder",
            Ts103759Check::SuddenAppearance => "suddenAppearance",
            Ts103759Check::PositionPlausibility => "positionPlausibility",
            Ts103759Check::ForeignRegion => "foreignRegion",
            Ts103759Check::PerceptionCrossCheck => "perceptionCrossCheck",
            Ts103759Check::Intersection => "intersection",
            Ts103759Check::CpmConsistency => "cpmConsistency",
            Ts103759Check::SignatureInvalid => "signatureInvalid",
            Ts103759Check::CertificateUnknown => "certificateUnknown",
        }
    }

    /// Parses a check id.
    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        Ts103759Check::ALL.into_iter().find(|c| c.as_str() == id)
    }

    /// Which observation class it belongs to.
    #[must_use]
    pub const fn class(self) -> ObservationClass {
        match self {
            Ts103759Check::RangePlausibility
            | Ts103759Check::SpeedPlausibility
            | Ts103759Check::AccelerationPlausibility
            | Ts103759Check::ConfidencePlausibility
            | Ts103759Check::OversizedMessage => ObservationClass::ImplausibleValues,
            Ts103759Check::PositionConsistency
            | Ts103759Check::PositionSpeedConsistency
            | Ts103759Check::SpeedConsistency
            | Ts103759Check::PositionHeadingConsistency
            | Ts103759Check::BeaconFrequency
            | Ts103759Check::GenerationTimeOrder
            | Ts103759Check::SuddenAppearance => ObservationClass::PreviousMessages,
            Ts103759Check::PositionPlausibility | Ts103759Check::ForeignRegion => {
                ObservationClass::LocalEnvironment
            }
            Ts103759Check::PerceptionCrossCheck => ObservationClass::OnBoardSensors,
            Ts103759Check::Intersection | Ts103759Check::CpmConsistency => {
                ObservationClass::OtherStations
            }
            Ts103759Check::SignatureInvalid | Ts103759Check::CertificateUnknown => {
                ObservationClass::Envelope
            }
        }
    }

    /// Every check in one class, in [`Ts103759Check::ALL`] order.
    #[must_use]
    pub fn in_class(class: ObservationClass) -> Vec<Ts103759Check> {
        Ts103759Check::ALL
            .into_iter()
            .filter(|c| c.class() == class)
            .collect()
    }
}

impl core::fmt::Display for Ts103759Check {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One check firing on one message.
#[derive(Debug, Clone, PartialEq)]
pub struct TsObservation {
    /// Which check.
    pub check: Ts103759Check,
    /// Its normalised score, ≈ 1 at the firing threshold.
    pub score: f64,
    /// The subject: the signer's certificate digest in hex.
    pub subject: String,
    /// When the observation was made, on the observing node's clock.
    pub at: SimTime,
}

/// What one message produced.
///
/// [`TsVerdict::score`] is an `Option` on purpose: a check that could not run is absent,
/// not zero. See the module documentation.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TsVerdict {
    /// The subject: the signer's certificate digest in hex.
    pub subject: String,
    /// The score of every check that **ran**.
    pub scores: BTreeMap<Ts103759Check, f64>,
    /// The checks that fired, highest score first.
    pub fired: Vec<TsObservation>,
}

impl TsVerdict {
    /// The score of `check`, or `None` when this receiver could not run it.
    #[must_use]
    pub fn score(&self, check: Ts103759Check) -> Option<f64> {
        self.scores.get(&check).copied()
    }

    /// Whether `check` ran at all.
    #[must_use]
    pub fn evaluated(&self, check: Ts103759Check) -> bool {
        self.scores.contains_key(&check)
    }

    /// True when at least one check fired.
    #[must_use]
    pub fn fired(&self) -> bool {
        !self.fired.is_empty()
    }

    /// The highest-scoring fired check: the reason a report leads with.
    #[must_use]
    pub fn leading(&self) -> Option<&TsObservation> {
        self.fired.first()
    }

    /// The checks that fired, as reason codes in firing order.
    #[must_use]
    pub fn reason_codes(&self) -> Vec<String> {
        self.fired
            .iter()
            .map(|o| o.check.as_str().to_string())
            .collect()
    }

    /// The scores that ran, as `(check id, score)` pairs in check order — the fingerprint
    /// columns a report carries.
    #[must_use]
    pub fn columns(&self) -> Vec<(String, f64)> {
        self.scores
            .iter()
            .map(|(c, v)| (c.as_str().to_string(), crate::records::q(*v)))
            .collect()
    }

    /// The largest score among the checks that ran.
    #[must_use]
    pub fn max(&self) -> f64 {
        self.scores.values().copied().fold(0.0, f64::max)
    }

    /// The highest score in one class, over the checks of that class that ran.
    #[must_use]
    pub fn class_max(&self, class: ObservationClass) -> f64 {
        self.scores
            .iter()
            .filter(|(c, _)| c.class() == class)
            .map(|(_, v)| *v)
            .fold(0.0, f64::max)
    }
}

/// Everything the suite compares against. Every default is cited to 04-models.md §14.
#[derive(Debug, Clone, PartialEq)]
pub struct Ts103759Params {
    /// An explicit acceptance range (ART), metres: the furthest a claim can plausibly be
    /// from the receiver. `None`, the default, uses [`Ts103759Params::acceptance_ranges`]
    /// for [`Ts103759Params::environment`] — the range derived from this engine's own
    /// link budget (F2MD's `MAX_PLAUSIBLE_RANGE` 420 m is Veins' 20 mW radio, not ours;
    /// docs/design/frameworks/f2md.md §5 P9). [`Ts103759Params::art_m`] is the value in
    /// force.
    pub max_plausible_range_m: Option<f64>,
    /// The receiver's propagation environment, which selects the acceptance range.
    pub environment: RadioEnvironment,
    /// The acceptance range per environment, metres ([`AcceptanceRanges::DERIVED`]).
    pub acceptance_ranges: AcceptanceRanges,
    /// The fastest a vehicle plausibly goes, m/s (F2MD `MIN_MAX_SPEED`, 40).
    pub max_plausible_speed_mps: f64,
    /// The hardest a vehicle plausibly accelerates, m/s² (F2MD `MIN_MAX_ACCEL`, 3).
    pub max_plausible_accel_mps2: f64,
    /// The hardest a vehicle plausibly brakes, m/s² (F2MD `MIN_MAX_DECEL`, 4.5).
    pub max_plausible_decel_mps2: f64,
    /// The largest plausible broadcast position confidence, metres (F2MD
    /// `MAX_CONFIDENCE_RANGE`, 10).
    pub max_confidence_m: f64,
    /// The largest conforming payload, bytes (04-models.md §4.6, maximum MSDU 2 304).
    pub max_payload_bytes: u32,
    /// The message profile the beacon-timing thresholds are taken from: the shortest
    /// and longest conforming interval and the jitter tolerance
    /// ([`BeaconProfile::BSM_J2945_1`] unless set).
    pub beacon_profile: BeaconProfile,
    /// An explicit longest interval the consistency checks will compare across, seconds.
    /// `None`, the default, is [`MAX_TIME_DELTA_INTERVALS`] (F2MD `MAX_TIME_DELTA` 3.1 s
    /// at 1 Hz, re-expressed in intervals) times the profile's longest conforming
    /// interval; [`Ts103759Params::time_delta_s`] is the value in force.
    pub max_time_delta_s: Option<f64>,
    /// The consistency margin, metres (F2MD `MAX_MGT_RNG`, 4).
    pub mgt_rng_m: f64,
    /// The margin on a claimed speed increase, m/s per second (F2MD `MAX_MGT_RNG_UP`,
    /// 2.1).
    pub mgt_rng_up_mps: f64,
    /// The margin on a claimed speed decrease, m/s per second (F2MD `MAX_MGT_RNG_DOWN`,
    /// 6.2).
    pub mgt_rng_down_mps: f64,
    /// The largest plausible heading change, degrees (F2MD `MAX_HEADING_CHANGE`, 90).
    pub max_heading_change_deg: f64,
    /// The interval within which the heading check compares, seconds (F2MD
    /// `POS_HEADING_TIME`, 1.1).
    pub pos_heading_time_s: f64,
    /// An explicit minimum conforming inter-beacon time, seconds. `None`, the default, is
    /// the profile's shortest conforming interval less its jitter tolerance
    /// ([`BeaconProfile::min_beacon_interval_s`]): 75 ms for a BSM. F2MD's
    /// `MAX_BEACON_FREQUENCY` 0.9 s is the same rule for a 1 Hz VeReMi beacon, and taken
    /// as it stands it flags every honest 10 Hz BSM sender (f2md.md §5 P1).
    /// [`Ts103759Params::min_interval_s`] is the value in force.
    pub min_beacon_interval_s: Option<f64>,
    /// An explicit time a receiver must have listened before a first sighting counts as
    /// sudden, seconds. `None`, the default, is [`MAX_SA_TIME_INTERVALS`] (F2MD
    /// `MAX_SA_TIME` 2.1 s at 1 Hz) times the profile's longest conforming interval;
    /// [`Ts103759Params::sa_time_s`] is the value in force.
    pub max_sa_time_s: Option<f64>,
    /// The furthest a sudden appearance is considered at all, metres (F2MD
    /// `MAX_SA_RANGE`, 420).
    pub max_sa_range_m: f64,
    /// Inside this distance, a first sighting is a sudden appearance, metres.
    pub sudden_appearance_m: f64,
    /// The tolerated distance from the nearest known road, metres (F2MD
    /// `MAX_DISTANCE_FROM_ROUTE`, 2).
    pub max_distance_from_route_m: f64,
    /// Two stations closer than this are claiming the same space, metres (F2MD
    /// `MAX_PROXIMITY_DISTANCE`, 2).
    pub proximity_distance_m: f64,
    /// The longitudinal proximity box, metres (F2MD `MAX_PROXIMITY_RANGE_L`, 30).
    pub proximity_range_l_m: f64,
    /// How long another station's claim stays comparable, seconds (F2MD
    /// `MAX_DELTA_INTER`, 2.0).
    pub max_delta_inter_s: f64,
    /// How many uncertainty sigmas a residual must exceed (the legacy
    /// `detector_z_threshold`, 3.0).
    pub z_threshold: f64,
    /// The floor on the uncertainty scale, metres (the legacy `consistency_threshold_m`,
    /// 5.0).
    pub gate_floor_m: f64,
    /// The score an envelope-level failure reports (the legacy hard-fail score, 1.5).
    pub hard_fail_score: f64,
    /// Consecutive violations before a check fires (the legacy `detector_min_consec`, 2).
    pub min_consecutive: u32,
    /// The fraction of a CPM's visible objects that must be uncorroborated before the
    /// CPM-consistency check fires.
    pub cpm_unmatched_fraction: f64,
    /// The region this receiver is in, when the scenario declares one.
    ///
    /// `None` leaves [`Ts103759Check::ForeignRegion`] unevaluated — not passed.
    pub own_region: Option<RegionId>,
}

impl Default for Ts103759Params {
    fn default() -> Self {
        Self {
            max_plausible_range_m: None,
            environment: RadioEnvironment::Highway,
            acceptance_ranges: AcceptanceRanges::DERIVED,
            max_plausible_speed_mps: 40.0,
            max_plausible_accel_mps2: 3.0,
            max_plausible_decel_mps2: 4.5,
            max_confidence_m: 10.0,
            max_payload_bytes: crate::attack::MAX_MSDU_BYTES,
            beacon_profile: BeaconProfile::BSM_J2945_1,
            max_time_delta_s: None,
            mgt_rng_m: 4.0,
            mgt_rng_up_mps: 2.1,
            mgt_rng_down_mps: 6.2,
            max_heading_change_deg: 90.0,
            pos_heading_time_s: 1.1,
            min_beacon_interval_s: None,
            max_sa_time_s: None,
            max_sa_range_m: 420.0,
            sudden_appearance_m: 30.0,
            max_distance_from_route_m: 2.0,
            proximity_distance_m: 2.0,
            proximity_range_l_m: 30.0,
            max_delta_inter_s: 2.0,
            z_threshold: 3.0,
            gate_floor_m: 5.0,
            hard_fail_score: 1.5,
            min_consecutive: 2,
            cpm_unmatched_fraction: 1.0,
            own_region: None,
        }
    }
}

impl Ts103759Params {
    /// The cited defaults for a receiver in `env` whose neighbours send `profile`.
    #[must_use]
    pub fn for_link(env: RadioEnvironment, profile: BeaconProfile) -> Self {
        Self::default()
            .with_environment(env)
            .with_beacon_profile(profile)
    }

    /// The same thresholds for a receiver in another propagation environment.
    #[must_use]
    pub fn with_environment(mut self, env: RadioEnvironment) -> Self {
        self.environment = env;
        self
    }

    /// The same thresholds with the beacon timing of another message profile.
    #[must_use]
    pub fn with_beacon_profile(mut self, profile: BeaconProfile) -> Self {
        self.beacon_profile = profile;
        self
    }

    /// The acceptance range in force, metres: the explicit `max_plausible_range_m`, or
    /// the derived range for this receiver's environment.
    #[must_use]
    pub fn art_m(&self) -> f64 {
        self.max_plausible_range_m
            .unwrap_or_else(|| self.acceptance_ranges.get(self.environment))
    }

    /// The minimum conforming inter-beacon time in force, seconds.
    #[must_use]
    pub fn min_interval_s(&self) -> f64 {
        self.min_beacon_interval_s
            .unwrap_or_else(|| self.beacon_profile.min_beacon_interval_s())
    }

    /// The longest interval the consistency checks compare across, in force, seconds.
    #[must_use]
    pub fn time_delta_s(&self) -> f64 {
        self.max_time_delta_s
            .unwrap_or_else(|| self.beacon_profile.max_time_delta_s())
    }

    /// The listening time before a first sighting counts as sudden, in force, seconds.
    #[must_use]
    pub fn sa_time_s(&self) -> f64 {
        self.max_sa_time_s
            .unwrap_or_else(|| self.beacon_profile.max_sa_time_s())
    }

    /// Sets one numeric parameter by name, the way `detection.local[].params` reaches a
    /// suite (as [`crate::us_mbr::UsMbrParams::apply`] does); false for a name the suite
    /// does not read. The keys are order-independent: an explicit threshold
    /// (`max_plausible_range_m`, `min_beacon_interval_s`, `max_time_delta_s`,
    /// `max_sa_time_s`) wins over the derived one whatever order the keys arrive in, and
    /// the `art_*_m` and `beacon_*` keys change what the derivation starts from.
    pub fn apply(&mut self, key: &str, v: f64) -> bool {
        match key {
            "max_plausible_range_m" => self.max_plausible_range_m = Some(v),
            "art_highway_m" => self.acceptance_ranges.highway_m = v,
            "art_rural_m" => self.acceptance_ranges.rural_m = v,
            "art_suburban_m" => self.acceptance_ranges.suburban_m = v,
            "art_urban_m" => self.acceptance_ranges.urban_m = v,
            "beacon_min_interval_s" => self.beacon_profile.min_interval_s = v,
            "beacon_max_interval_s" => self.beacon_profile.max_interval_s = v,
            "beacon_jitter_tolerance_s" => self.beacon_profile.jitter_tolerance_s = v,
            "min_beacon_interval_s" => self.min_beacon_interval_s = Some(v),
            "max_time_delta_s" => self.max_time_delta_s = Some(v),
            "max_sa_time_s" => self.max_sa_time_s = Some(v),
            "max_plausible_speed_mps" => self.max_plausible_speed_mps = v,
            "max_plausible_accel_mps2" => self.max_plausible_accel_mps2 = v,
            "max_plausible_decel_mps2" => self.max_plausible_decel_mps2 = v,
            "max_confidence_m" => self.max_confidence_m = v,
            "mgt_rng_m" => self.mgt_rng_m = v,
            "mgt_rng_up_mps" => self.mgt_rng_up_mps = v,
            "mgt_rng_down_mps" => self.mgt_rng_down_mps = v,
            "max_heading_change_deg" => self.max_heading_change_deg = v,
            "pos_heading_time_s" => self.pos_heading_time_s = v,
            "max_sa_range_m" => self.max_sa_range_m = v,
            "sudden_appearance_m" => self.sudden_appearance_m = v,
            "max_distance_from_route_m" => self.max_distance_from_route_m = v,
            "proximity_distance_m" => self.proximity_distance_m = v,
            "proximity_range_l_m" => self.proximity_range_l_m = v,
            "max_delta_inter_s" => self.max_delta_inter_s = v,
            "z_threshold" => self.z_threshold = v,
            "gate_floor_m" => self.gate_floor_m = v,
            "hard_fail_score" => self.hard_fail_score = v,
            "cpm_unmatched_fraction" => self.cpm_unmatched_fraction = v,
            // Counts: rounded, and saturating at the type's bounds.
            "min_consecutive" => self.min_consecutive = v.round() as u32,
            "max_payload_bytes" => self.max_payload_bytes = v.round() as u32,
            _ => return false,
        }
        true
    }
}

/// Every key [`Ts103759Params::apply`] reads.
pub const PARAM_KEYS: [&str; 32] = [
    "max_plausible_range_m",
    "art_highway_m",
    "art_rural_m",
    "art_suburban_m",
    "art_urban_m",
    "beacon_min_interval_s",
    "beacon_max_interval_s",
    "beacon_jitter_tolerance_s",
    "min_beacon_interval_s",
    "max_time_delta_s",
    "max_sa_time_s",
    "max_plausible_speed_mps",
    "max_plausible_accel_mps2",
    "max_plausible_decel_mps2",
    "max_confidence_m",
    "mgt_rng_m",
    "mgt_rng_up_mps",
    "mgt_rng_down_mps",
    "max_heading_change_deg",
    "pos_heading_time_s",
    "max_sa_range_m",
    "sudden_appearance_m",
    "max_distance_from_route_m",
    "proximity_distance_m",
    "proximity_range_l_m",
    "max_delta_inter_s",
    "z_threshold",
    "gate_floor_m",
    "hard_fail_score",
    "cpm_unmatched_fraction",
    "min_consecutive",
    "max_payload_bytes",
];

// ---------------------------------------------------------------------------------------
// Beacon timing from the message profile (docs/design/frameworks/f2md.md §5 P1)
// ---------------------------------------------------------------------------------------

/// F2MD's `MAX_TIME_DELTA` (3.1 s) in beacon intervals: F2MD sets it for a 1 Hz VeReMi
/// beacon, so it is 3.1 intervals, and that is what carries over to another rate.
pub const MAX_TIME_DELTA_INTERVALS: f64 = 3.1;

/// F2MD's `MAX_SA_TIME` (2.1 s) in beacon intervals, for the same reason.
pub const MAX_SA_TIME_INTERVALS: f64 = 2.1;

/// The beacon timing a message profile allows, from which the beacon-frequency check and
/// the interval windows are derived.
///
/// The windows scale with the **longest** conforming interval rather than the nominal
/// one: a J2945/1 sender under congestion control stretches its interval towards
/// `vMaxITT`, and a window sized for 100 ms would then compare no two of its messages at
/// all. At 1 Hz the two are the same, which is how F2MD's 3.1 s and 2.1 s come back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeaconProfile {
    /// The profile's id (the preset it started from).
    pub id: &'static str,
    /// The shortest conforming interval between two messages, seconds.
    pub min_interval_s: f64,
    /// The longest conforming interval, seconds.
    pub max_interval_s: f64,
    /// How far below the shortest interval an honest arrival gap may fall, seconds:
    /// generation jitter and channel-access delay on the two messages a gap spans.
    pub jitter_tolerance_s: f64,
}

impl BeaconProfile {
    /// The US BSM, SAE J2945/1: 100 ms nominal and `vMinITT`, `vMaxITT` 600 ms
    /// (04-models.md §8.1; `v2xw_msg::BsmGenParams::j2945_1`). The 25 ms tolerance
    /// covers the engine's own 10 ms hand-off jitter (`v2xw_msg::GenerationTiming`,
    /// ns-3's `TxMaxDelay`), which alone brings an honest gap down to 90 ms, and leaves
    /// 15 ms for EDCA channel-access and queueing differences between two consecutive
    /// frames. A 20 Hz sender's 50 ms gaps stay well under the resulting 75 ms.
    pub const BSM_J2945_1: BeaconProfile = BeaconProfile {
        id: "bsm-j2945-1",
        min_interval_s: 0.1,
        max_interval_s: 0.6,
        jitter_tolerance_s: 0.025,
    };

    /// The ETSI CAM, EN 302 637-2 V1.4.1 §6.1.3: `T_GenCamMin` 100 ms, `T_GenCamMax`
    /// 1,000 ms, with the BSM's tolerance.
    pub const CAM_EN302637_2: BeaconProfile = BeaconProfile {
        id: "cam-en302637-2",
        min_interval_s: 0.1,
        max_interval_s: 1.0,
        jitter_tolerance_s: 0.025,
    };

    /// F2MD's own 1 Hz VeReMi beacon, for comparison runs: it reproduces
    /// `MAX_BEACON_FREQUENCY` 0.9 s, `MAX_TIME_DELTA` 3.1 s and `MAX_SA_TIME` 2.1 s
    /// (veins-f2md `F2MDParameters.h`, 04-models.md §14).
    pub const F2MD_VEREMI_1HZ: BeaconProfile = BeaconProfile {
        id: "f2md-veremi-1hz",
        min_interval_s: 1.0,
        max_interval_s: 1.0,
        jitter_tolerance_s: 0.1,
    };

    /// Every shipped profile.
    pub const ALL: [BeaconProfile; 3] = [
        BeaconProfile::BSM_J2945_1,
        BeaconProfile::CAM_EN302637_2,
        BeaconProfile::F2MD_VEREMI_1HZ,
    ];

    /// The profile with this id.
    #[must_use]
    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id == id)
    }

    /// The minimum conforming arrival gap: the shortest interval less the tolerance.
    #[must_use]
    pub fn min_beacon_interval_s(&self) -> f64 {
        (self.min_interval_s - self.jitter_tolerance_s).max(0.0)
    }

    /// The longest interval the consistency checks compare across.
    #[must_use]
    pub fn max_time_delta_s(&self) -> f64 {
        MAX_TIME_DELTA_INTERVALS * self.max_interval_s
    }

    /// How long a receiver listens before a first sighting counts as sudden.
    #[must_use]
    pub fn max_sa_time_s(&self) -> f64 {
        MAX_SA_TIME_INTERVALS * self.max_interval_s
    }
}

// ---------------------------------------------------------------------------------------
// The acceptance range from this engine's own link budget (f2md.md §5 P9)
// ---------------------------------------------------------------------------------------

/// The propagation environment a receiver is in: the same four classes, with the same
/// labels, as `v2xw_world::model::EnvClass`, which this crate does not depend on (see
/// the crate documentation, "The seam").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RadioEnvironment {
    /// A motorway corridor.
    Highway,
    /// Open country.
    Rural,
    /// Low-rise built-up area.
    Suburban,
    /// Dense built-up area.
    Urban,
}

impl RadioEnvironment {
    /// Every environment.
    pub const ALL: [RadioEnvironment; 4] = [
        RadioEnvironment::Highway,
        RadioEnvironment::Rural,
        RadioEnvironment::Suburban,
        RadioEnvironment::Urban,
    ];

    /// The label `EnvClass` uses.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            RadioEnvironment::Highway => "highway",
            RadioEnvironment::Rural => "rural",
            RadioEnvironment::Suburban => "suburban",
            RadioEnvironment::Urban => "urban",
        }
    }

    /// The environment with this label.
    #[must_use]
    pub fn parse(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.label() == label)
    }
}

/// The transmit power the acceptance range is derived at, dBm: the engine's broadcast
/// default (`v2xw_radio::types::FrameDescriptor::broadcast`).
pub const ART_TX_POWER_DBM: f64 = 23.0;
/// The antenna gain at each end, dBi: a car's default (`ActorClass::Car`, TR 36.885
/// Table A.1.1-1).
pub const ART_ANTENNA_GAIN_DBI: f64 = 3.0;
/// The receiver sensitivity floor, dBm: the engine's default, a fielded OBU at
/// 6 Mbit/s (`SensitivityPreset::MeasuredObu` at `Mcs::R6Qpsk12`).
pub const ART_SENSITIVITY_DBM: f64 = -92.0;
/// The carrier the derivation uses, Hz: the engine's default channel (`ChannelId::CCH`).
pub const ART_CARRIER_HZ: f64 = 5.9e9;
/// The margin above the median reach, in shadowing standard deviations of the law: a
/// link this far out is decodable only on a 3σ shadowing upside (0.13 % one-sided).
pub const ART_MARGIN_SIGMAS: f64 = 3.0;

/// One row of the acceptance-range derivation: the line-of-sight reach of one of the
/// engine's tier-default propagation laws in one environment, metres.
///
/// The distance `d` solves `P_tx + 2·G − PL_LOS(d) + k·σ = S` ([`ART_TX_POWER_DBM`],
/// [`ART_ANTENNA_GAIN_DBI`], [`ART_MARGIN_SIGMAS`], [`ART_SENSITIVITY_DBM`]) with the
/// law's own line-of-sight σ. `tests/ts103759.rs` recomputes every row from
/// `v2xw-radio` and fails when one drifts by more than a metre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArtRow {
    /// The propagation model id.
    pub law: &'static str,
    /// The engine tier that selects it by default.
    pub tier: &'static str,
    /// The environment.
    pub environment: RadioEnvironment,
    /// The law's line-of-sight shadowing σ, dB.
    pub sigma_db: f64,
    /// The reach, metres (rounded to the metre).
    pub reach_m: f64,
}

const fn art_row(
    law: &'static str,
    tier: &'static str,
    environment: RadioEnvironment,
    sigma_db: f64,
    reach_m: f64,
) -> ArtRow {
    ArtRow {
        law,
        tier,
        environment,
        sigma_db,
        reach_m,
    }
}

/// The derivation, for every tier-default law and environment.
///
/// Free space has no shadowing. The log-distance rows are Abbas 2015's measured highway
/// and urban LOS fits (`LogDistancePreset::for_environment`: rural takes the highway row
/// and suburban the urban one). The TR 37.885 rows are its highway and urban LOS laws,
/// which the high tier's `propagation/v2v-urban-geometric` also uses in line of sight;
/// the urban law's 16.7 dB per decade reaches furthest of all.
pub const ART_DERIVATION: [ArtRow; 12] = {
    use RadioEnvironment::{Highway, Rural, Suburban, Urban};
    const FS: &str = "propagation/free-space";
    const LD: &str = "propagation/log-distance-shadowing";
    const TR: &str = "propagation/tr37885";
    [
        art_row(FS, "abstract", Highway, 0.0, 4_537.0),
        art_row(FS, "abstract", Rural, 0.0, 4_537.0),
        art_row(FS, "abstract", Suburban, 0.0, 4_537.0),
        art_row(FS, "abstract", Urban, 0.0, 4_537.0),
        art_row(LD, "medium", Highway, 3.95, 5_604.0),
        art_row(LD, "medium", Rural, 3.95, 5_604.0),
        art_row(LD, "medium", Suburban, 4.15, 6_479.0),
        art_row(LD, "medium", Urban, 4.15, 6_479.0),
        art_row(TR, "high", Highway, 3.0, 12_857.0),
        art_row(TR, "high", Rural, 3.0, 12_857.0),
        art_row(TR, "high", Suburban, 3.0, 41_955.0),
        art_row(TR, "high", Urban, 3.0, 41_955.0),
    ]
};

/// The acceptance range per environment, metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AcceptanceRanges {
    /// Highway.
    pub highway_m: f64,
    /// Rural.
    pub rural_m: f64,
    /// Suburban.
    pub suburban_m: f64,
    /// Urban.
    pub urban_m: f64,
}

impl AcceptanceRanges {
    /// The longest reach of [`ART_DERIVATION`] in each environment, so that an honest
    /// line-of-sight link is inside the range whichever tier the run uses.
    pub const DERIVED: AcceptanceRanges = AcceptanceRanges {
        highway_m: 12_857.0,
        rural_m: 12_857.0,
        suburban_m: 41_955.0,
        urban_m: 41_955.0,
    };

    /// The range for one environment.
    #[must_use]
    pub const fn get(&self, env: RadioEnvironment) -> f64 {
        match env {
            RadioEnvironment::Highway => self.highway_m,
            RadioEnvironment::Rural => self.rural_m,
            RadioEnvironment::Suburban => self.suburban_m,
            RadioEnvironment::Urban => self.urban_m,
        }
    }
}

/// The extra inputs the class-3, class-4 and class-5 checks need, beyond the message and
/// the node's own belief.
///
/// A struct rather than three more arguments, and every field optional-by-construction, so
/// that a host which has only some of them says so in the types instead of passing a
/// default that looks like a measurement.
pub struct CrossCheckInputs<'a> {
    /// This node's own perception. [`NoPerception`] is the honest answer for a node
    /// without sensors.
    pub perception: &'a dyn LocalPerception,
    /// The two envelope fields the message itself does not carry.
    pub envelope: Option<&'a EnvelopeExtras>,
}

impl CrossCheckInputs<'static> {
    /// The inputs a node with neither sensors nor recorded envelope extras has.
    ///
    /// Not "everything checks out": every check whose input is missing is absent from the
    /// verdict and counted as a skip.
    #[must_use]
    pub fn none() -> Self {
        CrossCheckInputs {
            perception: &NoPerception,
            envelope: None,
        }
    }
}

impl<'a> CrossCheckInputs<'a> {
    /// The inputs a node with perception but no recorded envelope extras has.
    #[must_use]
    pub fn with_perception(perception: &'a dyn LocalPerception) -> Self {
        Self {
            perception,
            envelope: None,
        }
    }
}

/// One claimed fix this receiver kept.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Fix {
    x: f64,
    y: f64,
    speed: f64,
    heading: f64,
    at: SimTime,
    generation_time: SimTime,
}

/// Per-subject state.
#[derive(Debug, Clone, PartialEq)]
struct SubjectState {
    last: Fix,
    first_seen: SimTime,
    streak: BTreeMap<Ts103759Check, u32>,
}

/// The TS 103 759 observation suite.
#[derive(Debug, Clone)]
pub struct Ts103759Suite {
    card: ModelCard,
    params: Ts103759Params,
    subjects: BTreeMap<[u8; 8], SubjectState>,
    /// The newest claim per signer, for the class-5 intersection check.
    recent: BTreeMap<[u8; 8], (f64, f64, SimTime)>,
    /// When this receiver started listening, on its own clock.
    listening_since: Option<SimTime>,
    deferred: u64,
    class4_evaluated: u64,
    class4_skipped_no_perception: u64,
    class4_skipped_outside_coverage: u64,
    skipped_no_envelope: u64,
    skipped_no_region: u64,
}

impl Ts103759Suite {
    /// The suite with the given thresholds.
    #[must_use]
    pub fn new(params: Ts103759Params) -> Self {
        Self {
            card: card(&params),
            params,
            subjects: BTreeMap::new(),
            recent: BTreeMap::new(),
            listening_since: None,
            deferred: 0,
            class4_evaluated: 0,
            class4_skipped_no_perception: 0,
            class4_skipped_outside_coverage: 0,
            skipped_no_envelope: 0,
            skipped_no_region: 0,
        }
    }

    /// The suite with the cited defaults of 04-models.md §14.
    #[must_use]
    pub fn cited_defaults() -> Self {
        Self::new(Ts103759Params::default())
    }

    /// The thresholds it reads.
    #[must_use]
    pub fn params(&self) -> &Ts103759Params {
        &self.params
    }

    /// How many messages were left unscored because this node's policy had not verified
    /// them.
    ///
    /// The number that matters most in this crate: under a verify-on-demand policy it is
    /// most of the traffic, and a suite that scored those messages as cryptographic
    /// failures produced a 98 % false-positive rate on honest traffic.
    #[must_use]
    pub fn deferred(&self) -> u64 {
        self.deferred
    }

    /// How many claims the class-4 cross-check actually evaluated.
    #[must_use]
    pub fn class4_evaluated(&self) -> u64 {
        self.class4_evaluated
    }

    /// How many claims class 4 skipped because this node has no perception.
    ///
    /// If this is the whole traffic, the class-4 recall of the run is meaningless, and
    /// this counter is how a reader knows.
    #[must_use]
    pub fn class4_skipped_no_perception(&self) -> u64 {
        self.class4_skipped_no_perception
    }

    /// How many claims class 4 skipped because they were outside the sensor's coverage.
    #[must_use]
    pub fn class4_skipped_outside_coverage(&self) -> u64 {
        self.class4_skipped_outside_coverage
    }

    /// How many messages the oversized-message check could not run on for want of a
    /// measured payload size.
    #[must_use]
    pub fn skipped_no_envelope(&self) -> u64 {
        self.skipped_no_envelope
    }

    /// How many messages the certificate-region check could not run on because the
    /// receiver's own region was not declared.
    #[must_use]
    pub fn skipped_no_region(&self) -> u64 {
        self.skipped_no_region
    }

    /// How many distinct subjects it is tracking.
    #[must_use]
    pub fn tracked_subjects(&self) -> usize {
        self.subjects.len()
    }

    /// When this receiver first heard `signer`, on its own clock.
    ///
    /// The sudden-appearance check's own input, and the quantity a privacy observer and a
    /// detector disagree about: a pseudonym change makes one vehicle two first sightings.
    #[must_use]
    pub fn first_seen(&self, signer: &[u8; 8]) -> Option<SimTime> {
        self.subjects.get(signer).map(|s| s.first_seen)
    }

    /// When this receiver started listening, on its own clock.
    #[must_use]
    pub fn listening_since(&self) -> Option<SimTime> {
        self.listening_since
    }

    /// The uncertainty gate for a claim of confidence `conf`: `z · max(conf, floor)`.
    fn gate(&self, conf_m: f64) -> f64 {
        self.params.z_threshold * conf_m.max(self.params.gate_floor_m)
    }

    /// The class-1 checks, which need nothing but the message.
    fn class1(
        &mut self,
        me: &SelfBelief,
        m: &ObservedMessage,
        x: &CrossCheckInputs<'_>,
        out: &mut TsVerdict,
    ) {
        let p = self.params.clone();
        let d = math::hypot(m.claimed_x_m - me.x_m, m.claimed_y_m - me.y_m);
        out.scores
            .insert(Ts103759Check::RangePlausibility, d / p.art_m());
        out.scores.insert(
            Ts103759Check::SpeedPlausibility,
            m.claimed_speed_mps.abs() / p.max_plausible_speed_mps,
        );
        out.scores.insert(
            Ts103759Check::ConfidencePlausibility,
            m.claimed_pos_confidence_m.max(0.0) / p.max_confidence_m,
        );
        match x.envelope.and_then(|e| e.payload_bytes) {
            Some(bytes) => {
                out.scores.insert(
                    Ts103759Check::OversizedMessage,
                    f64::from(bytes) / f64::from(p.max_payload_bytes),
                );
            }
            None => self.skipped_no_envelope += 1,
        }
    }

    /// The class-2 checks, which need the previous message from this station.
    fn class2(&self, prev: Fix, m: &ObservedMessage, t: SimTime, out: &mut TsVerdict) {
        let p = &self.params;
        let dt = ns_to_secs(t.saturating_sub(prev.at));
        let disp = math::hypot(m.claimed_x_m - prev.x, m.claimed_y_m - prev.y);
        let tol = self.gate(m.claimed_pos_confidence_m);

        // The generation time must advance. A repeat or a rewind is a replay, and it is a
        // hard finding rather than a scaled residual.
        if m.claimed_generation_time <= prev.generation_time {
            out.scores
                .insert(Ts103759Check::GenerationTimeOrder, p.hard_fail_score);
        } else {
            out.scores.insert(Ts103759Check::GenerationTimeOrder, 0.0);
        }

        // The beacon rate: fires when the gap is shorter than the conforming minimum.
        if dt > 0.0 {
            out.scores
                .insert(Ts103759Check::BeaconFrequency, p.min_interval_s() / dt);
        }

        // Everything else compares across an interval, and an interval longer than
        // MAX_TIME_DELTA is not a comparison: the station may legitimately have gone
        // anywhere. Absent, not zero.
        if dt <= 0.0 || dt > p.time_delta_s() {
            return;
        }
        out.scores.insert(
            Ts103759Check::PositionConsistency,
            disp / (p.max_plausible_speed_mps * dt + tol),
        );
        let avg_v = 0.5 * (m.claimed_speed_mps + prev.speed);
        out.scores.insert(
            Ts103759Check::PositionSpeedConsistency,
            (disp - avg_v * dt).abs() / (p.mgt_rng_m + tol),
        );
        let dv = m.claimed_speed_mps - prev.speed;
        let allowed = if dv >= 0.0 {
            p.mgt_rng_up_mps * dt
        } else {
            p.mgt_rng_down_mps * dt
        };
        out.scores.insert(
            Ts103759Check::SpeedConsistency,
            dv.abs() / allowed.max(1e-9),
        );
        out.scores.insert(
            Ts103759Check::AccelerationPlausibility,
            (dv / dt).abs()
                / if dv >= 0.0 {
                    p.max_plausible_accel_mps2
                } else {
                    p.max_plausible_decel_mps2
                },
        );
        // The heading check needs a short baseline and a displacement big enough for the
        // bearing to mean anything; otherwise it is measuring noise.
        if dt <= p.pos_heading_time_s && disp > tol {
            let bearing = bearing_rad(prev.x, prev.y, m.claimed_x_m, m.claimed_y_m);
            let off = angle_diff_rad(m.claimed_heading_rad, bearing).to_degrees();
            out.scores.insert(
                Ts103759Check::PositionHeadingConsistency,
                off / p.max_heading_change_deg,
            );
        }
    }

    /// The class-3 checks: the node's own map, and the region its own credentials say it
    /// is in.
    fn class3(
        &mut self,
        m: &ObservedMessage,
        env: &dyn LocalEnvironment,
        x: &CrossCheckInputs<'_>,
        out: &mut TsVerdict,
    ) {
        let p = self.params.clone();
        out.scores.insert(
            Ts103759Check::PositionPlausibility,
            env.distance_to_road_m(m.claimed_x_m, m.claimed_y_m) / p.max_distance_from_route_m,
        );
        match (x.envelope.and_then(|e| e.cert_region), p.own_region) {
            (Some(stated), Some(own)) => {
                let score = if stated == own {
                    0.0
                } else {
                    p.hard_fail_score
                };
                out.scores.insert(Ts103759Check::ForeignRegion, score);
            }
            _ => self.skipped_no_region += 1,
        }
    }

    /// The class-4 cross-check: the claim against this node's own sensors.
    ///
    /// Runs only for a claim inside the sensor's coverage. Outside it, the absence of a
    /// sensed object is the sensor's blind spot, and scoring it would accuse every vehicle
    /// behind a building.
    fn class4(
        &mut self,
        me: &SelfBelief,
        m: &ObservedMessage,
        x: &CrossCheckInputs<'_>,
        out: &mut TsVerdict,
    ) {
        if !x.perception.available() {
            self.class4_skipped_no_perception += 1;
            return;
        }
        if !x.perception.covers(me, m.claimed_x_m, m.claimed_y_m) {
            self.class4_skipped_outside_coverage += 1;
            return;
        }
        self.class4_evaluated += 1;
        let gate = self.gate(m.claimed_pos_confidence_m);
        let score = match x.perception.nearest_object_m(m.claimed_x_m, m.claimed_y_m) {
            // Nothing sensed anywhere, inside coverage: the strongest evidence this check
            // can produce, and it saturates rather than running to infinity.
            None => self.params.hard_fail_score,
            Some(d) => (d / gate.max(1e-9)).min(self.params.hard_fail_score),
        };
        out.scores
            .insert(Ts103759Check::PerceptionCrossCheck, score);
    }

    /// The class-5 checks: this message against what other stations said.
    fn class5(
        &mut self,
        me: &SelfBelief,
        m: &ObservedMessage,
        t: SimTime,
        x: &CrossCheckInputs<'_>,
        out: &mut TsVerdict,
    ) {
        let p = self.params.clone();
        let window = v2xw_core::time::secs_to_ns(p.max_delta_inter_s);
        let cutoff = t.saturating_sub(window);
        let mut nearest: Option<f64> = None;
        for (signer, (ox, oy, at)) in &self.recent {
            if *signer == m.signer || *at < cutoff {
                continue;
            }
            let d = math::hypot(*ox - m.claimed_x_m, *oy - m.claimed_y_m);
            if nearest.is_none_or(|b| d < b) {
                nearest = Some(d);
            }
        }
        if let Some(d) = nearest {
            out.scores.insert(
                Ts103759Check::Intersection,
                p.proximity_distance_m / d.max(1e-9),
            );
        }
        self.recent
            .insert(m.signer, (m.claimed_x_m, m.claimed_y_m, t));
        // A cell nobody has claimed inside the window is dead; the sweep is bounded by the
        // number of signers heard in one window.
        self.recent.retain(|_, (_, _, at)| *at >= cutoff);

        if let ObservedKind::Cpm(objects) = &m.kind {
            self.cpm_consistency(me, objects, &p, x, out);
        }
    }

    /// The CPM cross-check: the objects a sender claims to perceive, against the ones this
    /// node's own sensors hold, over the part of the region this node can see.
    fn cpm_consistency(
        &mut self,
        me: &SelfBelief,
        objects: &[PerceivedObject],
        p: &Ts103759Params,
        x: &CrossCheckInputs<'_>,
        out: &mut TsVerdict,
    ) {
        if !x.perception.available() {
            self.class4_skipped_no_perception += 1;
            return;
        }
        // A CPM carries no per-object confidence a receiver can read, so the gate is the
        // floor rather than a broadcast uncertainty.
        let gate = p.z_threshold * p.gate_floor_m;
        let mut visible = 0_u32;
        let mut unmatched = 0_u32;
        for o in objects {
            // Only objects THIS node can see are evidence. An object the sender reports
            // from beyond our range, outside our field of view or behind a building is
            // exactly what collective perception is for, and counting it as
            // uncorroborated would accuse every honest CPM in the run.
            if !x.perception.covers(me, o.x_m(), o.y_m()) {
                continue;
            }
            visible += 1;
            match x.perception.nearest_object_m(o.x_m(), o.y_m()) {
                None => unmatched += 1,
                Some(d) => {
                    if d > gate {
                        unmatched += 1;
                    }
                }
            }
        }
        if visible == 0 {
            return;
        }
        let fraction = f64::from(unmatched) / f64::from(visible);
        out.scores.insert(
            Ts103759Check::CpmConsistency,
            fraction / p.cpm_unmatched_fraction.max(1e-9),
        );
    }

    /// Runs the suite on one message.
    ///
    /// The entry point with the cross-check inputs. [`Ts103759Suite::on_message`] is the
    /// same call with [`CrossCheckInputs::none`], for a host that has neither perception
    /// nor recorded envelope extras — and it is *counted* as unchecked rather than passed.
    pub fn check(
        &mut self,
        ctx: &mut dyn ThreatCtx,
        me: &SelfBelief,
        m: &ObservedMessage,
        env: &dyn LocalEnvironment,
        x: &CrossCheckInputs<'_>,
    ) -> TsVerdict {
        // The receiver's own clock at reception, never the simulator's.
        let t = m.received_at;
        if self.listening_since.is_none() {
            self.listening_since = Some(t);
        }
        let mut out = TsVerdict {
            subject: m.signer_hex(),
            ..TsVerdict::default()
        };

        // The four verification states mean four different things. No catch-all arm.
        match m.verification {
            VerificationState::Unverified => {
                // NOT CHECKED, because this node's policy deferred it. Nothing is scored,
                // nothing fires, and the count is what a reader needs to interpret every
                // other number in the run.
                self.deferred += 1;
                return out;
            }
            VerificationState::BadSignature => {
                out.scores
                    .insert(Ts103759Check::SignatureInvalid, self.params.hard_fail_score);
                self.fire(m, t, &mut out);
                emit(ctx, me.node, &out);
                return out;
            }
            VerificationState::UnknownCertificate => {
                out.scores.insert(
                    Ts103759Check::CertificateUnknown,
                    self.params.hard_fail_score,
                );
                self.fire(m, t, &mut out);
                emit(ctx, me.node, &out);
                return out;
            }
            VerificationState::Valid => {}
        }

        // Verified: the content is evidence, so every class that has its inputs runs.
        self.class1(me, m, x, &mut out);
        let known = self.subjects.get(&m.signer).map(|s| s.last);
        match known {
            Some(prev) => self.class2(prev, m, t, &mut out),
            None => {
                // A first sighting has no previous message, so class 2 does not run — with
                // one exception, which is the check that exists *because* it is a first
                // sighting.
                let d = math::hypot(m.claimed_x_m - me.x_m, m.claimed_y_m - me.y_m);
                let listening = ns_to_secs(t.saturating_sub(self.listening_since.unwrap_or(t)));
                if listening > self.params.sa_time_s() && d < self.params.max_sa_range_m {
                    out.scores.insert(
                        Ts103759Check::SuddenAppearance,
                        self.params.sudden_appearance_m / d.max(1e-9),
                    );
                }
            }
        }
        self.class3(m, env, x, &mut out);
        self.class4(me, m, x, &mut out);
        self.class5(me, m, t, x, &mut out);

        self.fire(m, t, &mut out);
        // Keep this claim as the previous one for the next message from this station.
        let fix = Fix {
            x: m.claimed_x_m,
            y: m.claimed_y_m,
            speed: m.claimed_speed_mps,
            heading: m.claimed_heading_rad,
            at: t,
            generation_time: m.claimed_generation_time,
        };
        // `entry` rather than `get_mut` + `insert`: the borrow checker rejects the second
        // form, and the entry form is also the one that cannot forget the `first_seen` a
        // fresh subject needs.
        self.subjects
            .entry(m.signer)
            .and_modify(|st| st.last = fix)
            .or_insert_with(|| SubjectState {
                last: fix,
                first_seen: t,
                streak: BTreeMap::new(),
            });
        emit(ctx, me.node, &out);
        out
    }

    /// The same call for a host with neither perception nor envelope extras.
    pub fn on_message(
        &mut self,
        ctx: &mut dyn ThreatCtx,
        me: &SelfBelief,
        m: &ObservedMessage,
        env: &dyn LocalEnvironment,
    ) -> TsVerdict {
        let x = CrossCheckInputs::none();
        self.check(ctx, me, m, env, &x)
    }

    /// What one call costs the node's CPU.
    ///
    /// Not measured: declared on the card as a placeholder, as the legacy suite's is.
    #[must_use]
    pub fn cost(&self) -> DetectorCost {
        DetectorCost {
            per_message_us: 25.0,
        }
    }

    /// Applies the streak gate and orders the fired set, highest score first.
    ///
    /// The gate is the legacy suite's: a check fires only after `min_consecutive` messages
    /// in a row from the same signer scored at or above 1. A single GNSS outlier is one
    /// sample; a real kinematic attack persists. A check that did not *run* does not reset
    /// the streak either, because an absent input is not a passing check.
    fn fire(&mut self, m: &ObservedMessage, t: SimTime, out: &mut TsVerdict) {
        let min = self.params.min_consecutive;
        let subject = out.subject.clone();
        let entry = self
            .subjects
            .entry(m.signer)
            .or_insert_with(|| SubjectState {
                last: Fix {
                    x: m.claimed_x_m,
                    y: m.claimed_y_m,
                    speed: m.claimed_speed_mps,
                    heading: m.claimed_heading_rad,
                    at: t,
                    generation_time: m.claimed_generation_time,
                },
                first_seen: t,
                streak: BTreeMap::new(),
            });
        let mut fired: Vec<TsObservation> = Vec::new();
        for (check, score) in &out.scores {
            let run = entry.streak.entry(*check).or_insert(0);
            *run = if *score >= 1.0 { *run + 1 } else { 0 };
            if *run >= min {
                fired.push(TsObservation {
                    check: *check,
                    score: *score,
                    subject: subject.clone(),
                    at: t,
                });
            }
        }
        fired.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(core::cmp::Ordering::Equal)
                .then(a.check.cmp(&b.check))
        });
        out.fired = fired;
    }
}

impl Model for Ts103759Suite {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

/// Writes a `det.observation` for each fired check.
fn emit(ctx: &mut dyn ThreatCtx, node: v2xw_core::ids::NodeId, v: &TsVerdict) {
    for o in &v.fired {
        ctx.emit(DetObservation::new(
            o.at,
            node,
            o.check.as_str(),
            &o.subject,
            o.score,
        ));
    }
}

/// A card row for an acceptance range, with the derivation's per-law reaches in its note.
fn art_param(name: &str, value: f64, env: RadioEnvironment, in_force: bool) -> Parameter {
    use serde_json::json;
    let rows: Vec<String> = ART_DERIVATION
        .iter()
        .filter(|r| r.environment == env)
        .map(|r| format!("{} ({}) {} m", r.law, r.tier, r.reach_m))
        .collect();
    let what = if in_force {
        format!(
            "the range in force: max_plausible_range_m when set, else art_{}_m",
            env.label()
        )
    } else {
        format!("the {} acceptance range", env.label())
    };
    let mut q = Parameter::new(
        name,
        "m",
        json!(value),
        Source {
            kind: SourceKind::Code,
            reference: "crates/v2xw-radio link budget (prop::friis_loss_db, \
                        LogDistancePreset::for_environment, prop::tr37885_*_los_db; \
                        phy::SensitivityPreset::MeasuredObu at Mcs::R6Qpsk12), \
                        docs/design/frameworks/f2md.md §5 P9"
                .to_string(),
            accessed: Some(crate::cards::LEGACY_ACCESSED.to_string()),
            note: Some(format!(
                "{what}; 23 dBm, 3 dBi at each end, −92 dBm, 3σ margin, 5.9 GHz; per law in \
                 {}: {}. Replaces F2MD's 420 m (Veins at 20 mW).",
                env.label(),
                rows.join(", ")
            )),
        },
    );
    q.range = Some(vec![json!(0.0), json!(1.0e6)]);
    q
}

/// A card row for a threshold derived from the beacon profile.
fn derived_param(name: &str, value: f64, rule: &str) -> Parameter {
    use serde_json::json;
    Parameter::new(
        name,
        "s",
        json!(value),
        Source {
            kind: SourceKind::Code,
            reference: "docs/design/frameworks/f2md.md §5 P1 (beacon timing from the \
                        message profile)"
                .to_string(),
            accessed: Some(crate::cards::LEGACY_ACCESSED.to_string()),
            note: Some(format!("{rule}; set explicitly to override")),
        },
    )
}

/// The model card for the suite.
#[must_use]
pub fn card(p: &Ts103759Params) -> ModelCard {
    use serde_json::json;
    let f2md = design("04-models.md §14 (detector/f2md-checks, veins-f2md F2MDParameters.h)");
    let ts = standard("ETSI TS 103 759 V2.2.1 (2026-01), misbehaviour observation classes 1–5");
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Detector,
        "1.0.0",
        "The ETSI TS 103 759 observation classes 1–5 with F2MD-style check semantics and \
         the perception cross-check, normalised so that a score of ≈ 1 is the firing \
         threshold. Runs alongside the legacy twelve, not instead of them.",
    );
    card.tier = vec![Tier::Medium, Tier::High];
    card.equations = vec![
        Equation::new("range plausibility", "|claim − self| / ART(environment)"),
        Equation::new(
            "acceptance range (ART)",
            "ART(env) = max over the tier-default LOS laws L of d solving \
             P_tx + 2·G − PL_L(d) + 3·σ_L = S, with P_tx 23 dBm, G 3 dBi, S −92 dBm \
             (ART_DERIVATION, recomputed from v2xw-radio by tests/ts103759.rs)",
        ),
        Equation::new("speed plausibility", "|v_claimed| / max_plausible_speed"),
        Equation::new(
            "acceleration plausibility",
            "|Δv/Δt| / (max_accel if Δv ≥ 0 else max_decel)",
        ),
        Equation::new(
            "position consistency",
            "Δs / (max_plausible_speed·Δt + Z·max(conf, floor))",
        ),
        Equation::new(
            "position/speed consistency",
            "|Δs − v̄·Δt| / (MAX_MGT_RNG + Z·max(conf, floor))",
        ),
        Equation::new(
            "speed consistency",
            "|Δv| / (MAX_MGT_RNG_UP·Δt if Δv ≥ 0 else MAX_MGT_RNG_DOWN·Δt)",
        ),
        Equation::new(
            "position/heading consistency",
            "angle(heading_claimed, bearing(prev → now)) / MAX_HEADING_CHANGE, for \
             Δt ≤ POS_HEADING_TIME and Δs > Z·max(conf, floor)",
        ),
        Equation::new(
            "beacon frequency",
            "(T_min − jitter_tolerance) / Δt_rx, from the message profile (BSM: \
             (100 − 25) ms) — fires when the gap is shorter than the conforming minimum",
        ),
        Equation::new(
            "interval windows",
            "MAX_TIME_DELTA = 3.1·T_max, MAX_SA_TIME = 2.1·T_max, T_max the profile's \
             longest conforming interval (F2MD's 3.1 s and 2.1 s at 1 Hz)",
        ),
        Equation::new(
            "sudden appearance",
            "on a first sighting after MAX_SA_TIME of listening and inside MAX_SA_RANGE: \
             sudden_appearance_m / |claim − self|",
        ),
        Equation::new(
            "map position plausibility",
            "distance to nearest known road / MAX_DISTANCE_FROM_ROUTE",
        ),
        Equation::new(
            "perception cross-check (class 4)",
            "for a claim inside the sensor's coverage: \
             min(d(claim, nearest sensed object) / (Z·max(conf, floor)), hard_fail); \
             saturates at hard_fail when nothing is sensed at all",
        ),
        Equation::new(
            "intersection (class 5)",
            "MAX_PROXIMITY_DISTANCE / d(claim, nearest other station's claim in \
             MAX_DELTA_INTER)",
        ),
        Equation::new(
            "CPM consistency (class 5)",
            "(uncorroborated visible objects / visible objects) / cpm_unmatched_fraction",
        ),
    ];
    card.parameters = vec![
        art_param("max_plausible_range_m", p.art_m(), p.environment, true),
        {
            let mut q = Parameter::new(
                "environment",
                "-",
                json!(p.environment.label()),
                design("frameworks/f2md.md §5 P9 (ART per environment)"),
            );
            q.range = Some(
                RadioEnvironment::ALL
                    .iter()
                    .map(|e| json!(e.label()))
                    .collect(),
            );
            q
        },
        art_param(
            "art_highway_m",
            p.acceptance_ranges.highway_m,
            RadioEnvironment::Highway,
            false,
        ),
        art_param(
            "art_rural_m",
            p.acceptance_ranges.rural_m,
            RadioEnvironment::Rural,
            false,
        ),
        art_param(
            "art_suburban_m",
            p.acceptance_ranges.suburban_m,
            RadioEnvironment::Suburban,
            false,
        ),
        art_param(
            "art_urban_m",
            p.acceptance_ranges.urban_m,
            RadioEnvironment::Urban,
            false,
        ),
        Parameter::new(
            "max_plausible_speed_mps",
            "m/s",
            json!(p.max_plausible_speed_mps),
            f2md.clone(),
        ),
        Parameter::new(
            "max_plausible_accel_mps2",
            "m/s^2",
            json!(p.max_plausible_accel_mps2),
            f2md.clone(),
        ),
        Parameter::new(
            "max_plausible_decel_mps2",
            "m/s^2",
            json!(p.max_plausible_decel_mps2),
            f2md.clone(),
        ),
        Parameter::new(
            "max_confidence_m",
            "m",
            json!(p.max_confidence_m),
            f2md.clone(),
        ),
        Parameter::new(
            "max_payload_bytes",
            "B",
            json!(p.max_payload_bytes),
            design("04-models.md §4.6 (maximum MSDU 2 304 B)"),
        ),
        {
            let mut q = Parameter::new(
                "beacon_profile",
                "-",
                json!(p.beacon_profile.id),
                standard(
                    "SAE J2945/1 (BSM 100 ms, vMinITT 100 ms, vMaxITT 600 ms; \
                     04-models.md §8.1) and EN 302 637-2 V1.4.1 §6.1.3 (T_GenCamMin \
                     100 ms, T_GenCamMax 1 s)",
                ),
            );
            q.range = Some(BeaconProfile::ALL.iter().map(|b| json!(b.id)).collect());
            q
        },
        Parameter::new(
            "beacon_min_interval_s",
            "s",
            json!(p.beacon_profile.min_interval_s),
            standard("SAE J2945/1 vMinITT / EN 302 637-2 V1.4.1 §6.1.3 T_GenCamMin (100 ms)"),
        ),
        {
            let mut q = Parameter::new(
                "beacon_max_interval_s",
                "s",
                json!(p.beacon_profile.max_interval_s),
                standard(
                    "SAE J2945/1 vMaxITT (600 ms, UNVERIFIED in 04-models.md §8.1) / \
                     EN 302 637-2 V1.4.1 §6.1.3 T_GenCamMax (1 s)",
                ),
            );
            q.calibration = Some(
                "confirm vMaxITT against the J2945/1 text once it is cached; the windows \
                 scale with it."
                    .to_string(),
            );
            q
        },
        {
            let mut q = Parameter::new(
                "beacon_jitter_tolerance_s",
                "s",
                json!(p.beacon_profile.jitter_tolerance_s),
                Source {
                    kind: SourceKind::TodoCalibrate,
                    reference: "v2xw_msg::GenerationTiming (10 ms hand-off jitter, ns-3 \
                                BsmApplication TxMaxDelay) plus a 15 ms channel-access \
                                allowance"
                        .to_string(),
                    accessed: Some(crate::cards::LEGACY_ACCESSED.to_string()),
                    note: Some("the F2MD 1 Hz profile's 0.1 s is F2MD's own 1 − 0.9 s".to_string()),
                },
            );
            q.calibration = Some(
                "measure the honest BSM inter-arrival distribution at the receiver in a \
                 dense benign run (CBR near the DCC thresholds) and set the tolerance so \
                 that fewer than 1 in 10^4 consecutive gap pairs fall under the minimum."
                    .to_string(),
            );
            q
        },
        derived_param(
            "max_time_delta_s",
            p.time_delta_s(),
            "MAX_TIME_DELTA_INTERVALS (3.1, F2MD MAX_TIME_DELTA at 1 Hz) × \
             beacon_max_interval_s",
        ),
        Parameter::new("mgt_rng_m", "m", json!(p.mgt_rng_m), f2md.clone()),
        Parameter::new(
            "mgt_rng_up_mps",
            "m/s per s",
            json!(p.mgt_rng_up_mps),
            f2md.clone(),
        ),
        Parameter::new(
            "mgt_rng_down_mps",
            "m/s per s",
            json!(p.mgt_rng_down_mps),
            f2md.clone(),
        ),
        Parameter::new(
            "max_heading_change_deg",
            "deg",
            json!(p.max_heading_change_deg),
            f2md.clone(),
        ),
        Parameter::new(
            "pos_heading_time_s",
            "s",
            json!(p.pos_heading_time_s),
            f2md.clone(),
        ),
        derived_param(
            "min_beacon_interval_s",
            p.min_interval_s(),
            "beacon_min_interval_s − beacon_jitter_tolerance_s (F2MD MAX_BEACON_FREQUENCY \
             0.9 s is the same rule at 1 Hz)",
        ),
        derived_param(
            "max_sa_time_s",
            p.sa_time_s(),
            "MAX_SA_TIME_INTERVALS (2.1, F2MD MAX_SA_TIME at 1 Hz) × beacon_max_interval_s",
        ),
        Parameter::new("max_sa_range_m", "m", json!(p.max_sa_range_m), f2md.clone()),
        {
            let mut q = Parameter::new(
                "sudden_appearance_m",
                "m",
                json!(p.sudden_appearance_m),
                Source {
                    kind: SourceKind::TodoCalibrate,
                    reference: "04-models.md §14 lists MAX_SA_RANGE and MAX_SA_TIME for \
                                SuddenAppearence but not the predicate"
                        .to_string(),
                    accessed: Some(crate::cards::LEGACY_ACCESSED.to_string()),
                    note: Some(
                        "the default is the F2MD longitudinal proximity box \
                         (MAX_PROXIMITY_RANGE_L = 30 m), which is a stand-in for the \
                         distance inside which a first sighting is implausible"
                            .to_string(),
                    ),
                },
            );
            q.calibration = Some(
                "re-implement the published description of F2MD's sudden-appearance check \
                 (behaviour only: the F2MD source is GPL and all rights reserved, and is \
                 not to be copied) and replace this with it; failing that, measure the \
                 distribution of first-sighting distances in the benign reference run and \
                 set the threshold below its lower tail."
                    .to_string(),
            );
            q
        },
        Parameter::new(
            "max_distance_from_route_m",
            "m",
            json!(p.max_distance_from_route_m),
            f2md.clone(),
        ),
        Parameter::new(
            "proximity_distance_m",
            "m",
            json!(p.proximity_distance_m),
            f2md.clone(),
        ),
        Parameter::new(
            "proximity_range_l_m",
            "m",
            json!(p.proximity_range_l_m),
            f2md.clone(),
        ),
        Parameter::new("max_delta_inter_s", "s", json!(p.max_delta_inter_s), f2md),
        legacy_param(
            "z_threshold",
            "-",
            json!(p.z_threshold),
            LEGACY_PY,
            "PipelineConfig.detector_z_threshold",
        ),
        legacy_param(
            "gate_floor_m",
            "m",
            json!(p.gate_floor_m),
            LEGACY_PY,
            "PipelineConfig.consistency_threshold_m",
        ),
        legacy_param(
            "hard_fail_score",
            "-",
            json!(p.hard_fail_score),
            LEGACY_PY,
            "the detection pass (1.5)",
        ),
        legacy_param(
            "min_consecutive",
            "-",
            json!(p.min_consecutive),
            LEGACY_PY,
            "PipelineConfig.detector_min_consec",
        ),
        {
            let mut q = Parameter::new(
                "cpm_unmatched_fraction",
                "-",
                json!(p.cpm_unmatched_fraction),
                Source {
                    kind: SourceKind::TodoCalibrate,
                    reference: "07-threats-and-detection.md §3.1 requires CPM consistency and \
                                gives no threshold"
                        .to_string(),
                    accessed: Some(crate::cards::LEGACY_ACCESSED.to_string()),
                    note: Some(
                        "1.0 fires only when none of a CPM's visible objects is corroborated, \
                         which is the one fraction that is a structural statement rather than \
                         a chosen number"
                            .to_string(),
                    ),
                },
            );
            q.range = Some(vec![json!(0.0), json!(1.0)]);
            q.calibration = Some(
                "measure the corroboration rate of benign CPMs against the perception model \
                 of 04-models.md §12.1 at each scenario density, and set the fraction above \
                 the benign tail; the perception model's own detection-probability curve is \
                 itself uncalibrated (§12.1), so this cannot be settled before that one is."
                    .to_string(),
            );
            q
        },
    ];
    card.sources = vec![
        ts.clone(),
        design("07-threats-and-detection.md §3.1 (new detector families)"),
        design("04-models.md §14 (detector/ts103759-observations, detector/f2md-checks)"),
        design("04-models.md §12.1 (the perception model the class-4 check reads)"),
        standard("ETSI TR 103 460 (misbehaviour-detection taxonomy: ART, eART, CoE, MPP, SAW)"),
        standard("ETSI TS 103 324 (collective perception, the class-5 CPM check's subject)"),
    ];
    card.assumptions = vec![
        "Every input is belief: the node's own position estimate, its own clock, its own \
         map, its own sensors and the messages it verified (invariant I-T2)."
            .to_string(),
        "A score of ≈ 1 is the firing threshold for every check, as in the legacy suite, \
         so the two suites' fingerprints are comparable check by check."
            .to_string(),
        "A check whose input this receiver does not have is absent from the verdict, not \
         scored zero, and the skip is counted."
            .to_string(),
        "The four verification states are handled separately: a deferred verification \
         scores nothing at all."
            .to_string(),
    ];
    card.limitations = vec![
        "The TS's DENM-specific observations (the 500 ms EEBL plausibility window, the \
         detection-time consistency at 80 % of the minimum threshold, the same-event \
         correlation distances of 1 km and 100 m, 04-models.md §14) are not implemented \
         here: they need the DENM event fields, which ObservedKind carries only as a cause \
         code. The legacy suite's denmPlausibility check covers the speed contradiction."
            .to_string(),
        "The TS's event-trust threshold (ETR ≥ TrustThreshold) is left to the deployment, \
         as the TS does; 04-models.md §14 records it as todo-calibrate."
            .to_string(),
        "Class 3 reads the map and the certificate region. Signal state is part of the \
         class-3 LDM in the TS and does not reach a detector here: SPaT is encoded \
         (v2xw_msg::j2735::spat) and broadcast (v2xw_node::rsu), but crate::obs carries no \
         signal-state belief, so a false SPaT from a compromised road-side unit \
         (crate::attack_rsu) cannot be cross-checked. Adding it means a LocalSignalState \
         trait beside LocalEnvironment, on the same declared-capability footing."
            .to_string(),
        "The class-4 cross-check needs a perception model. With NoPerception it runs on \
         nothing, and class4_skipped_no_perception is the count that says so; a run that \
         reports a class-4 recall without reading that counter is reporting nothing."
            .to_string(),
        "The F2MD checks are re-implemented from the constant table of 04-models.md §14 \
         and the published descriptions; no F2MD source was used. The scores are \
         normalised on this crate's convention rather than F2MD's own, so a threshold \
         matches but a score does not compare digit for digit with an F2MD run."
            .to_string(),
        "The acceptance range is the line-of-sight reach of the longest-reaching \
         tier-default law. In urban and suburban areas that is TR 37.885's urban LOS law, \
         whose 16.7 dB per decade puts the range near 42 km, so range plausibility is close \
         to inert there: in a city the obstacle stack, not the range, ends a link. The \
         range includes no fast-fading upside (a Nakagami up-fade at the high tier), and it \
         is taken at 23 dBm; a fleet radiating the US 33 dBm EIRP ceiling reaches about \
         7 dB further and needs art_*_m raised."
            .to_string(),
        "The beacon-timing thresholds assume every sender in range uses the receiver's \
         beacon_profile; a mixed BSM and CAM neighbourhood shares the 100 ms minimum but \
         not the longest interval."
            .to_string(),
        "The per-message CPU cost is a placeholder, not a measurement.".to_string(),
    ];
    card.determinism = Determinism {
        uses_rng: false,
        rng_domains: Vec::new(),
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: vec![ts],
        tests: vec![
            "ts103759::a_deferred_verification_scores_nothing_at_all".to_string(),
            "ts103759::the_four_verification_states_produce_four_outcomes".to_string(),
            "ts103759::the_perception_cross_check_catches_a_ghost_inside_the_field_of_view"
                .to_string(),
            "ts103759::a_claim_outside_the_sensor_coverage_is_unchecked_not_passed".to_string(),
            "ts103759::benign_traffic_stays_quiet".to_string(),
            "ts103759::an_honest_10_hz_bsm_sender_raises_no_beacon_frequency_event".to_string(),
            "ts103759::a_20_hz_sender_raises_beacon_frequency".to_string(),
            "ts103759::the_1_hz_profile_reproduces_f2md".to_string(),
            "ts103759::the_acceptance_range_is_the_engine_link_budget".to_string(),
            "ts103759::an_honest_los_highway_link_never_fires_and_twice_the_range_does".to_string(),
        ],
    };
    card.cost = Some(v2xw_core::card::CostClass {
        per_call_us: Some(25.0),
        notes: Some(
            "Placeholder: nineteen scalar checks, one map query and one nearest-object \
             query per message. Not measured."
                .to_string(),
        ),
    });
    card
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_check_parses_and_belongs_to_exactly_one_class() {
        assert_eq!(Ts103759Check::ALL.len(), 19);
        for c in Ts103759Check::ALL {
            assert_eq!(Ts103759Check::parse(c.as_str()), Some(c));
        }
        assert_eq!(Ts103759Check::parse("positionJump"), None);
        let mut total = 0;
        for class in ObservationClass::ALL {
            total += Ts103759Check::in_class(class).len();
        }
        assert_eq!(total, Ts103759Check::ALL.len());
        // The five observation classes are 1..=5, and the envelope group is not one.
        assert_eq!(ObservationClass::ImplausibleValues.number(), 1);
        assert_eq!(ObservationClass::OtherStations.number(), 5);
        assert_eq!(ObservationClass::Envelope.number(), 0);
    }

    #[test]
    fn each_class_has_at_least_one_check() {
        for class in ObservationClass::ALL {
            assert!(
                !Ts103759Check::in_class(class).is_empty(),
                "class {} has no check",
                class.number()
            );
        }
    }

    #[test]
    fn the_card_validates_and_every_default_is_cited_or_planned() {
        let c = card(&Ts103759Params::default());
        c.validate().unwrap();
        c.check_api_version().unwrap();
        for p in &c.parameters {
            let cited = p.source.kind != SourceKind::TodoCalibrate;
            let planned = p.calibration.as_ref().is_some_and(|s| !s.trim().is_empty());
            assert!(cited || planned, "{}", p.name);
        }
        // Every threshold the suite reads is declared (invariant I-C3).
        for name in [
            "max_plausible_range_m",
            "max_plausible_speed_mps",
            "max_plausible_accel_mps2",
            "max_plausible_decel_mps2",
            "max_confidence_m",
            "max_payload_bytes",
            "max_time_delta_s",
            "mgt_rng_m",
            "mgt_rng_up_mps",
            "mgt_rng_down_mps",
            "max_heading_change_deg",
            "pos_heading_time_s",
            "min_beacon_interval_s",
            "max_sa_time_s",
            "max_sa_range_m",
            "sudden_appearance_m",
            "max_distance_from_route_m",
            "proximity_distance_m",
            "proximity_range_l_m",
            "max_delta_inter_s",
            "z_threshold",
            "gate_floor_m",
            "hard_fail_score",
            "min_consecutive",
            "cpm_unmatched_fraction",
            "environment",
            "art_highway_m",
            "art_rural_m",
            "art_suburban_m",
            "art_urban_m",
            "beacon_profile",
            "beacon_min_interval_s",
            "beacon_max_interval_s",
            "beacon_jitter_tolerance_s",
        ] {
            assert!(
                c.parameters.iter().any(|p| p.name == name),
                "undeclared parameter {name}"
            );
        }
    }

    #[test]
    fn the_cited_defaults_are_the_numbers_04_models_gives() {
        let p = Ts103759Params::default();
        // The acceptance range is this engine's own (f2md.md §5 P9), not Veins' 420 m.
        assert_eq!(p.max_plausible_range_m, None);
        assert_eq!(p.environment, RadioEnvironment::Highway);
        assert_eq!(p.art_m(), AcceptanceRanges::DERIVED.highway_m);
        assert_eq!(p.max_plausible_speed_mps, 40.0);
        assert_eq!(p.max_plausible_accel_mps2, 3.0);
        assert_eq!(p.max_plausible_decel_mps2, 4.5);
        assert_eq!(p.max_confidence_m, 10.0);
        // The beacon timing is the BSM's (f2md.md §5 P1); the 1 Hz profile gives
        // F2MD's 3.1 / 0.9 / 2.1 s (tests/ts103759.rs::the_1_hz_profile_reproduces_f2md).
        assert_eq!(p.beacon_profile, BeaconProfile::BSM_J2945_1);
        assert!((p.time_delta_s() - 1.86).abs() < 1e-12);
        assert_eq!(p.mgt_rng_m, 4.0);
        assert_eq!(p.mgt_rng_up_mps, 2.1);
        assert_eq!(p.mgt_rng_down_mps, 6.2);
        assert_eq!(p.max_heading_change_deg, 90.0);
        assert_eq!(p.pos_heading_time_s, 1.1);
        assert!((p.min_interval_s() - 0.075).abs() < 1e-12);
        assert!((p.sa_time_s() - 1.26).abs() < 1e-12);
        assert_eq!(p.max_sa_range_m, 420.0);
        assert_eq!(p.max_distance_from_route_m, 2.0);
        assert_eq!(p.proximity_distance_m, 2.0);
        assert_eq!(p.proximity_range_l_m, 30.0);
        assert_eq!(p.max_delta_inter_s, 2.0);
        assert_eq!(p.max_payload_bytes, 2_304);
        assert_eq!(p.own_region, None);
    }

    #[test]
    fn every_settings_key_is_on_the_card_and_is_read() {
        let c = card(&Ts103759Params::default());
        for key in PARAM_KEYS {
            assert!(
                c.parameters.iter().any(|p| p.name == key),
                "{key} is a setting the card does not declare"
            );
            let mut p = Ts103759Params::default();
            assert!(p.apply(key, 7.0), "{key} is listed but not read");
            assert_ne!(p, Ts103759Params::default(), "{key} changed nothing");
        }
        assert!(!Ts103759Params::default().apply("maxPlausibleRange", 1.0));
    }

    #[test]
    fn an_explicit_threshold_wins_over_the_derivation_in_either_order() {
        let mut a = Ts103759Params::default();
        a.apply("max_plausible_range_m", 900.0);
        a.apply("art_highway_m", 3_000.0);
        let mut b = Ts103759Params::default();
        b.apply("art_highway_m", 3_000.0);
        b.apply("max_plausible_range_m", 900.0);
        assert_eq!(a, b);
        assert_eq!(a.art_m(), 900.0);
        let mut c = Ts103759Params::default().with_environment(RadioEnvironment::Urban);
        c.apply("art_urban_m", 3_000.0);
        assert_eq!(c.art_m(), 3_000.0);
        let mut d = Ts103759Params::default();
        d.apply("beacon_jitter_tolerance_s", 0.05);
        assert!((d.min_interval_s() - 0.05).abs() < 1e-12);
        d.apply("min_beacon_interval_s", 0.08);
        assert_eq!(d.min_interval_s(), 0.08);
    }

    #[test]
    fn the_acceptance_range_per_environment_is_the_longest_derived_reach() {
        for env in RadioEnvironment::ALL {
            let longest = ART_DERIVATION
                .iter()
                .filter(|r| r.environment == env)
                .map(|r| r.reach_m)
                .fold(0.0, f64::max);
            assert_eq!(
                AcceptanceRanges::DERIVED.get(env),
                longest,
                "{}",
                env.label()
            );
            assert_eq!(RadioEnvironment::parse(env.label()), Some(env));
        }
        for b in BeaconProfile::ALL {
            assert_eq!(BeaconProfile::parse(b.id), Some(b));
        }
    }

    #[test]
    fn an_unevaluated_check_is_absent_rather_than_zero() {
        let v = TsVerdict::default();
        assert_eq!(v.score(Ts103759Check::PerceptionCrossCheck), None);
        assert!(!v.evaluated(Ts103759Check::PerceptionCrossCheck));
        let mut v2 = TsVerdict::default();
        v2.scores.insert(Ts103759Check::PerceptionCrossCheck, 0.0);
        assert_eq!(v2.score(Ts103759Check::PerceptionCrossCheck), Some(0.0));
        assert!(v2.evaluated(Ts103759Check::PerceptionCrossCheck));
    }
}
