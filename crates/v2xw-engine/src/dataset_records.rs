//! The records the dataset release reads (`docs/dataset/schema.md`,
//! `docs/dataset/record-contract.md`), recorded only when the scenario names the
//! `dataset` exporter ([`dataset_requested`]); every other run records exactly what it did
//! before.
//!
//! Two kinds of addition:
//!
//! * fields on existing channels, each `None` (and so absent from the JSON) unless the
//!   gate is on: `node.tx.content` (the SPDU digest and header, the rest of Part I),
//!   `ma.received` ([`MaReceivedDataset`], read from the report the MA opened),
//!   `pki.report_fate` ([`ReportFateDataset`]), `ma.decision`
//!   ([`v2xw_threat::records::MaDecisionDataset`]);
//! * new channels: `node.rx_sec` ([`RxSec`], NODE), `node.pki_event` ([`PkiEvent`], NODE),
//!   `node.report_built` ([`ReportBuilt`], NODE), `ma.crl_entry` ([`CrlEntry`], NODE),
//!   `gt.tx_truth` ([`GtTxTruth`], GT) and `gt.device` ([`GtDevice`], GT).
//!
//! The join keys are computed from the bytes: `spdu_digest` is the first eight octets of
//! SHA-256 over the signed SPDU as it went on the air, so the MA, re-encoding an evidence
//! BSM it decoded from a report, computes the same key the sender recorded
//! ([`spdu_digest_of`]). A NODE record never carries ground truth: no node id of another
//! device, no attacker label, no true position.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use v2xw_core::ctx::{Record, Visibility};
use v2xw_core::ids::NodeId;
use v2xw_core::time::SimTime;

use crate::scenario::schema::{ExporterSpec, Scenario};

/// The exporter id that turns the dataset records on.
pub const EXPORTER_ID: &str = "dataset";

/// Whether the scenario names the `dataset` exporter, which turns every record of this
/// module on.
#[must_use]
pub fn dataset_requested(s: &Scenario) -> bool {
    names_dataset(&s.exporters)
}

/// [`dataset_requested`] over an exporter list.
#[must_use]
pub fn names_dataset(specs: &[ExporterSpec]) -> bool {
    specs.iter().any(|x| x.id == EXPORTER_ID)
}

/// The first eight octets of SHA-256 over `bytes`, as sixteen lower-case hex digits.
#[must_use]
pub fn digest_hex16(bytes: &[u8]) -> String {
    let d = v2xw_core::hash::sha256(bytes);
    v2xw_core::hash::hex_encode(&d[..8])
}

/// `spdu_digest` of a signed SPDU, as anyone holding its octets computes it.
#[must_use]
pub fn spdu_digest_of(spdu: &[u8]) -> String {
    digest_hex16(spdu)
}

/// The `spdu_digest` of a size-modelled message (no SPDU was encoded): SHA-256 over a
/// domain-separated canonical encoding of exactly what a receiver decodes from it: the
/// message type, the signer's digest, the generation time and the decoded content (its
/// JSON, fields in declaration order). `digest_basis` is then `fields`.
#[must_use]
pub fn fields_digest(
    msg_type: &str,
    signer: &[u8],
    generation_time: SimTime,
    content: &v2xw_metrics::channels::MsgContentView,
) -> String {
    let mut b = Vec::with_capacity(256);
    b.extend_from_slice(b"v2xw/dataset/fields-digest/v1");
    b.extend_from_slice(msg_type.as_bytes());
    b.push(0);
    b.extend_from_slice(signer);
    b.extend_from_slice(&generation_time.to_le_bytes());
    if let Ok(j) = serde_json::to_vec(content) {
        b.extend_from_slice(&j);
    }
    digest_hex16(&b)
}

/// The psid and `generationTime` (1609.2 Time64, TAI µs since 2004) of a signed SPDU, as
/// a receiver reads them from its `headerInfo`; `None` when the octets are not one signed
/// `Ieee1609Dot2Data`.
#[must_use]
pub fn spdu_header(spdu: &[u8]) -> Option<(u64, Option<u64>)> {
    let d = v2xw_proto::scms::mbr::decode_signed_bsm(spdu).ok()?;
    let o = v2xw_sec::sig1609::open_signed(&d)?;
    Some((o.psid, o.header.generation_time.as_ref().map(|t| t.0.0)))
}

/// What the engine keeps while the dataset records are on.
#[derive(Debug, Default)]
pub struct DatasetState {
    /// Each node's last closed channel-busy-ratio window (`mac.cbr`).
    pub cbr: BTreeMap<NodeId, f64>,
    /// Each recent message's `(spdu_digest, signer HashedId8 hex)`, by message id, for
    /// the receivers' `node.rx_sec`.
    pub tx: BTreeMap<u64, (String, String)>,
    /// The devices whose `gt.device` row is written.
    pub devices: BTreeSet<NodeId>,
    /// Each node's last `node.security` state seen, for `node.pki_event`.
    pub pki: BTreeMap<NodeId, PkiSeen>,
}

/// What `node.pki_event` diffs between two `node.security` windows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PkiSeen {
    /// The pseudonym signed with.
    pub pseudonym: Option<String>,
    /// The certificates stored.
    pub pool_stored: Option<u64>,
    /// The CRL version installed.
    pub crl_version: Option<u64>,
}

/// Messages kept for the receivers' join: settled receptions are milliseconds to seconds
/// behind the transmission, so the oldest are dropped past this many.
const TX_KEEP: usize = 100_000;

impl DatasetState {
    /// Keeps message `msg`'s digest and signer for its receptions.
    pub fn note_tx(&mut self, msg: u64, digest: String, cert: String) {
        self.tx.insert(msg, (digest, cert));
        if self.tx.len() > TX_KEEP
            && let Some(cut) = self.tx.keys().nth(TX_KEEP / 2).copied()
        {
            self.tx = self.tx.split_off(&cut);
        }
    }
}

/// The `node.pki_event` rows one `node.security` window implies, against the node's
/// previous window: its first window (`enrolled`), a new signing pseudonym
/// (`pseudonym_change`), more certificates stored (`batch_downloaded`) and a newer CRL
/// installed (`crl_downloaded`). All of it is the device's own state.
pub fn pki_events(
    ds: &mut DatasetState,
    v: &crate::sec_records::NodeSecurityView,
) -> Vec<PkiEvent> {
    let now = PkiSeen {
        pseudonym: v.pseudonym.clone(),
        pool_stored: Some(u64::from(v.pool_stored)),
        crl_version: Some(u64::from(v.crl_version)),
    };
    let ev = |event: &str| PkiEvent {
        t: v.t,
        node: v.node,
        event: event.to_string(),
        cert_id: v.pseudonym.clone(),
        crl_series: None,
        crl_number: None,
        crl_entries: None,
        bytes: None,
        latency_ms: None,
    };
    let mut out = Vec::new();
    match ds.pki.get(&v.node) {
        None => out.push(ev("enrolled")),
        Some(before) => {
            if before.pseudonym != now.pseudonym && now.pseudonym.is_some() {
                out.push(ev("pseudonym_change"));
            }
            if now.pool_stored > before.pool_stored {
                out.push(ev("batch_downloaded"));
            }
            if now.crl_version > before.crl_version {
                let newest = v.crls.iter().max_by_key(|c| c.counter);
                out.push(PkiEvent {
                    crl_series: newest.map(|c| u64::from(c.series)),
                    crl_number: newest.map(|c| u64::from(c.counter)),
                    crl_entries: Some(u64::from(v.crl_entries)),
                    ..ev("crl_downloaded")
                });
            }
        }
    }
    ds.pki.insert(v.node, now);
    out
}

/// The `verify_result` of one settled reception, from what the receiver's security stack
/// concluded; `None` when the receiver was off (it concluded nothing).
#[must_use]
pub fn verify_result_of(d: &v2xw_node::RxDisposition) -> Option<&'static str> {
    use v2xw_node::RxDisposition as D;
    use v2xw_node::rxval::RxReject as R;
    use v2xw_node::stores::VerificationState as V;
    Some(match d {
        D::Delivered(V::Verified) => "ok",
        D::Delivered(V::Unverified) => "not_checked",
        D::Delivered(V::Invalid) => "bad_signature",
        D::Delivered(V::Revoked) => "on_crl",
        D::UnknownSigner => "unknown_issuer",
        D::P2pcdResponse => "unsecured",
        D::PsidNotRegistered => "psid_not_registered",
        D::Dropped(_) => "dropped_unchecked",
        D::NodeOff => return None,
        D::Refused(r) | D::NotRelevant(r) => match r {
            R::Chain(v2xw_sec::chain::ChainErrorKind::BadSignature) => "bad_signature",
            R::Chain(_) => "chain_invalid",
            R::UnknownIssuer => "unknown_issuer",
            R::PsidNotPermitted => "psid_not_permitted",
            R::GenerationTimeOutsideValidity => "outside_validity",
            R::OutsideRegion => "outside_region",
            R::RevokedByHash | R::IssuerRevoked => "on_crl",
            R::CrlOverdue => "crl_overdue",
            R::GenerationTimeTooOld => "stale",
            R::GenerationTimeInFuture => "future",
            R::CrlMalformed | R::CrlSignerUntrusted | R::CrlRejected | R::CrlStale => {
                "crl_rejected"
            }
        },
    })
}

/// `node.rx_sec` — the receiver's security verdict on one message it decoded (NODE).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RxSec {
    /// When the receiver settled it.
    pub t: SimTime,
    /// The receiving node (its own record).
    pub rx: NodeId,
    /// The engine's message id: the join key to `phy.rx.msg` and `node.tx.msg`. An
    /// exporter uses it for the truth join only; no observer table carries it.
    pub msg: u64,
    /// The message's `spdu_digest`, as the receiver computes it from the octets it got.
    #[serde(default)]
    pub spdu_digest: Option<String>,
    /// The signing certificate's HashedId8, hex (the SPDU's signer).
    #[serde(default)]
    pub cert_id: Option<String>,
    /// The message type.
    pub msg_type: String,
    /// The verdict ([`verify_result_of`]).
    pub verify_result: String,
    /// The signature check's duration, µs; `None` when no check ran.
    #[serde(default)]
    pub verify_latency_us: Option<u64>,
    /// The receiver's channel busy ratio, its last closed measurement window (J2945/1).
    #[serde(default)]
    pub cbr: Option<f64>,
}

impl Record for RxSec {
    const CHANNEL: &'static str = "node.rx_sec";
    const VISIBILITY: Visibility = Visibility::Node;
}

/// `node.pki_event` — one step of an OBU's own credential life (NODE).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PkiEvent {
    /// The instant (the node's telemetry window that saw it).
    pub t: SimTime,
    /// The device.
    pub node: NodeId,
    /// `enrolled`, `batch_downloaded`, `pseudonym_change`, `crl_downloaded`,
    /// `download_refused`.
    pub event: String,
    /// The pseudonym it signs with after the event, hex HashedId8.
    #[serde(default)]
    pub cert_id: Option<String>,
    /// The CRL series, for a CRL download.
    #[serde(default)]
    pub crl_series: Option<u64>,
    /// The CRL number (`crlSerial`), for a CRL download.
    #[serde(default)]
    pub crl_number: Option<u64>,
    /// The entries the device holds after a CRL download.
    #[serde(default)]
    pub crl_entries: Option<u64>,
    /// Bytes of the download, when the device measured them.
    #[serde(default)]
    pub bytes: Option<u64>,
    /// The download's latency, ms, when the device measured it.
    #[serde(default)]
    pub latency_ms: Option<u64>,
}

impl Record for PkiEvent {
    const CHANNEL: &'static str = "node.pki_event";
    const VISIBILITY: Visibility = Visibility::Node;
}

/// `ma.crl_entry` — one entry the MA caused to be published on a CRL (NODE: the CRL is
/// public, the link token is the MA's own).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrlEntry {
    /// The instant the CRL carrying it was issued.
    pub t_issued: SimTime,
    /// The CRL series.
    pub crl_series: u64,
    /// The CRL number (the CRL generator's version after the entry).
    pub crl_number: u64,
    /// `linkage_seed` or `hash_id`.
    pub entry_kind: String,
    /// The MA's `la_link` token of the revoked device.
    pub la_link: String,
    /// The i-period the entry's linkage seed is for.
    pub i_period: u64,
    /// When it takes effect (the issue instant: a device applies a CRL on receipt).
    pub t_effective: SimTime,
    /// The certificate the decision named, hex HashedId8.
    pub subject_cert_id: String,
}

impl Record for CrlEntry {
    const CHANNEL: &'static str = "ma.crl_entry";
    const VISIBILITY: Visibility = Visibility::Node;
}

/// `gt.tx_truth` — the sender's true state when it transmitted (GT).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GtTxTruth {
    /// The transmit instant.
    pub t: SimTime,
    /// The message (`node.tx.msg`).
    pub msg: u64,
    /// The sending node.
    pub node: NodeId,
    /// True position, world frame, metres east.
    pub x_m: f64,
    /// True position, metres north.
    pub y_m: f64,
    /// True ground speed, m/s.
    pub speed_mps: f64,
    /// True heading, degrees clockwise from north (J2735 convention).
    pub heading_deg: f64,
    /// True longitudinal acceleration, m/s².
    pub accel_mps2: f64,
    /// The sender's own position error (its estimate against the truth), metres: the
    /// honest error even for an attacker, whose on-air claim is falsified on top.
    #[serde(default)]
    pub gnss_error_m: Option<f64>,
    /// From the GNSS fix the message reports to its stamp, ms, where the engine models it.
    #[serde(default)]
    pub fix_to_stamp_ms: Option<f64>,
}

impl Record for GtTxTruth {
    const CHANNEL: &'static str = "gt.tx_truth";
    const VISIBILITY: Visibility = Visibility::Gt;
}

/// `gt.device` — one device's identity and class, once (GT).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GtDevice {
    /// The instant it was first seen on the air.
    pub t: SimTime,
    /// The device.
    pub node: NodeId,
    /// `vehicle`, `rsu` or `vru`.
    pub class: String,
    /// Its SCMS provider, when it holds SCMS credentials.
    #[serde(default)]
    pub provider: Option<String>,
    /// Its BSM timing class's name (`locked`, `free`, or a calibration's own class), when
    /// the per-device BSM timing model runs.
    #[serde(default)]
    pub gnss_class: Option<String>,
    /// Whether it is an honest device with an out-of-specification fault.
    pub faulty: bool,
    /// The fault, when it is faulty.
    #[serde(default)]
    pub fault_kind: Option<String>,
    /// Whether it is one of the run's attackers (a compromised roadside unit included).
    pub attacker: bool,
    /// The attack a device runs when no `gt.attack.action` names it: a compromised
    /// roadside unit's kind (`FalseCrl`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attack_id: Option<String>,
    /// The attack a vehicle attacker is scheduled to run (the last segment of its model
    /// id, `RandomSpeed`), so a device that leaves the run before its attack window opens,
    /// and so never logs a `gt.attack.action`, still names the attack it was given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_attack: Option<String>,
}

impl Record for GtDevice {
    const CHANNEL: &'static str = "gt.device";
    const VISIBILITY: Visibility = Visibility::Gt;
}

/// One SM-MBR subtype the report carries, decoded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtypeCode {
    /// The report type's number (1 1VInconsistent, 2 Implausible, 3 ObsImplausible).
    #[serde(rename = "type")]
    pub mbr_type: u8,
    /// The subtype's number within its type.
    pub subtype: u8,
    /// Its SM-MBR name.
    pub name: String,
    /// Its parameters, as `name=value` in the SM-MBR unit.
    pub params: Vec<String>,
}

/// The report's `generationLocation`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    /// Latitude, degrees.
    pub lat_deg: f64,
    /// Longitude, degrees.
    pub lon_deg: f64,
    /// Elevation, metres; `None` when the encoding says unknown.
    #[serde(default)]
    pub elevation_m: Option<f64>,
}

/// One evidence BSM, decoded from the report as the MA reads it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceBsm {
    /// Its `spdu_digest`, over the COER of the signed BSM the report carries.
    pub spdu_digest: String,
    /// Its 1609.2 `generationTime`, Time64 µs.
    #[serde(default)]
    pub generation_time_us: Option<u64>,
    /// `secMark`, ms.
    #[serde(default)]
    pub sec_mark_ms: Option<u16>,
    /// `msgCnt`.
    #[serde(default)]
    pub msg_cnt: Option<u8>,
    /// Latitude, degrees.
    #[serde(default)]
    pub lat_deg: Option<f64>,
    /// Longitude, degrees.
    #[serde(default)]
    pub lon_deg: Option<f64>,
    /// Speed, m/s.
    #[serde(default)]
    pub speed_mps: Option<f64>,
    /// Heading, degrees clockwise from north.
    #[serde(default)]
    pub heading_deg: Option<f64>,
    /// `accelSet.long`, m/s².
    #[serde(default)]
    pub accel_long_mps2: Option<f64>,
    /// `brakes.wheelBrakes`, the raw J2735 `BrakeAppliedStatus` bits.
    #[serde(default)]
    pub brakes: Option<u8>,
    /// `PositionalAccuracy.semiMajor`, metres.
    #[serde(default)]
    pub pos_accuracy_m: Option<f64>,
}

/// What `ma.received` gains under the dataset gate: the report as the MA decoded it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaReceivedDataset {
    /// First eight octets of SHA-256 over the reporter-signed MBR SPDU the MA decrypted;
    /// `None` for a report on the modelled path (no bytes).
    pub report_digest: Option<String>,
    /// The application PSID the report is about (0x20, `Psid20Mbr`).
    pub psid: u64,
    /// `1VInconsistent`, `Implausible` or `ObsImplausible`; `None` on the modelled path.
    pub mbr_type: Option<String>,
    /// The SM-MBR subtypes it carries.
    pub subtype_codes: Vec<SubtypeCode>,
    /// The reporter's fired checks with no SM-MBR code (research-suite checks).
    pub nonstandard_triggers: Vec<String>,
    /// The signed report's 1609.2 `headerInfo.generationTime` (Time64, TAI µs since
    /// 2004-01-01) as the MA read it; `None` on the modelled path.
    #[serde(default)]
    pub generation_time_us: Option<u64>,
    /// The report's `generationLocation`.
    pub generation_location: Option<Location>,
    /// The evidence BSMs.
    pub evidence: Vec<EvidenceBsm>,
    /// When the MA obtained `la_link` from the LAs; `None`: it had not asked when the
    /// report arrived (the token is filled from the device's chain, see the contract).
    pub la_link_t: Option<SimTime>,
}

/// What `pki.report_fate` gains under the dataset gate, once the report is built.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReportFateDataset {
    /// The report's digest (as [`MaReceivedDataset::report_digest`]).
    pub report_digest: Option<String>,
    /// The secured report's size, bytes (encrypted to the MA).
    pub size_bytes: Option<u64>,
    /// `cellular` or `rsu_relay`.
    pub path: Option<String>,
    /// The evidence BSMs' digests.
    pub evidence_spdu_digests: Vec<String>,
    /// The reporter's own checks that fired on the subject (its detector's names): what
    /// the OBU knows of its report from the moment it is created.
    #[serde(default)]
    pub checks: Vec<String>,
    /// The reporter's detector score for the report (its excess over the threshold).
    #[serde(default)]
    pub score: Option<f64>,
}

/// `node.report_built` — a report its reporter built and secured for upload: the bytes
/// that left the device (NODE: the reporter's own knowledge of its own report).
///
/// The engine builds a report's octets when the upload reaches the Location Obscurer
/// Proxy, after `pki.report_fate`'s `uploaded` step, and before this record only the
/// `delivered` step carried them; a report still in the RA's shuffle at the run's end
/// (a third of the first v1.1 build's uploads) was left with no digest and no evidence.
/// The fate channel is left exactly as it is without the dataset gate, so the build is a record
/// of its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportBuilt {
    /// The instant the build was recorded (the upload's arrival at the proxy).
    pub t: SimTime,
    /// The reporting device.
    pub reporter: NodeId,
    /// The report's id on its device (as `pki.report_fate.report_id`).
    pub report_id: String,
    /// The accused certificate digest, hex (as `pki.report_fate.subject`).
    pub subject: String,
    /// The built report's digest, size, path and evidence digests.
    #[serde(flatten)]
    pub built: ReportFateDataset,
}

impl Record for ReportBuilt {
    const CHANNEL: &'static str = "node.report_built";
    const VISIBILITY: Visibility = Visibility::Node;
}

/// The decoded view of an evidence BSM (`signed` COER `Ieee1609Dot2Data`).
#[must_use]
pub fn evidence_bsm(b: &v2xw_msg::sec_types::ieee1609_dot2::Ieee1609Dot2Data) -> EvidenceBsm {
    use v2xw_core::math::quantize_to;
    use v2xw_msg::j2735::bsm;
    let bytes = v2xw_msg::sec_types::coer::encode(v2xw_msg::MsgType::Bsm, b).unwrap_or_default();
    let mut e = EvidenceBsm {
        spdu_digest: spdu_digest_of(&bytes),
        generation_time_us: None,
        sec_mark_ms: None,
        msg_cnt: None,
        lat_deg: None,
        lon_deg: None,
        speed_mps: None,
        heading_deg: None,
        accel_long_mps2: None,
        brakes: None,
        pos_accuracy_m: None,
    };
    let Some(o) = v2xw_sec::sig1609::open_signed(b) else {
        return e;
    };
    e.generation_time_us = o.header.generation_time.as_ref().map(|t| t.0.0);
    if let Ok(m) = bsm::decode_message_frame(&o.payload) {
        let k = &m.core;
        e.msg_cnt = Some(k.msg_cnt);
        e.sec_mark_ms = (k.sec_mark != bsm::D_SECOND_UNAVAILABLE).then_some(k.sec_mark);
        e.lat_deg = (k.lat != bsm::LATITUDE_UNAVAILABLE)
            .then(|| quantize_to(f64::from(k.lat) * 1e-7, 1e-7));
        e.lon_deg = (k.lon != bsm::LONGITUDE_UNAVAILABLE)
            .then(|| quantize_to(f64::from(k.lon) * 1e-7, 1e-7));
        e.speed_mps = (k.speed != bsm::SPEED_UNAVAILABLE)
            .then(|| quantize_to(f64::from(k.speed) * 0.02, 1e-3));
        e.heading_deg = (k.heading != bsm::HEADING_UNAVAILABLE)
            .then(|| quantize_to(f64::from(k.heading) * 0.0125, 1e-7));
        e.accel_long_mps2 = (k.accel_set.long != bsm::ACCELERATION_UNAVAILABLE)
            .then(|| quantize_to(f64::from(k.accel_set.long) * 0.01, 1e-3));
        e.brakes = Some(k.brakes.wheel_brakes.0);
        e.pos_accuracy_m = (k.accuracy.semi_major != bsm::SEMI_AXIS_UNAVAILABLE)
            .then(|| quantize_to(f64::from(k.accuracy.semi_major) * 0.05, 1e-3));
    }
    e
}

/// A subtype as the dataset records it.
#[must_use]
pub fn subtype_code(s: v2xw_proto::scms::mbr::Subtype) -> SubtypeCode {
    use v2xw_proto::scms::mbr::Subtype as S;
    let params = match s {
        S::ConstantPosition | S::BrakeAcceleration => Vec::new(),
        S::RandomPosition { gps_drift_mm } => vec![format!("gpsDrift_mm={gps_drift_mm}")],
        S::MaxSpeed { threshold_mps } => vec![format!("threshold_mps={threshold_mps}")],
        S::MaxAcceleration { threshold_cmps2 } => {
            vec![format!("threshold_cmps2={threshold_cmps2}")]
        }
        S::ObsDistance {
            threshold_m,
            gps_drift_m,
        } => vec![
            format!("threshold_m={threshold_m}"),
            format!("gpsDrift_m={gps_drift_m}"),
        ],
    };
    SubtypeCode {
        mbr_type: s.mbr_type().number(),
        subtype: s.number(),
        name: s.name().to_string(),
        params,
    }
}

/// What the MA reads from a report it opened.
#[must_use]
pub fn opened_view(o: &v2xw_proto::scms::mbr::OpenedReport) -> MaReceivedDataset {
    use v2xw_core::math::quantize_to;
    let loc = &o.report.location;
    // 1609.2 `Elevation` (Uint16): decimetres above the WGS-84 ellipsoid with a
    // −4,096 dm offset (`v2xw_sec::envelope::GenerationLocation`).
    let elevation_m = Some(quantize_to(
        (f64::from(loc.elevation) - 4_096.0) * 0.1,
        1e-3,
    ));
    MaReceivedDataset {
        report_digest: Some(digest_hex16(&o.signed)),
        psid: v2xw_proto::scms::mbr::BSM_APP_PSID,
        mbr_type: Some(o.report.mbr_type.name().to_string()),
        subtype_codes: o.report.subtypes.iter().map(|s| subtype_code(*s)).collect(),
        nonstandard_triggers: Vec::new(),
        generation_time_us: Some(o.signed_at_us),
        generation_location: Some(Location {
            lat_deg: quantize_to(f64::from(loc.lat_tenth_microdeg) * 1e-7, 1e-7),
            lon_deg: quantize_to(f64::from(loc.lon_tenth_microdeg) * 1e-7, 1e-7),
            elevation_m,
        }),
        evidence: o
            .report
            .evidence
            .suspect()
            .bsms
            .iter()
            .map(evidence_bsm)
            .collect(),
        la_link_t: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lint(rows: &[serde_json::Value]) -> bool {
        v2xw_record::dataset::leakage::lint_rows("node", rows).is_clean()
    }

    /// Each new channel is registered with the visibility its record type declares, so the
    /// writer's tag check (a GT record on a NODE channel is refused) governs it.
    #[test]
    fn every_new_channel_is_registered_with_its_records_visibility() {
        fn check<R: Record>() {
            let spec = v2xw_record::channels::by_name(R::CHANNEL)
                .unwrap_or_else(|| panic!("{} is not in the channel table", R::CHANNEL));
            assert_eq!(spec.visibility, R::VISIBILITY, "{}", R::CHANNEL);
        }
        check::<RxSec>();
        check::<PkiEvent>();
        check::<CrlEntry>();
        check::<GtTxTruth>();
        check::<GtDevice>();
    }

    /// The NODE records carry no ground-truth key at any depth (the leakage linter).
    #[test]
    fn the_node_records_pass_the_leakage_linter() {
        let rx = RxSec {
            t: 1,
            rx: NodeId(2),
            msg: 3,
            spdu_digest: Some("00".repeat(8)),
            cert_id: Some("11".repeat(8)),
            msg_type: "bsm".to_string(),
            verify_result: "ok".to_string(),
            verify_latency_us: Some(10),
            cbr: Some(0.1),
        };
        let ev = PkiEvent {
            t: 1,
            node: NodeId(2),
            event: "crl_downloaded".to_string(),
            cert_id: None,
            crl_series: Some(1),
            crl_number: Some(2),
            crl_entries: Some(3),
            bytes: None,
            latency_ms: None,
        };
        let crl = CrlEntry {
            t_issued: 1,
            crl_series: 1,
            crl_number: 2,
            entry_kind: "linkage_seed".to_string(),
            la_link: "ab".repeat(8),
            i_period: 0,
            t_effective: 1,
            subject_cert_id: "cd".repeat(8),
        };
        let ma = MaReceivedDataset {
            report_digest: Some("ef".repeat(8)),
            psid: 0x20,
            mbr_type: Some("1VInconsistent".to_string()),
            subtype_codes: vec![SubtypeCode {
                mbr_type: 1,
                subtype: 1,
                name: "ConstantPosition".to_string(),
                params: Vec::new(),
            }],
            nonstandard_triggers: vec!["staleOrReplay".to_string()],
            generation_time_us: Some(725_846_400_000_000),
            generation_location: None,
            evidence: Vec::new(),
            la_link_t: None,
        };
        let rows: Vec<serde_json::Value> = vec![
            serde_json::to_value(&rx).unwrap(),
            serde_json::to_value(&ev).unwrap(),
            serde_json::to_value(&crl).unwrap(),
            serde_json::to_value(&ma).unwrap(),
        ];
        assert!(
            lint(&rows),
            "a NODE dataset record carries a ground-truth key"
        );
    }

    /// The gate reads the exporter list and nothing else.
    #[test]
    fn the_gate_is_the_dataset_exporter() {
        let spec = |id: &str| ExporterSpec {
            id: id.to_string(),
            opts: serde_json::Value::Null,
        };
        assert!(!names_dataset(&[]));
        assert!(!names_dataset(&[spec("veremi"), spec("recording")]));
        assert!(names_dataset(&[spec("recording"), spec(EXPORTER_ID)]));
    }
}
