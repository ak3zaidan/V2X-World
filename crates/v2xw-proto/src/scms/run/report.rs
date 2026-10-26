//! Misbehaviour reporting: device → (LOP) → RA shuffle → MA.
//!
//! Split out of `run.rs` without behaviour change (WP-B1); the state types and the shared
//! helpers stay in [`super`].

use super::*;
use crate::scms::msg::SecuredReport;
use crate::scms::pdu::SpduKind;

impl ScmsRun {
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
                secured: None,
            })),
            FlowId::Report,
            run,
        );
        run
    }

    /// Submits a real US misbehaviour report (WP-B4; `crate::scms::mbr`): encoded, signed
    /// by the reporter's active pseudonym certificate under PSID 0x26, encrypted to the
    /// MA whose `MaSsp.relevantPsids` covers 0x20, and POSTed through the proxy to the RA
    /// with a `Download-Req` header signed by the reporter's enrolment certificate.
    ///
    /// The accused certificate is the one in the evidence (`signingCertificate`): its
    /// `linkageData` names the `(iCert, linkage value)` the investigation starts from.
    ///
    /// # Errors
    /// The PDU layer is off, the reporter has no pseudonym or enrolment credential, no MA
    /// covers PSID 0x20, the evidence certificate has no linkage data, or the report does
    /// not encode.
    pub fn submit_mbr(
        &mut self,
        reporter: NodeId,
        report: &crate::scms::mbr::MisbehaviorReport,
    ) -> core::result::Result<FlowRun, crate::scms::pdu::PduError> {
        let now = self.kernel.now();
        let (subject_i, lv, secured) = self.secure_mbr(reporter, report, now)?;
        Ok(self.submit_secured(reporter, subject_i, lv, secured))
    }

    /// The bytes a device uploads for `report` at `at` ([`ScmsRun::submit_mbr`]'s
    /// securing, without sending it): the encrypted report and its enrolment-signed
    /// `Download-Req` header, with the `(iCert, linkage value)` its evidence accuses.
    ///
    /// # Errors
    /// As [`ScmsRun::submit_mbr`].
    pub fn secure_mbr(
        &mut self,
        reporter: NodeId,
        report: &crate::scms::mbr::MisbehaviorReport,
        at: SimTime,
    ) -> core::result::Result<(u32, LinkageValue, SecuredReport), crate::scms::pdu::PduError> {
        self.secure_mbr_as(reporter, report, at, None)
    }

    /// The pseudonym `(i, j)` the reporter signs a report generated at `at` with: the one
    /// whose certificate is `prefer` (the HashedId8 the vehicle signs its BSMs with now),
    /// else the device's active one, else its newest i-period's lowest index; in every
    /// case one whose validity window holds `at` and that no CRL it holds revokes. `None`
    /// when it holds none: a device whose pool has run out cannot sign a report.
    ///
    /// Until the edge-case package the first usable certificate in `(i, j)` order signed
    /// every report, valid or not: after the week rollover every report went out under
    /// last week's `(i, 0)`, an expired certificate, and every report of a week under one
    /// pseudonym (so they were linkable to each other).
    pub fn mbr_signer(
        &self,
        reporter: NodeId,
        at: SimTime,
        prefer: Option<[u8; 8]>,
    ) -> Option<(u32, u32)> {
        let dev = self.state.devices.get(&reporter)?;
        let p = &self.state.params;
        let fits = |i: u32, j: u32| {
            let (from, until) = p.validity(i);
            at >= from
                && at < until
                && !dev.is_revoked(i, j)
                && dev
                    .credentials
                    .get(&(i, j))
                    .is_some_and(|c| c.implicit.is_some())
                && dev.credential_is_usable(i, j)
        };
        if let Some(want) = prefer {
            let hit = dev.credentials.iter().find(|(_, c)| {
                c.implicit
                    .as_ref()
                    .is_some_and(|imp| crate::scms::pdu::cert_id8(&imp.certificate) == want)
            });
            if let Some((&(i, j), _)) = hit
                && fits(i, j)
            {
                return Some((i, j));
            }
        }
        if let Some((i, j)) = dev.store.active()
            && fits(i, j)
        {
            return Some((i, j));
        }
        dev.credentials
            .keys()
            .copied()
            .filter(|&(i, j)| fits(i, j))
            .min_by_key(|&(i, j)| (core::cmp::Reverse(i), j))
    }

    /// [`ScmsRun::secure_mbr`], signed with the pseudonym [`ScmsRun::mbr_signer`] picks
    /// for `prefer`.
    ///
    /// # Errors
    /// As [`ScmsRun::submit_mbr`].
    pub fn secure_mbr_as(
        &mut self,
        reporter: NodeId,
        report: &crate::scms::mbr::MisbehaviorReport,
        at: SimTime,
        prefer: Option<[u8; 8]>,
    ) -> core::result::Result<(u32, LinkageValue, SecuredReport), crate::scms::pdu::PduError> {
        use crate::scms::mbr;
        use crate::scms::pdu::{Pdu, PduError};
        let pki = self
            .state
            .pki
            .clone()
            .ok_or(PduError::Shape("the PDU layer is off"))?;
        let dev = self
            .state
            .devices
            .get(&reporter)
            .ok_or(PduError::Shape("no such device"))?;
        // The pseudonym in use, valid when the report is generated (edge-case package,
        // item 1).
        let cred = self
            .mbr_signer(reporter, at, prefer)
            .and_then(|k| dev.credentials.get(&k))
            .and_then(|c| c.implicit.as_deref())
            .ok_or(PduError::Key(
                "no pseudonym certificate valid at the report's time",
            ))?
            .clone();
        let enrol = dev
            .enrolment_credential
            .clone()
            .ok_or(PduError::Key("no enrolment credential"))?;
        let key = v2xw_sec::sig1609::Signer1609::p256(&ec::scalar_to_be32(&cred.private))
            .ok_or(PduError::Key("pseudonym key"))?;
        let mas: Vec<&crate::scms::pdu::Authority> =
            core::iter::once(&pki.ma).chain(pki.extra.iter()).collect();
        let ma = mas
            .iter()
            .find(|a| mbr::ma_covers(&a.cert, mbr::BSM_APP_PSID))
            .ok_or(PduError::Shape("no MA certificate covers PSID 0x20"))?;
        let ma_enc = ma.enc_public().ok_or(PduError::Key("MA encryption key"))?;
        let v2xw_msg::sec_types::ieee1609_dot2::CertificateId::linkageData(ld) =
            &report.evidence.suspect().cert.0.to_be_signed.id
        else {
            return Err(PduError::Shape(
                "the accused certificate has no linkageData",
            ));
        };
        let subject_i = u32::from(ld.i_cert.0.0);
        let mut lv = [0u8; 9];
        lv.copy_from_slice(&ld.linkage_value.0[..]);
        let t32 = self.state.time32(at);
        let t64 = self.state.wall.time64(at).unwrap_or(0);
        let coer = mbr::encode_report(report)?;
        let body = mbr::secure_report(
            &coer,
            t64,
            &cred.certificate,
            &key,
            &ma.cert,
            &ma_enc,
            &mut self.state.pdu_rng,
        )?;
        let ra_enc = pki
            .ra
            .enc_public()
            .ok_or(PduError::Key("RA encryption key"))?;
        let header =
            mbr::upload_header(t32, &enrol, &pki.ra.cert, &ra_enc, &mut self.state.pdu_rng)?;
        Ok((
            subject_i,
            LinkageValue::new(lv),
            SecuredReport {
                report: Pdu::new(body),
                header: Some(Pdu::new(header)),
            },
        ))
    }

    /// Sends report bytes as `reporter` exactly as given: what an attack study uses to
    /// put a forged, replayed or unauthenticated upload on the wire. The RA and the MA
    /// apply their checks and record what they refuse in `pdu_rejections`.
    pub fn submit_secured(
        &mut self,
        reporter: NodeId,
        subject_i: u32,
        subject_lv: LinkageValue,
        secured: SecuredReport,
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
                secured: Some(secured),
            })),
            FlowId::Report,
            run,
        );
        run
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
        self.at_proxy(
            reporter,
            subject_i,
            subject_lv,
            [detected_at, sent_at, arrive_at],
            transport,
            bytes,
            None,
        )
    }

    /// A real US misbehaviour report (the bytes of [`ScmsRun::secure_mbr`]) whose access
    /// leg the caller carried, as [`ScmsRun::report_at_proxy`]: the leg is charged the
    /// encrypted report plus its `Download-Req` header, and from the proxy on the RA and
    /// the MA apply their real checks (header signature and enrolment, MA decryption,
    /// evidence signatures), recording every refusal in `pdu_rejections`.
    #[allow(clippy::too_many_arguments)]
    pub fn secured_at_proxy(
        &mut self,
        reporter: NodeId,
        subject_i: u32,
        subject_lv: LinkageValue,
        secured: SecuredReport,
        detected_at: SimTime,
        sent_at: SimTime,
        arrive_at: SimTime,
        transport: Transport,
    ) -> FlowRun {
        let bytes = secured.report.len() + secured.header.as_ref().map_or(0, |h| h.len());
        self.at_proxy(
            reporter,
            subject_i,
            subject_lv,
            [detected_at, sent_at, arrive_at],
            transport,
            bytes,
            Some(secured),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn at_proxy(
        &mut self,
        reporter: NodeId,
        subject_i: u32,
        subject_lv: LinkageValue,
        [detected_at, sent_at, arrive_at]: [SimTime; 3],
        transport: Transport,
        bytes: u32,
        secured: Option<SecuredReport>,
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
        let pdu0 = self.state.pdus.len();
        if let Some(s) = secured.as_ref() {
            // The real octets of the device's upload, as the kernel's own device hop
            // records them (`ScmsState::report_size`).
            self.state.emit_pdu(
                SpduKind::SecuredMisbehaviorReport,
                reporter,
                lop,
                sent_at,
                &s.report,
            );
            if let Some(h) = s.header.as_ref() {
                self.state
                    .emit_pdu(SpduKind::EeRaDownloadRequest, reporter, lop, sent_at, h);
            }
        }
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
        // Both PDUs ride this one upload step (WP-E1).
        let step = u32::try_from(self.kernel.steps_since(usize::MAX).1 - 1).ok();
        for p in &mut self.state.pdus[pdu0..] {
            p.step = step;
        }
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
                    secured,
                })),
                flow,
                run,
            },
        );
        run
    }
}

impl ScmsState {
    /// The charged size of a report hop: the real bytes when the report is real (the
    /// encrypted report plus, device → RA, the `Download-Req` header), recorded as an
    /// emitted SPDU; the size model otherwise.
    fn report_size(
        &mut self,
        r: &ReportSubmission,
        from: NodeId,
        to: NodeId,
        at: SimTime,
        record: bool,
    ) -> WireSize {
        let Some(s) = r.secured.as_ref() else {
            return self.sizes.report();
        };
        let header = s.header.as_ref().map_or(0, crate::scms::pdu::Pdu::len);
        if record {
            let p = s.report.clone();
            self.emit_pdu(SpduKind::SecuredMisbehaviorReport, from, to, at, &p);
            if let Some(h) = s.header.clone() {
                self.emit_pdu(SpduKind::EeRaDownloadRequest, from, to, at, &h);
            }
        }
        WireSize::real(
            s.report.len() + header,
            SpduKind::SecuredMisbehaviorReport.name(),
        )
    }

    /// The RA's check of a report upload's `Download-Req` header.
    fn ra_check_upload(
        &self,
        reporter: NodeId,
        s: &SecuredReport,
    ) -> core::result::Result<(), crate::scms::pdu::PduError> {
        use crate::scms::pdu::{self, PduError};
        let pki = self
            .pki
            .as_ref()
            .ok_or(PduError::Shape("the PDU layer is off"))?;
        let h = s.header.as_ref().ok_or(PduError::Shape(
            "a report upload carries a Download-Req header",
        ))?;
        let plain = pki.decrypt_as(&pki.ra, h.bytes())?;
        let (p, signer) = pdu::open_signed_pdu(&plain, pki)?;
        // The signer must be an enrolment certificate the ECA issued (implicit, under the
        // ECA), not a pseudonym or a component certificate.
        let issued_by_eca = matches!(&signer.0.issuer,
            v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier::sha256AndDigest(d)
                if pki.is_role(&pki.eca, &v2xw_sec::hashedid::id8_bytes(d)));
        if !issued_by_eca {
            return Err(PduError::Signer("the enrolment certificate"));
        }
        use v2xw_msg::sec_types::ieee1609_dot2_dot1_ee_ra_interface::EeRaInterfacePdu;
        use v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::ScmsPduContent;
        let ScmsPduContent::ee_ra(EeRaInterfacePdu::eeRaDownloadRequest(d)) = p.content else {
            return Err(PduError::Shape("expected ee-ra.eeRaDownloadRequest"));
        };
        if d.filename.as_str() != crate::scms::mbr::UPLOAD_FILENAME {
            return Err(PduError::Shape("Download-Req names misbehavior-report"));
        }
        if !self
            .ra
            .is_registered(reporter, Some(pdu::cert_id8(&signer)))
        {
            return Err(PduError::UntrustedSigner);
        }
        Ok(())
    }

    /// Closes the RA's report shuffle: every waiting report goes on to the MA. Run by the
    /// window's timer or, first, by the count threshold (CAMP-EE SCMS-765).
    pub(super) fn release_report_shuffle(
        &mut self,
        out: &mut Outbox<ScmsMsg>,
        n: ScmsNodes,
        to: NodeId,
        out_now: SimTime,
    ) {
        self.ra.report_shuffle_armed = false;
        self.ra.report_shuffle_generation += 1;
        // The RA *shuffles* the batch before it goes on (CAMP-EE SCMS-765): the MA must
        // not learn the order in which the reports reached the RA, which would let it
        // correlate a report with its upload. Before, the batch left in arrival order
        // (Kendall tau 1.0 between arrival and forward order in every batch of the
        // misbehaviour-pipeline audit). A seeded Fisher-Yates permutation of its own
        // stream, so no other draw moves and the same seed gives the same order.
        let mut batch = core::mem::take(&mut self.ra.reports);
        {
            let mut g = self.rng.checkout(
                RngDomain::Backend,
                EntityRef::custom("scms/report-shuffle", self.ra.report_shuffle_generation),
            );
            for i in (1..batch.len()).rev() {
                let j = g.below(i as u64 + 1) as usize;
                batch.swap(i, j);
            }
        }
        for (r_run, mut report) in batch {
            out.stage_at(StageId::Shuffled, to, None, FlowId::Report, r_run);
            // The RA keeps the upload's authentication to itself: only the encrypted
            // report goes on to the MA (the LOP/RA strip identifiers, 03 §8.2).
            if let Some(s) = report.secured.as_mut() {
                s.header = None;
            }
            let size = match report.secured.as_ref() {
                Some(s) => {
                    let p = s.report.clone();
                    self.emit_pdu(SpduKind::SecuredMisbehaviorReport, to, n.ma, out_now, &p)
                }
                None => self.sizes.report(),
            };
            out.send(
                n.ma,
                ScmsMsg::Report(Box::new(report)),
                "report-forward",
                size,
                Transport::BackendNet,
                FlowId::Report,
                r_run,
            );
        }
    }

    /// Handles every message of this flow; [`ScmsState::handle`] routes here.
    #[allow(unused_variables)]
    pub(super) fn handle_report(
        &mut self,
        d: Delivery<ScmsMsg>,
        out: &mut Outbox<ScmsMsg>,
    ) -> Result<()> {
        let n = self.nodes;
        let (flow, run, to, at, from) = (d.flow, d.run, d.to, d.at, d.from);
        match d.msg {
            ScmsMsg::Report(r) if to == r.reporter => {
                Self::sign(out, 1);
                Self::scalar_mults(out, 1);
                out.stage_at(StageId::Detect, r.reporter, None, flow, run);
                out.stage_at(StageId::ReportSent, r.reporter, None, flow, run);
                let size = self.report_size(&r, to, n.lop, at, true);
                out.send(
                    n.lop,
                    ScmsMsg::Report(r),
                    "report",
                    size,
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::Report(r) if to == n.lop => {
                self.lop.upstream += 1;
                out.stage_at(StageId::ProxyForwarded, to, None, flow, run);
                let size = self.report_size(&r, to, n.ra, at, false);
                out.send(
                    n.ra,
                    ScmsMsg::Report(r),
                    "report-proxied",
                    size,
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::Report(r) if to == n.ra => {
                // A real upload is accepted only with a valid `Download-Req`: an
                // `EeRaDownloadRequest` for `misbehavior-report`, signed by an enrolment
                // certificate of this PKI and encrypted to the RA ([SM-MBR] §4). The RA
                // cannot open the report itself.
                if let Some(s) = r.secured.as_ref() {
                    Self::decrypt(out, 1);
                    Self::verify(out, 1);
                    if let Err(e) = self.ra_check_upload(r.reporter, s) {
                        self.reject_pdu(SpduKind::SecuredMisbehaviorReport, to, at, e);
                        return Ok(());
                    }
                }
                self.ra.reports.push((run, *r));
                if self.ra.reports.len() >= self.params.report_shuffle_threshold as usize {
                    // The count closes the batch before the day does (CAMP-EE SCMS-765).
                    self.release_report_shuffle(out, n, to, at);
                } else if !self.ra.report_shuffle_armed {
                    self.ra.report_shuffle_armed = true;
                    out.start_timer(
                        self.params.report_shuffle_window,
                        ScmsMsg::ReportShuffleTimer {
                            generation: self.ra.report_shuffle_generation,
                        },
                        flow,
                        run,
                    );
                }
            }
            ScmsMsg::Report(r) => {
                Self::verify(out, 1);
                Self::scalar_mults(out, 1);
                let mut evidence = None;
                if let (Some(s), Some(pki)) = (r.secured.as_ref(), self.ma_trust.clone()) {
                    // The MA decrypts, checks the reporter's pseudonym signature and the
                    // evidence ([SM-MBR] §3; [5GAA] (263)). A report that fails is
                    // recorded and goes no further.
                    Self::decrypt(out, 1);
                    let n_bsm = 1;
                    Self::verify(out, n_bsm);
                    // Under the MA certificate the reporter encrypted to: the current
                    // one, or the one a root rollover superseded.
                    let mut opened = Err(crate::scms::pdu::PduError::Key("MA encryption key"));
                    for ma in pki.in_role(&pki.ma) {
                        let Some(key) = ma.enc_private.as_ref() else {
                            continue;
                        };
                        opened =
                            crate::scms::mbr::ma_open_report(s.report.bytes(), &ma.cert, key, &pki);
                        if opened.is_ok() {
                            break;
                        }
                    }
                    let opened = match opened {
                        Ok(o) => o,
                        Err(e) => {
                            self.reject_pdu(SpduKind::SecuredMisbehaviorReport, to, at, e);
                            return Ok(());
                        }
                    };
                    evidence = Some(super::ReportEvidence {
                        mbr_type: Some(opened.report.mbr_type.number()),
                        type_name: Some(opened.report.mbr_type.name()),
                        subtypes: opened
                            .report
                            .subtypes
                            .iter()
                            .map(|s| (s.number(), s.name()))
                            .collect(),
                        verified: Some(opened.evidence_verified),
                        ..super::ReportEvidence::default()
                    });
                    self.ma.opened_reports.push(opened);
                }
                if evidence.is_none() {
                    // A report another provider's MA shared, or a modelled one: what is
                    // known of its evidence travels with it (`ScmsRun::shared_report_evidence`,
                    // `ScmsRun::modelled_report_evidence`).
                    evidence = self.ma.pending_evidence.remove(&run);
                }
                self.ma.reports.push(*r);
                self.ma.report_arrived.push(at);
                self.ma.report_evidence.push(evidence);
                self.ma.report_runs.push(run);
                out.stage_at(StageId::ReportReceived, to, None, flow, run);
            }
            ScmsMsg::ReportShuffleTimer { generation } => {
                if generation == self.ra.report_shuffle_generation {
                    self.release_report_shuffle(out, n, to, at);
                }
            }
            _ => unreachable!("ScmsState::handle routes only this flow's messages here"),
        }
        Ok(())
    }
}
