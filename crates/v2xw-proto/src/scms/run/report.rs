//! Misbehaviour reporting: device → (LOP) → RA shuffle → MA.
//!
//! Split out of `run.rs` without behaviour change (WP-B1); the state types and the shared
//! helpers stay in [`super`].

use super::*;

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
}

impl ScmsState {
    /// Closes the RA's report shuffle: every waiting report goes on to the MA. Run by the
    /// window's timer or, first, by the count threshold (CAMP-EE SCMS-765).
    pub(super) fn release_report_shuffle(
        &mut self,
        out: &mut Outbox<ScmsMsg>,
        n: ScmsNodes,
        to: NodeId,
    ) {
        self.ra.report_shuffle_armed = false;
        self.ra.report_shuffle_generation += 1;
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
                if self.ra.reports.len() >= self.params.report_shuffle_threshold as usize {
                    // The count closes the batch before the day does (CAMP-EE SCMS-765).
                    self.release_report_shuffle(out, n, to);
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
                self.ma.reports.push(*r);
                out.stage_at(StageId::ReportReceived, to, None, flow, run);
            }
            ScmsMsg::ReportShuffleTimer { generation } => {
                if generation == self.ra.report_shuffle_generation {
                    self.release_report_shuffle(out, n, to);
                }
            }
            _ => unreachable!("ScmsState::handle routes only this flow's messages here"),
        }
        Ok(())
    }
}
