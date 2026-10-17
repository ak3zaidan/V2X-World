//! Pseudonym provisioning and top-up: the butterfly request, the LA pre-linkage values, the RA shuffle, PCA issuance and the batch download.
//!
//! Split out of `run.rs` without behaviour change (WP-B1); the state types and the shared
//! helpers stay in [`super`].

use super::*;

use crate::scms::pdu::{self, Pdu, PduError, RequestSigner, RequestSignerFound, SpduKind};
use v2xw_msg::sec_types::ieee1609_dot2::{IssuerIdentifier, VerificationKeyIndicator};
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    BasePublicEncryptionKey, PublicVerificationKey,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_ee_ra_interface::{
    AdditionalParams, ButterflyExpansion, EeRaInterfacePdu,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::ScmsPduContent;

/// The hash the RA knows a provisioning request by.
///
/// Over the request's public material, so two different requests cannot collide and the
/// same request replayed is recognised — the "one request per period" check of
/// [CAMP-EE §2.2.7.6].
pub(super) fn request_hash(req: &ProvisioningRequest) -> [u8; 32] {
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

impl ScmsRun {
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

    pub(super) fn start_provisioning(
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
            spdu: None,
        };
        if let Some(d) = self.state.devices.get_mut(&device) {
            d.caterpillar = Some(cat);
            d.download_retries = 0;
            d.refused = None;
            d.topup_refused = None;
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
        host_committees(&mut scratch, &self.state.committees, &p);
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
        // The RA's request shuffle belongs to the run's kernel. Provisioning jobs of devices
        // topping up *in the run* may be waiting in it, with its timer armed on the run's
        // kernel: left there, this device's job joined that queue, no timer fired on the
        // scratch kernel, its download polls ran out and it joined the road with no
        // certificate at all (13 of 218 vehicles at 6,000 veh/h on credential-lifecycle,
        // no error raised) — and a flush on the scratch kernel would have released the run's
        // jobs where the run never sees them. The scratch run gets an empty shuffle; the
        // run's queue and its timer are put back untouched.
        let run_jobs = core::mem::take(&mut self.state.ra.jobs);
        // A pre-run pool was issued long before the run's faults: they do not apply.
        let run_faults = core::mem::take(&mut self.state.faults);
        let run_armed = core::mem::replace(&mut self.state.ra.shuffle_armed, false);
        // The run's armed timer names the run's batch generation; the scratch release bumps
        // the counter, so it is put back with the timer or that batch would never close.
        let run_generation = self.state.ra.shuffle_generation;
        self.enrol_at(device, 0);
        let mut outcome = Self::drain_into(&mut self.state, &mut self.kernel);
        if outcome.is_ok() {
            let at = self.kernel.now();
            self.start_provisioning(device, at, start_i, periods, jmax, FlowId::Provisioning);
            outcome = Self::drain_into(&mut self.state, &mut self.kernel);
        }
        core::mem::swap(&mut self.kernel, &mut real);
        self.state.params = saved;
        let leftover = core::mem::replace(&mut self.state.ra.jobs, run_jobs);
        self.state.ra.jobs.extend(leftover);
        self.state.ra.shuffle_armed = run_armed;
        self.state.ra.shuffle_generation = run_generation;
        self.state.faults = run_faults;
        outcome
    }
}

impl ScmsState {
    /// Device side: the `EeRaCertRequestSpdu` for `req` — original butterfly parameters,
    /// signed by the enrolment certificate, encrypted to the RA. `None` when the layer is
    /// off or the device holds no real enrolment credential.
    fn ee_ra_request_pdu(&mut self, req: &ProvisioningRequest, at: SimTime) -> Option<Pdu> {
        let pki = self.pki.clone()?;
        let cred = self
            .devices
            .get(&req.device)?
            .enrolment_credential
            .clone()?;
        let ra_key = pki.ra.enc_public()?;
        let t = self.time32(at);
        let first = self.time32(self.params.period_start(req.start_i));
        let p = pdu::ee_ra_cert_request(
            t,
            &req.signing_public,
            &req.encryption_public,
            &req.ck,
            &req.ek,
            first,
        )
        .ok()?;
        let bytes = pdu::signed_encrypted_cert_request(
            &p,
            RequestSigner::Cert(&cred.cert, &cred.key),
            &pki.ra.cert,
            &ra_key,
            &mut self.pdu_rng,
        )
        .ok()?;
        let pdu = Pdu::new(bytes);
        if let Some(d) = self.devices.get_mut(&req.device) {
            d.request_hash8 = Some(pdu.hashed_id8());
        }
        Some(pdu)
    }

    /// RA side: decrypt, verify under an ECA-issued enrolment certificate, and bind the
    /// signed butterfly parameters to the request's `A`, `ck`, `H`, `ek`.
    fn ra_open_request(&self, req: &ProvisioningRequest, spdu: &Pdu) -> pdu::Result<()> {
        let pki = self.pki.clone().ok_or(PduError::Shape("PDU layer off"))?;
        let enc = pki
            .ra
            .enc_private
            .ok_or(PduError::Key("RA encryption key"))?;
        let plain = pdu::decrypt_cert(spdu.bytes(), &pki.ra.cert, &enc)?;
        let (p, signer) = pdu::open_signed_cert_request(&plain, &pki)?;
        let RequestSignerFound::Cert(ec_cert) = signer else {
            return Err(PduError::Signer(
                "a provisioning request is signed by an enrolment certificate",
            ));
        };
        let issued_by_eca = matches!(&ec_cert.0.issuer,
            IssuerIdentifier::sha256AndDigest(h) if v2xw_sec::hashedid::id8_bytes(h) == pki.eca.id8());
        if !issued_by_eca {
            return Err(PduError::UntrustedSigner);
        }
        let ScmsPduContent::ee_ra(EeRaInterfacePdu::eeRaCertRequest(r)) = &p.content else {
            return Err(PduError::Shape("expected ee-ra.eeRaCertRequest"));
        };
        let VerificationKeyIndicator::verificationKey(PublicVerificationKey::ecdsaNistP256(a)) =
            &r.tbs_cert.verify_key_indicator
        else {
            return Err(PduError::Shape(
                "verifyKeyIndicator is the caterpillar key A",
            ));
        };
        let Some(AdditionalParams::original(b)) = &r.additional_params else {
            return Err(PduError::Shape(
                "original butterfly parameters (the US profile)",
            ));
        };
        let ButterflyExpansion::aes128(ck) = &b.signing_expansion else {
            return Err(PduError::Shape("signingExpansion aes128"));
        };
        let ButterflyExpansion::aes128(ek) = &b.encryption_expansion else {
            return Err(PduError::Shape("encryptionExpansion aes128"));
        };
        let BasePublicEncryptionKey::eciesNistP256(h) = &b.encryption_key.public_key else {
            return Err(PduError::Shape("encryptionKey eciesNistP256"));
        };
        let same = pdu::point_of(a)? == req.signing_public
            && pdu::point_of(h)? == req.encryption_public
            && ck[..] == req.ck[..]
            && ek[..] == req.ek[..];
        if !same {
            return Err(PduError::Shape(
                "signed butterfly parameters differ from the request's",
            ));
        }
        Ok(())
    }

    /// RA side: the RA-signed `RaEeCertAckSpdu` (`firstI` = the first i-period,
    /// `nextDlTime` = the first batch time).
    fn ra_ack_pdu(&mut self, request_hash8: [u8; 8], start_i: u32, at: SimTime) -> Option<Pdu> {
        let pki = self.pki.clone()?;
        let t = self.time32(at);
        let next = self.time32(self.params.first_batch_delay.after(at));
        let first_i = u16::try_from(start_i).unwrap_or(u16::MAX);
        let ack = pdu::ra_ee_cert_ack(t, request_hash8, first_i, next);
        Some(Pdu::new(pdu::signed(&ack, &pki.ra.cert, &pki.ra.key).ok()?))
    }

    /// Device side: the ack is RA-signed and answers the request this device sent.
    fn ee_check_ack(&self, device: NodeId, ack: &Pdu) -> pdu::Result<()> {
        let pki = self.pki.clone().ok_or(PduError::Shape("PDU layer off"))?;
        let (p, signer) = pdu::open_signed_pdu(ack.bytes(), &pki)?;
        if pdu::cert_id8(&signer) != pki.ra.id8() {
            return Err(PduError::Signer("an ack is signed by the RA"));
        }
        let ScmsPduContent::ee_ra(EeRaInterfacePdu::raEeCertAck(a)) = p.content else {
            return Err(PduError::Shape("expected ee-ra.raEeCertAck"));
        };
        let sent = self.devices.get(&device).and_then(|d| d.request_hash8);
        if sent != Some(v2xw_sec::hashedid::id8_bytes(&a.request_hash)) {
            return Err(PduError::Shape(
                "requestHash is not the hash of the request sent",
            ));
        }
        Ok(())
    }

    /// The PCA's answer to the RA: the certified batch.
    pub(super) fn send_batch(
        &mut self,
        device: NodeId,
        i: u32,
        credentials: Vec<IssuedCredential>,
        flow: FlowId,
        run: FlowRun,
        out: &mut Outbox<ScmsMsg>,
    ) {
        let n = self.nodes;
        let count = u32::try_from(credentials.len()).unwrap_or(u32::MAX);
        if self.pca.certified_runs.insert(run) {
            out.stage_at(StageId::Certified, n.pca, None, flow, run);
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

    /// Closes the RA's certificate-request shuffle: every job whose pre-linkage values are
    /// in goes to the PCA, period by period. Run by the window's timer or, first, by the
    /// count threshold ("10,000 requests or one day, whichever comes first",
    /// CAMP-EE §2.2.7); either way the next batch starts with a fresh window.
    pub(super) fn release_request_shuffle(
        &mut self,
        out: &mut Outbox<ScmsMsg>,
        n: ScmsNodes,
        to: NodeId,
    ) {
        self.ra.shuffle_armed = false;
        self.ra.shuffle_generation += 1;
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
                    self.sizes.cert_request_for(count),
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

    /// Handles every message of this flow; [`ScmsState::handle`] routes here.
    #[allow(unused_variables)]
    pub(super) fn handle_provision(
        &mut self,
        d: Delivery<ScmsMsg>,
        out: &mut Outbox<ScmsMsg>,
    ) -> Result<()> {
        let n = self.nodes;
        let (flow, run, to, at, from) = (d.flow, d.run, d.to, d.at, d.from);
        match d.msg {
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
                    (
                        d.trust.lpf.as_ref().map(|l| l.version),
                        d.trust.lccf_version,
                    )
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
                // Under a hybrid scheme there is no post-quantum butterfly: the device
                // generates one post-quantum key pair per certificate and encrypts each
                // public key to the PCA (an ephemeral key and a point multiplication
                // apiece), so the RA can shuffle them without reading them.
                let certs = req.periods.saturating_mul(req.jmax);
                if out.is_hybrid() {
                    out.charge_pq(PrimitiveOpKind::KeyGen, certs);
                    // Each upload is encrypted to the PCA: an ECIES ephemeral key and a
                    // point multiplication, and/or an ML-KEM encapsulation.
                    if out.ecies() {
                        Self::scalar_mults(out, certs.saturating_mul(2));
                    }
                    out.charge_kem(PrimitiveOpKind::Sign, certs);
                }
                out.stage_at(StageId::Requested, req.device, None, flow, run);
                req.spdu = self.ee_ra_request_pdu(&req, at);
                let size = match req.spdu.clone() {
                    Some(p) => self.emit_pdu(SpduKind::EeRaCertRequest, req.device, n.lop, at, &p),
                    None => self.sizes.provisioning_request_for(certs),
                };
                out.send(
                    n.lop,
                    ScmsMsg::ProvisioningRequest(req),
                    "provisioning-request",
                    size,
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
                let size = match req.spdu.clone() {
                    Some(p) => self.emit_pdu(SpduKind::EeRaCertRequest, to, n.ra, at, &p),
                    None => self
                        .sizes
                        .provisioning_request_for(req.periods.saturating_mul(req.jmax)),
                };
                out.send(
                    n.ra,
                    ScmsMsg::ProvisioningRequest(req),
                    "provisioning-request-proxied",
                    size,
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::ProvisioningRequest(mut req) => {
                Self::verify(out, 1);
                // The real request: decrypt with the RA's encryption key, verify the
                // enrolment-certificate signature, and require the signed butterfly
                // parameters to be exactly the ones this request carries. Anything else is
                // refused and recorded.
                let request_hash8 = match req.spdu.clone() {
                    Some(p) if self.pki.is_some() => match self.ra_open_request(&req, &p) {
                        Ok(()) => Some(p.hashed_id8()),
                        Err(e) => {
                            self.reject_pdu(SpduKind::EeRaCertRequest, to, at, e);
                            return Ok(());
                        }
                    },
                    _ => None,
                };
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

                // Butterfly expansion (none under a pure post-quantum scheme, which has no
                // elliptic-curve key to expand): two scalar multiplications per certificate, and
                // nothing that reveals the certified key, which only the PCA's `c` fixes.
                let total = req.periods.saturating_mul(req.jmax);
                if out.classical_signs() {
                    Self::scalar_mults(out, 2 * total);
                }
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

                let ack = request_hash8.and_then(|h| self.ra_ack_pdu(h, req.start_i, at));
                let size = match &ack {
                    Some(p) => self.emit_pdu(SpduKind::RaEeCertAck, to, n.lop, at, p),
                    None => self.sizes.provisioning_ack(),
                };
                out.send(
                    n.lop,
                    ScmsMsg::ProvisioningAck {
                        device: req.device,
                        request_hash,
                        periods: req.periods,
                        pdu: ack,
                        jmax: req.jmax,
                    },
                    "provisioning-ack",
                    size,
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
                pdu,
                jmax,
            } if to == n.lop => {
                self.lop.downstream += 1;
                let size = match &pdu {
                    Some(p) => self.emit_pdu(SpduKind::RaEeCertAck, to, device, at, p),
                    None => self.sizes.provisioning_ack(),
                };
                out.send(
                    device,
                    ScmsMsg::ProvisioningAck {
                        device,
                        request_hash,
                        periods,
                        pdu,
                        jmax,
                    },
                    "provisioning-ack-proxied",
                    size,
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::ProvisioningAck {
                device,
                periods,
                pdu,
                ..
            } => {
                // The device verifies the RA's signature and that the ack answers the
                // request it sent; an ack that does not is ignored, and recorded.
                if let Some(p) = pdu {
                    if let Err(e) = self.ee_check_ack(device, &p) {
                        self.reject_pdu(SpduKind::RaEeCertAck, device, at, e);
                        return Ok(());
                    }
                }
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
                    let waiting = self
                        .ra
                        .jobs
                        .iter()
                        .filter(|j| j.responses >= 2 && !j.sent)
                        .count();
                    if waiting >= self.params.shuffle_threshold as usize {
                        // The count closes this batch before the day does.
                        self.release_request_shuffle(out, n, to);
                    } else if !self.ra.shuffle_armed {
                        self.ra.shuffle_armed = true;
                        out.start_timer(
                            self.params.shuffle_window,
                            ScmsMsg::ShuffleTimer {
                                generation: self.ra.shuffle_generation,
                            },
                            flow,
                            run,
                        );
                    }
                }
            }
            ScmsMsg::ShuffleTimer { generation } => {
                if generation == self.ra.shuffle_generation {
                    self.release_request_shuffle(out, n, to);
                }
            }
            ScmsMsg::CertRequest { device, i, items } => {
                let count = u32::try_from(items.len()).unwrap_or(u32::MAX);
                // Per certificate: two decryptions of the pre-linkage values, one ECQV
                // issuance, one encryption to the device and one signature (05-protocols
                // §3.2). The decryptions and the encryption are ECIES scalar
                // multiplications, ML-KEM operations under a post-quantum `security.kem`, or
                // both under a hybrid one; a pure post-quantum certificate is explicit, so
                // it has no ECQV issuance.
                Self::decrypt(out, 2 * count);
                if out.classical_signs() {
                    Self::scalar_mults(out, count);
                }
                Self::encrypt(out, count);
                // Under a post-quantum scheme, one more decryption per certificate (the
                // device's post-quantum key); `sign` adds the post-quantum signature.
                if out.is_hybrid() {
                    Self::decrypt(out, count);
                }
                // A committee PCA signs through its session instead (below).
                let committee = self.committees.role("pca");
                if committee.is_none() {
                    Self::sign(out, count);
                }
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
                if let Some(role) = committee {
                    // No single server can issue: the batch waits for the committee's
                    // signatures, one per certificate (one per umbrella under an umbrella
                    // authority, which the protocol accounts for).
                    self.committees.start(
                        role,
                        count,
                        Pending::IssueBatch {
                            device,
                            i,
                            credentials: Box::new(credentials),
                            flow,
                            run,
                        },
                        at,
                        flow,
                        run,
                        out,
                        wrap_committee,
                    );
                    return Ok(());
                }
                self.send_batch(device, i, credentials, flow, run, out);
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
                if out.classical_signs() {
                    out.compute(OpDescriptor::new(ECQV, PrimitiveOpKind::Verify, count));
                }
                // Under a post-quantum scheme each certificate's post-quantum half is
                // explicit: the device checks the PCA's post-quantum signature on every one.
                // Under a post-quantum KEM it decapsulates each certificate's wrapper.
                let leaves = self.sizes.umbrella_leaves;
                let issued = count.div_ceil(leaves.max(1));
                out.charge_pq(PrimitiveOpKind::Verify, issued);
                out.charge_kem(PrimitiveOpKind::Verify, issued);
                if leaves > 1 {
                    // Umbrella: the device derives each leaf and certifies it with its
                    // umbrella key, one signature per leaf (`threshold::umbrella`).
                    Self::sign(out, count);
                }
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
                    dev.topup_refused = Some(reason);
                    dev.refusals += 1;
                    dev.provisioning = None;
                }
            }
            _ => unreachable!("ScmsState::handle routes only this flow's messages here"),
        }
        Ok(())
    }
}
