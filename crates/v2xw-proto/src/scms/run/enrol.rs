//! Enrolment at the factory (device → DCM → ECA → DCM → device, then registration with
//! the RA) and successor enrolment through the RA (WP-B2).
//!
//! **Bootstrap** ([SMP] §2.1.7–2.1.8, 01 §5, 03 §5). In the secure environment the device
//! makes its canonical key, and the EE operator registers the canonical public key with
//! the ECA. The device's `EeEcaCertRequestSpdu` carries `canonicalId`, is signed with the
//! canonical key, and asks for an implicit certificate under the US profile
//! ([`crate::scms::enrolment`]). The ECA checks the profile, that the request key was
//! never used before and that the name is unique, and issues. The DCM relays the
//! `EcaEeCertResponseSpdu`, installs the elector anchors, CTL, chain file (root, ICA, RA,
//! MA, …), policy and RA URL, and registers the new enrolment certificate with the RA,
//! which serves only registered certificates.
//!
//! **Successor enrolment** ([SM-REENR] §1–3, 03 §3.2). `reenrol_lead` before expiry the
//! device sends `EeRaSuccessorEnrollmentCertRequestSpdu` to the RA. The RA decrypts,
//! verifies the current enrolment certificate's signature and its registration, answers
//! with `RaEeEnrollmentCertAckSpdu` (`nextDlTime`) and forwards the inner request to the
//! ECA. The ECA issues a successor that keeps the name, region and permissions and starts
//! exactly when the current certificate ends. The RA registers it (linking old and new)
//! and hands it over when the device downloads `<requestHash>.zip`. The device holds the
//! successor until the current certificate ends ([`DeviceState::roll_enrolment`]). A second
//! request before the download supersedes the first, and a successor already issued for
//! the first is blocklisted.

use super::*;

use crate::scms::enrolment::{
    self as us, EnrolAuth, EnrolmentRequest, RA_URL, US_ENROLMENT_REGIONS,
};
use crate::scms::pdu::{self, Pdu, PduError, RequestSigner, SpduKind};
use v2xw_msg::sec_types::ieee1609_dot2::Certificate;
use v2xw_msg::sec_types::ieee1609_dot2_dot1_eca_ee_interface::EcaEeInterfacePdu;
use v2xw_msg::sec_types::ieee1609_dot2_dot1_ee_ra_interface::EeRaInterfacePdu;
use v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::ScmsPduContent;
use v2xw_sec::sig1609::Signer1609;

impl ScmsRun {
    /// Replaces a device's enrolment certificate: how a driver states the age of the
    /// certificate a vehicle on the road already holds (it was issued before the run, at
    /// the factory or at its last renewal), so that a run long enough — or a lifecycle
    /// compressed enough — sees it renewed. With the PDU layer on, the real certificate is
    /// re-issued out of band with the same validity, so the bytes and the model agree.
    pub fn set_enrolment(&mut self, device: NodeId, cert: EnrolmentCert) {
        let cred = self.state.issue_out_of_band(device, cert, false);
        if let Some(d) = self.state.devices.get_mut(&device) {
            d.enrolment = Some(cert);
            d.successor = None;
            d.successor_credential = None;
            if cred.is_some() {
                d.enrolment_credential = cred;
            }
        }
    }

    /// States that a device already holds its successor enrolment certificate (it
    /// re-enrolled before the run, inside the lead): `cert` takes over when the current
    /// certificate ends.
    pub fn set_successor_enrolment(&mut self, device: NodeId, cert: EnrolmentCert) {
        let cred = self.state.issue_out_of_band(device, cert, true);
        if let Some(d) = self.state.devices.get_mut(&device) {
            d.successor = Some(cert);
            d.successor_credential = cred;
        }
    }

    /// States the age of the enrolment certificate `device` holds when it joins, before
    /// its pre-run enrolment ([`ScmsRun::preload`]): the certificate ends at `until` and
    /// was issued one full lifetime earlier. The device's one ECA exchange then asks for,
    /// and the ECA issues, a certificate with that back-dated start and the profile's
    /// whole lifetime ([CAMP122] Table 3, six years); the DCM registers it and it signs
    /// every Download-Req (OF-1: the age used to be a second certificate, issued out of
    /// band with a shortened lifetime, which replaced the one the exchange issued). A
    /// start before the issuing ECA's own is moved up to it (the ECA issues only inside
    /// its validity, [SMP] §6.3.1), which moves the end later by as much. Returns the
    /// model certificate, whose `valid_until` is the end in force.
    pub fn age_enrolment(&mut self, device: NodeId, until: SimTime) -> EnrolmentCert {
        const NS: u64 = 1_000_000_000;
        if !self.state.devices.contains_key(&device) {
            self.add_device(device);
        }
        let st = &mut self.state;
        let life_ns = st.params.enrolment_lifetime.as_nanos();
        let life_s = life_ns / NS;
        let eca_start = st
            .pki
            .as_ref()
            .map_or(0, |p| v2xw_sec::ctl::validity(&p.eca.cert).0);
        let end = u64::from(st.time32(until));
        let start = end.saturating_sub(life_s).max(eca_start);
        let until = until.saturating_add((start + life_s).saturating_sub(end) * NS);
        let cert = EnrolmentCert {
            generation: 0,
            valid_from: until.saturating_sub(life_ns),
            valid_until: until,
        };
        if let Some(d) = st.devices.get_mut(&device) {
            d.enrolment_age = Some((cert, u32::try_from(start).unwrap_or(u32::MAX)));
        }
        cert
    }

    /// Starts IEEE 1609.2.1's successor enrolment at `at`: the device asks the RA for a
    /// new enrolment certificate, signing with the one it holds.
    pub fn reenrol_at(&mut self, device: NodeId, at: SimTime) -> FlowRun {
        let run = self.new_run();
        let at = at.max(self.kernel.now());
        let current = self
            .state
            .devices
            .get_mut(&device)
            .and_then(|d| d.roll_enrolment(at))
            .unwrap_or(EnrolmentCert {
                generation: 0,
                valid_from: 0,
                valid_until: 0,
            });
        if let Some(d) = self.state.devices.get_mut(&device) {
            d.reenrol_refused = None;
        }
        self.inject_at(
            at,
            device,
            ScmsMsg::SuccessorEnrolRequest {
                device,
                current,
                pdu: None,
            },
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

/// The `id.name` a device's enrolment certificate carries: unique per device and
/// provider (neutral label).
fn device_name(device: NodeId) -> String {
    format!("ee-{:08x}.provider-a", device.index())
}

fn signer_of(k: &p256::Scalar) -> Option<Signer1609> {
    Signer1609::p256(&ec::scalar_to_be32(k))
}

impl ScmsState {
    fn lifetime_s(&self) -> u64 {
        self.params.enrolment_lifetime.as_nanos() / 1_000_000_000
    }

    /// The ECA's records of what it issued and the RA's registration, for an issuance
    /// made outside the run (the factory or a renewal before the run): a real implicit
    /// certificate with `cert`'s validity, on a fresh key. `None` when the layer is off or
    /// the device never enrolled through the flow.
    fn issue_out_of_band(
        &mut self,
        device: NodeId,
        cert: EnrolmentCert,
        successor: bool,
    ) -> Option<pdu::EnrolmentCredential> {
        let pki = self.pki.clone()?;
        let name = self.devices.get(&device)?.name.clone()?;
        let end = u64::from(self.wall.time32(cert.valid_until).ok()?);
        let life = (cert.valid_until.saturating_sub(cert.valid_from)) / 1_000_000_000;
        let start = u32::try_from(end.saturating_sub(life)).ok()?;
        let k_u = self.pdu_rng.scalar();
        let r_u = Point::mul_base(&k_u);
        let req = EnrolmentRequest {
            name: &name,
            regions: &US_ENROLMENT_REGIONS,
            valid_from: start,
            lifetime_s: life,
        };
        let tbs = us::us_enrolment_tbs(&req, &r_u).ok()?;
        let (c, r) = pdu::issue_enrolment(&pki, &tbs, &r_u, &mut self.pdu_rng).ok()?;
        let cred = pdu::reconstruct_enrolment(&pki, &c, &k_u, &r).ok()?;
        if let Some(k) = r_u.compressed() {
            self.eca.seen_keys.insert(k);
        }
        let reg = self.ra.registered.entry(device).or_default();
        if !successor {
            reg.clear();
        }
        reg.insert(pdu::cert_id8(&cred.cert));
        Some(cred)
    }

    /// Device side, in the secure environment: the canonical key (made once, and its
    /// public half registered with the ECA by the EE operator, out of band), a fresh ECQV
    /// request key `k_U`, and the canonical-signed `EeEcaCertRequestSpdu` for the US
    /// profile on `R_U = k_U·G`.
    fn enrol_request_pdu(&mut self, device: NodeId, at: SimTime) -> Option<Pdu> {
        self.pki.as_ref()?;
        let c = match self.devices.get(&device)?.canonical {
            Some(c) => c,
            None => self.pdu_rng.scalar(),
        };
        let c_pub = Point::mul_base(&c);
        let cid = us::canonical_id(&c_pub).ok()?;
        self.eca.canonical.insert(cid.clone(), c_pub);
        let k_u = self.pdu_rng.scalar();
        let t = self.time32(at);
        let name = device_name(device);
        // A device that joins with an aged certificate asks for its back-dated start
        // ([`ScmsRun::age_enrolment`]); a new one for a certificate valid from now.
        let start = self
            .devices
            .get(&device)
            .and_then(|d| d.enrolment_age)
            .map_or(t, |(_, s)| s);
        let req = EnrolmentRequest {
            name: &name,
            regions: &US_ENROLMENT_REGIONS,
            valid_from: start,
            lifetime_s: self.lifetime_s(),
        };
        let p = us::ee_eca_cert_request_us(t, &Point::mul_base(&k_u), &req, Some(&cid)).ok()?;
        let key = signer_of(&c)?;
        let pdu = Pdu::new(pdu::signed_cert_request(&p, RequestSigner::SelfSigned(&key)).ok()?);
        let dev = self.devices.get_mut(&device)?;
        dev.canonical = Some(c);
        dev.name = Some(name);
        dev.enrol_request = Some((k_u, pdu.clone()));
        Some(pdu)
    }

    /// ECA side, shared by the first enrolment and the successor: open and verify the
    /// request, apply the US profile and the ECA's records (key never used before, name
    /// unique or the device's own), and issue on `R_U`. `current` is the certificate a
    /// successor must follow. Returns the ECA-signed `EcaEeCertResponseSpdu`.
    fn eca_issue(
        &mut self,
        at: SimTime,
        device: NodeId,
        req: &Pdu,
        current: Option<&Certificate>,
    ) -> pdu::Result<Pdu> {
        let pki = self.pki.clone().ok_or(PduError::Shape("PDU layer off"))?;
        let canonical = &self.eca.canonical;
        // The first request comes through the DCM from the secure environment, signed by
        // the canonical key (direct authorisation) or self-signed on the DCM's word
        // (indirect); either verifies here. A successor is always self-signed.
        let (r, _auth): (_, EnrolAuth) =
            us::open_ee_eca_cert_request(req.bytes(), &|id| canonical.get(id).copied())?;
        let t = self.time32(at);
        us::check_us_profile(&r, &pki.eca.cert, t)?;
        if let Some(cur) = current {
            us::check_successor(&r, cur)?;
        }
        let r_u = us::request_key(&r)?;
        let key = r_u.compressed().ok_or(PduError::Key("request key"))?;
        if self.eca.seen_keys.contains(&key) {
            return Err(PduError::Policy(
                "the request key was used before with this ECA",
            ));
        }
        let name = us::tbs_name(&r.tbs_cert)
            .ok_or(PduError::Policy("id.name is present"))?
            .to_string();
        match self.eca.names.get(&name) {
            Some(d) if *d != device => {
                return Err(PduError::Policy("id.name is unique at the ECA"));
            }
            Some(_) if current.is_none() => {
                return Err(PduError::Policy(
                    "id.name is unique at the ECA (a renewal is a successor request)",
                ));
            }
            _ => {}
        }
        let (cert, contribution) =
            pdu::issue_enrolment(&pki, &r.tbs_cert, &r_u, &mut self.pdu_rng)?;
        self.eca.seen_keys.insert(key);
        self.eca.names.insert(name, device);
        let resp = pdu::eca_ee_cert_response(t, req.hashed_id8(), &pki, cert, &contribution);
        Ok(Pdu::new(pdu::signed(&resp, &pki.eca.cert, &pki.eca.key)?))
    }

    /// Opens an ECA response: signed by the ECA, its `requestHash` the request `sent`, the
    /// key reconstructed from `privateKeyInfo` with `k_u` and checked.
    fn open_eca_response(
        &self,
        resp: &Pdu,
        k_u: &p256::Scalar,
        sent: [u8; 8],
    ) -> pdu::Result<pdu::EnrolmentCredential> {
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
        if v2xw_sec::hashedid::id8_bytes(&r.request_hash) != sent {
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
        pdu::reconstruct_enrolment(&pki, &r.certificate, k_u, &contribution)
    }

    /// The enrolment certificate an ECA response carries (the RA's and DCM's view: they
    /// verify the ECA's signature but hold no key to reconstruct).
    fn response_cert_id8(&self, resp: &Pdu) -> pdu::Result<[u8; 8]> {
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
        Ok(pdu::cert_id8(&r.certificate))
    }

    /// Device side: open the ECA's response and install the reconstructed credential.
    fn ee_install_enrolment(&mut self, device: NodeId, resp: &Pdu) -> pdu::Result<()> {
        let Some((k_u, sent)) = self
            .devices
            .get(&device)
            .and_then(|d| d.enrol_request.clone())
        else {
            return Err(PduError::Shape("no enrolment request in flight"));
        };
        let cred = self.open_eca_response(resp, &k_u, sent.hashed_id8())?;
        let dev = self
            .devices
            .get_mut(&device)
            .ok_or(PduError::Shape("no such device"))?;
        dev.enrol_request = None;
        dev.enrolment_credential = Some(cred);
        Ok(())
    }

    /// Device side: the `EeRaSuccessorEnrollmentCertRequestSpdu` — a new key, the inner
    /// request for a successor that starts when the current certificate ends, self-signed
    /// by the new key, then signed by the current enrolment certificate and encrypted to
    /// the RA.
    fn successor_request_pdu(&mut self, device: NodeId, at: SimTime, run: FlowRun) -> Option<Pdu> {
        let pki = self.pki.clone()?;
        let cred = self.devices.get(&device)?.enrolment_credential.clone()?;
        let name = us::tbs_name(&cred.cert.0.to_be_signed)?.to_string();
        let (_, end) = v2xw_sec::ctl::validity(&cred.cert);
        let k_new = self.pdu_rng.scalar();
        let t = self.time32(at);
        let req = EnrolmentRequest {
            name: &name,
            regions: &US_ENROLMENT_REGIONS,
            valid_from: u32::try_from(end).ok()?,
            lifetime_s: self.lifetime_s(),
        };
        let p = us::ee_eca_cert_request_us(t, &Point::mul_base(&k_new), &req, None).ok()?;
        let inner = Pdu::new(
            pdu::signed_cert_request(&p, RequestSigner::SelfSigned(&signer_of(&k_new)?)).ok()?,
        );
        let outer = us::ee_ra_successor_request(inner.bytes()).ok()?;
        let ra_key = pki.ra.enc_public()?;
        let spdu = Pdu::new(
            pdu::signed_encrypted_cert_request(
                &outer,
                RequestSigner::Cert(&cred.cert, &cred.key),
                &pki.ra.cert,
                &ra_key,
                &mut self.pdu_rng,
            )
            .ok()?,
        );
        let dev = self.devices.get_mut(&device)?;
        dev.successor_request = Some((k_new, inner, spdu.hashed_id8()));
        dev.successor_run = Some(run);
        Some(spdu)
    }

    /// RA side: the RA-signed `RaEeEnrollmentCertAckSpdu` (`raEeCertInfo`, `currentI` 0,
    /// the request hash, `nextDlTime`).
    fn ra_successor_ack(&mut self, hash8: [u8; 8], at: SimTime, next: SimTime) -> Option<Pdu> {
        let pki = self.pki.clone()?;
        let info = pdu::ra_ee_cert_info(self.time32(at), 0, hash8, self.time32(next));
        Some(Pdu::new(
            pdu::signed(&info, &pki.ra.cert, &pki.ra.key).ok()?,
        ))
    }

    /// Device side: the `EeRaDownloadRequestSpdu` for `<requestHash>.zip`, signed by the
    /// current enrolment certificate and encrypted to the RA.
    fn successor_download_pdu(&mut self, device: NodeId, at: SimTime) -> Option<Pdu> {
        let pki = self.pki.clone()?;
        let dev = self.devices.get(&device)?;
        let cred = dev.enrolment_credential.clone()?;
        let hash8 = dev.successor_request.as_ref()?.2;
        let p = pdu::ee_ra_download_request(self.time32(at), &us::successor_filename(hash8));
        let ra_key = pki.ra.enc_public()?;
        Some(Pdu::new(
            pdu::signed_encrypted(
                &p,
                &cred.cert,
                &cred.key,
                &pki.ra.cert,
                &ra_key,
                &mut self.pdu_rng,
            )
            .ok()?,
        ))
    }

    /// RA side: the download request decrypts, verifies under a registered enrolment
    /// certificate of `device`, and names the file of its successor request.
    fn ra_open_download(&self, device: NodeId, spdu: &Pdu, hash8: [u8; 8]) -> pdu::Result<()> {
        let pki = self.pki.clone().ok_or(PduError::Shape("PDU layer off"))?;
        let enc = pki
            .ra
            .enc_private
            .ok_or(PduError::Key("RA encryption key"))?;
        let plain = pdu::decrypt_cert(spdu.bytes(), &pki.ra.cert, &enc)?;
        let (p, signer) = pdu::open_signed_pdu(&plain, &pki)?;
        if !self.ra.is_registered(device, Some(pdu::cert_id8(&signer))) {
            return Err(PduError::UntrustedSigner);
        }
        let ScmsPduContent::ee_ra(EeRaInterfacePdu::eeRaDownloadRequest(r)) = p.content else {
            return Err(PduError::Shape("expected ee-ra.eeRaDownloadRequest"));
        };
        if r.filename.as_str() != us::successor_filename(hash8) {
            return Err(PduError::Shape("the file is not this device's successor"));
        }
        Ok(())
    }

    /// Sends `device`'s refusal to `dest` (the device itself, or the DCM or RA that
    /// relays it).
    #[allow(clippy::too_many_arguments)]
    fn refuse_successor(
        &mut self,
        out: &mut Outbox<ScmsMsg>,
        dest: NodeId,
        device: NodeId,
        reason: Refusal,
        flow: FlowId,
        run: FlowRun,
    ) {
        Self::sign(out, 1);
        let transport = if self.devices.contains_key(&dest) {
            Transport::CellularUu
        } else {
            Transport::BackendNet
        };
        out.send(
            dest,
            ScmsMsg::EnrolRefused { device, reason },
            "enrol-refused",
            self.sizes.refusal(),
            transport,
            flow,
            run,
        );
    }

    /// Handles every message of this flow; [`ScmsState::handle`] routes here.
    #[allow(unused_variables, clippy::too_many_lines)]
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
                // anchors, the trust list, the Local Certificate Chain File (root, ICA,
                // RA, MA, …), the Local Policy File and the RA's URL, without which it
                // could verify nothing it will receive.
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
                dev.ra_url = Some(RA_URL.to_string());
                let checks = dev.trust.verifications.saturating_sub(before);
                Self::verify(out, checks);
            }
            ScmsMsg::EnrolForward { device, pdu } => {
                Self::verify(out, 1);
                Self::sign(out, 1);
                // The real request: the ECA verifies the canonical signature, applies the
                // US profile and its records, and issues an implicit certificate on the
                // request key. A refused request is recorded and answered.
                let response = match pdu {
                    Some(req) if self.pki.is_some() => {
                        match self.eca_issue(at, device, &req, None) {
                            Ok(resp) => Some(resp),
                            Err(e) => {
                                let policy = matches!(e, PduError::Policy(_));
                                self.reject_pdu(SpduKind::EeEcaCertRequest, to, at, e);
                                if policy {
                                    self.eca.profile_refused += 1;
                                    self.refuse_successor(
                                        out,
                                        n.dcm,
                                        device,
                                        Refusal::ProfileViolation,
                                        flow,
                                        run,
                                    );
                                }
                                return Ok(());
                            }
                        }
                    }
                    _ => None,
                };
                self.issued_enrolment.insert(device);
                self.eca.issued += 1;
                // The certificate the request asked for: an aged one's back-dated
                // validity ([`ScmsRun::age_enrolment`]), otherwise from now.
                let aged = self
                    .devices
                    .get_mut(&device)
                    .and_then(|d| d.enrolment_age.take());
                let cert = aged.map_or(
                    EnrolmentCert {
                        generation: 0,
                        valid_from: at,
                        valid_until: self.params.enrolment_lifetime.after(at),
                    },
                    |(c, _)| c,
                );
                out.stage_at(StageId::Certified, to, None, flow, run);
                let size = match &response {
                    Some(p) => self.emit_pdu(SpduKind::EcaEeCertResponse, to, n.dcm, at, p),
                    None => self.sizes.enrol_response(),
                };
                out.send(
                    n.dcm,
                    ScmsMsg::EnrolResponse {
                        device,
                        cert,
                        pdu: response,
                    },
                    "enrol-response",
                    size,
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::EnrolResponse { device, cert, pdu } if to == n.dcm => {
                // The DCM relays the response into the secure environment and, for the EE
                // operator, registers the new enrolment certificate with the RA.
                Self::verify(out, 1);
                let cert_id8 = match &pdu {
                    Some(p) => match self.response_cert_id8(p) {
                        Ok(h) => Some(h),
                        Err(e) => {
                            self.reject_pdu(SpduKind::EcaEeCertResponse, to, at, e);
                            return Ok(());
                        }
                    },
                    None => None,
                };
                let size = match &pdu {
                    Some(p) => self.emit_pdu(SpduKind::EcaEeCertResponse, to, device, at, p),
                    None => self.sizes.enrol_response(),
                };
                out.send(
                    device,
                    ScmsMsg::EnrolResponse { device, cert, pdu },
                    "enrol-response-proxied",
                    size,
                    Transport::CellularUu,
                    flow,
                    run,
                );
                Self::sign(out, 1);
                out.send(
                    n.ra,
                    ScmsMsg::EnrolmentRegistration { device, cert_id8 },
                    "enrolment-registration",
                    self.sizes.registration(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::EnrolmentRegistration { device, cert_id8 } => {
                Self::verify(out, 1);
                let reg = self.ra.registered.entry(device).or_default();
                reg.extend(cert_id8);
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
            ScmsMsg::SuccessorEnrolRequest {
                device, current, ..
            } if to == device => {
                // A new key pair, and the request signed with the certificate it replaces.
                Self::scalar_mults(out, 1);
                Self::sign(out, 2);
                out.stage_at(StageId::Requested, device, None, flow, run);
                if let Some(d) = self.devices.get_mut(&device) {
                    d.successor_polls = 0;
                }
                if let Some(d) = self.devices.get_mut(&device) {
                    d.successor_run = Some(run);
                }
                let pdu = self.successor_request_pdu(device, at, run);
                let size = match &pdu {
                    Some(p) => self.emit_pdu(
                        SpduKind::EeRaSuccessorEnrollmentCertRequest,
                        device,
                        n.ra,
                        at,
                        p,
                    ),
                    None => self.sizes.successor_request(),
                };
                out.send(
                    n.ra,
                    ScmsMsg::SuccessorEnrolRequest {
                        device,
                        current,
                        pdu,
                    },
                    "successor-enrol-request",
                    size,
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::SuccessorEnrolRequest {
                device,
                current,
                pdu,
            } => {
                // The RA: decrypt, verify the current enrolment certificate's signature,
                // and serve only a registered, unblocked certificate.
                Self::verify(out, 1);
                let (inner, current_cert, hash8) = match pdu {
                    Some(p) if self.pki.is_some() => {
                        let pki = self.pki.clone().expect("checked");
                        match us::ra_open_successor_request(&pki, p.bytes()) {
                            Ok((c, inner)) => {
                                (Some(Pdu::new(inner)), Some(Box::new(c)), p.hashed_id8())
                            }
                            Err(e) => {
                                self.reject_pdu(
                                    SpduKind::EeRaSuccessorEnrollmentCertRequest,
                                    to,
                                    at,
                                    e,
                                );
                                return Ok(());
                            }
                        }
                    }
                    _ => {
                        let mut seed = device.index().to_be_bytes().to_vec();
                        seed.extend_from_slice(&at.to_be_bytes());
                        let h = sha256(&seed);
                        let mut b = [0u8; 8];
                        b.copy_from_slice(&h[24..]);
                        (None, None, b)
                    }
                };
                if self.ra.blocklist.contains(&device) {
                    self.ra.refused += 1;
                    self.refuse_successor(out, device, device, Refusal::Blocklisted, flow, run);
                    return Ok(());
                }
                let signer = current_cert.as_deref().map(pdu::cert_id8);
                if !self.ra.is_registered(device, signer) {
                    self.ra.unregistered_refused += 1;
                    self.refuse_successor(
                        out,
                        device,
                        device,
                        Refusal::UnknownEnrolment,
                        flow,
                        run,
                    );
                    return Ok(());
                }
                // A second request before the download invalidates the first; a successor
                // already issued for it is blocklisted ([SM-REENR]).
                if let Some(old) = self.ra.successors.remove(&device) {
                    self.ra.successors_superseded += 1;
                    if let Some((_, Some(p))) = &old.response
                        && let Ok(h) = self.response_cert_id8(p)
                    {
                        self.ra.successors_blocklisted.insert(h);
                        if let Some(reg) = self.ra.registered.get_mut(&device) {
                            reg.remove(&h);
                        }
                    }
                }
                self.ra.successors.insert(
                    device,
                    SuccessorRecord {
                        request_hash8: hash8,
                        inner_hash8: inner.as_ref().map(Pdu::hashed_id8),
                        response: None,
                    },
                );
                self.ra.successor_requests += 1;
                out.stage_at(StageId::Acknowledged, to, None, flow, run);
                Self::sign(out, 1);
                let next_dl = self.params.reenrol_download_delay.after(at);
                let ack = self.ra_successor_ack(hash8, at, next_dl);
                let size = match &ack {
                    Some(p) => self.emit_pdu(SpduKind::RaEeEnrollmentCertAck, to, device, at, p),
                    None => self.sizes.provisioning_ack(),
                };
                out.send(
                    device,
                    ScmsMsg::SuccessorEnrolAck {
                        device,
                        next_dl,
                        pdu: ack,
                    },
                    "successor-enrol-ack",
                    size,
                    Transport::CellularUu,
                    flow,
                    run,
                );
                let size = match &inner {
                    Some(p) => self.emit_pdu(SpduKind::EeEcaCertRequest, to, n.eca, at, p),
                    None => self.sizes.successor_request(),
                };
                out.send(
                    n.eca,
                    ScmsMsg::SuccessorEnrolForward {
                        device,
                        current,
                        pdu: inner,
                        current_cert,
                    },
                    "successor-enrol-forward",
                    size,
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::SuccessorEnrolForward {
                device,
                current,
                pdu,
                current_cert,
            } => {
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
                        n.ra,
                        ScmsMsg::EnrolRefused { device, reason },
                        "enrol-refused",
                        self.sizes.refusal(),
                        Transport::BackendNet,
                        flow,
                        run,
                    );
                    return Ok(());
                }
                let response = match (pdu, current_cert) {
                    (Some(req), Some(cur)) if self.pki.is_some() => {
                        match self.eca_issue(at, device, &req, Some(&cur)) {
                            Ok(p) => Some(p),
                            Err(e) => {
                                self.reject_pdu(SpduKind::EeEcaCertRequest, to, at, e);
                                self.eca.refused += 1;
                                self.eca.profile_refused += 1;
                                out.send(
                                    n.ra,
                                    ScmsMsg::EnrolRefused {
                                        device,
                                        reason: Refusal::ProfileViolation,
                                    },
                                    "enrol-refused",
                                    self.sizes.refusal(),
                                    Transport::BackendNet,
                                    flow,
                                    run,
                                );
                                return Ok(());
                            }
                        }
                    }
                    _ => None,
                };
                self.eca.issued += 1;
                self.eca.successors += 1;
                // No overlap: the successor starts when the current certificate ends.
                let cert = EnrolmentCert {
                    generation: current.generation + 1,
                    valid_from: current.valid_until,
                    valid_until: self.params.enrolment_lifetime.after(current.valid_until),
                };
                out.stage_at(StageId::Certified, to, None, flow, run);
                let size = match &response {
                    Some(p) => self.emit_pdu(SpduKind::EcaEeCertResponse, to, n.ra, at, p),
                    None => self.sizes.enrol_response(),
                };
                out.send(
                    n.ra,
                    ScmsMsg::SuccessorEnrolResponse {
                        device,
                        cert,
                        pdu: response,
                    },
                    "successor-enrol-certified",
                    size,
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::SuccessorEnrolResponse { device, cert, pdu } if to == n.ra => {
                // The RA keeps the successor for the download and registers it: old and
                // new are linked, and the device's batches stay downloadable with either.
                Self::verify(out, 1);
                let id8 = match &pdu {
                    Some(p) => match self.response_cert_id8(p) {
                        Ok(h) => Some(h),
                        Err(e) => {
                            self.reject_pdu(SpduKind::EcaEeCertResponse, to, at, e);
                            return Ok(());
                        }
                    },
                    None => None,
                };
                let request_hash = pdu.as_ref().and_then(|p| {
                    let pki = self.pki.clone()?;
                    let (q, _) = pdu::open_signed_pdu(p.bytes(), &pki).ok()?;
                    match q.content {
                        ScmsPduContent::eca_ee(EcaEeInterfacePdu::ecaEeCertResponse(r)) => {
                            Some(v2xw_sec::hashedid::id8_bytes(&r.request_hash))
                        }
                        _ => None,
                    }
                });
                match self.ra.successors.get_mut(&device) {
                    Some(rec) if rec.inner_hash8 == request_hash => {
                        rec.response = Some((cert, pdu));
                        self.ra.registered.entry(device).or_default().extend(id8);
                    }
                    // The answer to a request a newer one superseded: never handed over,
                    // and blocklisted.
                    _ => {
                        self.ra.successors_superseded += 1;
                        self.ra.successors_blocklisted.extend(id8);
                    }
                }
            }
            ScmsMsg::SuccessorEnrolAck {
                device,
                next_dl,
                pdu,
            } => {
                Self::verify(out, 1);
                let Some(dev) = self.devices.get(&device) else {
                    return Err(ProtoError::NoEntity { node: device });
                };
                // An answer to a request the device has since replaced is stale.
                let Some(hash8) = dev.successor_request.as_ref().map(|r| r.2) else {
                    return Ok(());
                };
                if dev.successor_run != Some(run) {
                    return Ok(());
                }
                if let (Some(p), Some(pki)) = (&pdu, self.pki.clone()) {
                    let ok = pdu::open_signed_pdu(p.bytes(), &pki)
                        .ok()
                        .and_then(|(q, s)| (pdu::cert_id8(&s) == pki.ra.id8()).then_some(q));
                    let matches = ok.is_some_and(|q| {
                        matches!(q.content,
                            ScmsPduContent::ee_ra(EeRaInterfacePdu::raEeCertInfo(i))
                                if v2xw_sec::hashedid::id8_bytes(&i.request_hash) == hash8)
                    });
                    if !matches {
                        self.reject_pdu(
                            SpduKind::RaEeEnrollmentCertAck,
                            device,
                            at,
                            PduError::Shape("the ack is the RA's, for the request sent"),
                        );
                        return Ok(());
                    }
                }
                let polls = self.devices.get_mut(&device).map_or(0, |d| {
                    d.successor_polls += 1;
                    d.successor_polls
                });
                if polls > self.params.max_download_polls {
                    return Ok(());
                }
                out.start_timer(
                    Duration::from_nanos(next_dl.saturating_sub(at)),
                    ScmsMsg::SuccessorEnrolDownload { device, pdu: None },
                    flow,
                    run,
                );
            }
            ScmsMsg::SuccessorEnrolDownload { device, .. } if to == device => {
                if self
                    .devices
                    .get(&device)
                    .is_none_or(|d| d.successor_request.is_none() || d.successor_run != Some(run))
                {
                    // Refused or replaced meanwhile: nothing to fetch for this request.
                    return Ok(());
                }
                Self::sign(out, 1);
                let pdu = self.successor_download_pdu(device, at);
                let size = match &pdu {
                    Some(p) => self.emit_pdu(SpduKind::EeRaDownloadRequest, device, n.ra, at, p),
                    None => self.sizes.batch_download_request(),
                };
                out.send(
                    n.ra,
                    ScmsMsg::SuccessorEnrolDownload { device, pdu },
                    "successor-enrol-download-request",
                    size,
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::SuccessorEnrolDownload { device, pdu } => {
                Self::verify(out, 1);
                let Some(rec) = self.ra.successors.get(&device).cloned() else {
                    return Ok(());
                };
                if let Some(p) = &pdu
                    && self.pki.is_some()
                    && let Err(e) = self.ra_open_download(device, p, rec.request_hash8)
                {
                    self.reject_pdu(SpduKind::EeRaDownloadRequest, to, at, e);
                    return Ok(());
                }
                match rec.response {
                    Some((cert, resp)) => {
                        self.ra.successors.remove(&device);
                        let size = match &resp {
                            Some(p) => {
                                self.emit_pdu(SpduKind::EcaEeCertResponse, to, device, at, p)
                            }
                            None => self.sizes.enrol_response(),
                        };
                        out.send(
                            device,
                            ScmsMsg::SuccessorEnrolResponse {
                                device,
                                cert,
                                pdu: resp,
                            },
                            "successor-enrol-response",
                            size,
                            Transport::CellularUu,
                            flow,
                            run,
                        );
                    }
                    None => {
                        // Not ready: come back after the poll interval (the `.info` answer).
                        Self::sign(out, 1);
                        let next_dl = self.params.download_poll_interval.after(at);
                        let ack = self.ra_successor_ack(rec.request_hash8, at, next_dl);
                        let size = match &ack {
                            Some(p) => {
                                self.emit_pdu(SpduKind::RaEeEnrollmentCertAck, to, device, at, p)
                            }
                            None => self.sizes.provisioning_ack(),
                        };
                        out.send(
                            device,
                            ScmsMsg::SuccessorEnrolAck {
                                device,
                                next_dl,
                                pdu: ack,
                            },
                            "successor-enrol-ack",
                            size,
                            Transport::CellularUu,
                            flow,
                            run,
                        );
                    }
                }
            }
            ScmsMsg::SuccessorEnrolResponse { device, cert, pdu } => {
                Self::verify(out, 2);
                // The device verifies the ECA's signature, matches the requestHash to its
                // inner request, reconstructs the new key and holds the successor until
                // the current certificate ends.
                let Some((k_new, inner, _)) = self
                    .devices
                    .get(&device)
                    .and_then(|d| d.successor_request.clone())
                else {
                    return Ok(());
                };
                let cred = match &pdu {
                    Some(p) if self.pki.is_some() => {
                        match self.open_eca_response(p, &k_new, inner.hashed_id8()) {
                            Ok(c) => Some(c),
                            Err(e) => {
                                self.reject_pdu(SpduKind::EcaEeCertResponse, device, at, e);
                                return Ok(());
                            }
                        }
                    }
                    _ => None,
                };
                if let Some(dev) = self.devices.get_mut(&device) {
                    dev.successor = Some(cert);
                    dev.successor_credential = cred;
                    dev.successor_request = None;
                    dev.successor_polls = 0;
                    dev.reenrolments += 1;
                    dev.refused = None;
                    dev.roll_enrolment(at);
                }
                out.stage_at(StageId::Installed, device, None, flow, run);
            }
            ScmsMsg::EnrolRefused { device, reason } if to == n.ra => {
                // The ECA refused: the RA drops the request and tells the device.
                Self::verify(out, 1);
                self.ra.successors.remove(&device);
                self.refuse_successor(out, device, device, reason, flow, run);
            }
            ScmsMsg::EnrolRefused { device, reason } if to == n.dcm => {
                // A first enrolment the ECA refused: the factory sees it; the device stays
                // unenrolled.
                Self::verify(out, 1);
                if let Some(dev) = self.devices.get_mut(&device) {
                    dev.enrolling = false;
                    dev.enrol_request = None;
                    dev.refused = Some(reason);
                    dev.refusals += 1;
                }
            }
            ScmsMsg::EnrolRefused { device, reason } => {
                Self::verify(out, 1);
                if let Some(dev) = self.devices.get_mut(&device) {
                    dev.refused = Some(reason);
                    dev.reenrol_refused = Some(reason);
                    dev.refusals += 1;
                    dev.successor_request = None;
                }
            }
            _ => unreachable!("ScmsState::handle routes only this flow's messages here"),
        }
        Ok(())
    }
}
