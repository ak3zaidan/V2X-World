//! The security lifecycle and the backend: credentials, pseudonyms, misbehaviour
//! reporting, the Misbehaviour Authority's decision, revocation and CRL distribution.
//!
//! 10-roadmap.md's Phase 2 is a *path*, not a feature list: a vehicle detects another one
//! lying about its position, files a misbehaviour report, the report crosses the
//! vehicle's backend link to the Misbehaviour Authority, the authority decides on
//! sustained evidence, the PCA and both Linkage Authorities identify the device, the CRL
//! Generator issues on its cadence, the list is distributed by cellular download and by
//! roadside broadcast, and every vehicle that installs it stops trusting the liar. This
//! module wires that path across the crates that model each piece and **reimplements
//! none of them**:
//!
//! | Piece | Whose | What this module does with it |
//! |---|---|---|
//! | the attacker | [`v2xw_threat::LegacyAttacker`] | calls `act` on the outgoing claim before it is signed |
//! | the detector suite | [`v2xw_threat::Legacy12`] | feeds it the messages the node's own runtime delivered |
//! | the report | [`v2xw_threat::MisbehaviourReport`] | builds one `from_verdict`, at most once a second per subject per reporter |
//! | the access leg | [`crate::backend::BackendAccess`] | cellular Uu, roadside relay, or none |
//! | the backend | [`v2xw_proto::ScmsRun`] | run **in lockstep** with the engine's clock: RA shuffles, PCA and LA lookups, CRL cadence |
//! | the decision | [`v2xw_threat::CorroboratedMa`] (default) or [`v2xw_threat::LegacyWindow`] | `detection.ma`: independent corroborated events over time, or the legacy persistence gate |
//! | the CRL | [`v2xw_sec::linkage::CrlLinkageEntry`] | installs it in the receiving node's own `CrlGate` |
//! | enforcement | [`v2xw_node::stores::CrlGate`] | the node's own revocation check does the rest |
//!
//! # Why honest traffic used to be revoked
//!
//! The Phase 2 wiring took the authority's decision itself: whenever it held two reports
//! about two pseudonyms it asked the Linkage Authorities whether they belonged to one
//! device, and revoked if they did. The Linkage Authorities answer *linkage*, not
//! *guilt*: two honest receivers reporting one honest vehicle across a pseudonym change —
//! a Gauss–Markov GNSS burst of 3 s at six times the nominal noise is exactly that, and
//! the legacy engine says its burst length was chosen to be shorter than "the revocation
//! persistence gate" — is correctly linked, and was revoked. The persistence gate is the
//! legacy authority's `threat/ma/legacy-window` (k = 3 trusted reporters, reports in 4
//! distinct seconds spanning at least 3.0 s inside a 15 s window; `PipelineConfig`
//! `report_threshold_k`, `revoke_min_seconds`, `revoke_persist_s`, `revoke_window_s`), and
//! it ships in `v2xw-threat` — it was simply not on this path. It is now: the decision is
//! the pipeline's, and the backend only carries it out.
//!
//! # Why honest traffic was still revoked, and the second fix (2026-09-30)
//!
//! With the persistence gate in place, the shipped `revocation-latency.yaml` with its
//! attackers removed still revoked 21 honest vehicles in 300 s at 6,000 veh/h, and
//! `phase2-manhattan` one (honest node 28: eight reporters flagged one burst between 257.1
//! and 257.9 s). Two things were wrong, and both are fixed here:
//!
//! 1. **The detector judged every message against a constant 5 m confidence.** A BSM
//!    states its own accuracy (J2735 `PositionalAccuracy`) and a CAM its own confidence
//!    ellipse; the detector host now joins each message's stated accuracy back by
//!    `(signer, generation time)` ([`broadcast_accuracy_95_m`]) and the suite's tolerance
//!    is that. A receiver in an urban canyon states a wider ellipse and is judged by it.
//! 2. **The authority counted one event heard by many as many pieces of evidence.** Eight
//!    receivers of one GNSS burst met "three reporters, four distinct seconds, a 3 s span"
//!    by themselves. The default authority is now `threat/ma/corroborated`
//!    ([`v2xw_threat::CorroboratedMa`]): reports about one pseudonym are correlated into
//!    events, an event counts when two independent trusted reporters witnessed it, and
//!    revocation takes several such events sustained over time. The legacy gate stays
//!    selectable for parity with the legacy corpus.
//!
//! A benign GNSS burst still gets reported — a real receiver under-reports its error in a
//! multipath spike, and a real fleet's detectors fire on it — and that is the honest
//! behaviour: what must not follow is a revocation.
//!
//! # What the detectors check, and against what (2026-10-06)
//!
//! Every check that compares positions allows for the confidence each message states
//! (CaTch, Kamel et al., IEEE WCNC 2019): the motion checks through the stated 95 %
//! radius, the heading check through the bearing error two stated radii imply over the
//! longest straight baseline the sender's own headings describe (up to 10 s), and the
//! map check through the stated disc. Each receiver holds a map of the motor-vehicle
//! lanes ([`RoadMap`]), as F2MD's position-plausibility check assumes, so a position
//! lie that stays self-consistent (the legacy `ConstPosOffset`) is visible at all. A PSM
//! sender is checked as a vulnerable road user. The authority's reporter budget is a
//! rate inside its window, not a run-long count, so a receiver that follows one liar
//! stays a witness against the next.
//!
//! # The joints, stated
//!
//! **1. The air credential is the SCMS credential (WP-C1; this joint is closed).** Each
//! vehicle signs with the implicit certificate the ACA issued in its batch and the key
//! `d = e·b + r` it derived from its download ([`ProvisionedCred::key`],
//! `ObuRuntime::install_credential`), so the digest on the air, the linkage value a CRL
//! revokes and the certificate bytes are the protocol's own. A receiver reads the linkage
//! value out of the certificate it verified against; no side channel carries it. Where no
//! protocol flow issues a credential the same ACA issues one without the flows
//! ([`crate::credentials`]).
//!
//! **2. The subject lookup.** A report names a certificate by its digest; the engine maps
//! digest → `(node, i, j)` from what it provisioned, standing in for the PCA's own table.
//! The PCA's *service time* for that lookup is charged by the backend.
//!
//! **3. Pre-run provisioning.** A vehicle on the road already holds its pool, so each
//! vehicle's first batches are provisioned *before* the run on a scratch kernel
//! ([`v2xw_proto::ScmsRun::preload`]) — real butterfly expansion, real LA and PCA state —
//! and the run's own backend clock starts clean. Top-ups during the run are real in-run
//! flows over the vehicle's access link.
//!
//! **4. The access leg of multi-message flows.** A report's upload and a CRL download are
//! carried packet by packet by the vehicle's own Uu model or relay. A top-up's exchange
//! (request, acknowledgement, batch polls) runs inside the backend kernel over a link set
//! to the access's nominal latency and capacity at the instant it starts.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use v2xw_core::geom::Vec3;
use v2xw_core::ids::{ActorId, NodeId};
use v2xw_core::time::{Duration, SimTime};
use v2xw_proto::net::Transport;
use v2xw_proto::scms::mbr::{self, MbrQueue};
use v2xw_proto::scms::run::{ScmsRun, crl_bytes};
use v2xw_proto::stage::{FlowRun, StageId};
use v2xw_proto::view::{BackendView, EdgeTracker};
use v2xw_proto::{RevocationLatency, ScmsParams};
use v2xw_sec::linkage::{CrlLinkageEntry, LinkageValue};
use v2xw_threat::{
    AttackKind, Attacker, AttackerView, DetectorParams, Emission, Evidence, HonestClaim, Legacy12,
    LegacyAttacker, LegacyAttackerParams, LegacyWindow, MaAction, MaParams, MisbehaviourReport,
    NoMap, ObservedKind, ObservedMessage, SelfBelief, StationType, ThreatCtx, VerificationState,
};
use v2xw_world::World;

use crate::backend::{AccessKind, BackendAccess, Backhaul};
use crate::error::{EngineError, Result};
use crate::scenario::Scenario;

/// The protocol id a scenario names in `actors.backend.protocol` or `security.protocol`.
pub const CAMP_SCMS: &str = v2xw_proto::CAMP_SCMS_ID;

/// The ETSI ITS PKI's protocol id. Recognised so it can be refused by name.
pub const ETSI_PKI: &str = "protocol/etsi/ts102941";

/// The detector suite's id, as `detection.local` names it.
pub const LEGACY_12: &str = "detect/legacy-12";

/// The authority pipeline's id, as `detection.ma` names it.
pub const MA_LEGACY_WINDOW: &str = v2xw_threat::ma::MODEL_ID;

/// The id of the authority pipeline that decides on independent evidence over time, and
/// the one a scenario runs when `detection.ma` is not set.
pub const MA_CORROBORATED: &str = v2xw_threat::ma_corroborate::MODEL_ID;

/// The authority pipelines this build ships, by id.
pub const MA_PIPELINES: [&str; 2] = [MA_CORROBORATED, MA_LEGACY_WINDOW];

/// The Misbehaviour Authority's decision pipeline, whichever the scenario chose.
#[derive(Debug, Clone)]
pub(crate) enum MaHost {
    /// `threat/ma/legacy-window`: the legacy persistence gate, kept for parity with the
    /// legacy corpus.
    Legacy(LegacyWindow),
    /// `threat/ma/corroborated`: reports correlated into events, revocation on
    /// independent corroborated events over time.
    Corroborated(v2xw_threat::CorroboratedMa),
}

impl MaHost {
    fn ingest_evidence(&mut self, r: &MisbehaviourReport, at: SimTime) -> Option<MaAction> {
        match self {
            MaHost::Legacy(m) => m.ingest_evidence(r, at),
            MaHost::Corroborated(m) => m.ingest_evidence(r, at),
        }
    }

    fn id(&self) -> &'static str {
        match self {
            MaHost::Legacy(_) => MA_LEGACY_WINDOW,
            MaHost::Corroborated(_) => MA_CORROBORATED,
        }
    }

    fn subjects(&self) -> usize {
        match self {
            MaHost::Legacy(m) => m.subjects(),
            MaHost::Corroborated(m) => m.subjects(),
        }
    }

    /// The corroborated events the authority holds about one pseudonym right now (the
    /// corroborating pipeline only).
    fn corroborated_events(&self, subject: &str) -> Option<u32> {
        match self {
            MaHost::Legacy(_) => None,
            MaHost::Corroborated(m) => Some(m.summary(subject).corroborated_events),
        }
    }

    fn peak_unrevoked_events(&self) -> Option<u32> {
        match self {
            MaHost::Legacy(_) => None,
            MaHost::Corroborated(m) => Some(m.peak_unrevoked_events()),
        }
    }

    fn trust_infrastructure(&mut self, digest: String) {
        match self {
            MaHost::Legacy(m) => m.trust_infrastructure(digest),
            MaHost::Corroborated(m) => m.trust_infrastructure(digest),
        }
    }

    /// The pipeline `detection.ma` names, with its parameters; the corroborating one
    /// when the key is absent.
    pub(crate) fn from_scenario(scenario: &Scenario) -> Result<MaHost> {
        let Some(choice) = &scenario.detection.ma else {
            return Ok(MaHost::Corroborated(v2xw_threat::CorroboratedMa::defaults()));
        };
        let empty = serde_json::Map::new();
        let map = choice.params.as_object().unwrap_or(&empty);
        // Every refusal names the key it is about (`detection.ma.params.<key>`), so the page
        // marks the setting rather than the whole block, and says what the key must be. The
        // two-pipeline rewrite (mbd track) had reported unknown keys against
        // `detection.ma.params` and accepted a negative `revoke_window_s`; the robustness
        // track's Check tests pin both.
        let bad = |key: &str, value: &serde_json::Value, must: &str| {
            conflict(
                &format!("detection.ma.params.{key}"),
                format!("is {value}; it must be {must}"),
            )
        };
        let count = |key: &str, value: &serde_json::Value| {
            value
                .as_u64()
                .filter(|v| *v >= 1)
                .map(|v| usize::try_from(v).unwrap_or(usize::MAX))
                .ok_or_else(|| bad(key, value, "a whole number of at least 1"))
        };
        let whole_u32 = |key: &str, value: &serde_json::Value| {
            value
                .as_u64()
                .map(|v| u32::try_from(v).unwrap_or(u32::MAX))
                .ok_or_else(|| bad(key, value, "a whole number, 0 or more"))
        };
        let seconds = |key: &str, value: &serde_json::Value| {
            value
                .as_f64()
                .filter(|v| v.is_finite() && *v > 0.0)
                .ok_or_else(|| bad(key, value, "a number of seconds above 0"))
        };
        let seconds_or_zero = |key: &str, value: &serde_json::Value| {
            value
                .as_f64()
                .filter(|v| v.is_finite() && *v >= 0.0)
                .ok_or_else(|| bad(key, value, "a number of seconds, 0 or more"))
        };
        let flag = |key: &str, value: &serde_json::Value| {
            value
                .as_bool()
                .ok_or_else(|| bad(key, value, "true or false"))
        };
        let unknown = |other: &str, takes: &[&str]| {
            conflict(
                &format!("detection.ma.params.{other}"),
                format!(
                    "'{other}' is not a parameter of {}; it takes {}. Remove the key or \
                     correct its spelling",
                    choice.id,
                    takes.join(", ")
                ),
            )
        };
        match choice.id.as_str() {
            MA_LEGACY_WINDOW => {
                let mut p = MaParams::default();
                for (key, value) in map {
                    match key.as_str() {
                        "report_threshold_k" => p.report_threshold_k = count(key, value)?,
                        "revoke_min_seconds" => p.revoke_min_seconds = count(key, value)?,
                        "revoke_persist_s" => p.revoke_persist_s = seconds_or_zero(key, value)?,
                        "revoke_window_s" => p.revoke_window_s = seconds_or_zero(key, value)?,
                        "defence" => p.defence = flag(key, value)?,
                        "reputation_max" => p.reputation_max = whole_u32(key, value)?,
                        "report_budget" => p.report_budget = whole_u32(key, value)?,
                        other => return Err(unknown(other, MA_LEGACY_PARAM_KEYS)),
                    }
                }
                Ok(MaHost::Legacy(LegacyWindow::new(p)))
            }
            MA_CORROBORATED => {
                let mut p = v2xw_threat::CorroborationParams::default();
                for (key, value) in map {
                    match key.as_str() {
                        "event_window_s" => p.event_window_s = seconds(key, value)?,
                        "event_min_reporters" => p.event_min_reporters = count(key, value)?,
                        "min_events" => p.min_events = count(key, value)?,
                        "window_s" => p.window_s = seconds(key, value)?,
                        "min_reporters" => p.min_reporters = count(key, value)?,
                        "defence" => p.defence = flag(key, value)?,
                        "reputation_max" => p.reputation_max = whole_u32(key, value)?,
                        "report_budget" => p.report_budget = whole_u32(key, value)?,
                        other => return Err(unknown(other, MA_CORROBORATED_PARAM_KEYS)),
                    }
                }
                Ok(MaHost::Corroborated(v2xw_threat::CorroboratedMa::new(p)))
            }
            other => Err(conflict(
                "detection.ma",
                format!(
                    "'{other}' is not an authority pipeline this build ships; one of: {}",
                    MA_PIPELINES.join(", ")
                ),
            )),
        }
    }
}

/// The parameters `threat/ma/legacy-window` takes (`detection.ma.params`).
pub const MA_LEGACY_PARAM_KEYS: &[&str] = &[
    "report_threshold_k",
    "revoke_min_seconds",
    "revoke_persist_s",
    "revoke_window_s",
    "defence",
    "reputation_max",
    "report_budget",
];

/// The parameters `threat/ma/corroborated` takes (`detection.ma.params`).
pub const MA_CORROBORATED_PARAM_KEYS: &[&str] = &[
    "event_window_s",
    "event_min_reporters",
    "min_events",
    "window_s",
    "min_reporters",
    "defence",
    "reputation_max",
    "report_budget",
];

/// How the SCMS's device numbering is kept clear of the engine's.
pub const SCMS_DEVICE_BASE: u32 = 1_000_000;

/// The mast height a roadside unit's antenna stands at above its ground position, metres.
pub const RSU_MAST_HEIGHT_M: f64 = 6.0;

/// The hardware profile a roadside unit runs on when the scenario names none.
pub const DEFAULT_RSU_PROFILE: &str = "rsu/commsignia-its-rs4";

/// How often a reporter may report one subject again, by default: the legacy engine
/// reported every step its detectors fired, at `PipelineConfig.dt` = 1.0 s.
pub const REPORT_INTERVAL_S: f64 = 1.0;

/// How often a cellular vehicle asks the CRL Store for the current list, by default.
///
/// **ASSUMPTION inside a cited bound** (constants.md). CAMP has the device fetch "as often
/// as possible but no later than once a week" [CAMP122 §3 item 10] and "on every RA
/// connection" [PRIMER pp.5, 8]; neither prints an interval, which an OEM configures. One
/// hour is well inside the weekly bound and is the profile's own polling granularity for
/// the RA (`ra-minWait` 1 h, [SMP] §2.1.21 Table 3); a scenario that measures revocation
/// latency states its own.
pub const CRL_FETCH_INTERVAL_S: f64 = 3_600.0;

/// How often a roadside unit with the `crl` role repeats the list it holds, by default.
///
/// **Cited bound, ASSUMED mechanism** (constants.md). No document specifies a roadside
/// CRL broadcast (the Primer names the path, p.5; TS 102 941 Annex D.3 gives the delta
/// CTL no rate). A US RSU repeats a back-office message as Store and Repeat, whose
/// `TxInterval` allows 1 to 5 seconds [USDOT RSU Specification v4.1, 2016, Appendix C,
/// p.117]; 5 s is the longest the RSU allows, the smallest share of the channel.
pub const CRL_BROADCAST_INTERVAL_S: f64 = 5.0;

/// Whether the engine puts an attack kind on the air; a scenario naming one it does not
/// is refused (see [`unrendered`]).
#[must_use]
pub fn renders_on_air(kind: AttackKind) -> bool {
    unrendered(kind).is_none()
}

/// Every attacker kind of the catalogue by name, with why the engine refuses it (`None`
/// for a kind it renders on the air). The agent's settings search reads this rather than
/// a copied list, so a kind that becomes runnable shows up without an edit elsewhere.
#[must_use]
pub fn attack_kinds() -> Vec<(&'static str, Option<&'static str>)> {
    AttackKind::ALL
        .iter()
        .map(|k| (k.as_str(), unrendered(*k)))
        .collect()
}

/// The keys `threats.attackers[].params` accepts, with the kind that uses each one
/// (`None`: every kind; only the Sybil keys are refused on another kind). The same set the params parser below matches; the agent lists
/// it so "5 Sybil ghosts" lands on `params.sybil_ghosts` (F2MD `SybilVehNumber`).
pub const ATTACKER_PARAMS: [(&str, Option<&str>); 33] = [
    ("intensity", None),
    ("dt_s", None),
    ("magnitude_scale", None),
    ("dos_burst", None),
    ("delay_s", Some("DelayedMessages")),
    ("expired_cert_lag_s", Some("ExpiredCert")),
    ("not_yet_valid_lead_s", Some("NotYetValid")),
    ("const_pos_offset_m", Some("ConstPosOffset")),
    ("random_pos_half_range_m", Some("RandomPos")),
    ("teleport_dx_m", Some("Teleport")),
    ("teleport_dy_m", Some("Teleport")),
    ("teleport_period_s", Some("Teleport")),
    ("sine_pos_amplitude_m", Some("SineWavePos")),
    ("sine_pos_omega_rad_s", Some("SineWavePos")),
    ("const_speed_offset_mps", Some("ConstSpeedOffset")),
    ("random_speed_max_mps", Some("RandomSpeed")),
    ("stop_and_go_speed_mps", Some("StopAndGo")),
    ("heading_offset_deg", Some("HeadingOffset")),
    ("replay_lag_samples", Some("DataReplay")),
    ("slow_drift_ramp_mps", Some("SlowDrift")),
    ("slow_drift_max_rate_mps", Some("SlowDrift")),
    ("along_road_offset_m", Some("AlongRoadOffset")),
    ("disruptive_pos_half_range_m", Some("Disruptive")),
    ("disruptive_speed_up_mps", Some("Disruptive")),
    ("disruptive_speed_down_mps", Some("Disruptive")),
    ("disruptive_heading_deg", Some("Disruptive")),
    (
        "pos_speed_inconsistent_drop_mps",
        Some("PosSpeedInconsistent"),
    ),
    ("pos_heading_swing_m", Some("PosHeadingInconsistent")),
    ("pos_heading_omega_rad_s", Some("PosHeadingInconsistent")),
    ("eventual_stop_delay_s", Some("EventualStop")),
    ("eventual_stop_base_speed_mps", Some("EventualStop")),
    ("sybil_ghosts", Some("Sybil")),
    ("sybil_jitter_m", Some("Sybil")),
];

/// Why the engine cannot render an attack kind on the air, or `None` when it can.
///
/// The host applies an attacker's claimed position, speed, heading and generation time,
/// a forged signature and a certificate outside its validity window. What the kinds below
/// change is none of those, and an attack that is counted as lying while nothing it does
/// reaches a receiver reads as an attack nobody detected; so they are refused by name.
pub(crate) fn unrendered(kind: AttackKind) -> Option<&'static str> {
    match kind {
        AttackKind::DoS | AttackKind::DoSRandom => Some(
            "a flood is extra transmissions per generation interval, and the host sends one \
             frame per generation",
        ),
        AttackKind::VruImpersonation | AttackKind::VruPositionSpoof => Some(
            "it changes the declared station type, which a BSM does not carry; the host does \
             not switch a vehicle to sending PSMs",
        ),
        AttackKind::FakeHazard => Some("the host does not send an attacker's event messages"),
        AttackKind::SelectiveDrop => Some("a vehicle here relays nothing for it to drop"),
        _ => None,
    }
}

/// How far a receiver's map search looks for a road, metres: a claim farther than this
/// from every motor-vehicle lane scores as this far, which is several times the map
/// check's tolerance, so the cap never turns a firing check into a quiet one.
const MAP_SEARCH_M: f64 = 100.0;

/// A receiver's map of the road network: the world's motor-vehicle lanes.
///
/// The position-plausibility check of the F2MD framework (Kamel et al., *Simulation
/// framework for misbehavior detection in vehicular networks*, IEEE T-VT 69(6), 2020)
/// asks whether a claimed position lies on a road of the receiver's own map; a
/// navigation-grade map of the streets is what a production vehicle carries. The
/// distance is to the nearest motor-vehicle lane's edge (its centreline less half its
/// width), so a claim anywhere on the carriageway is on the road.
struct RoadMap<'a> {
    world: &'a World,
    vehicles: v2xw_world::model::ClassMask,
}

impl<'a> RoadMap<'a> {
    fn new(world: &'a World) -> Self {
        Self {
            world,
            vehicles: Self::vehicles(),
        }
    }

    /// The classes whose lanes are "the road" to the map check: motor vehicles.
    fn vehicles() -> v2xw_world::model::ClassMask {
        use v2xw_world::model::ClassMask;
        ClassMask::CAR
            .union(ClassMask::TRUCK)
            .union(ClassMask::BUS)
            .union(ClassMask::MOTO)
            .union(ClassMask::EMERGENCY)
    }
}

impl v2xw_threat::obs::LocalEnvironment for RoadMap<'_> {
    fn distance_to_road_m(&self, x_m: f64, y_m: f64) -> f64 {
        self.world
            .nearest_lane_within(Vec3::new(x_m, y_m, 0.0), MAP_SEARCH_M, Some(self.vehicles))
            .map_or(MAP_SEARCH_M, |hit| {
                let half = 0.5 * self.world.lane(hit.pos.lane).width_m;
                (hit.distance_m - half).max(0.0)
            })
    }
}

/// The SCMS device id of an engine node.
fn device_of(node: NodeId) -> NodeId {
    NodeId::new(SCMS_DEVICE_BASE + node.index())
}

/// The link-layer source address a pseudonym is sent under: six octets of its digest,
/// with the locally-administered bit set and the group bit clear (IEEE 802 §8.2), so the
/// address changes exactly when the certificate does.
#[must_use]
pub fn l2_address(digest: &[u8]) -> [u8; 6] {
    let mut a = [0u8; 6];
    for (k, b) in a.iter_mut().enumerate() {
        *b = digest.get(k + 2).copied().unwrap_or(0);
    }
    a[0] = (a[0] & 0xFC) | 0x02;
    a
}

/// One credential the SCMS provisioning issued, as the engine installs it on the air.
#[derive(Debug, Clone)]
pub struct ProvisionedCred {
    /// The i-period.
    pub i: u32,
    /// The index within the period.
    pub j: u32,
    /// The linkage value the certificate's `linkageData` carries.
    pub lv: LinkageValue,
    /// The start of the certificate's validity window.
    pub valid_from: SimTime,
    /// Its end.
    pub valid_until: SimTime,
    /// What the device derived from its download on the real 1609.2.1 path: the implicit
    /// certificate the ACA issued, the ACA's certificate and `d = e·b + r` (WP-C1). `None`
    /// on the paths that issue no SPDU (the ETSI tickets, the modelled explicit path under
    /// a hybrid or KEM scheme): those are issued by [`crate::credentials`] instead, keeping
    /// this linkage value.
    pub key: Option<std::sync::Arc<v2xw_sec::credential::ImplicitKey>>,
}

/// One RSU application certificate the SCMS issued, as the engine installs it.
#[derive(Debug, Clone)]
pub struct RsuAppCred {
    /// Its local period (the i-period on the real calendar).
    pub period: u32,
    /// The start of its validity, run time.
    pub valid_from: SimTime,
    /// Its end (exclusive): the period's start plus 169 h.
    pub valid_until: SimTime,
    /// The certificate, its issuing ACA and `d`, from the unit's own download.
    pub key: std::sync::Arc<v2xw_sec::credential::ImplicitKey>,
    /// Issued before the run.
    pub preloaded: bool,
}

/// A roadside unit's place in the application-certificate cycle.
#[derive(Debug, Clone, Default)]
struct RsuCycle {
    handed: usize,
    /// The composite CRL version the unit last installed.
    crl_version: u32,
    withheld_seen: u64,
    starved: bool,
    starved_since: Option<SimTime>,
    starved_last: SimTime,
}

/// One roadside unit the scenario declared.
#[derive(Debug, Clone)]
pub struct RsuSpec {
    /// Where it stands, world-local metres — the antenna phase centre.
    pub position: Vec3,
    /// What it does: `crl`, `report-forward`, `provisioning-proxy`, …
    pub roles: Vec<String>,
    /// Its hardware profile id.
    pub profile: String,
    /// Its backhaul.
    pub backhaul: Backhaul,
}

impl RsuSpec {
    /// Whether this unit carries a role.
    #[must_use]
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}

/// The credential lifecycle's parameters (`security.protocol`).
#[derive(Debug, Clone, Copy)]
pub struct LifecycleParams {
    /// The backend deployment's parameters.
    pub scms: ScmsParams,
    /// Certificates per i-period (`certs_per_period`, 20 [CAMP-EE Table 2.1.2.6.2]).
    pub jmax: u32,
    /// i-periods a vehicle holds at the start (`pool_periods`).
    pub pool_periods: u32,
    /// A vehicle asks for the next period when fewer than this many periods, counting the
    /// current one, remain in its pool (`topup_below_periods`).
    pub topup_below_periods: u32,
    /// How often a cellular vehicle polls the CRL Store.
    pub crl_fetch_interval: Duration,
    /// How often a `crl` unit repeats the list.
    pub crl_broadcast_interval: Duration,
    /// ETSI only: top up with one butterfly authorization request and a batch download
    /// (TS 102 941 V2 §6.2.3.5) instead of one standard request per ticket.
    pub etsi_butterfly: bool,
    /// How often the backend's state is published for the Backend view.
    pub view_interval: Duration,
    /// When the EAC decides the home provider's root rollover (`root_rollover_at_s`):
    /// a new root, ICA and component certificates, and a CTL adding the new root,
    /// published at the SLA deadline (3 business days, Provider Req. v1.1 §3.4.11).
    pub root_rollover_at: Option<SimTime>,
    /// When the EAC removes the superseded root (`root_removal_at_s`): a CTL without it,
    /// published within 48 h (Elector Policy v1.0 §2.2.4).
    pub root_removal_at: Option<SimTime>,
}

/// The US i-period: one week, 10,080 minutes [CAMP-EE §2.1.5.3.2; SMP §2.1.17 Table 2].
pub const US_I_PERIOD_S: u64 = 7 * 86_400;

fn secs(v: f64) -> Duration {
    Duration::from_nanos((v * 1e9).round().max(0.0) as u64)
}

impl LifecycleParams {
    /// The parameters a scenario states, over the cited defaults.
    ///
    /// # Errors
    /// [`EngineError::Scenario`] for a key this build does not read or a value out of range.
    pub fn from_scenario(scenario: &Scenario) -> Result<LifecycleParams> {
        let mut scms = ScmsParams::default();
        scms.master_seed = scenario.seed;
        // A run publishes the CRL on the deployment's cadence: that wait is part of what
        // revocation costs (05-protocols §2.5), and `crl_cadence_s: 0` turns it off.
        scms.publish_on_cadence = true;
        // Every Time32 the SCMS puts in a certificate or a CRL is read on the scenario's
        // own wall clock (`pdu.t0_unix_s`, set from `time.t0` below with the calendar):
        // WP-C2's receivers refuse a message generated outside its signer's validity.
        // WP-C2: the pseudonym certificates permit every PSID the fleet signs under —
        // what the scenario's vehicles send (an emergency vehicle's SRM included) and
        // the misbehaviour report — so a receiver checking the SPDU PSID against
        // `appPermissions` refuses no honest message.
        {
            let etsi = crate::wiring::etsi_facilities(scenario);
            let s = crate::wiring::vehicle_services(
                scenario,
                v2xw_mobility::classes::VehicleClass::Emergency,
            );
            let mut psids = vec![
                v2xw_msg::registry::psid(v2xw_msg::MsgType::Bsm, etsi),
                v2xw_msg::registry::psid(v2xw_msg::MsgType::Mbr, etsi),
            ];
            for (on, m) in [
                (s.bsm, v2xw_msg::MsgType::Bsm),
                (s.cam, v2xw_msg::MsgType::Cam),
                (s.denm, v2xw_msg::MsgType::Denm),
                (s.cpm, v2xw_msg::MsgType::Cpm),
                (s.srm, v2xw_msg::MsgType::Srm),
            ] {
                if on {
                    psids.push(v2xw_msg::registry::psid(m, etsi));
                }
            }
            scms.pdu.pseudonym_psids = v2xw_proto::scms::params::PsidList::of(&psids);
        }
        let mut p = LifecycleParams {
            scms,
            jmax: scms.certs_per_period,
            pool_periods: v2xw_proto::scms::DEVICE_POOL_PERIODS,
            topup_below_periods: 1,
            crl_fetch_interval: secs(CRL_FETCH_INTERVAL_S),
            crl_broadcast_interval: secs(CRL_BROADCAST_INTERVAL_S),
            etsi_butterfly: false,
            view_interval: secs(BACKEND_VIEW_INTERVAL_S),
            root_rollover_at: None,
            root_removal_at: None,
        };
        let empty = serde_json::Map::new();
        let map = scenario
            .security
            .protocol
            .as_ref()
            .and_then(|c| c.params.as_object())
            .unwrap_or(&empty);
        for (key, value) in map {
            let v = value.as_f64().ok_or_else(|| {
                conflict(
                    &format!("security.protocol.params.{key}"),
                    format!("must be a number, got {value}"),
                )
            })?;
            if !(v.is_finite() && v >= 0.0) {
                return Err(conflict(
                    &format!("security.protocol.params.{key}"),
                    format!("must be a finite number ≥ 0, got {v}"),
                ));
            }
            match key.as_str() {
                "i_period_s" => p.scms.i_period = secs(v.max(1.0)),
                "cert_lifetime_s" => p.scms.cert_lifetime = secs(v.max(1.0)),
                "certs_per_period" => {
                    p.jmax = (v.round() as u32).clamp(1, 60);
                    p.scms.certs_per_period = p.jmax;
                }
                "pool_periods" => p.pool_periods = (v.round() as u32).clamp(1, 64),
                "topup_below_periods" => p.topup_below_periods = (v.round() as u32).min(64),
                "cert_shuffle_window_s" => p.scms.shuffle_window = secs(v),
                "report_shuffle_window_s" => p.scms.report_shuffle_window = secs(v),
                "cert_shuffle_threshold" => {
                    p.scms.shuffle_threshold = (v.round() as u32).max(1);
                }
                "report_shuffle_threshold" => {
                    p.scms.report_shuffle_threshold = (v.round() as u32).max(1);
                }
                "crl_cadence_s" => {
                    p.scms.crl_cadence = secs(v);
                    p.scms.publish_on_cadence = v > 0.0;
                }
                "crl_fetch_interval_s" => p.crl_fetch_interval = secs(v.max(0.1)),
                "crl_broadcast_interval_s" => p.crl_broadcast_interval = secs(v.max(0.1)),
                "first_batch_delay_s" => p.scms.first_batch_delay = secs(v),
                "download_poll_interval_s" => p.scms.download_poll_interval = secs(v.max(0.1)),
                "enrolment_lifetime_s" => p.scms.enrolment_lifetime = secs(v.max(1.0)),
                "reenrol_lead_s" => p.scms.reenrol_lead = secs(v),
                "reenrol_download_delay_s" => p.scms.reenrol_download_delay = secs(v),
                "max_periods_ahead" => {
                    p.scms.max_periods_ahead = (v.round() as u32).max(1);
                }
                "etsi_butterfly" => p.etsi_butterfly = v >= 0.5,
                "root_rollover_at_s" => p.root_rollover_at = Some(secs(v).as_nanos()),
                "root_removal_at_s" => p.root_removal_at = Some(secs(v).as_nanos()),
                // The MA operator's review before a revocation is carried out (gap row
                // 28): the CV-pilot manual MA by default (8 h, an assumption,
                // `MaMode::ManualReview`); 0 is the automatic MA.
                "ma_review_s" => {
                    p.scms.ma.mode = if v > 0.0 {
                        v2xw_proto::scms::params::MaMode::ManualReview { review: secs(v) }
                    } else {
                        v2xw_proto::scms::params::MaMode::Automatic
                    };
                }
                other => {
                    return Err(conflict(
                        "security.protocol.params",
                        format!(
                            "'{other}' is not a lifecycle parameter; allowed: {}",
                            LIFECYCLE_KEYS.join(", ")
                        ),
                    ));
                }
            }
        }
        // The deployment's clock (gap rows 4 and 43; G8): every `Time32` a PDU carries
        // is derived from the scenario's own `time.t0`, and on the real weekly grid the
        // i-period counts from the US epoch, 2015-01-06 04:00 EST (03 §1 fact 4), so a
        // run starting 2026-10-16 provisions i = 614. A compressed grid (`i_period_s`
        // other than one week, which validation admits only under `time.calendar:
        // compressed`, WP-D2) keeps i-period 0 at the run's start: its numbers are not
        // the deployment's, and the manifest and the run report say so.
        if let Ok(wall) = v2xw_core::time::WallClock::parse_rfc3339(&scenario.time.t0) {
            p.scms.pdu.t0_unix_s = wall.t0_unix_s();
        }
        // WP-E1: the PKI view publishes every SPDU's bytes; the publisher drops them
        // again once a `pki.flow` record carries them (`Phase2::publish_pki`).
        p.scms.pdu.keep_bytes = pki_view_of(scenario);
        if scenario.time.calendar.is_real()
            && p.scms.i_period == v2xw_core::time::Duration::from_secs(US_I_PERIOD_S)
        {
            p.scms.calendar_epoch_unix_s =
                Some(v2xw_proto::scms::governance::US_IPERIOD_EPOCH_UNIX_S);
        } else if !map.contains_key("ma_review_s") {
            // A compressed calendar compresses the MA's operator with it: an 8-hour
            // review in a run whose week lasts a minute would only hide the protocol.
            // On the real calendar the CV-pilot manual review is the default (owner
            // decision); `ma_review_s` sets either explicitly.
            p.scms.ma.mode = v2xw_proto::scms::params::MaMode::Automatic;
        }
        if p.scms.cert_lifetime < p.scms.i_period {
            return Err(conflict(
                "security.protocol.params.cert_lifetime_s",
                "is shorter than i_period_s, which would leave a gap between two periods' \
                 certificates in which a vehicle can sign with nothing (CAMP-EE §2.1.5.3.2 \
                 makes the lifetime an hour longer than the period)"
                    .to_string(),
            ));
        }
        Ok(p)
    }

    /// The i-period `t` falls in.
    #[must_use]
    pub fn period_at(&self, t: SimTime) -> u32 {
        self.scms.period_at(t)
    }

    /// The instant i-period `i` begins: the inverse of [`LifecycleParams::period_at`].
    #[must_use]
    pub fn period_start(&self, i: u32) -> SimTime {
        self.scms.period_start(i)
    }

    /// When a device whose pool reaches `last_period` asks for its next batch: the start
    /// of the period at which only `topup_below_periods` periods remain, the condition
    /// the backend step tests ([`Phase2::topups_due`]'s `remaining <= below`). `None` when
    /// top-ups are turned off (`topup_below_periods: 0`).
    #[must_use]
    pub fn topup_at(&self, last_period: u32) -> Option<SimTime> {
        let below = self.topup_below_periods;
        (below > 0).then(|| self.period_start(last_period.saturating_add(1).saturating_sub(below)))
    }
}

/// The home provider's CRL publication rule on the real calendar (WP-D2): under the US
/// presets the CRL Generator publishes at the next business-day slot after a change, the
/// "within 1 business day" SLA ([SM-PROV] §3.4.9; 01 §1.4, 03 §9), in real Eastern time
/// with the federal holidays observed (`v2xw_proto::scms::calendar`); `camp-reference`
/// keeps the fixed daily cadence, and `on-decision` publishes at once. An explicit
/// `crl_cadence_s`, and every compressed calendar, keep the cadence the scenario states —
/// a business day has no meaning on a grid whose week lasts a minute.
fn apply_crl_publication(
    scenario: &Scenario,
    deployment: &v2xw_proto::scms::deployment::Deployment,
    p: &mut ScmsParams,
) {
    apply_crl_publication_of(scenario, deployment, 0, p);
}

/// Provider `k`'s CRL publication rule on its run's parameters `p` (WP-D1: each
/// provider's CRL Generator follows its own rule).
fn apply_crl_publication_of(
    scenario: &Scenario,
    deployment: &v2xw_proto::scms::deployment::Deployment,
    k: usize,
    p: &mut ScmsParams,
) {
    use v2xw_proto::scms::deployment::CrlPublication;
    let explicit = scenario
        .security
        .protocol
        .as_ref()
        .and_then(|c| c.params.get("crl_cadence_s"))
        .is_some();
    if explicit || p.calendar_epoch_unix_s.is_none() {
        return;
    }
    match deployment.providers.get(k).map(|h| h.crl_publication) {
        Some(CrlPublication::SlaOneBusinessDay) => p.crl.business_day_sla = true,
        Some(CrlPublication::OnDecision) => p.publish_on_cadence = false,
        Some(CrlPublication::Cadence) | None => {}
    }
}

/// The lifecycle keys `security.protocol.params` accepts.
pub const LIFECYCLE_KEYS: [&str; 22] = [
    "i_period_s",
    "cert_lifetime_s",
    "certs_per_period",
    "pool_periods",
    "topup_below_periods",
    "cert_shuffle_window_s",
    "report_shuffle_window_s",
    "cert_shuffle_threshold",
    "report_shuffle_threshold",
    "crl_cadence_s",
    "crl_fetch_interval_s",
    "crl_broadcast_interval_s",
    "first_batch_delay_s",
    "download_poll_interval_s",
    "enrolment_lifetime_s",
    "reenrol_lead_s",
    "reenrol_download_delay_s",
    "max_periods_ahead",
    "etsi_butterfly",
    "ma_review_s",
    "root_rollover_at_s",
    "root_removal_at_s",
];

/// How often, in simulated seconds, the backend's state is published on `backend.state`
/// for the Backend view: often enough to watch a queue fill, rarely enough that a
/// five-minute run records a few hundred snapshots.
pub const BACKEND_VIEW_INTERVAL_S: f64 = 1.0;

/// An armed attacker: the model, and the window it is allowed to act in.
struct AttackerSlot {
    model: Box<dyn Attacker>,
    id: String,
    from: SimTime,
    to: SimTime,
}

/// What the scenario asked for, before any node exists to be it.
#[derive(Debug, Clone)]
struct AttackerSpec {
    id: String,
    params: LegacyAttackerParams,
    count: Option<u32>,
    fraction: Option<f64>,
    actor_ids: BTreeSet<u32>,
    from: SimTime,
    to: SimTime,
}

/// The revocation, once the backend has issued it.
#[derive(Debug, Clone)]
pub struct Revocation {
    /// The CRL entry the CRL Generator issued.
    pub entry: CrlLinkageEntry,
    /// The decomposition 05-protocols §8 asks for, to the first vehicle that enforced.
    pub stages: Vec<(StageId, SimTime)>,
    /// Detection to first enforcement.
    pub latency: Duration,
    /// The CRL's size when this entry was published, bytes.
    pub bytes: u32,
    /// Which engine node was revoked.
    pub subject: NodeId,
    /// The certificate the authority decided on, hex.
    pub subject_digest: String,
}

/// One CRL version a provider's CRL Store published.
#[derive(Debug, Clone)]
struct CrlVersion {
    /// When it was published.
    at: SimTime,
    /// Its version: the store's entry count.
    version: u32,
    /// Its entries.
    entries: Vec<CrlLinkageEntry>,
    /// The signed CRL its CRL Generator issued (`None` with the PDU layer off).
    secured: Option<std::sync::Arc<[u8]>>,
}

/// The internet leg a device's backend message crosses after its access leg: the
/// scenario's city to the provider's region (WP-D1; ASSUMPTION, see
/// [`v2xw_proto::scms::deployment::Deployment::city_latency`]), added to whatever the
/// access model answers for a device link. Backend-internal links are untouched.
#[derive(Debug)]
struct InternetLeg {
    inner: v2xw_proto::net::SharedOracle,
    extra: Duration,
}

impl v2xw_proto::net::PathOracle for InternetLeg {
    fn carry(
        &mut self,
        from: NodeId,
        to: NodeId,
        link: &v2xw_proto::Link,
        bytes: u32,
        at: SimTime,
    ) -> Option<SimTime> {
        let base = self
            .inner
            .lock()
            .ok()
            .and_then(|mut o| o.carry(from, to, link, bytes, at));
        if link.transport == Transport::BackendNet {
            return base;
        }
        Some(
            base.unwrap_or_else(|| link.delay(bytes).after(at))
                .saturating_add(self.extra.as_nanos()),
        )
    }
}

/// A decision waiting for, or being carried out by, the backend.
#[derive(Debug, Clone)]
struct Case {
    /// The provider whose MA decides (the one that issued the accused certificate).
    provider: usize,
    subject: NodeId,
    subject_digest: String,
    /// The triggering report's backend run.
    report_run: FlowRun,
    /// The resolution and CRL-issuance runs, once started.
    runs: Option<(FlowRun, FlowRun)>,
    /// The entry, once issued, and the CRL version (entry count) it first appears in.
    entry: Option<(CrlLinkageEntry, u32)>,
    stamps_emitted: BTreeSet<(u8, SimTime)>,
    enforced_first: Option<SimTime>,
    /// Every vehicle that has installed the entry, in install order, with the instant
    /// (the PKI view's per-vehicle `enforced` stamps).
    enforcers: Vec<(NodeId, SimTime)>,
}

/// An ETSI decision carried out as the EA's blocklist.
#[derive(Debug, Clone)]
struct Block {
    subject: NodeId,
    subject_digest: String,
    run: FlowRun,
    detected: SimTime,
    decided: SimTime,
    blocked: Option<SimTime>,
}

/// A report on its way to the authority, keyed by its backend run.
#[derive(Debug, Clone)]
struct InFlightReport {
    report: MisbehaviourReport,
    subject: NodeId,
}

/// A device's misbehaviour-report store, [SM-MBR] §2.2 ([`MbrQueue`]): each report
/// weighted 1.0, multiplied by 0.9 every day at 05:00 Eastern; uploaded highest weight
/// first (newer first on ties); over the quota the lowest weight is deleted (older first);
/// unsent reports older than a week are deleted.
#[derive(Debug, Clone)]
struct ReportStore(MbrQueue<(MisbehaviourReport, SimTime)>);

/// The device's report-storage quota, bytes. [SM-MBR] §2.2 leaves it device-specific;
/// **assumption**: 64 KiB, 58 reports at the reference report's 1,119 B
/// ([`report_bytes`]).
pub const REPORT_STORE_QUOTA_BYTES: u64 = 64 * 1024;

impl Default for ReportStore {
    fn default() -> Self {
        ReportStore(MbrQueue::new(REPORT_STORE_QUOTA_BYTES))
    }
}

/// The evidence a reporter attaches: the last signed BSMs heard from each certificate.
const EVIDENCE_BSMS: usize = 2;

/// One vehicle's security and backend state.
#[derive(Debug, Clone, Default)]
struct NodeSec {
    access: Option<AccessKind>,
    /// Reports waiting for connectivity, with the instant each was detected.
    outbox: ReportStore,
    reports_uploaded: u32,
    crl_version: u32,
    /// The CRL entries this vehicle holds, so a list arriving in several frames, or by two
    /// paths, is installed once.
    crl_installed: Vec<CrlLinkageEntry>,
    next_crl_fetch: SimTime,
    crl_fetch_in_flight: bool,
    topup: Option<FlowRun>,
    /// ETSI: the ticket count at which the requested top-up is complete.
    etsi_target: Option<u32>,
    /// The last i-period this vehicle holds certificates for.
    last_period: u32,
    changes_seen: u32,
    active_digest: Option<[u8; 8]>,
    self_revoked: bool,
    starved: bool,
    /// When this vehicle's enrolment certificate expires.
    enrolment_until: SimTime,
    /// A successor enrolment in flight.
    reenrol: Option<FlowRun>,
    /// The RA refused it as blocklisted: it asks for nothing more.
    blocked: bool,
    /// ETSI butterfly: the i-period of a batch waiting at the EA, and whether its
    /// download has been asked for.
    etsi_batch: Option<(u32, bool)>,
    /// ETSI: when this station next asks the Distribution Centre for the ECTL and the
    /// CA-CRL, and the fetch in flight.
    next_trust_fetch: SimTime,
    /// The fetch in flight, with the ECTL and CA-CRL sequences held when it started.
    trust_fetch: Option<(FlowRun, u32, u32)>,
    /// When the top-up, the successor enrolment and the trust-list fetch in flight were
    /// started, for the per-exchange latency.
    topup_started: SimTime,
    /// See [`NodeSec::topup_started`].
    reenrol_started: SimTime,
    /// See [`NodeSec::topup_started`].
    trust_started: SimTime,
    /// The node step at which the current spell without a valid pseudonym began, and the
    /// last starved step seen.
    starved_since: Option<SimTime>,
    /// See [`NodeSec::starved_since`].
    starved_last: SimTime,
}

/// Counters the run report carries for this path.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Phase2Report {
    /// How many roadside units were created.
    pub rsus: u64,
    /// Roadside units in the SCMS application-certificate cycle (WP-B3b): each holds
    /// only weekly certificates its provider's ACA issued and renews them over its own
    /// backhaul.
    pub rsus_in_cert_cycle: u64,
    /// RSU application certificates installed before the run (this week's, and next
    /// week's for a unit whose backhaul was up).
    pub rsu_app_certs_preloaded: u64,
    /// RSU application certificates renewed during the run, over the unit's backhaul.
    pub rsu_app_certs_renewed: u64,
    /// RSU application certificates the node refused to install.
    pub rsu_app_certs_refused: u64,
    /// Roadside units that held no valid application certificate at some step: they
    /// sign no SPaT, MAP or CRL frame then (00 gap row 24, the Wyoming starvation).
    pub rsus_starved: u64,
    /// Roadside-unit steps spent without a valid application certificate.
    pub rsu_starved_steps: u64,
    /// Time the starving units spent without one, summed over units, ns.
    pub rsu_starved_ns: u64,
    /// Messages roadside units had due and did not send for want of a valid
    /// certificate (SPaT and MAP; [CAMP-EE §2.2.10.2]: no unsigned transmission).
    pub rsu_messages_withheld: u64,
    /// CRL frames a starving unit could not sign and so did not broadcast.
    pub rsu_crl_frames_withheld: u64,
    /// Composite CRLs roadside units fetched from their RA over the backhaul and
    /// installed after verifying every signed CRL themselves (WP-C2 rules).
    pub rsu_crls_installed: u64,
    /// Composite CRLs a roadside unit refused (a CRL that did not verify).
    pub rsu_crls_refused: u64,
    /// Received envelopes that would not parse at all.
    pub spdu_parse_failures: u64,
    /// Received envelopes that parsed but whose signature did not verify.
    pub spdu_signature_failures: u64,
    /// How many received messages landed in each node verification state.
    pub verification_states: std::collections::BTreeMap<String, u64>,
    /// How many times each detector fired, by detector id.
    pub verdicts_by_detector: std::collections::BTreeMap<String, u64>,
    /// How many nodes were armed as attackers.
    pub attackers: u64,
    /// How many outgoing claims an attacker falsified.
    pub falsified_claims: u64,
    /// Sybil ghost frames put on the air: each a genuine SPDU signed under one of the
    /// attacker's concurrent pseudonyms, with its certificate attached.
    pub ghost_frames: u64,
    /// How many messages the local detectors checked.
    pub messages_checked: u64,
    /// How many detector verdicts fired.
    pub verdicts_fired: u64,
    /// How many misbehaviour reports were filed.
    pub reports_sent: u64,
    /// Of those, uploaded over a cellular modem.
    pub reports_uploaded_cellular: u64,
    /// Of those, put on the air to a relaying roadside unit.
    pub reports_uploaded_relay: u64,
    /// Reports roadside units filed themselves, straight onto their backhaul.
    pub reports_from_rsus: u64,
    /// Reports still in an outbox at the end of the run, for want of connectivity.
    pub reports_unsent: u64,
    /// Reports a device's store deleted to stay within its quota ([SM-MBR] §2.2).
    pub reports_evicted: u64,
    /// Reports a device's store deleted unsent after a week.
    pub reports_expired: u64,
    /// Reports sent as real US misbehaviour reports (SM-MBR v1.1: signed under PSID
    /// 0x26 with the reporter's pseudonym, encrypted to the MA, uploaded with an
    /// enrolment-signed `Download-Req`), their evidence the accused's own signed BSMs.
    pub reports_mbr_real: u64,
    /// Reports whose detection has no PSID-0x20 type and subtype in the v1.1 object
    /// set (or ObsImplausible, off by default), sent on the modelled path.
    pub reports_mbr_unmapped: u64,
    /// Reports a device could not secure (no evidence heard, no certificate or key),
    /// sent on the modelled path; see `scms.state.pdu_rejections` for the backend's.
    pub reports_mbr_unsecured: u64,
    /// EAC decisions the run scheduled and took (root rollover, root removal).
    pub gov_decisions: u64,
    /// CTLs the governance entities published during the run (after the genesis CTL).
    pub ctls_published: u64,
    /// [`Phase2Report::reports_mbr_unsecured`] by reason.
    pub reports_mbr_unsecured_by: BTreeMap<String, u64>,
    /// Reports an MA shared with the provider that issued the accused certificate
    /// (WP-D1, inter-provider report sharing).
    pub reports_shared: u64,
    /// Reports an MA held about another provider's certificate and could neither
    /// resolve nor share (`ma_report_sharing: false`).
    pub reports_foreign_unresolvable: u64,
    /// SCMS devices each provider enrolled, by provider name (the seeded fleet split).
    pub devices_by_provider: BTreeMap<String, u64>,
    /// CRL entries each provider's CRL Generator issued, by provider name.
    pub revocations_by_provider: BTreeMap<String, u64>,
    /// Reports that reached a reporter's privacy proxy, by `"<reporter's provider><-<the
    /// accused's provider>"`.
    pub reports_by_provider: BTreeMap<String, u64>,
    /// Messages receivers verified, by `"<receiver's provider><-<signer's provider>"`.
    pub verified_by_provider: BTreeMap<String, u64>,
    /// Messages receivers refused because a CRL revoked the signer, keyed the same way.
    pub revoked_by_provider: BTreeMap<String, u64>,
    /// Reports the access link lost.
    pub reports_lost: u64,
    /// How many reached the Location Obscurer Proxy.
    pub reports_received: u64,
    /// How many the authority ingested, after the RA's shuffle.
    pub reports_at_ma: u64,
    /// How many revocation decisions the authority's pipeline took.
    pub ma_revoke_decisions: u64,
    /// How many investigations the backend opened.
    pub cases_opened: u64,
    /// How many ended without an entry.
    pub cases_unresolved: u64,
    /// How many CRL entries the backend issued.
    pub crls_issued: u64,
    /// How many CRL versions the CRL Store published.
    pub crl_versions_published: u64,
    /// How many CRL frames roadside units put on the air.
    pub crl_broadcasts: u64,
    /// How many CRL downloads cellular vehicles completed.
    pub crl_downloads: u64,
    /// Issued revocations whose publication the run horizon cut off.
    pub crl_past_horizon: u64,
    /// How many node installations of a CRL entry.
    pub crls_installed: u64,
    /// How many receptions the installed CRL caused to be classified revoked, on every
    /// node (vehicles and roadside units).
    pub revoked_receptions: u64,
    /// The part of `revoked_receptions` on roadside units: under the SCMS a unit installs
    /// its RA's composite CRL over its own backhaul (WP-B3b cycle), a path that reaches no
    /// vehicle.
    pub rsu_revoked_receptions: u64,
    /// Detection to the first vehicle enforcing, ns, for the first revocation.
    pub revocation_latency_ns: u64,
    /// That revocation's stages, `(stage, ns since detection)`.
    pub revocation_stages: Vec<(String, u64)>,
    /// Certificate top-ups started.
    pub topups_started: u64,
    /// Top-ups whose batch was installed.
    pub topups_completed: u64,
    /// Certificates installed by top-ups.
    pub certs_topped_up: u64,
    /// Top-ups the RA refused: blocklisted, or the enrolment certificate had expired.
    pub topups_refused: u64,
    /// Of those, refused because the enrolment was blocklisted (passive revocation).
    pub topups_refused_blocklisted: u64,
    /// Successor enrolments started, completed and refused.
    pub reenrolments_started: u64,
    /// See [`Phase2Report::reenrolments_started`].
    pub reenrolments_completed: u64,
    /// See [`Phase2Report::reenrolments_started`].
    pub reenrolments_refused: u64,
    /// CRL frames a vehicle discarded because their entries were not the CRL
    /// Generator's signed list (a compromised roadside unit's forgery).
    pub crl_frames_rejected: u64,
    /// ETSI: fetches of the ECTL and the CA-CRL from the Distribution Centre that
    /// completed, and of those, how many installed a newer list.
    pub trust_fetches: u64,
    /// See [`Phase2Report::trust_fetches`].
    pub trust_lists_installed: u64,
    /// Roadside units under an attacker's control (`threats.compromised_rsus`).
    pub compromised_rsus: u64,
    /// Relayed reports a compromised unit dropped instead of forwarding.
    pub rsu_reports_suppressed: u64,
    /// Relayed reports a compromised unit re-targeted at an innocent subject.
    pub rsu_reports_poisoned: u64,
    /// CRL frames a compromised unit put on the air with fabricated entries.
    pub rsu_crl_frames_forged: u64,
    /// Pseudonym changes, all vehicles.
    pub pseudonym_changes: u64,
    /// Links the passive observer claimed across a pseudonym change.
    pub privacy_links_claimed: u64,
    /// Of those, links between two pseudonyms of one vehicle (the ground-truth join).
    pub privacy_links_correct: u64,
    /// Safety frames the eavesdropper read (all of them without `threats.eavesdropper`).
    pub privacy_frames_read: u64,
    /// New pseudonyms the eavesdropper heard for the first time: its linkage decisions.
    pub privacy_link_decisions: u64,
    /// The anonymity-set sizes |Ψ| of those decisions, summed (mean = sum / decisions).
    pub privacy_anonymity_set_sum: u64,
    /// The degrees of anonymity d of those decisions, summed, in millionths.
    pub privacy_degree_micro_sum: u64,
    /// Vehicles the eavesdropper followed at all (ground truth): whose frames it read and
    /// attributed to one continuous track for some time.
    pub privacy_tracked_vehicles: u64,
    /// Of those, the vehicles it followed correctly across at least one pseudonym change.
    pub privacy_followed_across_change: u64,
    /// Over the followed vehicles, the longest time each was followed correctly — its
    /// tracking duration, within one pseudonym and across correctly linked changes —
    /// summed and at its largest, ns.
    pub privacy_tracked_sum_ns: u64,
    /// See [`Phase2Report::privacy_tracked_sum_ns`].
    pub privacy_tracked_max_ns: u64,
    /// Safety frames withheld by a silent period after a pseudonym change.
    pub pseudonym_silenced_frames: u64,
    /// Pseudonym changes the engine asked for under `c2c-cc` or `mix-zone`.
    pub pseudonym_changes_requested: u64,
    /// Vehicles that at some instant held no usable certificate and could not sign.
    pub vehicles_starved: u64,
    /// Node steps a vehicle spent unable to sign for want of a certificate.
    pub starved_node_steps: u64,
    /// Certificate availability: simulated time vehicles spent with no valid pseudonym,
    /// summed over vehicles, ns (measured between node steps).
    pub cert_unavailable_ns: u64,
    /// The longest single spell one vehicle spent with no valid pseudonym, ns.
    pub cert_unavailable_max_ns: u64,
    /// The access legs.
    pub access: crate::backend::AccessReport,
    /// Distinct armed attackers the backend revoked (ground truth).
    pub revoked_attackers: u64,
    /// Distinct honest devices the backend revoked (ground truth): false revocations.
    pub revoked_honest: u64,
    /// Revocations issued for a device an earlier one had already revoked: two of its
    /// pseudonyms decided before the first case reached the list. Counted once above.
    pub revocations_repeated: u64,
    /// Decisions about a certificate the published list already revoked.
    pub decisions_already_covered: u64,
    /// Revoke verdicts about a pseudonym of a device the authority had already decided:
    /// the linkage authorities put both pseudonyms on one linkage chain, so the device is
    /// revoked once and the verdict is not a second decision.
    pub decisions_linked: u64,
    /// The most corroborated events any pseudonym the authority did not revoke reached
    /// (`threat/ma/corroborated` only): the margin an honest fleet leaves to the threshold.
    pub ma_peak_unrevoked_events: u64,
    /// Vehicles the pre-run provisioning left with no pseudonym certificate at all.
    pub vehicles_unprovisioned: u64,
    /// Receptions the live nodes' revocation gates refused for a certificate period
    /// outside their plausibility window, at the end of the run (classified `Invalid`).
    pub crl_period_refusals: u64,
    /// Checked messages whose stated positional accuracy the detector used.
    pub accuracy_stated: u64,
    /// Checked messages that stated their accuracy "unavailable".
    pub accuracy_unavailable: u64,
    /// Checked messages the accuracy join had no entry for (a modelling gap, expected 0).
    pub accuracy_unjoined: u64,
    /// Reports filed whose subject was an armed attacker (ground truth, run report only):
    /// the report-level true positives.
    pub reports_about_attackers: u64,
    /// Reports filed whose subject was honest (ground truth): report-level false positives.
    pub reports_about_honest: u64,
    /// Distinct attackers at least one report named (ground truth): detection recall's
    /// numerator.
    pub attackers_reported: u64,
    /// Distinct honest vehicles at least one report named (ground truth).
    pub honest_reported: u64,
    /// Revocation decisions the authority took about an honest device (ground truth),
    /// counted at the decision and before the backend carried it out.
    pub decisions_honest: u64,
    /// Per attacker the authority decided to revoke: its first falsified claim to the
    /// first report about it, ns, in decision order.
    pub onset_to_detection_ns: Vec<u64>,
    /// Per attacker the authority decided to revoke: the first report about it to the
    /// authority's decision, ns, in decision order.
    pub detection_to_decision_ns: Vec<u64>,
    /// Backend deliveries the protocol kernel refused (modelling defects).
    pub backend_errors: u64,
    /// The first of them, for the report.
    pub first_backend_error: String,
    /// The `security.signature` scheme the credential system ran, and its mode
    /// (`classical`, `hybrid`, `post-quantum`).
    pub signature_scheme: String,
    /// See [`Phase2Report::signature_scheme`].
    pub signature_mode: String,
    /// The `security.kem` scheme.
    pub kem_scheme: String,
    /// Per committee role (`pca`, `crl`, `linkage`): sessions, rounds, bytes, restarts,
    /// timeouts, invalid shares, failures and latency (`v2xw_proto::threshold::session`).
    pub committees:
        std::collections::BTreeMap<String, v2xw_proto::threshold::session::CommitteeStats>,
    /// Committee sessions that failed for good, by role: certificates never issued, CRLs
    /// never signed, revocations never resolved.
    pub committee_failures: std::collections::BTreeMap<String, u64>,
    /// Backend deliveries dropped because their receiver was offline
    /// (`actors.backend.failures`).
    pub backend_deliveries_dropped: u64,
    /// One row per armed attacker, in node order (ground truth, for the evaluation only):
    /// how long it lied, what the fleet reported about it and what the authority made of
    /// it, so a missed attacker can be told apart from one that was barely on the air.
    pub attacker_outcomes: Vec<AttackerOutcome>,
}

/// What became of one armed attacker (ground truth, for the evaluation only).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct AttackerOutcome {
    /// The engine node.
    pub node: u32,
    /// The attack model it ran.
    pub attack: String,
    /// Its first falsified claim, ns of simulated time.
    pub onset_ns: Option<u64>,
    /// Its last falsified claim, ns: with the onset, how long it was on the air lying.
    pub last_falsified_ns: Option<u64>,
    /// Falsified claims it signed.
    pub falsified_claims: u64,
    /// The first report any receiver filed about it, ns.
    pub first_reported_ns: Option<u64>,
    /// Reports filed about it.
    pub reports: u64,
    /// Distinct pseudonyms of its that those reports named.
    pub pseudonyms_reported: u64,
    /// Reports about it that reached the authority.
    pub reports_at_ma: u64,
    /// The most corroborated events the authority held about any one of its pseudonyms
    /// inside the decision window.
    pub peak_events: u32,
    /// The authority's revocation decision, ns.
    pub decided_ns: Option<u64>,
}

/// The PKI view's records one backend tick produced (WP-E1).
#[derive(Debug, Default)]
pub struct PkiTick {
    /// `pki.state`.
    pub state: Option<v2xw_proto::scms::pki_view::PkiState>,
    /// `pki.flow`.
    pub flows: Vec<v2xw_proto::scms::pki_view::PkiFlow>,
    /// `pki.case`.
    pub cases: Vec<v2xw_proto::scms::pki_view::PkiCase>,
    /// `pki.knowledge`.
    pub knowledge: Vec<v2xw_proto::scms::pki_view::Knowledge>,
}

/// `pki.knowledge` is published every this many backend views: what an entity holds
/// changes slowly and each record lists rows.
pub const KNOWLEDGE_EVERY: u32 = 10;

/// Rows per list in a `pki.knowledge` record (totals are uncapped).
pub const KNOWLEDGE_ROWS: usize = 64;

/// What the engine does after a backend step: records to write and changes to apply.
#[derive(Debug, Default)]
pub struct BackendTick {
    /// Revocation stages to write on `proto.revocation`.
    pub stages: Vec<crate::sec_records::ProtoRevocation>,
    /// Reports the authority ingested.
    pub ma_reports: Vec<v2xw_threat::records::MaReportRecord>,
    /// Its decisions.
    pub ma_decisions: Vec<v2xw_threat::records::MaDecisionRecord>,
    /// A new CRL version was published at the store (its entry count).
    pub published: Option<u32>,
    /// Certificates a completed top-up installs, per node.
    pub installs: Vec<(NodeId, Vec<ProvisionedCred>, u64)>,
    /// Credential events to write on `sec.cert`: `(node, event)` — `topup-refused`,
    /// `reenrolled`, `reenrol-refused`.
    pub events: Vec<(NodeId, &'static str)>,
    /// The backend's state, when it is due for publication.
    pub view: Option<BackendView>,
    /// The PKI view's records (WP-E1), when it is on: the trust domain's state on the
    /// view's cadence, the messages, stage stamps and SPDUs since the last tick, the case
    /// board's transitions and, every [`KNOWLEDGE_EVERY`] views, what each entity holds.
    pub pki: PkiTick,
    /// Backend-network and backhaul bytes moved since the last tick:
    /// `(t, bucket, bytes, node)`. The sidelink's own bytes are not here: a relayed
    /// report's air hop is a frame on `node.tx` like any other.
    pub bytes: Vec<(
        SimTime,
        v2xw_metrics::channels::ByteBucket,
        u64,
        Option<NodeId>,
    )>,
}

/// The Phase 2 state of one run.
pub struct Phase2 {
    /// One SCMS run per provider of the deployment (WP-D1), in deployment order: the
    /// home provider's first. Read through [`Phase2::home`], [`Phase2::run_of`] and
    /// [`Phase2::run_of_mut`], never by index at a call site.
    runs: Vec<ScmsRun>,
    /// Which provider serves each SCMS device (by its device id), from the seeded fleet
    /// split ([`v2xw_proto::scms::deployment::Deployment::provider_of`]).
    device_provider: BTreeMap<NodeId, usize>,
    /// The scenario seed the fleet split is drawn under (the one roadside units use).
    fleet_seed: u64,
    /// Every CRL version each provider's CRL Store published: when, its entry count, its
    /// entries and the signed CRL. A provider's RA serves every provider's latest CRL
    /// once the inter-provider link has carried it (WP-D1; the composite CRL of
    /// 1609.2.1 §6.3.5.7).
    crl_feeds: Vec<Vec<CrlVersion>>,
    /// The backend's clock at its last step.
    backend_now: SimTime,
    /// Per provider, the internet leg from the scenario's city to its region that a
    /// device's backend message crosses after its access leg (zero under `flat`).
    internet_legs: Vec<Duration>,
    /// The ETSI ITS PKI, when `security.protocol` selects it; `scms` then carries nothing.
    etsi: Option<v2xw_proto::etsi::ts102941::EtsiRun>,
    /// ETSI decisions, carried out as blocklistings.
    blocks: Vec<Block>,
    params: LifecycleParams,
    access: BackendAccess,
    creds: BTreeMap<NodeId, Vec<ProvisionedCred>>,
    by_digest: BTreeMap<[u8; 8], (NodeId, u32, u32)>,
    rsus: Vec<RsuSpec>,
    rsu_nodes: Vec<NodeId>,
    /// The SCMS (`protocol/scms/camp`) runs this backend: roadside units then join the
    /// application-certificate cycle.
    camp: bool,
    /// Per roadside unit in that cycle: how many of its installed certificates the
    /// engine has handed to the node, and its starvation spell.
    rsu_cycle: BTreeMap<NodeId, RsuCycle>,
    /// `crl_delivery: mbms`: when the eMBMS carousel next repeats the current list.
    mbms_next: SimTime,
    specs: Vec<AttackerSpec>,
    attackers: BTreeMap<NodeId, AttackerSlot>,
    detectors: BTreeMap<NodeId, Legacy12>,
    detector_params: DetectorParams,
    detection_on: bool,
    /// Whether the detectors read each message's stated accuracy (`true`, the default) or
    /// the legacy constant 5 m (`detection.local[].params.use_stated_accuracy: 0`, kept to
    /// reproduce the runs before 2026-09-30).
    stated_accuracy: bool,
    /// Whether each receiver checks a claimed position against its own map of the road
    /// network (`true`, the default; `detection.local[].params.use_map: 0` is a receiver
    /// with no map, whose `mapOffRoad` check cannot fire).
    map_check: bool,
    report_interval: Duration,
    /// One Misbehaviour Authority per provider (WP-D1), in deployment order.
    ma: Vec<MaHost>,
    vehicles: u64,
    /// When each (reporter, subject) pair was last filed.
    filed: BTreeMap<(NodeId, String), SimTime>,
    /// When each receiver last ran its suite on each sender.
    checked_at: BTreeMap<(NodeId, [u8; 8]), SimTime>,
    forwarded: BTreeSet<String>,
    in_flight: BTreeMap<(usize, FlowRun), InFlightReport>,
    /// The home root a scheduled rollover superseded (`root_removal_at_s` removes it).
    superseded_root: Option<[u8; 32]>,
    /// The last signed BSMs on the air per signer digest: a report's evidence.
    evidence: BTreeMap<[u8; 8], VecDeque<Vec<u8>>>,
    stage_cursor: Vec<usize>,
    step_cursor: Vec<usize>,
    queue: Vec<VecDeque<Case>>,
    cases: Vec<Case>,
    /// Each provider's last published CRL version (its store's entry count).
    published: Vec<u32>,
    broadcast_version: u32,
    nodes: BTreeMap<NodeId, NodeSec>,
    revocation: Option<Revocation>,
    report: Phase2Report,
    /// The passive privacy observer: every safety frame on the air, linked across
    /// pseudonym changes by kinematics (07-threats-and-detection.md §6).
    observer: v2xw_threat::PrivacyObserver,
    /// Its link claims since the last backend step, for the recorder.
    link_claims: Vec<v2xw_threat::records::PrivacyLinkClaim>,
    /// The fold of the backend's wire log into edges, for the Backend view.
    tracker: EdgeTracker,
    /// The same for every other provider's run, in deployment order from provider 1.
    peer_trackers: Vec<EdgeTracker>,
    /// `threats.compromised_rsus`: the attack each compromised unit runs, by the unit's
    /// index in `actors.rsus`, with its active window.
    compromised_specs: BTreeMap<usize, (v2xw_threat::RsuAttackParams, SimTime, SimTime)>,
    /// The compromised units, once they exist, by node.
    compromised: BTreeMap<NodeId, v2xw_threat::CompromisedRsu>,
    /// Which node each compromised unit's spec belongs to, before its model is built.
    compromised_nodes: BTreeMap<NodeId, usize>,
    /// When the backend's state is next published.
    next_view: SimTime,
    /// The PKI view's publisher (WP-E1), when `security.scms.pki_view` is on and an SCMS
    /// runs: what each record has already carried.
    pki: Option<v2xw_proto::scms::pki_view::PkiTracker>,
    /// Views published, for the knowledge cadence.
    views: u32,
    /// Ground truth for the run report only, never read by a decision: each attacker's
    /// first falsified claim, each subject's first report, each subject's decision.
    first_falsified: BTreeMap<NodeId, SimTime>,
    first_reported: BTreeMap<NodeId, SimTime>,
    decided: BTreeMap<NodeId, SimTime>,
    /// Per attacker (ground truth, run report only): `(last falsified claim, falsified
    /// claims)`, reports filed about it and the pseudonyms they named, reports about it
    /// at the authority, and the most corroborated events it reached.
    attacker_lies: BTreeMap<NodeId, (SimTime, u64)>,
    attacker_reports: BTreeMap<NodeId, (u64, BTreeSet<[u8; 8]>)>,
    attacker_at_ma: BTreeMap<NodeId, u64>,
    attacker_peak_events: BTreeMap<NodeId, u32>,
    /// The devices a revocation has been issued or decided for (ground truth, run report
    /// only), so a device two of whose pseudonyms were decided is counted once.
    revoked_devices: BTreeSet<NodeId>,
    /// The devices the authority has decided to revoke, as the PCA and linkage
    /// authorities resolve a reported pseudonym (joint 2): one decision per device.
    ma_decided_devices: BTreeSet<NodeId>,
    /// The positional accuracy each safety message stated on the air, as a 95 % radius in
    /// metres, by signer and generation time: `None` when the message said "unavailable".
    /// `v2xw_node::VerifiedMessage` does not carry it, so the detector host joins it back
    /// by `(signer, generation time)`, which is what the message itself carries.
    broadcast_accuracy: BTreeMap<[u8; 8], VecDeque<(SimTime, Option<f64>)>>,
    /// The ground-truth side of the eavesdropper's chains, for the run report only: by the
    /// pseudonym at a chain's end, the vehicle it is truly following and since when.
    privacy_chains: BTreeMap<[u8; 8], (Option<NodeId>, SimTime)>,
    /// Per vehicle, the longest it was followed correctly, ns.
    privacy_tracked: BTreeMap<NodeId, u64>,
    /// The vehicles followed correctly across at least one change.
    privacy_across: BTreeSet<NodeId>,
}

/// The position confidence the detector host used for every message before it read each
/// message's own (`use_stated_accuracy: 0`).
pub const LEGACY_CONFIDENCE_M: f64 = 5.0;

/// How many recent messages per signer the accuracy join keeps: three seconds at 10 Hz,
/// far longer than any delivery takes.
const ACCURACY_JOIN_DEPTH: usize = 32;

/// The 95 % horizontal accuracy radius a safety message states on the air, metres, as
/// its receiver decodes it; `None` when it states "unavailable".
///
/// * A **BSM** carries J2735 `PositionalAccuracy`, whose `semiMajor` is one standard
///   deviation in 0.05 m steps (254 = 12.70 m or more, 255 = unavailable). The receiver
///   scales it to 95 % by the Rayleigh factor.
/// * A **CAM** carries ETSI `PosConfidenceEllipse`, whose `semiMajorConfidence` is already
///   the 95 % semi-axis, in centimetres (4094 = out of range, 4095 = unavailable).
///
/// Computed from the belief through the encoders' own quantisers rather than by decoding
/// the payload again: the value is the one the octets carry (the codec tests round-trip
/// it), without a second decode of every frame.
#[must_use]
pub fn broadcast_accuracy_95_m(
    msg_type: v2xw_msg::MsgType,
    belief: &v2xw_core::PositionEstimate,
) -> Option<f64> {
    use v2xw_msg::j2735::bsm;
    match msg_type {
        v2xw_msg::MsgType::Cam => {
            let cm = v2xw_msg::units::semi_axis_length(belief.semi_major_m);
            (cm < 4095).then(|| f64::from(cm) / 100.0)
        }
        _ => bsm::semi_axis_sigma_m(bsm::semi_axis_accuracy_from_95(belief.semi_major_m))
            .map(|sigma| sigma * bsm::RADIUS_95_PER_SIGMA),
    }
}

impl core::fmt::Debug for Phase2 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Phase2")
            .field("rsus", &self.rsus.len())
            .field("attackers", &self.attackers.len())
            .field("detectors", &self.detectors.len())
            .field("cases", &self.cases.len())
            .field("revoked", &self.revocation.is_some())
            .finish_non_exhaustive()
    }
}

/// `detection.local`: the local detector suite's parameters and the report interval, as the
/// run reads them. Shared with the loader (`crate::scenario::validate`), so a key or value
/// the run would refuse is refused at Check rather than when the run is built.
///
/// # Errors
/// [`EngineError::Scenario`] naming the detector id, the key or the value.
pub(crate) fn detection_params(scenario: &Scenario) -> Result<DetectionParams> {
    // The heading check allows for the bearing error the stated accuracies imply
    // (`Legacy12::heading_bounded`), and the map check for the stated confidence disc.
    // `heading_bearing_bound: 0` and `offroad_confidence_bound: 0` restore the legacy
    // checks, as `use_stated_accuracy: 0` restores the constant 5 m and `use_map: 0`
    // turns the map check off.
    let mut detector_params = DetectorParams {
        heading_bearing_bound: true,
        offroad_confidence_bound: true,
        ..DetectorParams::default()
    };
    let mut report_interval = secs(REPORT_INTERVAL_S);
    let mut stated_accuracy = true;
    let mut map_check = true;
    for (i, choice) in scenario.detection.local.iter().enumerate() {
        if choice.id != LEGACY_12 {
            return Err(conflict(
                &format!("detection.local[{i}]"),
                format!(
                    "this build ships one local detector suite, {LEGACY_12}; got {}",
                    choice.id
                ),
            ));
        }
        if let Some(map) = choice.params.as_object() {
            for (key, value) in map {
                let field = format!("detection.local[{i}].params.{key}");
                if !HOST_PARAM_KEYS.contains(&key.as_str())
                    && !DETECTOR_PARAM_KEYS.contains(&key.as_str())
                {
                    return Err(conflict(
                        &field,
                        format!(
                            "'{key}' is not a parameter of {LEGACY_12}; it takes \
                             {}, {}. Remove the key or correct its spelling",
                            HOST_PARAM_KEYS.join(", "),
                            DETECTOR_PARAM_KEYS.join(", ")
                        ),
                    ));
                }
                let v = value
                    .as_f64()
                    .filter(|v| v.is_finite() && *v >= 0.0)
                    .ok_or_else(|| {
                        conflict(
                            &field,
                            format!("is {value}; it must be a finite number, 0 or more"),
                        )
                    })?;
                if key == "report_interval_s" {
                    report_interval = secs(v);
                } else if key == "use_stated_accuracy" {
                    stated_accuracy = v != 0.0;
                } else if key == "use_map" {
                    map_check = v != 0.0;
                } else if !apply_detector_param(&mut detector_params, key, v) {
                    // `DETECTOR_PARAM_KEYS` names a key this function does not read: the
                    // list and the reader disagree, and the loader test over the list fails.
                    return Err(conflict(
                        &field,
                        format!("'{key}' is listed but not read by {LEGACY_12}"),
                    ));
                }
            }
        } else if !choice.params.is_null() {
            return Err(conflict(
                &format!("detection.local[{i}].params"),
                format!(
                    "is {}; it must be an object of parameter names and values",
                    choice.params
                ),
            ));
        }
    }
    Ok(DetectionParams {
        detector_params,
        report_interval,
        stated_accuracy,
        map_check,
    })
}

/// What [`detection_params`] reads from `detection.local`.
pub(crate) struct DetectionParams {
    pub detector_params: DetectorParams,
    pub report_interval: Duration,
    /// Whether the detectors take a message's stated accuracy (`use_stated_accuracy`).
    pub stated_accuracy: bool,
    /// Whether the map-bound check runs (`use_map`).
    pub map_check: bool,
}

/// The keys `detection.local[].params` accepts that the host reads rather than the suite.
pub const HOST_PARAM_KEYS: [&str; 3] = ["report_interval_s", "use_stated_accuracy", "use_map"];

/// The keys `detection.local[].params` accepts besides `report_interval_s`: exactly the
/// names [`apply_detector_param`] reads.
pub const DETECTOR_PARAM_KEYS: [&str; 16] = [
    "consistency_threshold_m",
    "heading_threshold_deg",
    "detector_lag_s",
    "z_threshold",
    "min_consecutive",
    "sybil_min_certs",
    "sybil_cell_m",
    "art_max_m",
    "max_accel_mps2",
    "stale_max_s",
    "heading_min_speed_mps",
    "heading_min_disp_m",
    "heading_bearing_bound",
    "offroad_confidence_bound",
    "heading_straight_tol_deg",
    "heading_baseline_max_s",
];

/// The detector-suite parameters `detection.local[].params` may override, by name.
fn apply_detector_param(p: &mut DetectorParams, key: &str, v: f64) -> bool {
    match key {
        "consistency_threshold_m" => p.consistency_threshold_m = v,
        "heading_threshold_deg" => p.heading_threshold_deg = v,
        "detector_lag_s" => p.detector_lag_s = v,
        "z_threshold" => p.z_threshold = v,
        "min_consecutive" => p.min_consecutive = v.round().max(1.0) as u32,
        "sybil_min_certs" => p.sybil_min_certs = v.round().max(1.0) as u32,
        "sybil_cell_m" => p.sybil_cell_m = v,
        "art_max_m" => p.art_max_m = v,
        "max_accel_mps2" => p.max_accel_mps2 = v,
        "stale_max_s" => p.stale_max_s = v,
        "heading_min_speed_mps" => p.heading_min_speed_mps = v,
        "heading_min_disp_m" => p.heading_min_disp_m = v,
        "heading_bearing_bound" => p.heading_bearing_bound = v != 0.0,
        "offroad_confidence_bound" => p.offroad_confidence_bound = v != 0.0,
        "heading_straight_tol_deg" => p.heading_straight_tol_deg = v,
        "heading_baseline_max_s" => p.heading_baseline_max_s = v,
        _ => return false,
    }
    true
}

impl Phase2 {
    /// The sizes of the collections this path keeps, by name, for a long run's growth
    /// census ([`crate::Engine::census`]). A diagnostic: nothing reads it back.
    pub fn census(&self, out: &mut Vec<(&'static str, u64)>) {
        let n = |v: usize| v as u64;
        out.push(("scms.providers", n(self.runs.len())));
        let st = &self.home().state;
        let k = &self.home().kernel;
        out.extend([
            ("p2.creds_nodes", n(self.creds.len())),
            ("p2.creds_total", n(self.creds.values().map(Vec::len).sum())),
            ("p2.by_digest", n(self.by_digest.len())),
            ("p2.detectors", n(self.detectors.len())),
            ("p2.filed", n(self.filed.len())),
            ("p2.checked_at", n(self.checked_at.len())),
            ("p2.forwarded", n(self.forwarded.len())),
            ("p2.in_flight", n(self.in_flight.len())),
            ("p2.queue", n(self.queue.len())),
            ("p2.cases", n(self.cases.len())),
            ("p2.nodes", n(self.nodes.len())),
            ("p2.link_claims", n(self.link_claims.len())),
            ("p2.broadcast_accuracy", n(self.broadcast_accuracy.len())),
            ("p2.privacy_chains", n(self.privacy_chains.len())),
            ("p2.privacy_tracked", n(self.privacy_tracked.len())),
            ("p2.blocks", n(self.blocks.len())),
            ("scms.devices", n(st.devices.len())),
            (
                "scms.device_creds",
                n(st.devices.values().map(|d| d.credentials.len()).sum()),
            ),
            (
                "scms.device_crl",
                n(st.devices.values().map(|d| d.crl.len()).sum()),
            ),
            ("scms.ra_requests", n(st.ra.requests.len())),
            ("scms.ra_enrolled", n(st.ra.enrolled.len())),
            ("scms.pca_issued", n(st.pca.issued.len())),
            ("scms.la0_chains", n(st.la[0].chains.len())),
            ("scms.ma_reports", n(st.ma.reports.len())),
            ("scms.crlg_entries", n(st.crlg.entries.len())),
            ("scms.crl_store_entries", n(st.crl_store.entries.len())),
            ("scms.issued_enrolment", n(st.issued_enrolment.len())),
            ("scms.kernel_stages", n(k.stages.stamps().len())),
            ("scms.kernel_steps", n(k.steps.len())),
            ("scms.kernel_ops", n(k.ops.len())),
        ]);
        if let Some(etsi) = self.etsi.as_ref() {
            out.extend([
                ("etsi.enrolled", n(etsi.enrolled.len())),
                ("etsi.tickets", n(etsi.tickets.len())),
            ]);
        }
    }

    /// Builds the Phase 2 state a scenario declared, or `None` when it declared none.
    ///
    /// # Errors
    /// [`EngineError::Scenario`] when the scenario names a protocol, a detector, an
    /// attacker model, a pipeline, a link model or a world site this build does not have.
    #[allow(clippy::too_many_lines)]
    pub fn build(scenario: &Scenario, world: &World) -> Result<Option<Phase2>> {
        let protocol = scenario
            .actors
            .backend
            .protocol
            .clone()
            .or_else(|| scenario.security.protocol.as_ref().map(|c| c.id.clone()));
        let wants_backend = protocol.is_some();
        if scenario.security.scms.is_some() && protocol.as_deref() != Some(CAMP_SCMS) {
            return Err(conflict(
                "security.scms",
                format!(
                    "describes an SCMS deployment, and this run's credential protocol is \
                     {}; set security.protocol to {CAMP_SCMS} or remove security.scms",
                    protocol.as_deref().unwrap_or("none")
                ),
            ));
        }
        let wants_rsus = !scenario.actors.rsus.is_empty();
        let wants_threats = !scenario.threats.attackers.is_empty();
        let wants_detection = !scenario.detection.local.is_empty();
        if !(wants_backend || wants_rsus || wants_threats || wants_detection) {
            return Ok(None);
        }

        if let (Some(a), Some(b)) = (
            scenario.actors.backend.protocol.as_ref(),
            scenario.security.protocol.as_ref(),
        ) && *a != b.id
        {
            return Err(conflict(
                "security.protocol",
                format!(
                    "names {} but actors.backend.protocol names {a}; one deployment runs one \
                     credential protocol",
                    b.id
                ),
            ));
        }
        if let Some(protocol) = &protocol
            && protocol != CAMP_SCMS
            && protocol != ETSI_PKI
        {
            return Err(conflict(
                "security.protocol",
                format!(
                    "{protocol} is not a credential protocol this build ships; \
                     {CAMP_SCMS} or {ETSI_PKI}"
                ),
            ));
        }

        // The host's defaults differ from the suite's legacy ones in two places, both the
        // confidence-range tolerance of CaTch (Kamel et al. 2019); see `detection_params`.
        let DetectionParams {
            detector_params,
            report_interval,
            stated_accuracy,
            map_check,
        } = detection_params(scenario)?;
        let ma = MaHost::from_scenario(scenario)?;

        let access = BackendAccess::from_scenario(scenario)?;

        let mut rsus = Vec::new();
        for spec in &scenario.actors.rsus {
            let position = match (spec.site, spec.position_m) {
                (Some(site), None) => {
                    let site = world.sites.get(site as usize).ok_or_else(|| {
                        conflict(
                            "actors.rsus[].site",
                            format!(
                                "site {site} does not exist: this world has {} \
                                 infrastructure site(s). The OSM importer produces none, so a \
                                 scenario on an imported city states `position_m` instead; \
                                 the procedural generator produces one site per junction \
                                 with `rsu_at_junctions: true`.",
                                world.sites.len()
                            ),
                        )
                    })?;
                    site.antenna_position()
                }
                (None, Some(p)) => Vec3::new(p[0], p[1], p[2] + RSU_MAST_HEIGHT_M),
                _ => {
                    return Err(conflict(
                        "actors.rsus[]",
                        "a roadside unit stands either at a world `site` or at an explicit \
                         `position_m`; give exactly one",
                    ));
                }
            };
            rsus.push(RsuSpec {
                position,
                roles: spec.roles.clone(),
                // The Commsignia ITS-RS4 by default: of the two RSU profiles that ship it is
                // the one whose datasheet publishes a verification rate (>2,000/s, R7 §C2).
                // The Cohda MK5 RSU brief publishes no compute figure at all, so a unit on
                // it could price no verification and dropped every frame it heard as a
                // verification-queue overflow — it received, but it never saw a message.
                profile: spec
                    .profile
                    .clone()
                    .unwrap_or_else(|| DEFAULT_RSU_PROFILE.to_string()),
                backhaul: access
                    .backhaul_with(spec.backhaul.as_deref(), spec.backhaul_params.as_ref())?,
            });
        }

        let mut specs = Vec::new();
        // An `attack.wave` on the timeline is the schedule of every population it names
        // (`crate::timeline::attack_windows`): the population acts inside the wave and not
        // outside it. The threat crate's `AttackSchedule` carries one window, which is why
        // `validate` refuses a population named by two waves.
        let waves = crate::timeline::attack_windows(scenario);
        for (population, a) in scenario.threats.attackers.iter().enumerate() {
            let kind =
                a.id.strip_prefix("threat/attacker/legacy/")
                    .and_then(AttackKind::parse)
                    .ok_or_else(|| {
                        conflict(
                            "threats.attackers[].id",
                            format!(
                                "{} is not an attacker this build ships: the legacy family is \
                                 `threat/attacker/legacy/<Kind>`, e.g. \
                                 threat/attacker/legacy/ConstPos",
                                a.id
                            ),
                        )
                    })?;
            if let Some(why) = unrendered(kind) {
                return Err(conflict(
                    "threats.attackers[].id",
                    format!(
                        "{} is in the catalogue, but this engine does not put it on the air \
                         yet: {why}. Running it would count lies nobody could hear.",
                        a.id
                    ),
                ));
            }
            let params = attacker_params(kind, &a.params)?;
            let horizon = (scenario.time.duration_s * 1e9).round().max(0.0) as u64;
            let (from, to) = match (waves.get(&population), &a.schedule) {
                (Some((from_s, to_s)), _) => (
                    (from_s * 1e9).round().max(0.0) as u64,
                    (to_s * 1e9).round().max(0.0) as u64,
                ),
                (None, Some(w)) => (
                    (w.from_s * 1e9).round().max(0.0) as u64,
                    (w.to_s * 1e9).round().max(0.0) as u64,
                ),
                (None, None) => (0, horizon),
            };
            specs.push(AttackerSpec {
                id: a.id.clone(),
                params,
                count: a.count,
                fraction: a.fraction,
                actor_ids: a.actor_ids.iter().copied().collect(),
                from,
                to,
            });
        }

        let compromised_specs = compromised_rsu_specs(scenario)?;
        let params = LifecycleParams::from_scenario(scenario)?;
        let mut scms_params = params.scms;
        apply_backend_net(scenario, &mut scms_params)?;
        // A hybrid `security.signature` is the credential system's scheme too: every
        // certificate and every signed backend message carries the post-quantum half, and
        // every entity pays for it (`v2xw_proto::hybrid`).
        // A post-quantum `security.kem` wraps every key encrypted to the PCA or a device;
        // `security.threshold` puts committees in place of single authorities; and
        // `actors.backend.failures` takes servers down (`crate::crypto_config`).
        let kem = crate::crypto_config::kem_spec(scenario);
        let hybrid = v2xw_proto::HybridScheme::from_signature(&scenario.security.signature)
            .map(|h| h.with_kem(kem.as_ref()));
        scms_params.hybrid = hybrid;
        scms_params.kem = kem;
        scms_params.threshold = crate::crypto_config::threshold_deployment(scenario)?;
        let deployment = deployment_of(scenario, protocol.as_deref() == Some(CAMP_SCMS))?;
        // The rules every provider starts from, before its own CRL publication rule.
        let base_params = scms_params;
        apply_crl_publication(scenario, &deployment, &mut scms_params);
        let mut scms = ScmsRun::new_deployed(scms_params, 0, deployment).map_err(|e| {
            conflict(
                "actors.backend",
                format!("the SCMS deployment refused to start: {e}"),
            )
        })?;
        apply_backend_topology(scenario, &mut scms)?;
        let faults = crate::crypto_config::backend_faults(scenario, &scms)?;
        scms.set_faults(faults);
        // One run per provider (WP-D1): every other provider's run is built from the
        // home one, on the same CTL, electors and clock, under its own PKI and seed.
        let mut runs = Vec::with_capacity(scms.state.deployment.providers.len());
        for k in 1..scms.state.deployment.providers.len() {
            runs.push(ScmsRun::new_peer(&scms, k).map_err(|e| {
                conflict(
                    "security.scms.deployment",
                    format!("provider {k}'s backend refused to start: {e}"),
                )
            })?);
        }
        runs.insert(0, scms);
        // Per provider (WP-D1): its own CRL publication rule, and its own MA mode when
        // the scenario names one for it (`providers[].ma_mode`; otherwise the run-wide
        // `ma_review_s` rule applies to every provider).
        let explicit_ma: BTreeSet<String> = scenario
            .security
            .scms
            .as_ref()
            .and_then(|s| s.deployment.as_ref())
            .map(|d| {
                d.providers
                    .iter()
                    .filter(|p| p.ma_mode.is_some())
                    .map(|p| p.name.clone())
                    .collect()
            })
            .unwrap_or_default();
        for (k, run) in runs.iter_mut().enumerate() {
            let p = &mut run.state.params;
            p.crl.business_day_sla = base_params.crl.business_day_sla;
            p.publish_on_cadence = base_params.publish_on_cadence;
            let deployment = run.state.deployment.clone();
            apply_crl_publication_of(scenario, &deployment, k, &mut run.state.params);
            if let Some(prov) = deployment.providers.get(k)
                && explicit_ma.contains(&prov.name)
            {
                run.state.params.ma.mode = prov.ma_mode;
            }
        }
        // The deployment sets the elector count and quorum and, under the region model,
        // the intra-region backend latency (WP-D1).
        let params = LifecycleParams {
            scms: runs[0].state.params,
            ..params
        };
        let etsi = if protocol.as_deref() == Some(ETSI_PKI) {
            let mut ep = v2xw_proto::etsi::ts102941::EtsiParams::default();
            ep.decide_on_report = false;
            ep.at_validity = params.scms.cert_lifetime;
            ep.butterfly_batch = params.jmax;
            ep.hybrid = hybrid;
            ep.kem = kem;
            let mut run = v2xw_proto::etsi::ts102941::EtsiRun::new(ep).map_err(|e| {
                conflict(
                    "security.protocol",
                    format!("the ETSI deployment refused to start: {e}"),
                )
            })?;
            // The TLM's ECTL and the Root CA's CA-CRL exist before any station asks for
            // them: both are signed once, at the start, and served by the Distribution
            // Centre to every station that fetches (TS 102 941 §6.3).
            run.issue_trust_lists(0);
            Some(run)
        } else {
            None
        };
        // Every message of every backend flow rides the run's access model: the kernels
        // ask the carrier for each one's arrival (`backend::Carrier`).
        // Under the region latency model each provider's device legs also cross the
        // internet from the scenario's city to that provider's region (ASSUMPTION).
        for (k, run) in runs.iter_mut().enumerate() {
            let leg =
                run.state
                    .deployment
                    .city_latency(k, world.origin.lat_deg, world.origin.lon_deg);
            match leg {
                Some(extra) => {
                    run.set_path_oracle(std::sync::Arc::new(std::sync::Mutex::new(InternetLeg {
                        inner: access.oracle(),
                        extra,
                    })))
                }
                None => run.set_path_oracle(access.oracle()),
            }
        }
        let providers = runs.len();
        let internet_legs: Vec<Duration> = (0..providers)
            .map(|k| {
                runs[0]
                    .state
                    .deployment
                    .city_latency(k, world.origin.lat_deg, world.origin.lon_deg)
                    .unwrap_or(Duration::ZERO)
            })
            .collect();
        let mut etsi = etsi;
        if let Some(e) = etsi.as_mut() {
            e.set_path_oracle(access.oracle());
        }
        Ok(Some(Phase2 {
            runs,
            device_provider: BTreeMap::new(),
            fleet_seed: scenario.seed,
            crl_feeds: vec![Vec::new(); providers],
            backend_now: 0,
            internet_legs,
            etsi,
            blocks: Vec::new(),
            params,
            access,
            creds: BTreeMap::new(),
            by_digest: BTreeMap::new(),
            rsus,
            rsu_nodes: Vec::new(),
            camp: protocol.as_deref() == Some(CAMP_SCMS),
            rsu_cycle: BTreeMap::new(),
            mbms_next: 0,
            specs,
            attackers: BTreeMap::new(),
            detectors: BTreeMap::new(),
            detector_params,
            detection_on: wants_detection,
            stated_accuracy,
            // A world with no motor-vehicle lane (a fixture, an empty extract) gives the
            // receivers no map rather than one on which every claim is off the road.
            map_check: map_check
                && world
                    .roads
                    .lanes()
                    .iter()
                    .any(|l| l.admits(RoadMap::vehicles())),
            report_interval,
            ma: vec![ma; providers],
            vehicles: 0,
            filed: BTreeMap::new(),
            checked_at: BTreeMap::new(),
            forwarded: BTreeSet::new(),
            in_flight: BTreeMap::new(),
            evidence: BTreeMap::new(),
            superseded_root: None,
            stage_cursor: vec![0; providers],
            step_cursor: vec![0; providers],
            queue: vec![VecDeque::new(); providers],
            cases: Vec::new(),
            published: vec![0; providers],
            broadcast_version: 0,
            nodes: BTreeMap::new(),
            revocation: None,
            report: Phase2Report {
                signature_scheme: scenario.security.signature.clone(),
                signature_mode: v2xw_sec::scheme::signature(&scenario.security.signature)
                    .map_or("classical", |x| x.mode.as_str())
                    .to_string(),
                kem_scheme: scenario.security.kem.clone(),
                ..Phase2Report::default()
            },
            observer: v2xw_threat::PrivacyObserver::cited_defaults(NodeId::new(u32::MAX)),
            link_claims: Vec::new(),
            tracker: EdgeTracker::default(),
            peer_trackers: Vec::new(),
            next_view: 0,
            pki: pki_view_of(scenario).then(v2xw_proto::scms::pki_view::PkiTracker::default),
            views: 0,
            compromised_specs,
            compromised: BTreeMap::new(),
            compromised_nodes: BTreeMap::new(),
            first_falsified: BTreeMap::new(),
            first_reported: BTreeMap::new(),
            decided: BTreeMap::new(),
            attacker_lies: BTreeMap::new(),
            attacker_reports: BTreeMap::new(),
            attacker_at_ma: BTreeMap::new(),
            attacker_peak_events: BTreeMap::new(),
            revoked_devices: BTreeSet::new(),
            ma_decided_devices: BTreeSet::new(),
            broadcast_accuracy: BTreeMap::new(),
            privacy_chains: BTreeMap::new(),
            privacy_tracked: BTreeMap::new(),
            privacy_across: BTreeSet::new(),
        }))
    }

    /// The lifecycle parameters in force.
    #[must_use]
    pub fn params(&self) -> &LifecycleParams {
        &self.params
    }

    /// The home provider's SCMS run (read-only): its components' state, every emitted
    /// SPDU and refusal, the governance entities and the stage log, for the tests and
    /// for an attack study that inspects what the backend saw. Every provider's run is
    /// [`Phase2::scms_runs`], a node's own [`Phase2::scms_of_node`].
    #[must_use]
    pub fn scms(&self) -> &ScmsRun {
        self.home()
    }

    /// Every provider's SCMS run, in deployment order (WP-D1).
    #[must_use]
    pub fn scms_runs(&self) -> &[ScmsRun] {
        &self.runs
    }

    /// The run of the provider that serves `node` (a vehicle once provisioned, or a
    /// roadside unit in the certificate cycle); `None` before.
    #[must_use]
    pub fn scms_of_node(&self, node: NodeId) -> Option<&ScmsRun> {
        self.provider_of_node(node).map(|k| &self.runs[k])
    }

    /// The provider (index in the deployment) that serves `node`'s vehicle, once it is
    /// provisioned; `None` before.
    #[must_use]
    pub fn provider_of_node(&self, node: NodeId) -> Option<usize> {
        self.device_provider.get(&device_of(node)).copied()
    }

    /// The home provider's run: the one the SCMS Manager's governance decisions are
    /// taken on, and the one every node's trust bundle is read from.
    fn home(&self) -> &ScmsRun {
        &self.runs[0]
    }

    /// The provider serving SCMS device `device` (0 until it is assigned).
    fn provider_of_device(&self, device: NodeId) -> usize {
        self.device_provider.get(&device).copied().unwrap_or(0)
    }

    /// The run of the provider serving SCMS device `device`.
    fn run_of(&self, device: NodeId) -> &ScmsRun {
        &self.runs[self.provider_of_device(device)]
    }

    /// The run of the provider serving SCMS device `device`, mutably.
    fn run_of_mut(&mut self, device: NodeId) -> &mut ScmsRun {
        let k = self.provider_of_device(device);
        &mut self.runs[k]
    }

    /// The roadside units to create, in declaration order.
    pub fn rsu_specs(&self) -> &[RsuSpec] {
        &self.rsus
    }

    /// Records the node id the engine gave one roadside unit.
    pub fn note_rsu(&mut self, node: NodeId) {
        let index = self.rsu_nodes.len();
        self.rsu_nodes.push(node);
        self.report.rsus += 1;
        if let Some(spec) = self.rsus.get(index) {
            self.access.register_rsu(node, spec.backhaul, spec.position);
        }
        if self.compromised_specs.contains_key(&index) {
            self.compromised_nodes.insert(node, index);
            self.report.compromised_rsus += 1;
        }
        self.join_rsu_cycle(node, index);
    }

    /// Puts roadside unit `node` into its provider's application-certificate cycle
    /// (WP-B3b): enrolled and issued this week's certificate before the run (and next
    /// week's when its backhaul is up), then renewing weekly over its own backhaul line.
    /// Only under the SCMS with its PDU layer; otherwise the unit keeps the factory
    /// certificate (`wiring::bootstrap_credentials`).
    fn join_rsu_cycle(&mut self, node: NodeId, index: usize) {
        if !self.camp || self.etsi.is_some() || self.home().state.pki.is_none() {
            return;
        }
        let Some(spec) = self.rsus.get(index) else {
            return;
        };
        let bh = spec.backhaul;
        let device = device_of(node);
        // The seeded fleet split, the same draw the factory used for this unit.
        let k = self
            .home()
            .state
            .deployment
            .provider_of(self.fleet_seed, u64::from(node.index()));
        self.device_provider.insert(device, k);
        let now = self.backend_now;
        let run = &mut self.runs[k];
        run.add_rsu_with(
            device,
            v2xw_proto::Link {
                latency: bh.latency,
                bandwidth_bps: bh.bandwidth_bps,
                transport: Transport::RsuBackhaul,
            },
        );
        if !bh.connected {
            run.set_backhaul_outage(device, now, SimTime::MAX);
        }
        if let Err(e) = run.preload_app_cert(
            device,
            v2xw_proto::scms::appcert::AppCertKind::RsuApplication,
            now,
            bh.connected,
        ) {
            self.report.backend_errors += 1;
            if self.report.first_backend_error.is_empty() {
                self.report.first_backend_error =
                    format!("roadside unit {node:?}'s pre-run certificate: {e}");
            }
        }
        self.rsu_cycle.insert(node, RsuCycle::default());
        self.report.rsus_in_cert_cycle += 1;
    }

    /// Whether roadside unit `node` signs only with certificates from the cycle.
    #[must_use]
    pub fn rsu_in_cert_cycle(&self, node: NodeId) -> bool {
        self.rsu_cycle.contains_key(&node)
    }

    /// The application certificates the SCMS installed since the engine last asked, per
    /// roadside unit (or for `only`), for the engine to put into each node.
    pub fn rsu_app_installs(&mut self, only: Option<NodeId>) -> Vec<(NodeId, Vec<RsuAppCred>)> {
        let mut out = Vec::new();
        let nodes: Vec<NodeId> = self
            .rsu_cycle
            .keys()
            .copied()
            .filter(|n| only.is_none_or(|o| o == *n))
            .collect();
        for node in nodes {
            let device = device_of(node);
            let handed = self.rsu_cycle[&node].handed;
            let run = self.run_of(device);
            let Some(pki) = run.state.pki.as_ref() else {
                continue;
            };
            let all = run.app_installed(device);
            if all.len() <= handed {
                continue;
            }
            let creds: Vec<RsuAppCred> = all[handed..]
                .iter()
                .filter_map(|c| {
                    Some(RsuAppCred {
                        period: c.cred.period,
                        valid_from: c.from,
                        valid_until: c.until,
                        key: std::sync::Arc::new(c.cred.implicit_key(&pki.aca.cert)?),
                        preloaded: c.at == 0,
                    })
                })
                .collect();
            let n = all.len();
            if let Some(c) = self.rsu_cycle.get_mut(&node) {
                c.handed = n;
            }
            for c in &creds {
                if c.preloaded {
                    self.report.rsu_app_certs_preloaded += 1;
                } else {
                    self.report.rsu_app_certs_renewed += 1;
                }
            }
            out.push((node, creds));
        }
        out
    }

    /// Counts certificates a roadside unit refused to install.
    pub fn note_rsu_app_refused(&mut self, n: u64) {
        self.report.rsu_app_certs_refused += n;
    }

    /// One roadside-unit step at `now`: whether the unit held a valid application
    /// certificate. Opens or closes a starvation spell.
    /// `withheld` is the node's running count of messages it had due and could not sign.
    pub fn note_rsu_cert(&mut self, node: NodeId, now: SimTime, valid: bool, withheld: u64) {
        let Some(c) = self.rsu_cycle.get_mut(&node) else {
            return;
        };
        self.report.rsu_messages_withheld += withheld.saturating_sub(c.withheld_seen);
        c.withheld_seen = c.withheld_seen.max(withheld);
        if valid {
            if c.starved_since.take().is_some() {
                self.report.rsu_starved_ns += now.saturating_sub(c.starved_last);
            }
            return;
        }
        self.report.rsu_starved_steps += 1;
        if !c.starved {
            c.starved = true;
            self.report.rsus_starved += 1;
        }
        if c.starved_since.is_some() {
            self.report.rsu_starved_ns += now.saturating_sub(c.starved_last);
        } else {
            c.starved_since = Some(now);
        }
        c.starved_last = now;
    }

    /// The roadside units in the cycle whose RA now serves a newer composite CRL than
    /// they installed, and whose backhaul is up: each fetches it (1609.2.1 §6.3.5.7,
    /// as any end entity) and installs what it verifies.
    #[must_use]
    pub fn rsu_crls_due(&self) -> Vec<NodeId> {
        self.rsu_cycle
            .iter()
            .filter(|(n, c)| {
                self.rsu_spec_of(**n).is_some_and(|s| s.backhaul.connected)
                    && self.crl_version_for(**n) > c.crl_version
            })
            .map(|(n, _)| *n)
            .collect()
    }

    /// The version of the composite CRL `node`'s RA serves now.
    #[must_use]
    pub fn crl_version_for(&self, node: NodeId) -> u32 {
        let h = self.provider_of_device(device_of(node));
        self.visible_versions(h)
            .iter()
            .fold(0u32, |a, (_, v)| a.saturating_add(v.version))
    }

    /// Roadside unit `node` installed (or refused) composite CRL `version`.
    pub fn note_rsu_crl(&mut self, node: NodeId, version: u32, installed: bool) {
        if let Some(c) = self.rsu_cycle.get_mut(&node) {
            c.crl_version = c.crl_version.max(version);
        }
        if installed {
            self.report.rsu_crls_installed += 1;
        } else {
            self.report.rsu_crls_refused += 1;
        }
    }

    /// Counts CRL frames a roadside unit could not sign.
    pub fn note_rsu_withheld(&mut self, crl_frames: u64) {
        self.report.rsu_crl_frames_withheld += crl_frames;
    }

    /// The intervals in `[from, to)` in which roadside unit `node` held no valid
    /// application certificate, from the SCMS's own record (ground truth).
    #[must_use]
    pub fn rsu_starvation(
        &self,
        node: NodeId,
        from: SimTime,
        to: SimTime,
    ) -> Vec<(SimTime, SimTime)> {
        let device = device_of(node);
        self.run_of(device).app_starvation(device, from, to)
    }

    /// Whether `node` is a roadside unit under an attacker's control.
    #[must_use]
    pub fn is_compromised(&self, node: NodeId) -> bool {
        self.compromised_nodes.contains_key(&node)
    }

    /// The compromised unit's model, built the first time the unit acts, with the
    /// certificate digest it signs under (the credentials every receiver trusts).
    fn compromised_model(
        &mut self,
        node: NodeId,
        own_digest: &str,
    ) -> Option<&mut v2xw_threat::CompromisedRsu> {
        let index = *self.compromised_nodes.get(&node)?;
        if !self.compromised.contains_key(&node) {
            let (params, from, to) = self.compromised_specs.get(&index)?.clone();
            let mut params = params;
            params.own_cert_digest = own_digest.to_string();
            let schedule = v2xw_threat::capability::AttackSchedule {
                from,
                to,
                ..v2xw_threat::capability::AttackSchedule::default()
            };
            self.compromised.insert(
                node,
                v2xw_threat::CompromisedRsu::new(node, params, schedule),
            );
        }
        self.compromised.get_mut(&node)
    }

    /// A report a roadside unit is about to forward to the authority, after the unit has
    /// had its say: an honest unit forwards it, a compromised one may drop it or replace
    /// it with a forgery that frames a vehicle it has heard (07-threats-and-detection §2.2).
    /// `None` means the report goes no further.
    pub fn rsu_forward(
        &mut self,
        ctx: &mut dyn ThreatCtx,
        rsu: NodeId,
        own_digest: &str,
        report: &MisbehaviourReport,
        now: SimTime,
    ) -> Option<MisbehaviourReport> {
        let Some(model) = self.compromised_model(rsu, own_digest) else {
            return Some(report.clone());
        };
        if !v2xw_threat::Attacker::schedule(model).active_at(now, 0.0, 0.0) {
            return Some(report.clone());
        }
        match model.on_forward(ctx, report, now).0 {
            v2xw_threat::ForwardDecision::Forward => Some(report.clone()),
            v2xw_threat::ForwardDecision::Drop => {
                self.report.rsu_reports_suppressed += 1;
                None
            }
            v2xw_threat::ForwardDecision::Replace(forged) => {
                self.report.rsu_reports_poisoned += 1;
                // The forgery arrives as the unit's own report, and the authority trusts
                // infrastructure: that is the attack.
                for ma in &mut self.ma {
                    ma.trust_infrastructure(forged.reporter_cert_digest.clone());
                }
                Some(*forged)
            }
        }
    }

    /// A compromised unit with `FalseCrl` adds fabricated entries to the CRL frame it is
    /// about to broadcast. It holds no CRL Generator key, so the frame no longer verifies
    /// and every receiver discards it. Returns whether the frame was altered.
    pub fn forge_crl_frame(
        &mut self,
        ctx: &mut dyn ThreatCtx,
        rsu: NodeId,
        own_digest: &str,
        now: SimTime,
        frame: &mut Vec<CrlLinkageEntry>,
    ) -> bool {
        let Some(model) = self.compromised_model(rsu, own_digest) else {
            return false;
        };
        if model.kind() != v2xw_threat::RsuAttackKind::FalseCrl
            || !v2xw_threat::Attacker::schedule(model).active_at(now, 0.0, 0.0)
        {
            return false;
        }
        let n = model.params().fabricated_entries.round().max(1.0) as u32;
        let mut rng = ctx.rng(
            v2xw_core::rng::RngDomain::Attack,
            v2xw_core::rng::EntityRef::Node(rsu),
        );
        for _ in 0..n {
            let mut a = [0u8; 16];
            let mut b = [0u8; 16];
            rng.fill_bytes(&mut a);
            rng.fill_bytes(&mut b);
            frame.push(CrlLinkageEntry {
                i: self.params.period_at(now),
                la_id1: v2xw_sec::linkage::LaId(1),
                la_id2: v2xw_sec::linkage::LaId(2),
                ls1_i: v2xw_sec::linkage::LinkageSeed::new(a),
                ls2_i: v2xw_sec::linkage::LinkageSeed::new(b),
                jmax: self.params.jmax,
                max_forward: v2xw_sec::linkage::DEFAULT_MAX_FORWARD_PERIODS,
            });
        }
        drop(rng);
        self.report.rsu_crl_frames_forged += 1;
        true
    }

    /// Lets a compromised unit note the senders it hears, which is where a poisoner picks
    /// its victims from.
    pub fn compromised_hears(
        &mut self,
        ctx: &mut dyn ThreatCtx,
        rsu: NodeId,
        own_digest: &str,
        me: &SelfBelief,
        heard: &[ObservedMessage],
    ) {
        if heard.is_empty() {
            return;
        }
        let Some(model) = self.compromised_model(rsu, own_digest) else {
            return;
        };
        let view = AttackerView {
            own_rx: heard,
            own_credentials: &[],
            crl_revocations_seen: None,
            own_belief: *me,
            honest: v2xw_threat::HonestClaim {
                x_m: me.x_m,
                y_m: me.y_m,
                speed_mps: 0.0,
                heading_rad: 0.0,
            },
            believed_time: me.believed_time,
        };
        v2xw_threat::Attacker::observe(model, ctx, &view);
    }

    /// The roadside units' node ids.
    pub fn rsu_nodes(&self) -> &[NodeId] {
        &self.rsu_nodes
    }

    /// The unit `node` is, if it is one.
    #[must_use]
    pub fn rsu_spec_of(&self, node: NodeId) -> Option<&RsuSpec> {
        let index = self.rsu_nodes.iter().position(|n| *n == node)?;
        self.rsus.get(index)
    }

    /// Whether `node` is a roadside unit that carries `role`.
    ///
    /// A unit that declares no role at all carries every one of them.
    #[must_use]
    pub fn rsu_has_role(&self, node: NodeId, role: &str) -> bool {
        self.rsu_spec_of(node)
            .is_some_and(|s| s.has_role(role) || s.roles.is_empty())
    }

    /// The roadside units that carry `role`, in declaration order.
    #[must_use]
    pub fn rsus_with_role(&self, role: &str) -> Vec<NodeId> {
        self.rsu_nodes
            .iter()
            .copied()
            .filter(|n| self.rsu_has_role(*n, role))
            .collect()
    }

    /// The counters, mutably, for the engine to fold node-local totals into.
    pub fn report_mut(&mut self) -> &mut Phase2Report {
        &mut self.report
    }

    /// The counters this path accumulated during the run, with the access legs' own.
    pub fn report(&self) -> Phase2Report {
        let mut r = self.report.clone();
        r.access = self.access.report_now();
        let st = &self.home().state;
        r.committees = st
            .committees
            .stats
            .iter()
            .map(|(k, v)| ((*k).to_string(), *v))
            .collect();
        r.committee_failures = st
            .threshold_failures
            .iter()
            .map(|(k, v)| ((*k).to_string(), *v))
            .collect();
        r.backend_deliveries_dropped = self.runs.iter().map(|x| x.state.faults.dropped).sum();
        r.reports_unsent = self.nodes.values().map(|n| n.outbox.0.len() as u64).sum();
        r.reports_evicted = self.nodes.values().map(|n| n.outbox.0.evicted).sum();
        r.reports_expired = self.nodes.values().map(|n| n.outbox.0.expired).sum();
        r
    }

    /// The first revocation, once a vehicle has enforced it.
    pub fn revocation(&self) -> Option<&Revocation> {
        self.revocation.as_ref()
    }

    /// The access legs.
    pub fn access(&self) -> &BackendAccess {
        &self.access
    }

    /// The access legs, mutably.
    pub fn access_mut(&mut self) -> &mut BackendAccess {
        &mut self.access
    }

    /// Whether a cellular modem has a serving cell at `pos`.
    #[must_use]
    pub fn uu_coverage(&self, pos: Vec3, t: SimTime) -> bool {
        self.access.uu_coverage(pos, t)
    }

    /// A vehicle's backend access.
    #[must_use]
    pub fn access_kind(&self, node: NodeId) -> AccessKind {
        self.nodes
            .get(&node)
            .and_then(|n| n.access)
            .unwrap_or(AccessKind::Offline)
    }

    /// Whether a relaying unit (one with `role`, and a connected backhaul) is within the
    /// relay radius of `pos`, and which.
    #[must_use]
    pub fn relay_in_range(&self, pos: Vec3, role: &str) -> Option<NodeId> {
        let range = self.access.relay_range_m();
        self.rsu_nodes
            .iter()
            .copied()
            .zip(self.rsus.iter())
            .find(|(n, s)| {
                s.backhaul.connected
                    && self.rsu_has_role(*n, role)
                    && s.position.distance(Vec3::new(pos.x, pos.y, s.position.z)) <= range
            })
            .map(|(n, _)| n)
    }

    /// Enrols and provisions one vehicle before the run and gives it its access.
    ///
    /// The pool is `pool_periods` i-periods of `certs_per_period` certificates starting
    /// at the period `now` falls in — what a vehicle on the road holds — provisioned by
    /// the real flows on a scratch kernel ([`ScmsRun::preload`], joint 3).
    pub fn provision(
        &mut self,
        node: NodeId,
        now: SimTime,
        rng: &v2xw_core::rng::RngRegistry,
    ) -> Vec<ProvisionedCred> {
        let relay = self.rsus.iter().any(|s| {
            s.backhaul.connected
                && (s.roles.is_empty()
                    || s.has_role("provisioning-proxy")
                    || s.has_role("report-forward"))
        });
        let kind = self.access.assign(rng, node, relay);
        let device = device_of(node);
        let start = self.params.period_at(now);
        if let Some(etsi) = self.etsi.as_mut() {
            // Authorization tickets: `certs_per_period` a week — the C2C-CC profile's 20
            // parallel tickets [TR 103 415 Table A.2] under the EU policy's cap of 100
            // [EUCP §7.2.1] — with no linkage value, because nothing ever revokes one
            // (TS 102 941 §6.1.4 NOTE 4).
            etsi.preload(device, self.params.jmax * self.params.pool_periods);
            let out: Vec<ProvisionedCred> = (start..start + self.params.pool_periods)
                .flat_map(|i| {
                    let (from, until) = self.params.scms.validity(i);
                    (0..self.params.jmax).map(move |j| ProvisionedCred {
                        i,
                        j,
                        lv: LinkageValue::new([0u8; 9]),
                        valid_from: from,
                        valid_until: until,
                        key: None,
                    })
                })
                .collect();
            self.nodes.insert(
                node,
                NodeSec {
                    access: Some(kind),
                    last_period: start + self.params.pool_periods.saturating_sub(1),
                    next_crl_fetch: SimTime::MAX,
                    enrolment_until: SimTime::MAX,
                    // A station checks the Distribution Centre when it joins, then on the
                    // fetch interval.
                    next_trust_fetch: now,
                    ..NodeSec::default()
                },
            );
            self.creds.insert(node, out.clone());
            return out;
        }
        // The seeded fleet split (WP-D1): the same draw that assigns a roadside unit's
        // application certificate, so a vehicle is enrolled, provisioned, reported and
        // revoked by one provider for its whole life.
        let k = self
            .home()
            .state
            .deployment
            .provider_of(self.fleet_seed, u64::from(node.index()));
        self.device_provider.insert(device, k);
        let preloaded =
            self.runs[k].preload(device, start, self.params.pool_periods, self.params.jmax);
        if preloaded.is_err() {
            self.nodes.insert(
                node,
                NodeSec {
                    access: Some(kind),
                    ..NodeSec::default()
                },
            );
            return Vec::new();
        }
        let out = self.creds_of_device(device, 0);
        if out.is_empty() {
            // The pre-run provisioning flows finished without an error and without a
            // single certificate for this device. It then signs with the bootstrap
            // stand-in (`wiring::bootstrap_credentials`), which carries no linkage value;
            // counted so the gap is visible. Seen on `credential-lifecycle` (60 s
            // i-periods) for vehicles joining a few seconds after a period boundary.
            self.report.vehicles_unprovisioned += 1;
        }
        let last = start + self.params.pool_periods.saturating_sub(1);
        // A vehicle on the road holds an enrolment certificate issued some time before the
        // run — at the factory or at its last renewal — so its remaining validity is
        // uniform over the lifetime, drawn from the vehicle's own keyed stream. With the
        // default six years almost no run sees a renewal; a compressed lifetime does.
        let lifetime = self.params.scms.enrolment_lifetime.as_nanos().max(1);
        let u = rng
            .checkout(
                v2xw_core::rng::RngDomain::Backend,
                v2xw_core::rng::EntityRef::custom("phase2/enrolment-age", u64::from(node.index())),
            )
            .f64();
        let remaining = ((u * lifetime as f64) as u64).clamp(1, lifetime);
        let until = now.saturating_add(remaining);
        self.run_of_mut(device).set_enrolment(
            device,
            v2xw_proto::scms::msg::EnrolmentCert {
                generation: 0,
                valid_from: until.saturating_sub(lifetime),
                valid_until: until,
            },
        );
        // A vehicle already inside its renewal lead re-enrolled before the run (WP-B2:
        // the request goes out `reenrol_lead` before expiry): it holds the successor,
        // which starts when the current certificate ends, with no overlap.
        let mut enrolment_until = until;
        if remaining <= self.params.scms.reenrol_lead.as_nanos() {
            enrolment_until = until.saturating_add(lifetime);
            self.run_of_mut(device).set_successor_enrolment(
                device,
                v2xw_proto::scms::msg::EnrolmentCert {
                    generation: 1,
                    valid_from: until,
                    valid_until: enrolment_until,
                },
            );
        }
        self.nodes.insert(
            node,
            NodeSec {
                access: Some(kind),
                last_period: last,
                next_crl_fetch: now,
                enrolment_until,
                ..NodeSec::default()
            },
        );
        self.creds.insert(node, out.clone());
        out
    }

    /// The device's credentials at period `from_i` and later.
    ///
    /// On the real path each carries what the device derived from its download (WP-C1):
    /// the implicit certificate, the issuing ACA's certificate, and the signing key
    /// `d = e·b_ij + r` from the device's own caterpillar. Nothing about the device's
    /// seeds or the LAs crosses: only the certificate, its issuer and `d`.
    fn creds_of_device(&self, device: NodeId, from_i: u32) -> Vec<ProvisionedCred> {
        let run = self.run_of(device);
        let Some(dev) = run.state.devices.get(&device) else {
            return Vec::new();
        };
        let pki = run.state.pki.as_ref();
        dev.credentials
            .iter()
            .filter(|((i, _), _)| *i >= from_i)
            .map(|(&(i, j), c)| {
                let (from, until) = self.params.scms.validity(i);
                let key = match (c.implicit.as_ref(), dev.caterpillar.as_ref(), pki) {
                    (Some(imp), Some(cat), Some(pki)) => pki.by_id8(&imp.aca_id8).map(|aca| {
                        std::sync::Arc::new(v2xw_sec::credential::ImplicitKey::new(
                            std::sync::Arc::new(imp.certificate.clone()),
                            std::sync::Arc::new(aca.cert.clone()),
                            cat.signing_private_implicit(i, j, &imp.e, &imp.r),
                        ))
                    }),
                    _ => None,
                };
                ProvisionedCred {
                    i,
                    j,
                    lv: c.lv,
                    valid_from: from,
                    valid_until: until,
                    key,
                }
            })
            .collect()
    }

    /// The SCMS run's ACA as an issuer, for the credentials no protocol flow issues
    /// ([`crate::credentials`]); `None` when the run has no PDU layer.
    #[must_use]
    pub fn aca_issuer(&self) -> Option<v2xw_sec::credential::ImplicitIssuer> {
        let pki = self.home().state.pki.as_ref()?;
        let d = pki.aca.private?;
        v2xw_sec::credential::ImplicitIssuer::new(pki.aca.cert.clone(), d).ok()
    }

    /// The trust a device of this run installs (WP-C2): the PKI's root as the
    /// CTL-approved root and every component certificate as the local certificate chain
    /// file. Empty with the PDU layer off.
    ///
    /// The governance entities (`v2xw_proto::scms::governance`) sign their CTL over their
    /// own root key set; until the deployment topology (WP-D1) puts the PDU layer's root
    /// on that CTL, the device installs the issuing root directly (labelled assumption).
    #[must_use]
    pub fn trust_bundle(
        &self,
    ) -> (
        Vec<std::sync::Arc<v2xw_msg::sec_types::Certificate>>,
        Vec<std::sync::Arc<v2xw_msg::sec_types::Certificate>>,
    ) {
        let Some(pki) = self.home().state.pki.as_ref() else {
            return (Vec::new(), Vec::new());
        };
        // Every provider whose root the CTL approves (WP-D1, gap row 42): its root as a
        // trust anchor and its components in the chain file, so a device verifies
        // another provider's pseudonyms and application certificates. A provider off
        // the CTL contributes nothing, and whatever it certifies does not verify.
        let st = &self.home().state;
        let on_ctl = |label: &str| {
            st.gov
                .root_of(label)
                .is_some_and(|r| st.gov.ctl.roots.contains(&r))
        };
        let mut pkis = vec![pki];
        pkis.extend(
            st.peer_pki
                .iter()
                .filter(|(label, _)| on_ctl(label))
                .map(|(_, p)| p),
        );
        let roots = pkis
            .iter()
            .map(|p| std::sync::Arc::new(p.root.cert.clone()))
            .collect();
        let cas = pkis
            .iter()
            .flat_map(|p| p.all().into_iter().chain(p.extra.iter()))
            .map(|a| std::sync::Arc::new(a.cert.clone()))
            .collect();
        (roots, cas)
    }

    /// Every provider's chain file, in deployment order (the home provider first, as
    /// [`Phase2::provider_issuers`]): the CA certificates of that provider alone, empty
    /// for a provider whose root is not on the CTL. What a device's chain file holds under
    /// `security.scms.chain_file: home-provider`.
    #[must_use]
    pub fn provider_chain_files(
        &self,
    ) -> Vec<Vec<std::sync::Arc<v2xw_msg::sec_types::Certificate>>> {
        let st = &self.home().state;
        let Some(home) = st.pki.as_ref() else {
            return Vec::new();
        };
        let on_ctl = |label: &str| {
            st.gov
                .root_of(label)
                .is_some_and(|r| st.gov.ctl.roots.contains(&r))
        };
        let file = |p: &v2xw_proto::scms::pdu::ScmsPki| {
            p.all()
                .into_iter()
                .chain(p.extra.iter())
                .map(|a| std::sync::Arc::new(a.cert.clone()))
                .collect::<Vec<_>>()
        };
        core::iter::once(file(home))
            .chain(
                st.peer_pki
                    .iter()
                    .map(|(label, p)| if on_ctl(label) { file(p) } else { Vec::new() }),
            )
            .collect()
    }

    /// Every provider's ACA as an issuer, in deployment order (the home provider first):
    /// the issuer [`crate::credentials::CredentialFactory`] uses for a device assigned to
    /// that provider. Empty without a PDU layer.
    #[must_use]
    pub fn provider_issuers(&self) -> Vec<v2xw_sec::credential::ImplicitIssuer> {
        let st = &self.home().state;
        let Some(home) = st.pki.as_ref() else {
            return Vec::new();
        };
        core::iter::once(home)
            .chain(st.peer_pki.iter().map(|(_, p)| p))
            .filter_map(|p| {
                v2xw_sec::credential::ImplicitIssuer::new(p.aca.cert.clone(), p.aca.private?).ok()
            })
            .collect()
    }

    /// The deployment this run's backend was built from (WP-D1).
    #[must_use]
    pub fn deployment(&self) -> &v2xw_proto::scms::deployment::Deployment {
        &self.home().state.deployment
    }

    /// The current signed CRL the CRL Generator distributes (series 1, full), as the
    /// bytes a store or an RSU relays unchanged. `None` with the PDU layer off.
    #[must_use]
    pub fn secured_crl(&self) -> Option<std::sync::Arc<[u8]>> {
        self.home()
            .state
            .crlg
            .secured
            .as_ref()
            .map(|p| std::sync::Arc::<[u8]>::from(p.bytes()))
    }

    /// Forgets a retired vehicle.
    pub fn retire(&mut self, node: NodeId) {
        self.access.retire(node);
        if let Some(n) = self.nodes.remove(&node) {
            // Its unsent reports leave with it; they are counted as unsent here because
            // the vehicle took them out of the run.
            self.report.reports_lost += n.outbox.0.len() as u64;
        }
        // Its own detector suite and the rate-limit stamps of what it checked go too: a
        // node id is never reused, and nothing but the node's own receptions reads them.
        // Kept, every vehicle that ever drove held its suite (every pseudonym it had heard,
        // and its co-location census) for the rest of the run.
        self.detectors.remove(&node);
        let checked: Vec<(NodeId, [u8; 8])> = self
            .checked_at
            .range((node, [0u8; 8])..=(node, [0xffu8; 8]))
            .map(|(k, _)| *k)
            .collect();
        for key in checked {
            self.checked_at.remove(&key);
        }
    }

    /// Registers the air digest one provisioned credential is carried under (joint 2).
    pub fn note_digest(
        &mut self,
        node: NodeId,
        digest: &v2xw_msg::sec_types::HashedId8,
        i: u32,
        j: u32,
    ) -> bool {
        self.by_digest
            .insert(digest_key(digest), (node, i, j))
            .is_none()
    }

    /// Arms this node as an attacker if the scenario's selection rule names it.
    pub fn arm_attacker(&mut self, ctx: &mut dyn ThreatCtx, node: NodeId, actor: ActorId) -> bool {
        let index = self.vehicles;
        self.vehicles += 1;
        for spec in &self.specs {
            let selected = if !spec.actor_ids.is_empty() {
                spec.actor_ids.contains(&actor.index())
            } else if let Some(count) = spec.count {
                index < u64::from(count)
            } else if let Some(fraction) = spec.fraction {
                ctx.rng(
                    v2xw_core::rng::RngDomain::Attack,
                    v2xw_core::rng::EntityRef::Node(node),
                )
                .bool(fraction)
            } else {
                false
            };
            if !selected {
                continue;
            }
            let schedule = v2xw_threat::AttackSchedule {
                from: spec.from,
                to: spec.to,
                ..v2xw_threat::AttackSchedule::default()
            };
            self.attackers.insert(
                node,
                AttackerSlot {
                    model: Box::new(LegacyAttacker::new(
                        node,
                        spec.params.clone(),
                        v2xw_threat::Capabilities::insider(self.params.jmax),
                        schedule.clone(),
                        Vec::new(),
                    )),
                    id: spec.id.clone(),
                    from: spec.from,
                    to: spec.to,
                },
            );
            self.report.attackers += 1;
            return true;
        }
        if self.detection_on {
            self.detectors
                .insert(node, Legacy12::new(self.detector_params.clone()));
        }
        false
    }

    /// Gives a roadside unit the local detector suite, when the scenario runs one.
    ///
    /// A unit verifies every frame it hears (`verify-all`, `wiring::build_rsu`) and the
    /// legacy authority counts it as trusted infrastructure — "never rate-limited or
    /// distrusted" (run.py `trusted()`, `LegacyWindow::trust_infrastructure`) — so a unit
    /// is a reporter like a vehicle, whose reports go straight onto its backhaul.
    pub fn arm_rsu_detector(&mut self, node: NodeId) {
        if self.detection_on {
            self.detectors
                .insert(node, Legacy12::new(self.detector_params.clone()));
        }
    }

    /// Counts one Sybil ghost frame handed to the access layer.
    pub fn note_ghost_frame(&mut self) {
        self.report.ghost_frames += 1;
    }

    /// Whether the local detector suite runs in this scenario.
    #[must_use]
    pub fn detection_on(&self) -> bool {
        self.detection_on
    }

    /// Notes the positional accuracy a safety message states, as it goes on the air (see
    /// [`broadcast_accuracy_95_m`]).
    pub fn note_broadcast_accuracy(
        &mut self,
        signer: [u8; 8],
        generated: SimTime,
        accuracy_95_m: Option<f64>,
    ) {
        let q = self.broadcast_accuracy.entry(signer).or_default();
        if q.len() >= ACCURACY_JOIN_DEPTH {
            q.pop_front();
        }
        q.push_back((generated, accuracy_95_m));
    }

    /// The accuracy a received message stated: `Some(None)` for "unavailable", `None` when
    /// the join has nothing (a frame that was never noted).
    fn accuracy_of(
        joined: &BTreeMap<[u8; 8], VecDeque<(SimTime, Option<f64>)>>,
        signer: &[u8; 8],
        generated: SimTime,
    ) -> Option<Option<f64>> {
        joined
            .get(signer)?
            .iter()
            .rev()
            .find(|(t, _)| *t == generated)
            .map(|(_, a)| *a)
    }

    /// Whether this node is an armed attacker.
    #[must_use]
    pub fn is_attacker(&self, node: NodeId) -> bool {
        self.attackers.contains_key(&node)
    }

    /// Lets an attacker edit the claim that is about to be signed.
    #[allow(clippy::too_many_arguments)]
    pub fn falsify(
        &mut self,
        ctx: &mut dyn ThreatCtx,
        node: NodeId,
        actor: ActorId,
        believed_time: SimTime,
        signer: [u8; 8],
        honest: HonestClaim,
        belief: SelfBelief,
        cert: (SimTime, SimTime),
        msg: u64,
        own_credentials: &[[u8; 8]],
    ) -> Option<Emission> {
        let slot = self.attackers.get_mut(&node)?;
        if believed_time < slot.from || believed_time >= slot.to {
            return None;
        }
        let view = AttackerView {
            own_rx: &[],
            own_credentials,
            crl_revocations_seen: None,
            own_belief: belief,
            honest,
            believed_time,
        };
        let mut out = Emission::honest(signer, honest, believed_time, cert.0, cert.1);
        let actions = slot.model.act(ctx, &view, &mut out);
        if actions.is_empty() {
            return Some(out);
        }
        let id = slot.id.clone();
        v2xw_threat::log_actions(ctx, believed_time, actor, &id, &actions, Some(msg));
        if v2xw_threat::is_falsified(&honest, &out, believed_time, StationType::Vehicle) {
            self.report.falsified_claims += 1;
            self.first_falsified.entry(node).or_insert(believed_time);
            let lies = self.attacker_lies.entry(node).or_insert((believed_time, 0));
            lies.0 = lies.0.max(believed_time);
            lies.1 += 1;
        }
        Some(out)
    }

    /// Runs the local detector suite over what one node's runtime delivered, returning any
    /// report the node decided to file.
    ///
    /// A reporter files about one subject at most once per `report_interval` (the legacy
    /// engine filed on every 1 s step its detectors fired): the authority's persistence
    /// gate needs evidence in several distinct seconds, and one report per pair ever could
    /// not give it that, while one per message would flood the uplink.
    pub fn detect(
        &mut self,
        ctx: &mut dyn ThreatCtx,
        world: &World,
        node: NodeId,
        me: &SelfBelief,
        reporter_digest: Option<String>,
        delivered: &[v2xw_node::VerifiedMessage],
    ) -> Vec<MisbehaviourReport> {
        let Some(detector) = self.detectors.get_mut(&node) else {
            return Vec::new();
        };
        let interval = secs(self.detector_params.generation_interval_s);
        let mut out = Vec::new();
        let mut truths: Vec<(String, SimTime)> = Vec::new();
        let mut heard: Vec<ObservedMessage> = Vec::new();
        let compromised = self.compromised_nodes.contains_key(&node);
        let map = RoadMap::new(world);
        let env: &dyn v2xw_threat::obs::LocalEnvironment =
            if self.map_check { &map } else { &NoMap };
        for m in delivered {
            let Some(signer) = &m.signer else { continue };
            let key = digest_key(signer);
            // The suite checks one message per sender per `generation_interval_s` (1 s,
            // `PipelineConfig.dt`), the rate every one of its thresholds was calibrated at:
            // `min_consecutive` = 2 is two *seconds* of violation there, the one-step
            // heading baseline needs a 5 m displacement, and the 1.5 s motion lag is a
            // count of steps. Fed every 10 Hz BSM, "two consecutive" becomes 0.2 s and the
            // heading check runs only when noise moves a claim 5 m in 100 ms — which is how
            // honest vehicles were reported. A frozen or falsified claim is as visible at
            // one check a second as at ten.
            if let Some(last) = self.checked_at.get(&(node, key))
                && m.received_at
                    < interval
                        .after(*last)
                        .saturating_sub(interval.as_nanos() / 10)
            {
                continue;
            }
            self.checked_at.insert((node, key), m.received_at);
            let claimed = m.claimed_pos.unwrap_or(Vec3::ZERO);
            // The sender's own statement of its accuracy, as the message carried it. A
            // message that says "unavailable" (or one the join never saw) earns no
            // tolerance beyond the suite's own floor: a sender cannot widen a receiver's
            // gate by withholding its accuracy.
            let stated =
                Self::accuracy_of(&self.broadcast_accuracy, &key, m.claimed_generation_time);
            match stated {
                Some(Some(_)) => self.report.accuracy_stated += 1,
                Some(None) => self.report.accuracy_unavailable += 1,
                None => self.report.accuracy_unjoined += 1,
            }
            let confidence_m = if self.stated_accuracy {
                stated.flatten().unwrap_or(0.0)
            } else {
                LEGACY_CONFIDENCE_M
            };
            let observed = ObservedMessage {
                signer: key,
                kind: match m.msg_type {
                    v2xw_msg::MsgType::Denm => ObservedKind::Denm(String::new()),
                    _ => ObservedKind::Beacon,
                },
                received_at: m.received_at,
                claimed_generation_time: m.claimed_generation_time,
                claimed_x_m: claimed.x,
                claimed_y_m: claimed.y,
                claimed_speed_mps: m.claimed_speed_mps,
                claimed_heading_rad: m.claimed_heading_rad,
                claimed_pos_confidence_m: confidence_m,
                repetitions: 1,
                cert_valid_from: 0,
                cert_valid_to: SimTime::MAX,
                // A PSM is a vulnerable road user's own declaration (J2735 PSM; a CAM's
                // stationType does the same in Europe): the suite then drops the vehicle
                // kinematic and map checks a pedestrian would fail honestly.
                station_type: if m.msg_type == v2xw_msg::MsgType::Psm {
                    StationType::Vru
                } else {
                    StationType::Vehicle
                },
                verification: match m.verification {
                    v2xw_node::stores::VerificationState::Verified => VerificationState::Valid,
                    v2xw_node::stores::VerificationState::Invalid => {
                        VerificationState::BadSignature
                    }
                    v2xw_node::stores::VerificationState::Revoked => {
                        VerificationState::UnknownCertificate
                    }
                    _ => VerificationState::Unverified,
                },
            };
            if compromised {
                heard.push(observed.clone());
            }
            let verdict = v2xw_threat::Detector::on_message(detector, ctx, me, &observed, env);
            self.report.messages_checked += 1;
            *self
                .report
                .verification_states
                .entry(format!("{:?}", m.verification))
                .or_insert(0) += 1;

            if !verdict.fired() {
                continue;
            }
            self.report.verdicts_fired += 1;
            for o in &verdict.fired {
                *self
                    .report
                    .verdicts_by_detector
                    .entry(o.detector.as_str().to_string())
                    .or_insert(0) += 1;
            }
            let pair = (node, verdict.subject.clone());
            if let Some(last) = self.filed.get(&pair)
                && m.received_at < self.report_interval.after(*last)
            {
                continue;
            }
            self.filed.insert(pair, m.received_at);
            let evidence = Evidence::at(m.received_at, m.received_at, confidence_m);
            let id = format!("r-{}-{}-{}", node.index(), verdict.subject, m.received_at);
            let reporter = reporter_digest
                .clone()
                .unwrap_or_else(|| v2xw_core::hash::hex_encode(&me.node.index().to_le_bytes()));
            if self.rsu_nodes.contains(&node) {
                for ma in &mut self.ma {
                    ma.trust_infrastructure(reporter.clone());
                }
            }
            if let Some(report) =
                MisbehaviourReport::from_verdict(id, node, reporter, &verdict, &evidence)
            {
                self.report.reports_sent += 1;
                truths.push((verdict.subject.clone(), m.received_at));
                out.push(report);
            }
        }
        for (subject, at) in truths {
            self.note_report_truth(&subject, at);
        }
        if compromised {
            let own = reporter_digest.unwrap_or_default();
            self.compromised_hears(ctx, node, &own, me, &heard);
        }
        out
    }

    /// The ground-truth join of a filed report, for the run report's precision and recall
    /// only: nothing here reaches a detector or the authority.
    fn note_report_truth(&mut self, subject_hex: &str, at: SimTime) {
        let Some(digest) = decode_hex8(subject_hex) else {
            return;
        };
        let Some(subject) = self.by_digest.get(&digest) else {
            return;
        };
        let subject = subject.0;
        let attacker = self.attackers.contains_key(&subject);
        if attacker {
            self.report.reports_about_attackers += 1;
            let entry = self.attacker_reports.entry(subject).or_default();
            entry.0 += 1;
            entry.1.insert(digest);
        } else {
            self.report.reports_about_honest += 1;
        }
        if let std::collections::btree_map::Entry::Vacant(e) = self.first_reported.entry(subject) {
            e.insert(at);
            if attacker {
                self.report.attackers_reported += 1;
            } else {
                self.report.honest_reported += 1;
            }
        }
    }

    /// Notes the authority's decision about `subject` for the run report's latencies.
    fn note_decision_truth(&mut self, subject: NodeId, at: SimTime) {
        if self.decided.contains_key(&subject) {
            return;
        }
        self.decided.insert(subject, at);
        if !self.attackers.contains_key(&subject) {
            self.report.decisions_honest += 1;
            return;
        }
        if let Some(first) = self.first_reported.get(&subject) {
            self.report
                .detection_to_decision_ns
                .push(at.saturating_sub(*first));
            if let Some(onset) = self.first_falsified.get(&subject) {
                self.report
                    .onset_to_detection_ns
                    .push(first.saturating_sub(*onset));
            }
        }
    }

    /// Resolves a report's subject to `(node, i, lv)` — the PCA's table (joint 2).
    fn resolve_subject(&self, digest_hex: &str) -> Option<(NodeId, u32, LinkageValue)> {
        let bytes = decode_hex8(digest_hex)?;
        let (node, i, j) = *self.by_digest.get(&bytes)?;
        let cred = self
            .creds
            .get(&node)?
            .iter()
            .find(|c| c.i == i && c.j == j)?;
        Some((node, cred.i, cred.lv))
    }

    /// Stores a report the vehicle could not upload yet in its report store; over the
    /// quota the store deletes its lowest-weight report (possibly this one).
    pub fn hold_report(&mut self, node: NodeId, report: MisbehaviourReport, detected_at: SimTime) {
        let unix = v2xw_core::time::WallClock::new(self.params.scms.pdu.t0_unix_s)
            .unix_seconds_at(detected_at);
        if let Some(n) = self.nodes.get_mut(&node) {
            let _evicted = n.outbox.0.push(
                (report, detected_at),
                u64::from(report_bytes()),
                mbr::Weighting::default(),
                unix,
            );
        }
    }

    /// Takes a vehicle's held reports at `now`, to try again, in the store's upload
    /// order (highest weight first, newer first on ties) after the 05:00-Eastern decays
    /// since the last look and the deletion of reports older than a week.
    pub fn take_outbox(
        &mut self,
        node: NodeId,
        now: SimTime,
    ) -> Vec<(MisbehaviourReport, SimTime)> {
        let unix =
            v2xw_core::time::WallClock::new(self.params.scms.pdu.t0_unix_s).unix_seconds_at(now);
        let Some(n) = self.nodes.get_mut(&node) else {
            return Vec::new();
        };
        drain_upload_order(&mut n.outbox.0, unix)
    }

    /// The reports `node` holds, in upload order, with their current weights.
    #[must_use]
    pub fn held_reports(&self, node: NodeId) -> Vec<(String, f64)> {
        let Some(n) = self.nodes.get(&node) else {
            return Vec::new();
        };
        let mut v: Vec<_> = n.outbox.0.iter().collect();
        v.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(core::cmp::Ordering::Equal)
                .then(b.seq.cmp(&a.seq))
        });
        v.into_iter()
            .map(|e| (e.item.0.report_id.clone(), e.weight))
            .collect()
    }

    /// The governance entities' scheduled EAC decisions that are due by `now`, then
    /// every decided CTL whose publication time has come ([`Governance::publish_due`]):
    /// a device fetching trust files after that installs it.
    ///
    /// [`Governance::publish_due`]: v2xw_proto::scms::governance::Governance::publish_due
    fn governance_due(&mut self, now: SimTime) {
        let st = &mut self.runs[0].state;
        if let Some(at) = self.params.root_rollover_at.filter(|t| *t <= now) {
            self.params.root_rollover_at = None;
            let (old, _new, _publish) = st.gov.rollover_root(&st.rng, &st.nodes, at);
            self.superseded_root = Some(old);
            self.report.gov_decisions += 1;
        }
        if let Some(at) = self.params.root_removal_at.filter(|t| *t <= now) {
            self.params.root_removal_at = None;
            if let Some(old) = self.superseded_root
                && st.gov.remove_root(old, at).is_some()
            {
                self.report.gov_decisions += 1;
            }
        }
        self.report.ctls_published += u64::from(st.gov.publish_due(now));
        // One CTL for every provider: the other providers' RAs serve what the SCMS
        // Manager published (WP-D1).
        if let Some((home, peers)) = self.runs.split_first_mut() {
            for run in peers {
                run.state.gov.sync_trust_from(&home.state.gov);
                // A home rollover re-certified the home hierarchy: every peer's copy of
                // it (and so its chain file) follows.
                run.state.gov.sync_peer_from(&home.state.gov);
            }
        }
    }

    fn note_unsecured(&mut self, why: &str) {
        self.report.reports_mbr_unsecured += 1;
        *self
            .report
            .reports_mbr_unsecured_by
            .entry(why.to_string())
            .or_insert(0) += 1;
    }

    /// A signed BSM `signer` put on the air: kept (the last [`EVIDENCE_BSMS`]) as the
    /// evidence a real misbehaviour report about that certificate attaches.
    pub fn note_signed_bsm(&mut self, signer: [u8; 8], spdu: &[u8]) {
        if self.etsi.is_some() || self.home().state.pki.is_none() {
            return;
        }
        let q = self.evidence.entry(signer).or_default();
        if q.len() == EVIDENCE_BSMS {
            q.pop_front();
        }
        q.push_back(spdu.to_vec());
    }

    /// Cellular vehicles with a multi-message backend flow (a top-up or a re-enrolment)
    /// in flight, in node order.
    #[must_use]
    pub fn cellular_flows_in_flight(&self) -> Vec<NodeId> {
        self.nodes
            .iter()
            .filter(|(_, n)| {
                n.access == Some(AccessKind::Cellular) && (n.topup.is_some() || n.reenrol.is_some())
            })
            .map(|(k, _)| *k)
            .collect()
    }

    /// Vehicles with reports held.
    #[must_use]
    pub fn nodes_with_outbox(&self) -> Vec<NodeId> {
        self.nodes
            .iter()
            .filter(|(_, n)| !n.outbox.0.is_empty())
            .map(|(k, _)| *k)
            .collect()
    }

    /// Counts a report uploaded over `kind`.
    pub fn note_upload(&mut self, node: NodeId, kind: AccessKind) {
        match kind {
            AccessKind::Cellular => self.report.reports_uploaded_cellular += 1,
            AccessKind::RsuRelay => self.report.reports_uploaded_relay += 1,
            AccessKind::Offline => {}
        }
        if let Some(n) = self.nodes.get_mut(&node) {
            n.reports_uploaded += 1;
        }
    }

    /// Counts a report a roadside unit put on its backhaul.
    pub fn note_rsu_report(&mut self) {
        self.report.reports_from_rsus += 1;
    }

    /// Counts a report the access link lost.
    pub fn note_report_lost(&mut self) {
        self.report.reports_lost += 1;
    }

    /// A report reaches the Location Obscurer Proxy, its access leg paid.
    ///
    /// The backend takes it from here: the RA's report shuffle, the forward to the MA,
    /// the MA's own verification. The authority's *decision* happens when the MA has
    /// ingested it ([`Phase2::advance`]), not here.
    ///
    /// Under the SCMS with the PDU layer on, the report is a real US misbehaviour report
    /// ([`Phase2::real_mbr`]) whenever its detection has a PSID-0x20 type and subtype:
    /// the RA checks the enrolment-signed upload header, the MA decrypts it and verifies
    /// the evidence, and each refusal is on `scms.state.pdu_rejections`.
    #[allow(clippy::too_many_arguments)]
    pub fn report_at_proxy(
        &mut self,
        report: MisbehaviourReport,
        reporter: NodeId,
        detected_at: SimTime,
        sent_at: SimTime,
        arrive_at: SimTime,
        transport: Transport,
        location: v2xw_sec::envelope::GenerationLocation,
    ) {
        self.report.reports_received += 1;
        let kind = if transport == Transport::RsuBackhaul {
            "report-relay"
        } else {
            "report-uu"
        };
        self.access.note_exchange(kind, sent_at, arrive_at);
        let Some((subject, i, lv)) = self.resolve_subject(&report.subject_cert_digest) else {
            return;
        };
        if self.runs.len() > 1 {
            let names = &self.runs[0].state.deployment.providers;
            let of = |n: NodeId| self.device_provider.get(&device_of(n)).copied();
            if let (Some(a), Some(b)) = (of(reporter), of(subject)) {
                let key = format!("{}<-{}", names[a].name, names[b].name);
                *self.report.reports_by_provider.entry(key).or_insert(0) += 1;
            }
        }
        if let Some(etsi) = self.etsi.as_mut() {
            // TS 103 759: the report goes to the MA, signed with the reporter's ticket and
            // encrypted to the authority — no RA shuffle in the ETSI system.
            let run = etsi.report_at_ma(
                device_of(reporter),
                device_of(subject),
                detected_at,
                sent_at,
                arrive_at,
                transport,
                report_bytes(),
            );
            self.in_flight
                .insert((0, run), InFlightReport { report, subject });
            return;
        }
        // The reporter's own provider's RA and MA receive it (WP-D1), after the internet
        // leg from the city to that provider's region.
        let kr = self.provider_of_device(device_of(reporter));
        let arrive_at = arrive_at.saturating_add(self.internet_legs[kr].as_nanos());
        // Why the report travels on the modelled path, when it does (the PKI view's case
        // evidence names it with the detector's checks).
        let mut modelled: Option<String> = None;
        let run = match self.real_mbr(&report, location) {
            MbrBuild::Real(mbr) => {
                match self.runs[kr].secure_mbr(device_of(reporter), &mbr, sent_at) {
                    Ok((si, slv, secured)) => {
                        self.report.reports_mbr_real += 1;
                        Some(self.runs[kr].secured_at_proxy(
                            device_of(reporter),
                            si,
                            slv,
                            secured,
                            detected_at,
                            sent_at,
                            arrive_at,
                            transport,
                        ))
                    }
                    Err(e) => {
                        self.note_unsecured(&e.to_string());
                        modelled = Some(format!("the report could not be secured: {e}"));
                        None
                    }
                }
            }
            MbrBuild::Unmapped => {
                self.report.reports_mbr_unmapped += 1;
                modelled = Some("no SM-MBR v1.1 subtype maps the detector checks".to_string());
                None
            }
            MbrBuild::Unsecured(why) => {
                self.note_unsecured(why);
                modelled = Some(why.to_string());
                None
            }
            MbrBuild::Off => {
                modelled = Some("real reports are off (no SCMS PDU layer)".to_string());
                None
            }
        };
        let run = match run {
            Some(run) => run,
            None => {
                let run = self.runs[kr].report_at_proxy(
                    device_of(reporter),
                    i,
                    lv,
                    detected_at,
                    sent_at,
                    arrive_at,
                    transport,
                    report_bytes(),
                );
                let why = modelled.as_deref().unwrap_or("modelled");
                self.runs[kr].modelled_report_evidence(run, why, &report.reason_codes);
                run
            }
        };
        self.in_flight
            .insert((kr, run), InFlightReport { report, subject });
    }

    /// The US misbehaviour report ([SM-MBR] v1.1, PSID 0x20) a device files for `r`.
    ///
    /// The type and subtype come from the first fired check the v1.1 object set has:
    /// `constantPositionFrozen` → 1VInconsistent/ConstantPosition, `positionJump` →
    /// 1VInconsistent/RandomPosition, `implausibleAcceleration` →
    /// Implausible/MaxAcceleration, each with the [SM-MBR] §2.3.1 default parameters
    /// (1 m drift, 10 m/s²). ObsImplausible/Distance is off by default in the US profile
    /// (a relay makes an innocent sender look far away, §2.3.1.4.2), and no other check
    /// has a subtype, so those reports stay on the modelled path and are counted.
    ///
    /// The evidence is the accused certificate's last signed BSMs on the air, byte for
    /// byte, with that certificate as `signingCertificate`. **Approximation:** the node
    /// layer does not keep the frames a receiver decoded, so these are the sender's last
    /// transmitted SPDUs, which the reporter heard if it detected on them.
    fn real_mbr(
        &self,
        r: &MisbehaviourReport,
        location: v2xw_sec::envelope::GenerationLocation,
    ) -> MbrBuild {
        if self.etsi.is_some() || self.home().state.pki.is_none() {
            return MbrBuild::Off;
        }
        let Some(subtype) = r.reason_codes.iter().find_map(|c| us_subtype(c)) else {
            return MbrBuild::Unmapped;
        };
        if !r.reporter.is_some_and(|n| {
            self.run_of(device_of(n))
                .state
                .devices
                .contains_key(&device_of(n))
        }) {
            // A roadside unit holds no pseudonym or enrolment certificate of this SCMS
            // (its application certificate is the factory's until WP-B3b is wired).
            return MbrBuild::Unsecured("the reporter holds no SCMS enrolment");
        }
        let Some(digest) = decode_hex8(&r.subject_cert_digest) else {
            return MbrBuild::Unsecured("the accused digest does not parse");
        };
        let Some((node, i, j)) = self.by_digest.get(&digest).copied() else {
            return MbrBuild::Unsecured("the accused certificate is not one the run issued");
        };
        let Some(cert) = self
            .run_of(device_of(node))
            .state
            .devices
            .get(&device_of(node))
            .and_then(|d| d.credentials.get(&(i, j)))
            .and_then(|c| c.implicit.as_deref())
            .map(|c| c.certificate.clone())
        else {
            return MbrBuild::Unsecured("the accused certificate is not an implicit one");
        };
        let bsms: Vec<_> = self
            .evidence
            .get(&digest)
            .into_iter()
            .flatten()
            .filter_map(|b| mbr::decode_signed_bsm(b).ok())
            .collect();
        let mbr_type = subtype.mbr_type();
        let bsms = match mbr_type {
            mbr::MbrType::OneVInconsistent if bsms.len() >= 2 => bsms,
            mbr::MbrType::Implausible if !bsms.is_empty() => bsms[bsms.len() - 1..].to_vec(),
            _ => return MbrBuild::Unsecured("too few of the accused's signed BSMs heard"),
        };
        let generation_time = self.home().state.time32(r.generation_time);
        MbrBuild::Real(mbr::MisbehaviorReport {
            generation_time,
            location,
            mbr_type,
            subtypes: vec![subtype],
            evidence: mbr::Evidence::Suspect(mbr::SignedBsms { bsms, cert }),
        })
    }

    /// Runs the backend to `now` and turns what happened into records and actions.
    ///
    /// Every provider's run steps to `now` (WP-D1). A report one provider's MA shares
    /// with the provider that issued the accused certificate lands in that provider's
    /// run, which then steps again, so a shared report is never a step late.
    #[allow(clippy::too_many_lines)]
    pub fn advance(&mut self, now: SimTime) -> BackendTick {
        if self.etsi.is_some() {
            return self.advance_etsi(now);
        }
        let mut tick = BackendTick::default();
        self.governance_due(now);
        // Start any queued case the backend can take.
        self.start_cases(now);
        let providers = self.runs.len();
        let mut stamps: Vec<Vec<v2xw_proto::stage::StageStamp>> = vec![Vec::new(); providers];
        for _pass in 0..=providers {
            let mut shared = false;
            for k in 0..providers {
                if let Err(e) = self.runs[k].run_until(now) {
                    self.note_backend_error(&e);
                }
                // Stage stamps since the last look.
                let new: Vec<v2xw_proto::stage::StageStamp> = {
                    let (new, cursor) = self.runs[k].kernel.stages_since(self.stage_cursor[k]);
                    let v = new.to_vec();
                    self.stage_cursor[k] = cursor;
                    v
                };
                let (decided, forwarded) = self.ingest_arrivals(k, &new, &mut tick);
                shared |= forwarded;
                stamps[k].extend(new);
                for (subject, subject_digest, report_run, _t) in decided {
                    self.queue[k].push_back(Case {
                        provider: k,
                        subject,
                        subject_digest,
                        report_run,
                        runs: None,
                        entry: None,
                        stamps_emitted: BTreeSet::new(),
                        enforced_first: None,
                        enforcers: Vec::new(),
                    });
                }
            }
            if !shared {
                break;
            }
        }
        self.backend_now = now;
        self.start_cases(now);
        for k in 0..providers {
            if (!self.queue[k].is_empty() || self.runs[k].case_open())
                && let Err(e) = self.runs[k].run_until(now)
            {
                self.note_backend_error(&e);
            }
        }
        // Cases: note issuance, and emit the stages each has reached, each on the
        // deciding provider's own log.
        for case in &mut self.cases {
            let Some((resolution, issuance)) = case.runs else {
                continue;
            };
            let run = &self.runs[case.provider];
            let log = &run.kernel.stages;
            if case.entry.is_none()
                && log.at(issuance, StageId::Issued).is_some()
                && let Some(entry) = run.state.crlg.entries.last()
            {
                self.report.crls_issued += 1;
                *self
                    .report
                    .revocations_by_provider
                    .entry(run.state.deployment.providers[case.provider].name.clone())
                    .or_insert(0) += 1;
                // The ground-truth join, for the run report only: whether the device the
                // authority revoked was the one that lied.
                if !self.revoked_devices.insert(case.subject) {
                    self.report.revocations_repeated += 1;
                } else if self.attackers.contains_key(&case.subject) {
                    self.report.revoked_attackers += 1;
                } else {
                    self.report.revoked_honest += 1;
                }
                case.entry = Some((*entry, run.state.crlg.entries.len() as u32));
            }
            for (stage, run) in [
                (StageId::Detect, case.report_run),
                (StageId::ReportSent, case.report_run),
                (StageId::Shuffled, case.report_run),
                (StageId::ReportReceived, case.report_run),
                (StageId::Decision, resolution),
                (StageId::Resolved, resolution),
                (StageId::Blocklisted, resolution),
                (StageId::Issued, issuance),
            ] {
                if let Some(t) = log.at(run, stage)
                    && case.stamps_emitted.insert((stage as u8, t))
                {
                    tick.stages.push(crate::sec_records::ProtoRevocation::stage(
                        t,
                        stage.as_str(),
                        &case.subject_digest,
                        None,
                        None,
                        None,
                    ));
                }
            }
        }
        // Newly finished cases that issued nothing.
        let unresolved = self
            .cases
            .iter()
            .filter(|c| c.runs.is_some() && c.entry.is_none())
            .count() as u64;
        if self.runs.iter().all(|r| !r.case_open()) {
            self.report.cases_unresolved = unresolved;
        }
        // Publication, per provider: each CRL Store's new version is a CRL every RA
        // serves once the inter-provider link has carried it.
        for k in 0..providers {
            let st = &self.runs[k].state;
            let store = u32::try_from(st.crl_store.entries.len()).unwrap_or(u32::MAX);
            if store <= self.published[k] {
                continue;
            }
            self.published[k] = store;
            self.report.crl_versions_published += 1;
            tick.published = Some(store);
            self.crl_feeds[k].push(CrlVersion {
                at: now,
                version: store,
                entries: st.crl_store.entries.clone(),
                secured: st
                    .crlg
                    .secured
                    .as_ref()
                    .map(|p| std::sync::Arc::<[u8]>::from(p.bytes())),
            });
            let size = u64::from(crl_bytes(&st.sizes, store).bytes());
            for s in &stamps[k] {
                if s.stage == StageId::Published || s.stage == StageId::FirstRsuBroadcast {
                    for case in self.cases.iter_mut().filter(|c| c.provider == k) {
                        if case.entry.as_ref().is_some_and(|(_, v)| *v <= store)
                            && case.stamps_emitted.insert((s.stage as u8, s.t))
                        {
                            tick.stages.push(crate::sec_records::ProtoRevocation::stage(
                                s.t,
                                s.stage.as_str(),
                                &case.subject_digest,
                                Some(size),
                                Some(u64::from(store)),
                                None,
                            ));
                        }
                    }
                }
            }
        }
        // Every provider's roadside list (WP-D1): a unit relays each provider's CRL, as
        // every RA serves each one, so a vehicle of either provider hears both.
        self.broadcast_version = self.runs.iter().fold(0u32, |acc, r| {
            acc.saturating_add(
                u32::try_from(r.state.crl_broadcast.entries.len()).unwrap_or(u32::MAX),
            )
        });
        // Top-ups whose batch has been installed, on each vehicle's provider's log.
        let mut finished = Vec::new();
        for (node, n) in &self.nodes {
            let Some(run) = n.topup else { continue };
            let log = &self.run_of(device_of(*node)).kernel.stages;
            if let Some(t) = log
                .at_node(run, StageId::Installed, device_of(*node))
                .or_else(|| log.at(run, StageId::Installed))
            {
                finished.push((*node, run));
                let started = n.topup_started;
                self.access.note_exchange("topup", started, t);
            }
        }
        for (node, run) in finished {
            let next_i = self.nodes.get(&node).map_or(0, |n| n.last_period + 1);
            let fresh = self.creds_of_device(device_of(node), next_i);
            // What the vehicle downloaded (`sec.cert`'s `bytes`): the run's steps addressed
            // to the device. Summing every step of the run counted the RA's, ACA's and
            // LAs' own exchanges too, so a two-vehicle run reported 7,319 octets per
            // top-up against a cellular downlink that carried about 1,100.
            let device = device_of(node);
            let bytes = self
                .run_of(device)
                .kernel
                .steps
                .iter()
                .filter(|s| s.run == run && s.to == device)
                .map(|s| u64::from(s.bytes))
                .sum();
            if let Some(n) = self.nodes.get_mut(&node) {
                n.topup = None;
                if !fresh.is_empty() {
                    n.last_period = fresh.iter().map(|c| c.i).max().unwrap_or(n.last_period);
                }
            }
            if !fresh.is_empty() {
                self.report.topups_completed += 1;
                self.report.certs_topped_up += fresh.len() as u64;
                self.creds
                    .entry(node)
                    .or_default()
                    .extend(fresh.iter().cloned());
                tick.installs.push((node, fresh, bytes));
            }
        }
        // Top-ups the RA refused, and successor enrolments that finished either way.
        self.settle_refusals_and_renewals(&mut tick, now);
        // Bytes every provider's backend moved.
        for k in 0..providers {
            let (steps, cursor) = {
                let (new, cursor) = self.runs[k].kernel.steps_since(self.step_cursor[k]);
                (new.to_vec(), cursor)
            };
            self.step_cursor[k] = cursor;
            push_bytes(&mut tick, steps);
        }
        self.maybe_publish_view(now, &mut tick);
        tick
    }

    /// The earliest instant after `now` at which the credential system has something to
    /// do (WP-D2's fast-forward): a delivery or timer in the backend's own kernel (a
    /// shuffle release, a pre-generation, the CRL publication slot, a download poll, an
    /// MA review), a CTL due for publication or a scheduled root rollover or removal, or
    /// a vehicle's own clock reaching its top-up horizon, its re-enrolment lead, its next
    /// CRL fetch or trust-list fetch, or the eMBMS carousel. `None` when nothing is ever
    /// due again.
    ///
    /// Work the backend holds — a kernel delivery, a CTL, a governance decision — counts
    /// even when it is due at `now` itself: a request started in this very tick is
    /// injected at `now` and runs on the next step, as in a full run. A vehicle's own
    /// horizon counts only strictly after `now`: one already due that could not be started
    /// (no access at a frozen position) does not become startable by time alone, and the
    /// fast-forward retries it at its next stop. The backend view's cadence is
    /// deliberately not a wake: it observes, and changes nothing.
    #[must_use]
    pub fn next_wake(&self, now: SimTime) -> Option<SimTime> {
        let mut best: Option<SimTime> = None;
        let mut offer = |t: SimTime| {
            if t > now && best.is_none_or(|b| t < b) {
                best = Some(t);
            }
        };
        let held = |t: SimTime| t.max(now.saturating_add(1));
        // Every provider's own kernel and governance (WP-D1): a report one MA shares is
        // a delivery in the receiving provider's kernel, so it is covered here.
        for run in &self.runs {
            if let Some(t) = run.kernel.next_due() {
                offer(held(t));
            }
            for (t, _) in &run.state.gov.pending {
                offer(held(*t));
            }
        }
        if let Some(t) = self.etsi.as_ref().and_then(|e| e.kernel.next_due()) {
            offer(held(t));
        }
        // A provider's signed CRL reaching another provider's RA once it has crossed the
        // inter-provider link (WP-D1): the instant that RA starts serving it.
        let d = &self.home().state.deployment;
        for (k, feed) in self.crl_feeds.iter().enumerate() {
            for h in (0..self.crl_feeds.len()).filter(|&h| h != k) {
                let lag = d.inter_provider_latency(k, h).unwrap_or(Duration::ZERO);
                for v in feed {
                    offer(held(v.at.saturating_add(lag.as_nanos())));
                }
            }
        }
        if let Some(t) = self.params.root_rollover_at {
            offer(held(t));
        }
        if let Some(t) = self.params.root_removal_at {
            offer(held(t));
        }
        if self.access.crl_delivery() == crate::backend::CrlDelivery::Mbms
            && self.published_crl_for_provider(0).0 > 0
        {
            offer(self.mbms_next);
        }
        let us = self.etsi.is_none();
        let lead = self.params.scms.reenrol_lead.as_nanos();
        for n in self.nodes.values() {
            if n.access == Some(AccessKind::Cellular) && !n.crl_fetch_in_flight {
                offer(n.next_crl_fetch);
            }
            if n.topup.is_none()
                && !n.self_revoked
                && !n.blocked
                && n.etsi_batch.is_none()
                && let Some(t) = self.params.topup_at(n.last_period)
            {
                offer(t);
            }
            if us && n.reenrol.is_none() && !n.blocked && !n.self_revoked {
                offer(n.enrolment_until.saturating_sub(lead));
            }
            if !us && n.trust_fetch.is_none() {
                offer(n.next_trust_fetch);
            }
        }
        best
    }

    /// Acts on what the RA and the ECA answered: a refused top-up ends the request (a
    /// blocklisted vehicle asks for nothing more; an expired enrolment is renewed first),
    /// and a successor enrolment moves the vehicle's certificate expiry.
    fn settle_refusals_and_renewals(&mut self, tick: &mut BackendTick, now: SimTime) {
        let mut refused = Vec::new();
        let mut renewed = Vec::new();
        for (node, n) in &self.nodes {
            let device = device_of(*node);
            let scms = self.run_of(device);
            if n.topup.is_some()
                && let Some(reason) = scms.topup_refusal_of(device)
            {
                refused.push((*node, reason));
            }
            if let Some(run) = n.reenrol {
                if scms.kernel.stages.at(run, StageId::Installed).is_some() {
                    let until = scms
                        .state
                        .devices
                        .get(&device)
                        .and_then(|d| d.latest_enrolment())
                        .map_or(n.enrolment_until, |e| e.valid_until);
                    renewed.push((*node, Some(until)));
                } else if scms.reenrol_refusal_of(device).is_some() {
                    renewed.push((*node, None));
                }
            }
        }
        for (node, reason) in refused {
            use v2xw_proto::scms::msg::Refusal;
            self.report.topups_refused += 1;
            tick.events.push((node, "topup-refused"));
            if let Some(n) = self.nodes.get_mut(&node) {
                n.topup = None;
                match reason {
                    Refusal::Blocklisted => {
                        n.blocked = true;
                        self.report.topups_refused_blocklisted += 1;
                    }
                    // The certificate it signed with had run out: renew before asking
                    // again (the ECA refuses an expired one, which is then final).
                    Refusal::EnrolmentExpired => n.enrolment_until = 0,
                    Refusal::UnknownEnrolment | Refusal::ProfileViolation => n.blocked = true,
                    // `ra-maxReqs` = 1: a second butterfly request. A vehicle that holds
                    // an account tops off by download, so this is a request raced
                    // against its own first ack; it asks again on the next top-up.
                    Refusal::RequestLimit => {}
                }
            }
        }
        for (node, until) in renewed {
            let Some(n) = self.nodes.get_mut(&node) else {
                continue;
            };
            n.reenrol = None;
            let started = n.reenrol_started;
            match until {
                Some(t) => {
                    n.enrolment_until = t;
                    self.access.note_exchange("reenrol", started, now);
                    self.report.reenrolments_completed += 1;
                    tick.events.push((node, "reenrolled"));
                }
                None => {
                    // Blocklisted, or already expired: the ECA will not renew it and the
                    // vehicle's credentials end with its last pseudonym batch.
                    n.blocked = true;
                    self.report.reenrolments_refused += 1;
                    tick.events.push((node, "reenrol-refused"));
                }
            }
        }
    }

    /// ETSI stations due to ask the Distribution Centre for the ECTL and the CA-CRL, with
    /// the access to use. Advances each one's next fetch by `crl_fetch_interval_s`, which
    /// under the CCMS is the interval of this fetch (the CCMS has no vehicle CRL).
    pub fn trust_fetches_due(&mut self, now: SimTime) -> Vec<(NodeId, AccessKind)> {
        if self.etsi.is_none() {
            return Vec::new();
        }
        let interval = self.params.crl_fetch_interval;
        let mut out = Vec::new();
        for (node, n) in &mut self.nodes {
            if n.trust_fetch.is_some() || n.next_trust_fetch > now {
                continue;
            }
            n.next_trust_fetch = interval.after(now);
            out.push((*node, n.access.unwrap_or(AccessKind::Offline)));
        }
        out
    }

    /// Starts one station's Distribution Centre fetch over `link`.
    pub fn start_trust_fetch(&mut self, node: NodeId, link: v2xw_proto::Link, now: SimTime) {
        let Some(etsi) = self.etsi.as_mut() else {
            return;
        };
        let device = device_of(node);
        etsi.set_access(device, link);
        let held = (
            etsi.installed_ctl.get(&device).copied().unwrap_or(0),
            etsi.installed_ca_crl_seq.get(&device).copied().unwrap_or(0),
        );
        let run = etsi.fetch_trust_lists(device, now);
        if let Some(n) = self.nodes.get_mut(&node) {
            n.trust_fetch = Some((run, held.0, held.1));
            n.trust_started = now;
        }
    }

    /// Vehicles whose enrolment certificate is within the renewal lead of expiring and
    /// that are not already renewing, with the access to use.
    pub fn reenrolments_due(&mut self, now: SimTime) -> Vec<(NodeId, AccessKind)> {
        if self.etsi.is_some() {
            return Vec::new();
        }
        let lead = self.params.scms.reenrol_lead;
        self.nodes
            .iter()
            .filter(|(_, n)| {
                n.reenrol.is_none()
                    && !n.blocked
                    && !n.self_revoked
                    && lead.after(now) >= n.enrolment_until
            })
            .map(|(node, n)| (*node, n.access.unwrap_or(AccessKind::Offline)))
            .collect()
    }

    /// Starts one vehicle's successor enrolment over `link`.
    pub fn start_reenrol(&mut self, node: NodeId, link: v2xw_proto::Link, now: SimTime) {
        let device = device_of(node);
        let scms = self.run_of_mut(device);
        scms.set_access(device, link);
        let run = scms.reenrol_at(device, now);
        self.report.reenrolments_started += 1;
        if let Some(n) = self.nodes.get_mut(&node) {
            n.reenrol = Some(run);
            n.reenrol_started = now;
        }
    }

    /// Publishes the backend's state on the view's cadence.
    fn maybe_publish_view(&mut self, now: SimTime, tick: &mut BackendTick) {
        if now < self.next_view {
            return;
        }
        self.next_view = self.params.view_interval.after(now);
        tick.view = Some(self.backend_view(now));
        self.publish_pki(now, tick);
        self.views = self.views.wrapping_add(1);
    }

    /// The PKI view's records at `now` (WP-E1): the trust domain's state, every message,
    /// stage stamp and SPDU since the last view, the case board's transitions and, every
    /// [`KNOWLEDGE_EVERY`] views, what each entity holds. Published SPDUs' bytes are
    /// dropped from the run's log once a record carries them, so a long run keeps none.
    fn publish_pki(&mut self, now: SimTime, tick: &mut BackendTick) {
        use v2xw_proto::scms::pki_view;
        if self.etsi.is_some() {
            return;
        }
        let Some(mut tracker) = self.pki.take() else {
            return;
        };
        tick.pki.state = pki_view::pki_state(&self.runs, now, &mut tracker);
        tick.pki.flows = pki_view::drain_flows(&self.runs, &mut tracker);
        for run in &mut self.runs {
            for p in &mut run.state.pdus {
                p.bytes = None;
            }
        }
        let vehicles = u32::try_from(self.nodes.len()).unwrap_or(u32::MAX);
        let enforced = |k: usize, c: &v2xw_proto::scms::run::Case| -> Option<(Vec<SimTime>, u32)> {
            let ours = self
                .cases
                .iter()
                .find(|e| e.provider == k && e.runs.is_some_and(|(r, _)| r == c.run))?;
            ours.entry.as_ref()?;
            let list = ours.enforcers.iter().map(|&(_, at)| at).collect();
            Some((list, vehicles))
        };
        tick.pki.cases = pki_view::drain_cases(&self.runs, &mut tracker, now, &enforced);
        if self.views % KNOWLEDGE_EVERY == 0 {
            for run in &self.runs {
                for role in pki_view::KNOWLEDGE_ROLES {
                    if let Some(k) = pki_view::knowledge(run, role, now, KNOWLEDGE_ROWS) {
                        tick.pki.knowledge.push(k);
                    }
                }
            }
        }
        self.pki = Some(tracker);
    }

    /// The PKI view's last records at the run's end (WP-F1): the SPDUs, stage stamps and
    /// case transitions logged since the last view, so a recording carries every SPDU the
    /// run put on a link (the view publishes on its cadence, and without this the last
    /// interval's were never recorded). `None` when nothing is pending, so a run whose
    /// last view drained everything records exactly what it did before.
    pub fn final_pki(&mut self, now: SimTime) -> Option<PkiTick> {
        let pending = self.pki.as_ref().is_some_and(|t| {
            self.runs
                .iter()
                .enumerate()
                .any(|(k, r)| r.state.pdus.len() > t.pdus_published(k))
        });
        if !pending {
            return None;
        }
        let mut tick = BackendTick::default();
        self.publish_pki(now, &mut tick);
        Some(tick.pki)
    }

    /// `node.credentials` for one vehicle (WP-E1), or `None` when the PKI view is off,
    /// the node is not an SCMS device, or its pool has not changed since the last record.
    pub fn node_credentials(
        &mut self,
        node: NodeId,
        now: SimTime,
        certs: &v2xw_node::CertStore,
    ) -> Option<v2xw_proto::scms::pki_view::NodeCredentials> {
        use v2xw_proto::scms::pki_view::HeldCert;
        if self.etsi.is_some() || !self.nodes.contains_key(&node) {
            return None;
        }
        let active = certs
            .active()
            .map(|c| v2xw_sec::hashedid::id8_bytes(&c.digest));
        let held: Vec<HeldCert> = certs
            .credentials()
            .iter()
            .map(|c| HeldCert {
                i: c.i_period,
                j: c.j_index,
                id8: v2xw_sec::hashedid::id8_bytes(&c.digest),
                coer: c.cert_coer.clone(),
                state: match c.state {
                    v2xw_node::CredState::Active if c.is_valid_at(now) => "valid",
                    v2xw_node::CredState::Active | v2xw_node::CredState::Expired => {
                        if now >= c.valid_until {
                            "expired"
                        } else {
                            "downloaded"
                        }
                    }
                    v2xw_node::CredState::Preloaded => "downloaded",
                    v2xw_node::CredState::Revoked => "revoked",
                },
            })
            .collect();
        let k = self
            .device_provider
            .get(&device_of(node))
            .copied()
            .unwrap_or(0);
        // Revoked as the vehicle itself knows it: it found itself on a CRL (an engine
        // run's vehicles install lists on the node, not in the backend's device record).
        let self_revoked = self.nodes.get(&node).is_some_and(|n| n.self_revoked);
        let mut tracker = self.pki.take()?;
        let rec = self.runs.get(k).and_then(|run| {
            v2xw_proto::scms::pki_view::node_credentials(
                run,
                device_of(node),
                now,
                &held,
                active,
                self_revoked,
                &mut tracker,
            )
        });
        self.pki = Some(tracker);
        // The engine's node id, not the backend's device id, so a reader joins the
        // record to the node's other channels.
        // Revoked as the vehicle itself knows it: it found itself on a CRL (the backend's
        // device record is not where an engine run's vehicles install lists).
        rec.map(|mut r| {
            r.node = node.index();
            r
        })
    }

    /// Every backend entity, the roadside units and the vehicles, and the traffic between
    /// them, at `now`: the credential system's own view (`v2xw_proto::view`) with what the
    /// engine carries itself — CRL downloads over the vehicles' Uu, roadside broadcasts,
    /// relayed reports and the authority's decisions — added to the entities that did it.
    pub fn backend_view(&mut self, now: SimTime) -> BackendView {
        let mut view = match self.etsi.as_ref() {
            Some(e) => e.backend_view(now, &mut self.tracker),
            None => self.runs[0].backend_view(now, &mut self.tracker),
        };
        let r = &self.report;
        let a_now = self.access.report_now();
        let a = &a_now;
        for e in &mut view.entities {
            match e.id.as_str() {
                "crl-store" => {
                    e.set("downloads_served", r.crl_downloads);
                    e.set("versions_published", r.crl_versions_published);
                }
                "crl-broadcast" => {
                    e.set("rsu_frames", r.crl_broadcasts);
                }
                "ma" => {
                    e.set("reports_ingested", r.reports_at_ma);
                    e.set("revocation_decisions", r.ma_revoke_decisions);
                    e.set("cases_opened", r.cases_opened);
                    e.set("decision_rule", self.ma[0].id());
                    e.set(
                        "subjects_under_evidence",
                        self.ma.iter().map(MaHost::subjects).sum::<usize>() as u64,
                    );
                    if let Some(peak) = self
                        .ma
                        .iter()
                        .filter_map(MaHost::peak_unrevoked_events)
                        .max()
                    {
                        // How close an unrevoked pseudonym came to the threshold: the
                        // margin an honest fleet leaves.
                        e.set("most_corroborated_events_unrevoked", u64::from(peak));
                    }
                }
                "ee" => {
                    e.set("vehicles", self.nodes.len() as u64);
                    e.set("reports_filed", r.reports_sent);
                    e.set("reports_over_cellular", r.reports_uploaded_cellular);
                    e.set("reports_via_rsu", r.reports_uploaded_relay);
                    e.set(
                        "reports_waiting_for_coverage",
                        self.nodes
                            .values()
                            .map(|n| n.outbox.0.len() as u64)
                            .sum::<u64>(),
                    );
                    e.set("topups_started", r.topups_started);
                    e.set("topups_completed", r.topups_completed);
                    e.set("topups_refused", r.topups_refused);
                    e.set(
                        "topups_in_flight",
                        self.nodes.values().filter(|n| n.topup.is_some()).count() as u64,
                    );
                    e.set("certificates_topped_up", r.certs_topped_up);
                    e.set("pseudonym_changes", r.pseudonym_changes);
                    e.set("reenrolments_completed", r.reenrolments_completed);
                    if self.etsi.is_some() {
                        e.set("trust_fetches", r.trust_fetches);
                        e.set("trust_lists_installed", r.trust_lists_installed);
                    }
                    e.set("vehicles_starved", r.vehicles_starved);
                    e.set("crl_installs", r.crls_installed);
                    e.set("revoked_receptions", r.revoked_receptions);
                    e.set("access_cellular", a.cellular_vehicles);
                    e.set("access_rsu_relay", a.relay_only_vehicles);
                    e.set("access_offline", a.offline_vehicles);
                    e.set(
                        "cert_unavailable_s",
                        (r.cert_unavailable_ns as f64 / 1e9 * 10.0).round() / 10.0,
                    );
                    // Backend latency per exchange type, ms (mean and worst so far).
                    for (kind, l) in &a.exchange_latency {
                        let key = kind.replace('-', "_");
                        e.set(
                            &format!("{key}_latency_ms"),
                            (l.mean_ms() * 10.0).round() / 10.0,
                        );
                        e.set(&format!("{key}_latency_max_ms"), l.max_ns / 1_000_000);
                    }
                    if a.mbms_broadcasts > 0 {
                        e.set("crl_mbms_broadcasts", a.mbms_broadcasts);
                        e.set("crl_mbms_receptions", a.mbms_receptions);
                    }
                }
                _ => {}
            }
        }
        if !self.rsus.is_empty() {
            let mut rsu = v2xw_proto::view::EntityView {
                id: "rsu".to_string(),
                name: "Roadside units".to_string(),
                system: view.system,
                tier: "distribution",
                online: true,
                node: None,
                role: "Relay reports and top-ups over their backhaul and repeat the CRL on \
                       the air; never on the safety channel's behalf.",
                queue: None,
                traffic: v2xw_proto::kernel::NodeTraffic::default(),
                ops: std::collections::BTreeMap::new(),
                state: std::collections::BTreeMap::new(),
            };
            rsu.set("units", self.rsus.len() as u64);
            rsu.set(
                "with_backhaul",
                self.rsus.iter().filter(|s| s.backhaul.connected).count() as u64,
            );
            rsu.set("crl_role", self.rsus_with_role("crl").len() as u64);
            rsu.set("crl_frames", r.crl_broadcasts);
            rsu.set("reports_filed_by_units", r.reports_from_rsus);
            rsu.set("reports_relayed", r.reports_uploaded_relay);
            rsu.set("backhaul_messages", a.carrier.backhaul_messages);
            rsu.set("backhaul_line_bytes", a.carrier.backhaul_line_bytes);
            rsu.set("backhaul_retransmissions", a.carrier.backhaul_retx);
            rsu.set("compromised", self.compromised_count() as u64);
            rsu.set("in_cert_cycle", r.rsus_in_cert_cycle);
            rsu.set("app_certs_preloaded", r.rsu_app_certs_preloaded);
            rsu.set("app_certs_renewed", r.rsu_app_certs_renewed);
            rsu.set(
                "starving",
                self.rsu_cycle
                    .values()
                    .filter(|c| c.starved_since.is_some())
                    .count() as u64,
            );
            rsu.set("ever_starved", r.rsus_starved);
            rsu.set("messages_withheld", r.rsu_messages_withheld);
            rsu.set("crls_installed", r.rsu_crls_installed);
            rsu.set("revoked_receptions", r.rsu_revoked_receptions);
            view.entities.push(rsu);
        }
        // What the engine carried outside the credential kernel, as edges.
        let mut extra = |from: &str, to: &str, messages: u64, step: &str, transport: &str| {
            if messages == 0 {
                return;
            }
            view.edges.push(v2xw_proto::view::EdgeView {
                from: from.to_string(),
                to: to.to_string(),
                messages,
                bytes: 0,
                last_t: now,
                last_step: step.to_string(),
                transport: transport.to_string(),
                steps: std::iter::once((step.to_string(), messages)).collect(),
            });
        };
        extra(
            "crl-store",
            "ee",
            r.crl_downloads,
            "crl-download",
            "cellular-uu",
        );
        extra(
            "crl-broadcast",
            "rsu",
            u64::from(self.broadcast_version > 0) * r.crl_versions_published,
            "crl-to-rsu",
            "rsu-backhaul",
        );
        extra(
            "rsu",
            "ee",
            r.crl_broadcasts,
            "crl-air-broadcast",
            "v2x-air",
        );
        extra(
            "ee",
            "rsu",
            r.reports_uploaded_relay,
            "report-relay",
            "v2x-air",
        );
        // Every other provider's backend (WP-D1), its entities and edges named
        // `<provider>/<role>`: the home provider's keep their bare ids.
        if self.etsi.is_none() {
            self.peer_trackers
                .resize_with(self.runs.len().saturating_sub(1), EdgeTracker::default);
            for k in 1..self.runs.len() {
                let name = self.runs[0]
                    .state
                    .deployment
                    .providers
                    .get(k)
                    .map_or_else(|| format!("provider-{k}"), |p| p.name.clone());
                let peer = self.runs[k].backend_view(now, &mut self.peer_trackers[k - 1]);
                let id = |s: &str| format!("{name}/{s}");
                for mut e in peer.entities {
                    e.id = id(&e.id);
                    e.name = format!("{} ({name})", e.name);
                    e.set("provider", name.clone());
                    view.entities.push(e);
                }
                for mut e in peer.edges {
                    e.from = id(&e.from);
                    e.to = id(&e.to);
                    view.edges.push(e);
                }
                for mut s in peer.recent {
                    s.from = id(&s.from);
                    s.to = id(&s.to);
                    view.recent.push(s);
                }
                for (f, n) in peer.flows {
                    *view.flows.entry(f).or_default() += n;
                }
            }
            view.recent.sort_by_key(|s| s.t);
            let excess = view.recent.len().saturating_sub(v2xw_proto::view::RECENT);
            view.recent.drain(..excess);
        }
        view
    }

    /// How many roadside units are under an attacker's control.
    #[must_use]
    pub fn compromised_count(&self) -> usize {
        self.compromised_nodes.len()
    }

    /// The reports the authority received in `stamps`, through the persistence gate, in the
    /// order their evidence was observed (a shuffle releases a batch at one instant, and
    /// the gate dates evidence by observation, not by arrival). Returns each decision:
    /// `(subject node, certificate digest, the triggering report's run, the instant)`.
    /// The reports provider `k`'s MA received in `stamps`, and its decisions about them.
    ///
    /// A report about a certificate another provider issued cannot be resolved here: only
    /// the issuing provider's ACA and LAs can link it to a device (Provider Req. §2.2;
    /// 01 §9). With `ma_report_sharing` on (the US default) this MA forwards it over the
    /// MA-to-MA link to that provider's MA, which decides; off, it is counted as
    /// unresolvable and goes no further. Returns the decisions and whether anything was
    /// forwarded.
    fn ingest_arrivals(
        &mut self,
        k: usize,
        stamps: &[v2xw_proto::stage::StageStamp],
        tick: &mut BackendTick,
    ) -> (Vec<(NodeId, String, FlowRun, SimTime)>, bool) {
        let mut arrived: Vec<(SimTime, FlowRun, InFlightReport)> = Vec::new();
        let mut forwarded = false;
        for s in stamps {
            if s.stage == StageId::ReportReceived
                && let Some(f) = self.in_flight.remove(&(k, s.run))
            {
                let ks = self.provider_of_device(device_of(f.subject));
                if self.etsi.is_some() || ks == k {
                    arrived.push((s.t, s.run, f));
                    continue;
                }
                let Some((_, i, lv)) = self.resolve_subject(&f.report.subject_cert_digest) else {
                    continue;
                };
                if !self.home().state.deployment.ma_report_sharing {
                    self.runs[k].state.shared.unresolvable += 1;
                    continue;
                }
                // The report as this MA holds it: its real length when it was a real
                // report, re-encrypted to the accused's provider's MA.
                let held = self.runs[k]
                    .state
                    .ma
                    .reports
                    .iter()
                    .rposition(|r| r.subject_lv == lv);
                let ma_k = &self.runs[k].state.ma;
                let bytes = held
                    .and_then(|x| ma_k.reports[x].secured.as_ref().map(|s| s.report.len()))
                    .map_or(report_bytes(), |n| u32::try_from(n).unwrap_or(u32::MAX));
                // What this MA read from the report travels with it (the PKI view's case
                // evidence on the deciding provider's side).
                let evidence = held.and_then(|x| ma_k.report_evidence.get(x).cloned().flatten());
                let from = self.runs[k].state.nodes.ma;
                let observed = f.report.detection_time;
                let run = self.runs[ks].shared_report_at_ma(from, i, lv, observed, s.t, bytes);
                if let Some(e) = evidence {
                    self.runs[ks].shared_report_evidence(run, e);
                }
                self.runs[k].state.shared.forwarded += 1;
                // The report's history before the sharing leg (detection, upload, the
                // RA's shuffle) carries over to the deciding provider's log, so its
                // revocation decomposes end to end; its arrival there is the deciding
                // MA's `ReportReceived`.
                let prior: Vec<v2xw_proto::stage::StageStamp> = [
                    StageId::Detect,
                    StageId::ReportSent,
                    StageId::ProxyForwarded,
                    StageId::Shuffled,
                ]
                .into_iter()
                .filter_map(|stage| {
                    let log = &self.runs[k].kernel.stages;
                    log.at(s.run, stage).map(|t| v2xw_proto::stage::StageStamp {
                        t,
                        run,
                        flow: s.flow,
                        stage,
                        node: None,
                        size: None,
                    })
                })
                .collect();
                for p in prior {
                    self.runs[ks].kernel.stages.push(p);
                }
                self.in_flight.insert((ks, run), f);
                forwarded = true;
            }
        }
        arrived.sort_by(|a, b| {
            (a.2.report.detection_time, &a.2.report.report_id)
                .cmp(&(b.2.report.detection_time, &b.2.report.report_id))
        });
        let mut decisions = Vec::new();
        for (t, run, f) in arrived {
            self.report.reports_at_ma += 1;
            let mut r = f.report;
            r.ingest_time = t;
            tick.ma_reports.push(v2xw_threat::records::MaReportRecord {
                t,
                reporter: r.reporter,
                subject: r.subject_cert_digest.clone(),
                detector: r.leading_reason().map(str::to_string),
            });
            let action = self.ma[k].ingest_evidence(&r, r.detection_time);
            if self.attackers.contains_key(&f.subject) {
                *self.attacker_at_ma.entry(f.subject).or_insert(0) += 1;
                if let Some(events) = self.ma[k].corroborated_events(&r.subject_cert_digest) {
                    let peak = self.attacker_peak_events.entry(f.subject).or_insert(0);
                    *peak = (*peak).max(events);
                }
            }
            if let Some(MaAction::Revoke { subject }) = action {
                // The authority decides about a device, not a pseudonym. In SCMS it learns
                // the device behind a reported certificate by asking the PCA for the
                // linkage value's pre-linkage values and the linkage authorities for the
                // chain they belong to (CAMP SCMS PoC EE Requirements §5; Whyte et al.,
                // VNC 2013, §V); a second pseudonym on a chain already decided adds
                // evidence, not a second revocation. It used to open one case per
                // pseudonym: 22 decisions for 12 attackers in a 300 s run.
                if let Some((device, _, _)) = self.resolve_subject(&subject)
                    && !self.ma_decided_devices.insert(device)
                {
                    self.report.decisions_linked += 1;
                    continue;
                }
                self.report.ma_revoke_decisions += 1;
                tick.ma_decisions
                    .push(v2xw_threat::records::MaDecisionRecord {
                        t,
                        subject: subject.clone(),
                        decision: "revoke".to_string(),
                    });
                decisions.push((f.subject, subject, run, t));
            }
        }
        for (subject, _, _, t) in &decisions {
            self.note_decision_truth(*subject, *t);
        }
        (decisions, forwarded)
    }

    /// The ETSI deployment's share of a step: reports through the gate, a decision
    /// carried out as the EA's blocklist (passive revocation, TS 102 941 §6.1.6), ticket
    /// top-ups, and the bytes it moved. There is no per-vehicle revocation list to publish:
    /// "revocation of authorization tickets is not possible as passive revocation is
    /// preferred" [TS 102 941 §6.1.4 NOTE 4], so a blocked vehicle keeps signing until its
    /// last ticket expires, and that instant is the revocation's last stage.
    fn advance_etsi(&mut self, now: SimTime) -> BackendTick {
        let mut tick = BackendTick::default();
        let Some(etsi) = self.etsi.as_mut() else {
            return tick;
        };
        if let Err(e) = etsi.run_until(now) {
            self.note_backend_error(&e);
        }
        let stamps: Vec<v2xw_proto::stage::StageStamp> = {
            let etsi = self.etsi.as_ref().expect("checked");
            let (new, cursor) = etsi.kernel.stages_since(self.stage_cursor[0]);
            let v = new.to_vec();
            self.stage_cursor[0] = cursor;
            v
        };
        for s in &stamps {
            if s.stage == StageId::Blocklisted
                && let Some(b) = self
                    .blocks
                    .iter_mut()
                    .find(|b| b.run == s.run && b.blocked.is_none())
            {
                b.blocked = Some(s.t);
                tick.stages.push(crate::sec_records::ProtoRevocation::stage(
                    s.t,
                    "blocklisted",
                    &b.subject_digest,
                    None,
                    None,
                    None,
                ));
                let last = self
                    .creds
                    .get(&b.subject)
                    .and_then(|c| c.iter().map(|c| c.valid_until).max())
                    .unwrap_or(s.t);
                tick.stages.push(crate::sec_records::ProtoRevocation::stage(
                    last,
                    "last_valid_credential_expiry",
                    &b.subject_digest,
                    None,
                    None,
                    Some(b.subject),
                ));
                if self.revocation.is_none() {
                    let stages = vec![
                        (StageId::Detect, b.detected),
                        (StageId::Decision, b.decided),
                        (StageId::Blocklisted, s.t),
                        (StageId::LastValidCredentialExpiry, last),
                    ];
                    let total = Duration::from_nanos(last.saturating_sub(b.detected));
                    self.report.revocation_latency_ns = total.as_nanos();
                    self.report.revocation_stages = stages
                        .iter()
                        .map(|(st, t)| (st.as_str().to_string(), t.saturating_sub(b.detected)))
                        .collect();
                }
            }
        }
        for (subject, subject_digest, run, t) in self.ingest_arrivals(0, &stamps, &mut tick).0 {
            if self.blocks.iter().any(|b| b.subject == subject) {
                self.report.decisions_already_covered += 1;
                continue;
            }
            let detected = self
                .etsi
                .as_ref()
                .and_then(|e| e.kernel.stages.at(run, StageId::Detect))
                .unwrap_or(t);
            let block_run = self
                .etsi
                .as_mut()
                .expect("checked")
                .decide_block(device_of(subject), t);
            self.report.cases_opened += 1;
            if !self.revoked_devices.insert(subject) {
                self.report.revocations_repeated += 1;
            } else if self.attackers.contains_key(&subject) {
                self.report.revoked_attackers += 1;
            } else {
                self.report.revoked_honest += 1;
            }
            for (stage, at) in [("detect", detected), ("decision", t)] {
                tick.stages.push(crate::sec_records::ProtoRevocation::stage(
                    at,
                    stage,
                    &subject_digest,
                    None,
                    None,
                    None,
                ));
            }
            self.blocks.push(Block {
                subject,
                subject_digest,
                run: block_run,
                detected,
                decided: t,
                blocked: None,
            });
        }
        // Distribution Centre fetches that came back.
        let fetched: Vec<(NodeId, bool)> = {
            let etsi = self.etsi.as_ref().expect("checked");
            self.nodes
                .iter()
                .filter_map(|(node, n)| {
                    let (run, ctl, crl) = n.trust_fetch?;
                    etsi.kernel.stages.at(run, StageId::Processed)?;
                    let device = device_of(*node);
                    let newer = etsi.installed_ctl.get(&device).copied().unwrap_or(0) > ctl
                        || etsi.installed_ca_crl_seq.get(&device).copied().unwrap_or(0) > crl;
                    Some((*node, newer))
                })
                .collect()
        };
        for (node, newer) in fetched {
            if let Some(n) = self.nodes.get_mut(&node) {
                n.trust_fetch = None;
                let started = n.trust_started;
                self.access.note_exchange("trust-fetch", started, now);
            }
            self.report.trust_fetches += 1;
            if newer {
                self.report.trust_lists_installed += 1;
                tick.events.push((node, "trust-list-installed"));
            }
        }
        // Butterfly top-ups: once the EA holds the certified batch, the station asks for
        // it; a blocklisted station's download is answered empty (passive revocation).
        let ready: Vec<(NodeId, u32)> = self
            .nodes
            .iter()
            .filter_map(|(node, n)| match n.etsi_batch {
                Some((i, false))
                    if self
                        .etsi
                        .as_ref()
                        .is_some_and(|e| e.pending_batches.contains_key(&device_of(*node))) =>
                {
                    Some((*node, i))
                }
                _ => None,
            })
            .collect();
        for (node, i) in ready {
            if let Some(e) = self.etsi.as_mut() {
                let run = e.download_ats(device_of(node), i);
                if let Some(n) = self.nodes.get_mut(&node) {
                    n.etsi_batch = Some((i, true));
                    n.topup = Some(run);
                    n.etsi_target = Some(e.tickets_of(device_of(node)) + self.params.jmax);
                }
            }
        }
        // A butterfly request the EA refused outright never produces a batch.
        let refused: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(node, n)| {
                matches!(n.etsi_batch, Some((_, false)))
                    && self.etsi.as_ref().is_some_and(|e| {
                        e.blocklist.contains(&device_of(**node))
                            && !e.pending_batches.contains_key(&device_of(**node))
                    })
            })
            .map(|(node, _)| *node)
            .collect();
        for node in refused {
            if let Some(n) = self.nodes.get_mut(&node) {
                n.etsi_batch = None;
                n.topup = None;
                n.blocked = true;
            }
            self.report.topups_refused += 1;
            self.report.topups_refused_blocklisted += 1;
            tick.events.push((node, "topup-refused"));
        }
        // Ticket top-ups: a request per ticket, and the batch installs when the AA has
        // granted them all.
        let mut finished = Vec::new();
        for (node, n) in &self.nodes {
            if let Some(target) = n.etsi_target
                && self
                    .etsi
                    .as_ref()
                    .is_some_and(|e| e.tickets_of(device_of(*node)) >= target)
            {
                finished.push(*node);
            }
        }
        for node in finished {
            let i = self.nodes.get(&node).map_or(0, |n| n.last_period + 1);
            let fresh: Vec<ProvisionedCred> = (0..self.params.jmax)
                .map(|j| {
                    let (from, until) = self.params.scms.validity(i);
                    ProvisionedCred {
                        i,
                        j,
                        lv: LinkageValue::new([0u8; 9]),
                        valid_from: from,
                        valid_until: until,
                        key: None,
                    }
                })
                .collect();
            if let Some(n) = self.nodes.get_mut(&node) {
                n.etsi_target = None;
                n.topup = None;
                n.etsi_batch = None;
                n.last_period = i;
                let started = n.topup_started;
                self.access.note_exchange("topup", started, now);
            }
            self.report.topups_completed += 1;
            self.report.certs_topped_up += fresh.len() as u64;
            self.creds
                .entry(node)
                .or_default()
                .extend(fresh.iter().cloned());
            tick.installs.push((node, fresh, 0));
        }
        let (steps, cursor) = {
            let etsi = self.etsi.as_ref().expect("checked");
            let (new, cursor) = etsi.kernel.steps_since(self.step_cursor[0]);
            (new.to_vec(), cursor)
        };
        self.step_cursor[0] = cursor;
        push_bytes(&mut tick, steps);
        self.maybe_publish_view(now, &mut tick);
        tick
    }

    /// A backend delivery the kernel refused: a modelling defect (an unhosted entity, a
    /// missing link, a flow driven out of order), counted and kept so a run says so rather
    /// than going quiet.
    fn note_backend_error(&mut self, e: &v2xw_proto::ProtoError) {
        self.report.backend_errors += 1;
        if self.report.first_backend_error.is_empty() {
            self.report.first_backend_error = e.to_string();
        }
    }

    fn start_cases(&mut self, now: SimTime) {
        for k in 0..self.runs.len() {
            self.start_cases_of(k, now);
        }
    }

    /// Provider `k`'s MA takes its queued decisions, one open case at a time.
    fn start_cases_of(&mut self, k: usize, now: SimTime) {
        while !self.runs[k].case_open() {
            let Some(mut case) = self.queue[k].pop_front() else {
                return;
            };
            // The report the authority holds about the decided certificate: the latest
            // one, which is the one the decision came in on.
            let Some((_, i, lv)) = self.resolve_subject(&case.subject_digest) else {
                self.report.cases_unresolved += 1;
                continue;
            };
            let Some(index) = self.runs[k]
                .state
                .ma
                .reports
                .iter()
                .rposition(|r| r.subject_lv == lv)
            else {
                continue;
            };
            let _ = now;
            // A certificate the Generator's list already revokes needs no second case: the
            // authority holds the list and checks it before spending lookups [BRECHT §VI-D:
            // one entry revokes every certificate of the device from `i` on].
            let mut covered = v2xw_sec::CrlStore::new();
            for e in &self.runs[k].state.crlg.entries {
                covered.add_linkage_entry(*e);
            }
            if covered.revokes_linkage_at_period(i, lv) {
                self.report.decisions_already_covered += 1;
                continue;
            }
            // Revoke from the reported certificate's own period forward: its entry must
            // match the certificate that was reported, and forward-only linkage keeps
            // every earlier certificate unlinkable [BRECHT §VI-D].
            // Through the MA's decision mode: the CV-pilot manual review by default
            // (`ma_review_s`), the automatic MA when it is 0 (gap row 28).
            if let Some(runs) = self.runs[k].request_revocation(index, i, self.params.jmax) {
                self.report.cases_opened += 1;
                case.runs = Some(runs);
                self.cases.push(case);
            } else {
                self.queue[k].push_front(case);
                return;
            }
        }
    }

    /// The CRL versions provider `h`'s RA serves at the backend's last step: every
    /// provider's latest version once it has crossed the inter-provider link (its own at
    /// once). Each is `(provider, version)`.
    fn visible_versions(&self, h: usize) -> Vec<(usize, &CrlVersion)> {
        let d = &self.home().state.deployment;
        let now = self.backend_now;
        self.crl_feeds
            .iter()
            .enumerate()
            .filter_map(|(k, feed)| {
                let lag = if k == h {
                    Duration::ZERO
                } else {
                    d.inter_provider_latency(k, h).unwrap_or(Duration::ZERO)
                };
                feed.iter()
                    .rev()
                    .find(|v| v.at.saturating_add(lag.as_nanos()) <= now)
                    .map(|v| (k, v))
            })
            .collect()
    }

    /// The composite list vehicle `node`'s RA serves (WP-D1): its version (the sum of
    /// every visible provider's version), every visible provider's entries, and their
    /// signed CRLs.
    #[must_use]
    pub fn published_crl_for(
        &self,
        node: NodeId,
    ) -> (u32, Vec<CrlLinkageEntry>, Vec<std::sync::Arc<[u8]>>) {
        let h = self.provider_of_device(device_of(node));
        let mut version = 0u32;
        let mut entries = Vec::new();
        let mut secured = Vec::new();
        for (_, v) in self.visible_versions(h) {
            version = version.saturating_add(v.version);
            entries.extend(v.entries.iter().copied());
            secured.extend(v.secured.clone());
        }
        (version, entries, secured)
    }

    /// What the eMBMS carousel carries (`crl_delivery: mbms`): every provider's latest
    /// published CRL as its own CRL Generator signed it — the version (their sum), their
    /// entries, home provider's first, and the signed CRLs. **ASSUMPTION**: the mobile
    /// operator's broadcast centre is fed by every provider's CRL store directly, with no
    /// inter-provider lag; MBMS CRL distribution is specified but not deployed (00 §2.3),
    /// so no document says who feeds it.
    #[must_use]
    pub fn mbms_crl(&self) -> (u32, Vec<CrlLinkageEntry>, Vec<std::sync::Arc<[u8]>>) {
        let mut version = 0u32;
        let mut entries = Vec::new();
        let mut secured = Vec::new();
        for feed in &self.crl_feeds {
            if let Some(v) = feed.iter().rev().find(|v| v.at <= self.backend_now) {
                version = version.saturating_add(v.version);
                entries.extend(v.entries.iter().copied());
                secured.extend(v.secured.clone());
            }
        }
        (version, entries, secured)
    }

    /// The list the CRL Store has published: its version (entry count) and entries.
    #[must_use]
    pub fn published_crl(&self) -> (u32, Vec<CrlLinkageEntry>) {
        // The home provider's RA's composite list (a vehicle's own is
        // [`Phase2::published_crl_for`]).
        let (v, e, _) = self.published_crl_for_provider(0);
        (v, e)
    }

    fn published_crl_for_provider(
        &self,
        h: usize,
    ) -> (u32, Vec<CrlLinkageEntry>, Vec<std::sync::Arc<[u8]>>) {
        let mut version = 0u32;
        let mut entries = Vec::new();
        let mut secured = Vec::new();
        for (_, v) in self.visible_versions(h) {
            version = version.saturating_add(v.version);
            entries.extend(v.entries.iter().copied());
            secured.extend(v.secured.clone());
        }
        (version, entries, secured)
    }

    /// The list the roadside broadcast path holds: its version (the sum of every
    /// provider's entry count) and every provider's entries, the home provider's first.
    ///
    /// ASSUMPTION (WP-D1): a roadside unit relays every provider's CRL, each as its own
    /// provider's CRL Broadcast published it, the way every RA serves each provider's
    /// list; no published text assigns roadside CRL relay to one provider.
    #[must_use]
    pub fn broadcast_crl(&self) -> (u32, Vec<CrlLinkageEntry>) {
        let entries = self
            .runs
            .iter()
            .flat_map(|r| r.state.crl_broadcast.entries.iter().copied())
            .collect();
        (self.broadcast_version, entries)
    }

    /// The signed CRLs the roadside broadcast path relays unchanged: one per provider
    /// whose CRL Broadcast has published, in [`Phase2::broadcast_crl`]'s order. Empty with
    /// the PDU layer off.
    #[must_use]
    pub fn broadcast_secured_crls(&self) -> Vec<std::sync::Arc<[u8]>> {
        self.runs
            .iter()
            .filter_map(|r| r.state.crl_broadcast.secured.as_ref())
            .map(|p| std::sync::Arc::<[u8]>::from(p.bytes()))
            .collect()
    }

    /// The CRL's size on the wire for `entries` entries.
    #[must_use]
    pub fn crl_size(&self, entries: u32) -> u32 {
        crl_bytes(&self.home().state.sizes, entries).bytes()
    }

    /// The size of a CRL request, bytes.
    #[must_use]
    pub fn crl_request_size(&self) -> u32 {
        self.home().state.sizes.crl_request().bytes()
    }

    /// Vehicles whose CRL poll is due at `now`, with the version they hold. Advances each
    /// one's next poll.
    pub fn crl_polls_due(&mut self, now: SimTime) -> Vec<(NodeId, u32)> {
        let interval = self.params.crl_fetch_interval;
        let published: Vec<u32> = (0..self.runs.len())
            .map(|h| self.published_crl_for_provider(h).0)
            .collect();
        let providers = &self.device_provider;
        let mut out = Vec::new();
        for (node, n) in &mut self.nodes {
            if n.access != Some(AccessKind::Cellular) || n.crl_fetch_in_flight {
                continue;
            }
            if n.next_crl_fetch <= now {
                n.next_crl_fetch = interval.after(now);
                let h = providers.get(&device_of(*node)).copied().unwrap_or(0);
                if n.crl_version < published[h] {
                    n.crl_fetch_in_flight = true;
                    out.push((*node, n.crl_version));
                }
            }
        }
        out
    }

    /// Gives each vehicle's first poll a phase inside the interval, from a keyed draw, so
    /// a fleet does not poll in one instant.
    pub fn phase_crl_poll(
        &mut self,
        node: NodeId,
        rng: &v2xw_core::rng::RngRegistry,
        now: SimTime,
    ) {
        let interval = self.params.crl_fetch_interval.as_nanos();
        if let Some(n) = self.nodes.get_mut(&node) {
            let u = rng
                .checkout(
                    v2xw_core::rng::RngDomain::Backend,
                    v2xw_core::rng::EntityRef::Node(node),
                )
                .f64();
            n.next_crl_fetch = now.saturating_add((u * interval as f64) as u64);
        }
    }

    /// `crl_delivery: mbms` only: the cellular vehicles a broadcast at `now` is for — on a
    /// newly `published` list, or on a carousel turn — each marked as receiving. `None`
    /// under unicast delivery or before the first list.
    pub fn mbms_due(&mut self, now: SimTime, published: bool) -> Option<Vec<NodeId>> {
        if self.access.crl_delivery() != crate::backend::CrlDelivery::Mbms {
            return None;
        }
        let version = self.mbms_crl().0;
        if version == 0 || (!published && now < self.mbms_next) {
            return None;
        }
        self.mbms_next = Duration::from_secs_f64(self.access.mbms().carousel_s).after(now);
        let mut out = Vec::new();
        for (node, n) in &mut self.nodes {
            if n.access == Some(AccessKind::Cellular)
                && !n.crl_fetch_in_flight
                && n.crl_version < version
            {
                n.crl_fetch_in_flight = true;
                out.push(*node);
            }
        }
        Some(out)
    }

    /// A vehicle refused a CRL frame or download (its signature, its signer, or an entry
    /// the signed CRL does not carry).
    pub fn crl_frame_refused(&mut self, node: NodeId, cellular: bool) {
        self.report.crl_frames_rejected += 1;
        if let Some(n) = self.nodes.get_mut(&node)
            && cellular
        {
            n.crl_fetch_in_flight = false;
        }
    }

    /// A CRL poll that ended without a download (lost, no coverage).
    pub fn crl_poll_failed(&mut self, node: NodeId) {
        if let Some(n) = self.nodes.get_mut(&node) {
            n.crl_fetch_in_flight = false;
        }
    }

    /// A vehicle received part or all of CRL `version`: `entries` are the entries the
    /// frame or download carried. Returns the entries new to this vehicle and the
    /// revocation stages its enforcement completes.
    ///
    /// A list may arrive split across several broadcast frames, or by both paths; an
    /// entry is installed once, and the vehicle holds `version` once it holds that many
    /// entries.
    #[allow(clippy::too_many_arguments)]
    pub fn install_crl(
        &mut self,
        node: NodeId,
        version: u32,
        entries: &[CrlLinkageEntry],
        now: SimTime,
        cellular: bool,
        node_verified: bool,
    ) -> (
        Vec<CrlLinkageEntry>,
        Vec<crate::sec_records::ProtoRevocation>,
    ) {
        let records = Vec::new();
        // WP-C2: with the PDU layer on, the vehicle verified the signed CRL itself
        // (`ObuRuntime::install_secured_crl`) before this is called, and refused a frame
        // carrying an entry the CRL does not sign. Only the modelled path (PDU layer off:
        // no signed bytes exist) still compares against the Generator's own list, which
        // is what the modelled digest signature covers (`crl_digest`).
        if !node_verified {
            let signed =
                |e: &CrlLinkageEntry| self.runs.iter().any(|r| r.state.crlg.entries.contains(e));
            if !entries.iter().all(signed) {
                self.crl_frame_refused(node, cellular);
                return (Vec::new(), records);
            }
        }
        let mut records = records;
        let Some(n) = self.nodes.get_mut(&node) else {
            return (Vec::new(), records);
        };
        if cellular {
            n.crl_fetch_in_flight = false;
            self.report.crl_downloads += 1;
        }
        let fresh: Vec<CrlLinkageEntry> = entries
            .iter()
            .filter(|e| !n.crl_installed.contains(e))
            .cloned()
            .collect();
        n.crl_installed.extend(fresh.iter().cloned());
        if n.crl_installed.len() >= version as usize {
            n.crl_version = n.crl_version.max(version);
        }
        let size = u64::from(crl_bytes(&self.home().state.sizes, version).bytes());
        self.report.crls_installed += fresh.len() as u64;
        for case in &mut self.cases {
            let Some((entry, _)) = case.entry.as_ref() else {
                continue;
            };
            if !fresh.contains(entry) {
                continue;
            }
            for stage in ["downloaded", "processed", "enforced"] {
                records.push(crate::sec_records::ProtoRevocation::stage(
                    now,
                    stage,
                    &case.subject_digest,
                    (stage == "downloaded").then_some(size),
                    (stage == "downloaded").then_some(u64::from(version)),
                    Some(node),
                ));
            }
            if case.enforced_first.is_none() {
                case.enforced_first = Some(now);
            }
            case.enforcers.push((node, now));
        }
        self.finish_first_revocation(now);
        (fresh, records)
    }

    /// Whether a report is being forwarded for the first time: two units that both hear
    /// one relayed report forward it once between them, as the RA's duplicate check
    /// would otherwise have to.
    pub fn claim_forward(&mut self, report_id: &str) -> bool {
        self.forwarded.insert(report_id.to_string())
    }

    /// How many CRL entries a vehicle holds.
    #[must_use]
    pub fn crl_entries_of(&self, node: NodeId) -> u32 {
        self.nodes
            .get(&node)
            .map_or(0, |n| n.crl_installed.len() as u32)
    }

    /// Assembles the first revocation's decomposition, once a vehicle enforces it.
    fn finish_first_revocation(&mut self, _now: SimTime) {
        if self.revocation.is_some() {
            return;
        }
        let Some(case) = self.cases.iter().find(|c| c.enforced_first.is_some()) else {
            return;
        };
        let (Some((resolution, issuance)), Some((entry, version)), Some(enforced)) =
            (case.runs, case.entry.clone(), case.enforced_first)
        else {
            return;
        };
        let log = &self.runs[case.provider].kernel.stages;
        let latency = RevocationLatency::assemble(
            log,
            device_of(case.subject),
            Transport::CellularUu,
            case.report_run,
            resolution,
            issuance,
            FlowRun(u32::MAX),
        );
        let mut stages = latency.stages.clone();
        stages.push((StageId::Enforced, enforced));
        let first = stages.first().map_or(enforced, |s| s.1);
        let total = Duration::from_nanos(enforced.saturating_sub(first));
        self.report.revocation_latency_ns = total.as_nanos();
        self.report.revocation_stages = stages
            .iter()
            .map(|(s, t)| (s.as_str().to_string(), t.saturating_sub(first)))
            .collect();
        self.revocation = Some(Revocation {
            entry,
            stages,
            latency: total,
            bytes: crl_bytes(&self.home().state.sizes, version).bytes(),
            subject: case.subject,
            subject_digest: case.subject_digest.clone(),
        });
    }

    /// Whether a CRL entry revokes any of `node`'s own certificates, and which `(i, j)`.
    #[must_use]
    pub fn own_revoked(&self, node: NodeId, gate: &v2xw_node::stores::CrlGate) -> Vec<(u32, u32)> {
        self.creds(node)
            .iter()
            .filter(|k| gate.revokes_linkage_at_period(k.i, k.lv))
            .map(|k| (k.i, k.j))
            .collect()
    }

    /// Marks a vehicle as having found itself on the CRL.
    pub fn note_self_revoked(&mut self, node: NodeId) {
        if let Some(n) = self.nodes.get_mut(&node) {
            n.self_revoked = true;
        }
    }

    /// Vehicles whose pool needs the next period, with the access to use. Marks each as
    /// having a top-up requested; the caller starts the flow with [`Phase2::start_topup`].
    pub fn topups_due(&mut self, now: SimTime) -> Vec<(NodeId, AccessKind, u32)> {
        let current = self.params.period_at(now);
        let below = self.params.topup_below_periods;
        let mut out = Vec::new();
        for (node, n) in &self.nodes {
            if n.topup.is_some() || n.self_revoked || n.blocked || n.etsi_batch.is_some() {
                continue;
            }
            let remaining = n.last_period.saturating_add(1).saturating_sub(current);
            if below > 0 && remaining <= below {
                out.push((
                    *node,
                    n.access.unwrap_or(AccessKind::Offline),
                    n.last_period + 1,
                ));
            }
        }
        out
    }

    /// Starts one vehicle's top-up over `link`, for period `i`.
    pub fn start_topup(&mut self, node: NodeId, link: v2xw_proto::Link, i: u32, now: SimTime) {
        let device = device_of(node);
        if let Some(etsi) = self.etsi.as_mut()
            && self.params.etsi_butterfly
        {
            // One butterfly request for the whole period (TS 102 941 V2 §6.2.3.5): the EA
            // expands the caterpillar keys, the AA certifies the batch without learning
            // whose it is, and the station comes back for it at the next backend step.
            etsi.set_access(device, link);
            etsi.current_i = i;
            let run = etsi.authorize_butterfly(device);
            self.report.topups_started += 1;
            if let Some(n) = self.nodes.get_mut(&node) {
                n.topup = Some(run);
                n.topup_started = now;
                n.etsi_batch = Some((i, false));
            }
            return;
        }
        if let Some(etsi) = self.etsi.as_mut() {
            // One standard authorization per ticket [TS 102 941 §6.2.3.3]; a blocklisted
            // station's requests are refused by the EA and its pool runs dry.
            etsi.set_access(device, link);
            let target = etsi.tickets_of(device) + self.params.jmax;
            let mut run = FlowRun(0);
            for _ in 0..self.params.jmax {
                run = etsi.authorize_at(device, now);
            }
            self.report.topups_started += 1;
            if let Some(n) = self.nodes.get_mut(&node) {
                n.topup = Some(run);
                n.topup_started = now;
                n.etsi_target = Some(target);
            }
            let _ = i;
            return;
        }
        let jmax = self.params.jmax;
        let scms = self.run_of_mut(device);
        scms.set_access(device, link);
        let run = scms.topup_at(device, now, i, jmax);
        self.report.topups_started += 1;
        if let Some(n) = self.nodes.get_mut(&node) {
            n.topup = Some(run);
            n.topup_started = now;
        }
    }

    /// Notes a pseudonym change the engine saw, returning the previous digest.
    pub fn note_change(
        &mut self,
        node: NodeId,
        changes: u32,
        digest: Option<[u8; 8]>,
    ) -> Option<Option<[u8; 8]>> {
        let n = self.nodes.get_mut(&node)?;
        if changes == n.changes_seen && digest == n.active_digest {
            return None;
        }
        let old = n.active_digest;
        let changed = changes > n.changes_seen;
        n.changes_seen = changes;
        n.active_digest = digest;
        if changed {
            self.report.pseudonym_changes += 1;
            Some(old)
        } else {
            None
        }
    }

    /// Counts a node step at `now` spent unable to sign, and the time since the last one.
    pub fn note_starved(&mut self, node: NodeId, now: SimTime) {
        self.report.starved_node_steps += 1;
        let Some(n) = self.nodes.get_mut(&node) else {
            return;
        };
        if !n.starved {
            n.starved = true;
            self.report.vehicles_starved += 1;
        }
        match n.starved_since {
            None => n.starved_since = Some(now),
            Some(since) => {
                self.report.cert_unavailable_ns += now.saturating_sub(n.starved_last);
                self.report.cert_unavailable_max_ns = self
                    .report
                    .cert_unavailable_max_ns
                    .max(now.saturating_sub(since));
            }
        }
        n.starved_last = now;
    }

    /// A node step at `now` at which the vehicle could sign: closes a spell without a
    /// valid pseudonym, counting the time up to this step.
    pub fn note_signable(&mut self, node: NodeId, now: SimTime) {
        let Some(n) = self.nodes.get_mut(&node) else {
            return;
        };
        if let Some(since) = n.starved_since.take() {
            self.report.cert_unavailable_ns += now.saturating_sub(n.starved_last);
            self.report.cert_unavailable_max_ns = self
                .report
                .cert_unavailable_max_ns
                .max(now.saturating_sub(since));
        }
    }

    /// The security panel's row for one vehicle.
    #[must_use]
    pub fn security_view(
        &self,
        node: NodeId,
        t: SimTime,
        certs: &v2xw_node::CertStore,
        crl_entries: u32,
        link_up: bool,
    ) -> Option<crate::sec_records::NodeSecurityView> {
        let n = self.nodes.get(&node)?;
        let active = certs.active();
        let hex = |d: &[u8]| v2xw_core::hash::hex_encode(d);
        let valid = certs
            .credentials()
            .iter()
            .filter(|c| c.is_valid_at(t))
            .count();
        let preloaded = certs
            .credentials()
            .iter()
            .filter(|c| t < c.valid_from)
            .count();
        Some(crate::sec_records::NodeSecurityView {
            t,
            node,
            protocol: if self.etsi.is_some() {
                ETSI_PKI
            } else {
                CAMP_SCMS
            }
            .to_string(),
            pseudonym: active.map(|c| hex(&c.digest.0[..])),
            temp_id: active.map(|c| hex(&c.digest.0[..4])),
            cert_i: active.map(|c| c.i_period),
            cert_j: active.map(|c| c.j_index),
            cert_valid_until: active.map(|c| c.valid_until),
            pool_valid: valid as u32,
            pool_preloaded: preloaded as u32,
            pool_stored: certs.stored_count() as u32,
            pool_last_period: Some(n.last_period),
            changes: certs.changes(),
            topup_in_flight: n.topup.is_some(),
            // Due now or later; a device that is revoked, blocklisted or already topping up
            // asks for nothing.
            next_topup: if n.topup.is_some() || n.self_revoked || n.blocked {
                None
            } else {
                self.params.topup_at(n.last_period)
            },
            link: n.access.unwrap_or(AccessKind::Offline).as_str().to_string(),
            link_up,
            outbox_reports: n.outbox.0.len() as u32,
            reports_uploaded: n.reports_uploaded,
            crl_entries,
            crl_version: n.crl_version,
            self_revoked: n.self_revoked,
        })
    }

    /// Notes that the roadside put a CRL frame on the air.
    pub fn note_crl_broadcast(&mut self) {
        self.report.crl_broadcasts += 1;
    }

    /// Notes a revocation whose publication fell past the run horizon.
    pub fn note_crl_past_horizon(&mut self) {
        self.report.crl_past_horizon += 1;
    }

    /// Notes that a node installed a CRL entry.
    pub fn note_crl_installed(&mut self) {
        self.report.crls_installed += 1;
    }

    /// Notes a reception the installed CRL caused to be classified revoked (`roadside`:
    /// on a roadside unit).
    pub fn note_revoked_reception(&mut self, roadside: bool) {
        self.report.revoked_receptions += 1;
        if roadside {
            self.report.rsu_revoked_receptions += 1;
        }
    }

    /// Counts what receiver `node` verified and refused as revoked, by the provider that
    /// serves it and the one that certified each signer (WP-D1): `"<receiver's
    /// provider><-<signer's provider>"`. Cross-provider trust is visible here, from each
    /// node's own verdicts; the signer's provider is the engine's bookkeeping (which
    /// vehicle holds the certificate), never read by a decision.
    pub fn note_provider_receptions(
        &mut self,
        node: NodeId,
        delivered: &[v2xw_node::VerifiedMessage],
    ) {
        if self.runs.len() < 2 {
            return;
        }
        let Some(&rk) = self.device_provider.get(&device_of(node)) else {
            return;
        };
        let names = &self.runs[0].state.deployment.providers;
        for m in delivered {
            let Some(signer) = &m.signer else { continue };
            let Some(&(sender, _, _)) = self.by_digest.get(&digest_key(signer)) else {
                continue;
            };
            let Some(&sk) = self.device_provider.get(&device_of(sender)) else {
                continue;
            };
            let key = format!("{}<-{}", names[rk].name, names[sk].name);
            let bucket = match m.verification {
                v2xw_node::stores::VerificationState::Verified => {
                    &mut self.report.verified_by_provider
                }
                v2xw_node::stores::VerificationState::Revoked => {
                    &mut self.report.revoked_by_provider
                }
                _ => continue,
            };
            *bucket.entry(key).or_insert(0) += 1;
        }
    }

    /// Settles the end of the run: an issued entry that no store published before the
    /// horizon is counted as cut off by it.
    pub fn finish(&mut self) {
        let issued_unpublished = self
            .cases
            .iter()
            .filter(|c| {
                c.entry
                    .as_ref()
                    .is_some_and(|(_, v)| *v > self.published[c.provider])
            })
            .count() as u64;
        self.report.crl_past_horizon = self.report.crl_past_horizon.max(issued_unpublished);
        self.report.ma_peak_unrevoked_events = u64::from(
            self.ma
                .iter()
                .filter_map(MaHost::peak_unrevoked_events)
                .max()
                .unwrap_or(0),
        );
        self.report.reports_shared = self.runs.iter().map(|r| r.state.shared.forwarded).sum();
        self.report.reports_foreign_unresolvable =
            self.runs.iter().map(|r| r.state.shared.unresolvable).sum();
        for run in &self.runs {
            let name = run.state.deployment.providers[run.state.provider]
                .name
                .clone();
            self.report
                .devices_by_provider
                .insert(name, run.state.devices.len() as u64);
        }
        self.report.privacy_tracked_vehicles = self.privacy_tracked.len() as u64;
        self.report.privacy_followed_across_change = self.privacy_across.len() as u64;
        self.report.privacy_tracked_sum_ns = self.privacy_tracked.values().sum();
        self.report.privacy_tracked_max_ns =
            self.privacy_tracked.values().copied().max().unwrap_or(0);
        self.report.attacker_outcomes = self
            .attackers
            .iter()
            .map(|(node, slot)| {
                let reports = self.attacker_reports.get(node);
                AttackerOutcome {
                    node: node.index(),
                    attack: slot.id.clone(),
                    onset_ns: self.first_falsified.get(node).copied(),
                    last_falsified_ns: self.attacker_lies.get(node).map(|l| l.0),
                    falsified_claims: self.attacker_lies.get(node).map_or(0, |l| l.1),
                    first_reported_ns: self.first_reported.get(node).copied(),
                    reports: reports.map_or(0, |r| r.0),
                    pseudonyms_reported: reports.map_or(0, |r| r.1.len() as u64),
                    reports_at_ma: self.attacker_at_ma.get(node).copied().unwrap_or(0),
                    peak_events: self.attacker_peak_events.get(node).copied().unwrap_or(0),
                    decided_ns: self.decided.get(node).copied(),
                }
            })
            .collect();
    }

    /// The pseudonym policy's totals, for the run report.
    pub fn note_policy_totals(&mut self, silenced: u64, requested: u64) {
        self.report.pseudonym_silenced_frames = silenced;
        self.report.pseudonym_changes_requested = requested;
    }

    /// Feeds one safety frame, as it goes on the air, to the passive observer.
    ///
    /// Without `threats.eavesdropper` the observer is a global eavesdropper: it hears
    /// every frame, which is the worst case for privacy and the upper bound a roadside
    /// receiver network approaches as its density grows; with it, the engine hands over
    /// only what a sniffer hears (`crate::pseudonym_policy`). It reads only what is on the
    /// air — the signer digest, the claim, the claimed confidence and a BSM's `msgCnt` —
    /// and whether a link it makes is right is judged from the ground truth for the run
    /// report only, never by the observer.
    #[allow(clippy::too_many_arguments)]
    pub fn observe_frame(
        &mut self,
        ctx: &mut dyn ThreatCtx,
        signer: [u8; 8],
        at: SimTime,
        generated: SimTime,
        pos: Vec3,
        speed_mps: f64,
        heading_rad: f64,
        confidence_m: f64,
        seq: Option<u8>,
    ) {
        let m = ObservedMessage {
            signer,
            kind: ObservedKind::Beacon,
            received_at: at,
            claimed_generation_time: generated,
            claimed_x_m: pos.x,
            claimed_y_m: pos.y,
            claimed_speed_mps: speed_mps,
            claimed_heading_rad: heading_rad,
            claimed_pos_confidence_m: confidence_m,
            repetitions: 1,
            cert_valid_from: 0,
            cert_valid_to: SimTime::MAX,
            station_type: StationType::Vehicle,
            verification: VerificationState::Valid,
        };
        let me = SelfBelief {
            node: NodeId::new(u32::MAX),
            believed_time: at,
            x_m: 0.0,
            y_m: 0.0,
            radio_range_m: f64::INFINITY,
        };
        self.report.privacy_frames_read += 1;
        let decision = self.observer.on_message_sequenced(ctx, &me, &m, seq);
        // Ground truth for the run report only: how long the eavesdropper's current track
        // of this pseudonym has been following its true vehicle.
        if decision.is_none()
            && let Some((Some(vehicle), since)) = self.privacy_chains.get(&signer).copied()
        {
            let best = self.privacy_tracked.entry(vehicle).or_insert(0);
            *best = (*best).max(at.saturating_sub(since));
        }
        if let Some(o) = decision {
            self.score_link(&o, at);
            self.link_claims
                .push(v2xw_threat::records::PrivacyLinkClaim {
                    t: at,
                    observer: NodeId::new(u32::MAX),
                    predecessor: o.predecessor.clone().unwrap_or_default(),
                    successor: o.successor.clone(),
                    posterior: v2xw_core::math::quantize_to(o.posterior, 1e-6),
                    candidates: o.anonymity_set_size.saturating_sub(1),
                    anonymity_set_size: o.anonymity_set_size,
                    effective_anonymity_set_bits: v2xw_core::math::quantize_to(
                        o.effective_anonymity_set_bits,
                        1e-6,
                    ),
                    degree_of_anonymity: v2xw_core::math::quantize_to(o.degree_of_anonymity, 1e-6),
                    method: if o.predecessor.is_some() {
                        v2xw_threat::privacy::METHOD.to_string()
                    } else {
                        v2xw_threat::privacy::METHOD_UNLINKED.to_string()
                    },
                });
            if o.predecessor.is_some() {
                self.report.privacy_links_claimed += 1;
                let owner = |hex: &str| {
                    decode_hex8(hex).and_then(|d| self.by_digest.get(&d).map(|(n, ..)| *n))
                };
                if let (Some(a), Some(b)) = (
                    o.predecessor.as_deref().and_then(owner),
                    owner(&o.successor),
                ) && a == b
                {
                    self.report.privacy_links_correct += 1;
                }
            }
        }
    }

    /// The ground-truth score of one linkage decision, for the run report only.
    ///
    /// A correct link extends the true vehicle's followed time; a wrong one means the
    /// eavesdropper's track now follows whoever the new pseudonym belongs to, from now.
    fn score_link(&mut self, o: &v2xw_threat::LinkOutcome, at: SimTime) {
        let owner = |hex: &str| decode_hex8(hex).and_then(|d| self.by_digest.get(&d).map(|v| v.0));
        let succ = decode_hex8(&o.successor);
        let succ_owner = owner(&o.successor);
        self.report.privacy_link_decisions += 1;
        self.report.privacy_anonymity_set_sum += u64::from(o.anonymity_set_size);
        self.report.privacy_degree_micro_sum +=
            (o.degree_of_anonymity.clamp(0.0, 1.0) * 1e6).round() as u64;
        let mut start = at;
        if let Some(pred) = o.predecessor.as_deref().and_then(decode_hex8)
            && let Some((pred_owner, since)) = self.privacy_chains.remove(&pred)
            && pred_owner.is_some()
            && pred_owner == succ_owner
        {
            start = since;
            if let Some(vehicle) = succ_owner {
                let followed = at.saturating_sub(since);
                let best = self.privacy_tracked.entry(vehicle).or_insert(0);
                *best = (*best).max(followed);
                self.privacy_across.insert(vehicle);
            }
        }
        if let Some(d) = succ {
            self.privacy_chains.insert(d, (succ_owner, start));
        }
    }

    /// The observer's link claims since the last call.
    pub fn take_link_claims(&mut self) -> Vec<v2xw_threat::records::PrivacyLinkClaim> {
        core::mem::take(&mut self.link_claims)
    }

    /// The credentials provisioned for a node.
    pub fn creds(&self, node: NodeId) -> &[ProvisionedCred] {
        self.creds.get(&node).map_or(&[], Vec::as_slice)
    }
}

/// The legacy attacker's parameters from a scenario's `threats.attackers[].params`.
///
/// Every key is read or refused — none is silently dropped: `intensity`, `dt_s`,
/// `magnitude_scale` (this attacker kind's multiplier), `dos_burst`, `delay_s`,
/// `expired_cert_lag_s`, `not_yet_valid_lead_s`, and each field of
/// [`v2xw_threat::Magnitudes`] by its own name.
pub fn attacker_params(
    kind: AttackKind,
    params: &serde_json::Value,
) -> Result<LegacyAttackerParams> {
    let mut p = LegacyAttackerParams::new(kind);
    if params.is_null() {
        return Ok(p);
    }
    let Some(map) = params.as_object() else {
        return Err(conflict(
            "threats.attackers[].params",
            "must be an object of named numbers".to_string(),
        ));
    };
    for (key, value) in map {
        let v = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| {
            conflict(
                &format!("threats.attackers[].params.{key}"),
                format!("must be a finite number, got {value}"),
            )
        })?;
        let m = &mut p.magnitudes;
        let whole = |v: f64| v.round().max(0.0) as u64;
        match key.as_str() {
            "intensity" => p.intensity = v,
            "dt_s" => p.dt_s = v,
            "magnitude_scale" => {
                p.magnitude_scale.insert(kind, v);
            }
            "dos_burst" => p.dos_burst = whole(v) as u32,
            "delay_s" => p.delay_s = v,
            "expired_cert_lag_s" => p.expired_cert_lag_s = v,
            "not_yet_valid_lead_s" => p.not_yet_valid_lead_s = v,
            "const_pos_offset_m" => m.const_pos_offset_m = v,
            "random_pos_half_range_m" => m.random_pos_half_range_m = v,
            "teleport_dx_m" => m.teleport_dx_m = v,
            "teleport_dy_m" => m.teleport_dy_m = v,
            "teleport_period_s" => m.teleport_period_s = whole(v).max(1),
            "sine_pos_amplitude_m" => m.sine_pos_amplitude_m = v,
            "sine_pos_omega_rad_s" => m.sine_pos_omega_rad_s = v,
            "const_speed_offset_mps" => m.const_speed_offset_mps = v,
            "random_speed_max_mps" => m.random_speed_max_mps = v,
            "stop_and_go_speed_mps" => m.stop_and_go_speed_mps = v,
            "heading_offset_deg" => m.heading_offset_deg = v,
            "replay_lag_samples" => m.replay_lag_samples = whole(v) as usize,
            "slow_drift_ramp_mps" => m.slow_drift_ramp_mps = v,
            "slow_drift_max_rate_mps" => m.slow_drift_max_rate_mps = v,
            "along_road_offset_m" => m.along_road_offset_m = v,
            "disruptive_pos_half_range_m" => m.disruptive_pos_half_range_m = v,
            "disruptive_speed_up_mps" => m.disruptive_speed_up_mps = v,
            "disruptive_speed_down_mps" => m.disruptive_speed_down_mps = v,
            "disruptive_heading_deg" => m.disruptive_heading_deg = v,
            "pos_speed_inconsistent_drop_mps" => m.pos_speed_inconsistent_drop_mps = v,
            "pos_heading_swing_m" => m.pos_heading_swing_m = v,
            "pos_heading_omega_rad_s" => m.pos_heading_omega_rad_s = v,
            "eventual_stop_delay_s" => m.eventual_stop_delay_s = v,
            "eventual_stop_base_speed_mps" => m.eventual_stop_base_speed_mps = v,
            // F2MD `SybilVehNumber` and the legacy pre-pass's `sr.uniform(-1, 1)`: how many
            // concurrent pseudonyms the ghost set uses, and how far each ghost's claim sits
            // from the attacker's own. Refused on any other kind, where nothing reads them.
            "sybil_ghosts" | "sybil_jitter_m" if kind != AttackKind::Sybil => {
                return Err(conflict(
                    &format!("threats.attackers[].params.{key}"),
                    "is read only by threat/attacker/legacy/Sybil".to_string(),
                ));
            }
            "sybil_ghosts" => p.sybil_ghosts = (whole(v) as u32).clamp(1, 59),
            "sybil_jitter_m" => p.sybil_jitter_m = v.clamp(0.0, 50.0),
            other => {
                return Err(conflict(
                    &format!("threats.attackers[].params.{other}"),
                    "is not a legacy attacker parameter this build reads".to_string(),
                ));
            }
        }
    }
    Ok(p)
}

/// `net.backend_net`: the links between backend entities.
///
/// Also the loader's check of the key (`crate::scenario::validate` runs it on a default
/// parameter set), so a wrong id or parameter is refused at Check, in the same words, and
/// not first discovered when the run is built.
/// Whether each device's chain file holds only its own provider's CA certificates
/// (`security.scms.chain_file: home-provider`) rather than every CTL provider's
/// (`all-providers`, the default).
///
/// # Errors
/// [`EngineError::Scenario`] for any other value.
pub fn home_chain_file_of(scenario: &Scenario) -> Result<bool> {
    match scenario
        .security
        .scms
        .as_ref()
        .and_then(|s| s.chain_file.as_deref())
    {
        None | Some("all-providers") => Ok(false),
        Some("home-provider") => Ok(true),
        Some(other) => Err(conflict(
            "security.scms.chain_file",
            format!("'{other}' is not a chain file scope; allowed: all-providers, home-provider"),
        )),
    }
}

/// Whether the scenario publishes the PKI view (`security.scms.pki_view`, WP-E1):
/// on unless set to `false`.
#[must_use]
pub fn pki_view_of(scenario: &Scenario) -> bool {
    scenario
        .security
        .scms
        .as_ref()
        .and_then(|s| s.pki_view)
        .unwrap_or(true)
}

/// The SCMS deployment a scenario runs on (`security.scms.deployment`, WP-D1).
///
/// An SCMS run (`protocol/scms/camp`) defaults to `us-production-2025`, the US system;
/// any other run keeps the single-SCMS `camp-reference` topology its backend has always
/// had. An explicit `net.backend_net.params.latency_ms` is a flat link latency, so it
/// selects the `flat` latency model unless the deployment names one.
///
/// # Errors
/// [`EngineError::Scenario`] for an unknown preset, provider, role, region, latency model
/// or butterfly, a refused butterfly (`unified`, `cubk`), a `custom` preset without
/// providers, or a deployment that does not validate.
pub fn deployment_of(
    scenario: &Scenario,
    scms_run: bool,
) -> Result<v2xw_proto::scms::deployment::Deployment> {
    use v2xw_proto::scms::deployment::{Deployment, LatencyModel, Preset, Provider, Region};
    let spec = scenario
        .security
        .scms
        .as_ref()
        .and_then(|s| s.deployment.as_ref());
    let field = "security.scms.deployment";
    let preset = match spec {
        Some(s) => Preset::parse(&s.preset).ok_or_else(|| {
            conflict(
                &format!("{field}.preset"),
                format!(
                    "'{}' is not a preset; allowed: {}",
                    s.preset,
                    Preset::IDS.join(", ")
                ),
            )
        })?,
        None if scms_run => Preset::UsProduction2025,
        None => Preset::CampReference,
    };
    let mut d = Deployment::preset(preset);
    let flat_override = scenario
        .net
        .backend_net
        .as_ref()
        .and_then(|c| c.params.get("latency_ms"))
        .is_some();
    if flat_override {
        d.latency = LatencyModel::Flat;
    }
    let Some(s) = spec else {
        return Ok(d);
    };
    if preset == Preset::Custom {
        if s.providers.is_empty() {
            return Err(conflict(
                &format!("{field}.providers"),
                "is empty; a custom deployment lists its providers".to_string(),
            ));
        }
        d.providers.clear();
    }
    for p in &s.providers {
        let at = format!("{field}.providers.{}", p.name);
        let idx = match d.providers.iter().position(|q| q.name == p.name) {
            Some(i) => i,
            None if preset == Preset::Custom => {
                let mut q = Deployment::preset(Preset::UsProduction2025).providers[0].clone();
                q.name.clone_from(&p.name);
                q.fleet_share = 0.0;
                d.providers.push(q);
                d.providers.len() - 1
            }
            None => {
                return Err(conflict(
                    &at,
                    format!(
                        "is not a provider of {}; it has {}. Use preset custom to name others",
                        preset.id(),
                        d.providers
                            .iter()
                            .map(|q| q.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
        };
        let q: &mut Provider = &mut d.providers[idx];
        for (role, n) in &p.components {
            q.components.insert(role.clone(), *n);
        }
        if let Some(r) = &p.region {
            q.region = Region::assumed(r).ok_or_else(|| {
                conflict(
                    &format!("{at}.region"),
                    format!(
                        "'{r}' is not a region; allowed: {}",
                        v2xw_proto::scms::deployment::REGIONS
                            .iter()
                            .map(|x| x.name)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                )
            })?;
        }
        if let Some(f) = p.fleet_share {
            q.fleet_share = f;
        }
        if let Some(on) = p.on_ctl {
            q.on_ctl = on;
        }
        if let Some(r) = p.ma_review_s
            && !(r.is_finite() && r > 0.0)
        {
            return Err(conflict(
                &format!("{at}.ma_review_s"),
                format!("{r} must be > 0 (use ma_mode: automatic for no review)"),
            ));
        }
        match p.ma_mode.as_deref() {
            None if p.ma_review_s.is_some() => {
                return Err(conflict(
                    &format!("{at}.ma_review_s"),
                    "is set without ma_mode: manual-review".to_string(),
                ));
            }
            None => {}
            Some("automatic") if p.ma_review_s.is_some() => {
                return Err(conflict(
                    &format!("{at}.ma_review_s"),
                    "is set with ma_mode: automatic, which has no review".to_string(),
                ));
            }
            Some("automatic") => q.ma_mode = v2xw_proto::scms::params::MaMode::Automatic,
            Some("manual-review") => {
                let review = p.ma_review_s.unwrap_or(8.0 * 3_600.0);
                q.ma_mode = v2xw_proto::scms::params::MaMode::ManualReview {
                    review: Duration::from_nanos((review * 1e9).round() as u64),
                };
            }
            Some(other) => {
                return Err(conflict(
                    &format!("{at}.ma_mode"),
                    format!("'{other}' is not an MA mode; allowed: manual-review, automatic"),
                ));
            }
        }
        if let Some(c) = &p.crl_publication {
            q.crl_publication =
                v2xw_proto::scms::deployment::CrlPublication::parse(c).ok_or_else(|| {
                    conflict(
                        &format!("{at}.crl_publication"),
                        format!(
                            "'{c}' is not a CRL publication rule; allowed: {}",
                            v2xw_proto::scms::deployment::CrlPublication::IDS.join(", ")
                        ),
                    )
                })?;
        }
    }
    if let Some(n) = s.electors {
        d.electors = n;
    }
    if let Some(n) = s.quorum {
        d.quorum = n;
    }
    if let Some(o) = &s.options {
        if let Some(lop) = o.lop {
            d.options.lop = lop;
        }
        if let Some(b) = &o.butterfly {
            d.options.butterfly = Deployment::butterfly(b)
                .map_err(|e| conflict(&format!("{field}.options.butterfly"), e.to_string()))?;
        }
    }
    let (fibre, per_link) = (s.fibre_fraction, s.per_link_ms);
    match s.latency.as_deref() {
        None if fibre.is_none() && per_link.is_none() => {}
        None | Some("region") => {
            let (f0, o0) = match LatencyModel::DEFAULT_REGION {
                LatencyModel::Region {
                    fibre_fraction,
                    per_link_overhead,
                } => (fibre_fraction, per_link_overhead),
                LatencyModel::Flat => unreachable!(),
            };
            let f = fibre.unwrap_or(f0);
            if !(f > 0.0 && f <= 1.0) {
                return Err(conflict(
                    &format!("{field}.fibre_fraction"),
                    format!("{f} must be in (0, 1]"),
                ));
            }
            let o = match per_link {
                Some(ms) if ms.is_finite() && ms >= 0.0 => {
                    Duration::from_nanos((ms * 1e6).round() as u64)
                }
                Some(ms) => {
                    return Err(conflict(
                        &format!("{field}.per_link_ms"),
                        format!("{ms} must be ≥ 0"),
                    ));
                }
                None => o0,
            };
            d.latency = LatencyModel::Region {
                fibre_fraction: f,
                per_link_overhead: o,
            };
        }
        Some("flat") if fibre.is_none() && per_link.is_none() => d.latency = LatencyModel::Flat,
        Some("flat") => {
            return Err(conflict(
                &format!("{field}.latency"),
                "is flat, which has no fibre_fraction or per_link_ms; remove them or choose region"
                    .to_string(),
            ));
        }
        Some(other) => {
            return Err(conflict(
                &format!("{field}.latency"),
                format!("'{other}' is not a latency model; allowed: region, flat"),
            ));
        }
    }
    if let Some(on) = s.ma_report_sharing {
        if d.providers.len() == 1 {
            return Err(conflict(
                &format!("{field}.ma_report_sharing"),
                "has nothing to share with: the deployment has one provider".to_string(),
            ));
        }
        d.ma_report_sharing = on;
    }
    d.validate().map_err(|e| conflict(field, e.to_string()))?;
    Ok(d)
}

/// What the run manifest says about the SCMS deployment (WP-D1): its preset and providers,
/// every labelled assumption it rests on, and what this build does not model yet. Empty
/// for a run that is not an SCMS run or whose deployment does not build (the loader
/// refuses that one).
#[must_use]
pub fn deployment_warnings(scenario: &Scenario) -> Vec<String> {
    let protocol = scenario
        .actors
        .backend
        .protocol
        .clone()
        .or_else(|| scenario.security.protocol.as_ref().map(|c| c.id.clone()));
    if protocol.as_deref() != Some(CAMP_SCMS) {
        return Vec::new();
    }
    let Ok(d) = deployment_of(scenario, true) else {
        return Vec::new();
    };
    let mut out = vec![format!(
        "SCMS deployment {}: {} ({}); {} electors, quorum {}{}",
        d.preset.id(),
        d.providers
            .iter()
            .map(|p| format!(
                "{}{} share {:.3}",
                p.name,
                if p.on_ctl { "" } else { " (NOT on the CTL)" },
                p.fleet_share
            ))
            .collect::<Vec<_>>()
            .join(", "),
        if d.options.lop { "LOP on" } else { "no LOP" },
        d.electors,
        d.quorum,
        if d.is_us() { "" } else { "; NOT the US system" }
    )];
    out.extend(d.assumptions());
    if d.providers.len() > 1 {
        out.push(format!(
            "SCMS deployment: one backend per provider; each vehicle and roadside unit is \
             served by one provider by the seeded fleet split, an MA {} reports about \
             another provider's certificate with that provider's MA, and every RA serves \
             every provider's CRL after the inter-provider link. The MA-to-MA report \
             format is modelled (no published specification); the governance decisions \
             (root rollover, removal) are taken on {}'s view and its CTL distributed to \
             every provider",
            if d.ma_report_sharing {
                "shares"
            } else {
                "does not share (ma_report_sharing off)"
            },
            d.providers[0].name
        ));
    }
    out
}

pub(crate) fn apply_backend_net(scenario: &Scenario, p: &mut ScmsParams) -> Result<()> {
    let Some(choice) = &scenario.net.backend_net else {
        return Ok(());
    };
    if !crate::backend::BACKEND_NET_MODELS.contains(&choice.id.as_str()) {
        return Err(conflict(
            "net.backend_net",
            format!(
                "'{}' is not a backend network model; this build ships {}. Choose it, or \
                 remove net.backend_net to keep the deployment's default links",
                choice.id,
                crate::backend::BACKEND_NET_MODELS.join(", ")
            ),
        ));
    }
    let map = match &choice.params {
        serde_json::Value::Null => return Ok(()),
        serde_json::Value::Object(map) => map,
        other => {
            return Err(conflict(
                "net.backend_net.params",
                format!("is {other}; it must be an object, e.g. {{latency_ms: 10}}"),
            ));
        }
    };
    for (key, value) in map {
        let field = format!("net.backend_net.params.{key}");
        let v = value.as_f64().filter(|v| v.is_finite());
        match key.as_str() {
            "latency_ms" => {
                let ms = v.filter(|v| *v >= 0.0).ok_or_else(|| {
                    conflict(
                        &field,
                        format!("is {value}; it must be milliseconds, 0 or more"),
                    )
                })?;
                p.backend_link_latency = Duration::from_nanos((ms * 1e6).round() as u64);
            }
            "capacity_mbps" => {
                let mbps = v.filter(|v| *v > 0.0).ok_or_else(|| {
                    conflict(
                        &field,
                        format!("is {value}; it must be a capacity above 0 Mbit/s"),
                    )
                })?;
                p.backend_link_bandwidth_bps = (mbps * 1e6).round().max(1.0) as u64;
            }
            other => {
                return Err(conflict(
                    &field,
                    format!(
                        "'{other}' is not a parameter of backend-net/fixed; it takes \
                         latency_ms and capacity_mbps. Remove the key or correct its spelling"
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// The SCMS roles a scenario may name in `actors.backend.entities` and `.links`.
pub const SCMS_ENTITIES: [&str; 11] = [
    "ra",
    "pca",
    "la1",
    "la2",
    "ma",
    "crlg",
    "lop",
    "crl_store",
    "crl_broadcast",
    "eca",
    "dcm",
];

/// The backend node an SCMS role name is hosted on.
fn scms_node(n: &v2xw_proto::ScmsNodes, name: &str) -> Option<NodeId> {
    Some(match name {
        "ra" => n.ra,
        "pca" => n.pca,
        "la1" => n.la1,
        "la2" => n.la2,
        "ma" => n.ma,
        "crlg" => n.crlg,
        "lop" => n.lop,
        "crl_store" => n.crl_store,
        "crl_broadcast" => n.crl_broadcast,
        "eca" => n.eca,
        "dcm" => n.dcm,
        _ => return None,
    })
}

/// `actors.backend.entities` and `actors.backend.links`: per-entity hardware profile and
/// service model, and per-link latency and capacity overrides.
fn apply_backend_topology(scenario: &Scenario, scms: &mut ScmsRun) -> Result<()> {
    let nodes = scms.state.nodes;
    let p = scms.state.params;
    // `nodes.backend_tier: abstract`: no backend entity ever queues. Sixty-four servers
    // and no fixed overhead leaves only the cryptography's own service time and the links,
    // which is the abstract tier's definition (06-node-models.md §4). The RA keeps its
    // shuffle: that is the protocol's batching, not a queue.
    if scenario.nodes.backend_tier == v2xw_core::card::Tier::Abstract {
        let free = v2xw_proto::ServiceModelSpec::new(64, Duration::ZERO);
        for node in nodes.all() {
            let spec = if node == nodes.ra {
                free.batched(v2xw_proto::BatchPolicy::CAMP_SHUFFLE)
            } else {
                free
            };
            scms.kernel.host(node, &spec, p.backend_profile);
        }
    }
    for (name, entity) in &scenario.actors.backend.entities {
        let node = scms_node(&nodes, name).ok_or_else(|| {
            conflict(
                "actors.backend.entities",
                format!(
                    "'{name}' is not a CAMP SCMS role; allowed: {}",
                    SCMS_ENTITIES.join(", ")
                ),
            )
        })?;
        let profile = match entity.profile.as_deref() {
            None => p.backend_profile,
            Some(id) => backend_profile(id).ok_or_else(|| {
                conflict(
                    &format!("actors.backend.entities.{name}.profile"),
                    format!("'{id}' is not a hardware profile the security crate prices"),
                )
            })?,
        };
        let servers = match entity.service_model.as_deref() {
            None | Some("service/mmc") => p.backend_servers,
            Some("service/mm1") => 1,
            Some(other) => {
                return Err(conflict(
                    &format!("actors.backend.entities.{name}.service_model"),
                    format!("'{other}' is not a service model; allowed: service/mmc, service/mm1"),
                ));
            }
        };
        let mut spec = v2xw_proto::ServiceModelSpec::new(servers, p.backend_overhead);
        if name == "ra" {
            spec = spec.batched(v2xw_proto::BatchPolicy::CAMP_SHUFFLE);
        }
        scms.kernel.host(node, &spec, profile);
    }
    for l in &scenario.actors.backend.links {
        let (Some(a), Some(b)) = (scms_node(&nodes, &l.from), scms_node(&nodes, &l.to)) else {
            return Err(conflict(
                "actors.backend.links",
                format!(
                    "'{}' → '{}': both ends must be CAMP SCMS roles ({})",
                    l.from,
                    l.to,
                    SCMS_ENTITIES.join(", ")
                ),
            ));
        };
        let base = scms.kernel.net().link(a, b).unwrap_or(v2xw_proto::Link {
            latency: p.backend_link_latency,
            bandwidth_bps: p.backend_link_bandwidth_bps,
            transport: Transport::BackendNet,
        });
        let link = v2xw_proto::Link {
            latency: l.latency_ms.map_or(base.latency, |ms| {
                Duration::from_nanos((ms * 1e6).round() as u64)
            }),
            bandwidth_bps: l
                .capacity_mbps
                .map_or(base.bandwidth_bps, |m| (m * 1e6).round().max(1.0) as u64),
            transport: Transport::BackendNet,
        };
        scms.kernel.net_mut().connect(a, b, link);
    }
    Ok(())
}

/// A backend hardware profile by id.
fn backend_profile(id: &str) -> Option<&'static str> {
    use v2xw_sec::primitive::profiles as p;
    [
        p::I9_11950H_WOLFSSL,
        p::COHDA_MK6_BOTAN,
        p::CORTEX_M4_NRF52840,
        p::CORTEX_M7_STM32F767,
    ]
    .into_iter()
    .find(|p| *p == id)
}

/// The backend's wire steps, in their byte buckets. The sidelink's own bytes are not here:
/// a relayed report's air hop is a frame on `node.tx` like any other.
fn push_bytes(tick: &mut BackendTick, steps: Vec<v2xw_proto::stage::WireStep>) {
    use v2xw_metrics::channels::ByteBucket;
    for s in steps {
        let from_device = s.from.index() >= SCMS_DEVICE_BASE;
        let node = [s.from, s.to]
            .into_iter()
            .find(|n| n.index() >= SCMS_DEVICE_BASE)
            .map(|n| NodeId::new(n.index() - SCMS_DEVICE_BASE));
        let bucket = match s.transport {
            Transport::BackendNet => ByteBucket::Backend,
            Transport::RsuBackhaul => ByteBucket::Backhaul,
            Transport::CellularUu if from_device => ByteBucket::CellularUl,
            Transport::CellularUu => ByteBucket::CellularDl,
            _ => continue,
        };
        tick.bytes.push((s.t, bucket, u64::from(s.bytes), node));
    }
}

/// The eight bytes of a certificate digest, as this module keys its maps by.
fn digest_key(digest: &v2xw_msg::sec_types::HashedId8) -> [u8; 8] {
    let mut key = [0u8; 8];
    key.copy_from_slice(&digest.0[..]);
    key
}

/// Eight bytes from a sixteen-character hex string.
/// Empties a report store at `now_unix` in its upload order ([SM-MBR] §2.2): first the
/// 05:00-Eastern decays since the last look and the deletion of week-old reports, then
/// highest weight first, newer first on ties.
fn drain_upload_order<T>(q: &mut MbrQueue<T>, now_unix: i64) -> Vec<T> {
    q.decay_to(now_unix);
    q.expire(now_unix);
    let mut out = Vec::with_capacity(q.len());
    while let Some(seq) = q.next_upload().map(|e| e.seq) {
        if let Some(item) = q.confirm(seq) {
            out.push(item);
        }
    }
    out
}

#[cfg(test)]
mod report_store_tests {
    use super::*;

    /// 2026-10-15 12:00 UTC (08:00 EDT), a Thursday.
    const T: i64 = 1_792_065_600;
    const DAY: i64 = 86_400;

    /// The device store the engine holds reports in decays at 05:00 Eastern and drains
    /// highest weight first. Fault: drain in insertion order (FIFO) and the decayed
    /// older report comes out first.
    #[test]
    fn the_store_drains_by_decayed_weight_and_drops_week_old_reports() {
        assert_eq!(ReportStore::default().0.quota, REPORT_STORE_QUOTA_BYTES);
        let mut q: MbrQueue<&str> = MbrQueue::new(REPORT_STORE_QUOTA_BYTES);
        let w = mbr::Weighting::default();
        // Report 1 at T; it then crosses one 05:00 ET (Friday) before report 2.
        q.push("old", 100, w, T);
        q.push("new", 100, w, T + DAY);
        let weights: Vec<f64> = {
            let mut v: Vec<_> = q.iter().map(|e| e.weight).collect();
            v.sort_by(f64::total_cmp);
            v
        };
        assert!((weights[0] - 0.9).abs() < 1e-12, "{weights:?}");
        assert_eq!(drain_upload_order(&mut q, T + DAY), vec!["new", "old"]);
        // Equal weights: newer first.
        q.push("a", 100, w, T + 2 * DAY);
        q.push("b", 100, w, T + 2 * DAY);
        assert_eq!(drain_upload_order(&mut q, T + 2 * DAY), vec!["b", "a"]);
        // An unsent report older than a week is deleted, not uploaded.
        q.push("stale", 100, w, T + 3 * DAY);
        assert!(drain_upload_order(&mut q, T + 11 * DAY).is_empty());
        assert_eq!(q.expired, 1);
        // Over the quota the lowest weight goes.
        let mut small: MbrQueue<&str> = MbrQueue::new(250);
        small.push("x", 100, w, T);
        small.push("y", 100, w, T + DAY);
        let evicted = small.push("z", 100, w, T + DAY);
        assert_eq!(evicted, vec!["x"]);
    }
}

/// What [`Phase2::real_mbr`] made of a report.
enum MbrBuild {
    /// A real report.
    Real(mbr::MisbehaviorReport),
    /// No PSID-0x20 subtype for the detection.
    Unmapped,
    /// No evidence, certificate or key to build it from.
    Unsecured(&'static str),
    /// Not an SCMS run with the PDU layer.
    Off,
}

/// The [SM-MBR] v1.1 PSID-0x20 subtype for a detector check, with the §2.3.1 default
/// parameters; `None` when the object set has none (see [`Phase2::real_mbr`]).
fn us_subtype(check: &str) -> Option<mbr::Subtype> {
    use v2xw_threat::detect::DetectorId as D;
    match check {
        c if c == D::ConstantPositionFrozen.as_str() => Some(mbr::Subtype::ConstantPosition),
        c if c == D::PositionJump.as_str() => Some(mbr::Subtype::RandomPosition {
            gps_drift_mm: 1_000,
        }),
        c if c == D::ImplausibleAcceleration.as_str() => Some(mbr::Subtype::MaxAcceleration {
            threshold_cmps2: 1_000,
        }),
        _ => None,
    }
}

fn decode_hex8(s: &str) -> Option<[u8; 8]> {
    if s.len() < 16 {
        return None;
    }
    let mut out = [0u8; 8];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

fn conflict(field: &str, message: impl Into<String>) -> EngineError {
    EngineError::Scenario(crate::ScenarioError::conflict(field, message.into()))
}

/// The eight bytes of a certificate digest, for a caller outside this module.
#[must_use]
pub fn digest_bytes(digest: &v2xw_msg::sec_types::HashedId8) -> [u8; 8] {
    digest_key(digest)
}

/// How many bytes a misbehaviour report takes on the wire.
///
/// The size charged where the engine carries a report it did not build as real bytes (an
/// unmapped reason, a roadside unit's forward, the ETSI path): the upload of the reference
/// US report, 1,119 B, the real encoder's signed-and-encrypted report plus its
/// `Download-Req` header ([`v2xw_proto::scms::mbr::REFERENCE_UPLOAD_BYTES`], derived and
/// pinned by `v2xw-proto/tests/mbr.rs`; constants.md). A real report is charged its own
/// length.
#[must_use]
pub const fn report_bytes() -> u32 {
    v2xw_proto::scms::mbr::REFERENCE_UPLOAD_BYTES
}

/// `threats.compromised_rsus` and `threats.compromised_rsu_attack`: which roadside units
/// (by their index in `actors.rsus`) are under an attacker's control, and what they do.
fn compromised_rsu_specs(
    scenario: &Scenario,
) -> Result<BTreeMap<usize, (v2xw_threat::RsuAttackParams, SimTime, SimTime)>> {
    let mut out = BTreeMap::new();
    let units = &scenario.threats.compromised_rsus;
    if units.is_empty() {
        if scenario.threats.compromised_rsu_attack.is_some() {
            return Err(conflict(
                "threats.compromised_rsu_attack",
                "is set but threats.compromised_rsus names no roadside unit",
            ));
        }
        return Ok(out);
    }
    let mut params = v2xw_threat::RsuAttackParams::default();
    let horizon = (scenario.time.duration_s * 1e9).round().max(0.0) as u64;
    let (mut from, mut to) = (0, horizon);
    if let Some(choice) = &scenario.threats.compromised_rsu_attack {
        if choice.id != v2xw_threat::attack_rsu::MODEL_ID {
            return Err(conflict(
                "threats.compromised_rsu_attack.id",
                format!(
                    "{} is not the compromised-unit model; use {}",
                    choice.id,
                    v2xw_threat::attack_rsu::MODEL_ID
                ),
            ));
        }
        if let Some(map) = choice.params.as_object() {
            for (key, value) in map {
                let bad = || {
                    conflict(
                        &format!("threats.compromised_rsu_attack.params.{key}"),
                        format!("has an unusable value {value}"),
                    )
                };
                let prob = |v: &serde_json::Value| {
                    v.as_f64()
                        .filter(|p| (0.0..=1.0).contains(p))
                        .ok_or_else(bad)
                };
                match key.as_str() {
                    "kind" => {
                        let name = value.as_str().ok_or_else(bad)?;
                        let kind = v2xw_threat::RsuAttackKind::parse(name).ok_or_else(|| {
                            conflict(
                                "threats.compromised_rsu_attack.params.kind",
                                format!(
                                    "'{name}' is not a compromised-unit attack; \
                                     SuppressForwardedReports, PoisonForwardedReports or \
                                     FalseCrl"
                                ),
                            )
                        })?;
                        if matches!(
                            kind,
                            v2xw_threat::RsuAttackKind::FalseSpat
                                | v2xw_threat::RsuAttackKind::FalseMap
                                | v2xw_threat::RsuAttackKind::FalseCtl
                        ) {
                            return Err(conflict(
                                "threats.compromised_rsu_attack.params.kind",
                                format!(
                                    "'{name}' is declared by the model but not driven: no \
                                     receiver application acts on SPaT or MAP content yet, \
                                     and roadside units do not broadcast the trust list in \
                                     this build; SuppressForwardedReports, \
                                     PoisonForwardedReports or FalseCrl"
                                ),
                            ));
                        }
                        params.kind = kind;
                    }
                    "suppress_prob" => params.suppress_prob = prob(value)?,
                    "poison_prob" => params.poison_prob = prob(value)?,
                    "fabricated_entries" => {
                        params.fabricated_entries =
                            value.as_f64().filter(|v| *v >= 1.0).ok_or_else(bad)?;
                    }
                    "from_s" => {
                        from = (value.as_f64().filter(|v| *v >= 0.0).ok_or_else(bad)? * 1e9).round()
                            as u64;
                    }
                    "to_s" => {
                        to = (value.as_f64().filter(|v| *v >= 0.0).ok_or_else(bad)? * 1e9).round()
                            as u64;
                    }
                    other => {
                        return Err(conflict(
                            "threats.compromised_rsu_attack.params",
                            format!(
                                "'{other}' is not a parameter; kind, suppress_prob, \
                                 poison_prob, fabricated_entries, from_s, to_s"
                            ),
                        ));
                    }
                }
            }
        }
    }
    for &index in units {
        let index = index as usize;
        if index >= scenario.actors.rsus.len() {
            return Err(conflict(
                "threats.compromised_rsus",
                format!(
                    "names roadside unit {index}, and actors.rsus declares {}; a unit is \
                     named by its position in that list, from 0",
                    scenario.actors.rsus.len()
                ),
            ));
        }
        out.insert(index, (params.clone(), from, to));
    }
    Ok(out)
}
