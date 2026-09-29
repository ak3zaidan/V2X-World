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
use crate::kernel::{Delivery, Kernel, Outbox};
use crate::net::{BackendNet, Link, Transport};
use crate::pseudonym::{CertEvent, PseudonymStore, PseudonymStrategy};
use crate::scms::governance::{DeviceTrust, Governance, Policy};
use crate::scms::msg::{
    CertRequestItem, EnrolmentCert, IssuedCredential, LaIndex, Lci, PcaLookup, PreLinkageBatch,
    ProvisioningRequest, Refusal, ReportSubmission, ScmsMsg, ScmsSizes, SealedForPca,
};
use crate::scms::params::{ScmsNodes, ScmsParams};
use crate::sizes::{CertificateSizes, WireSize};
use crate::spec::{AES128_BLOCK, OpDescriptor};
use crate::stage::{FlowId, FlowRun, StageId};

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
    /// How many refusals it has received.
    pub refusals: u32,
    /// How many successor enrolment certificates it has installed.
    pub reenrolments: u32,
    /// CRL versions it refused because the signature did not verify.
    pub crls_rejected: u32,
    /// Whether an enrolment request is in flight.
    enrolling: bool,
    /// A provisioning request held until enrolment completes.
    deferred: Option<(Box<ProvisioningRequest>, FlowId, FlowRun)>,
    /// The run of the provisioning flow in progress.
    provisioning: Option<ProvisioningProgress>,
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
            refusals: 0,
            reenrolments: 0,
            crls_rejected: 0,
            enrolling: false,
            deferred: None,
            provisioning: None,
        }
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
        let private = cat.signing_private(i, j, &cred.c);
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
    /// Batches ready for download: device → i → credentials.
    repos: BTreeMap<NodeId, BTreeMap<u32, Vec<IssuedCredential>>>,
    shuffle_armed: bool,
    report_shuffle_armed: bool,
}

impl RaState {
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
    plv: [BTreeMap<(u32, u32), SealedForPca<PreLinkageValue>>; 2],
    lci: [Option<Lci>; 2],
    responses: u8,
    periods_certified: u32,
    sent: bool,
    batch_ready_stamped: bool,
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
    /// The investigation in progress, if any.
    pub case: Option<Case>,
    /// Devices it has concluded should be revoked, by request hash.
    pub decisions: Vec<[u8; 32]>,
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
}

/// The SHA-256 a CRL Generator signs: every entry, in order.
#[must_use]
pub fn crl_digest(entries: &[CrlLinkageEntry]) -> [u8; 32] {
    let mut b = Vec::with_capacity(entries.len() * 64);
    for e in entries {
        b.extend_from_slice(&e.i.to_be_bytes());
        b.extend_from_slice(&e.la_id1.0.to_be_bytes());
        b.extend_from_slice(&e.la_id2.0.to_be_bytes());
        b.extend_from_slice(e.ls1_i.as_bytes());
        b.extend_from_slice(e.ls2_i.as_bytes());
        b.extend_from_slice(&e.jmax.to_be_bytes());
    }
    sha256(&b)
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
    next_run: u32,
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
        let sizes = ScmsSizes::new(CertificateSizes::measured()?, params.sizes);
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
        let gov = Governance::ceremony(&params, &nodes, &rng, t0);
        for (a, b) in governance_links(&nodes, &gov) {
            net.connect(a, b, backend);
        }
        let mut kernel = Kernel::new_at(net, t0);
        for (node, spec) in nodes.backend_service_models(&params) {
            kernel.host(node, &spec, params.backend_profile);
        }
        host_governance(&mut kernel, &gov, &params);
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
                next_run: 0,
            },
            kernel,
        })
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

    /// Replaces the link `device` reaches the backend over (privacy proxy, RA, DCM, ECA,
    /// CRL Store) with `link`.
    ///
    /// The deployment's default device link is a fixed cellular uplink; a driver that
    /// models the access leg itself — the cellular Uu model's latency and capacity at the
    /// device's position, or a roadside unit's relay over its backhaul — states the link
    /// the next exchange will see here, so the flow's round trips cost what that access
    /// really costs and are charged to the transport it really used.
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

    /// Replaces a device's enrolment certificate: how a driver states the age of the
    /// certificate a vehicle on the road already holds (it was issued before the run, at
    /// the factory or at its last renewal), so that a run long enough — or a lifecycle
    /// compressed enough — sees it renewed.
    pub fn set_enrolment(&mut self, device: NodeId, cert: EnrolmentCert) {
        if let Some(d) = self.state.devices.get_mut(&device) {
            d.enrolment = Some(cert);
        }
    }

    /// Starts IEEE 1609.2.1's successor enrolment at `at`: the device asks the ECA for a
    /// new enrolment certificate, signing with the one it holds.
    pub fn reenrol_at(&mut self, device: NodeId, at: SimTime) -> FlowRun {
        let run = self.new_run();
        let current = self
            .state
            .devices
            .get(&device)
            .and_then(|d| d.enrolment)
            .unwrap_or(EnrolmentCert {
                generation: 0,
                valid_from: 0,
                valid_until: 0,
            });
        let at = at.max(self.kernel.now());
        self.inject_at(
            at,
            device,
            ScmsMsg::SuccessorEnrolRequest { device, current },
            FlowId::Reenrolment,
            run,
        );
        run
    }

    /// The SCMS Manager decides a new policy at `at`: the Policy Generator signs a new
    /// Global Policy File and the Registration Authority issues the Local Policy File its
    /// devices fetch on their next connection.
    pub fn decide_policy_at(&mut self, policy: Policy, at: SimTime) -> FlowRun {
        let run = self.new_run();
        let manager = self.state.gov.nodes.manager;
        let at = at.max(self.kernel.now());
        self.inject_at(
            at,
            manager,
            ScmsMsg::PolicyUpdate(Box::new(policy)),
            FlowId::PolicyDistribution,
            run,
        );
        run
    }

    /// Why the RA or the ECA last refused `device`, if either did since its last request.
    #[must_use]
    pub fn refusal_of(&self, device: NodeId) -> Option<Refusal> {
        self.state.devices.get(&device).and_then(|d| d.refused)
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

    /// Starts the enrolment flow for a device.
    pub fn enrol(&mut self, device: NodeId) -> FlowRun {
        let at = self.kernel.now();
        self.enrol_at(device, at)
    }

    /// Starts the enrolment flow at `at`.
    pub fn enrol_at(&mut self, device: NodeId, at: SimTime) -> FlowRun {
        let run = self.new_run();
        if let Some(d) = self.state.devices.get_mut(&device) {
            d.enrolling = true;
        }
        self.inject_at(
            at,
            device,
            ScmsMsg::EnrolRequest { device },
            FlowId::Enrolment,
            run,
        );
        run
    }

    /// Starts the butterfly provisioning flow.
    pub fn provision(&mut self, device: NodeId, start_i: u32, periods: u32, jmax: u32) -> FlowRun {
        let at = self.kernel.now();
        self.provision_at(device, at, start_i, periods, jmax)
    }

    /// Starts the butterfly provisioning flow at `at`.
    pub fn provision_at(
        &mut self,
        device: NodeId,
        at: SimTime,
        start_i: u32,
        periods: u32,
        jmax: u32,
    ) -> FlowRun {
        self.start_provisioning(device, at, start_i, periods, jmax, FlowId::Provisioning)
    }

    fn start_provisioning(
        &mut self,
        device: NodeId,
        at: SimTime,
        start_i: u32,
        periods: u32,
        jmax: u32,
        flow: FlowId,
    ) -> FlowRun {
        let run = self.new_run();
        let mut seed = [0u8; butterfly::SEED_BYTES];
        {
            let mut g = self
                .state
                .rng
                .checkout(RngDomain::Crypto, EntityRef::Node(device));
            g.fill_bytes(&mut seed);
        }
        let cat = Caterpillar::from_seed(&seed).expect("64 bytes of seed");
        let request = ProvisioningRequest {
            // Filled by the device when it signs the request (see its handler).
            enrolment: None,
            device,
            signing_public: cat.signing_public(),
            encryption_public: cat.encryption_public(),
            ck: *cat.ck(),
            ek: *cat.ek(),
            start_i,
            periods,
            jmax,
        };
        if let Some(d) = self.state.devices.get_mut(&device) {
            d.caterpillar = Some(cat);
            d.download_retries = 0;
            d.refused = None;
            d.provisioning = Some(ProvisioningProgress {
                start_i,
                periods,
                downloaded: 0,
                first_batch_seen: false,
            });
        }
        self.inject_at(
            at,
            device,
            ScmsMsg::ProvisioningRequest(Box::new(request)),
            flow,
            run,
        );
        run
    }

    /// Starts a top-up: one more i-period on an existing caterpillar.
    ///
    /// The same flow with a different name, which is what 05-protocols §3.2 says top-up is
    /// — "RA pre-generates up to 3 years ahead and adds a week every week".
    pub fn topup(&mut self, device: NodeId, i: u32, jmax: u32) -> FlowRun {
        let at = self.kernel.now();
        self.start_provisioning(device, at, i, 1, jmax, FlowId::Topup)
    }

    /// Starts a top-up at `at`, a driver's instant (never earlier than the kernel's
    /// clock): how an engine running the backend in lockstep asks for the next i-period
    /// the moment the device's pool runs low, rather than at whatever instant the
    /// backend last processed.
    pub fn topup_at(&mut self, device: NodeId, at: SimTime, i: u32, jmax: u32) -> FlowRun {
        let at = at.max(self.kernel.now());
        self.start_provisioning(device, at, i, 1, jmax, FlowId::Topup)
    }

    /// Submits a misbehaviour report about the certificate `(subject_i, subject_lv)`.
    pub fn submit_report(
        &mut self,
        reporter: NodeId,
        subject_i: u32,
        subject_lv: LinkageValue,
    ) -> FlowRun {
        let run = self.new_run();
        let observed_at = self.kernel.now();
        self.inject(
            reporter,
            ScmsMsg::Report(Box::new(ReportSubmission {
                reporter,
                subject_i,
                subject_lv,
                observed_at,
            })),
            FlowId::Report,
            run,
        );
        run
    }

    /// Opens an investigation over the two reports the MA holds at `indices`, revoking
    /// from period `i_rev` forward if they resolve to one device.
    ///
    /// Returns the resolution run and the CRL-issuance run.
    pub fn investigate(
        &mut self,
        a: usize,
        b: usize,
        i_rev: u32,
        jmax: u32,
    ) -> Option<(FlowRun, FlowRun)> {
        let run = self.new_run();
        let crl_run = self.new_run();
        let (ra, rb) = {
            let reports = &self.state.ma.reports;
            (reports.get(a)?.clone(), reports.get(b)?.clone())
        };
        self.state.ma.case = Some(Case {
            run,
            crl_run,
            subjects: [(ra.subject_i, ra.subject_lv), (rb.subject_i, rb.subject_lv)],
            lookups: Vec::new(),
            same: [None, None],
            seeds: [None, None],
            i_rev,
            jmax,
            resolved: false,
            direct: false,
            done: false,
        });
        let ma = self.state.nodes.ma;
        self.inject(
            ma,
            ScmsMsg::PcaLookupRequest {
                i: ra.subject_i,
                lv: ra.subject_lv,
            },
            FlowId::LinkageResolution,
            run,
        );
        Some((run, crl_run))
    }

    /// Revokes the device behind the report the MA holds at `index`, from period `i_rev`
    /// forward: the revocation sequence of [BRECHT §VI-D] for one reported certificate.
    ///
    /// MA → PCA (`lv` → request hash), MA → RA (hash → blocklist, LCIs), MA → LA1 and LA2
    /// (LCI → `ls(i_rev)`), MA → CRLG. **Both** Linkage Authorities must answer with a seed
    /// before an entry exists: a CRL entry is the pair of seeds, and one LA alone can
    /// neither issue nor refuse on the other's behalf.
    ///
    /// The decision to revoke is the authority's pipeline's, taken before this is called
    /// (`v2xw_threat::LegacyWindow`); this is only the protocol that carries it out.
    ///
    /// Returns the resolution run and the CRL-issuance run, or `None` when no such report
    /// is held or a case is still in progress (cases are carried out one at a time, as the
    /// single `MaState::case` slot says; the caller queues).
    pub fn revoke(&mut self, index: usize, i_rev: u32, jmax: u32) -> Option<(FlowRun, FlowRun)> {
        if self.case_open() {
            return None;
        }
        let r = self.state.ma.reports.get(index)?.clone();
        let run = self.new_run();
        let crl_run = self.new_run();
        self.state.ma.case = Some(Case {
            run,
            crl_run,
            subjects: [(r.subject_i, r.subject_lv), (r.subject_i, r.subject_lv)],
            lookups: Vec::new(),
            same: [None, None],
            seeds: [None, None],
            i_rev,
            jmax,
            resolved: false,
            direct: true,
            done: false,
        });
        let ma = self.state.nodes.ma;
        self.inject(
            ma,
            ScmsMsg::PcaLookupRequest {
                i: r.subject_i,
                lv: r.subject_lv,
            },
            FlowId::LinkageResolution,
            run,
        );
        Some((run, crl_run))
    }

    /// Whether a case is in progress.
    #[must_use]
    pub fn case_open(&self) -> bool {
        self.state.ma.case.as_ref().is_some_and(|c| !c.done)
    }

    /// A misbehaviour report whose **access leg** — the device's own link to the privacy
    /// proxy — was carried by the caller, arriving at the Location Obscurer Proxy at
    /// `arrive_at`.
    ///
    /// An engine models that leg itself (cellular Uu with its latency, capacity and
    /// coverage, or an RSU relay over the sidelink and the unit's backhaul), so the
    /// deployment's fixed device link must not be charged a second time. The device-side
    /// stages are stamped at the instants the caller supplies, the leg is written to the
    /// wire log with the transport and the bytes it really used, and the report enters the
    /// backend at the proxy.
    #[allow(clippy::too_many_arguments)]
    pub fn report_at_proxy(
        &mut self,
        reporter: NodeId,
        subject_i: u32,
        subject_lv: LinkageValue,
        detected_at: SimTime,
        sent_at: SimTime,
        arrive_at: SimTime,
        transport: Transport,
        bytes: u32,
    ) -> FlowRun {
        let run = self.new_run();
        let flow = FlowId::Report;
        for (stage, t) in [
            (StageId::Detect, detected_at),
            (StageId::ReportSent, sent_at),
        ] {
            self.kernel.stages.push(crate::stage::StageStamp {
                t,
                run,
                flow,
                stage,
                node: Some(reporter),
                size: None,
            });
        }
        let lop = self.state.nodes.lop;
        self.kernel.steps.push(crate::stage::WireStep {
            t: sent_at,
            from: reporter,
            to: lop,
            flow,
            run,
            step: "report",
            bytes,
            transport,
        });
        self.kernel.inject_at(
            arrive_at,
            Delivery {
                at: arrive_at,
                from: reporter,
                to: lop,
                msg: ScmsMsg::Report(Box::new(ReportSubmission {
                    reporter,
                    subject_i,
                    subject_lv,
                    observed_at: detected_at,
                })),
                flow,
                run,
            },
        );
        run
    }

    /// Enrols and provisions `device` **before the run**, leaving the backend's clock,
    /// queues and logs untouched.
    ///
    /// A vehicle on the road already holds its pool: enrolment happens at the factory or
    /// dealership and the first batch is downloaded long before the drive a scenario
    /// simulates (05-protocols.md §3.2, bootstrap "offline in the PoC; modeled as a
    /// scenario-time-zero event"). The flows run for real — butterfly expansion, the two
    /// Linkage Authorities' pre-linkage values, the PCA's issuance — on a scratch kernel
    /// over the same topology, so every entity's *state* (the RA's request record, the
    /// LAs' chains, the PCA's `lv` table, the device's credentials) is exactly what the
    /// in-run flows would have left; only the scratch kernel's time and logs are thrown
    /// away. That is what lets a revocation during the run resolve this device.
    ///
    /// # Errors
    /// Whatever the flows return on the scratch kernel.
    pub fn preload(&mut self, device: NodeId, start_i: u32, periods: u32, jmax: u32) -> Result<()> {
        if !self.state.devices.contains_key(&device) {
            self.add_device(device);
        }
        let mut scratch: Kernel<ScmsMsg> = Kernel::new_at(self.kernel.net().clone(), 0);
        let p = self.state.params;
        for (node, spec) in self.state.nodes.backend_service_models(&p) {
            scratch.host(node, &spec, p.backend_profile);
        }
        host_governance(&mut scratch, &self.state.gov, &p);
        // Every device already hosted keeps a host on the scratch kernel too, so a device
        // that is being provisioned is never the only one the handlers can address.
        scratch.host(
            device,
            &crate::service::ServiceModelSpec::new(1, p.device_overhead),
            p.device_profile,
        );
        let mut real = core::mem::replace(&mut self.kernel, scratch);
        // The request shuffle happened long before the run and its length is not what a
        // pre-run pool is for; with the cited one-day window the device's download polls
        // would give up before the batch was ready. The window is restored below.
        let saved = self.state.params;
        self.state.params.shuffle_window = Duration::ZERO;
        self.state.params.first_batch_delay = Duration::ZERO;
        self.enrol_at(device, 0);
        let mut outcome = Self::drain_into(&mut self.state, &mut self.kernel);
        if outcome.is_ok() {
            let at = self.kernel.now();
            self.start_provisioning(device, at, start_i, periods, jmax, FlowId::Provisioning);
            outcome = Self::drain_into(&mut self.state, &mut self.kernel);
        }
        core::mem::swap(&mut self.kernel, &mut real);
        self.state.params = saved;
        outcome
    }

    fn drain_into(state: &mut ScmsState, kernel: &mut Kernel<ScmsMsg>) -> Result<()> {
        while let Some(d) = kernel.next_delivery() {
            let profile = kernel.profile_of(d.to);
            let mut out = Outbox::new(profile);
            let (at, to) = (d.at, d.to);
            state.handle(d, &mut out)?;
            kernel.dispatch(at, to, out)?;
        }
        Ok(())
    }

    /// Has a device fetch, expand and enforce the current CRL.
    pub fn distribute_crl(&mut self, device: NodeId) -> FlowRun {
        let run = self.new_run();
        self.inject(
            device,
            ScmsMsg::CrlDownloadRequest { device },
            FlowId::CrlDistribution,
            run,
        );
        run
    }

    /// Has the roadside broadcast path push the current CRL to `device`.
    ///
    /// The device must have been attached with [`ScmsRun::attach_rsu`]; without a link the
    /// kernel refuses to deliver rather than delivering for free (invariant I-P1).
    pub fn broadcast_crl_to(&mut self, device: NodeId) -> FlowRun {
        let run = self.new_run();
        let broadcast = self.state.nodes.crl_broadcast;
        self.inject(
            broadcast,
            ScmsMsg::CrlAirBroadcast { device },
            FlowId::CrlDistribution,
            run,
        );
        run
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
            let profile = kernel.profile_of(d.to);
            let mut out = Outbox::new(profile);
            let (at, to) = (d.at, d.to);
            state.handle(d, &mut out)?;
            kernel.dispatch(at, to, out)?;
        }
        Ok(())
    }

    /// The single entry on the published CRL, for a test that needs to inspect it.
    pub fn crl_entry(&self) -> Option<&v2xw_sec::linkage::CrlLinkageEntry> {
        self.state.crl_store.entries.first()
    }

    /// Runs until nothing is scheduled.
    ///
    /// # Errors
    /// Whatever [`Kernel::dispatch`] or a handler returns: an unhosted node, a missing
    /// link or a flow driven out of order — all modelling defects.
    pub fn run(&mut self) -> Result<()> {
        let ScmsRun { state, kernel } = self;
        while let Some(d) = kernel.next_delivery() {
            let profile = kernel.profile_of(d.to);
            let mut out = Outbox::new(profile);
            let (at, to) = (d.at, d.to);
            state.handle(d, &mut out)?;
            kernel.dispatch(at, to, out)?;
        }
        Ok(())
    }
}

impl ScmsState {
    fn sign(out: &mut Outbox<ScmsMsg>, n: u32) {
        out.compute(OpDescriptor::new(ECDSA, PrimitiveOpKind::Sign, n));
    }

    fn verify(out: &mut Outbox<ScmsMsg>, n: u32) {
        out.compute(OpDescriptor::new(ECDSA, PrimitiveOpKind::Verify, n));
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
        let n = self.nodes;
        let (flow, run, to, at, from) = (d.flow, d.run, d.to, d.at, d.from);
        match d.msg {
            // ---------------- enrolment ----------------
            ScmsMsg::EnrolRequest { device } if to == device => {
                Self::sign(out, 1);
                out.stage_at(StageId::Requested, device, None, flow, run);
                out.send(
                    n.dcm,
                    ScmsMsg::EnrolRequest { device },
                    "enrol-request",
                    self.sizes.enrol_request(),
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::EnrolRequest { device } => {
                // Device Configuration Manager: checks the device is one it configured,
                // re-signs and forwards to the Enrolment CA.
                Self::verify(out, 1);
                Self::sign(out, 1);
                out.stage_at(StageId::ProxyForwarded, to, None, flow, run);
                out.send(
                    n.eca,
                    ScmsMsg::EnrolForward { device },
                    "enrol-forward",
                    self.sizes.enrol_forward(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
                // The bootstrap's other half: the DCM hands the device the elector
                // anchors, the trust list, the Local Certificate Chain File and the Local
                // Policy File, without which it could verify nothing it will receive.
                self.dcm.bootstrapped += 1;
                self.dcm.bundles += 1;
                let anchors = u32::try_from(self.gov.nodes.electors.len()).unwrap_or(0);
                let ctl_certs =
                    u32::try_from(self.gov.ctl.roots.len() + self.gov.ctl.electors.len())
                        .unwrap_or(0);
                let chain = u32::try_from(self.gov.lccf.certs.len()).unwrap_or(0);
                out.send(
                    device,
                    ScmsMsg::TrustBundle { device },
                    "trust-bundle",
                    self.sizes.trust_bundle(anchors, ctl_certs, chain),
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::TrustBundle { device } => {
                let (anchors, ctl, lccf, lpf) = (
                    self.gov.anchors(),
                    self.gov.ctl.clone(),
                    self.gov.lccf.clone(),
                    self.gov.lpf.clone(),
                );
                let Some(dev) = self.devices.get_mut(&device) else {
                    return Err(ProtoError::NoEntity { node: device });
                };
                let before = dev.trust.verifications;
                dev.trust.anchors = anchors;
                // A refusal is recorded on the device's trust store and leaves it
                // unable to verify what the refused artefact would have vouched for.
                let _ = dev.trust.install_ctl(&ctl);
                let _ = dev.trust.install_chain(&lccf, at);
                let _ = dev.trust.install_policy(&lpf);
                let checks = dev.trust.verifications.saturating_sub(before);
                Self::verify(out, checks);
            }
            ScmsMsg::EnrolForward { device } => {
                Self::verify(out, 1);
                Self::sign(out, 1);
                self.issued_enrolment.insert(device);
                self.eca.issued += 1;
                let cert = EnrolmentCert {
                    generation: 0,
                    valid_from: at,
                    valid_until: self.params.enrolment_lifetime.after(at),
                };
                out.stage_at(StageId::Certified, to, None, flow, run);
                out.send(
                    device,
                    ScmsMsg::EnrolResponse { device, cert },
                    "enrol-response",
                    self.sizes.enrol_response(),
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::EnrolResponse { device, cert } => {
                Self::verify(out, 2);
                let deferred = self.devices.get_mut(&device).and_then(|dev| {
                    dev.enrolled = true;
                    dev.enrolment = Some(cert);
                    dev.enrolling = false;
                    dev.deferred.take()
                });
                out.stage_at(StageId::Installed, device, None, flow, run);
                // A provisioning request made while enrolment was still in flight goes
                // out now, signed with the certificate that just arrived.
                if let Some((req, pflow, prun)) = deferred {
                    out.start_timer(
                        Duration::ZERO,
                        ScmsMsg::ProvisioningRequest(req),
                        pflow,
                        prun,
                    );
                }
            }

            // ---------------- provisioning ----------------
            ScmsMsg::ProvisioningRequest(mut req) if to == req.device => {
                // The request is signed with the enrolment certificate, so a device still
                // waiting for one holds the request until it arrives.
                if let Some(dev) = self.devices.get_mut(&req.device) {
                    if dev.enrolment.is_none() && dev.enrolling {
                        dev.deferred = Some((req, flow, run));
                        return Ok(());
                    }
                    req.enrolment = dev.enrolment;
                }
                // Every connection to the RA first asks for newer local policy and chain
                // files (IEEE 1609.2.1's LPF and LCCF download): a policy change reaches a
                // device the next time it tops up.
                let (lpf, lccf) = self.devices.get(&req.device).map_or((None, None), |d| {
                    (d.trust.lpf.as_ref().map(|l| l.version), d.trust.lccf_version)
                });
                Self::sign(out, 1);
                out.send(
                    n.lop,
                    ScmsMsg::FileRequest {
                        device: req.device,
                        lpf,
                        lccf,
                    },
                    "file-request",
                    self.sizes.file_request(),
                    Transport::CellularUu,
                    flow,
                    run,
                );
                // The device signs the request with its enrolment certificate and
                // encrypts it to the RA.
                Self::sign(out, 1);
                Self::scalar_mults(out, 1);
                out.stage_at(StageId::Requested, req.device, None, flow, run);
                out.send(
                    n.lop,
                    ScmsMsg::ProvisioningRequest(req),
                    "provisioning-request",
                    self.sizes.provisioning_request(),
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::ProvisioningRequest(req) if to == n.lop => {
                // The Location Obscurer Proxy strips the network identifiers. It performs
                // no cryptography: the payload is encrypted to the RA and it cannot read
                // it, which is the whole reason it is a separate organisation.
                self.lop.upstream += 1;
                out.stage_at(StageId::ProxyForwarded, to, None, flow, run);
                out.send(
                    n.ra,
                    ScmsMsg::ProvisioningRequest(req),
                    "provisioning-request-proxied",
                    self.sizes.provisioning_request(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::ProvisioningRequest(mut req) => {
                Self::verify(out, 1);
                // The checks IEEE 1609.2.1 has the RA make before it spends anything: the
                // enrolment certificate the request is signed with was issued by the ECA,
                // is inside its validity, and is not blocklisted. A refusal is an answer,
                // not silence, so the device knows whether to stop or to renew.
                let refusal = if self.ra.blocklist.contains(&req.device) {
                    Some(Refusal::Blocklisted)
                } else {
                    match req.enrolment {
                        Some(ec) if self.issued_enrolment.contains(&req.device) => {
                            (!ec.valid_at(at)).then_some(Refusal::EnrolmentExpired)
                        }
                        _ => Some(Refusal::UnknownEnrolment),
                    }
                };
                if let Some(reason) = refusal {
                    if reason == Refusal::Blocklisted {
                        // The passive half of revocation: the RA stops issuing.
                        self.ra.refused += 1;
                    } else {
                        self.ra.refused_enrolment += 1;
                    }
                    Self::sign(out, 1);
                    out.send(
                        n.lop,
                        ScmsMsg::ProvisioningRefused {
                            device: req.device,
                            reason,
                        },
                        "provisioning-refused",
                        self.sizes.refusal(),
                        Transport::BackendNet,
                        flow,
                        run,
                    );
                    return Ok(());
                }
                // The policy the RA serves is the policy it enforces.
                let policy = self.gov.lpf.policy;
                let jmax = req.jmax.min(policy.certs_per_period.max(1));
                let periods = req.periods.min(policy.max_periods_ahead.max(1));
                if jmax != req.jmax || periods != req.periods {
                    self.ra.clipped += 1;
                    req.jmax = jmax;
                    req.periods = periods;
                }
                self.ra.accepted += 1;
                self.ra.enrolled.insert(req.device);
                let request_hash = request_hash(&req);
                self.ra.requests.insert(
                    request_hash,
                    RequestRecord {
                        device: req.device,
                        lci1: None,
                        lci2: None,
                    },
                );
                out.stage_at(StageId::Acknowledged, to, None, flow, run);

                // Butterfly expansion: two scalar multiplications per certificate, and
                // nothing that reveals the certified key, which only the PCA's `c` fixes.
                let total = req.periods.saturating_mul(req.jmax);
                Self::scalar_mults(out, 2 * total);
                let mut cocoons = BTreeMap::new();
                for i in req.start_i..req.start_i + req.periods {
                    for j in 0..req.jmax {
                        let (b, _q) = butterfly::ra_cocoon_keys(
                            &req.signing_public,
                            &req.encryption_public,
                            &req.ck,
                            &req.ek,
                            i,
                            j,
                        );
                        cocoons.insert((i, j), b);
                    }
                }
                out.stage_at(StageId::Expanded, to, None, flow, run);

                self.ra.jobs.push(ProvisioningJob {
                    run,
                    flow,
                    device: req.device,
                    request_hash,
                    start_i: req.start_i,
                    periods: req.periods,
                    jmax: req.jmax,
                    cocoons,
                    plv: [BTreeMap::new(), BTreeMap::new()],
                    lci: [None, None],
                    responses: 0,
                    periods_certified: 0,
                    sent: false,
                    batch_ready_stamped: false,
                });

                out.send(
                    n.lop,
                    ScmsMsg::ProvisioningAck {
                        device: req.device,
                        request_hash,
                        periods: req.periods,
                        jmax: req.jmax,
                    },
                    "provisioning-ack",
                    self.sizes.provisioning_ack(),
                    Transport::CellularUu,
                    flow,
                    run,
                );
                for la in [LaIndex::One, LaIndex::Two] {
                    out.send(
                        self.nodes.la(la),
                        ScmsMsg::PreLinkageRequest {
                            device: req.device,
                            la,
                            start_i: req.start_i,
                            periods: req.periods,
                            jmax: req.jmax,
                        },
                        "pre-linkage-request",
                        self.sizes.pre_linkage_request(),
                        Transport::BackendNet,
                        flow,
                        run,
                    );
                }
            }
            ScmsMsg::ProvisioningAck {
                device,
                request_hash,
                periods,
                jmax,
            } if to == n.lop => {
                self.lop.downstream += 1;
                out.send(
                    device,
                    ScmsMsg::ProvisioningAck {
                        device,
                        request_hash,
                        periods,
                        jmax,
                    },
                    "provisioning-ack-proxied",
                    self.sizes.provisioning_ack(),
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::ProvisioningAck {
                device, periods, ..
            } => {
                // What the RA accepted, which may be less than was asked.
                if let Some(p) = self
                    .devices
                    .get_mut(&device)
                    .and_then(|d| d.provisioning.as_mut())
                {
                    p.periods = periods;
                }
                // The device now waits for the first batch time and polls the repository.
                out.start_timer(
                    self.params.first_batch_delay,
                    ScmsMsg::BatchDownloadRequest {
                        device,
                        i: self
                            .devices
                            .get(&device)
                            .and_then(|d| d.provisioning)
                            .map_or(0, |p| p.start_i),
                    },
                    flow,
                    run,
                );
            }
            ScmsMsg::PreLinkageRequest {
                device,
                la,
                start_i,
                periods,
                jmax,
            } => {
                let idx = la.idx();
                let lci = {
                    let st = &mut self.la[idx];
                    let lci = Lci(st.next_lci);
                    st.next_lci += 1;
                    lci
                };
                let mut seed_bytes = [0u8; linkage::LS_BYTES];
                {
                    let mut g = self
                        .rng
                        .checkout(RngDomain::Crypto, EntityRef::Node(self.nodes.la(la)));
                    g.fill_bytes(&mut seed_bytes);
                }
                let ls0 = LinkageSeed::new(seed_bytes);
                {
                    let st = &mut self.la[idx];
                    st.chains.insert(lci, ls0);
                    st.owner.insert(lci, device);
                }
                let la_id = self.la[idx].la_id;

                // One hash per period to walk the chain, two AES blocks per pre-linkage
                // value ([BRECHT §V-B]).
                Self::hashes(out, periods);
                Self::aes(out, periods.saturating_mul(jmax).saturating_mul(2));

                let mut values = Vec::new();
                for i in start_i..start_i + periods {
                    let ls_i = linkage::linkage_seed_at(la_id, ls0, i);
                    for j in 0..jmax {
                        values.push((
                            i,
                            j,
                            SealedForPca::seal(linkage::pre_linkage_value(la_id, ls_i, j)),
                        ));
                    }
                }
                let count = u32::try_from(values.len()).unwrap_or(u32::MAX);
                self.la[idx].plv_issued += u64::from(count);
                out.send(
                    n.ra,
                    ScmsMsg::PreLinkageResponse(Box::new(PreLinkageBatch {
                        la,
                        la_id,
                        lci,
                        values,
                    })),
                    "pre-linkage-response",
                    self.sizes.pre_linkage_response(count),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::PreLinkageResponse(batch) => {
                let Some(job) = self.ra.jobs.iter_mut().find(|j| j.run == run) else {
                    return Err(ProtoError::Flow {
                        flow: "provisioning",
                        detail: "pre-linkage values for a request the RA does not hold".into(),
                    });
                };
                let idx = batch.la.idx();
                job.lci[idx] = Some(batch.lci);
                for (i, j, plv) in batch.values {
                    job.plv[idx].insert((i, j), plv);
                }
                job.responses += 1;
                let hash = job.request_hash;
                let ready = job.responses == 2;
                let lcis = job.lci;
                if let Some(rec) = self.ra.requests.get_mut(&hash) {
                    rec.lci1 = lcis[0];
                    rec.lci2 = lcis[1];
                }
                if ready {
                    out.stage_at(StageId::PreLinkageReady, to, None, flow, run);
                    if !self.ra.shuffle_armed {
                        self.ra.shuffle_armed = true;
                        out.start_timer(
                            self.params.shuffle_window,
                            ScmsMsg::ShuffleTimer,
                            flow,
                            run,
                        );
                    }
                }
            }
            ScmsMsg::ShuffleTimer => {
                self.ra.shuffle_armed = false;
                let jobs = core::mem::take(&mut self.ra.jobs);
                let mut still_waiting = Vec::new();
                for mut job in jobs {
                    if job.responses < 2 || job.sent {
                        still_waiting.push(job);
                        continue;
                    }
                    job.sent = true;
                    out.stage_at(StageId::Shuffled, to, None, job.flow, job.run);
                    for i in job.start_i..job.start_i + job.periods {
                        let mut items = Vec::new();
                        for j in 0..job.jmax {
                            let (Some(b), Some(p1), Some(p2), Some(l1), Some(l2)) = (
                                job.cocoons.get(&(i, j)).copied(),
                                job.plv[0].get(&(i, j)).cloned(),
                                job.plv[1].get(&(i, j)).cloned(),
                                job.lci[0],
                                job.lci[1],
                            ) else {
                                continue;
                            };
                            items.push(CertRequestItem {
                                i,
                                j,
                                cocoon_signing: b,
                                plv1: p1,
                                plv2: p2,
                                lci1: SealedForPca::seal(l1),
                                lci2: SealedForPca::seal(l2),
                                request_hash: job.request_hash,
                            });
                        }
                        let count = u32::try_from(items.len()).unwrap_or(u32::MAX);
                        out.send(
                            n.pca,
                            ScmsMsg::CertRequest {
                                device: job.device,
                                i,
                                items: Box::new(items),
                            },
                            "cert-request",
                            self.sizes.cert_request(count),
                            Transport::BackendNet,
                            job.flow,
                            job.run,
                        );
                    }
                    self.ra.repos.entry(job.device).or_default();
                    still_waiting.push(job);
                }
                self.ra.jobs = still_waiting;
            }
            ScmsMsg::CertRequest { device, i, items } => {
                let count = u32::try_from(items.len()).unwrap_or(u32::MAX);
                // Per certificate: two ECIES decryptions of the pre-linkage values, one
                // ECQV issuance, one ECIES encryption and one signature (05-protocols §3.2).
                Self::scalar_mults(out, 4 * count);
                Self::sign(out, count);
                let mut credentials = Vec::new();
                for item in items.into_iter() {
                    let (Some(plv1), Some(plv2), Some(lci1), Some(lci2)) = (
                        item.plv1.open(to, n.pca),
                        item.plv2.open(to, n.pca),
                        item.lci1.open(to, n.pca),
                        item.lci2.open(to, n.pca),
                    ) else {
                        return Err(ProtoError::Flow {
                            flow: "provisioning",
                            detail: "only the PCA may open a sealed pre-linkage value".into(),
                        });
                    };
                    let lv = linkage::linkage_value(plv1, plv2);
                    let mut be = [0u8; 32];
                    {
                        let mut g = self.rng.checkout(RngDomain::Crypto, EntityRef::Node(n.pca));
                        g.fill_bytes(&mut be);
                    }
                    let c = ec::scalar_from_be_mod_n(&be);
                    let (certified_public, _big_c) =
                        butterfly::pca_certify_explicit(&item.cocoon_signing, &c);
                    self.pca.issued.insert(
                        (item.i, *lv.as_bytes()),
                        PcaLookup {
                            lci1,
                            lci2,
                            request_hash: item.request_hash,
                        },
                    );
                    self.pca.issued_count += 1;
                    credentials.push(IssuedCredential {
                        i: item.i,
                        j: item.j,
                        lv,
                        certified_public,
                        c,
                    });
                }
                if self.pca.certified_runs.insert(run) {
                    out.stage_at(StageId::Certified, to, None, flow, run);
                }
                out.send(
                    n.ra,
                    ScmsMsg::CertResponse {
                        device,
                        i,
                        credentials: Box::new(credentials),
                    },
                    "cert-response",
                    self.sizes.cert_batch(count),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::CertResponse {
                device,
                i,
                credentials,
            } => {
                self.ra
                    .repos
                    .entry(device)
                    .or_default()
                    .insert(i, *credentials);
                // `batch_ready` is the first moment the repository holds something the
                // device can fetch, not the last: a device that polls early downloads the
                // first i-period while later ones are still being certified, and a stage
                // that came after that download would be a lie about the order.
                let mut first = false;
                let mut finished = false;
                if let Some(j) = self.ra.jobs.iter_mut().find(|j| j.run == run) {
                    j.periods_certified += 1;
                    first = !core::mem::replace(&mut j.batch_ready_stamped, true);
                    finished = j.periods_certified >= j.periods;
                }
                if first {
                    out.stage_at(StageId::BatchReady, to, None, flow, run);
                }
                if finished {
                    self.ra.jobs.retain(|j| j.run != run);
                }
            }
            ScmsMsg::BatchDownloadRequest { device, i } if to == device => {
                out.send(
                    n.lop,
                    ScmsMsg::BatchDownloadRequest { device, i },
                    "batch-download-request",
                    self.sizes.batch_download_request(),
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::BatchDownloadRequest { device, i } if to == n.lop => {
                self.lop.upstream += 1;
                out.send(
                    n.ra,
                    ScmsMsg::BatchDownloadRequest { device, i },
                    "batch-download-request-proxied",
                    self.sizes.batch_download_request(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::BatchDownloadRequest { device, i } => {
                let credentials = self
                    .ra
                    .repos
                    .get(&device)
                    .and_then(|r| r.get(&i))
                    .cloned()
                    .unwrap_or_default();
                let count = u32::try_from(credentials.len()).unwrap_or(u32::MAX);
                // The batch file travels back through the proxy, encrypted end to end: each
                // certificate to the device's butterfly encryption key, which neither the
                // RA nor the LOP holds.
                out.send(
                    n.lop,
                    ScmsMsg::BatchDownload {
                        device,
                        i,
                        credentials: Box::new(credentials),
                    },
                    "batch-download",
                    self.sizes.batch_download(count),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::BatchDownload {
                device,
                i,
                credentials,
            } if to == n.lop => {
                self.lop.downstream += 1;
                let count = u32::try_from(credentials.len()).unwrap_or(u32::MAX);
                out.send(
                    device,
                    ScmsMsg::BatchDownload {
                        device,
                        i,
                        credentials,
                    },
                    "batch-download-proxied",
                    self.sizes.batch_download(count),
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::BatchDownload {
                device,
                i,
                credentials,
            } => {
                let poll = self.params.download_poll_interval;
                let cap = self.params.max_download_polls;
                let Some(dev) = self.devices.get_mut(&device) else {
                    return Err(ProtoError::NoEntity { node: device });
                };
                if credentials.is_empty() {
                    dev.download_retries += 1;
                    if dev.download_retries <= cap {
                        out.start_timer(
                            poll,
                            ScmsMsg::BatchDownloadRequest { device, i },
                            flow,
                            run,
                        );
                    }
                    return Ok(());
                }
                // One ECQV reconstruction per certificate on download (05-protocols §3.2).
                let count = u32::try_from(credentials.len()).unwrap_or(u32::MAX);
                out.compute(OpDescriptor::new(ECQV, PrimitiveOpKind::Verify, count));
                for cred in credentials.into_iter() {
                    dev.credentials.insert((cred.i, cred.j), cred);
                }
                let mut finished = false;
                let mut next_i = None;
                if let Some(p) = dev.provisioning.as_mut() {
                    p.downloaded += 1;
                    if !p.first_batch_seen {
                        p.first_batch_seen = true;
                        out.stage_at(StageId::Downloaded, device, None, flow, run);
                    }
                    if p.downloaded >= p.periods {
                        finished = true;
                    } else {
                        next_i = Some(p.start_i + p.downloaded);
                    }
                }
                if finished {
                    dev.provisioning = None;
                    out.stage_at(StageId::Installed, device, None, flow, run);
                } else if let Some(i_next) = next_i {
                    out.send(
                        n.lop,
                        ScmsMsg::BatchDownloadRequest { device, i: i_next },
                        "batch-download-request",
                        self.sizes.batch_download_request(),
                        Transport::CellularUu,
                        flow,
                        run,
                    );
                }
            }

            // ---------------- misbehaviour reporting ----------------
            ScmsMsg::Report(r) if to == r.reporter => {
                Self::sign(out, 1);
                Self::scalar_mults(out, 1);
                out.stage_at(StageId::Detect, r.reporter, None, flow, run);
                out.stage_at(StageId::ReportSent, r.reporter, None, flow, run);
                out.send(
                    n.lop,
                    ScmsMsg::Report(r),
                    "report",
                    self.sizes.report(),
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::Report(r) if to == n.lop => {
                self.lop.upstream += 1;
                out.stage_at(StageId::ProxyForwarded, to, None, flow, run);
                out.send(
                    n.ra,
                    ScmsMsg::Report(r),
                    "report-proxied",
                    self.sizes.report(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::Report(r) if to == n.ra => {
                self.ra.reports.push((run, *r));
                if !self.ra.report_shuffle_armed {
                    self.ra.report_shuffle_armed = true;
                    out.start_timer(
                        self.params.report_shuffle_window,
                        ScmsMsg::ReportShuffleTimer,
                        flow,
                        run,
                    );
                }
            }
            ScmsMsg::Report(r) => {
                Self::verify(out, 1);
                Self::scalar_mults(out, 1);
                self.ma.reports.push(*r);
                out.stage_at(StageId::ReportReceived, to, None, flow, run);
            }
            ScmsMsg::ReportShuffleTimer => {
                self.ra.report_shuffle_armed = false;
                for (r_run, report) in core::mem::take(&mut self.ra.reports) {
                    out.stage_at(StageId::Shuffled, to, None, FlowId::Report, r_run);
                    out.send(
                        n.ma,
                        ScmsMsg::Report(Box::new(report)),
                        "report-forward",
                        self.sizes.report(),
                        Transport::BackendNet,
                        FlowId::Report,
                        r_run,
                    );
                }
            }

            // ---------------- investigation and revocation ----------------
            ScmsMsg::PcaLookupRequest { i, lv } if to == n.ma => {
                out.stage_at(StageId::Decision, to, None, flow, run);
                out.send(
                    n.pca,
                    ScmsMsg::PcaLookupRequest { i, lv },
                    "pca-lookup-request",
                    self.sizes.pca_lookup_request(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::PcaLookupRequest { i, lv } => {
                self.pca.lookups += 1;
                let found = self.pca.issued.get(&(i, *lv.as_bytes())).copied();
                out.send(
                    n.ma,
                    ScmsMsg::PcaLookupResponse { found },
                    "pca-lookup-response",
                    self.sizes.pca_lookup_response(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::PcaLookupResponse { found } => {
                let Some(case) = self.ma.case.as_mut() else {
                    return Ok(());
                };
                let Some(found) = found else {
                    // The PCA issued no certificate with that linkage value: nothing to
                    // resolve, and the case ends here.
                    case.done = true;
                    return Ok(());
                };
                case.lookups.push(found);
                if case.direct {
                    // One certificate names one enrolment: the PCA's answer is the
                    // request hash, which is the device ([BRECHT §VI-D]).
                    case.resolved = true;
                    let hash = found.request_hash;
                    self.ma.decisions.push(hash);
                    out.stage_at(StageId::Resolved, to, None, flow, run);
                    out.send(
                        n.ra,
                        ScmsMsg::BlocklistRequest { request_hash: hash },
                        "blocklist-request",
                        self.sizes.blocklist_request(),
                        Transport::BackendNet,
                        flow,
                        run,
                    );
                } else if case.lookups.len() == 1 {
                    let (i, lv) = case.subjects[1];
                    out.send(
                        n.pca,
                        ScmsMsg::PcaLookupRequest { i, lv },
                        "pca-lookup-request",
                        self.sizes.pca_lookup_request(),
                        Transport::BackendNet,
                        flow,
                        run,
                    );
                } else if case.lookups.len() == 2 {
                    let (a, b) = (case.lookups[0], case.lookups[1]);
                    for (la, x, y) in [
                        (LaIndex::One, a.lci1, b.lci1),
                        (LaIndex::Two, a.lci2, b.lci2),
                    ] {
                        out.send(
                            self.nodes.la(la),
                            ScmsMsg::SameDeviceRequest { la, a: x, b: y },
                            "same-device-request",
                            self.sizes.same_device_request(),
                            Transport::BackendNet,
                            flow,
                            run,
                        );
                    }
                }
            }
            ScmsMsg::SameDeviceRequest { la, a, b } => {
                // The LA answers a single bit: whether the two chains belong to one
                // device. It never reveals which device, and the MA never learns a seed
                // from this exchange ([BRECHT §VI-C]).
                let st = &self.la[la.idx()];
                let same = match (st.owner.get(&a), st.owner.get(&b)) {
                    (Some(x), Some(y)) => x == y,
                    _ => false,
                };
                out.send(
                    n.ma,
                    ScmsMsg::SameDeviceResponse { la, same },
                    "same-device-response",
                    self.sizes.same_device_response(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::SameDeviceResponse { la, same } => {
                let (ready, hash) = {
                    let Some(case) = self.ma.case.as_mut() else {
                        return Ok(());
                    };
                    case.same[la.idx()] = Some(same);
                    match (case.same[0], case.same[1]) {
                        (Some(a), Some(b)) => {
                            case.resolved = a && b;
                            if !case.resolved {
                                case.done = true;
                            }
                            (case.resolved, case.lookups[0].request_hash)
                        }
                        _ => (false, [0u8; 32]),
                    }
                };
                if ready {
                    self.ma.decisions.push(hash);
                    out.stage_at(StageId::Resolved, to, None, flow, run);
                    out.send(
                        n.ra,
                        ScmsMsg::BlocklistRequest { request_hash: hash },
                        "blocklist-request",
                        self.sizes.blocklist_request(),
                        Transport::BackendNet,
                        flow,
                        run,
                    );
                }
            }
            ScmsMsg::BlocklistRequest { request_hash } => {
                let rec = self.ra.requests.get(&request_hash).copied();
                if let Some(r) = rec {
                    self.ra.blocklist.insert(r.device);
                    // The ECA must not issue the blocklisted device a successor either,
                    // or re-enrolment would undo the revocation.
                    out.send(
                        n.eca,
                        ScmsMsg::BlocklistNotice { device: r.device },
                        "blocklist-notice",
                        self.sizes.blocklist_notice(),
                        Transport::BackendNet,
                        flow,
                        run,
                    );
                }
                out.stage_at(StageId::Blocklisted, to, None, flow, run);
                out.send(
                    n.ma,
                    ScmsMsg::BlocklistResponse {
                        known: rec.is_some(),
                        lci1: rec.and_then(|r| r.lci1).unwrap_or(Lci(0)),
                        lci2: rec.and_then(|r| r.lci2).unwrap_or(Lci(0)),
                    },
                    "blocklist-response",
                    self.sizes.blocklist_response(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::BlocklistResponse { known, lci1, lci2 } => {
                if !known {
                    if let Some(case) = self.ma.case.as_mut() {
                        case.done = true;
                    }
                    return Ok(());
                }
                let Some(case) = self.ma.case.as_ref() else {
                    return Ok(());
                };
                let i_rev = case.i_rev;
                for (la, lci) in [(LaIndex::One, lci1), (LaIndex::Two, lci2)] {
                    out.send(
                        self.nodes.la(la),
                        ScmsMsg::SeedRequest { la, lci, i: i_rev },
                        "seed-request",
                        self.sizes.seed_request(),
                        Transport::BackendNet,
                        flow,
                        run,
                    );
                }
            }
            ScmsMsg::SeedRequest { la, lci, i } => {
                let st = &self.la[la.idx()];
                let Some(ls0) = st.chains.get(&lci).copied() else {
                    return Ok(());
                };
                // The LA releases `ls_x(i)` and not `ls_x(0)`. That single choice is what
                // makes revocation forward-only: nobody downstream can walk the chain
                // backwards, so the device's certificates from before period `i` stay
                // unlinkable ([BRECHT §VI-D]).
                Self::hashes(out, i);
                let seed = linkage::linkage_seed_at(st.la_id, ls0, i);
                let la_id = st.la_id;
                self.la[la.idx()].seeds_released += 1;
                out.send(
                    n.ma,
                    ScmsMsg::SeedResponse { la, la_id, seed, i },
                    "seed-response",
                    self.sizes.seed_response(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::SeedResponse { la, la_id, seed, i } => {
                let (ready, jmax, crl_run) = {
                    let Some(case) = self.ma.case.as_mut() else {
                        return Ok(());
                    };
                    case.seeds[la.idx()] = Some((la_id, seed));
                    (
                        case.seeds[0].is_some() && case.seeds[1].is_some(),
                        case.jmax,
                        case.crl_run,
                    )
                };
                if ready {
                    let case = self.ma.case.as_ref().expect("checked");
                    let (la1, la2) = (
                        case.seeds[0].expect("both present"),
                        case.seeds[1].expect("both present"),
                    );
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
            ScmsMsg::CrlAppend { i, la1, la2, jmax } => {
                let entry = CrlLinkageEntry {
                    i,
                    la_id1: la1.0,
                    la_id2: la2.0,
                    ls1_i: la1.1,
                    ls2_i: la2.1,
                    jmax,
                    max_forward: linkage::DEFAULT_MAX_FORWARD_PERIODS,
                };
                self.crlg.entries.push(entry);
                if let Some(case) = self.ma.case.as_mut() {
                    case.done = true;
                }
                Self::sign(out, 1);
                let entries = u32::try_from(self.crlg.entries.len()).unwrap_or(u32::MAX);
                out.stage_at(
                    StageId::Issued,
                    to,
                    Some(self.sizes.crl(entries).bytes()),
                    flow,
                    run,
                );
                let cadence = self.params.crl_cadence;
                if self.params.publish_on_cadence && cadence > Duration::ZERO {
                    // The list is re-issued on the cadence, not per entry: the entry waits
                    // for the next boundary of the calendar that starts at the epoch
                    // [BRECHT §VI-G], and every entry appended before it rides along.
                    if !self.crlg.publish_armed {
                        self.crlg.publish_armed = true;
                        let c = cadence.as_nanos().max(1);
                        let since = at.saturating_sub(self.params.epoch);
                        let next = (since / c + 1) * c;
                        out.start_timer(
                            Duration::from_nanos(next - since),
                            ScmsMsg::CrlPublishTimer,
                            flow,
                            run,
                        );
                    }
                    return Ok(());
                }
                self.crlg.signature = self.gov.sign_as("crlg", &crl_digest(&self.crlg.entries));
                self.crlg.versions += 1;
                // The store first, then the broadcast path, so that two links with the
                // same parameters stamp `published` before `first_rsu_broadcast`.
                out.send(
                    n.crl_store,
                    ScmsMsg::CrlPublish { entries },
                    "crl-publish",
                    self.sizes.crl(entries),
                    Transport::BackendNet,
                    flow,
                    run,
                );
                out.send(
                    n.crl_broadcast,
                    ScmsMsg::CrlPublish { entries },
                    "crl-broadcast",
                    self.sizes.crl(entries),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::CrlPublishTimer => {
                self.crlg.publish_armed = false;
                Self::sign(out, 1);
                self.crlg.signature = self.gov.sign_as("crlg", &crl_digest(&self.crlg.entries));
                self.crlg.versions += 1;
                let entries = u32::try_from(self.crlg.entries.len()).unwrap_or(u32::MAX);
                out.send(
                    n.crl_store,
                    ScmsMsg::CrlPublish { entries },
                    "crl-publish",
                    self.sizes.crl(entries),
                    Transport::BackendNet,
                    flow,
                    run,
                );
                out.send(
                    n.crl_broadcast,
                    ScmsMsg::CrlPublish { entries },
                    "crl-broadcast",
                    self.sizes.crl(entries),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::CrlPublish { entries } => {
                let list = self.crlg.entries.clone();
                let size = self.sizes.crl(entries).bytes();
                let signature = self.crlg.signature;
                if to == n.crl_store {
                    self.crl_store.entries = list;
                    self.crl_store.signature = signature;
                    self.crl_store.versions += 1;
                    out.stage_at(StageId::Published, to, Some(size), flow, run);
                } else {
                    self.crl_broadcast.entries = list;
                    self.crl_broadcast.signature = signature;
                    self.crl_broadcast.versions += 1;
                    out.stage_at(StageId::FirstRsuBroadcast, to, Some(size), flow, run);
                }
            }

            // ---------------- CRL distribution ----------------
            ScmsMsg::CrlDownloadRequest { device } if to == device => {
                out.send(
                    n.crl_store,
                    ScmsMsg::CrlDownloadRequest { device },
                    "crl-request",
                    self.sizes.crl_request(),
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::CrlDownloadRequest { device } => {
                let entries = u32::try_from(self.crl_store.entries.len()).unwrap_or(u32::MAX);
                out.send(
                    device,
                    ScmsMsg::CrlDownload { device, entries },
                    "crl-download",
                    self.sizes.crl(entries),
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::CrlAirBroadcast { device } => {
                // The roadside unit puts the list it holds on the air. It performs no
                // cryptography: the CRL Generator already signed it, and re-signing at
                // every RSU would be both wrong and a cost the deployment does not pay.
                let entries = u32::try_from(self.crl_broadcast.entries.len()).unwrap_or(u32::MAX);
                out.send(
                    device,
                    ScmsMsg::CrlDownload { device, entries },
                    "crl-air-broadcast",
                    self.sizes.crl(entries),
                    Transport::V2xAir,
                    flow,
                    run,
                );
            }
            ScmsMsg::CrlDownload { device, entries } => {
                let size = self.sizes.crl(entries).bytes();
                out.stage_at(StageId::Downloaded, device, Some(size), flow, run);
                // One signature verification over the list, then per entry: two SHA-256
                // per i-period walked and two AES per index searched ([ACPC §2],
                // [BRECHT §VII]).
                Self::verify(out, 1);
                // The device processes the list it *received*, from whichever path
                // delivered it. The two paths carry the same signed artefact, and reading
                // the store's copy on the broadcast path would make an RSU-only vehicle
                // silently enforce a CRL it never heard.
                let (list, signature) = if from == n.crl_broadcast {
                    (
                        self.crl_broadcast.entries.clone(),
                        self.crl_broadcast.signature,
                    )
                } else {
                    (self.crl_store.entries.clone(), self.crl_store.signature)
                };
                let Some(dev) = self.devices.get_mut(&device) else {
                    return Err(ProtoError::NoEntity { node: device });
                };
                // The device checks the CRL Generator's signature against the chain it
                // was given at bootstrap. An empty list has nothing to trust; a list that
                // does not verify is discarded whole.
                if !list.is_empty() {
                    let digest = crl_digest(&list);
                    let ok = signature.is_some_and(|(signer, sig)| {
                        dev.trust.verify_signed(signer, &digest, &sig)
                    });
                    if !ok {
                        dev.crls_rejected += 1;
                        return Ok(());
                    }
                }
                let periods: u32 = dev
                    .credentials
                    .keys()
                    .map(|&(i, _)| i)
                    .max()
                    .unwrap_or(0)
                    .saturating_add(1);
                let mut hashes = 0u32;
                let mut aes = 0u32;
                for e in &list {
                    let walked = periods.saturating_sub(e.i).min(e.max_forward);
                    hashes = hashes.saturating_add(2u32.saturating_mul(walked));
                    aes = aes
                        .saturating_add(2u32.saturating_mul(e.jmax).saturating_mul(walked.max(1)));
                }
                dev.crl = list;
                let silenced = !dev.revoked_credentials().is_empty();
                dev.silenced = silenced;
                Self::hashes(out, hashes);
                Self::aes(out, aes);
                out.stage_at(StageId::Processed, device, None, flow, run);
                out.stage_at(StageId::Enforced, device, None, flow, run);
                let _ = at;
            }

            // ---------------- local policy and chain files ----------------
            ScmsMsg::FileRequest { device, lpf, lccf } if to == n.lop => {
                self.lop.upstream += 1;
                out.send(
                    n.ra,
                    ScmsMsg::FileRequest { device, lpf, lccf },
                    "file-request-proxied",
                    self.sizes.file_request(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::FileRequest { device, lpf, lccf } => {
                // The RA serves what it has signed: the Local Policy File and the Local
                // Certificate Chain File, each only if newer than the device's.
                let newer_lpf = lpf
                    .is_none_or(|v| v < self.gov.lpf.version)
                    .then(|| Box::new(self.gov.lpf.clone()));
                let newer_lccf = lccf
                    .is_none_or(|v| v < self.gov.lccf.version)
                    .then(|| Box::new(self.gov.lccf.clone()));
                self.ra.files_served += u32::from(newer_lpf.is_some()) + u32::from(newer_lccf.is_some());
                Self::sign(out, 1);
                let size = self.sizes.file_response(
                    newer_lpf.is_some(),
                    newer_lccf
                        .as_ref()
                        .map(|c| u32::try_from(c.certs.len()).unwrap_or(0)),
                );
                out.send(
                    n.lop,
                    ScmsMsg::FileResponse {
                        device,
                        lpf: newer_lpf,
                        lccf: newer_lccf,
                    },
                    "file-response",
                    size,
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::FileResponse { device, lpf, lccf } if to == n.lop => {
                self.lop.downstream += 1;
                let size = self.sizes.file_response(
                    lpf.is_some(),
                    lccf.as_ref()
                        .map(|c| u32::try_from(c.certs.len()).unwrap_or(0)),
                );
                out.send(
                    device,
                    ScmsMsg::FileResponse { device, lpf, lccf },
                    "file-response-proxied",
                    size,
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::FileResponse { device, lpf, lccf } => {
                let Some(dev) = self.devices.get_mut(&device) else {
                    return Err(ProtoError::NoEntity { node: device });
                };
                let before = dev.trust.verifications;
                if let Some(chain) = lccf {
                    let _ = dev.trust.install_chain(&chain, at);
                }
                if let Some(policy) = lpf {
                    let _ = dev.trust.install_policy(&policy);
                }
                let checks = dev.trust.verifications.saturating_sub(before);
                Self::verify(out, checks);
            }
            ScmsMsg::ProvisioningRefused { device, reason } if to == n.lop => {
                self.lop.downstream += 1;
                out.send(
                    device,
                    ScmsMsg::ProvisioningRefused { device, reason },
                    "provisioning-refused-proxied",
                    self.sizes.refusal(),
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::ProvisioningRefused { device, reason } => {
                Self::verify(out, 1);
                if let Some(dev) = self.devices.get_mut(&device) {
                    dev.refused = Some(reason);
                    dev.refusals += 1;
                    dev.provisioning = None;
                }
            }

            // ---------------- successor enrolment ----------------
            ScmsMsg::SuccessorEnrolRequest { device, current } if to == device => {
                // A new key pair, and the request signed with the certificate it replaces.
                Self::scalar_mults(out, 1);
                Self::sign(out, 1);
                out.stage_at(StageId::Requested, device, None, flow, run);
                out.send(
                    n.eca,
                    ScmsMsg::SuccessorEnrolRequest { device, current },
                    "successor-enrol-request",
                    self.sizes.successor_request(),
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::SuccessorEnrolRequest { device, current } => {
                Self::verify(out, 1);
                // The ECA renews only a certificate it issued, that is still valid — an
                // expired one means the device goes back to its bootstrap channel — and
                // that the RA has not blocklisted.
                let reason = if self.eca.blocklist.contains(&device) {
                    Some(Refusal::Blocklisted)
                } else if !self.issued_enrolment.contains(&device) {
                    Some(Refusal::UnknownEnrolment)
                } else if !current.valid_at(at) {
                    Some(Refusal::EnrolmentExpired)
                } else {
                    None
                };
                Self::sign(out, 1);
                if let Some(reason) = reason {
                    self.eca.refused += 1;
                    out.send(
                        device,
                        ScmsMsg::EnrolRefused { device, reason },
                        "enrol-refused",
                        self.sizes.refusal(),
                        Transport::CellularUu,
                        flow,
                        run,
                    );
                    return Ok(());
                }
                self.eca.issued += 1;
                self.eca.successors += 1;
                let cert = EnrolmentCert {
                    generation: current.generation + 1,
                    valid_from: at,
                    valid_until: self.params.enrolment_lifetime.after(at),
                };
                out.stage_at(StageId::Certified, to, None, flow, run);
                out.send(
                    device,
                    ScmsMsg::SuccessorEnrolResponse { device, cert },
                    "successor-enrol-response",
                    self.sizes.enrol_response(),
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::SuccessorEnrolResponse { device, cert } => {
                Self::verify(out, 1);
                if let Some(dev) = self.devices.get_mut(&device) {
                    dev.enrolment = Some(cert);
                    dev.reenrolments += 1;
                    dev.refused = None;
                }
                out.stage_at(StageId::Installed, device, None, flow, run);
            }
            ScmsMsg::EnrolRefused { device, reason } => {
                Self::verify(out, 1);
                if let Some(dev) = self.devices.get_mut(&device) {
                    dev.refused = Some(reason);
                    dev.refusals += 1;
                }
            }
            ScmsMsg::BlocklistNotice { device } => {
                Self::verify(out, 1);
                self.eca.blocklist.insert(device);
            }

            // ---------------- policy ----------------
            ScmsMsg::PolicyUpdate(policy) if to == self.gov.nodes.manager => {
                self.gov.counters.manager_decisions += 1;
                Self::sign(out, 1);
                out.stage_at(StageId::Decision, to, None, flow, run);
                out.send(
                    self.gov.nodes.pg,
                    ScmsMsg::PolicyUpdate(policy),
                    "policy-update",
                    self.sizes.policy_file(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::PolicyUpdate(policy) if to == self.gov.nodes.pg => {
                Self::verify(out, 1);
                Self::sign(out, 2);
                self.gov.sign_global(*policy);
                out.stage_at(StageId::Issued, to, None, flow, run);
                out.send(
                    n.ra,
                    ScmsMsg::PolicyUpdate(policy),
                    "policy-publish",
                    self.sizes.policy_file(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::PolicyUpdate(_) => {
                Self::verify(out, 1);
                Self::sign(out, 2);
                self.gov.sign_local();
                out.stage_at(StageId::Published, to, None, flow, run);
            }
        }
        Ok(())
    }
}

/// The hash the RA knows a provisioning request by.
///
/// Over the request's public material, so two different requests cannot collide and the
/// same request replayed is recognised — the "one request per period" check of
/// [CAMP-EE §2.2.7.6].
fn request_hash(req: &ProvisioningRequest) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(128);
    bytes.extend_from_slice(&req.device.index().to_be_bytes());
    if let Some(a) = req.signing_public.compressed() {
        bytes.extend_from_slice(&a);
    }
    if let Some(p) = req.encryption_public.compressed() {
        bytes.extend_from_slice(&p);
    }
    bytes.extend_from_slice(&req.ck);
    bytes.extend_from_slice(&req.ek);
    bytes.extend_from_slice(&req.start_i.to_be_bytes());
    bytes.extend_from_slice(&req.periods.to_be_bytes());
    bytes.extend_from_slice(&req.jmax.to_be_bytes());
    sha256(&bytes)
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

/// The size, in bytes, of a CRL with `entries` entries under this deployment's size model.
pub fn crl_bytes(sizes: &ScmsSizes, entries: u32) -> WireSize {
    sizes.crl(entries)
}

/// The links the governance entities need: the SCMS Manager to the Policy Generator, the
/// Policy Generator to the Registration Authority it publishes to.
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
