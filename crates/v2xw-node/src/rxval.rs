//! Receiver validation on every node (WP-C2; IEEE 1609.2 §5.1.2, §5.2; 02 §12).
//!
//! A relying party accepts a signed SPDU only when all of this holds, and [`RxTrust`]
//! checks every item itself, from the bytes it was handed and the trust it installed:
//!
//! | Rule | Source | [`RxReject`] |
//! |---|---|---|
//! | the signer's certificate chains, link by link, to a root on the installed CTL, and every link is consistent (permissions, SSP, chain length, validity and region inside the issuer's) | §5.1.2.1–§5.1.2.4 | `Chain(kind)` |
//! | every certificate of the chain is current at the receiver's time (TAI) | §5.1.2.4a | `Chain(Expired)` |
//! | the SPDU's PSID is in the signer's `appPermissions` | §5.2 | `PsidNotPermitted` |
//! | `generationTime` lies inside the signer's validity period | §5.2, SDEE profile | `GenerationTimeOutsideValidity` |
//! | `generationLocation` lies inside the signer's region, when it has one | §5.2, SDEE profile | `OutsideRegion` |
//! | no certificate of the chain is on a hash CRL (series 2, 3, 4, 256) whose `crlCraca` and `crlSeries` match it | §5.1.3; 03 §10 | `RevokedByHash`, `IssuerRevoked` |
//! | the CRL of the signer's CRACA is not more than `overdue_CRL_tolerance` (2 weeks) past its `nextCrl` | [CAMP122] Table 67 | `CrlOverdue` |
//! | `generationTime` is not older than the message type's maximum age, nor later than the receiver's own time by more than the future tolerance (checked on every received SPDU, before the verification policy) | §5.2 relevance checks; values: C2C-CC BSP RS_BSP_168 (ASSUMPTION for the US) | `GenerationTimeTooOld`, `GenerationTimeInFuture` |
//!
//! Series-1 (linkage) revocation stays on the node's [`crate::stores::CrlGate`], which
//! now matches the certificate's `cracaId` and `crlSeries` against the CRL it came from.
//!
//! A CRL is installed only after [`RxTrust::verify_secured_crl`] has decoded it, chained
//! its signer to the installed root, and [`v2xw_sec::crl::verify_crl`] accepted it (PSID
//! 256 header, signature, `CrlSsp` series and CRACA, contents). An altered byte fails it.
//!
//! # What is cached, and what it costs
//!
//! A device validates a CA certificate once and remembers it; it validates a pseudonym
//! certificate the first time it sees it, and every later SPDU under that certificate
//! costs only its own signature verification (the frame's base verify operation).
//! IEEE 1609.2 §5.2 makes an SPDU valid when its signer's chain is (§5.1.2.1–§5.1.2.4a)
//! and the SPDU itself is consistent and verifies; the chain part depends only on the
//! certificates and on revocation, so it is the same for every SPDU under one
//! certificate until a CRL changes (§5.1.2.4a: "nothing revoked"). Deployed stacks
//! therefore validate once per certificate: Vanetza's receiver skips the chain check for
//! a signer found in its certificate cache ("certificates from cache are already
//! verified as trusted", `vanetza/security/straight_verify_service.cpp`, master at
//! `453fd973`, 2026-10-08), and its `v2/certificate_cache.cpp` keeps an authorization
//! ticket for 2 s after it was last heard and a CA certificate for an hour.
//!
//! The end-entity cache ([`EeCache`]) is keyed by the certificate's HashedId8, holds at
//! most [`EE_CACHE`] entries and evicts the least recently used one when full. An entry
//! leaves it three ways, and the next SPDU from that signer then pays the whole
//! first-sight cost again (the EE link, its hash and the ECQV reconstruction):
//! - **capacity**: the least recently used entry is evicted ([`RxStats::ee_evictions`]);
//! - **the peer certificate cache dropped the certificate** ([`RxTrust::forget`]): a
//!   node keeps no validated state for a certificate it no longer holds;
//! - **a CRL changed**: a freshly installed CRL invalidates every cached entry, CA paths
//!   included, whose `cracaId` and `crlSeries` it governs ([`RxStats::invalidated`]). A
//!   repeat of an already installed CRL changes nothing. Dropping the whole entry, rather
//!   than only re-reading revocation, is the upper bound a device that re-runs chain
//!   validation on a CRL change pays (the reconstruction is charged again too).
//!
//! Each operation that runs is recorded as a
//! `(primitive, op, bytes)` triple and priced by the node's own hardware profile through
//! [`v2xw_sec::primitive::PrimitiveCatalogue::price_all`] (see [`RxTrust::take_ops`]):
//! a CA link is IEEE 1609.2 §5.3.1's three hashes and one verification with the issuer's
//! curve, a new implicit certificate is one ECQV reconstruction plus the SHA-256 that
//! names it. The host memoises chain verdicts across nodes (a pure function of the bytes
//! and the anchors), which changes no verdict and no simulated cost.
//!
//! # Profile deviation (documented)
//!
//! Chains are validated under [`v2xw_sec::chain::ChainPolicy::us_device`], which accepts
//! the published US RA profile `{all, minChainLength 0, chainLengthRange 0}`; every
//! finding it tolerates is counted in [`RxStats::tolerated`].
//!
//! # Where this does not reach yet
//!
//! Identified (country) regions are not checked against a location: the build has no
//! country polygons, so they are counted in [`RxStats::region_unchecked`] rather than
//! passed silently. Group linkage CRL entries are not installed.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use v2xw_msg::scms_types::WireEdition;
use v2xw_msg::sec_types::Certificate;
use v2xw_msg::sec_types::ieee1609_dot2::{CertificateType, IssuerIdentifier};
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{GeographicRegion, TwoDLocation};
use v2xw_sec::cert::{self, CertKey};
use v2xw_sec::chain::{self, ChainErrorKind, ChainPolicy};
use v2xw_sec::envelope::ParsedSecured;
use v2xw_sec::hashedid;
use v2xw_sec::linkage::CrlLinkageEntry;
use v2xw_sec::primitive::{ChainLink, PrimitiveId, PrimitiveOpKind};
use v2xw_sec::sig1609::SpduSigner;

/// `overdue_CRL_tolerance` ([CAMP122] Table 67): two weeks past `nextCrl`, then the device
/// distrusts every message and warns the driver.
pub const OVERDUE_CRL_TOLERANCE_S: u32 = 14 * 86_400;

/// The pseudonym CRL series (linkage based).
pub const SERIES_PSEUDONYM: u16 = 1;

/// The oldest a safety beacon (BSM, CAM, PSM, VAM) may be on arrival, seconds.
///
/// IEEE 1609.2 (§5.2, the SDEE's relevance checks) lets a receiving entity refuse an SPDU
/// whose `generationTime` is too far in the past or in the future, and leaves the values
/// to the application's security profile. The US BSM profile is SAE J2945/1's, whose text
/// is not openly published; the one deployed profile whose values are public is the
/// CAR 2 CAR Communication Consortium Basic System Profile, RS_BSP_168: a CAM older than
/// 2 s, any other message older than 10 minutes, or anything more than 40 ms in the future
/// is discarded (the values are the profile's; Vanetza's `check_generation_time`, read
/// for behaviour only on 2026-10-21, names the same requirement, "C2C-CC Basic System
/// Profile v1.1.0, see RS_BSP_168", and none of its code is used). ASSUMPTION: the US receiver uses the same window
/// for the BSM. Settable: `security.scms.rx_beacon_max_age_s`.
pub const BEACON_MAX_AGE_S: f64 = 2.0;

/// The oldest any other application SPDU (SPaT, MAP, DENM, CPM, SRM, SSM) may be on
/// arrival, seconds (RS_BSP_168: 10 minutes; see [`BEACON_MAX_AGE_S`]).
/// Settable: `security.scms.rx_max_age_s`.
pub const MAX_AGE_S: f64 = 600.0;

/// How far after the receiver's own time a `generationTime` may lie, seconds
/// (RS_BSP_168: 40 ms; see [`BEACON_MAX_AGE_S`]). Settable: `security.scms.rx_max_future_s`.
pub const MAX_FUTURE_S: f64 = 0.040;

/// The receiver's `generationTime` relevance window (IEEE 1609.2 §5.2), in nanoseconds.
///
/// A message outside it is discarded before the verification policy, so it costs no
/// verification and reaches no application and no misbehaviour detector: a stale or
/// future-dated message is *dropped, not reported*. That is what keeps an honest sender
/// whose clock is wrong from being reported for the kinematics its wrong clock implies,
/// and what makes a replay older than the window harmless.
///
/// Misbehaviour reports, CRLs, P2PCD PDUs and WSAs are not checked here: a report may
/// wait a week on its device before upload ([SM-MBR] §2.2), and a CRL's currency has
/// its own rule (`CrlOverdue`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationTimeWindow {
    /// Maximum age of a safety beacon (BSM, CAM, PSM, VAM), ns.
    pub beacon_max_age_ns: u64,
    /// Maximum age of any other application SPDU, ns.
    pub max_age_ns: u64,
    /// Maximum lead of `generationTime` over the receiver's own time, ns.
    pub max_future_ns: u64,
}

impl Default for GenerationTimeWindow {
    fn default() -> Self {
        GenerationTimeWindow::from_secs(BEACON_MAX_AGE_S, MAX_AGE_S, MAX_FUTURE_S)
    }
}

impl GenerationTimeWindow {
    /// The window from seconds (negative values are taken as zero).
    pub fn from_secs(beacon_max_age_s: f64, max_age_s: f64, max_future_s: f64) -> Self {
        let ns = |s: f64| v2xw_core::time::secs_to_ns(s.max(0.0));
        GenerationTimeWindow {
            beacon_max_age_ns: ns(beacon_max_age_s),
            max_age_ns: ns(max_age_s),
            max_future_ns: ns(max_future_s),
        }
    }

    /// The maximum age for `ty`, or `None` when the type is not checked here.
    pub fn max_age_ns(&self, ty: v2xw_msg::MsgType) -> Option<u64> {
        use v2xw_msg::MsgType as T;
        match ty {
            T::Bsm | T::Cam | T::Psm | T::Vam => Some(self.beacon_max_age_ns),
            T::Denm | T::Spat | T::Map | T::Cpm | T::Srm | T::Ssm => Some(self.max_age_ns),
            T::Wsa | T::Crl | T::Mbr | T::P2pcd => None,
        }
    }

    /// Whether an SPDU of type `ty` stamped `generated` (on its sender's clock) is
    /// relevant at the receiver's own time `now`.
    ///
    /// # Errors
    /// [`RxReject::GenerationTimeTooOld`] or [`RxReject::GenerationTimeInFuture`].
    pub fn admits(
        &self,
        ty: v2xw_msg::MsgType,
        generated: v2xw_core::time::SimTime,
        now: v2xw_core::time::SimTime,
    ) -> Result<(), RxReject> {
        let Some(max_age) = self.max_age_ns(ty) else {
            return Ok(());
        };
        if generated > now.saturating_add(self.max_future_ns) {
            return Err(RxReject::GenerationTimeInFuture);
        }
        if now.saturating_sub(generated) > max_age {
            return Err(RxReject::GenerationTimeTooOld);
        }
        Ok(())
    }
}

/// How many validated end-entity certificates a node remembers (a bounded cache with
/// least-recently-used eviction, like the peer certificate cache, which in practice
/// bounds it first: see [`RxTrust::forget`]).
pub const EE_CACHE: usize = 4_096;

/// Why a receiver refused an SPDU or a CRL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RxReject {
    /// A chain rule failed.
    Chain(ChainErrorKind),
    /// The issuer is not in the node's CA cache (the P2PCD case: nothing concluded).
    UnknownIssuer,
    /// The SPDU's PSID is not in the signer's `appPermissions`.
    PsidNotPermitted,
    /// `generationTime` is outside the signer's validity period.
    GenerationTimeOutsideValidity,
    /// `generationLocation` is outside the signer's region.
    OutsideRegion,
    /// The signer is on a hash CRL.
    RevokedByHash,
    /// A CA above the signer is on a hash CRL.
    IssuerRevoked,
    /// The signer's CRACA's CRL is more than two weeks overdue.
    CrlOverdue,
    /// `generationTime` is older than the message type's maximum age
    /// ([`GenerationTimeWindow`]): a stale or replayed message, or a sender whose clock
    /// is behind.
    GenerationTimeTooOld,
    /// `generationTime` is later than the receiver's own time by more than the future
    /// tolerance ([`GenerationTimeWindow`]): a sender whose clock is ahead.
    GenerationTimeInFuture,
    /// A CRL that does not decode.
    CrlMalformed,
    /// A CRL whose signer cannot be resolved or does not chain to the installed root.
    CrlSignerUntrusted,
    /// A CRL that [`v2xw_sec::crl::verify_crl`] does not accept (signature, PSID 256
    /// header, `CrlSsp` series or CRACA, contents).
    CrlRejected,
    /// A CRL, full or delta, that is not newer than the one installed for the same CRACA
    /// and series (its `crlSerial`, or `iRev` and `indexWithinI`, then `issueDate`).
    CrlStale,
}

impl RxReject {
    /// A stable name for counters and records.
    pub fn name(self) -> &'static str {
        match self {
            RxReject::Chain(k) => match k {
                ChainErrorKind::Empty => "chain-empty",
                ChainErrorKind::TooLong => "chain-too-long",
                ChainErrorKind::Malformed => "chain-malformed",
                ChainErrorKind::NotRootedInSelfSigned => "chain-not-rooted",
                ChainErrorKind::BadRootSignature => "chain-bad-root-signature",
                ChainErrorKind::UntrustedRoot => "chain-untrusted-root",
                ChainErrorKind::IssuerMismatch => "chain-issuer-mismatch",
                ChainErrorKind::BadSignature => "chain-bad-signature",
                ChainErrorKind::BadReconstruction => "chain-bad-reconstruction",
                ChainErrorKind::PermissionNotCovered => "chain-permission-not-covered",
                ChainErrorKind::EeTypeNotAllowed => "chain-ee-type",
                ChainErrorKind::SspNotCovered => "chain-ssp",
                ChainErrorKind::ChainLength => "chain-length",
                ChainErrorKind::ValidityNotContained => "chain-validity-not-contained",
                ChainErrorKind::RegionNotContained => "chain-region-not-contained",
                ChainErrorKind::RegionUnsupported => "chain-region-unsupported",
                ChainErrorKind::Expired => "chain-expired",
            },
            RxReject::UnknownIssuer => "unknown-issuer",
            RxReject::PsidNotPermitted => "psid-not-permitted",
            RxReject::GenerationTimeOutsideValidity => "generation-time-outside-validity",
            RxReject::OutsideRegion => "outside-region",
            RxReject::RevokedByHash => "revoked-by-hash",
            RxReject::IssuerRevoked => "issuer-revoked",
            RxReject::CrlOverdue => "crl-overdue",
            RxReject::GenerationTimeTooOld => "generation-time-too-old",
            RxReject::GenerationTimeInFuture => "generation-time-in-future",
            RxReject::CrlMalformed => "crl-malformed",
            RxReject::CrlSignerUntrusted => "crl-signer-untrusted",
            RxReject::CrlRejected => "crl-rejected",
            RxReject::CrlStale => "crl-stale",
        }
    }

    /// Whether the refusal is a revocation (the message is `Revoked`, not `Invalid`).
    pub fn is_revocation(self) -> bool {
        matches!(self, RxReject::RevokedByHash | RxReject::IssuerRevoked)
    }
}

/// What a node's receiver validation did, for the run report and attack studies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RxStats {
    /// SPDUs checked with a trust store installed.
    pub checked: u64,
    /// SPDUs checked with no trust store installed (signature only).
    pub chain_unchecked: u64,
    /// Refusals by [`RxReject::name`].
    pub refused: BTreeMap<&'static str, u64>,
    /// CA paths validated (cache misses).
    pub ca_validations: u64,
    /// End-entity certificates validated (cache misses).
    pub ee_validations: u64,
    /// Post-quantum issuer signatures checked on newly learned certificates (the NDSS 2024
    /// Partially-Hybrid mode: `NodeConfig::cert_verify_op`).
    pub cert_pq_verifications: u64,
    /// SPDUs whose signer was already validated (cache hits: signature only).
    pub ee_hits: u64,
    /// Validated end-entity certificates evicted for capacity or because the peer
    /// certificate cache dropped them.
    pub ee_evictions: u64,
    /// Cached entries (end-entity and CA path) a freshly installed CRL invalidated.
    pub invalidated: u64,
    /// Chain findings tolerated as the published RA profile.
    pub tolerated: u64,
    /// SPDUs whose signer has an identified region the node cannot place a location in.
    pub region_unchecked: u64,
    /// CRLs installed after verification.
    pub crls_accepted: u64,
    /// CRLs refused, by reason.
    pub crls_refused: BTreeMap<&'static str, u64>,
    /// Validation operations the node's hardware profile could not price (charged
    /// nothing, and counted so the gap is visible).
    pub unpriced: u64,
    /// Simulated time charged for validation, ns.
    pub charged_ns: u64,
}

/// Prices `ops` on the node hardware profile `profile_id` through
/// [`v2xw_sec::primitive::PrimitiveCatalogue::price_all`]; `None` when the profile has no
/// anchors or one operation is unpriced.
pub fn price(
    ops: &[(PrimitiveId, PrimitiveOpKind, u32)],
    profile_id: &str,
) -> Option<v2xw_core::time::Duration> {
    if ops.is_empty() {
        return Some(v2xw_core::time::Duration::from_nanos(0));
    }
    let anchors = v2xw_sec::primitive::anchors::for_node_profile(profile_id)?;
    v2xw_sec::primitive::PrimitiveCatalogue::standard().price_all(ops, anchors.anchors)
}

/// A validated CA path, from the signer's issuer up to the root.
#[derive(Debug, Clone)]
struct CaPath {
    /// The certificates, issuer of the EE first, root last.
    certs: Vec<Arc<Certificate>>,
    /// The verification key of `certs[0]` (the EE's issuer).
    issuer_key: CertKey,
}

/// One validated end-entity certificate, with what a CRL is matched against.
#[derive(Debug, Clone, Copy)]
struct EeEntry {
    verdict: Result<(), RxReject>,
    craca: [u8; 3],
    series: u16,
    /// Recency stamp: the key of this entry in [`EeCache::order`].
    used: u64,
}

/// The validated end-entity certificate cache: HashedId8 → verdict, at most `capacity`
/// entries, least recently used evicted first (see the module documentation).
#[derive(Debug, Clone)]
pub struct EeCache {
    capacity: usize,
    entries: BTreeMap<[u8; 8], EeEntry>,
    /// Recency stamp → HashedId8; the first key is the least recently used.
    order: BTreeMap<u64, [u8; 8]>,
    tick: u64,
}

impl Default for EeCache {
    fn default() -> Self {
        Self::new(EE_CACHE)
    }
}

impl EeCache {
    /// An empty cache holding at most `capacity` (at least one) certificates.
    pub fn new(capacity: usize) -> Self {
        EeCache {
            capacity: capacity.max(1),
            entries: BTreeMap::new(),
            order: BTreeMap::new(),
            tick: 0,
        }
    }

    /// The most certificates it holds.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// How many certificates it holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether it holds `id`, without refreshing it.
    pub fn contains(&self, id: &[u8; 8]) -> bool {
        self.entries.contains_key(id)
    }

    fn get(&mut self, id: &[u8; 8]) -> Option<Result<(), RxReject>> {
        let e = self.entries.get_mut(id)?;
        self.order.remove(&e.used);
        self.tick += 1;
        e.used = self.tick;
        self.order.insert(self.tick, *id);
        Some(e.verdict)
    }

    /// Inserts `id`; returns how many entries were evicted to make room (0 or 1).
    fn insert(&mut self, id: [u8; 8], verdict: Result<(), RxReject>, c: &Certificate) -> u64 {
        self.remove(&id);
        let mut evicted = 0;
        while self.entries.len() >= self.capacity {
            let Some((_, lru)) = self.order.pop_first() else {
                break;
            };
            self.entries.remove(&lru);
            evicted += 1;
        }
        self.tick += 1;
        self.order.insert(self.tick, id);
        self.entries.insert(
            id,
            EeEntry {
                verdict,
                craca: craca3(c),
                series: series(c),
                used: self.tick,
            },
        );
        evicted
    }

    fn remove(&mut self, id: &[u8; 8]) -> bool {
        match self.entries.remove(id) {
            Some(e) => {
                self.order.remove(&e.used);
                true
            }
            None => false,
        }
    }

    /// Drops every entry whose `(cracaId, crlSeries)` is `(craca, series)`.
    fn invalidate(&mut self, craca: [u8; 3], series: u16) -> u64 {
        let gone: Vec<[u8; 8]> = self
            .entries
            .iter()
            .filter(|(_, e)| e.craca == craca && e.series == series)
            .map(|(id, _)| *id)
            .collect();
        for id in &gone {
            self.remove(id);
        }
        gone.len() as u64
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }
}

/// What one installed CRL said, for the overdue rule and freshness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CrlMeta {
    next_crl: u32,
    /// [`crl_order`] of the CRL (its `issueDate` last): a later one of the same CRACA and
    /// series must be higher.
    order: (u32, u32),
}

/// Where a CRL stands in its `(crlCraca, crlSeries)` sequence (R2): `crlSerial` in a hash
/// CRL, `iRev · 256 + indexWithinI` in a linkage CRL (both count every full and delta
/// CRL the CRACA issues for the series, `Ieee1609Dot2CrlBaseTypes`), then `issueDate`,
/// which keeps a Generator whose Uint8 `indexWithinI` has saturated in order. A CRL whose
/// order is not above the installed one's is stale: a replay of an earlier list, full or
/// delta, cannot move the node's `nextCrl` (the overdue clock) backwards.
fn crl_order(c: &v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::CrlContents) -> (u32, u32) {
    use v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::TypeSpecificCrlContents as T;
    let counter = match &c.type_specific {
        T::fullHashCrl(h) | T::deltaHashCrl(h) => h.crl_serial.0,
        T::fullLinkedCrl(l) | T::deltaLinkedCrl(l) => {
            u32::from(l.i_rev.0.0) * 256 + u32::from(l.index_within_i.0)
        }
        _ => 0,
    };
    (counter, c.issue_date.0.0)
}

/// One signed CRL a node holds ([`RxTrust::held_crls`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeldCrl {
    /// `crlCraca`.
    pub craca: [u8; 8],
    /// `crlSeries`.
    pub series: u16,
    /// Its place in the series: `crlSerial` (a hash CRL) or `iRev · 256 + indexWithinI`
    /// (a linkage CRL), as [`crl_order`] reads it.
    pub counter: u32,
    /// `issueDate`, Time32.
    pub issue_date: u32,
    /// `nextCrl`, Time32.
    pub next_crl: u32,
}

/// What a verified CRL hands the node to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCrl {
    /// `crlCraca`.
    pub craca: [u8; 8],
    /// `crlSeries`.
    pub series: u16,
    /// `issueDate` / `nextCrl`.
    pub issue_date: u32,
    /// `nextCrl`.
    pub next_crl: u32,
    /// `iRev` of a linkage CRL.
    pub i_rev: Option<u32>,
    /// The linkage entries (series 1) at `iRev`, empty for a hash CRL.
    pub linkage: Vec<CrlLinkageEntry>,
    /// The `HashedId10` entries (hash CRL), empty for a linkage CRL.
    pub hashes: Vec<[u8; 10]>,
    /// False when this exact CRL was already installed (no work, no change).
    pub fresh: bool,
}

/// What a series-1 revocation check reads from a certificate: `iCert`, the linkage
/// value, `cracaId` and `crlSeries`, all out of the certificate's own bytes.
pub type LinkageKey = (u16, v2xw_sec::linkage::LinkageValue, [u8; 3], u16);

/// The [`LinkageKey`] of `c`, when it carries `linkageData`.
pub fn linkage_key(c: &Certificate) -> Option<LinkageKey> {
    let (i, lv) = v2xw_sec::credential::linkage_of(c)?;
    Some((i, lv, craca3(c), series(c)))
}

/// Takes the validation operations `sec` performed and prices them on the hardware
/// profile `profile_id`: the extra service time the frame costs. Unpriced work charges
/// nothing and is counted in [`RxStats::unpriced`].
pub fn charge(
    sec: &mut crate::secure::NodeSecurity,
    profile_id: &str,
) -> v2xw_core::time::Duration {
    let ops = sec.take_validation_ops();
    if ops.is_empty() {
        return v2xw_core::time::Duration::from_nanos(0);
    }
    match price(&ops, profile_id) {
        Some(d) => {
            let t = sec.trust_mut();
            t.stats.charged_ns = t.stats.charged_ns.saturating_add(d.as_nanos());
            d
        }
        None => {
            sec.trust_mut().stats.unpriced += ops.len() as u64;
            v2xw_core::time::Duration::from_nanos(0)
        }
    }
}

/// One memoised chain verdict (host-side only; see the module documentation).
#[derive(Debug, Clone)]
enum Memo {
    Ok { key: CertKey, tolerated: u32 },
    Err(ChainErrorKind),
}

thread_local! {
    static CHAIN_MEMO: RefCell<BTreeMap<[u8; 32], Memo>> = RefCell::new(BTreeMap::new());
}

fn id8(c: &Certificate) -> Option<[u8; 8]> {
    cert::cert_id8(c).ok().map(|h| hashedid::id8_bytes(&h))
}

fn id10(c: &Certificate) -> Option<[u8; 10]> {
    v2xw_sec::sig1609::cert_hashed_id10(c).ok()
}

fn issuer_id8(c: &Certificate) -> Option<[u8; 8]> {
    match &c.0.issuer {
        IssuerIdentifier::sha256AndDigest(d) | IssuerIdentifier::sha384AndDigest(d) => {
            Some(hashedid::id8_bytes(d))
        }
        _ => None,
    }
}

fn craca3(c: &Certificate) -> [u8; 3] {
    let mut b = [0u8; 3];
    b.copy_from_slice(&c.0.to_be_signed.craca_id.0[..]);
    b
}

fn series(c: &Certificate) -> u16 {
    c.0.to_be_signed.crl_series.0.0
}

/// The primitive operations validating the explicit link `sub` → `iss` performs.
fn explicit_link_ops(
    sub: &Certificate,
    iss: &Certificate,
) -> Vec<(PrimitiveId, PrimitiveOpKind, u32)> {
    let signer = match cert::verification_key(iss, None) {
        Ok(CertKey::P384(_)) => PrimitiveId::ECDSA_P384,
        _ => PrimitiveId::ECDSA_P256_SHA256,
    };
    let tbs = cert::encode(sub).map_or(0, |b| b.len());
    let ib = cert::encode(iss).map_or(0, |b| b.len());
    ChainLink::Explicit {
        signer,
        tbs_bytes: u32::try_from(tbs).unwrap_or(u32::MAX),
        issuer_bytes: u32::try_from(ib).unwrap_or(u32::MAX),
    }
    .ops()
}

/// Lat/lon of a `TwoDLocation`, degrees.
fn deg(l: &TwoDLocation) -> (f64, f64) {
    (
        f64::from(l.latitude.0.0) / 1e7,
        f64::from(l.longitude.0.0) / 1e7,
    )
}

/// `Some(inside)` when the region kind is one a location can be placed in.
fn location_in(region: &GeographicRegion, lat: f64, lon: f64) -> Option<bool> {
    match region {
        GeographicRegion::circularRegion(c) => {
            let (clat, clon) = deg(&c.center);
            // Equirectangular distance: within 0.1 % of the great circle at the radii a
            // certificate region uses (Uint16 metres, at most 65.5 km).
            let r = 6_371_008.8_f64;
            let x =
                (lon - clon).to_radians() * v2xw_core::math::cos(((lat + clat) / 2.0).to_radians());
            let y = (lat - clat).to_radians();
            Some((x * x + y * y).sqrt() * r <= f64::from(c.radius.0))
        }
        GeographicRegion::rectangularRegion(rs) => Some(rs.0.iter().any(|r| {
            let (n, w) = deg(&r.north_west);
            let (s, e) = deg(&r.south_east);
            lat <= n && lat >= s && lon >= w && lon <= e
        })),
        _ => None,
    }
}

/// The node's receiver trust: installed roots, validated CA paths, hash revocations and
/// installed CRLs. See the module documentation.
#[derive(Debug, Clone, Default)]
pub struct RxTrust {
    /// HashedId32 of every root the installed CTL approves.
    anchors: BTreeSet<[u8; 32]>,
    /// Validated CA paths, by the HashedId8 of their bottom certificate.
    paths: BTreeMap<[u8; 8], Result<CaPath, RxReject>>,
    /// Validated end-entity certificates, by HashedId8.
    ees: EeCache,
    /// Hash revocations: `(cracaId, crlSeries) → HashedId10`.
    hash_revoked: BTreeMap<([u8; 3], u16), BTreeSet<[u8; 10]>>,
    /// The installed CRL of each `(crlCraca, crlSeries)`.
    crls: BTreeMap<([u8; 8], u16), CrlMeta>,
    /// SHA-256 of every CRL already installed.
    seen_crls: BTreeSet<[u8; 32]>,
    /// Operations performed since the last [`RxTrust::take_ops`].
    ops: Vec<(PrimitiveId, PrimitiveOpKind, u32)>,
    /// What validation did.
    pub stats: RxStats,
    /// `overdue_CRL_tolerance` for this device, seconds; `None` is the profile's
    /// [`OVERDUE_CRL_TOLERANCE_S`] (`security.scms.crl_overdue_tolerance_s`).
    overdue_tolerance_s: Option<u32>,
    /// The `generationTime` relevance window; `None` is [`GenerationTimeWindow::default`].
    generation_window: Option<GenerationTimeWindow>,
}

impl VerifiedCrl {
    /// What a `SecuredCrl` the node could **not** verify claims, decoded without any
    /// trust (`fresh` false): only to recognise the entries of a composite CRL download
    /// that this CRL alone carries, which the node then drops instead of installing (or
    /// refusing the whole download for). `None` when it does not decode.
    #[must_use]
    pub fn claimed(bytes: &[u8]) -> Option<VerifiedCrl> {
        use v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::TypeSpecificCrlContents as T;
        let opened = v2xw_sec::crl::decode_crl(bytes).ok()?;
        let c = &opened.contents;
        let mut craca = [0u8; 8];
        craca.copy_from_slice(&c.crl_craca.0[..]);
        Some(VerifiedCrl {
            craca,
            series: c.crl_series.0.0,
            issue_date: c.issue_date.0.0,
            next_crl: c.next_crl.0.0,
            i_rev: match &c.type_specific {
                T::fullLinkedCrl(l) | T::deltaLinkedCrl(l) => Some(u32::from(l.i_rev.0.0)),
                _ => None,
            },
            linkage: v2xw_sec::crl::linkage_entries(c),
            hashes: v2xw_sec::crl::hash_entries(c)
                .into_iter()
                .map(|(h, _)| h)
                .collect(),
            fresh: false,
        })
    }

    /// `e` (an entry at its own i-period, as the CRL Generator records it) as this CRL
    /// carries it: seeds walked forward to `iRev` and `iMax` kept, or `None` when the
    /// entry is exhausted before `iRev` (pruned, [CAMP122] §5.2.10.2.3) or this is not a
    /// linkage CRL. `Some(Err(()))` when the CRL should carry it and does not, or when the
    /// entry's period is after `iRev` (no CRL signs an entry from its future).
    pub fn signed_form(&self, e: &CrlLinkageEntry) -> Option<Result<CrlLinkageEntry, ()>> {
        let i_rev = self.i_rev?;
        // An entry recorded after `iRev` is one this CRL cannot sign: the Generator's
        // `iRev` is at least every entry's own period. Treating it as pruned let a
        // relay append fabricated entries at the current period to an older signed CRL.
        if e.i > i_rev {
            return Some(Err(()));
        }
        if e.i.saturating_add(e.max_forward) < i_rev {
            return None;
        }
        let walk =
            |la, ls| (e.i..i_rev).fold(ls, |s, _| v2xw_sec::linkage::linkage_seed_next(la, s));
        let r = CrlLinkageEntry {
            i: i_rev,
            la_id1: e.la_id1,
            la_id2: e.la_id2,
            ls1_i: walk(e.la_id1, e.ls1_i),
            ls2_i: walk(e.la_id2, e.ls2_i),
            jmax: e.jmax.min(255),
            max_forward: e.i.saturating_add(e.max_forward).min(65_535) - i_rev,
        };
        Some(if self.linkage.contains(&r) {
            Ok(r)
        } else {
            Err(())
        })
    }
}

impl RxTrust {
    /// Whether a trust store is installed (otherwise SPDUs are checked by signature only,
    /// and counted in [`RxStats::chain_unchecked`]).
    pub fn is_installed(&self) -> bool {
        !self.anchors.is_empty()
    }

    /// How many signed CRLs are installed, one per `(crlCraca, crlSeries)`.
    pub fn crls_held(&self) -> usize {
        self.crls.len()
    }

    /// The signed CRL this node holds for each `(crlCraca, crlSeries)`, in that order:
    /// what a record can show to prove a device holds a CRACA's current list (OF-3).
    pub fn held_crls(&self) -> Vec<HeldCrl> {
        self.crls
            .iter()
            .map(|((craca, series), m)| HeldCrl {
                craca: *craca,
                series: *series,
                counter: m.order.0,
                issue_date: m.order.1,
                next_crl: m.next_crl,
            })
            .collect()
    }

    /// Installs a CTL-approved root as a trust anchor.
    pub fn install_root(&mut self, root: &Certificate) {
        if let Ok(h) = cert::hashed_id32(root) {
            self.anchors.insert(h);
            self.paths.clear();
            self.ees.clear();
        }
    }

    /// Installs `root` as a trust anchor unless it already is one; `true` when it was new.
    pub fn install_root_if_new(&mut self, root: &Certificate) -> bool {
        match cert::hashed_id32(root) {
            Ok(h) if !self.anchors.contains(&h) => {
                self.install_root(root);
                true
            }
            _ => false,
        }
    }

    /// Keeps only the anchors in `keep` (a newer CTL's approved roots): returns how many
    /// were dropped, and when any was, forgets every validated path and signer, since a
    /// path that ended at a dropped root no longer validates (edge-case package, item 8).
    pub fn retain_anchors(&mut self, keep: &BTreeSet<[u8; 32]>) -> usize {
        let before = self.anchors.len();
        self.anchors.retain(|a| keep.contains(a));
        let removed = before - self.anchors.len();
        if removed > 0 {
            self.paths.clear();
            self.ees.clear();
        }
        removed
    }

    /// Sets how many validated end-entity certificates the node remembers, emptying the
    /// cache (a device configured with a different capacity starts with none).
    pub fn set_ee_capacity(&mut self, capacity: usize) {
        self.ees = EeCache::new(capacity);
    }

    /// Sets how long past a CRL's `nextCrl` the device keeps trusting the certificates
    /// that CRL governs ([CAMP122] Table 67 `overdue_CRL_tolerance`; two weeks unless set).
    pub fn set_overdue_crl_tolerance(&mut self, seconds: u32) {
        self.overdue_tolerance_s = Some(seconds);
    }

    /// The device's `overdue_CRL_tolerance`, seconds.
    pub fn overdue_crl_tolerance_s(&self) -> u32 {
        self.overdue_tolerance_s.unwrap_or(OVERDUE_CRL_TOLERANCE_S)
    }

    /// Sets the device's `generationTime` relevance window (`security.scms.rx_*`).
    pub fn set_generation_time_window(&mut self, w: GenerationTimeWindow) {
        self.generation_window = Some(w);
    }

    /// The device's `generationTime` relevance window.
    pub fn generation_time_window(&self) -> GenerationTimeWindow {
        self.generation_window.unwrap_or_default()
    }

    /// The IEEE 1609.2 §5.2 relevance check of a received SPDU's `generationTime`
    /// against this device's own time `now` ([`GenerationTimeWindow`]). Runs whether or
    /// not a trust store is installed, before the verification policy. A refusal is
    /// counted in [`RxStats::refused`] by name.
    ///
    /// # Errors
    /// [`RxReject::GenerationTimeTooOld`] or [`RxReject::GenerationTimeInFuture`].
    pub fn check_generation_time(
        &mut self,
        ty: v2xw_msg::MsgType,
        generated: v2xw_core::time::SimTime,
        now: v2xw_core::time::SimTime,
    ) -> Result<(), RxReject> {
        self.generation_time_window()
            .admits(ty, generated, now)
            .map_err(|r| self.refuse(r))
    }

    /// The validated end-entity certificate cache.
    pub fn ee_cache(&self) -> &EeCache {
        &self.ees
    }

    /// Forgets the validated state of the certificate `id` (its HashedId8), when the
    /// node's peer certificate cache evicts it: a device keeps no verdict for a
    /// certificate it no longer holds, so its next SPDU pays first sight again. Counted in
    /// [`RxStats::ee_evictions`] when there was something to forget.
    pub fn forget(&mut self, id: [u8; 8]) {
        if self.ees.remove(&id) {
            self.stats.ee_evictions += 1;
        }
    }

    /// Validates the CA path of every certificate of a chain file the node installs (a
    /// device checks its local certificate chain file on install, as `DeviceTrust` does):
    /// the published RA profile is met here, and counted. Returns how many validated.
    pub fn validate_chain_file(&mut self, cas: &BTreeMap<[u8; 8], Arc<Certificate>>) -> usize {
        let ids: Vec<[u8; 8]> = cas.keys().copied().collect();
        ids.into_iter()
            .filter(|id| self.ca_path(cas, *id).is_ok())
            .count()
    }

    /// Validates the CA path of `id`, a CA certificate just added to `cas` by P2PCD
    /// (WP-C3), to an installed root. Every path cached as "issuer unknown" is forgotten
    /// first, since the new certificate may be the missing link; a refused certificate's
    /// own entry is dropped so the cache does not keep a path the caller removes.
    ///
    /// # Errors
    /// The rule its path breaks; [`RxReject::UnknownIssuer`] when a link above it is still
    /// missing.
    pub fn validate_learned_ca(
        &mut self,
        cas: &BTreeMap<[u8; 8], Arc<Certificate>>,
        id: [u8; 8],
    ) -> Result<(), RxReject> {
        self.paths
            .retain(|_, p| !matches!(p, Err(RxReject::UnknownIssuer)));
        let r = self.ca_path(cas, id).map(|_| ());
        if r.is_err() {
            self.paths.remove(&id);
        }
        r
    }

    /// The operations performed since the last call, for the node to price with its
    /// hardware profile.
    pub fn take_ops(&mut self) -> Vec<(PrimitiveId, PrimitiveOpKind, u32)> {
        core::mem::take(&mut self.ops)
    }

    fn refuse(&mut self, r: RxReject) -> RxReject {
        *self.stats.refused.entry(r.name()).or_insert(0) += 1;
        r
    }

    /// The validated CA path whose bottom certificate is `ca` (its issuer's path up to a
    /// root), from the CA cache `cas`. Charged once per node.
    fn ca_path(
        &mut self,
        cas: &BTreeMap<[u8; 8], Arc<Certificate>>,
        ca_id: [u8; 8],
    ) -> Result<CaPath, RxReject> {
        if let Some(p) = self.paths.get(&ca_id) {
            return p.clone();
        }
        let r = self.build_path(cas, ca_id);
        self.paths.insert(ca_id, r.clone());
        r
    }

    fn build_path(
        &mut self,
        cas: &BTreeMap<[u8; 8], Arc<Certificate>>,
        ca_id: [u8; 8],
    ) -> Result<CaPath, RxReject> {
        let first = cas.get(&ca_id).ok_or(RxReject::UnknownIssuer)?;
        let mut certs = vec![Arc::clone(first)];
        while let Some(next) = issuer_id8(certs.last().expect("non-empty")) {
            if certs.len() >= chain::MAX_CHAIN_LENGTH {
                return Err(RxReject::Chain(ChainErrorKind::TooLong));
            }
            let c = cas.get(&next).ok_or(RxReject::UnknownIssuer)?;
            certs.push(Arc::clone(c));
        }
        self.stats.ca_validations += 1;
        // The cost a device pays for this path once: every explicit link (the root's
        // self-signature included) and every implicit reconstruction.
        for k in 0..certs.len() {
            let sub = &certs[k];
            let iss = certs.get(k + 1).unwrap_or(sub);
            match sub.0.r_type {
                CertificateType::explicit => self.ops.extend(explicit_link_ops(sub, iss)),
                _ => self.ops.extend(ChainLink::Implicit.ops()),
            }
        }
        let owned: Vec<Certificate> = certs.iter().map(|c| (**c).clone()).collect();
        let mut key_bytes = Vec::new();
        for c in &owned {
            key_bytes.extend(cert::encode(c).unwrap_or_default());
        }
        for a in &self.anchors {
            key_bytes.extend_from_slice(a);
        }
        let memo_key = v2xw_core::hash::sha256(&key_bytes);
        let memo = CHAIN_MEMO.with(|m| m.borrow().get(&memo_key).cloned());
        let memo = match memo {
            Some(m) => m,
            None => {
                // Currency is checked per node at its own time below, so the memo is
                // independent of when it was computed.
                let policy = ChainPolicy {
                    now: None,
                    ..ChainPolicy::us_device(self.anchors.clone(), 0)
                };
                let m = match chain::validate(&owned, &policy) {
                    Ok(v) => Memo::Ok {
                        key: v.keys[0].clone(),
                        tolerated: u32::try_from(v.tolerated.len()).unwrap_or(u32::MAX),
                    },
                    Err(errs) => {
                        Memo::Err(errs.first().map_or(ChainErrorKind::Malformed, |e| e.kind))
                    }
                };
                CHAIN_MEMO.with(|c| c.borrow_mut().insert(memo_key, m.clone()));
                m
            }
        };
        match memo {
            Memo::Ok { key, tolerated } => {
                self.stats.tolerated += u64::from(tolerated);
                Ok(CaPath {
                    certs,
                    issuer_key: key,
                })
            }
            Memo::Err(k) => Err(RxReject::Chain(k)),
        }
    }

    fn overdue(&self, craca: [u8; 3], now32: u32) -> bool {
        self.crls.iter().any(|((c, s), m)| {
            *s == SERIES_PSEUDONYM
                && c[5..] == craca
                && now32 > m.next_crl.saturating_add(self.overdue_crl_tolerance_s())
        })
    }

    fn hash_revokes(&self, c: &Certificate) -> bool {
        let Some(id) = id10(c) else {
            return false;
        };
        self.hash_revoked
            .get(&(craca3(c), series(c)))
            .is_some_and(|s| s.contains(&id))
    }

    /// Validates the signer `ee` of `parsed` at the receiver's `now32` (Time32), with the
    /// node's CA cache `cas`. `Ok` when every rule of the module table holds.
    ///
    /// # Errors
    /// The first rule that failed; [`RxReject::UnknownIssuer`] means nothing was
    /// concluded (the caller answers `Unverifiable`).
    pub fn validate_signer(
        &mut self,
        cas: &BTreeMap<[u8; 8], Arc<Certificate>>,
        ee: &Certificate,
        parsed: &ParsedSecured,
        now32: u32,
    ) -> Result<(), RxReject> {
        if !self.is_installed() {
            self.stats.chain_unchecked += 1;
            return Ok(());
        }
        self.stats.checked += 1;
        let r = self.validate_inner(cas, ee, parsed, now32);
        match r {
            Err(RxReject::UnknownIssuer) | Ok(()) => r,
            Err(e) => Err(self.refuse(e)),
        }
    }

    /// Validates a signer certificate that reached the node without the SPDU it signed:
    /// a frame the engine sizes rather than encodes carries the certificate it would have
    /// attached ([`crate::runtime::RxFrame::signer_certificate`]). Every rule of
    /// [`RxTrust::validate_signer`] that reads the certificate alone applies (the overdue
    /// CRL, validity at the receiver's `now32`, the chain to an installed root, hash
    /// revocation); the SPDU-level ones (PSID, `generationTime`, `generationLocation`)
    /// need bytes the frame does not carry. Before this check such a frame verified under
    /// any root, one the CTL had dropped included (R-TRUSTLEAK).
    ///
    /// # Errors
    /// As [`RxTrust::validate_signer`].
    pub fn validate_certificate(
        &mut self,
        cas: &BTreeMap<[u8; 8], Arc<Certificate>>,
        ee: &Certificate,
        now32: u32,
    ) -> Result<(), RxReject> {
        if !self.is_installed() {
            self.stats.chain_unchecked += 1;
            return Ok(());
        }
        self.stats.checked += 1;
        let r = if self.overdue(craca3(ee), now32) {
            Err(RxReject::CrlOverdue)
        } else {
            self.validate_certificate_inner(cas, ee, now32)
        };
        match r {
            Err(RxReject::UnknownIssuer) | Ok(()) => r,
            Err(e) => Err(self.refuse(e)),
        }
    }

    fn validate_inner(
        &mut self,
        cas: &BTreeMap<[u8; 8], Arc<Certificate>>,
        ee: &Certificate,
        parsed: &ParsedSecured,
        now32: u32,
    ) -> Result<(), RxReject> {
        if self.overdue(craca3(ee), now32) {
            return Err(RxReject::CrlOverdue);
        }
        // SPDU-level rules first: they cost nothing.
        if !chain::ee_permits_psid(ee, parsed.psid) {
            return Err(RxReject::PsidNotPermitted);
        }
        let (start, end) = v2xw_sec::ctl::validity(ee);
        if let Some(gt) = parsed.generation_time {
            let s = gt / 1_000_000;
            if s < start || s >= end {
                return Err(RxReject::GenerationTimeOutsideValidity);
            }
        }
        if let (Some(region), Some(loc)) = (&ee.0.to_be_signed.region, &parsed.generation_location)
        {
            let lat = f64::from(loc.lat_tenth_microdeg) / 1e7;
            let lon = f64::from(loc.lon_tenth_microdeg) / 1e7;
            match location_in(region, lat, lon) {
                Some(true) => {}
                Some(false) => return Err(RxReject::OutsideRegion),
                None => self.stats.region_unchecked += 1,
            }
        }
        self.validate_certificate_inner(cas, ee, now32)
    }

    /// The certificate rules of [`RxTrust::validate_inner`], after the SPDU-level ones:
    /// validity at `now32`, the chain, the end-entity link and hash revocation.
    fn validate_certificate_inner(
        &mut self,
        cas: &BTreeMap<[u8; 8], Arc<Certificate>>,
        ee: &Certificate,
        now32: u32,
    ) -> Result<(), RxReject> {
        let (start, end) = v2xw_sec::ctl::validity(ee);
        let now = u64::from(now32);
        if now < start || now >= end {
            return Err(RxReject::Chain(ChainErrorKind::Expired));
        }
        // The chain: the issuer's validated path, then the EE link.
        let ca_id = issuer_id8(ee).ok_or(RxReject::Chain(ChainErrorKind::NotRootedInSelfSigned))?;
        let path = self.ca_path(cas, ca_id)?;
        for c in &path.certs {
            let (s, e) = v2xw_sec::ctl::validity(c);
            if now < s || now >= e {
                return Err(RxReject::Chain(ChainErrorKind::Expired));
            }
            if self.hash_revokes(c) {
                return Err(RxReject::IssuerRevoked);
            }
        }
        let ee_id = id8(ee).ok_or(RxReject::Chain(ChainErrorKind::Malformed))?;
        let verdict = match self.ees.get(&ee_id) {
            Some(v) => {
                self.stats.ee_hits += 1;
                v
            }
            None => {
                self.stats.ee_validations += 1;
                let coer = cert::encode(ee).map_or(0, |b| b.len());
                self.ops.push((
                    PrimitiveId::SHA_256,
                    PrimitiveOpKind::Verify,
                    u32::try_from(coer).unwrap_or(u32::MAX),
                ));
                let v = self.check_ee_link(ee, &path);
                self.stats.ee_evictions += self.ees.insert(ee_id, v, ee);
                v
            }
        };
        verdict?;
        if self.hash_revokes(ee) {
            return Err(RxReject::RevokedByHash);
        }
        Ok(())
    }

    fn check_ee_link(&mut self, ee: &Certificate, path: &CaPath) -> Result<(), RxReject> {
        let mut seg: Vec<&Certificate> = vec![ee];
        seg.extend(path.certs.iter().map(|c| c.as_ref()));
        if let Err(errs) = chain::check_links(&seg) {
            let first = errs
                .iter()
                .find(|e| e.index == 0)
                .or(errs.first())
                .map_or(ChainErrorKind::Malformed, |e| e.kind);
            return Err(RxReject::Chain(first));
        }
        match ee.0.r_type {
            CertificateType::explicit => {
                self.ops
                    .extend(explicit_link_ops(ee, path.certs[0].as_ref()));
                match cert::verify_explicit_cert(ee, Some((&path.certs[0], &path.issuer_key))) {
                    Ok(true) => Ok(()),
                    _ => Err(RxReject::Chain(ChainErrorKind::BadSignature)),
                }
            }
            // An implicit certificate is authenticated by the SPDU signature verifying
            // under the key reconstructed from it (02 §6); the reconstruction is charged.
            _ => {
                self.ops.extend(ChainLink::Implicit.ops());
                Ok(())
            }
        }
    }

    /// Verifies a `SecuredCrl` and, when it is accepted, records it (overdue rule, hash
    /// revocations) and returns what the node installs. A CRL already installed returns
    /// `fresh: false` and costs nothing.
    ///
    /// # Errors
    /// Why the CRL was refused; the node installs nothing from it.
    pub fn verify_secured_crl(
        &mut self,
        cas: &BTreeMap<[u8; 8], Arc<Certificate>>,
        bytes: &[u8],
        now32: u32,
    ) -> Result<VerifiedCrl, RxReject> {
        let r = self.verify_crl_inner(cas, bytes, now32);
        match &r {
            Ok(v) if v.fresh => self.stats.crls_accepted += 1,
            Ok(_) => {}
            Err(e) => *self.stats.crls_refused.entry(e.name()).or_insert(0) += 1,
        }
        r
    }

    fn verify_crl_inner(
        &mut self,
        cas: &BTreeMap<[u8; 8], Arc<Certificate>>,
        bytes: &[u8],
        now32: u32,
    ) -> Result<VerifiedCrl, RxReject> {
        let digest = v2xw_core::hash::sha256(bytes);
        let opened = v2xw_sec::crl::decode_crl(bytes).map_err(|_| RxReject::CrlMalformed)?;
        let c = &opened.contents;
        let mut craca = [0u8; 8];
        craca.copy_from_slice(&c.crl_craca.0[..]);
        let series_no = c.crl_series.0.0;
        let out = |fresh: bool| {
            VerifiedCrl {
            craca,
            series: series_no,
            issue_date: c.issue_date.0.0,
            next_crl: c.next_crl.0.0,
            i_rev: match &c.type_specific {
                v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::TypeSpecificCrlContents::fullLinkedCrl(l)
                | v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::TypeSpecificCrlContents::deltaLinkedCrl(l) => {
                    Some(u32::from(l.i_rev.0.0))
                }
                _ => None,
            },
            linkage: v2xw_sec::crl::linkage_entries(c),
            hashes: v2xw_sec::crl::hash_entries(c).into_iter().map(|(h, _)| h).collect(),
            fresh,
        }
        };
        if self.seen_crls.contains(&digest) {
            return Ok(out(false));
        }
        let signer = match &opened.signed.signer {
            SpduSigner::Certificate(cert) => Arc::new(cert.clone()),
            SpduSigner::Digest(d) => Arc::clone(cas.get(d).ok_or(RxReject::CrlSignerUntrusted)?),
            SpduSigner::SelfSigned => return Err(RxReject::CrlSignerUntrusted),
        };
        // The signer chains to the installed root: its issuer's path, then its own link.
        let iss = issuer_id8(&signer).ok_or(RxReject::CrlSignerUntrusted)?;
        let path = self
            .ca_path(cas, iss)
            .map_err(|_| RxReject::CrlSignerUntrusted)?;
        let now = u64::from(now32);
        let mut seg: Vec<&Certificate> = vec![signer.as_ref()];
        seg.extend(path.certs.iter().map(|c| c.as_ref()));
        if chain::check_links(&seg).is_err()
            || seg.iter().any(|c| {
                let (s, e) = v2xw_sec::ctl::validity(c);
                now < s || now >= e
            })
        {
            return Err(RxReject::CrlSignerUntrusted);
        }
        self.ops.extend(explicit_link_ops(&signer, &path.certs[0]));
        match cert::verify_explicit_cert(&signer, Some((&path.certs[0], &path.issuer_key))) {
            Ok(true) => {}
            _ => return Err(RxReject::CrlSignerUntrusted),
        }
        // The CRL's own signature: 1609.2 §5.3.1, H(H(tbsData) ‖ H(signer)) and one P-256
        // verification (the tbsData is bounded by the SPDU's length).
        self.ops.extend(
            ChainLink::Explicit {
                signer: PrimitiveId::ECDSA_P256_SHA256,
                tbs_bytes: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
                issuer_bytes: u32::try_from(cert::encode(&signer).map_or(0, |b| b.len()))
                    .unwrap_or(u32::MAX),
            }
            .ops(),
        );
        let verdict = v2xw_sec::crl::verify_crl(
            &opened,
            &signer,
            Some(&path.certs[0]),
            WireEdition::default(),
        );
        if !verdict.accepted() {
            return Err(RxReject::CrlRejected);
        }
        let key = (craca, series_no);
        let order = crl_order(c);
        if let Some(m) = self.crls.get(&key)
            && order <= m.order
        {
            return Err(RxReject::CrlStale);
        }
        self.crls.insert(
            key,
            CrlMeta {
                next_crl: c.next_crl.0.0,
                order,
            },
        );
        self.seen_crls.insert(digest);
        let v = out(true);
        let mut c3 = [0u8; 3];
        c3.copy_from_slice(&craca[5..]);
        let set = self.hash_revoked.entry((c3, series_no)).or_default();
        set.extend(v.hashes.iter().copied());
        // The CRL changed: every cached validation it governs is run again on the next
        // SPDU (see the module documentation). A revoked CA is still refused at once by
        // the per-SPDU check of every path certificate against the hash revocations.
        self.stats.invalidated += self.ees.invalidate(c3, series_no);
        let before = self.paths.len();
        self.paths.retain(|_, p| match p {
            Ok(p) => !p
                .certs
                .iter()
                .any(|c| craca3(c) == c3 && series(c) == series_no),
            Err(_) => true,
        });
        self.stats.invalidated += (before - self.paths.len()) as u64;
        Ok(v)
    }
}
