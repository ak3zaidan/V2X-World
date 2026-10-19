//! Investigation and revocation: MA ↔ PCA ↔ LA ↔ RA, the linkage-seed release and the RA/ECA blocklists.
//!
//! Split out of `run.rs` without behaviour change (WP-B1); the state types and the shared
//! helpers stay in [`super`].

use super::*;

impl ScmsRun {
    /// Opens an investigation over the two reports the MA holds at `indices`, revoking
    /// from period `i_rev` forward if they resolve to one device: the "link" collaboration
    /// {MA, PCA, one LA per chain} and then, on "same device", the revocation set
    /// [BRECHT §VI-B/C/D]. The MA's decision is already taken; see
    /// [`ScmsRun::request_link`] for one that goes through [`MaMode`].
    ///
    /// Returns the resolution run and the CRL-issuance run, or `None` when a report is
    /// not held or the MA's case table is full ([`crate::scms::params::MaPolicy::max_open_cases`]).
    pub fn investigate(
        &mut self,
        a: usize,
        b: usize,
        i_rev: u32,
        jmax: u32,
    ) -> Option<(FlowRun, FlowRun)> {
        let (ra, rb) = {
            let reports = &self.state.ma.reports;
            (reports.get(a)?.clone(), reports.get(b)?.clone())
        };
        self.open_case(
            [(ra.subject_i, ra.subject_lv), (rb.subject_i, rb.subject_lv)],
            false,
            i_rev,
            jmax,
            Duration::ZERO,
            vec![a, b],
        )
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
    /// [`ScmsRun::request_revocation`] is the same through the MA's decision mode.
    ///
    /// Returns the resolution run and the CRL-issuance run, or `None` when no such report
    /// is held or the MA's case table is full ([`crate::scms::params::MaPolicy::max_open_cases`]); cases run
    /// concurrently, each message naming its case by run (gap row 27).
    pub fn revoke(&mut self, index: usize, i_rev: u32, jmax: u32) -> Option<(FlowRun, FlowRun)> {
        let r = self.state.ma.reports.get(index)?.clone();
        self.open_case(
            [(r.subject_i, r.subject_lv), (r.subject_i, r.subject_lv)],
            true,
            i_rev,
            jmax,
            Duration::ZERO,
            vec![index],
        )
    }

    /// A revocation the MA's pipeline asks for, carried out under the MA's decision mode
    /// (gap row 28): in [`MaMode::ManualReview`] the case is opened now and the operator's
    /// go-ahead (the `decision` stage) comes `review` later, as in the CV pilots' manual
    /// MA; in [`MaMode::Automatic`] it is [`ScmsRun::revoke`].
    pub fn request_revocation(
        &mut self,
        index: usize,
        i_rev: u32,
        jmax: u32,
    ) -> Option<(FlowRun, FlowRun)> {
        let r = self.state.ma.reports.get(index)?.clone();
        let delay = self.review_delay();
        self.open_case(
            [(r.subject_i, r.subject_lv), (r.subject_i, r.subject_lv)],
            true,
            i_rev,
            jmax,
            delay,
            vec![index],
        )
    }

    /// [`ScmsRun::investigate`] under the MA's decision mode.
    pub fn request_link(
        &mut self,
        a: usize,
        b: usize,
        i_rev: u32,
        jmax: u32,
    ) -> Option<(FlowRun, FlowRun)> {
        let (ra, rb) = {
            let reports = &self.state.ma.reports;
            (reports.get(a)?.clone(), reports.get(b)?.clone())
        };
        let delay = self.review_delay();
        self.open_case(
            [(ra.subject_i, ra.subject_lv), (rb.subject_i, rb.subject_lv)],
            false,
            i_rev,
            jmax,
            delay,
            vec![a, b],
        )
    }

    fn review_delay(&self) -> Duration {
        match self.state.params.ma.mode {
            MaMode::ManualReview { review } => review,
            MaMode::Automatic => Duration::ZERO,
        }
    }

    fn open_case(
        &mut self,
        subjects: [(u32, LinkageValue); 2],
        direct: bool,
        i_rev: u32,
        jmax: u32,
        delay: Duration,
        reports: Vec<usize>,
    ) -> Option<(FlowRun, FlowRun)> {
        let cap = self.state.params.ma.max_open_cases as usize;
        if self.state.ma.open_cases() >= cap {
            return None;
        }
        let run = self.new_run();
        let crl_run = self.new_run();
        let opened_at = self.kernel.now();
        self.state.ma.cases.insert(
            run,
            Case {
                run,
                crl_run,
                subjects,
                lookups: Vec::new(),
                same: [None, None],
                seeds: [None, None],
                chains: Vec::new(),
                chain_seeds: Vec::new(),
                i_rev,
                jmax,
                resolved: false,
                direct,
                done: false,
                reports,
                opened_at,
            },
        );
        self.state.ma.last_case = Some(run);
        let ma = self.state.nodes.ma;
        let (i, lv) = subjects[0];
        let at = delay.after(self.kernel.now());
        self.inject_at(
            at,
            ma,
            ScmsMsg::PcaLookupRequest { i, lv },
            FlowId::LinkageResolution,
            run,
        );
        Some((run, crl_run))
    }

    /// Whether any case is in progress.
    #[must_use]
    pub fn case_open(&self) -> bool {
        self.state.ma.open_cases() > 0
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
                let Some(case) = self.ma.cases.get_mut(&run) else {
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
                    let hash = found.handle;
                    self.ma.decisions.push(hash);
                    out.stage_at(StageId::Resolved, to, None, flow, run);
                    out.send(
                        n.ra,
                        ScmsMsg::BlocklistRequest { handle: hash },
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
                // It opens the two sealed identifiers (only it can) and compares owners.
                let nodes = self.nodes;
                let st = &self.la[la.idx()];
                let same = match (a.open(to, &nodes), b.open(to, &nodes)) {
                    (Some(a), Some(b)) => match (st.owner.get(&a), st.owner.get(&b)) {
                        (Some(x), Some(y)) => x == y,
                        _ => false,
                    },
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
                    let Some(case) = self.ma.cases.get_mut(&run) else {
                        return Ok(());
                    };
                    case.same[la.idx()] = Some(same);
                    match (case.same[0], case.same[1]) {
                        (Some(a), Some(b)) => {
                            case.resolved = a && b;
                            if !case.resolved {
                                case.done = true;
                            }
                            (case.resolved, case.lookups[0].handle)
                        }
                        _ => (false, [0u8; 32]),
                    }
                };
                if ready {
                    self.ma.decisions.push(hash);
                    out.stage_at(StageId::Resolved, to, None, flow, run);
                    out.send(
                        n.ra,
                        ScmsMsg::BlocklistRequest { handle: hash },
                        "blocklist-request",
                        self.sizes.blocklist_request(),
                        Transport::BackendNet,
                        flow,
                        run,
                    );
                }
            }
            ScmsMsg::BlocklistRequest { handle } => {
                // The RA alone maps the PCA's per-certificate handle to the request.
                let rec = self
                    .ra
                    .handles
                    .get(&handle)
                    .and_then(|h| self.ra.requests.get(h))
                    .copied();
                // B4: the RA blocklists the enrolment, not the request, so it answers
                // with the chains of every request it accepted from that enrolment,
                // the reported one first. Revoking only the reported chain would leave a
                // second request's batch ([CAMP122] SCMS-520) valid on every receiver
                // for as long as it was issued for; [BRECHT §VI-D] describes the return
                // for one request because a device has one. The MA still learns only
                // opaque identifiers ([BRECHT §VI-C]).
                let chains: Vec<(Lci, Lci)> = rec.map_or_else(Vec::new, |r| {
                    let mut v: Vec<(Lci, Lci)> = Vec::new();
                    let mine = self.ra.requests.values().filter(|o| o.device == r.device);
                    for o in core::iter::once(&r).chain(mine) {
                        if let (Some(a), Some(b)) = (o.lci1, o.lci2) {
                            if !v.contains(&(a, b)) {
                                v.push((a, b));
                            }
                        }
                    }
                    v
                });
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
                let pairs = u32::try_from(chains.len().max(1)).unwrap_or(u32::MAX);
                out.send(
                    n.ma,
                    ScmsMsg::BlocklistResponse {
                        known: rec.is_some() && !chains.is_empty(),
                        chains,
                    },
                    "blocklist-response",
                    self.sizes.blocklist_response_for(pairs),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::BlocklistResponse { known, chains } => {
                if !known {
                    if let Some(case) = self.ma.cases.get_mut(&run) {
                        case.done = true;
                    }
                    return Ok(());
                }
                let Some(case) = self.ma.cases.get_mut(&run) else {
                    return Ok(());
                };
                let i_rev = case.i_rev;
                let chains: Vec<[Lci; 2]> = chains.into_iter().map(|(a, b)| [a, b]).collect();
                case.chain_seeds = vec![[None, None]; chains.len()];
                case.chains = chains.clone();
                if let Some(role) = self.committees.role("linkage") {
                    // t-of-n linkage: the seeds are released by any quorum of the
                    // committee, two secrets per revocation (`threshold::linkage`).
                    let secrets = u32::try_from(2 * chains.len()).unwrap_or(u32::MAX);
                    self.committees.start(
                        role,
                        secrets,
                        Pending::SeedsReleased {
                            case: run,
                            chains,
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
                    ScmsMsg::SeedRetry { i: i_rev, tries: 0 },
                    flow,
                    run,
                );
                for pair in &chains {
                    for (la, lci) in [(LaIndex::One, pair[0]), (LaIndex::Two, pair[1])] {
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
                    ScmsMsg::SeedResponse {
                        la,
                        lci,
                        la_id,
                        seed,
                        i,
                    },
                    "seed-response",
                    self.sizes.seed_response(),
                    Transport::BackendNet,
                    flow,
                    run,
                );
            }
            ScmsMsg::SeedResponse {
                la,
                lci,
                la_id,
                seed,
                i,
            } => {
                let (pairs, jmax, crl_run) = {
                    let Some(case) = self.ma.cases.get_mut(&run) else {
                        return Ok(());
                    };
                    let k = la.idx();
                    let Some(c) = case.chains.iter().position(|p| p[k] == lci) else {
                        return Ok(());
                    };
                    let complete = |s: &[[Option<(LaId, LinkageSeed)>; 2]]| {
                        s.iter().all(|x| x[0].is_some() && x[1].is_some())
                    };
                    // A retry's duplicate answer, after every chain is in, adds nothing.
                    let was_ready = complete(&case.chain_seeds[..]);
                    case.chain_seeds[c][k] = Some((la_id, seed));
                    if c == 0 {
                        case.seeds[k] = Some((la_id, seed));
                    }
                    let pairs: Vec<_> = if complete(&case.chain_seeds[..]) && !was_ready {
                        case.chain_seeds
                            .iter()
                            .filter_map(|s| Some((s[0]?, s[1]?)))
                            .collect()
                    } else {
                        Vec::new()
                    };
                    (pairs, case.jmax, case.crl_run)
                };
                if !pairs.is_empty() {
                    let entries = u32::try_from(pairs.len()).unwrap_or(u32::MAX);
                    out.send(
                        n.crlg,
                        ScmsMsg::CrlAppend { i, pairs, jmax },
                        "crl-append",
                        self.sizes.crl_append_for(entries),
                        Transport::BackendNet,
                        FlowId::CrlIssuance,
                        crl_run,
                    );
                }
            }
            ScmsMsg::SeedRetry { i, tries } => {
                let Some(case) = self.ma.cases.get(&run) else {
                    return Ok(());
                };
                // A timer from an earlier case, a case already complete, or one that has
                // asked enough: nothing to do.
                let complete = case
                    .chain_seeds
                    .iter()
                    .all(|s| s[0].is_some() && s[1].is_some());
                if case.run != run || case.done || complete || tries >= SEED_RETRY_MAX {
                    return Ok(());
                }
                let missing: Vec<(LaIndex, Lci)> = case
                    .chains
                    .iter()
                    .zip(&case.chain_seeds)
                    .flat_map(|(p, s)| [(LaIndex::One, p[0], s[0]), (LaIndex::Two, p[1], s[1])])
                    .filter(|(_, _, s)| s.is_none())
                    .map(|(la, lci, _)| (la, lci))
                    .collect();
                out.start_timer(
                    self.params.seed_retry,
                    ScmsMsg::SeedRetry {
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
