//! The misbehaviour-report records behind `security.scms.report_records` (off by
//! default): what became of every report on its device ([SM-MBR] §2.2 store), what each
//! Misbehaviour Authority received, and each device's exposure, so the store's crowd-out
//! (paper plan P1-E6) and an offline MA-rule replay (P1-E7, `v2xw ma-replay`) can be
//! measured from a finished run without re-simulating it.
//!
//! The three channels keep the firewall between what the MA knows and what is true:
//!
//! * `ma.received` (NODE) carries only fields the MA reads from the report it received
//!   or can obtain through the protocol: the reporter's and the accused's pseudonym
//!   certificate digests, the subtypes, the evidence's time window, the accused's
//!   broadcast positional accuracy, the secMark phase of the evidence, the issuing
//!   provider of the accused certificate, and `la_link`, an opaque token equal for two
//!   reports exactly when the Linkage Authorities would answer "same device" ([BRECHT]
//!   §VI-B/C: the MA, the PCA and one LA can link). No node id, no attacker label.
//! * `pki.report_fate` (NODE_AND_GT) is the device's own store log with the ground-truth
//!   join of the accused beside it, for the P1-E6 accounting.
//! * `gt.device_exposure` (GT) is each device's time in the run and its attack window:
//!   the scoring step's denominator and the attacker's start, joined only after the rules
//!   have decided.

use serde::{Deserialize, Serialize};
use v2xw_core::ctx::{Record, Visibility};
use v2xw_core::ids::NodeId;
use v2xw_core::time::SimTime;

/// A report's fate on its device or on the way to the MA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fate {
    /// Generated and put in the device's store ([SM-MBR] §2.2).
    Stored,
    /// Deleted by the quota at its initial weight.
    EvictedQuota,
    /// Deleted by the quota after the daily multiplier had lowered its weight: it lost
    /// its place to decay.
    DecayedOut,
    /// Deleted unsent after the maximum age (one week, [CAMP122] §5.2.8.4 step 5).
    Expired,
    /// Left the device on an access link.
    Uploaded,
    /// Reached the deciding MA.
    Delivered,
    /// Still in the store when the run ended.
    HeldAtEnd,
    /// Left the run inside a vehicle that left it.
    LeftWithDevice,
}

/// The accused, as ground truth: who stands behind the subject certificate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FateTruth {
    /// The accused device, when the subject digest resolves to one.
    pub subject_node: Option<NodeId>,
    /// Whether the accused is one of the run's attackers.
    pub subject_attacker: bool,
}

/// `pki.report_fate` — one step of one report's life (NODE_AND_GT).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportFate {
    /// The instant.
    pub t: SimTime,
    /// The report.
    pub report_id: String,
    /// The reporting device (its own store).
    pub reporter: Option<NodeId>,
    /// What happened.
    pub fate: Fate,
    /// The report's store weight at that instant, when it was in the store.
    #[serde(default)]
    pub weight: Option<f64>,
    /// The subject certificate digest, hex.
    pub subject: String,
    /// The ground-truth join.
    pub truth: FateTruth,
}

impl Record for ReportFate {
    const CHANNEL: &'static str = "pki.report_fate";
    const VISIBILITY: Visibility = Visibility::NodeAndGt;
}

/// `ma.received` — one report as the deciding MA received it (NODE).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaReceived {
    /// The instant it reached the MA.
    pub t: SimTime,
    /// The report's id.
    pub report_id: String,
    /// The provider index of the deciding MA.
    pub ma: u32,
    /// The reporter's pseudonym certificate digest, hex (the report's signer).
    pub reporter_cert: String,
    /// The accused's pseudonym certificate digest, hex (`signingCertificate`).
    pub subject_cert: String,
    /// The checks the report lists, as the engine's check ids (one SM-MBR subtype each).
    pub subtypes: Vec<String>,
    /// The report's `generationTime`.
    pub generation_time: SimTime,
    /// The evidence BSMs' generation-time window.
    pub evidence_start: SimTime,
    /// Its end (the judged BSM).
    pub evidence_end: SimTime,
    /// How far the leading check's evidence exceeds its threshold, as the MA re-derives
    /// it from the evidence BSMs (≈ 1 at the threshold).
    pub excess: f64,
    /// The accused's broadcast positional accuracy, metres (J2735 PositionalAccuracy
    /// semi-major axis, as the BSM carried it).
    pub pos_accuracy_m: f64,
    /// The judged BSM's secMark modulo 50 ms, ms: the GNSS-epoch phase of the evidence.
    pub secmark_phase_ms: u32,
    /// The provider whose ACA issued the accused certificate.
    pub issuer: String,
    /// Opaque: equal for two reports exactly when the LAs link their subjects to one
    /// device.
    pub la_link: String,
    /// `None` when the MA accepted the report as evidence; else why it set it aside.
    #[serde(default)]
    pub set_aside: Option<String>,
}

impl Record for MaReceived {
    const CHANNEL: &'static str = "ma.received";
    const VISIBILITY: Visibility = Visibility::Node;
}

/// `gt.device_exposure` — one device's time in the run and its attack window (GT).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceExposure {
    /// The run's end (the record's instant).
    pub t: SimTime,
    /// The device.
    pub node: NodeId,
    /// Its `la_link` token, as [`MaReceived::la_link`] would carry it.
    pub la_link: String,
    /// When it joined the SCMS run.
    pub joined: SimTime,
    /// When it left, or the run's end.
    pub left: SimTime,
    /// Whether it is an attacker.
    pub attacker: bool,
    /// Its first falsified message, else its attack window's start (attackers only).
    #[serde(default)]
    pub attack_start: Option<SimTime>,
}

impl Record for DeviceExposure {
    const CHANNEL: &'static str = "gt.device_exposure";
    const VISIBILITY: Visibility = Visibility::Gt;
}

/// The `la_link` token of a device: a domain-separated digest of its node index, the
/// same for all its pseudonyms. It stands for the LAs' "same device?" answer and reveals
/// nothing else (no node id can be read back from it).
#[must_use]
pub fn la_link(node: NodeId, seed: u64) -> String {
    let mut b = Vec::with_capacity(40);
    b.extend_from_slice(b"v2xw/ma-replay/la-link/v1");
    b.extend_from_slice(&seed.to_le_bytes());
    b.extend_from_slice(&node.index().to_le_bytes());
    v2xw_core::hash::sha256_hex(&b)[..16].to_string()
}
