//! Enrolment at the factory (device → DCM → ECA) and successor enrolment through the RA.
//!
//! Split out of `run.rs` without behaviour change (WP-B1); the state types and the shared
//! helpers stay in [`super`].

use super::*;

use crate::scms::pdu::{self, Pdu, PduError, RequestSigner, RequestSignerFound, SpduKind};
use v2xw_msg::sec_types::ieee1609_dot2::VerificationKeyIndicator;
use v2xw_msg::sec_types::ieee1609_dot2_base_types::PublicVerificationKey;
use v2xw_msg::sec_types::ieee1609_dot2_dot1_eca_ee_interface::EcaEeInterfacePdu;
use v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::ScmsPduContent;
use v2xw_sec::sig1609::Signer1609;

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
            ScmsMsg::EnrolRequest { device, pdu: None },
            FlowId::Enrolment,
            run,
        );
        run
    }
}

impl ScmsState {
    /// The enrolment certificate lifetime in 1609.2 `Duration` hours.
    fn enrolment_hours(&self) -> u16 {
        u16::try_from(self.params.enrolment_lifetime.as_nanos() / 3_600_000_000_000)
            .unwrap_or(u16::MAX)
    }

    /// Device side: a fresh ECQV request key `k_U` and the self-signed
    /// `EeEcaCertRequestSpdu` on `R_U = k_U·G`.
    fn enrol_request_pdu(&mut self, device: NodeId, at: SimTime) -> Option<Pdu> {
        self.pki.as_ref()?;
        let k_u = self.pdu_rng.scalar();
        let t = self.time32(at);
        let p =
            pdu::ee_eca_cert_request(t, &Point::mul_base(&k_u), t, self.enrolment_hours()).ok()?;
        let key = Signer1609::p256(&ec::scalar_to_be32(&k_u))?;
        let pdu = Pdu::new(pdu::signed_cert_request(&p, RequestSigner::SelfSigned(&key)).ok()?);
        self.devices.get_mut(&device)?.enrol_request = Some((k_u, pdu.clone()));
        Some(pdu)
    }

    /// ECA side: verify the self-signed request, issue on `R_U` with the request's
    /// `toBeSigned`, and answer with the ECA-signed `EcaEeCertResponseSpdu`.
    fn eca_issue(&mut self, at: SimTime, req: &Pdu) -> pdu::Result<Pdu> {
        let pki = self.pki.clone().ok_or(PduError::Shape("PDU layer off"))?;
        let (p, signer) = pdu::open_signed_cert_request(req.bytes(), &pki)?;
        if !matches!(signer, RequestSignerFound::SelfSigned) {
            return Err(PduError::Signer("an enrolment request is SignerSelf"));
        }
        let ScmsPduContent::eca_ee(EcaEeInterfacePdu::eeEcaCertRequest(r)) = &p.content else {
            return Err(PduError::Shape("expected eca-ee.eeEcaCertRequest"));
        };
        let VerificationKeyIndicator::verificationKey(PublicVerificationKey::ecdsaNistP256(pt)) =
            &r.tbs_cert.verify_key_indicator
        else {
            return Err(PduError::Shape(
                "request key is not a P-256 verificationKey",
            ));
        };
        let r_u = pdu::point_of(pt)?;
        let t = self.time32(at);
        let (cert, contribution) =
            pdu::issue_enrolment(&pki, &r.tbs_cert, &r_u, &mut self.pdu_rng)?;
        let resp = pdu::eca_ee_cert_response(t, req.hashed_id8(), &pki, cert, &contribution);
        Ok(Pdu::new(pdu::signed(&resp, &pki.eca.cert, &pki.eca.key)?))
    }

    /// Device side: open the ECA's response and install the reconstructed credential.
    fn ee_install_enrolment(&mut self, device: NodeId, resp: &Pdu) -> pdu::Result<()> {
        let pki = self.pki.clone().ok_or(PduError::Shape("PDU layer off"))?;
        let (p, signer) = pdu::open_signed_pdu(resp.bytes(), &pki)?;
        if pdu::cert_id8(&signer) != pki.eca.id8() {
            return Err(PduError::Signer(
                "an enrolment response is signed by the ECA",
            ));
        }
        let ScmsPduContent::eca_ee(EcaEeInterfacePdu::ecaEeCertResponse(r)) = p.content else {
            return Err(PduError::Shape("expected eca-ee.ecaEeCertResponse"));
        };
        let dev = self
            .devices
            .get_mut(&device)
            .ok_or(PduError::Shape("no such device"))?;
        let Some((k_u, sent)) = dev.enrol_request.clone() else {
            return Err(PduError::Shape("no enrolment request in flight"));
        };
        if v2xw_sec::hashedid::id8_bytes(&r.request_hash) != sent.hashed_id8() {
            return Err(PduError::Shape(
                "requestHash is not the hash of the request sent",
            ));
        }
        let pki_bytes: [u8; 32] = r
            .private_key_info
            .as_deref()
            .and_then(|b| b.try_into().ok())
            .ok_or(PduError::Shape("privateKeyInfo is 32 bytes"))?;
        let contribution =
            ec::scalar_from_be32(&pki_bytes).map_err(|_| PduError::Key("privateKeyInfo"))?;
        let cred = pdu::reconstruct_enrolment(&pki, &r.certificate, &k_u, &contribution)?;
        dev.enrol_request = None;
        dev.enrolment_credential = Some(cred);
        Ok(())
    }

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
            ScmsMsg::EnrolRequest { device, .. } if to == device => {
                Self::sign(out, 1);
                out.stage_at(StageId::Requested, device, None, flow, run);
                let pdu = self.enrol_request_pdu(device, at);
                let size = match &pdu {
                    Some(p) => self.emit_pdu(SpduKind::EeEcaCertRequest, device, n.dcm, at, p),
                    None => self.sizes.enrol_request(),
                };
                out.send(
                    n.dcm,
                    ScmsMsg::EnrolRequest { device, pdu },
                    "enrol-request",
                    size,
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::EnrolRequest { device, pdu } => {
                // Device Configuration Manager: checks the device is one it configured,
                // re-signs and forwards to the Enrolment CA.
                Self::verify(out, 1);
                Self::sign(out, 1);
                out.stage_at(StageId::ProxyForwarded, to, None, flow, run);
                out.send(
                    n.eca,
                    ScmsMsg::EnrolForward { device, pdu },
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
            ScmsMsg::EnrolForward { device, pdu } => {
                Self::verify(out, 1);
                Self::sign(out, 1);
                // The real request: the ECA verifies the self-signature and issues an
                // implicit certificate on the request key. A request that does not verify
                // is refused, and the refusal is recorded.
                let response = match pdu {
                    Some(req) if self.pki.is_some() => match self.eca_issue(at, &req) {
                        Ok(resp) => Some(resp),
                        Err(e) => {
                            self.reject_pdu(SpduKind::EeEcaCertRequest, to, at, e);
                            return Ok(());
                        }
                    },
                    _ => None,
                };
                self.issued_enrolment.insert(device);
                self.eca.issued += 1;
                let cert = EnrolmentCert {
                    generation: 0,
                    valid_from: at,
                    valid_until: self.params.enrolment_lifetime.after(at),
                };
                out.stage_at(StageId::Certified, to, None, flow, run);
                let size = match &response {
                    Some(p) => self.emit_pdu(SpduKind::EcaEeCertResponse, to, device, at, p),
                    None => self.sizes.enrol_response(),
                };
                out.send(
                    device,
                    ScmsMsg::EnrolResponse {
                        device,
                        cert,
                        pdu: response,
                    },
                    "enrol-response",
                    size,
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::EnrolResponse { device, cert, pdu } => {
                Self::verify(out, 2);
                // The device verifies the ECA's signature and the chain, matches the
                // requestHash to its own request, reconstructs its private key and checks
                // it against the certificate before installing anything.
                if let Some(resp) = pdu {
                    if let Err(e) = self.ee_install_enrolment(device, &resp) {
                        self.reject_pdu(SpduKind::EcaEeCertResponse, device, at, e);
                        return Ok(());
                    }
                }
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
