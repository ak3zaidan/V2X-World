//! CRL issuance (CRL Generator) and distribution (RA download, air broadcast).
//!
//! Split out of `run.rs` without behaviour change (WP-B1); the state types and the shared
//! helpers stay in [`super`].

use super::*;

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

/// The size, in bytes, of a CRL with `entries` entries under this deployment's size model.
pub fn crl_bytes(sizes: &ScmsSizes, entries: u32) -> WireSize {
    sizes.crl(entries)
}

impl ScmsRun {
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

    /// The single entry on the published CRL, for a test that needs to inspect it.
    pub fn crl_entry(&self) -> Option<&v2xw_sec::linkage::CrlLinkageEntry> {
        self.state.crl_store.entries.first()
    }
}

impl ScmsState {
    /// Handles every message of this flow; [`ScmsState::handle`] routes here.
    #[allow(unused_variables)]
    pub(super) fn handle_crl(
        &mut self,
        d: Delivery<ScmsMsg>,
        out: &mut Outbox<ScmsMsg>,
    ) -> Result<()> {
        let n = self.nodes;
        let (flow, run, to, at, from) = (d.flow, d.run, d.to, d.at, d.from);
        match d.msg {
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
                let crl_committee = self.committees.role("crl");
                if crl_committee.is_none() {
                    Self::sign(out, 1);
                }
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
                if let Some(role) = crl_committee {
                    // A t-of-n CRL Generator: the list is published once the committee
                    // has signed it.
                    self.committees.start(
                        role,
                        1,
                        Pending::PublishCrl { flow, run },
                        at,
                        flow,
                        run,
                        out,
                        wrap_committee,
                    );
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
                if let Some(role) = self.committees.role("crl") {
                    self.committees.start(
                        role,
                        1,
                        Pending::PublishCrl { flow, run },
                        at,
                        flow,
                        run,
                        out,
                        wrap_committee,
                    );
                    return Ok(());
                }
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
            _ => unreachable!("ScmsState::handle routes only this flow's messages here"),
        }
        Ok(())
    }
}
