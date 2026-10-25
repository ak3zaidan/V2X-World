//! Pseudonym provisioning and top-up: the butterfly request, the LA pre-linkage values,
//! the RA shuffle, PCA issuance and the batch download.
//!
//! Split out of `run.rs` without behaviour change (WP-B1); the state types and the shared
//! helpers stay in [`super`].

use super::*;

use crate::scms::pdu::{self, Pdu, PduError, RequestSigner, SpduKind};
use crate::scms::pdu_prov;
use v2xw_msg::sec_types::ieee1609_dot2_dot1_ee_ra_interface::EeRaInterfacePdu;
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
        let fresh = Caterpillar::from_seed(&seed).expect("64 bytes of seed");
        // A top-up keeps the device's caterpillar: the butterfly keys are made once and the
        // RA expands every later period from the same A, ck, H, ek (1609.2.1 §9.3; 03 §6.4
        // "request once"). A fresh caterpillar here orphaned every certificate already
        // installed: the device could no longer derive their keys (WP-B3a NYC test). The
        // seed is still drawn so no other stream shifts.
        let cat = match (
            flow,
            self.state
                .devices
                .get(&device)
                .and_then(|d| d.caterpillar.clone()),
        ) {
            (FlowId::Topup, Some(c)) => c,
            _ => fresh,
        };
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
        self.preload_at(device, 0, start_i, periods, jmax)
    }

    /// [`ScmsRun::preload`] with the pre-run exchanges at `at` on the scratch kernel's
    /// clock rather than at 0: the instant the device was enrolled and asked for its pool.
    /// A vehicle that joins mid-run holding an enrolment certificate whose back-dated start
    /// ([`ScmsRun::age_enrolment`]) lies after 0 was enrolled at that start, so its RA
    /// checks the request against a certificate already valid; at 0 the RA would refuse it
    /// as not yet valid, the device would join with no RA account, and its first top-up
    /// would be a fresh in-run request (`scms_knobs_runs`
    /// `the_request_shuffle_does_not_touch_in_run_topups`).
    ///
    /// # Errors
    /// Whatever the flows return on the scratch kernel.
    pub fn preload_at(
        &mut self,
        device: NodeId,
        at: SimTime,
        start_i: u32,
        periods: u32,
        jmax: u32,
    ) -> Result<()> {
        if !self.state.devices.contains_key(&device) {
            self.add_device(device);
        }
        let mut scratch: Kernel<ScmsMsg> = Kernel::new_at(self.kernel.net().clone(), at);
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
        // The PDUs the scratch flows emit name scratch-kernel steps, which are thrown
        // away with it: they are marked as pre-run and carry no step (WP-E1).
        let pdu0 = self.state.pdus.len();
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
        self.enrol_at(device, at);
        let mut outcome = Self::drain_into(&mut self.state, &mut self.kernel);
        if outcome.is_ok() {
            let at = self.kernel.now();
            self.start_provisioning(device, at, start_i, periods, jmax, FlowId::Provisioning);
            outcome = Self::drain_into(&mut self.state, &mut self.kernel);
        }
        core::mem::swap(&mut self.kernel, &mut real);
        for p in &mut self.state.pdus[pdu0..] {
            p.step = None;
            p.preload = true;
        }
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
        let first = self.params.period_start_time32(req.start_i);
        let p = pdu::ee_ra_cert_request_for(
            t,
            &req.signing_public,
            &req.encryption_public,
            &req.ck,
            &req.ek,
            first,
            self.pseudonym_psids_of(req.device).as_slice(),
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
    fn ra_open_request(&self, req: &ProvisioningRequest, spdu: &Pdu) -> pdu::Result<[u8; 8]> {
        let pki = self.pki.clone().ok_or(PduError::Shape("PDU layer off"))?;
        let expected = pdu::ButterflyOriginal {
            a: req.signing_public,
            h: req.encryption_public,
            ck: req.ck,
            ek: req.ek,
        };
        pdu::ra_open_ee_ra_cert_request(&pki, spdu.bytes(), &expected).map(|c| pdu::cert_id8(&c))
    }

    /// RA side: the RA-signed `RaEeCertAckSpdu` (`firstI` = the first i-period,
    /// `nextDlTime` = the first batch time).
    fn ra_ack_pdu(&mut self, request_hash8: [u8; 8], start_i: u32, at: SimTime) -> Option<Pdu> {
        let pki = self.pki.clone()?;
        let t = self.time32(at);
        let next = self.time32(self.next_download_at(at));
        let first_i = u16::try_from(start_i).unwrap_or(u16::MAX);
        let ack = pdu::ra_ee_cert_ack(t, request_hash8, first_i, next);
        Some(Pdu::new(pdu::signed(&ack, &pki.ra.cert, &pki.ra.key).ok()?))
    }

    /// Device side: the ack is RA-signed and answers the request this device sent.
    fn ee_check_ack(&self, device: NodeId, ack: &Pdu) -> pdu::Result<()> {
        let pki = self.pki.clone().ok_or(PduError::Shape("PDU layer off"))?;
        let (p, signer) = pdu::open_signed_pdu(ack.bytes(), &pki)?;
        if !pki.is_role(&pki.ra, &pdu::cert_id8(&signer)) {
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

    /// Device side: the `EeRaDownloadRequestSpdu` naming `<requestHash>_<i>.zip`, or
    /// `None` when the layer is off or the device has no real enrolment credential.
    fn ee_download_request_pdu(&mut self, device: NodeId, i: u32, at: SimTime) -> Option<Pdu> {
        let pki = self.pki.clone()?;
        let d = self.devices.get(&device)?;
        let cred = d.enrolment_credential.clone()?;
        let name = pdu::batch_filename(d.request_hash8?, i);
        let t = self.time32(at);
        pdu_prov::ee_download_request_spdu(&pki, &cred.cert, &cred.key, t, &name, &mut self.pdu_rng)
            .ok()
    }

    /// When the RA tells a device to come for its first batch (`RaEeCertAck.nextDlTime`,
    /// 03 §3.3): no earlier than `first_batch_delay` after the ack, and no earlier than the
    /// latest the request shuffle can close (the armed batch's deadline, or a whole window
    /// from now). A device that polled before the shuffle's age could close would spend
    /// its `download-maxReqs` polls on a 2-day CAMP 1.2.2 window and give up.
    pub(super) fn next_download_at(&self, at: SimTime) -> SimTime {
        let release = if self.ra.shuffle_armed {
            self.ra.shuffle_deadline.max(at)
        } else {
            self.params.shuffle_window.after(at)
        };
        self.params.first_batch_delay.after(at).max(release)
    }

    /// Device side: the `EeRaDownloadRequestSpdu` naming `<requestHash>.info`.
    fn ee_info_request_pdu(&mut self, device: NodeId, at: SimTime) -> Option<Pdu> {
        let pki = self.pki.clone()?;
        let d = self.devices.get(&device)?;
        let cred = d.enrolment_credential.clone()?;
        let name = pdu_prov::info_filename(d.request_hash8?);
        let t = self.time32(at);
        pdu_prov::ee_download_request_spdu(&pki, &cred.cert, &cred.key, t, &name, &mut self.pdu_rng)
            .ok()
    }

    /// When the weekly job generates period `i` of `account`: the request's own periods,
    /// and every period within `ra-maxPreloadTime` (the Local Policy File's
    /// `max_periods_ahead`, 156 weeks = 3 years) of `firstI`, at acceptance; each later
    /// period at the i-period boundary that brings it inside the horizon ("adds one week
    /// every week", [PRIMER] p.6). The job runs at the boundary, so the generation delay
    /// is zero, inside `ra-maxGenDelay` = 1 week ([SMP] Table 3). `None` before `firstI`.
    #[must_use]
    pub fn pregen_due(&self, account: &PregenAccount, i: u32) -> Option<SimTime> {
        if i < account.first_i {
            return None;
        }
        // `ra-maxPreloadTime` is 2 weeks to 3 years ([SMP] Table 3): a policy outside it
        // is held to the range (it still clips requests as it says).
        let h = self.gov.lpf.policy.max_periods_ahead.clamp(
            crate::scms::params::RA_MIN_PRELOAD_PERIODS,
            crate::scms::params::RA_MAX_PRELOAD_PERIODS,
        );
        let covered = account.first_i.saturating_add(h);
        if i < covered || account.generated.contains(&i) {
            return Some(account.accepted_at);
        }
        let p = &self.params;
        // The i-period the request was accepted in, on the run's calendar (the real US
        // grid from 2015-01-06 when `calendar_epoch_unix_s` is set, else from `epoch`).
        let p0 = u64::from(p.period_at(account.accepted_at));
        let k = u64::from(i - covered) + 1;
        let boundary = u32::try_from(p0 + k).unwrap_or(u32::MAX);
        Some(p.period_start(boundary).max(account.accepted_at))
    }

    /// The RA's answer to a `.info` request for period `i`: when the batch can be
    /// downloaded. A held batch: now. One the schedule has reached: generated now, ready
    /// `first_batch_delay` later. One it has not: the weekly job is armed for that
    /// boundary.
    fn pregen_next(
        &mut self,
        device: NodeId,
        i: u32,
        at: SimTime,
        out: &mut Outbox<ScmsMsg>,
        flow: FlowId,
        run: FlowRun,
    ) -> Option<SimTime> {
        let account = self.ra.accounts.get(&device)?.clone();
        let due = self.pregen_due(&account, i)?;
        if self
            .ra
            .repos
            .get(&device)
            .is_some_and(|r| r.contains_key(&i))
        {
            return Some(at);
        }
        let ready_after = self.params.first_batch_delay;
        if account.generated.contains(&i) {
            return Some(ready_after.after(at));
        }
        if due <= at {
            self.start_pregen_job(device, i, out, flow, run);
            return Some(ready_after.after(at));
        }
        self.ra.pregen_deferred += 1;
        if self.ra.pregen_timers.insert((device, i)) {
            out.start_timer(
                Duration::from_nanos(due - at),
                ScmsMsg::PregenTimer { device, i },
                flow,
                run,
            );
        }
        Some(ready_after.after(due))
    }

    /// Expands, and sends to the LAs, one pre-generated week of `device`'s account, under
    /// the policy in force now (a smaller `certs_per_period` clips it).
    fn start_pregen_job(
        &mut self,
        device: NodeId,
        i: u32,
        out: &mut Outbox<ScmsMsg>,
        flow: FlowId,
        run: FlowRun,
    ) {
        let n = self.nodes;
        // A timer armed before the MA's decision still fires: the RA checks its
        // blocklist when the job runs, not only when the device asks ([CAMP122] SCMS-509).
        if self.ra.blocklist.contains(&device) {
            self.ra.pregen_blocked += 1;
            return;
        }
        let Some(account) = self.ra.accounts.get_mut(&device) else {
            return;
        };
        if !account.generated.insert(i) {
            return;
        }
        let req = account.request.clone();
        let request_hash = account.request_hash;
        let request_hash8 = account.request_hash8;
        let policy = self.gov.lpf.policy;
        let jmax = req.jmax.min(policy.certs_per_period.max(1));
        if jmax != req.jmax {
            self.ra.clipped += 1;
        }
        self.ra.pregen_periods += 1;
        let chains = self
            .ra
            .requests
            .get(&request_hash)
            .map_or([None, None], |r| [r.lci1, r.lci2]);
        if out.classical_signs() {
            Self::scalar_mults(out, 2 * jmax);
        }
        let mut cocoons = BTreeMap::new();
        let mut cocoon_enc = BTreeMap::new();
        // Each certificate's two cocoon keys are a pure function of (i, j): expanded side by
        // side, kept by (i, j).
        let expanded: Vec<_> = {
            use rayon::prelude::*;
            (0..jmax)
                .into_par_iter()
                .map(|j| {
                    butterfly::ra_cocoon_keys(
                        &req.signing_public,
                        &req.encryption_public,
                        &req.ck,
                        &req.ek,
                        i,
                        j,
                    )
                })
                .collect()
        };
        for (j, (b, q)) in (0..jmax).zip(expanded) {
            cocoons.insert((i, j), b);
            cocoon_enc.insert((i, j), q);
        }
        out.stage_at(StageId::Expanded, n.ra, None, flow, run);
        self.ra.jobs.push(ProvisioningJob {
            run,
            flow,
            device,
            request_hash,
            start_i: i,
            periods: 1,
            jmax,
            cocoons,
            cocoon_enc,
            plv: [BTreeMap::new(), BTreeMap::new()],
            enc_plv: [BTreeMap::new(), BTreeMap::new()],
            aca_requests: BTreeMap::new(),
            request_hash8,
            lci: [None, None],
            lci_sealed: [BTreeMap::new(), BTreeMap::new()],
            responses: 0,
            periods_certified: 0,
            sent: false,
            batch_ready_stamped: false,
            pregen: true,
        });
        for la in [LaIndex::One, LaIndex::Two] {
            out.send(
                n.la(la),
                ScmsMsg::PreLinkageRequest {
                    device,
                    la,
                    start_i: i,
                    periods: 1,
                    jmax,
                    chain: chains[la.idx()],
                },
                "pre-linkage-request",
                self.sizes.pre_linkage_request(),
                Transport::BackendNet,
                flow,
                run,
            );
        }
    }

    /// `nextDlTime` of a (verified) ack.
    fn ack_next_dl(&self, ack: &Pdu) -> Option<u32> {
        let pki = self.pki.clone()?;
        let (p, _) = pdu::open_signed_pdu(ack.bytes(), &pki).ok()?;
        let ScmsPduContent::ee_ra(EeRaInterfacePdu::raEeCertAck(a)) = p.content else {
            return None;
        };
        Some(a.next_dl_time.0.0)
    }

    /// From `at` to the `Time32` `t` (zero if already past).
    fn wait_until_time32(&self, at: SimTime, t: u32) -> Duration {
        Duration::from_secs(u64::from(t.saturating_sub(self.time32(at))))
    }

    /// The PCA's answer to the RA: the certified batch.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn send_batch(
        &mut self,
        i: u32,
        credentials: Vec<IssuedCredential>,
        flow: FlowId,
        run: FlowRun,
        at: SimTime,
        out: &mut Outbox<ScmsMsg>,
    ) {
        let n = self.nodes;
        let count = u32::try_from(credentials.len()).unwrap_or(u32::MAX);
        if self.pca.certified_runs.insert(run) {
            out.stage_at(StageId::Certified, n.pca, None, flow, run);
        }
        // The real path: one `AcaRaCertResponseSpdu` per certificate, charged its bytes.
        let mut real = 0u32;
        let mut any = false;
        for c in &credentials {
            if let Some(p) = &c.response {
                any = true;
                real = real.saturating_add(
                    self.emit_pdu(SpduKind::AcaRaCertResponse, n.pca, n.ra, at, p)
                        .bytes(),
                );
            }
        }
        let size = if any {
            WireSize::real(real, "AcaRaCertResponseSpdu x n")
        } else {
            self.sizes.cert_batch(count)
        };
        out.send(
            n.ra,
            ScmsMsg::CertResponse {
                i,
                credentials: Box::new(credentials),
            },
            "cert-response",
            size,
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
        at: SimTime,
    ) {
        self.ra.shuffle_armed = false;
        self.ra.shuffle_generation += 1;
        self.release_jobs(out, n, to, at, None);
    }

    /// Sends the RA→ACA requests of every job whose pre-linkage values are in (`only`:
    /// just that run's, a pre-generated week).
    fn release_jobs(
        &mut self,
        out: &mut Outbox<ScmsMsg>,
        n: ScmsNodes,
        to: NodeId,
        at: SimTime,
        only: Option<FlowRun>,
    ) {
        let jobs = core::mem::take(&mut self.ra.jobs);
        let pki = self.pki.clone();
        let now32 = self.time32(at);
        let mut still_waiting = Vec::new();
        for mut job in jobs {
            if job.responses < 2 || job.sent || only.is_some_and(|r| r != job.run) {
                still_waiting.push(job);
                continue;
            }
            job.sent = true;
            out.stage_at(StageId::Shuffled, to, None, job.flow, job.run);
            for i in job.start_i..job.start_i + job.periods {
                // The requests' inputs in order (each dummy linkage value drawn where the
                // sequential loop drew it), then every RA signature at once (a pure
                // function of its input), then the items in order: the same bytes.
                let mut staged = Vec::new();
                for j in 0..job.jmax {
                    let (Some(b), Some(p1), Some(p2), Some(l1), Some(l2)) = (
                        job.cocoons.get(&(i, j)).copied(),
                        job.plv[0].get(&(i, j)).cloned(),
                        job.plv[1].get(&(i, j)).cloned(),
                        job.lci_sealed[0].get(&(i, j)).copied(),
                        job.lci_sealed[1].get(&(i, j)).copied(),
                    ) else {
                        continue;
                    };
                    // The real `RaAcaCertRequestSpdu`: B_ij, J_ij, both encrypted PLVs and a
                    // dummy linkage value, signed by the RA (03 §3.3, §9).
                    let input = match (
                        pki.as_deref(),
                        job.cocoon_enc.get(&(i, j)),
                        job.enc_plv[0].get(&(i, j)),
                        job.enc_plv[1].get(&(i, j)),
                    ) {
                        (Some(_), Some(jk), Some(e1), Some(e2)) => {
                            let mut dummy = [0u8; 9];
                            dummy.copy_from_slice(&self.pdu_rng.next32()[..9]);
                            Some(pdu_prov::RaAcaInput {
                                generation_time: now32,
                                i: u16::try_from(i).unwrap_or(u16::MAX),
                                cocoon_signing: b,
                                cocoon_encryption: *jk,
                                plv: [e1.clone(), e2.clone()],
                                valid_from: self.params.period_start_time32(i),
                                dummy_lv: dummy,
                                // What the device's request asked for (its own
                                // profile: a PSM device's differs from a vehicle's).
                                psids: self.pseudonym_psids_of(job.device).as_slice().to_vec(),
                            })
                        }
                        _ => None,
                    };
                    staged.push((j, b, p1, p2, l1, l2, input));
                }
                let spdus: Vec<Option<Pdu>> = {
                    use rayon::prelude::*;
                    staged
                        .par_iter()
                        .map(|st| match (pki.as_deref(), &st.6) {
                            (Some(pki), Some(x)) => pdu_prov::ra_aca_cert_request_spdu(pki, x).ok(),
                            _ => None,
                        })
                        .collect()
                };
                let mut items = Vec::new();
                for ((j, b, p1, p2, l1, l2, _), spdu) in staged.into_iter().zip(spdus) {
                    if let Some(p) = &spdu {
                        job.aca_requests.insert(p.hashed_id8(), (i, j));
                    }
                    // The PCA's handle on this certificate: the hash of this one request
                    // (Brecht §VI-D), unique per certificate; the RA alone maps it back.
                    let handle = match &spdu {
                        Some(p) => sha256(p.bytes()),
                        None => {
                            let mut m = Vec::with_capacity(60);
                            m.extend_from_slice(b"ra-aca-request");
                            m.extend_from_slice(&self.params.master_seed.to_be_bytes());
                            m.extend_from_slice(&job.request_hash);
                            m.extend_from_slice(&i.to_be_bytes());
                            m.extend_from_slice(&j.to_be_bytes());
                            sha256(&m)
                        }
                    };
                    self.ra.handles.insert(handle, job.request_hash);
                    items.push(CertRequestItem {
                        i,
                        j,
                        cocoon_signing: b,
                        plv1: p1,
                        plv2: p2,
                        lci1: l1,
                        lci2: l2,
                        handle,
                        spdu,
                    });
                }
                let count = u32::try_from(items.len()).unwrap_or(u32::MAX);
                let mut real = 0u32;
                let mut signed = 0u32;
                for it in &items {
                    if let Some(p) = &it.spdu {
                        signed += 1;
                        real = real.saturating_add(
                            self.emit_pdu(SpduKind::RaAcaCertRequest, to, n.pca, at, p)
                                .bytes(),
                        );
                    }
                }
                // The RA signs every request it forwards.
                Self::sign(out, signed);
                let size = if signed > 0 {
                    WireSize::real(real, "RaAcaCertRequestSpdu x n")
                } else {
                    self.sizes.cert_request_for(count)
                };
                out.send(
                    n.pca,
                    ScmsMsg::CertRequest {
                        i,
                        items: Box::new(items),
                    },
                    "cert-request",
                    size,
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
    /// The RA's answer to a request it could not open, whose signer it does not trust or
    /// that names another device's file: a refusal, as an HTTP service answers a bad
    /// request. Without one the device's SCMS-522 retry would send the request again for
    /// ever. ASSUMPTION: [CAMP122] gives no error code for this case; `UnknownEnrolment`
    /// is the nearest of the modelled refusals.
    fn refuse_unopened(
        &mut self,
        out: &mut Outbox<ScmsMsg>,
        device: NodeId,
        flow: FlowId,
        run: FlowRun,
    ) {
        self.ra.refused_enrolment += 1;
        Self::sign(out, 1);
        out.send(
            self.nodes.lop,
            ScmsMsg::ProvisioningRefused {
                device,
                reason: Refusal::UnknownEnrolment,
            },
            "provisioning-refused",
            self.sizes.refusal(),
            Transport::BackendNet,
            flow,
            run,
        );
    }

    /// SCMS-522: the device has sent `msg`'s request to the RA and now waits for the
    /// answer `kind`; if none comes within `ee_retry_timeout` it sends the request again.
    /// Recorded even with the retry off, so a late duplicate answer is recognised.
    fn await_answer(
        &mut self,
        out: &mut Outbox<ScmsMsg>,
        device: NodeId,
        kind: Awaited,
        msg: ScmsMsg,
        flow: FlowId,
        run: FlowRun,
    ) {
        let timeout = self.params.ee_retry_timeout;
        let retries = self.params.ee_retry_max_per_hour;
        let Some(dev) = self.devices.get_mut(&device) else {
            return;
        };
        dev.request_seq = dev.request_seq.wrapping_add(1);
        let seq = dev.request_seq;
        dev.awaiting = Some((seq, kind, msg, flow, run));
        if timeout > Duration::ZERO && retries > 0 {
            out.start_timer(timeout, ScmsMsg::EeRetryTimer { device, seq }, flow, run);
        }
    }

    /// The batch file's size on the wire for `device`'s download of period `i`: the whole
    /// file, or under a resumed download (`Range: bytes=<n>-`, the bytes it already holds,
    /// [SMP] §4.3.2) the 206 answer's remainder. The value is the one the device's Range
    /// header carries; read from its state rather than modelled as a request field.
    fn range_remainder(&self, device: NodeId, i: u32, size: WireSize) -> WireSize {
        let held = self
            .devices
            .get(&device)
            .and_then(|d| d.batch_resume.get(&i))
            .copied()
            .unwrap_or(0);
        if held == 0 || held >= size.bytes() {
            return size;
        }
        WireSize::real(size.bytes() - held, "batch zip remainder (Range, HTTP 206)")
    }

    /// Whether `device` is waiting for `kind`, clearing the wait when it is. An answer it
    /// is not waiting for (the second answer to a request it sent twice) is dropped by the
    /// device and counted.
    fn take_answer(&mut self, device: NodeId, kind: Awaited) -> bool {
        let Some(dev) = self.devices.get_mut(&device) else {
            return false;
        };
        if dev.awaiting.as_ref().is_some_and(|a| a.1 == kind) {
            dev.awaiting = None;
            true
        } else {
            dev.duplicate_answers += 1;
            false
        }
    }

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
                    // A successor whose validity has begun signs from now on.
                    req.enrolment = dev.roll_enrolment(at);
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
                // A device the RA already pre-generates for does not request again
                // (`ra-maxReqs` = 1): it asks `/download/info` when its next batch is
                // ready, in the same session as the file check ([CAMP122] §3 step 8).
                let topoff = flow == FlowId::Topup
                    && self
                        .devices
                        .get(&req.device)
                        .is_some_and(|d| d.ra_account.is_some());
                if topoff {
                    let pdu = self.ee_info_request_pdu(req.device, at);
                    if pdu.is_some() {
                        Self::sign(out, 1);
                        Self::scalar_mults(out, 1);
                    }
                    if let Some(dev) = self.devices.get_mut(&req.device) {
                        dev.info_requests += 1;
                    }
                    out.stage_at(StageId::Requested, req.device, None, flow, run);
                    let size = match &pdu {
                        Some(p) => {
                            self.emit_pdu(SpduKind::EeRaDownloadRequest, req.device, n.lop, at, p)
                        }
                        None => self.sizes.batch_download_request(),
                    };
                    let (device, i, enrolment) = (req.device, req.start_i, req.enrolment);
                    // A retry repeats the whole session: the file check and the `.info`
                    // request, signed afresh.
                    self.await_answer(
                        out,
                        device,
                        Awaited::Info,
                        ScmsMsg::ProvisioningRequest(req),
                        flow,
                        run,
                    );
                    out.send(
                        n.lop,
                        ScmsMsg::InfoRequest {
                            device,
                            i,
                            enrolment,
                            pdu,
                        },
                        "info-request",
                        size,
                        Transport::CellularUu,
                        flow,
                        run,
                    );
                    return Ok(());
                }
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
                // A retry (SCMS-522) sends the same request again, byte for byte, as an
                // HTTP client repeats a POST: its `requestHash`, which names the batch
                // files, must not change.
                if req.spdu.is_none() {
                    req.spdu = self.ee_ra_request_pdu(&req, at);
                }
                let size = match req.spdu.clone() {
                    Some(p) => self.emit_pdu(SpduKind::EeRaCertRequest, req.device, n.lop, at, &p),
                    None => self.sizes.provisioning_request_for(certs),
                };
                self.await_answer(
                    out,
                    req.device,
                    Awaited::Ack,
                    ScmsMsg::ProvisioningRequest(req.clone()),
                    flow,
                    run,
                );
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
                let mut signer_id8 = None;
                let request_hash8 = match req.spdu.clone() {
                    Some(p) if self.pki.is_some() => match self.ra_open_request(&req, &p) {
                        Ok(id8) => {
                            signer_id8 = Some(id8);
                            Some(p.hashed_id8())
                        }
                        Err(e) => {
                            // A request the RA cannot open or whose signer it does not
                            // trust is refused with an answer, as an HTTP service answers
                            // a bad POST: without one the device's SCMS-522 retry would
                            // send it again for ever. ASSUMPTION: [CAMP122] gives no error
                            // code for this case; `UnknownEnrolment` is the nearest.
                            self.reject_pdu(SpduKind::EeRaCertRequest, to, at, e);
                            self.refuse_unopened(out, req.device, flow, run);
                            return Ok(());
                        }
                    },
                    _ => None,
                };
                // The checks IEEE 1609.2.1 has the RA make before it spends anything: the
                // enrolment certificate the request is signed with was registered with the
                // RA by the EE operator ([SMP] §2.1.8; with the PDU layer on, that very
                // certificate), is inside its validity, and is not blocklisted. A refusal
                // is an answer, not silence, so the device knows whether to stop or to
                // renew.
                let registered = self.ra.is_registered(req.device, signer_id8);
                if !registered && req.enrolment.is_some() {
                    self.ra.unregistered_refused += 1;
                }
                let refusal = if self.ra.blocklist.contains(&req.device) {
                    Some(Refusal::Blocklisted)
                } else {
                    match req.enrolment {
                        Some(ec) if registered => {
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
                // An SCMS-522 retry of a request the RA already accepted (the same bytes,
                // or on the modelled path the same request): the device never saw the
                // ack, so the RA acknowledges it again instead of refusing it as a second
                // request or expanding it twice. ASSUMPTION: [CAMP122] states the retry,
                // not the RA's answer to it; an idempotent answer is what an HTTP service
                // gives a repeated POST.
                let retransmitted = self.ra.accounts.get(&req.device).and_then(|a| {
                    let same = match (request_hash8, a.request_hash8) {
                        (Some(x), Some(y)) => x == y,
                        (None, None) => {
                            let mut probe = (*req).clone();
                            probe.spdu = None;
                            probe.jmax = a.request.jmax;
                            probe.periods = a.request.periods;
                            request_hash(&probe) == a.request_hash
                        }
                        _ => false,
                    };
                    same.then_some((
                        a.request_hash,
                        a.request.start_i,
                        a.request.periods,
                        a.request.jmax,
                    ))
                });
                if let Some((request_hash, start_i, periods, jmax)) = retransmitted {
                    self.ra.retransmissions_reacked += 1;
                    Self::sign(out, 1);
                    let ack = request_hash8.and_then(|h| self.ra_ack_pdu(h, start_i, at));
                    let size = match &ack {
                        Some(p) => self.emit_pdu(SpduKind::RaEeCertAck, to, n.lop, at, p),
                        None => self.sizes.provisioning_ack(),
                    };
                    out.send(
                        n.lop,
                        ScmsMsg::ProvisioningAck {
                            device: req.device,
                            request_hash,
                            periods,
                            pdu: ack,
                            jmax,
                        },
                        "provisioning-ack",
                        size,
                        Transport::CellularUu,
                        flow,
                        run,
                    );
                    return Ok(());
                }
                // `ra-maxReqs` = 1: a device the RA already pre-generates for is refused
                // a second request, unless the policy changed since ([CAMP122] SCMS-520,
                // "or again when parameters change"). Recorded: a device re-requesting
                // with fresh keys is how one would try to hoard pseudonyms.
                if self
                    .ra
                    .accounts
                    .get(&req.device)
                    .is_some_and(|a| a.lpf_version == self.gov.lpf.version)
                {
                    self.ra.max_reqs_refused += 1;
                    Self::sign(out, 1);
                    out.send(
                        n.lop,
                        ScmsMsg::ProvisioningRefused {
                            device: req.device,
                            reason: Refusal::RequestLimit,
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
                let mut cocoon_enc = BTreeMap::new();
                for i in req.start_i..req.start_i + req.periods {
                    for j in 0..req.jmax {
                        let (b, q) = butterfly::ra_cocoon_keys(
                            &req.signing_public,
                            &req.encryption_public,
                            &req.ck,
                            &req.ek,
                            i,
                            j,
                        );
                        cocoons.insert((i, j), b);
                        cocoon_enc.insert((i, j), q);
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
                    cocoon_enc,
                    plv: [BTreeMap::new(), BTreeMap::new()],
                    enc_plv: [BTreeMap::new(), BTreeMap::new()],
                    aca_requests: BTreeMap::new(),
                    request_hash8,
                    lci: [None, None],
                    lci_sealed: [BTreeMap::new(), BTreeMap::new()],
                    responses: 0,
                    periods_certified: 0,
                    sent: false,
                    batch_ready_stamped: false,
                    pregen: false,
                });
                // Pre-generation expands the butterfly keys the request carries. Under a
                // hybrid scheme there is no post-quantum butterfly (`crate::hybrid`): the
                // post-quantum key of each certificate is a fresh upload, so the RA cannot
                // pre-generate, opens no account, and the device requests again per top-up.
                if self.params.hybrid.is_none() {
                    let mut account_req = (*req).clone();
                    account_req.spdu = None;
                    self.ra.accounts.insert(
                        req.device,
                        PregenAccount {
                            request: account_req,
                            request_hash,
                            request_hash8,
                            first_i: req.start_i,
                            accepted_at: at,
                            lpf_version: self.gov.lpf.version,
                            generated: (req.start_i..req.start_i + req.periods).collect(),
                        },
                    );
                }

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
                            chain: None,
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
                request_hash,
                periods,
                pdu,
                ..
            } => {
                // The device verifies the RA's signature and that the ack answers the
                // request it sent; an ack that does not is ignored, and recorded (and the
                // device keeps waiting for the real one).
                if let Some(p) = &pdu {
                    if let Err(e) = self.ee_check_ack(device, p) {
                        self.reject_pdu(SpduKind::RaEeCertAck, device, at, e);
                        return Ok(());
                    }
                }
                // A second ack, to a request the device sent twice, is not acted on: the
                // first already started the download.
                if !self.take_answer(device, Awaited::Ack) {
                    return Ok(());
                }
                // No account under a hybrid scheme (no post-quantum butterfly to
                // pre-generate from): the device's next top-up is a new request.
                if self.params.hybrid.is_none()
                    && let Some(dev) = self.devices.get_mut(&device)
                {
                    dev.ra_account = Some(request_hash);
                }
                // What the RA accepted, which may be less than was asked.
                if let Some(p) = self
                    .devices
                    .get_mut(&device)
                    .and_then(|d| d.provisioning.as_mut())
                {
                    p.periods = periods;
                }
                // The device now waits for the first batch time and polls the repository:
                // `nextDlTime` from the ack on the real path (it read it from the bytes),
                // the RA's same estimate on the modelled one.
                let wait = match &pdu {
                    Some(p) => self.ack_next_dl(p).map_or_else(
                        || self.params.first_batch_delay,
                        |t| self.wait_until_time32(at, t),
                    ),
                    None => Duration::from_nanos(self.next_download_at(at).saturating_sub(at)),
                };
                out.start_timer(
                    wait,
                    ScmsMsg::BatchDownloadRequest {
                        device,
                        i: self
                            .devices
                            .get(&device)
                            .and_then(|d| d.provisioning)
                            .map_or(0, |p| p.start_i),
                        pdu: None,
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
                chain,
            } => {
                let idx = la.idx();
                // A pre-generated week of a request this LA already holds a chain for
                // walks that chain forward; anything else gets a new chain.
                let held = chain.and_then(|c| self.la[idx].chains.get(&c).map(|s| (c, *s)));
                let (lci, ls0) = if let Some(h) = held {
                    h
                } else {
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
                    let st = &mut self.la[idx];
                    st.chains.insert(lci, ls0);
                    st.owner.insert(lci, device);
                    (lci, ls0)
                };
                let la_id = self.la[idx].la_id;

                // One hash per period to walk the chain, two AES blocks per pre-linkage
                // value ([BRECHT §V-B]).
                Self::hashes(out, periods);
                Self::aes(out, periods.saturating_mul(jmax).saturating_mul(2));

                let mut values = Vec::new();
                let mut sealed_lci = Vec::new();
                let mut encrypted = Vec::new();
                // The real path: each PLV sealed with AES-128-CCM under the key this LA
                // shares with the ACA (`EncryptedIndividualPLV`, 02 §10.3).
                let seal = match (self.pki.is_some(), self.la_aca) {
                    (true, Some(k)) => Some(k.current(la_id.0, self.time32(at))),
                    _ => None,
                };
                for i in start_i..start_i + periods {
                    let ls_i = linkage::linkage_seed_at(la_id, ls0, i);
                    for j in 0..jmax {
                        let plv = linkage::pre_linkage_value(la_id, ls_i, j);
                        if let Some(key) = &seal {
                            if let Ok(e) = pdu_prov::encrypted_individual_plv(
                                la_id.0,
                                &plv,
                                key,
                                &mut self.pdu_rng,
                            ) {
                                encrypted.push(e);
                            }
                        }
                        values.push((i, j, SealedForPca::seal(plv)));
                        // The LA's own encryption of its chain identifier, fresh per
                        // value: the nonce comes from the seed only this LA holds.
                        let mut m = Vec::with_capacity(40);
                        m.extend_from_slice(b"lci-nonce");
                        m.extend_from_slice(ls_i.as_bytes());
                        m.extend_from_slice(&j.to_be_bytes());
                        let mut nonce = [0u8; 16];
                        nonce.copy_from_slice(&sha256(&m)[..16]);
                        sealed_lci.push(SealedForLa::seal(la, nonce, lci));
                    }
                }
                let count = u32::try_from(values.len()).unwrap_or(u32::MAX);
                self.la[idx].plv_issued += u64::from(count);
                let size = if encrypted.is_empty() {
                    self.sizes.pre_linkage_response(count)
                } else {
                    // Per PLV: the data key wrapped (16 B: 2⌈16/16⌉+2 = 4 AES calls) and
                    // the 9-byte value (4 calls), CCM as C4 prices it.
                    Self::aes(out, count.saturating_mul(8));
                    let mut real = 0u32;
                    for e in &encrypted {
                        if let Ok(b) = pdu_prov::encode_plv(e) {
                            let p = Pdu::new(b);
                            real = real.saturating_add(
                                self.emit_pdu(SpduKind::EncryptedIndividualPlv, to, n.ra, at, &p)
                                    .bytes(),
                            );
                        }
                    }
                    WireSize::real(real, "EncryptedIndividualPLV x n")
                };
                out.send(
                    n.ra,
                    ScmsMsg::PreLinkageResponse(Box::new(PreLinkageBatch {
                        la,
                        la_id,
                        lci,
                        values,
                        sealed_lci,
                        encrypted,
                    })),
                    "pre-linkage-response",
                    size,
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
                let real = batch.encrypted.len() == batch.values.len();
                let mut enc = batch.encrypted.into_iter();
                let mut sealed = batch.sealed_lci.into_iter();
                for (i, j, plv) in batch.values {
                    job.plv[idx].insert((i, j), plv);
                    if let Some(l) = sealed.next() {
                        job.lci_sealed[idx].insert((i, j), l);
                    }
                    if real {
                        if let Some(e) = enc.next() {
                            job.enc_plv[idx].insert((i, j), e);
                        }
                    }
                }
                job.responses += 1;
                let hash = job.request_hash;
                let ready = job.responses == 2;
                let pregen = job.pregen;
                let lcis = job.lci;
                if let Some(rec) = self.ra.requests.get_mut(&hash) {
                    rec.lci1 = lcis[0];
                    rec.lci2 = lcis[1];
                }
                if ready && pregen {
                    // A pre-generated week: the weekly job's population shuffle is not
                    // the initial request's age-bound one; it goes to the ACA now.
                    out.stage_at(StageId::PreLinkageReady, to, None, flow, run);
                    self.release_jobs(out, n, to, at, Some(run));
                } else if ready {
                    out.stage_at(StageId::PreLinkageReady, to, None, flow, run);
                    let waiting = self
                        .ra
                        .jobs
                        .iter()
                        .filter(|j| j.responses >= 2 && !j.sent)
                        .count();
                    if waiting >= self.params.shuffle_threshold as usize {
                        // The count closes this batch before the window does.
                        self.release_request_shuffle(out, n, to, at);
                    } else if !self.ra.shuffle_armed {
                        self.ra.shuffle_armed = true;
                        self.ra.shuffle_deadline = self.params.shuffle_window.after(at);
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
                    self.release_request_shuffle(out, n, to, at);
                }
            }
            ScmsMsg::CertRequest { i, items } => {
                let count = u32::try_from(items.len()).unwrap_or(u32::MAX);
                let real = self.pki.is_some() && items.iter().all(|it| it.spdu.is_some());
                if real {
                    // The real path (WP-B3a): the ACA reads only the RA-signed request.
                    // Per certificate: verify the RA's signature, two AES-CCM PLV
                    // decryptions (8 AES calls each, as the LA's sealing), one ECQV
                    // issuance, one ECIES encryption to J_ij, and two signatures (the
                    // private response and the AcaRaCertResponse around it).
                    Self::verify(out, count);
                    Self::aes(out, count.saturating_mul(16));
                    Self::scalar_mults(out, count);
                    Self::encrypt(out, count);
                    // A committee ACA signs through its session instead (below).
                    let committee = self.committees.role("pca");
                    if committee.is_none() {
                        Self::sign(out, count.saturating_mul(2));
                    }
                    let pki = self.pki.clone().expect("checked");
                    let keys = self.la_aca.unwrap_or(pdu_prov::LaAcaKeys::new(0, 0));
                    let now32 = self.time32(at);
                    let mut credentials = Vec::new();
                    // Each certificate's issuance is a pure function of its request and
                    // the generator's block, so they run side by side and are taken in
                    // order (`pdu::par_map_drbg`: the bytes of the sequential loop).
                    let issued = pdu::par_map_drbg(&mut self.pdu_rng, items.len(), |k, g| {
                        let spdu = items[k].spdu.as_ref().expect("checked");
                        pdu_prov::aca_issue(&pki, &keys, spdu.bytes(), now32, g)
                    });
                    for (item, issued) in items.into_iter().zip(issued) {
                        let (lci1, lci2) = (item.lci1, item.lci2);
                        match issued {
                            Ok(iss) => {
                                self.pca.issued.insert(
                                    (u32::from(iss.i), *iss.lv.as_bytes()),
                                    PcaLookup {
                                        lci1,
                                        lci2,
                                        handle: item.handle,
                                    },
                                );
                                self.pca.issued_count += 1;
                                // A carrier for the response: the RA reads only
                                // `response`; the device derives everything else.
                                credentials.push(IssuedCredential {
                                    i: u32::from(iss.i),
                                    j: item.j,
                                    lv: iss.lv,
                                    certified_public: item.cocoon_signing,
                                    c: p256::Scalar::ZERO,
                                    response: Some(iss.to_ra),
                                    implicit: None,
                                });
                            }
                            Err(e) => self.reject_pdu(SpduKind::RaAcaCertRequest, to, at, e),
                        }
                    }
                    if let Some(role) = committee {
                        // Two committee signatures per certificate (the private response
                        // and the AcaRaCertResponse). The SPDUs carry the single ACA key's
                        // signature as a stand-in for the threshold one (same length,
                        // P-256 ECDSA).
                        let n_sig = u32::try_from(credentials.len())
                            .unwrap_or(u32::MAX)
                            .saturating_mul(2);
                        self.committees.start(
                            role,
                            n_sig,
                            Pending::IssueBatch {
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
                    self.send_batch(i, credentials, flow, run, at, out);
                    return Ok(());
                }
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
                    let (lci1, lci2) = (item.lci1, item.lci2);
                    let (Some(plv1), Some(plv2)) =
                        (item.plv1.open(to, n.pca), item.plv2.open(to, n.pca))
                    else {
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
                            handle: item.handle,
                        },
                    );
                    self.pca.issued_count += 1;
                    credentials.push(IssuedCredential {
                        i: item.i,
                        j: item.j,
                        lv,
                        certified_public,
                        c,
                        response: None,
                        implicit: None,
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
                self.send_batch(i, credentials, flow, run, at, out);
            }
            ScmsMsg::CertResponse { i, credentials } => {
                // The PCA does not know whose batch this is; the RA routes it by its own
                // job (the request it holds for this run).
                let Some(device) = self.ra.jobs.iter().find(|j| j.run == run).map(|j| j.device)
                else {
                    self.ra.unrouted_responses += 1;
                    return Ok(());
                };
                // The real path: the RA verifies each ACA-signed answer, routes it by the
                // request hash to the `(i, j)` it kept, and files the private response as
                // `i_j` in `<requestHash>_<i>.zip`. It cannot open what it files.
                let mut batch = RepoBatch::default();
                let pki = self.pki.clone();
                let job = self.ra.jobs.iter().position(|j| j.run == run);
                let mut opened = 0u32;
                // Every signature check at once (pure), each answer then filed in order.
                let checked: Vec<Option<_>> = {
                    use rayon::prelude::*;
                    credentials
                        .par_iter()
                        .map(|c| match (&c.response, pki.as_deref()) {
                            (Some(resp), Some(pki)) => {
                                Some(pdu_prov::ra_open_aca_response(pki, resp.bytes()))
                            }
                            _ => None,
                        })
                        .collect()
                };
                for (c, checked) in credentials.into_iter().zip(checked) {
                    match checked {
                        Some(checked) => {
                            opened += 1;
                            match checked {
                                Ok((rh, private)) => {
                                    let ij = job.and_then(|k| {
                                        self.ra.jobs[k].aca_requests.get(&rh).copied()
                                    });
                                    match ij {
                                        Some((ii, jj)) if ii == i => {
                                            batch.files.push((jj, private));
                                        }
                                        _ => self.ra.unrouted_responses += 1,
                                    }
                                }
                                Err(e) => {
                                    self.reject_pdu(SpduKind::AcaRaCertResponse, to, at, e);
                                }
                            }
                        }
                        _ => batch.modelled.push(c),
                    }
                }
                Self::verify(out, opened);
                batch.files.sort_by_key(|(j, _)| *j);
                if let Some(h) = job.and_then(|k| self.ra.jobs[k].request_hash8) {
                    batch.name = pdu::batch_filename(h, i);
                }
                self.ra.repos.entry(device).or_default().insert(i, batch);
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
            ScmsMsg::BatchDownloadRequest { device, i, .. } if to == device => {
                // The real `EeRaDownloadRequestSpdu`: the file name, signed by the
                // enrolment certificate, encrypted to the RA.
                let pdu = self.ee_download_request_pdu(device, i, at);
                if pdu.is_some() {
                    Self::sign(out, 1);
                    Self::scalar_mults(out, 1);
                }
                let size = match &pdu {
                    Some(p) => self.emit_pdu(SpduKind::EeRaDownloadRequest, device, n.lop, at, p),
                    None => self.sizes.batch_download_request(),
                };
                self.await_answer(
                    out,
                    device,
                    Awaited::Download(i),
                    ScmsMsg::BatchDownloadRequest {
                        device,
                        i,
                        pdu: None,
                    },
                    flow,
                    run,
                );
                out.send(
                    n.lop,
                    ScmsMsg::BatchDownloadRequest { device, i, pdu },
                    "batch-download-request",
                    size,
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::BatchDownloadRequest { device, i, pdu } if to == n.lop => {
                self.lop.upstream += 1;
                let size = match &pdu {
                    Some(p) => self.emit_pdu(SpduKind::EeRaDownloadRequest, to, n.ra, at, p),
                    None => self.sizes.batch_download_request(),
                };
                out.send(
                    n.ra,
                    ScmsMsg::BatchDownloadRequest { device, i, pdu },
                    "batch-download-request-proxied",
                    size,
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::BatchDownloadRequest { device, i, pdu } => {
                // The RA opens the request and serves the file it names, from the folder
                // of the request hash in that name.
                if let (Some(p), Some(pki)) = (&pdu, self.pki.clone()) {
                    Self::decrypt(out, 1);
                    Self::verify(out, 1);
                    let tol = u32::try_from(
                        self.params.download_request_tolerance.as_nanos() / 1_000_000_000,
                    )
                    .unwrap_or(u32::MAX);
                    match pdu_prov::ra_open_download_request(&pki, p.bytes(), self.time32(at), tol)
                    {
                        Ok(o) => {
                            let held = self
                                .ra
                                .repos
                                .get(&device)
                                .and_then(|r| r.get(&i))
                                .map(|b| b.name.clone());
                            let expected = self
                                .devices
                                .get(&device)
                                .and_then(|d| d.request_hash8)
                                .map(|h| pdu::batch_filename(h, i));
                            if held.is_some_and(|h| !h.is_empty() && h != o.filename)
                                || expected.is_some_and(|e| e != o.filename)
                            {
                                self.reject_pdu(
                                    SpduKind::EeRaDownloadRequest,
                                    to,
                                    at,
                                    PduError::Shape("filename names another device's batch"),
                                );
                                self.refuse_unopened(out, device, flow, run);
                                return Ok(());
                            }
                        }
                        Err(e) => {
                            self.reject_pdu(SpduKind::EeRaDownloadRequest, to, at, e);
                            self.refuse_unopened(out, device, flow, run);
                            return Ok(());
                        }
                    }
                }
                let batch = self
                    .ra
                    .repos
                    .get(&device)
                    .and_then(|r| r.get(&i))
                    .cloned()
                    .unwrap_or_default();
                let files = batch.files;
                let credentials = batch.modelled;
                let count = u32::try_from(credentials.len()).unwrap_or(u32::MAX);
                let size = if files.is_empty() {
                    self.sizes.batch_download(count)
                } else {
                    let mut entries = Vec::with_capacity(files.len());
                    for (j, p) in &files {
                        self.emit_pdu(SpduKind::AcaEeCertResponsePrivate, to, n.lop, at, p);
                        entries.push((pdu_prov::batch_entry_name(i, *j), p.len()));
                    }
                    WireSize::real(pdu_prov::stored_zip_len(&entries), "batch zip (stored)")
                };
                let size = self.range_remainder(device, i, size);
                // The batch file travels back through the proxy, encrypted end to end: each
                // certificate to the device's butterfly encryption key, which neither the
                // RA nor the LOP holds.
                out.send(
                    n.lop,
                    ScmsMsg::BatchDownload {
                        device,
                        i,
                        credentials: Box::new(credentials),
                        files: Box::new(files),
                    },
                    "batch-download",
                    size,
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::BatchDownload {
                device,
                i,
                credentials,
                files,
            } if to == n.lop => {
                self.lop.downstream += 1;
                let count = u32::try_from(credentials.len()).unwrap_or(u32::MAX);
                let size = if files.is_empty() {
                    self.sizes.batch_download(count)
                } else {
                    // The device leg carries the same files, as the ack's does: with no
                    // LOP deployed the view folds the RA-LOP leg away, and this copy is
                    // the one the records keep.
                    let mut entries = Vec::with_capacity(files.len());
                    for (j, p) in files.iter() {
                        self.emit_pdu(SpduKind::AcaEeCertResponsePrivate, to, device, at, p);
                        entries.push((pdu_prov::batch_entry_name(i, *j), p.len()));
                    }
                    WireSize::real(pdu_prov::stored_zip_len(&entries), "batch zip (stored)")
                };
                let size = self.range_remainder(device, i, size);
                out.send(
                    device,
                    ScmsMsg::BatchDownload {
                        device,
                        i,
                        credentials,
                        files,
                    },
                    "batch-download-proxied",
                    size,
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::BatchDownload {
                device,
                i,
                credentials,
                files,
            } => {
                if !self.take_answer(device, Awaited::Download(i)) {
                    return Ok(());
                }
                if let Some(dev) = self.devices.get_mut(&device)
                    && dev.batch_resume.remove(&i).is_some()
                {
                    dev.range_resumes += 1;
                }
                let poll = self.params.download_poll_interval;
                let cap = self.params.max_download_polls;
                // The real path: open every `i_j` with h_ij, check the issuing ACA signed
                // it, and derive d = e·b_ij + r against e·P_U + Q_ACA. A file that fails
                // is refused and recorded; the rest install.
                let mut credentials = *credentials;
                if !files.is_empty() {
                    let pki = self.pki.clone();
                    let cat = self
                        .devices
                        .get(&device)
                        .and_then(|d| d.caterpillar.clone());
                    let n_files = u32::try_from(files.len()).unwrap_or(u32::MAX);
                    Self::verify(out, n_files);
                    Self::decrypt(out, n_files);
                    if let (Some(pki), Some(cat)) = (pki, cat) {
                        // Each file opens on its own (pure); refusals are recorded in order.
                        let opened: Vec<_> = {
                            use rayon::prelude::*;
                            files
                                .par_iter()
                                .map(|(j, p)| {
                                    pdu_prov::ee_open_aca_response(&pki, p.bytes(), &cat, i, *j)
                                })
                                .collect()
                        };
                        for ((j, _), opened) in files.iter().zip(opened) {
                            match opened {
                                Ok(o) => credentials.push(IssuedCredential {
                                    i,
                                    j: *j,
                                    lv: o.lv,
                                    certified_public: o.public,
                                    c: p256::Scalar::ZERO,
                                    response: None,
                                    implicit: Some(Box::new(
                                        crate::scms::msg::ImplicitCredential {
                                            aca_id8: pdu::cert_id8(&o.aca),
                                            certificate: std::sync::Arc::new(o.certificate),
                                            e: o.e,
                                            r: o.r,
                                            private: o.private,
                                        },
                                    )),
                                }),
                                Err(e) => self.reject_pdu(
                                    SpduKind::AcaEeCertResponsePrivate,
                                    device,
                                    at,
                                    e,
                                ),
                            }
                        }
                    }
                }
                let Some(dev) = self.devices.get_mut(&device) else {
                    return Err(ProtoError::NoEntity { node: device });
                };
                if credentials.is_empty() {
                    dev.download_retries += 1;
                    if dev.download_retries <= cap {
                        out.start_timer(
                            poll,
                            ScmsMsg::BatchDownloadRequest {
                                device,
                                i,
                                pdu: None,
                            },
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
                    // The device's next request goes out as its own message, so it is
                    // built (and signed) by the device handler like the first.
                    out.start_timer(
                        Duration::ZERO,
                        ScmsMsg::BatchDownloadRequest {
                            device,
                            i: i_next,
                            pdu: None,
                        },
                        flow,
                        run,
                    );
                }
            }
            ScmsMsg::InfoRequest {
                device,
                i,
                enrolment,
                pdu,
            } if to == n.lop => {
                self.lop.upstream += 1;
                out.stage_at(StageId::ProxyForwarded, to, None, flow, run);
                let size = match &pdu {
                    Some(p) => self.emit_pdu(SpduKind::EeRaDownloadRequest, to, n.ra, at, p),
                    None => self.sizes.batch_download_request(),
                };
                out.send(
                    n.ra,
                    ScmsMsg::InfoRequest {
                        device,
                        i,
                        enrolment,
                        pdu,
                    },
                    "info-request-proxied",
                    size,
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::InfoRequest {
                device,
                i,
                enrolment,
                pdu,
            } => {
                // The RA authenticates the `Download-Req` exactly as a batch download's:
                // enrolment-signed, fresh within the client-signature TTL, naming this
                // device's own `.info` file.
                let mut signer_id8 = None;
                if let (Some(p), Some(pki)) = (&pdu, self.pki.clone()) {
                    Self::decrypt(out, 1);
                    Self::verify(out, 1);
                    let tol = u32::try_from(
                        self.params.download_request_tolerance.as_nanos() / 1_000_000_000,
                    )
                    .unwrap_or(u32::MAX);
                    match pdu_prov::ra_open_download_request(&pki, p.bytes(), self.time32(at), tol)
                    {
                        Ok(o) => {
                            let own = self.ra.accounts.get(&device).and_then(|a| a.request_hash8);
                            let named = match pdu_prov::parse_download_filename(&o.filename) {
                                Some(pdu_prov::DownloadFile::Info { request_hash8 }) => {
                                    Some(request_hash8)
                                }
                                _ => None,
                            };
                            if named.is_none() || named != own {
                                self.reject_pdu(
                                    SpduKind::EeRaDownloadRequest,
                                    to,
                                    at,
                                    PduError::Shape("the .info file names another request"),
                                );
                                self.refuse_unopened(out, device, flow, run);
                                return Ok(());
                            }
                            signer_id8 = Some(pdu::cert_id8(&o.enrolment));
                        }
                        Err(e) => {
                            self.reject_pdu(SpduKind::EeRaDownloadRequest, to, at, e);
                            self.refuse_unopened(out, device, flow, run);
                            return Ok(());
                        }
                    }
                } else {
                    Self::verify(out, 1);
                }
                // The request's own checks, on every top-off: registered, valid, not
                // blocklisted, and an account to top off.
                let registered = self.ra.is_registered(device, signer_id8);
                if !registered && enrolment.is_some() {
                    self.ra.unregistered_refused += 1;
                }
                let refusal = if self.ra.blocklist.contains(&device) {
                    Some(Refusal::Blocklisted)
                } else {
                    match enrolment {
                        Some(ec) if registered && self.ra.accounts.contains_key(&device) => {
                            (!ec.valid_at(at)).then_some(Refusal::EnrolmentExpired)
                        }
                        _ => Some(Refusal::UnknownEnrolment),
                    }
                };
                if let Some(reason) = refusal {
                    if reason == Refusal::Blocklisted {
                        self.ra.refused += 1;
                    } else {
                        self.ra.refused_enrolment += 1;
                    }
                    Self::sign(out, 1);
                    out.send(
                        n.lop,
                        ScmsMsg::ProvisioningRefused { device, reason },
                        "provisioning-refused",
                        self.sizes.refusal(),
                        Transport::BackendNet,
                        flow,
                        run,
                    );
                    return Ok(());
                }
                self.ra.info_served += 1;
                out.stage_at(StageId::Acknowledged, to, None, flow, run);
                let next_dl = self.pregen_next(device, i, at, out, flow, run);
                let body = match (next_dl, self.pki.is_some()) {
                    (Some(t), true) => pdu_prov::info_body(self.time32(t)).ok(),
                    _ => None,
                };
                let size = match &body {
                    Some(b) => {
                        WireSize::real(u32::try_from(b.len()).unwrap_or(u32::MAX), ".info Time32")
                    }
                    None => self.sizes.provisioning_ack(),
                };
                out.send(
                    n.lop,
                    ScmsMsg::InfoResponse {
                        device,
                        i,
                        next_dl,
                        body,
                    },
                    "info-response",
                    size,
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::PregenTimer { device, i } => {
                self.ra.pregen_timers.remove(&(device, i));
                let held = self
                    .ra
                    .accounts
                    .get(&device)
                    .is_some_and(|a| a.generated.contains(&i));
                if !held {
                    self.start_pregen_job(device, i, out, flow, run);
                }
            }
            ScmsMsg::InfoResponse {
                device,
                i,
                next_dl,
                body,
            } if to == n.lop => {
                self.lop.downstream += 1;
                let size = match &body {
                    Some(b) => {
                        WireSize::real(u32::try_from(b.len()).unwrap_or(u32::MAX), ".info Time32")
                    }
                    None => self.sizes.provisioning_ack(),
                };
                out.send(
                    device,
                    ScmsMsg::InfoResponse {
                        device,
                        i,
                        next_dl,
                        body,
                    },
                    "info-response-proxied",
                    size,
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::InfoResponse {
                device,
                i,
                next_dl,
                body,
            } => {
                if !self.take_answer(device, Awaited::Info) {
                    return Ok(());
                }
                // The device reads the Time32 it was sent (the real path) and comes back
                // for the batch then.
                let at_dl = match &body {
                    Some(b) => pdu_prov::open_info_body(b)
                        .ok()
                        .map(|t| self.wait_until_time32(at, t).after(at)),
                    None => next_dl,
                };
                let Some(when) = at_dl else {
                    if let Some(dev) = self.devices.get_mut(&device) {
                        dev.topoffs_unavailable += 1;
                        dev.provisioning = None;
                    }
                    return Ok(());
                };
                out.start_timer(
                    Duration::from_nanos(when.saturating_sub(at)),
                    ScmsMsg::BatchDownloadRequest {
                        device,
                        i,
                        pdu: None,
                    },
                    flow,
                    run,
                );
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
            ScmsMsg::EeRetryTimer { device, seq } => {
                // SCMS-522: no answer within the timeout. SCMS-523: at most
                // `ee_retry_max_per_hour` retries in any 60 minutes; past that the device
                // waits for the oldest to leave the window. (`ScmsRun::step` has already
                // dropped a timer whose request was answered.)
                let window = Duration::from_secs(crate::scms::params::EE_RETRY_WINDOW_S);
                let cap = self.params.ee_retry_max_per_hour as usize;
                let Some(dev) = self.devices.get_mut(&device) else {
                    return Ok(());
                };
                let Some((s, _, msg, f, r)) = dev.awaiting.clone() else {
                    return Ok(());
                };
                if s != seq || cap == 0 {
                    return Ok(());
                }
                dev.retry_log.retain(|t| window.after(*t) > at);
                if dev.retry_log.len() >= cap {
                    dev.ee_retries_capped += 1;
                    let wait = window.after(dev.retry_log[0]).saturating_sub(at);
                    out.start_timer(
                        Duration::from_nanos(wait),
                        ScmsMsg::EeRetryTimer { device, seq },
                        f,
                        r,
                    );
                    return Ok(());
                }
                dev.retry_log.push(at);
                dev.ee_retries += 1;
                dev.awaiting = None;
                out.start_timer(Duration::ZERO, msg, f, r);
            }
            ScmsMsg::ProvisioningRefused { device, reason } => {
                Self::verify(out, 1);
                if let Some(dev) = self.devices.get_mut(&device) {
                    dev.awaiting = None;
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
