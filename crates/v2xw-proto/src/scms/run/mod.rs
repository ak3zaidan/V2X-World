//! The CAMP SCMS deployment: nine online entities, seven flows, one event kernel.
//!
//! Each backend role is a node with a service queue and links, never a function call,
//! because the question the simulator exists to answer is what provisioning and revocation
//! *cost* — in queueing delay, in round trips and in bytes. An investigation here is six
//! messages across four organisations, and its latency is the sum of their service times
//! and their link delays, exactly as 06-node-models §4 requires.
//!
//! The cryptography is the real thing from `v2xw-sec`: butterfly expansion, ECQV-shaped
//! key derivation and linkage values, with the arithmetic identity
//! `b'(i,j) = a + f₁(ck,(i,j)) + c` checked on the device at install time.

use std::collections::{BTreeMap, BTreeSet};

use v2xw_core::hash::sha256;
use v2xw_core::ids::NodeId;
use v2xw_core::rng::{EntityRef, RngDomain, RngRegistry};
use v2xw_core::time::{Duration, SimTime};
use v2xw_sec::butterfly::{self, Caterpillar};
use v2xw_sec::ec::{self, Point};
use v2xw_sec::linkage::{self, CrlLinkageEntry, LaId, LinkageSeed, LinkageValue, PreLinkageValue};
use v2xw_sec::primitive::{PrimitiveId, PrimitiveOpKind};

use crate::error::{ProtoError, Result};
use crate::fault::{Fault, FaultPlan};
use crate::kernel::{Delivery, Kernel, Outbox};
use crate::net::{BackendNet, Link, Transport};
use crate::pseudonym::{CertEvent, PseudonymStore, PseudonymStrategy};
use crate::scms::governance::{DeviceTrust, Governance, Policy};
use crate::scms::msg::{
    CertRequestItem, EnrolmentCert, IssuedCredential, LaIndex, Lci, PcaLookup, PreLinkageBatch,
    ProvisioningRequest, Refusal, ReportSubmission, ScmsMsg, ScmsSizes, SealedForLa, SealedForPca,
};
use crate::scms::params::{MaMode, ScmsNodes, ScmsParams};
use crate::sizes::{CertificateSizes, WireSize};
use crate::spec::{AES128_BLOCK, OpDescriptor};
use crate::stage::{FlowId, FlowRun, StageId};

pub mod appcert;
mod crl;
mod enrol;
mod gov;
mod provision;
mod report;
mod revoke;

use crate::threshold::session::{CommitteeMsg, Committees, Outcome, RoleSpec};
pub use crl::{crl_bytes, crl_digest, linkage_revocations};

const ECDSA: PrimitiveId = PrimitiveId::ECDSA_P256_SHA256;
const ECQV: PrimitiveId = PrimitiveId::ECQV_P256;
const SHA256: PrimitiveId = PrimitiveId::SHA_256;

/// What one device holds.
#[derive(Debug, Clone)]
pub struct DeviceState {
    /// Its node id.
    pub node: NodeId,
    /// Whether it has an enrolment certificate.
    pub enrolled: bool,
    /// Its butterfly caterpillar, once it has made a provisioning request.
    pub caterpillar: Option<Caterpillar>,
    /// The credentials it holds, by `(i, j)`.
    pub credentials: BTreeMap<(u32, u32), IssuedCredential>,
    /// The CRL entries it has processed.
    pub crl: Vec<CrlLinkageEntry>,
    /// Whether it has found itself on the CRL and stopped transmitting
    /// ([CAMP-EE §2.2.10.2 step 8.4]).
    pub silenced: bool,
    /// How many times it has polled for a batch that was not ready.
    pub download_retries: u32,
    /// Which of its pseudonyms is active, and the rule that changes it.
    pub store: PseudonymStore,
    /// Its enrolment certificate, once the ECA has issued one.
    pub enrolment: Option<EnrolmentCert>,
    /// Its trust store: anchors, trust list, chain and policy.
    pub trust: DeviceTrust,
    /// The last refusal the RA or the ECA answered it with.
    pub refused: Option<Refusal>,
    /// Why the RA refused its current provisioning or top-up request, if it did. Kept
    /// apart from the ECA's answer: a successor enrolment refused while a top-up is in
    /// flight says nothing about the top-up.
    pub topup_refused: Option<Refusal>,
    /// Why the ECA refused its current successor-enrolment request, if it did.
    pub reenrol_refused: Option<Refusal>,
    /// How many refusals it has received.
    pub refusals: u32,
    /// How many successor enrolment certificates it has installed.
    pub reenrolments: u32,
    /// CRL versions it refused because the signature did not verify.
    pub crls_rejected: u32,
    /// Its real enrolment credential (WP-B1 PDU layer): the implicit certificate the ECA
    /// issued and the key reconstructed from `privateKeyInfo`. `None` when the layer is
    /// off or the device was enrolled out of band ([`ScmsRun::set_enrolment`]).
    pub enrolment_credential: Option<crate::scms::pdu::EnrolmentCredential>,
    /// The HashedId8 of its last `EeRaCertRequestSpdu` as sent: the 1609.2.1
    /// `requestHash` its batch and `.info` file names are formed from.
    pub request_hash8: Option<[u8; 8]>,
    /// Its enrolment certificate's `id.name`, unique at the ECA ([SMP] §6.3.1).
    pub name: Option<String>,
    /// The RA's URL, which the bootstrap installs with the RA certificate (03 §5).
    pub ra_url: Option<String>,
    /// The successor enrolment certificate, held until the current one ends: the
    /// successor's validity starts when the current certificate's ends ([SM-REENR]).
    pub successor: Option<EnrolmentCert>,
    /// The successor's real credential (WP-B2), promoted with [`Self::successor`].
    pub successor_credential: Option<crate::scms::pdu::EnrolmentCredential>,
    /// Download polls the RA answered "not ready" for the successor in flight.
    pub successor_polls: u32,
    /// The canonical private key, made in the secure environment at manufacture
    /// (direct authorisation, [SMP] §2.1.7); it signs the first enrolment request.
    canonical: Option<p256::Scalar>,
    /// The ECQV request key `k_U` and the bytes of the enrolment request in flight.
    enrol_request: Option<(p256::Scalar, crate::scms::pdu::Pdu)>,
    /// The successor request in flight: its new key `k_U`, the inner request as sent (the
    /// ECA's `requestHash` names it), and the outer SPDU's HashedId8 (the RA's).
    successor_request: Option<(p256::Scalar, crate::scms::pdu::Pdu, [u8; 8])>,
    /// The flow run of the successor request in flight: an ack or a download timer of an
    /// earlier, superseded request is ignored.
    successor_run: Option<FlowRun>,
    /// Whether an enrolment request is in flight.
    enrolling: bool,
    /// A provisioning request held until enrolment completes.
    deferred: Option<(Box<ProvisioningRequest>, FlowId, FlowRun)>,
    /// The run of the provisioning flow in progress.
    provisioning: Option<ProvisioningProgress>,
    /// The request hash of the one butterfly request the RA acknowledged: from then on
    /// the device tops up by `.info` and batch download, never by a new request
    /// (`ra-maxReqs` = 1, [SMP] Table 3; [CAMP122] SCMS-520).
    pub ra_account: Option<[u8; 32]>,
    /// `.info` requests the device sent (top-offs).
    pub info_requests: u32,
    /// Top-offs the RA answered "never" (a period before the request's `firstI`).
    pub topoffs_unavailable: u32,
}

#[derive(Debug, Clone, Copy)]
struct ProvisioningProgress {
    start_i: u32,
    periods: u32,
    downloaded: u32,
    first_batch_seen: bool,
}

impl DeviceState {
    /// A device that has done nothing yet.
    pub fn new(node: NodeId) -> DeviceState {
        DeviceState {
            node,
            enrolled: false,
            caterpillar: None,
            credentials: BTreeMap::new(),
            crl: Vec::new(),
            silenced: false,
            download_retries: 0,
            store: PseudonymStore::default(),
            enrolment: None,
            trust: DeviceTrust::default(),
            refused: None,
            topup_refused: None,
            reenrol_refused: None,
            refusals: 0,
            reenrolments: 0,
            crls_rejected: 0,
            enrolment_credential: None,
            request_hash8: None,
            name: None,
            ra_url: None,
            successor: None,
            successor_credential: None,
            successor_polls: 0,
            canonical: None,
            enrol_request: None,
            successor_request: None,
            successor_run: None,
            enrolling: false,
            deferred: None,
            provisioning: None,
            ra_account: None,
            info_requests: 0,
            topoffs_unavailable: 0,
        }
    }

    /// The enrolment certificate in force at `at`. A successor whose validity has begun
    /// replaces the current certificate here, with its credential: there is no overlap,
    /// so the switch is exactly at the current certificate's end.
    pub fn roll_enrolment(&mut self, at: SimTime) -> Option<EnrolmentCert> {
        if let Some(next) = self.successor
            && at >= next.valid_from
        {
            self.enrolment = Some(next);
            self.successor = None;
            if let Some(c) = self.successor_credential.take() {
                self.enrolment_credential = Some(c);
            }
        }
        self.enrolment
    }

    /// The latest-ending enrolment certificate the device holds: the pending successor if
    /// there is one, else the current certificate.
    #[must_use]
    pub fn latest_enrolment(&self) -> Option<EnrolmentCert> {
        self.successor.or(self.enrolment)
    }

    /// The policy the device follows: its installed Local Policy File, if any.
    #[must_use]
    pub fn policy(&self) -> Option<Policy> {
        self.trust.policy()
    }

    /// The `(i, j)` pairs this device could sign with at `now`, in ascending order.
    ///
    /// Usable means three things at once: downloaded, inside its validity window, and not
    /// matched by the CRL this device has processed. All three are the *device's* own
    /// view — the CRL it holds, not the CRL that exists — which is invariant I-P5.
    pub fn usable_at(&self, params: &ScmsParams, now: SimTime) -> Vec<(u32, u32)> {
        self.credentials
            .keys()
            .copied()
            .filter(|&(i, j)| {
                let (from, until) = params.validity(i);
                now >= from && now < until && !self.is_revoked(i, j)
            })
            .collect()
    }

    /// The credential the device would sign with now, if it has one.
    pub fn active_credential(&self) -> Option<&IssuedCredential> {
        self.store.active().and_then(|k| self.credentials.get(&k))
    }

    /// Whether the credential for `(i, j)` is revoked by the CRL this device holds.
    ///
    /// Forward-only by construction: [`CrlLinkageEntry::matches`] refuses a period before
    /// the entry's own, which is the backward-privacy property `tests/backward_privacy.rs`
    /// exercises end to end.
    pub fn is_revoked(&self, i: u32, j: u32) -> bool {
        self.credentials
            .get(&(i, j))
            .is_some_and(|c| self.crl.iter().any(|e| e.matches(i, j, c.lv)))
    }

    /// Every credential this device holds that its CRL revokes.
    pub fn revoked_credentials(&self) -> Vec<(u32, u32)> {
        self.credentials
            .keys()
            .copied()
            .filter(|&(i, j)| self.is_revoked(i, j))
            .collect()
    }

    /// Checks that a credential's derived private key matches the certified public key.
    ///
    /// `b'(i,j) = a + f₁(ck,(i,j)) + c` — the identity the whole butterfly construction
    /// rests on, and the difference between "the device downloaded bytes" and "the device
    /// has a usable certificate".
    pub fn credential_is_usable(&self, i: u32, j: u32) -> bool {
        let (Some(cat), Some(cred)) = (self.caterpillar.as_ref(), self.credentials.get(&(i, j)))
        else {
            return false;
        };
        // The real implicit certificate: d = e·b_ij + r (02 §9.3), checked against the key
        // a verifier reconstructs, e·P_U + Q_ACA.
        let private = match &cred.implicit {
            Some(imp) => cat.signing_private_implicit(i, j, &imp.e, &imp.r),
            None => cat.signing_private(i, j, &cred.c),
        };
        butterfly::derived_key_matches(&private, &cred.certified_public)
    }
}

/// The Registration Authority's state.
///
/// **What is not here is the point.** No linkage value, no pre-linkage value, no seed and
/// no certified public key: the RA holds enrolment records, request hashes, chain
/// identifiers and sealed blobs it cannot open. `tests/privacy.rs` asserts that by walking
/// this struct.
#[derive(Debug, Default)]
pub struct RaState {
    /// Devices with an enrolment certificate.
    pub enrolled: BTreeSet<NodeId>,
    /// Blocklisted enrolment certificates (the passive half of revocation).
    pub blocklist: BTreeSet<NodeId>,
    /// Request hash → device, and the chains the LAs allocated.
    pub requests: BTreeMap<[u8; 32], RequestRecord>,
    /// Requests refused because the device is blocklisted.
    pub refused: u32,
    /// Requests refused because the enrolment certificate had expired or was unknown.
    pub refused_enrolment: u32,
    /// Enrolment certificates the EE operator registered, by device: the RA serves only
    /// these ([SMP] §2.1.8). The HashedId8s are empty when the PDU layer is off. A
    /// successor is registered when the ECA's answer passes through the RA, linking old
    /// and new ([SM-REENR]).
    pub registered: BTreeMap<NodeId, BTreeSet<[u8; 8]>>,
    /// Requests refused because their enrolment certificate was not registered.
    pub unregistered_refused: u32,
    /// Successor requests the RA verified and forwarded to the ECA.
    pub successor_requests: u32,
    /// Successor requests a newer request from the same device invalidated before
    /// download ([SM-REENR]).
    pub successors_superseded: u32,
    /// Successor certificates issued for a request that was then superseded: blocklisted,
    /// so a second copy of the device's identity cannot be used ([SM-REENR]).
    pub successors_blocklisted: BTreeSet<[u8; 8]>,
    /// The successor in flight per device.
    successors: BTreeMap<NodeId, SuccessorRecord>,
    /// Requests the RA clipped to its policy (more certificates per period, or more
    /// periods ahead, than the Local Policy File allows).
    pub clipped: u32,
    /// Local policy and chain files the RA served.
    pub files_served: u32,
    /// Provisioning requests the RA accepted.
    pub accepted: u32,
    /// Provisioning jobs waiting for the shuffle window.
    jobs: Vec<ProvisioningJob>,
    /// Reports waiting for the report shuffle window.
    reports: Vec<(FlowRun, ReportSubmission)>,
    /// Batches ready for download: device → i → the batch file.
    repos: BTreeMap<NodeId, BTreeMap<u32, RepoBatch>>,
    /// When the armed certificate-request shuffle closes at the latest (the RA's
    /// `nextDlTime` estimate).
    shuffle_deadline: SimTime,
    /// `AcaRaCertResponseSpdu`s whose request hash the RA does not know.
    pub unrouted_responses: u32,
    shuffle_armed: bool,
    report_shuffle_armed: bool,
    /// Which batch of each shuffle is filling, bumped at each release, so the window timer
    /// of a batch the count threshold already released releases nothing.
    shuffle_generation: u64,
    report_shuffle_generation: u64,
    /// The pre-generation accounts, one per device the RA accepted a request from: what
    /// the RA expands every later week from ([PRIMER] p.6).
    pub accounts: BTreeMap<NodeId, PregenAccount>,
    /// Second butterfly requests refused under `ra-maxReqs` = 1.
    pub max_reqs_refused: u32,
    /// `.info` requests answered.
    pub info_served: u32,
    /// i-periods generated by pre-generation (not by the request itself).
    pub pregen_periods: u32,
    /// `.info` answers naming a period the weekly job has not reached yet.
    pub pregen_deferred: u32,
    /// Weekly-job timers armed, as `(device, i)`.
    pregen_timers: BTreeSet<(NodeId, u32)>,
    /// RA→ACA request hash → the device request it came from: what the RA keeps so a
    /// PCA answer resolves, and the ACA never sees.
    pub handles: BTreeMap<[u8; 32], [u8; 32]>,
}

/// What the RA keeps from a device's one butterfly request to pre-generate every later
/// week ([SMP] Table 3: `ra-maxReqs` 1, `ra-maxPreloadTime` 2 weeks – 3 years,
/// `ra-maxGenDelay` 1 week; [PRIMER] p.6 "keeps 3 years generated and adds one week every
/// week"; 03 §6.4).
///
/// Generation is evaluated lazily against that schedule: a period the schedule has
/// already reached is expanded and certified when first asked for, without the age-based
/// request shuffle (CAMP 1.2.2 `max_cert_request_age` applies to initial requests only,
/// [CAMP122] Table 68), and a later one at the weekly job's instant. **Modelled**: the
/// backend hops of a period generated on demand are charged at that instant, not at the
/// (earlier) job instant.
#[derive(Debug, Clone)]
pub struct PregenAccount {
    /// The request as accepted (butterfly keys `A`, `H`, `ck`, `ek`; no SPDU).
    pub request: ProvisioningRequest,
    /// The RA's request hash (its record).
    pub request_hash: [u8; 32],
    /// The low 8 bytes of the request SPDU's hash: the download file names.
    pub request_hash8: Option<[u8; 8]>,
    /// `firstI` of the acknowledgement.
    pub first_i: u32,
    /// When the RA accepted the request.
    pub accepted_at: SimTime,
    /// The Local Policy File version in force then: a new request is accepted only once
    /// it changed ("again when parameters change", [CAMP122] SCMS-520).
    pub lpf_version: u32,
    /// The i-periods expanded so far (the request's own, then pre-generated ones).
    pub generated: BTreeSet<u32>,
}

#[derive(Debug, Clone)]
struct SuccessorRecord {
    /// The outer request SPDU's HashedId8 (the download file name).
    request_hash8: [u8; 8],
    /// The inner `EeEcaCertRequestSpdu`'s HashedId8: the ECA's `requestHash`.
    inner_hash8: Option<[u8; 8]>,
    /// The ECA's answer, once it has arrived.
    response: Option<(EnrolmentCert, Option<crate::scms::pdu::Pdu>)>,
}

impl RaState {
    /// Whether `device` has a registered enrolment certificate; with `id8`, whether that
    /// one is registered.
    #[must_use]
    pub fn is_registered(&self, device: NodeId, id8: Option<[u8; 8]>) -> bool {
        self.registered
            .get(&device)
            .is_some_and(|s| id8.is_none_or(|h| s.contains(&h)))
    }

    /// Provisioning requests waiting for their pre-linkage values or the shuffle.
    #[must_use]
    pub fn pending_jobs(&self) -> usize {
        self.jobs.len()
    }

    /// Reports waiting for the report shuffle.
    #[must_use]
    pub fn reports_waiting(&self) -> usize {
        self.reports.len()
    }

    /// Batch files held for download, over every device.
    #[must_use]
    pub fn batches_held(&self) -> usize {
        self.repos.values().map(BTreeMap::len).sum()
    }

    /// The batch file names the RA holds for `device` (`<requestHash>_<i>.zip`), and the
    /// entries `i_j` in each: what an observer of the RA's storage sees.
    #[must_use]
    pub fn batch_files(&self, device: NodeId) -> Vec<(String, Vec<String>)> {
        self.repos.get(&device).map_or_else(Vec::new, |r| {
            r.iter()
                .map(|(i, b)| {
                    (
                        b.name.clone(),
                        b.files
                            .iter()
                            .map(|(j, _)| crate::scms::pdu_prov::batch_entry_name(*i, *j))
                            .collect(),
                    )
                })
                .collect()
        })
    }
}

/// One batch file in the RA's per-device folder.
#[derive(Debug, Default, Clone)]
struct RepoBatch {
    /// `<requestHash, 16 hex>_<i hex>.zip` (empty on the modelled path).
    name: String,
    /// The modelled path's credentials.
    modelled: Vec<IssuedCredential>,
    /// The real path: `(j, AcaEeCertResponsePrivateSpdu)`, ciphertext the RA cannot open.
    files: Vec<(u32, crate::scms::pdu::Pdu)>,
}

/// What the RA records about one provisioning request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestRecord {
    /// The device that made it.
    pub device: NodeId,
    /// LA1's chain, once LA1 has allocated one.
    pub lci1: Option<Lci>,
    /// LA2's chain.
    pub lci2: Option<Lci>,
}

#[derive(Debug)]
struct ProvisioningJob {
    run: FlowRun,
    flow: FlowId,
    device: NodeId,
    request_hash: [u8; 32],
    start_i: u32,
    periods: u32,
    jmax: u32,
    cocoons: BTreeMap<(u32, u32), Point>,
    /// The cocoon encryption keys `J_ij` (`certEncKey`).
    cocoon_enc: BTreeMap<(u32, u32), Point>,
    plv: [BTreeMap<(u32, u32), SealedForPca<PreLinkageValue>>; 2],
    /// The LAs' `EncryptedIndividualPLV`s (the real path).
    enc_plv: [BTreeMap<
        (u32, u32),
        v2xw_msg::sec_types::ieee1609_dot2_dot1_aca_ra_interface::EncryptedIndividualPLV,
    >; 2],
    /// HashedId8 of each `RaAcaCertRequestSpdu` → `(i, j)`: the RA keeps `j`, which the
    /// ACA never sees, and routes the answer by this hash (02 §9.5).
    aca_requests: BTreeMap<[u8; 8], (u32, u32)>,
    /// HashedId8 of the device's `EeRaCertRequestSpdu`: the batch files' name prefix.
    request_hash8: Option<[u8; 8]>,
    lci: [Option<Lci>; 2],
    /// The LAs' per-value sealed chain identifiers, for the ACA.
    lci_sealed: [BTreeMap<(u32, u32), SealedForLa<Lci>>; 2],
    responses: u8,
    periods_certified: u32,
    sent: bool,
    batch_ready_stamped: bool,
    /// A pre-generated period of an accepted request: released to the ACA as soon as its
    /// pre-linkage values are in, not held for the initial-request shuffle.
    pregen: bool,
}

/// The Pseudonym Certificate Authority's state ([BRECHT Table II]).
#[derive(Debug, Default)]
pub struct PcaState {
    /// `(i, linkage value)` → what the PCA stored when it issued that certificate.
    pub issued: BTreeMap<(u32, [u8; 9]), PcaLookup>,
    /// How many certificates it has issued.
    pub issued_count: u64,
    /// Linkage-value lookups it answered for the Misbehaviour Authority.
    pub lookups: u32,
    certified_runs: BTreeSet<FlowRun>,
}

/// One Linkage Authority's state.
#[derive(Debug)]
pub struct LaState {
    /// Which LA this is.
    pub index: LaIndex,
    /// Its identifier, as it appears in a CRL entry.
    pub la_id: LaId,
    /// Chain → initial seed. The seed never leaves this map except as `ls(i)` for a
    /// revocation period ([BRECHT §VI-D]).
    pub chains: BTreeMap<Lci, LinkageSeed>,
    /// Chain → the device it belongs to. The LA knows this and nothing else about the
    /// device: not its keys, not its certificates, not its linkage values.
    pub owner: BTreeMap<Lci, NodeId>,
    /// Pre-linkage values it has computed and sealed for the PCA.
    pub plv_issued: u64,
    /// Seeds `ls(i)` it has released for a revocation.
    pub seeds_released: u32,
    next_lci: u64,
}

impl LaState {
    fn new(index: LaIndex, la_id: LaId) -> LaState {
        LaState {
            index,
            la_id,
            chains: BTreeMap::new(),
            owner: BTreeMap::new(),
            plv_issued: 0,
            seeds_released: 0,
            next_lci: 1,
        }
    }
}

/// The Misbehaviour Authority's state.
#[derive(Debug, Default)]
pub struct MaState {
    /// Reports it has received, in arrival order.
    pub reports: Vec<ReportSubmission>,
    /// Its case table, by resolution run: every investigation it has opened, open or
    /// closed (gap row 27: cases run concurrently, each message names its case by run).
    pub cases: BTreeMap<FlowRun, Case>,
    /// The most recently opened case.
    pub last_case: Option<FlowRun>,
    /// Devices it has concluded should be revoked, by request hash.
    pub decisions: Vec<[u8; 32]>,
    /// Revocation requests waiting for the operator (manual-review mode), by the run
    /// the request was given: `(report index, i_rev, jmax, two-report partner)`.
    pub awaiting_review: BTreeMap<FlowRun, (usize, u32, u32, Option<usize>)>,
    /// Real misbehaviour reports it opened (WP-B4), with what it learned.
    pub opened_reports: Vec<crate::scms::mbr::OpenedReport>,
}

impl MaState {
    /// The most recently opened case.
    #[must_use]
    pub fn case(&self) -> Option<&Case> {
        self.last_case.and_then(|r| self.cases.get(&r))
    }

    /// How many cases are open.
    #[must_use]
    pub fn open_cases(&self) -> usize {
        self.cases.values().filter(|c| !c.done).count()
    }

    /// The case whose CRL-issuance run is `crl_run`.
    pub fn case_by_crl_run(&mut self, crl_run: FlowRun) -> Option<&mut Case> {
        self.cases.values_mut().find(|c| c.crl_run == crl_run)
    }
}

/// One investigation.
#[derive(Debug, Clone)]
pub struct Case {
    /// The flow run of the resolution.
    pub run: FlowRun,
    /// The flow run reserved for the CRL issuance that follows it.
    pub crl_run: FlowRun,
    /// The two reports being correlated.
    pub subjects: [(u32, LinkageValue); 2],
    /// What the PCA answered, in order.
    pub lookups: Vec<PcaLookup>,
    /// What the two LAs answered.
    pub same: [Option<bool>; 2],
    /// The seeds the LAs released.
    pub seeds: [Option<(LaId, LinkageSeed)>; 2],
    /// The first revoked period.
    pub i_rev: u32,
    /// Certificates per period, for the CRL entry.
    pub jmax: u32,
    /// Whether the resolution succeeded.
    pub resolved: bool,
    /// A single-subject revocation ([BRECHT §VI-D]): the MA's decision names one
    /// pseudonym certificate, so there is no "same device?" question to put to the Linkage
    /// Authorities; the PCA lookup identifies the request, the RA blocklists it and both
    /// LAs must release their seeds before the CRL Generator can append an entry.
    pub direct: bool,
    /// Whether the case has finished, either by an appended CRL entry or by a refusal
    /// somewhere on the path (an unknown linkage value, an unknown request, an LA that
    /// said "different devices"). A driver serialising cases waits on this.
    pub done: bool,
}

/// The CRL Generator's and the CRL Store's state.
#[derive(Debug, Default)]
pub struct CrlState {
    /// The published entries.
    pub entries: Vec<CrlLinkageEntry>,
    /// Whether a cadence publication is already scheduled (CRL Generator only).
    pub publish_armed: bool,
    /// The CRL Generator's signature over these entries: its certificate digest and the
    /// ECDSA signature over [`crl_digest`].
    pub signature: Option<([u8; 8], [u8; 64])>,
    /// How many CRL versions this entity has signed or received.
    pub versions: u32,
    /// The current full series-1 `SecuredCrl` (WP-B4), when the PDU layer is on.
    pub secured: Option<crate::scms::pdu::Pdu>,
    /// The latest delta linkage CRL (CRL Generator only).
    pub delta: Option<crate::scms::pdu::Pdu>,
    /// Every CRL the Generator signed, in order (CRL Generator only).
    pub issued: Vec<crl::IssuedCrl>,
    /// CRLs signed per `iRev`, for `indexWithinI`.
    pub crls_at_i: BTreeMap<u16, u8>,
    /// Entries already covered by the previous publication (the delta's base).
    pub published_entries: usize,
    /// Certificates revoked by hash, by series (3: RSU application and OBU
    /// identification; 4: enrolment).
    pub hash_entries: BTreeMap<u16, Vec<v2xw_sec::crl::HashRevocation>>,
    /// `crlSerial` per hash series.
    pub hash_serial: BTreeMap<u16, u32>,
    /// The latest full hash CRL per series.
    pub hash_crls: BTreeMap<u16, crate::scms::pdu::Pdu>,
}

/// The Enrolment CA's state.
#[derive(Debug, Default)]
pub struct EcaState {
    /// Enrolment certificates it has issued, successors included.
    pub issued: u32,
    /// Successor certificates it has issued.
    pub successors: u32,
    /// Successor requests it refused.
    pub refused: u32,
    /// Enrolment certificates the RA told it are blocklisted.
    pub blocklist: BTreeSet<NodeId>,
    /// Canonical public keys the EE operators registered, by canonical identity (direct
    /// authorisation, [SMP] §2.1.7).
    pub canonical: BTreeMap<String, Point>,
    /// Every request key it has seen (compressed SEC1): a key used before with this
    /// operator is refused ([SMP] §6.3.1).
    pub seen_keys: BTreeSet<[u8; 33]>,
    /// The `id.name` of every enrolment certificate it issued, and whose it is: a name is
    /// unique ([SMP] §6.3.1), kept only by a successor.
    pub names: BTreeMap<String, NodeId>,
    /// Requests refused under the US enrolment profile (the reason is in
    /// `pdu_rejections`).
    pub profile_refused: u32,
}

/// The Device Configuration Manager's state.
#[derive(Debug, Default)]
pub struct DcmState {
    /// Devices it bootstrapped.
    pub bootstrapped: u32,
    /// Trust bundles it delivered.
    pub bundles: u32,
}

/// The Location Obscurer Proxy's state: what it relayed, and nothing about content.
#[derive(Debug, Default)]
pub struct LopState {
    /// Messages relayed device → RA.
    pub upstream: u64,
    /// Messages relayed RA → device.
    pub downstream: u64,
}

/// Everything the deployment knows.
pub struct ScmsState {
    /// Which node hosts which role.
    pub nodes: ScmsNodes,
    /// The protocol's parameters.
    pub params: ScmsParams,
    /// The size model.
    pub sizes: ScmsSizes,
    /// The Registration Authority.
    pub ra: RaState,
    /// The Pseudonym Certificate Authority.
    pub pca: PcaState,
    /// The two Linkage Authorities.
    pub la: [LaState; 2],
    /// The Misbehaviour Authority.
    pub ma: MaState,
    /// What the CRL Generator has assembled.
    pub crlg: CrlState,
    /// What the CRL Store holds.
    pub crl_store: CrlState,
    /// What the broadcast path has carried.
    pub crl_broadcast: CrlState,
    /// The devices.
    pub devices: BTreeMap<NodeId, DeviceState>,
    /// The governance entities: SCMS Manager, Policy Generator, Root CA, electors,
    /// Intermediate CA, and their artefacts.
    pub gov: Governance,
    /// The Enrolment CA.
    pub eca: EcaState,
    /// The Device Configuration Manager.
    pub dcm: DcmState,
    /// The Location Obscurer Proxy.
    pub lop: LopState,
    /// The deterministic streams.
    pub rng: RngRegistry,
    /// Which devices the Enrolment CA has issued a certificate to.
    ///
    /// The simulator's bookkeeping, not an entity's knowledge: the RA learns a device is
    /// enrolled by verifying the signature on its provisioning request, which is charged
    /// where that happens.
    pub issued_enrolment: BTreeSet<NodeId>,
    /// Backend servers that are down, slow or lying (`crate::fault`).
    pub faults: FaultPlan,
    /// The committees that replace single authorities, and their sessions
    /// (`crate::threshold::session`); empty for the CAMP deployment.
    pub committees: Committees<Pending>,
    /// Committee sessions that failed for good, by role: work that never happened.
    pub threshold_failures: BTreeMap<&'static str, u64>,
    /// The deployment PKI the real 1609.2.1 SPDUs are signed and encrypted under
    /// (`crate::scms::pdu`); `None` when the layer is off or under a hybrid or KEM
    /// scheme, which 1609.2.1 does not define.
    pub pki: Option<std::sync::Arc<crate::scms::pdu::ScmsPki>>,
    /// Every SPDU the run put on a link (bytes kept only with `params.pdu.keep_bytes`).
    pub pdus: Vec<crate::scms::pdu::EmittedPdu>,
    /// SPDUs a receiver refused, and why: forged, tampered or misaddressed traffic.
    pub pdu_rejections: Vec<crate::scms::pdu::RejectedPdu>,
    /// The layer's deterministic randomness (separate from the model streams).
    pdu_rng: crate::scms::pdu::Drbg,
    /// The keys each LA shares with the ACA (`None` with the layer off).
    pub la_aca: Option<crate::scms::pdu_prov::LaAcaKeys>,
    /// Simulated time 0 on the wall clock, for every `Time32` in a PDU.
    wall: v2xw_core::time::WallClock,
    next_run: u32,
    /// RSU application and OBU identification certificates (WP-B3b).
    pub apps: appcert::AppCertState,
}

/// What an authority does once its committee has signed: the continuation a committee
/// session hands back.
#[derive(Debug)]
pub enum Pending {
    /// The PCA sends the certified batch to the RA.
    IssueBatch {
        /// The i-period.
        i: u32,
        /// The certificates, computed before the committee signed.
        credentials: Box<Vec<IssuedCredential>>,
        /// The flow.
        flow: FlowId,
        /// Its run.
        run: FlowRun,
    },
    /// The CRL Generator publishes the list it has.
    PublishCrl {
        /// The flow.
        flow: FlowId,
        /// Its run.
        run: FlowRun,
    },
    /// The Misbehaviour Authority has reconstructed both chains' seeds.
    SeedsReleased {
        /// The case (its resolution run).
        case: FlowRun,
        /// LA 1's chain.
        lci1: Lci,
        /// LA 2's chain.
        lci2: Lci,
        /// The revocation period.
        i: u32,
    },
}

/// How many times the MA asks a silent Linkage Authority again before giving the case up:
/// with `ScmsParams::seed_retry` at 30 s, five minutes. A model parameter, uncited.
pub const SEED_RETRY_MAX: u32 = 10;

/// Committee member node ids start here, `COMMITTEE_STRIDE` apart per role.
pub const COMMITTEE_NODE_BASE: u32 = 200;
/// The id space each role's committee occupies.
pub const COMMITTEE_STRIDE: u32 = 64;

fn wrap_committee(m: CommitteeMsg) -> ScmsMsg {
    ScmsMsg::Committee(m)
}

/// A deployment and the kernel it runs on.
pub struct ScmsRun {
    /// The entities.
    pub state: ScmsState,
    /// The schedule, the queues and the logs.
    pub kernel: Kernel<ScmsMsg>,
}

impl ScmsRun {
    /// Builds the reference topology: every online role on its own node, connected to the
    /// entities it talks to.
    ///
    /// # Errors
    /// [`ProtoError::Size`] if the certificate profile does not encode.
    pub fn new(params: ScmsParams) -> Result<ScmsRun> {
        ScmsRun::new_at(params, 0)
    }

    /// The same deployment with its clock starting at `t0`.
    ///
    /// The engine's clock does not start at zero, and a provisioning flow that began at
    /// the scenario's `t0` must stamp its stages on the engine's timeline rather than on
    /// one of its own. `t0` is supplied by the caller; nothing here reads a clock.
    ///
    /// # Errors
    /// [`ProtoError::Size`] if the certificate profile does not encode.
    pub fn new_at(params: ScmsParams, t0: SimTime) -> Result<ScmsRun> {
        let nodes = ScmsNodes::default();
        let umbrella_leaves = params
            .threshold
            .filter(|t| t.pca && t.protocol == crate::threshold::umbrella::ID)
            .map_or(1, |t| t.protocol_params.umbrella_leaves);
        let sizes = ScmsSizes::new(CertificateSizes::measured()?, params.sizes)
            .with_hybrid(params.hybrid.as_ref())
            .with_kem(params.kem.as_ref())
            .with_umbrella(umbrella_leaves);
        let mut net = BackendNet::new();
        let backend = Link {
            latency: params.backend_link_latency,
            bandwidth_bps: params.backend_link_bandwidth_bps,
            transport: Transport::BackendNet,
        };
        for (a, b) in nodes.backend_links() {
            net.connect(a, b, backend);
        }
        let rng = RngRegistry::new(params.master_seed);
        // One clock for the whole deployment: the governance artefacts (CTL
        // `effectiveDate`, component validity) and every PDU's `Time32` count from the
        // same `t0` (`PduLayer::t0_unix_s`).
        let gov = Governance::ceremony_with(
            &params,
            &nodes,
            &rng,
            t0,
            crate::scms::governance::GovernanceConfig {
                clock: v2xw_core::time::WallClock::new(params.pdu.t0_unix_s),
                ..crate::scms::governance::GovernanceConfig::default()
            },
        );
        for (a, b) in governance_links(&nodes, &gov) {
            net.connect(a, b, backend);
        }
        let mut kernel = Kernel::new_at(net, t0);
        for (node, spec) in nodes.backend_service_models(&params) {
            kernel.host(node, &spec, params.backend_profile);
        }
        host_governance(&mut kernel, &gov, &params);
        let committees = build_committees(&params, &nodes, &mut kernel, sizes.envelope_digest());
        let wall = v2xw_core::time::WallClock::new(params.pdu.t0_unix_s);
        let pki = if params.pdu.enabled && params.hybrid.is_none() && params.kem.is_none() {
            // The Root CA and ICA are the governance entities' own (the root the
            // genesis CTL approves, WP-B5); the online components are certified under
            // that ICA from the start of its validity.
            let refuse = |detail: String| ProtoError::Payload {
                what: "SCMS PKI",
                detail,
            };
            let anchor = |role: &str| {
                gov.cert(role)
                    .zip(gov.signing_key(role))
                    .ok_or_else(|| refuse(format!("governance holds no {role}")))
            };
            let (root, ica) = (anchor("root")?, anchor("ica")?);
            let start = ica.0.0.to_be_signed.validity_period.start.0.0;
            Some(std::sync::Arc::new(
                crate::scms::pdu::ScmsPki::under_anchors(params.master_seed, start, root, ica)
                    .map_err(|e| refuse(e.to_string()))?,
            ))
        } else {
            None
        };
        Ok(ScmsRun {
            state: ScmsState {
                nodes,
                params,
                sizes,
                ra: RaState::default(),
                pca: PcaState::default(),
                la: [
                    LaState::new(LaIndex::One, LaId(1)),
                    LaState::new(LaIndex::Two, LaId(2)),
                ],
                ma: MaState::default(),
                crlg: CrlState::default(),
                crl_store: CrlState::default(),
                crl_broadcast: CrlState::default(),
                devices: BTreeMap::new(),
                gov,
                eca: EcaState::default(),
                dcm: DcmState::default(),
                lop: LopState::default(),
                rng,
                issued_enrolment: BTreeSet::new(),
                faults: FaultPlan::default(),
                committees,
                threshold_failures: BTreeMap::new(),
                pki,
                pdus: Vec::new(),
                pdu_rejections: Vec::new(),
                pdu_rng: crate::scms::pdu::Drbg::new(params.master_seed, b"run"),
                la_aca: params.pdu.enabled.then(|| {
                    crate::scms::pdu_prov::LaAcaKeys::new(
                        params.master_seed,
                        params.pdu.la_aca_key_rotation_s,
                    )
                }),
                wall,
                next_run: 0,
                apps: appcert::AppCertState::new(params.master_seed),
            },
            kernel,
        })
    }

    /// Declares which backend servers are down, slow or lying, and when.
    pub fn set_faults(&mut self, faults: Vec<Fault>) {
        self.state.faults.faults = faults;
    }

    /// The committee member nodes of `role` (`pca`, `crl`, `linkage`), in index order.
    pub fn committee_members(&self, role: &str) -> Vec<NodeId> {
        self.state
            .committees
            .role(role)
            .map(|r| self.state.committees.roles[r].members.clone())
            .unwrap_or_default()
    }

    /// Adds a device, with its uplink to the privacy proxy and to the CRL store.
    pub fn add_device(&mut self, node: NodeId) {
        let p = &self.state.params;
        let uu = Link {
            latency: p.uu_link_latency,
            bandwidth_bps: p.uu_link_bandwidth_bps,
            transport: Transport::CellularUu,
        };
        let n = self.state.nodes;
        {
            let net = self.kernel.net_mut();
            for peer in [n.lop, n.ra, n.dcm, n.eca, n.crl_store] {
                net.connect(node, peer, uu);
            }
        }
        self.kernel.host(
            node,
            &crate::service::ServiceModelSpec::new(1, p.device_overhead),
            p.device_profile,
        );
        self.state.devices.insert(node, DeviceState::new(node));
    }

    /// Installs the driver's per-message access model on this deployment's topology: every
    /// message on a device's link is then carried by it ([`crate::net::PathOracle`]).
    ///
    /// The link `device` reaches the backend over (privacy proxy, RA, DCM, ECA, CRL Store)
    /// is set with [`ScmsRun::set_access`].
    ///
    /// The deployment's default device link is a fixed cellular uplink; a driver that
    /// models the access leg itself — the cellular Uu model's latency and capacity at the
    /// device's position, or a roadside unit's relay over its backhaul — states the link
    /// the next exchange will see here, so the flow's round trips cost what that access
    /// really costs and are charged to the transport it really used.
    pub fn set_path_oracle(&mut self, oracle: crate::net::SharedOracle) {
        self.kernel.net_mut().set_oracle(oracle);
    }

    /// Replaces the link `device` reaches the backend over with `link` (see above).
    pub fn set_access(&mut self, device: NodeId, link: Link) {
        let n = self.state.nodes;
        let net = self.kernel.net_mut();
        for peer in [n.lop, n.ra, n.dcm, n.eca, n.crl_store] {
            net.connect(device, peer, link);
        }
    }

    /// Puts `device` in range of the roadside CRL broadcast path.
    ///
    /// The second distribution path of 05-protocols §3.2: the same signed CRL, over the
    /// 5.9 GHz air interface instead of the cellular uplink. The two differ in exactly one
    /// modelled thing — the link — which is what makes "how long until an RSU-only vehicle
    /// enforces it" a different number from "how long until a connected one does".
    ///
    /// The air interface itself belongs to `v2xw-radio`: this is a point-to-point link
    /// with the OFDM rate as its bandwidth, so a 400 kB CRL takes the 533 ms it takes at
    /// 6 Mbit/s. Contention, fragmentation and the loss process are the radio crate's, and
    /// a scenario that needs them drives the broadcast through the engine's PHY instead.
    pub fn attach_rsu(&mut self, device: NodeId) {
        let p = &self.state.params;
        let air = Link {
            latency: p.v2x_air_latency,
            bandwidth_bps: p.v2x_air_bandwidth_bps,
            transport: Transport::V2xAir,
        };
        let broadcast = self.state.nodes.crl_broadcast;
        self.kernel.net_mut().connect(device, broadcast, air);
    }

    /// Sets the pseudonym-change rule `device` follows.
    pub fn set_strategy(&mut self, device: NodeId, strategy: PseudonymStrategy) {
        if let Some(d) = self.state.devices.get_mut(&device) {
            d.store.set_strategy(strategy);
        }
    }

    /// Adds travel since the last pseudonym change, for the `distance` rule.
    pub fn travelled_cm(&mut self, device: NodeId, cm: u64) {
        if let Some(d) = self.state.devices.get_mut(&device) {
            d.store.travelled_cm(cm);
        }
    }

    /// Tells `device` it has left a mix zone, for the `mix-zone` rule.
    pub fn left_mix_zone(&mut self, device: NodeId) {
        if let Some(d) = self.state.devices.get_mut(&device) {
            d.store.left_mix_zone();
        }
    }

    /// Rotates `device`'s pseudonym if its rule says one is due at `now`.
    ///
    /// Returns the `sec.cert` record for the change, or `None` if none happened. The
    /// device's own store is what swaps: [`DeviceState::active_credential`] returns a
    /// different credential afterwards, with a different linkage value, which is the only
    /// identifier a receiver of its next message would see.
    pub fn rotate(&mut self, device: NodeId, now: SimTime) -> Option<CertEvent> {
        let params = self.state.params;
        let dev = self.state.devices.get_mut(&device)?;
        let usable = dev.usable_at(&params, now);
        let active_revoked = dev
            .store
            .active()
            .is_some_and(|(i, j)| dev.is_revoked(i, j));
        let (reason, next) = dev.store.rotate(now, &usable, active_revoked)?;
        let (i, j) = next.unwrap_or((0, 0));
        let lv = next
            .and_then(|k| dev.credentials.get(&k))
            .map_or([0u8; 9], |c| *c.lv.as_bytes());
        Some(CertEvent {
            t: now,
            node: device,
            event: "change",
            reason: Some(reason),
            i_period: i,
            j_index: j,
            linkage_value: lv,
            changes: dev.store.changes(),
        })
    }

    fn new_run(&mut self) -> FlowRun {
        self.state.next_run += 1;
        FlowRun(self.state.next_run)
    }

    /// Why the RA or the ECA last refused `device`, if either did since its last request.
    #[must_use]
    pub fn refusal_of(&self, device: NodeId) -> Option<Refusal> {
        self.state.devices.get(&device).and_then(|d| d.refused)
    }

    /// Why the RA refused `device`'s current provisioning or top-up request, if it did.
    #[must_use]
    pub fn topup_refusal_of(&self, device: NodeId) -> Option<Refusal> {
        self.state
            .devices
            .get(&device)
            .and_then(|d| d.topup_refused)
    }

    /// Why the ECA refused `device`'s current successor-enrolment request, if it did.
    #[must_use]
    pub fn reenrol_refusal_of(&self, device: NodeId) -> Option<Refusal> {
        self.state
            .devices
            .get(&device)
            .and_then(|d| d.reenrol_refused)
    }

    fn inject(&mut self, to: NodeId, msg: ScmsMsg, flow: FlowId, run: FlowRun) {
        let at = self.kernel.now();
        self.inject_at(at, to, msg, flow, run);
    }

    /// Schedules a flow's first message at `at`, never earlier than the kernel's clock.
    ///
    /// A device that spawns 1.5 s into a run asks for credentials at 1.5 s. Clamping to
    /// the clock matters: the heap is ordered by `(time, sequence)`, and an injection in
    /// the past would be dispatched before deliveries already in flight, which is a
    /// causality violation rather than an early message.
    fn inject_at(&mut self, at: SimTime, to: NodeId, msg: ScmsMsg, flow: FlowId, run: FlowRun) {
        self.kernel.inject_at(
            at,
            Delivery {
                at,
                from: to,
                to,
                msg,
                flow,
                run,
            },
        );
    }

    fn drain_into(state: &mut ScmsState, kernel: &mut Kernel<ScmsMsg>) -> Result<()> {
        while let Some(d) = kernel.next_delivery() {
            Self::step(state, kernel, d)?;
        }
        Ok(())
    }

    /// Handles one delivery: dropped if its receiver is offline, otherwise handled and
    /// dispatched, with a slow receiver's extra time charged.
    fn step(
        state: &mut ScmsState,
        kernel: &mut Kernel<ScmsMsg>,
        d: Delivery<ScmsMsg>,
    ) -> Result<()> {
        if state.faults.offline(d.to, d.at) {
            state.faults.dropped += 1;
            return Ok(());
        }
        let profile = kernel.profile_of(d.to);
        let mut out = state.outbox(profile, d.to, d.at);
        let (at, to) = (d.at, d.to);
        state.handle(d, &mut out)?;
        kernel.dispatch(at, to, out)?;
        Ok(())
    }

    /// Runs every delivery due at or before `horizon`, then stops.
    ///
    /// How the engine drives the deployment: it advances its own clock to `t`, calls this,
    /// and the backend does exactly the work that was due by then. What is still in flight
    /// stays in flight, which is the difference between a provisioning round trip that
    /// costs simulated time and one that completes inside a single engine step.
    ///
    /// # Errors
    /// Whatever [`Kernel::dispatch`] or a handler returns.
    pub fn run_until(&mut self, horizon: SimTime) -> Result<()> {
        let ScmsRun { state, kernel } = self;
        while let Some(d) = kernel.next_delivery_before(horizon) {
            Self::step(state, kernel, d)?;
        }
        Ok(())
    }

    /// Runs until nothing is scheduled.
    ///
    /// # Errors
    /// Whatever [`Kernel::dispatch`] or a handler returns: an unhosted node, a missing
    /// link or a flow driven out of order — all modelling defects.
    pub fn run(&mut self) -> Result<()> {
        let ScmsRun { state, kernel } = self;
        while let Some(d) = kernel.next_delivery() {
            Self::step(state, kernel, d)?;
        }
        Ok(())
    }
}

impl ScmsState {
    /// `Time32` (TAI seconds since the 1609.2 epoch) at simulated time `at`.
    /// The `Time32` of simulated instant `at` (the run's wall clock).
    pub fn time32(&self, at: SimTime) -> u32 {
        self.wall.time32(at).unwrap_or(0)
    }

    /// Records an SPDU put on a link and returns its charged size: the encoded length
    /// (invariant I-S3 as equality).
    fn emit_pdu(
        &mut self,
        kind: crate::scms::pdu::SpduKind,
        from: NodeId,
        to: NodeId,
        at: SimTime,
        pdu: &crate::scms::pdu::Pdu,
    ) -> WireSize {
        self.pdus.push(crate::scms::pdu::EmittedPdu {
            kind,
            from: from.index(),
            to: to.index(),
            at,
            bytes: self.params.pdu.keep_bytes.then(|| pdu.clone()),
            len: pdu.len(),
        });
        WireSize::real(pdu.len(), kind.name())
    }

    /// Records an SPDU its receiver refused.
    fn reject_pdu(
        &mut self,
        kind: crate::scms::pdu::SpduKind,
        node: NodeId,
        at: SimTime,
        reason: crate::scms::pdu::PduError,
    ) {
        self.pdu_rejections.push(crate::scms::pdu::RejectedPdu {
            kind,
            at_node: node.index(),
            at,
            reason,
        });
    }

    /// An empty outbox for an entity hosted on `profile`. Under a hybrid scheme every
    /// signature is two, and the post-quantum half is charged against the device's or the
    /// authority's published figure (`crate::hybrid`).
    fn outbox(&self, profile: &'static str, to: NodeId, at: SimTime) -> Outbox<ScmsMsg> {
        let device = self.devices.contains_key(&to);
        let pq = self.params.hybrid.map(|h| h.charge(device));
        let kem = self.params.kem.map(|k| k.charge(device));
        let mut out = Outbox::new(profile).with_pq(pq).with_kem(kem);
        // A slow backend server pays its extra time on everything it handles
        // (`crate::fault`).
        let extra = self.faults.slow_extra(to, at);
        if extra > Duration::ZERO {
            out.charge_time("fault/slow", extra);
        }
        out
    }

    /// What an authority does once its committee has signed.
    fn resume(&mut self, cont: Pending, _at: SimTime, out: &mut Outbox<ScmsMsg>) -> Result<()> {
        let n = self.nodes;
        match cont {
            Pending::IssueBatch {
                i,
                credentials,
                flow,
                run,
            } => self.send_batch(i, *credentials, flow, run, _at, out),
            Pending::PublishCrl { flow, run } => {
                // The simulation's real ECDSA signature stands in for the committee's (a
                // CGGMP21 output *is* one; a FROST one is a Schnorr signature the size
                // model carries).
                self.publish_crl(_at, out, flow, run);
            }
            Pending::SeedsReleased {
                case: case_run,
                lci1,
                lci2,
                i,
            } => {
                // The MA holds a quorum of verified shares of each chain's `ls(i)` and
                // interpolates them. The value it obtains is the one the dealer shared:
                // the LA's own `ls(i)`, computed here from the chain it dealt.
                let mut seeds = [None, None];
                for (k, (la, lci)) in [(LaIndex::One, lci1), (LaIndex::Two, lci2)]
                    .into_iter()
                    .enumerate()
                {
                    let st = &self.la[la.idx()];
                    if let Some(ls0) = st.chains.get(&lci).copied() {
                        seeds[k] = Some((st.la_id, linkage::linkage_seed_at(st.la_id, ls0, i)));
                    }
                }
                let (Some(la1), Some(la2)) = (seeds[0], seeds[1]) else {
                    if let Some(case) = self.ma.cases.get_mut(&case_run) {
                        case.done = true;
                    }
                    return Ok(());
                };
                let Some(case) = self.ma.cases.get_mut(&case_run) else {
                    return Ok(());
                };
                case.seeds = [Some(la1), Some(la2)];
                let (jmax, crl_run) = (case.jmax, case.crl_run);
                out.send(
                    n.crlg,
                    ScmsMsg::CrlAppend { i, la1, la2, jmax },
                    "crl-append",
                    self.sizes.crl_append(),
                    Transport::BackendNet,
                    FlowId::CrlIssuance,
                    crl_run,
                );
            }
        }
        Ok(())
    }

    /// `n` signatures: ECDSA unless the scheme is pure post-quantum, and the
    /// post-quantum one under a hybrid or pure scheme.
    fn sign(out: &mut Outbox<ScmsMsg>, n: u32) {
        if out.classical_signs() {
            out.compute(OpDescriptor::new(ECDSA, PrimitiveOpKind::Sign, n));
        }
        out.charge_pq(PrimitiveOpKind::Sign, n);
    }

    /// `n` verifications, both halves under a hybrid scheme.
    fn verify(out: &mut Outbox<ScmsMsg>, n: u32) {
        if out.classical_signs() {
            out.compute(OpDescriptor::new(ECDSA, PrimitiveOpKind::Verify, n));
        }
        out.charge_pq(PrimitiveOpKind::Verify, n);
    }

    /// `n` decryptions of an encrypted-key structure: one ECIES scalar multiplication
    /// unless the KEM is pure post-quantum, plus an ML-KEM decapsulation under a
    /// post-quantum or hybrid `security.kem`.
    fn decrypt(out: &mut Outbox<ScmsMsg>, n: u32) {
        if out.ecies() {
            Self::scalar_mults(out, n);
        }
        out.charge_kem(PrimitiveOpKind::Verify, n);
    }

    /// `n` encryptions to a recipient: the ECIES scalar multiplication and/or an ML-KEM
    /// encapsulation, as [`ScmsState::decrypt`].
    fn encrypt(out: &mut Outbox<ScmsMsg>, n: u32) {
        if out.ecies() {
            Self::scalar_mults(out, n);
        }
        out.charge_kem(PrimitiveOpKind::Sign, n);
    }

    /// One elliptic-curve scalar multiplication, charged against the ECQV descriptor,
    /// whose declared cost proxy in `v2xw-sec` is the ECDSA P-256 anchor precisely because
    /// a reconstruction is "one point multiplication plus one addition".
    fn scalar_mults(out: &mut Outbox<ScmsMsg>, n: u32) {
        out.compute(OpDescriptor::new(ECQV, PrimitiveOpKind::Sign, n));
    }

    /// AES-128 block operations. No profile in 04-models §9.4 publishes an AES anchor, so
    /// these are **counted and charged zero time** rather than given an invented rate; the
    /// count is what the linkage-expansion cost metric reports.
    fn aes(out: &mut Outbox<ScmsMsg>, n: u32) {
        out.compute(OpDescriptor::new(AES128_BLOCK, PrimitiveOpKind::Sign, n));
    }

    /// SHA-256 compressions, counted for the same reason and with the same caveat.
    fn hashes(out: &mut Outbox<ScmsMsg>, n: u32) {
        out.compute(OpDescriptor::new(SHA256, PrimitiveOpKind::Verify, n));
    }

    #[allow(clippy::too_many_lines)]
    fn handle(&mut self, d: Delivery<ScmsMsg>, out: &mut Outbox<ScmsMsg>) -> Result<()> {
        match &d.msg {
            ScmsMsg::EnrolRequest { .. }
            | ScmsMsg::TrustBundle { .. }
            | ScmsMsg::EnrolForward { .. }
            | ScmsMsg::EnrolResponse { .. }
            | ScmsMsg::SuccessorEnrolRequest { .. }
            | ScmsMsg::SuccessorEnrolForward { .. }
            | ScmsMsg::SuccessorEnrolAck { .. }
            | ScmsMsg::SuccessorEnrolDownload { .. }
            | ScmsMsg::SuccessorEnrolResponse { .. }
            | ScmsMsg::EnrolmentRegistration { .. }
            | ScmsMsg::EnrolRefused { .. } => self.handle_enrol(d, out),
            ScmsMsg::ProvisioningRequest(..)
            | ScmsMsg::ProvisioningAck { .. }
            | ScmsMsg::PreLinkageRequest { .. }
            | ScmsMsg::PreLinkageResponse(..)
            | ScmsMsg::ShuffleTimer { .. }
            | ScmsMsg::CertRequest { .. }
            | ScmsMsg::CertResponse { .. }
            | ScmsMsg::BatchDownloadRequest { .. }
            | ScmsMsg::BatchDownload { .. }
            | ScmsMsg::InfoRequest { .. }
            | ScmsMsg::InfoResponse { .. }
            | ScmsMsg::PregenTimer { .. }
            | ScmsMsg::ProvisioningRefused { .. } => self.handle_provision(d, out),
            ScmsMsg::Report(..) | ScmsMsg::ReportShuffleTimer { .. } => self.handle_report(d, out),
            ScmsMsg::PcaLookupRequest { .. }
            | ScmsMsg::PcaLookupResponse { .. }
            | ScmsMsg::SameDeviceRequest { .. }
            | ScmsMsg::SameDeviceResponse { .. }
            | ScmsMsg::BlocklistRequest { .. }
            | ScmsMsg::BlocklistResponse { .. }
            | ScmsMsg::SeedRequest { .. }
            | ScmsMsg::SeedResponse { .. }
            | ScmsMsg::SeedRetry { .. }
            | ScmsMsg::BlocklistNotice { .. } => self.handle_revoke(d, out),
            ScmsMsg::CrlAppend { .. }
            | ScmsMsg::CrlPublishTimer
            | ScmsMsg::CrlPublish { .. }
            | ScmsMsg::CrlDownloadRequest { .. }
            | ScmsMsg::CrlAirBroadcast { .. }
            | ScmsMsg::CrlDownload { .. } => self.handle_crl(d, out),
            ScmsMsg::FileRequest { .. }
            | ScmsMsg::FileResponse { .. }
            | ScmsMsg::PolicyUpdate(..) => self.handle_gov(d, out),
            ScmsMsg::Committee(_) => self.handle_committee(d, out),
            ScmsMsg::AppCert(_) => self.handle_appcert(d, out),
        }
    }

    /// Threshold committee traffic: the committee machine, then the authority's continuation.
    fn handle_committee(&mut self, d: Delivery<ScmsMsg>, out: &mut Outbox<ScmsMsg>) -> Result<()> {
        let (to, at) = (d.to, d.at);
        match d.msg {
            ScmsMsg::Committee(m) => {
                let outcome = self
                    .committees
                    .handle(m, at, to, &self.faults, out, wrap_committee);
                match outcome {
                    Some(Outcome::Done { cont, .. }) => self.resume(cont, at, out)?,
                    Some(Outcome::Failed { cont }) => {
                        let role = match cont {
                            Pending::IssueBatch { .. } => "pca",
                            Pending::PublishCrl { .. } => "crl",
                            Pending::SeedsReleased { .. } => "linkage",
                        };
                        *self.threshold_failures.entry(role).or_insert(0) += 1;
                    }
                    None => {}
                }
            }
            _ => unreachable!("ScmsState::handle routes only committee messages here"),
        }
        Ok(())
    }
}

/// A device's own view, for an inspector.
pub fn device_summary(dev: &DeviceState) -> BTreeMap<&'static str, u64> {
    let mut m = BTreeMap::new();
    m.insert("credentials", dev.credentials.len() as u64);
    m.insert("crl_entries", dev.crl.len() as u64);
    m.insert("revoked", dev.revoked_credentials().len() as u64);
    m.insert("silenced", u64::from(dev.silenced));
    m
}

/// Time helper: the deployment's idea of "later", for tests that step the clock.
pub const fn after(t: SimTime, d: Duration) -> SimTime {
    d.after(t)
}

/// The linkage value a device's credential `(i, j)` carries, for a test that needs to
/// report on it.
pub fn credential_lv(dev: &DeviceState, i: u32, j: u32) -> Option<LinkageValue> {
    dev.credentials.get(&(i, j)).map(|c| c.lv)
}

/// The links the governance entities need: the SCMS Manager to the Policy Generator, the
/// Policy Generator to the Registration Authority it publishes to.
/// The committees a deployment's `threshold` field asks for, their member nodes hosted on
/// `kernel` and linked to their coordinator over the inter-organisation WAN.
fn build_committees(
    params: &ScmsParams,
    nodes: &ScmsNodes,
    kernel: &mut Kernel<ScmsMsg>,
    envelope: u32,
) -> Committees<Pending> {
    let Some(t) = params.threshold else {
        return Committees::default();
    };
    let mut roles = Vec::new();
    let mut add = |role: &'static str,
                   slot: u32,
                   coordinator: NodeId,
                   members: u16,
                   quorum: u16,
                   protocol: &str| {
        let Some(protocol) = crate::threshold::build(protocol, &t.protocol_params) else {
            return;
        };
        let base = COMMITTEE_NODE_BASE + slot * COMMITTEE_STRIDE;
        let members: Vec<NodeId> = (0..u32::from(members.min(64)))
            .map(|k| NodeId::new(base + k))
            .collect();
        roles.push(RoleSpec {
            role,
            coordinator,
            members,
            quorum,
            protocol,
            round_timeout: t.round_timeout,
            fanout_all: t.fanout_all,
            max_attempts: t.max_attempts.max(1),
            retry_after: t.retry_after,
        });
    };
    if t.pca {
        add("pca", 0, nodes.pca, t.members, t.quorum, t.protocol);
    }
    if t.crl {
        // An umbrella is a credential-issuance structure; the CRL committee signs a list
        // with the umbrella's inner protocol.
        let p = if t.protocol == crate::threshold::umbrella::ID {
            t.protocol_params.umbrella_inner
        } else {
            t.protocol
        };
        add("crl", 1, nodes.crlg, t.members, t.quorum, p);
    }
    if let Some((members, quorum)) = t.linkage {
        add(
            "linkage",
            2,
            nodes.ma,
            members,
            quorum,
            crate::threshold::linkage::ID,
        );
    }
    let committees = Committees::new(roles, envelope);
    let link = Link {
        latency: t.member_link_latency,
        bandwidth_bps: params.backend_link_bandwidth_bps,
        transport: Transport::BackendNet,
    };
    for r in &committees.roles {
        for m in &r.members {
            kernel.net_mut().connect(r.coordinator, *m, link);
        }
    }
    host_committees(kernel, &committees, params);
    committees
}

/// Hosts every committee member on `kernel` with the backend's service model and profile.
fn host_committees(kernel: &mut Kernel<ScmsMsg>, committees: &Committees<Pending>, p: &ScmsParams) {
    let spec = crate::service::ServiceModelSpec::new(p.backend_servers, p.backend_overhead);
    for r in &committees.roles {
        for m in &r.members {
            kernel.host(*m, &spec, p.backend_profile);
        }
    }
}

fn governance_links(n: &ScmsNodes, gov: &Governance) -> Vec<(NodeId, NodeId)> {
    vec![(gov.nodes.manager, gov.nodes.pg), (gov.nodes.pg, n.ra)]
}

/// Hosts the two governance entities that exchange messages during a run. The Root CA,
/// the electors and the ICA are offline and have no queue.
fn host_governance(kernel: &mut Kernel<ScmsMsg>, gov: &Governance, p: &ScmsParams) {
    let spec = crate::service::ServiceModelSpec::new(1, p.backend_overhead);
    kernel.host(gov.nodes.manager, &spec, p.backend_profile);
    kernel.host(gov.nodes.pg, &spec, p.backend_profile);
}
