//! Investigation and revocation: MA ↔ PCA ↔ LA ↔ RA, the linkage-seed release and the RA/ECA blocklists.
//!
//! Split out of `run.rs` without behaviour change (WP-B1); the state types and the shared
//! helpers stay in [`super`].

use super::*;

impl ScmsRun {
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
}

impl ScmsState {
    /// Handles every message of this flow; [`ScmsState::handle`] routes here.
    #[allow(unused_variables)]
    pub(super) fn handle_revoke(
        &mut self,
        d: Delivery<ScmsMsg>,
        out: &mut Outbox<ScmsMsg>,
    ) -> Result<()> {
        let n = self.nodes;
        let (flow, run, to, at, from) = (d.flow, d.run, d.to, d.at, d.from);
        match d.msg {
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
                if let Some(role) = self.committees.role("linkage") {
                    // t-of-n linkage: the seeds are released by any quorum of the
                    // committee, two secrets per revocation (`threshold::linkage`).
                    self.committees.start(
                        role,
                        2,
                        Pending::SeedsReleased {
                            lci1,
                            lci2,
                            i: i_rev,
                        },
                        at,
                        flow,
                        run,
                        out,
                        wrap_committee,
                    );
                    return Ok(());
                }
                // Two-of-two: both LAs must answer; the MA asks again after `seed_retry`
                // if one has not (a down LA blocks the revocation until it returns).
                out.start_timer(
                    self.params.seed_retry,
                    ScmsMsg::SeedRetry {
                        lci1,
                        lci2,
                        i: i_rev,
                        tries: 0,
                    },
                    flow,
                    run,
                );
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
            ScmsMsg::SeedRetry {
                lci1,
                lci2,
                i,
                tries,
            } => {
                let Some(case) = self.ma.case.as_ref() else {
                    return Ok(());
                };
                // A timer from an earlier case, a case already complete, or one that has
                // asked enough: nothing to do.
                if case.run != run
                    || case.done
                    || (case.seeds[0].is_some() && case.seeds[1].is_some())
                    || tries >= SEED_RETRY_MAX
                {
                    return Ok(());
                }
                let missing: Vec<(LaIndex, Lci)> = [(LaIndex::One, lci1), (LaIndex::Two, lci2)]
                    .into_iter()
                    .filter(|(la, _)| case.seeds[la.idx()].is_none())
                    .collect();
                out.start_timer(
                    self.params.seed_retry,
                    ScmsMsg::SeedRetry {
                        lci1,
                        lci2,
                        i,
                        tries: tries + 1,
                    },
                    flow,
                    run,
                );
                for (la, lci) in missing {
                    out.send(
                        self.nodes.la(la),
                        ScmsMsg::SeedRequest { la, lci, i },
                        "seed-request",
                        self.sizes.seed_request(),
                        Transport::BackendNet,
                        flow,
                        run,
                    );
                }
            }
            ScmsMsg::BlocklistNotice { device } => {
                Self::verify(out, 1);
                self.eca.blocklist.insert(device);
            }
            _ => unreachable!("ScmsState::handle routes only this flow's messages here"),
        }
        Ok(())
    }
}
