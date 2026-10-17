//! Enrolment at the factory (device → DCM → ECA) and successor enrolment through the RA.
//!
//! Split out of `run.rs` without behaviour change (WP-B1); the state types and the shared
//! helpers stay in [`super`].

use super::*;

impl ScmsRun {
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
        if let Some(d) = self.state.devices.get_mut(&device) {
            d.reenrol_refused = None;
        }
        self.inject_at(
            at,
            device,
            ScmsMsg::SuccessorEnrolRequest { device, current },
            FlowId::Reenrolment,
            run,
        );
        run
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
}

impl ScmsState {
    /// Handles every message of this flow; [`ScmsState::handle`] routes here.
    #[allow(unused_variables)]
    pub(super) fn handle_enrol(
        &mut self,
        d: Delivery<ScmsMsg>,
        out: &mut Outbox<ScmsMsg>,
    ) -> Result<()> {
        let n = self.nodes;
        let (flow, run, to, at, from) = (d.flow, d.run, d.to, d.at, d.from);
        match d.msg {
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
                    dev.reenrol_refused = Some(reason);
                    dev.refusals += 1;
                }
            }
            _ => unreachable!("ScmsState::handle routes only this flow's messages here"),
        }
        Ok(())
    }
}
