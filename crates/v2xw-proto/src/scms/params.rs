//! The CAMP SCMS deployment's nodes and its tunable numbers.
//!
//! Every field of [`ScmsParams`] appears on the plug-in's model card. The ones with a
//! standard behind them cite the clause; the ones without carry a calibration plan, and
//! `tests/model_card.rs` checks that no parameter is in the second group without one.

use v2xw_core::ids::NodeId;
use v2xw_core::time::{Duration, SimTime};
use v2xw_sec::primitive::profiles;

use crate::scms::msg::LaIndex;
use crate::service::{BatchPolicy, ServiceModelSpec};
use crate::sizes::SizeParams;

/// Which node hosts which role.
///
/// Fixed ids rather than a map, because the topology of the reference deployment is fixed
/// and a scenario that wants another one builds the links itself. Root CA, the Policy
/// Generator and the electors are offline in the CAMP PoC (05-protocols §3.1) and have no
/// node at all — modelling them as nodes would put messages on the wire that the PoC never
/// sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScmsNodes {
    /// Registration Authority.
    pub ra: NodeId,
    /// Pseudonym Certificate Authority (the ACA of 1609.2.1).
    pub pca: NodeId,
    /// Linkage Authority 1.
    pub la1: NodeId,
    /// Linkage Authority 2.
    pub la2: NodeId,
    /// Misbehaviour Authority.
    pub ma: NodeId,
    /// CRL Generator.
    pub crlg: NodeId,
    /// Location Obscurer Proxy.
    pub lop: NodeId,
    /// CRL Store.
    pub crl_store: NodeId,
    /// The CRL broadcast path (roadside or satellite).
    pub crl_broadcast: NodeId,
    /// Enrolment Certificate Authority.
    pub eca: NodeId,
    /// Device Configuration Manager.
    pub dcm: NodeId,
}

impl Default for ScmsNodes {
    fn default() -> ScmsNodes {
        ScmsNodes {
            ra: NodeId::new(1),
            pca: NodeId::new(2),
            la1: NodeId::new(3),
            la2: NodeId::new(4),
            ma: NodeId::new(5),
            crlg: NodeId::new(6),
            lop: NodeId::new(7),
            crl_store: NodeId::new(8),
            crl_broadcast: NodeId::new(9),
            eca: NodeId::new(10),
            dcm: NodeId::new(11),
        }
    }
}

impl ScmsNodes {
    /// The node hosting one of the two Linkage Authorities.
    pub const fn la(&self, which: LaIndex) -> NodeId {
        match which {
            LaIndex::One => self.la1,
            LaIndex::Two => self.la2,
        }
    }

    /// Every backend node, in a fixed order.
    pub const fn all(&self) -> [NodeId; 11] {
        [
            self.ra,
            self.pca,
            self.la1,
            self.la2,
            self.ma,
            self.crlg,
            self.lop,
            self.crl_store,
            self.crl_broadcast,
            self.eca,
            self.dcm,
        ]
    }

    /// The links the reference deployment needs, as unordered pairs.
    pub fn backend_links(&self) -> Vec<(NodeId, NodeId)> {
        vec![
            (self.lop, self.ra),
            (self.ra, self.la1),
            (self.ra, self.la2),
            (self.ra, self.pca),
            (self.ra, self.ma),
            (self.ma, self.pca),
            (self.ma, self.la1),
            (self.ma, self.la2),
            (self.ma, self.crlg),
            (self.crlg, self.crl_store),
            (self.crlg, self.crl_broadcast),
            (self.dcm, self.eca),
            (self.eca, self.ra),
            // The EE operator registers each enrolment certificate with the RA ([SMP]
            // §2.1.8); the DCM does it at bootstrap (WP-B2).
            (self.dcm, self.ra),
        ]
    }

    /// The service model each backend node runs.
    pub fn backend_service_models(&self, p: &ScmsParams) -> Vec<(NodeId, ServiceModelSpec)> {
        let plain = ServiceModelSpec::new(p.backend_servers, p.backend_overhead);
        let shuffling = plain.batched(BatchPolicy::CAMP_SHUFFLE);
        vec![
            (self.ra, shuffling),
            (self.pca, plain),
            (self.la1, plain),
            (self.la2, plain),
            (self.ma, plain),
            (self.crlg, plain),
            (self.lop, plain),
            (self.crl_store, plain),
            (self.crl_broadcast, plain),
            (self.eca, plain),
            (self.dcm, plain),
        ]
    }
}

/// The CAMP SCMS plug-in's parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScmsParams {
    /// The master seed of the deterministic streams.
    pub master_seed: u64,
    /// The [`SimTime`] at which i-period 0 begins.
    ///
    /// A certificate's validity window is `[epoch + i·i_period, + cert_lifetime)`, so this
    /// is what aligns the protocol's week numbering with the scenario's `time.t0`. It is a
    /// supplied instant, never a clock read.
    pub epoch: SimTime,
    /// The i-period: 10,080 minutes [CAMP-EE §2.1.5.3.2].
    pub i_period: Duration,
    /// Certificate lifetime: 10,140 minutes, one hour of overlap [CAMP-EE §2.1.5.3.2].
    pub cert_lifetime: Duration,
    /// Certificates per i-period: 20 [CAMP-EE Table 2.1.2.6.2].
    pub certs_per_period: u32,
    /// The initial batch: 3,120 certificates, three years [PRIMER p.7].
    pub initial_batch: u32,
    /// How many i-periods ahead the RA provisions a device at most: 156, the three years
    /// of the initial batch [PRIMER p.7: 3,120 = 156 × 20]. The RA clips a request for
    /// more.
    pub max_periods_ahead: u32,
    /// How long an enrolment certificate is valid: **6 years** (365-day years), CAMP SCMS
    /// Release 1.2.2 Table 3 / SCMS-1307 ([CAMP122]; 03 §5), the owner's default. CAMP
    /// Release 1.1 allowed 7 years and the 5GAA draft CP 3 ([5GAA]); both are settings. A
    /// run compresses it to see re-enrolment.
    pub enrolment_lifetime: Duration,
    /// How long before its enrolment certificate expires a device requests the successor
    /// through the RA (IEEE 1609.2.1 `eeRaSuccessorEnrollmentCertRequest`): **18 months**
    /// (548 days), the middle of the 12–24 months SCMS Manager's re-enrolment standard
    /// recommends ([SM-REENR] §1–3; 03 §5, gap row 17). The successor starts when the
    /// current certificate ends, so an early request costs no validity.
    pub reenrol_lead: Duration,
    /// How long after a successor request the RA tells the device to come back for the
    /// certificate (`nextDlTime` in `RaEeEnrollmentCertAck`): 5 minutes, the profile's
    /// `download-minWait` ([SMP] Table 3; 03 §6.2). Re-enrolment is two-phase, request
    /// then download ([SM-REENR] §1–3).
    pub reenrol_download_delay: Duration,
    /// How many electors sign the trust list: 5 (SCMS Manager Elector Policy v1.0 §2;
    /// Profile §2.1.15: EEs are initialised with the five elector certificates).
    pub electors: u32,
    /// How many valid elector signatures a device requires of a trust list: any 3 of the
    /// 5 (Elector Technical Specification v1.1 §2.2; the published CTLs' `quorum` 3).
    pub elector_quorum: u32,
    /// The RA's certificate-request shuffle window: the maximum age of a request in the
    /// aggregator, 2 days by default ([CAMP122] Table 67 `max_cert_request_age`;
    /// [`ShufflePreset`]).
    pub shuffle_window: Duration,
    /// The RA's report shuffle window [CAMP-EE SCMS-765].
    pub report_shuffle_window: Duration,
    /// How many provisioning requests release the certificate-request shuffle before its
    /// window closes: 1,000 by default ([CAMP122] Table 67 `shuffle_threshold`;
    /// [`ShufflePreset::Brecht`] is the 10,000 / one day of [BRECHT §V-B]).
    pub shuffle_threshold: u32,
    /// How many reports release the report shuffle before its window closes
    /// [CAMP-EE SCMS-765, the same rule].
    pub report_shuffle_threshold: u32,
    /// CRL cadence: daily, the USDOT 2013 working assumption [BRECHT §VI-G].
    pub crl_cadence: Duration,
    /// Whether the CRL Generator holds a new entry until the next [`Self::crl_cadence`]
    /// boundary before publishing it.
    ///
    /// Clear by default, which publishes on decision: this crate's own flows and tests
    /// measure the revocation *path*, and a day of calendar wait in every one of them
    /// would measure the calendar instead. A deployment study sets it — the engine does
    /// whenever a scenario states a cadence — and the wait then appears as its own
    /// `issued → published` step of the decomposition, which is where 05-protocols §2.5
    /// puts it.
    pub publish_on_cadence: bool,
    /// How long after the acknowledgement the device first polls for a batch.
    pub first_batch_delay: Duration,
    /// How long between polls of a repository that is not ready: `download-minWait`,
    /// 5 minutes ([SMP] §2.1.21 Table 3; the 408 rule "retry after minWait, up to
    /// maxReqs", [SMP] §4.3.1).
    pub download_poll_interval: Duration,
    /// How many polls before the device gives up: `download-maxReqs`, 100 ([SMP] Table 3).
    pub max_download_polls: u32,
    /// How far a download request's `generationTime` may be from the RA's clock: 5 s
    /// ([CAMP122] SCMS-1203, `ra.client-signature-ttl-sec`).
    pub download_request_tolerance: Duration,
    /// Backend servers per entity, the `c` of the M/M/c.
    pub backend_servers: u32,
    /// Per-request overhead at a backend entity, beyond the cryptography.
    pub backend_overhead: Duration,
    /// Per-request overhead at a device.
    pub device_overhead: Duration,
    /// One-way latency on a backend link.
    pub backend_link_latency: Duration,
    /// Backend link bandwidth.
    pub backend_link_bandwidth_bps: u64,
    /// One-way latency on the cellular uplink.
    pub uu_link_latency: Duration,
    /// Cellular link bandwidth.
    pub uu_link_bandwidth_bps: u64,
    /// One-way latency on the 5.9 GHz air interface between a roadside unit and a vehicle.
    ///
    /// Not propagation — that is under a microsecond at V2X ranges — but the channel access
    /// a broadcast frame waits for before its first bit goes out.
    pub v2x_air_latency: Duration,
    /// The 5.9 GHz air interface's data rate, bits per second.
    ///
    /// 6 Mbit/s is the 10 MHz OFDM PHY's QPSK rate-1/2 mode, which is what the Phase 1
    /// scenario's medium PHY transmits at [EN 302 663 V1.3.1 Annex C.3 Table C.1].
    pub v2x_air_bandwidth_bps: u64,
    /// The hardware profile backend costs are read from.
    pub backend_profile: &'static str,
    /// The hardware profile device costs are read from.
    pub device_profile: &'static str,
    /// The five wire sizes no standard publishes.
    pub sizes: SizeParams,
    /// The hybrid post-quantum scheme every authority and device signs with, `None` for
    /// ECDSA P-256 alone (`security.signature`; `crate::hybrid`).
    pub hybrid: Option<crate::hybrid::HybridScheme>,
    /// The post-quantum KEM encryption to the PCA and to devices uses, `None` for ECIES
    /// alone (`security.kem`; `crate::hybrid::KemSpec`).
    pub kem: Option<crate::hybrid::KemSpec>,
    /// Committees that replace single authorities (`security.threshold`;
    /// `crate::threshold`). `None`: the CAMP deployment, one server per role and two-of-two
    /// linkage.
    pub threshold: Option<ThresholdDeployment>,
    /// How long the Misbehaviour Authority waits for a Linkage Authority's seed before
    /// asking again. **Uncited**: CAMP describes no retry; 30 s is a model parameter so
    /// that a two-of-two deployment with an LA down retries as a committee session does,
    /// and the two are compared like for like.
    pub seed_retry: Duration,
    /// The real IEEE 1609.2.1 PDU layer (`crate::scms::pdu`, WP-B1).
    pub pdu: PduLayer,
    /// How the CRL Generator issues real `SecuredCrl`s (WP-B4).
    pub crl: CrlIssuance,
    /// How the Misbehaviour Authority decides and how many cases it runs (WP-B4).
    pub ma: MaPolicy,
}

/// How the provider's CRL Generator issues 1609.2 CRLs (WP-B4; 02 §11, 03 §10).
///
/// With the PDU layer on, every publication signs a real full linkage CRL (series 1)
/// and, when series 3 or 4 have entries, full hash CRLs; with [`Self::deltas`] it also
/// signs a delta linkage CRL holding what was added since the previous publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrlIssuance {
    /// Publish only at the provider's business-day slot: a change decided on a Friday
    /// evening, a weekend or a US federal holiday goes out on the next business day,
    /// inside the "within 1 business day of any change" rule ([SM-PROV] §3.4.9).
    ///
    /// Off in the crate default for the same reason as
    /// [`ScmsParams::publish_on_cadence`]: the flow tests measure the protocol path. A
    /// deployment study (and the US production preset of WP-D1) turns it on.
    pub business_day_sla: bool,
    /// The slot's Eastern hour and minute. **Assumption**: 10:00 ET (the rule fixes the
    /// deadline, not the hour).
    pub slot_et: (u32, u32),
    /// `nextCrl - issueDate`. **Assumption**: 7 days, so that the "no later than once a
    /// week" device fetch ([CAMP122] §3 item 10) always finds a current CRL; the draft
    /// CP's ceiling is monthly ([5GAA] (254)).
    pub next_crl: Duration,
    /// Also issue a delta linkage CRL with the entries added since the previous
    /// publication.
    pub deltas: bool,
}

impl Default for CrlIssuance {
    fn default() -> CrlIssuance {
        CrlIssuance {
            business_day_sla: false,
            slot_et: (10, 0),
            next_crl: Duration::from_secs(7 * 86_400),
            deltas: true,
        }
    }
}

/// How the Misbehaviour Authority decides (gap row 28) and how many investigations it
/// carries at once (gap row 27).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaPolicy {
    /// The MA's case table capacity: investigations open at once. **Assumption**: 1,000.
    pub max_open_cases: u32,
    /// Who decides to revoke.
    pub mode: MaMode,
}

impl Default for MaPolicy {
    fn default() -> MaPolicy {
        MaPolicy {
            max_open_cases: 1_000,
            mode: MaMode::ManualReview {
                review: Duration::from_secs(8 * 3600),
            },
        }
    }
}

/// Who decides to revoke (gap row 28).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaMode {
    /// An operator reviews each case before the MA acts: the CV-pilot "minimum viable"
    /// MA ([CAMP122] §4.2-4.3; [PRIMER] pp. 6, 8). The default (owner decision). The
    /// operator's delay is **an assumption** (8 h, one working shift); a scenario sets it.
    ManualReview {
        /// Time from opening the case to the operator's go-ahead.
        review: Duration,
    },
    /// The pipeline's decision is acted on at once (today's `v2xw-threat` pipelines).
    Automatic,
}

/// The real 1609.2.1 SPDU layer's settings (`crate::scms::pdu`).
///
/// On by default: every enrolment and provisioning message the run sends is a real
/// `Ieee1609Dot2Data` carrying a `ScmsPdu` under PSID 35, signed and encrypted with real
/// keys, charged its encoded length, and opened and verified by its receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PduLayer {
    /// Build, send and verify real SPDUs. `false` falls back to the modelled sizes.
    pub enabled: bool,
    /// Keep every emitted SPDU's bytes on `ScmsState::pdus` (for the oracle and for
    /// attack studies); off by default to keep long runs lean, the counts are always kept.
    pub keep_bytes: bool,
    /// UTC Unix seconds of simulated time 0, which every `Time32` in a PDU is derived from
    /// through [`v2xw_core::time::WallClock`] (TAI since the 1609.2 epoch). Default
    /// 2026-01-05T00:00:00Z; a scenario sets its `time.t0`.
    pub t0_unix_s: i64,
    /// How often each LA's AES-128 key shared with the ACA rotates, in seconds of
    /// `Time32` (CAMP `shared_key_update_interval`, a local option, [CAMP122] Table 68).
    /// **Assumption**: one week, the i-period; 0 never rotates.
    pub la_aca_key_rotation_s: u32,
    /// The PSIDs a pseudonym certificate's `appPermissions` lists: what the EE requests
    /// in its `EeRaCertRequest` and the RA forwards to the ACA (03 §3.3). Every PSID an
    /// honest vehicle signs under must be here, or a receiver that checks the SPDU PSID
    /// against `appPermissions` (1609.2 §5.2; WP-C2) refuses the message. Default: BSM
    /// (0x20) and misbehaviour reporting (38, the PSID the reporter's pseudonym signs a
    /// report under, WP-B4); the engine adds what the scenario's vehicles send.
    /// **Assumption**: one permission profile for the whole fleet.
    pub pseudonym_psids: PsidList,
}

/// Up to eight PSIDs, ascending and distinct, by value (1609.2 requires a verifier to
/// support at least eight `appPermissions` entries, 02 §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PsidList {
    n: u8,
    psids: [u64; 8],
}

impl PsidList {
    /// The list of `psids`, sorted and deduplicated; entries past the eighth are dropped.
    #[must_use]
    pub fn of(psids: &[u64]) -> PsidList {
        let mut v = psids.to_vec();
        v.sort_unstable();
        v.dedup();
        v.truncate(8);
        let mut out = PsidList {
            n: u8::try_from(v.len()).unwrap_or(8),
            psids: [0; 8],
        };
        out.psids[..v.len()].copy_from_slice(&v);
        out
    }

    /// The PSIDs, ascending.
    #[must_use]
    pub fn as_slice(&self) -> &[u64] {
        &self.psids[..usize::from(self.n)]
    }
}

impl Default for PduLayer {
    fn default() -> PduLayer {
        PduLayer {
            enabled: true,
            keep_bytes: false,
            t0_unix_s: 1_767_571_200,
            la_aca_key_rotation_s: 7 * 86_400,
            pseudonym_psids: PsidList::of(&[0x20, 38]),
        }
    }
}

/// The certificate-request shuffle presets (03 §11 conflict, owner decision 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShufflePreset {
    /// CAMP 1.2.2: 1,000 requests or 2 days, whichever first (the default).
    Camp122,
    /// Brecht et al. 2018: 10,000 requests or one day's worth.
    Brecht,
}

impl ShufflePreset {
    /// Sets `p`'s certificate-request shuffle to this preset.
    pub fn apply(self, p: &mut ScmsParams) {
        let b = match self {
            ShufflePreset::Camp122 => BatchPolicy::CAMP_122_CERT_SHUFFLE,
            ShufflePreset::Brecht => BatchPolicy::CAMP_SHUFFLE,
        };
        p.shuffle_threshold = b.max_items;
        p.shuffle_window = b.max_delay;
    }
}

/// Which roles a committee plays, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThresholdDeployment {
    /// The signing protocol (`crate::threshold::THRESHOLD_IDS`) for the PCA and the CRL
    /// Generator.
    pub protocol: &'static str,
    /// Its build parameters.
    pub protocol_params: crate::threshold::ProtocolParams,
    /// Committee size for the signing roles.
    pub members: u16,
    /// How many must sign.
    pub quorum: u16,
    /// Whether the PCA's certificate signing is a committee's.
    pub pca: bool,
    /// Whether the CRL Generator's signing is a committee's.
    pub crl: bool,
    /// The linkage authorities as a t-of-n release committee instead of two-of-two:
    /// `(members, quorum)`.
    pub linkage: Option<(u16, u16)>,
    /// How long a coordinator waits for a round before excluding the silent.
    pub round_timeout: Duration,
    /// How long it waits before retrying a session that ran out of members.
    pub retry_after: Duration,
    /// Attempts before a session fails for good.
    pub max_attempts: u16,
    /// Ask every live member and keep the first quorum (true), or exactly a quorum.
    pub fanout_all: bool,
    /// One-way latency between a coordinator and a member: the members are separate
    /// organisations, so this is a WAN figure, not the in-house backend's.
    pub member_link_latency: Duration,
}

impl Default for ThresholdDeployment {
    /// FROST, 3-of-5, on the PCA and the CRL Generator, linkage 3-of-5; timeouts are model
    /// parameters (no deployment publishes one).
    fn default() -> ThresholdDeployment {
        ThresholdDeployment {
            protocol: crate::threshold::frost::ID,
            protocol_params: crate::threshold::ProtocolParams::default(),
            members: 5,
            quorum: 3,
            pca: true,
            crl: true,
            linkage: Some((5, 3)),
            round_timeout: Duration::from_millis(500),
            retry_after: Duration::from_secs(30),
            max_attempts: 10,
            fanout_all: true,
            member_link_latency: Duration::from_millis(20),
        }
    }
}

impl Default for ScmsParams {
    fn default() -> ScmsParams {
        ScmsParams {
            master_seed: 0,
            epoch: 0,
            i_period: Duration::from_secs(10_080 * 60),
            cert_lifetime: Duration::from_secs(10_140 * 60),
            certs_per_period: 20,
            initial_batch: 3_120,
            max_periods_ahead: 156,
            enrolment_lifetime: Duration::from_secs(6 * 365 * 86_400),
            reenrol_lead: Duration::from_secs(548 * 86_400),
            reenrol_download_delay: Duration::from_secs(5 * 60),
            electors: 5,
            elector_quorum: 3,
            shuffle_window: BatchPolicy::CAMP_122_CERT_SHUFFLE.max_delay,
            report_shuffle_window: BatchPolicy::CAMP_SHUFFLE.max_delay,
            shuffle_threshold: BatchPolicy::CAMP_122_CERT_SHUFFLE.max_items,
            report_shuffle_threshold: BatchPolicy::CAMP_SHUFFLE.max_items,
            crl_cadence: Duration::from_secs(86_400),
            publish_on_cadence: false,
            first_batch_delay: Duration::from_secs(60),
            download_poll_interval: Duration::from_secs(5 * 60),
            max_download_polls: 100,
            download_request_tolerance: Duration::from_secs(5),
            backend_servers: 4,
            backend_overhead: Duration::from_millis(1),
            device_overhead: Duration::from_millis(1),
            backend_link_latency: Duration::from_millis(10),
            backend_link_bandwidth_bps: 1_000_000_000,
            uu_link_latency: Duration::from_millis(50),
            uu_link_bandwidth_bps: 10_000_000,
            v2x_air_latency: Duration::from_millis(1),
            v2x_air_bandwidth_bps: 6_000_000,
            backend_profile: profiles::I9_11950H_WOLFSSL,
            device_profile: profiles::COHDA_MK6_BOTAN,
            sizes: SizeParams::default(),
            hybrid: None,
            kem: None,
            threshold: None,
            seed_retry: Duration::from_secs(30),
            pdu: PduLayer::default(),
            crl: CrlIssuance::default(),
            ma: MaPolicy::default(),
        }
    }
}

impl ScmsParams {
    /// When i-period `i` begins.
    #[must_use]
    pub const fn period_start(&self, i: u32) -> SimTime {
        self.i_period.saturating_mul(i as u64).after(self.epoch)
    }

    /// The validity window of a certificate issued for i-period `i`.
    ///
    /// `[start(i), start(i) + cert_lifetime)`. The lifetime is an hour longer than the
    /// period, so consecutive windows overlap by an hour — which is the whole reason
    /// CAMP-EE §2.1.5.3.2 states the two numbers separately, and the reason a device that
    /// rotates at a period boundary always has something valid to rotate to.
    #[must_use]
    pub const fn validity(&self, i: u32) -> (SimTime, SimTime) {
        let from = self.period_start(i);
        (from, self.cert_lifetime.after(from))
    }

    /// A deployment whose batching windows are short enough for a test to run through.
    ///
    /// The CAMP shuffle is "10,000 requests or one day"; a test that waits a day of
    /// simulated time measures nothing extra, so this variant sets the window to a minute
    /// and leaves every cited number alone. It exists so that no test is tempted to change
    /// a *cited* default to make itself convenient.
    #[must_use]
    pub fn quick(mut self) -> ScmsParams {
        self.shuffle_window = Duration::from_secs(60);
        self.report_shuffle_window = Duration::from_secs(60);
        self
    }
}
