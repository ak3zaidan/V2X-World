# Ideas, engine-evidence-first lens (2026-10-18)

Lens: start from quantities only this engine can produce, then check whether the field needs them
and whether anyone has measured them. Seven directions, none on the THOUGHTS.md status table.
Each one is checked against the three criteria in `research/explore/THOUGHTS.md`. The engine limits
are respected: no per-unit counterfactuals unless the change is backend-only, open-loop or local;
warnings do not change driving (GLOSA and platooning are the exceptions); ~2.4 h per 600 s at ~450
vehicles (`docs/validation/test-run-cost.md`, fleet rows: 14.27 s/s at 453 vehicles, 1.74 s/s at 145).

Search log, in short: catalog regex scripts over the 11,315 records; OpenAlex (anonymous); arXiv
export API (3 s spacing); WebSearch. Searches that found nothing are recorded as such. Absence of
evidence after four or five searches is not proof of absence, and each idea still needs a blind
prior-art hunt.

## 1. Engine quantities that no dataset or analytical model provides

| # | Quantity | Why no dataset or model has it | Engine source |
|---|---|---|---|
| Q1 | Misbehaviour reports raised by **honest** vehicles from measured GNSS error, urban-canyon multipath, radio loss and stale tracks, per vehicle-hour on real US maps | Real pilot BSMs (Wyoming, THEA) have no reporter side, no ground-truth "honest" label after de-identification, and no MA. VeReMi-style sets inject "faulty" nodes as labels | GNSS Gauss-Markov fitted to Reid 2019 quantiles with urban-canyon scaling (`crates/v2xw-mobility/src/gnss/mod.rs` header; `gauss_markov.rs` L79-90; `common.rs` L64); detector suites legacy-12 and TS 103 759 classes 1-5 (`crates/v2xw-threat/src/detect.rs` L112-150; `ts103759.rs` header) |
| Q2 | Per-device accumulation of reports at the MA, with the true device behind every pseudonym | Only the SCMS (linkage authorities) can link pseudonyms; no public data | Per-message joint ground truth (`research/PROTOCOL.md` §"What the engine does") |
| Q3 | Cross-provider report flow: which MA receives which report, and what it can resolve | Production runs 2 providers; no public data on MA cooperation | `ScmsRun::new_peer`, MA-to-MA sharing; test with 1,494 reports shared (`docs/design/scms-real/00-gap-analysis-and-plan.md` §4.3, D1 row) |
| Q4 | RSU signing availability under the weekly B3b certificate cycle and backhaul faults, and its effect on SPaT/MAP consumers | Pilots report expiry problems only as lessons, not rates | RSUC: `rsus_starved`, `rsu_messages_withheld` (`docs/design/BUILD-STATUS.md`, 2026-10-18 follow-ups section); RSU `Degraded` backhaul state (`crates/v2xw-node/src/rsu.rs` L294-304); GLOSA closes the loop (THOUGHTS.md lessons) |
| Q5 | Backend and cellular load on the real calendar (i-period anchored 2015-01-06 04:00 ET, daily CRL on a 1-business-day SLA, report weight decay at 05:00 ET) with diurnal demand | Load models in the literature use uniform arrivals; real per-cell contention needs spatial demand | `time.calendar: real`, `fast_forward` (00 plan §4.3 D2); NYC DOT hourly profile (`crates/v2xw-mobility/src/demand/daily.rs` header); ns-3-matched LTE packet model (`crates/v2xw-radio/src/cellular_packet.rs` L124-153) |
| Q6 | Encounter-weighted verifiability: of the BSM pairs that matter (inside a conflict zone), how many a receiver can validate given its trust store, CTL version and chain file | Needs who-meets-whom plus per-device PKI state | Full receiver validation on every node, P2PCD on the air, CTL/CCF per device (00 plan §4.3 C2, AIR rows) |
| Q7 | Receiver verification work per second under real certificate churn, cache eviction and CRL installs at city density | Analytical estimates assume a neighbour count; the engine gets it from ray-traced propagation | VCOST `rxval::EeCache` (4,096 entries; repeat frame 400 µs), `ee_hits`, `ee_evictions`, `invalidated` (BUILD-STATUS 2026-10-18 follow-ups) |
| Q8 | Bit-level conformance of real published SCMS artefacts, and the receiver behaviour each deviation causes | No published audit of the US V2X PKI found | `oracle.py`, `check_run.py` 19,183 checks, `real_artefacts` 7/7, two chain-length deviations in real artefacts (BUILD-STATUS 2026-10-18 Phase F section) |

## 2. The seven directions

### D1. Honest noise at city scale: what the deployed misbehaviour-reporting rules do with benign GNSS error (key `benign-mbr-load`)

- **Problem.** The only published US report specification, the CV-pilot MBR v1.0, triggers a
  `RandomPosition` report at a **gpsDrift of 1 m**, plus MaxSpeed > 90 m/s and MaxAcceleration >
  10 m/s² checks. Reports are stored on the device with weight 1.0, multiplied by 0.9 daily at
  05:00 ET; over quota the lowest weight is deleted; upload is a per-report HTTPS POST "whenever in
  range of an RSU offering SCMS connectivity" (`docs/design/scms-real/03-protocols-and-operations.md`
  rows L295-300, citing SCMS Manager MBR spec v1.0 §2.2-2.3, §4). A production automotive receiver
  has 3.07 m 68th-percentile horizontal error in open sky (Reid et al. 2019, arXiv:1906.01061, as
  fitted in `gauss_markov.rs` L79-81), and much more in urban canyons. Nobody has measured what these
  rules do to an honest city fleet.
- **Decision it informs.** 5GAA MBD-Op v1.0 (29 May 2024) §5.2.1 says minimum performance
  requirements for local detection "may need to be considered", and §5 calls a full cost-benefit
  analysis "unfeasible" (`research/synthesis/big-needs.md` need 2 and 7). SAE J3287_202408 fixes the
  format but not detector quality (doi:10.4271/J3287_202408). Provider requirements make each
  provider run an MA and share reports (`01-deployment-and-governance.md` §1.4). The result sets
  three numbers: the admissible benign false-report rate per vehicle-hour, the on-device quota and
  decay that keep real-attack evidence from being crowded out, and MA intake staffing under
  `manual-review` (the engine default, 00 plan §2.1).
- **Engine evidence.** Benign report volume per vehicle-hour by map (Manhattan canyons vs suburban
  grid), radio (DSRC vs LTE-V2X) and density. The per-device accumulation at the MA uses ground
  truth that only simulation has (Q1, Q2). Crowd-out: with a small fraction of real attackers
  injected, the share of attack evidence lost to quota deletion or never uploaded because the
  reporter never passed an SCMS-connected RSU. RSU coverage is set to the USDOT Plan targets (25% of
  top-75-metro signalised intersections by 2028, 85% by 2036; big-needs.md "Plan"). The upload path
  and the quota logic are local or backend-only once traces are recorded, so they replay exactly
  (an open-loop replay of recorded reports).
- **Expected headline.** "Under the CV-pilot rules, an honest Manhattan fleet raises N reports per
  vehicle-hour, X% of them RandomPosition from canyon multipath; at Plan-2028 RSU coverage, Y% of
  real-attack reports are deleted before upload." If N is near zero, the clean negative result
  is still worth publishing as a requirement (the thresholds are safe).
- **Adopter.** SCMS Manager (the MBR spec owner), 5GAA MBD4V2X (MPR), SAE J3287 revision, SCMS
  providers sizing MA intake.
- **Closest prior work.** VeReMi Extension (kamel2020veremi, 237 cites) injects "faulty" nodes as a
  labelled class; it does not derive benign error from measured GNSS on real geometry. F2MD
  (kamel2020simulation). DARE (haidar2020dare, doi:10.1109/vtc2020-spring48590.2020.9128687) is a
  report dataset for global MA evaluation, built from attacker scenarios. bimeyer2012central. Patent
  US 12132635, "Managing a volume of misbehavior reports"
  (https://image-ppubs.uspto.gov/dirsearch-public/print/downloadPdf/12132635), shows industry
  concern but gives no measurement. A Windsor MSc thesis builds labelled attacks on Wyoming pilot BSMs
  (https://www.uwindsor.ca/science/computerscience/478464/), which covers attacks, not benign report
  load. Searches for benign GNSS false positives combined with report volume found nothing (OpenAlex
  "misbehavior detection GNSS error false positive V2X": 45 hits, none on point; arXiv
  `abs:misbehavior AND abs:GPS AND abs:vehicular`: 0).
- **Engine role and size.** Central. Extensions: the CV-pilot MBR check set (5 subtypes, days);
  an on-device report store with weight, decay, quota and RSU-gated upload (about 1-2 weeks). Runs:
  about 6 maps × 2 radios × 3 densities at ~150 vehicles × 600 s (~5-20 min each); Manhattan at
  450 vehicles for 3 seeds (~7 h). Real-data calibration: compare the engine's benign trigger rate
  on recorded honest BSMs against the same checks run over ITS DataHub pilot BSMs. That is a
  strength, but those BSMs cannot replace the engine because they lack reporter-side and MA-side
  ground truth.
- **Criteria.** (1) yes, as far as searched. (2) a named specification with concrete parameters
  would change. (3) strong: the benign rate depends on geometry, GNSS and radio jointly, and the
  crowd-out effect needs device-level ground truth.
- **Risks.** The engine's GNSS canyon scaling (`common.rs` L64: 31.02/3.07 for deep canyon) is
  itself a model and needs a sensitivity sweep. J3287 content is paywalled, so its triggers may
  differ from the CV-pilot spec.

### D2. Two providers, one fleet: the cost of "should cooperate" between misbehaviour authorities (key `cross-provider-ma`)

- **Problem.** Production US V2X runs two SCMS providers on one CTL (ISS and SAESOL roots,
  production CTL 13 Aug 2025; `01-deployment-and-governance.md` §3.1). A reporter encrypts to "the
  MA certificate whose MaSsp.relevantPsids contains the application PSID" (03 doc, L298, citing
  1609.2.1 ASN MaSsp), which is in practice its own provider's MA. That MA cannot resolve another
  provider's linkage values without the other provider's LAs and PCA. Provider requirements say
  providers "must agree to share misbehaviour reports" (01 §1.4). 5GAA's draft CP says an MA
  "should cooperate with all SCMS Providers" and "should support end entity revocation requests from
  other MAs" (big-needs.md need 3, CP-TR 14 Jul 2025). No format or deadline is published (00 plan
  §4.3 D1: "format modelled: no published spec").
- **Decision it informs.** Whether the CP should make cross-MA cooperation a "shall" with a
  deadline; whether evidence thresholds must be shared; whether an attacker gains by enrolling with
  the provider whose MA is slowest or most lenient ("provider shopping"). It also bears on the Plan's
  requirement that 2 or more providers demonstrate interoperable security (big-needs.md need 3).
- **Engine evidence.** For each attacker, which provider's MA holds how much of the evidence and
  when. This depends on who is near the attacker (fleet split × traffic geometry), on which reports
  survive the radio, and on per-provider MA modes and CRL cadences (Q3). Metrics: time to
  revocation and evidence completeness, with cooperation off, delayed (business days) or
  immediate, under 50/50, 80/20 and 95/5 market shares; and the honest-revocation rate when
  thresholds differ. Policy changes are backend-only, so they replay exactly over the same air.
- **Expected headline.** "With a 50/50 split and no binding cooperation SLA, a single attacker's
  evidence is split so that neither MA crosses threshold for D days; a 48 h sharing SLA restores
  single-provider performance" (or shows that it does not).
- **Adopter.** SCMS Manager EAC (CP and Provider Requirements), 5GAA CP-TR authors, USDOT Plan
  interoperability demonstrations.
- **Closest prior work.** brecht2018security (the single-SCMS design, 325 cites);
  sarker2021blockchain (root-of-trust management); gu2020efficient (cross-domain authentication);
  khodaei2019scaling. None measures split evidence across MAs. WebSearch for cross-provider MA
  revocation found only architecture entries (ARC-IT "Other CCMS",
  https://local.iteris.com/arc-it/html/triples/s192d187f1197.html). OpenAlex "misbehaviour reporting
  multiple PKI ... MA cooperation": 72 hits, none on point.
- **Engine role and size.** Central. Mostly built (D1). Missing: per-provider `ma_mode` and
  `crl_publication`, since the run's single `ma_review_s` applies to all (00 plan §4.3 D1 "still
  open"); a few days of work. A fully analytical version ("cross-provider share = 1 − Σ s_i²") is
  the baseline the engine must beat, and the engine beats it through evidence quality and timing,
  not the share.
- **Risks.** Real MA procedures are unpublished, so MA behaviour is modelled. Needs a proof-based
  MA (THOUGHTS.md lesson), which overlaps the "framing" work and could share its MA model.

### D3. When the roadside goes silent: RSU certificate lifecycle as an availability risk for signal applications (key `rsu-cert-availability`)

- **Problem.** RSUs sign SPaT/MAP under weekly application certificates fetched over backhaul. In
  the engine, an RSU stops signing when its certificate lapses (RSUC). Pilots report "persistent
  issues with security certificate expiration" that "can hinder the normal processing of CV
  messages", and USDOT recommends "pre-downloading three years' worth of security certificates or
  setting up weekly monitoring" (USDOT ITS KR lesson 2024-L01214,
  https://www.itskrs.its.dot.gov/2024-l01214, read via Wayback). NYC ran with "only 1 week
  certificate life and only 2 weeks of certificates onboard" (WebSearch summary of a USDOT 5.9 GHz
  presentation, https://www.transportation.gov/sites/dot.gov/files/docs/subdoc/531/4mtalas-5-9-ghz-presentation-v3.pdf;
  to verify). Tennessee reports RSUs failing to get initial SCMS responses and an SCMS deactivation
  (arXiv:2304.02885).
- **Decision it informs.** State DOTs deploying RSUs at the Plan targets choose the application
  certificate preload depth and the backhaul SLA, and choose between monitoring and preloading. The
  privacy argument for short RSU certificates is weak, since RSUs are fixed. The result gives the
  traffic-efficiency price of each choice.
- **Engine evidence.** Backhaul outage processes (`Degraded`/down states) × preload depth × renewal
  timing on the real calendar give the RSU-hours of withheld SPaT/MAP. Because GLOSA closes the
  loop, the engine then measures stops, delay and fuel or emissions lost on a real corridor
  (`v2xw-energy` crate). That chain from security policy to traffic efficiency cannot be computed
  analytically: GLOSA benefit depends on the platoon structure and the signal plan.
- **Expected headline.** "A one-week preload with a 99% backhaul SLA loses X% of GLOSA benefit
  city-wide; two weeks recovers it; the privacy gain of short RSU certificates buys nothing."
- **Adopter.** State DOTs and AASHTO/ITE (the RSU specification), USDOT ITS JPO deployment
  guidance, SCMS providers selling RSU certificate services.
- **Closest prior work.** wang2015allocation (RSU allocation for certificate update, 1 cite);
  owen2010certificate; GLOSA benefit studies (e.g. Katsaros 2011, cited in `apps.rs` L33). No work
  found that ties RSU certificate lifecycle to signal-application benefit (OpenAlex "V2X roadside
  unit certificate expiration traffic signal SPaT availability": 7 hits, none on point).
- **Engine role and size.** Central for the impact half. Extensions: a backhaul outage process
  calibrated to published RSU uptime (if any exists; otherwise a sweep), about a week. RSU UI fields
  and the RSU CRL install transit time are open (BUILD-STATUS follow-ups "Still open"). Note that
  `applications::glosa` was failing at `6f335bb` (11/32), and it must be fixed first.
- **Risks.** It could look like an operations paper. The novelty is the quantified security →
  availability → efficiency chain. Backhaul outage statistics may not be public.

### D4. The weekly heartbeat: SCMS and cellular load on the real calendar at Plan scale (key `scms-calendar-load`)

- **Problem.** Every device's certificate validity is aligned to the same i-period grid (I-period 0
  at 2015-01-06 04:00 ET; 00 plan G8). CRLs publish on a one-business-day SLA (00 plan §2.2). Report
  weights decay at 05:00 ET. Devices preload either 3 years (THEA) or 1 week (NYC) (00 plan §2.2
  `fleet.preload`). Ignition is diurnal. All of this synchronises demand on the RA and on the cells
  near dense corridors. A long preload removes weekly top-ups but keeps linkage entries on the CRL
  for the preload's life (patents US 12284293 and US 12143511, summarised by WebSearch; Brecht et
  al. arXiv:1802.05323 §on batch size), which raises CRL bytes and receiver CRL-check cost.
- **Decision it informs.** USDOT's own lesson poses this choice: "pre-downloading three years" or
  "weekly monitoring" (ITS KR 2024-L01214). SCMS providers also size the RA for the Plan's 2028-2036
  targets (big-needs.md need 8), and 1609.2.1 implementers choose download jitter windows.
- **Engine evidence.** Measured per-cell TLS completion times and RA queue lengths at the weekly
  boundary and the morning peak, on a real map with spatially clustered demand (Q5). Also the
  CRL-size path: linkage entries × preload horizon → bytes per vehicle per day over LTE Uu → CRL
  check cost per received BSM with the validated-certificate cache (Q7). `fast_forward` makes
  multi-day backend horizons feasible (00 plan §4.3 D2); radio cost is paid only in the peak windows.
- **Expected headline.** "Preload depth moves the bottleneck: 1-week preload puts a Tuesday-04:00
  burst of K requests/s per RA; 3-year preload makes CRL bytes dominate after M revocations per
  million devices. A jitter window of W hours flattens the burst."
- **Adopter.** USDOT JPO (deployment guidance), SCMS providers (RA sizing), OEMs (preload policy),
  IEEE 1609 WG (jitter guidance).
- **Closest prior work.** arXiv:2501.03237 (SCMS 1609.2.1 vs ETSI TS 102 941 performance);
  chen2023performance (component timings); khodaei2019scaling and khodaei2023secmace (VPKI cloud
  scaling); simplicio2021revocation (ACPC vs CRL, analytical). None uses the real calendar,
  diurnal demand or per-cell contention.
- **Engine role and size.** Supporting-to-central. The engine is needed for the cell-level
  contention and receiver costs; the RA-level burst is partly analytical, and a hostile reviewer
  will say so. Extensions: download jitter and retry (SCMS-522/523), infoStatus and Range resume
  (BUILD-STATUS 2026-10-18 validation "Still open"); vehicle CRL downloads charged by real CRL bytes
  rather than the 40 B/entry model (same list). About 2 weeks.
- **Risks.** Criterion 3 is the weakest of the set unless cell contention turns out to be the
  binding constraint. That is a falsifiable early check: run a Manhattan peak window first.

### D5. Changing the root of trust in a moving fleet: CTL additions, removals and the encounter-weighted trust gap (key `ctl-change-propagation`)

- **Problem.** The Plan wants more providers (big-needs.md need 3). The 2025 test CTL already
  carries 5-6 roots for OmniAir plugfests (`01-deployment-and-governance.md` L23, L118). Elector
  policy executes an addition within 5 business days and a removal within 48 h (01 §3.1, Elector
  Policy §2.2.4-2.2.5). Devices learn a new CTL only through `CompositeCrl.homeCtl` or
  `infoStatus` polling from their RA (03 doc L140-145). Until then, a newly added provider's
  vehicles are unverifiable to them, and a removed (compromised) root stays trusted.
- **Decision it informs.** The SCMS Manager's CTL Update Process document is still "in development"
  (01 §1.2, L83). It needs a minimum lead time between a CTL addition and the new provider's first
  issuance, and a poll cadence that bounds the exposure after a removal. OEMs choose the polling
  interval.
- **Engine evidence.** Fleet-share of updated devices over time comes from connectivity and
  driving. The safety-relevant quantity is the encounter-weighted share of conflict-zone BSM pairs
  that fail validation, and how many P2PCD learning requests the gap triggers on the air (Q6).
  Both need who-meets-whom at intersections and per-device trust-store state.
- **Expected headline.** "A new provider at 10% market share leaves Z% of its vehicles' conflict-zone
  BSMs unverifiable to the incumbent fleet for T days unless issuance waits L days after the CTL
  add; a root removal leaves the compromised root trusted by Y% of encounters for 48 h + P."
- **Adopter.** SCMS Manager (CTL Update Process, Elector Requirements), OmniAir certification, new
  SCMS providers entering the US market.
- **Closest prior work.** sarker2021blockchain (blockchain root-of-trust management in the SCMS,
  18 cites), giannetsos2019securing. OpenAlex "trust list update propagation vehicles C-ITS ECTL"
  returned 5 hits, none on point. No measurement of CTL propagation was found.
- **Engine role and size.** Moderate. Fleet-update curves are partly analytical, and the encounter
  weighting and P2PCD load are the engine part. Multi-day horizons need fast-forward, and vehicles
  persisting across days is an engine extension, since trips enter and leave today (about 1-2
  weeks; size unverified).
- **Risks.** Smaller than D1-D3. It may fold into D2 as one "multi-provider operations" paper.

### D6. The first audit of the deployed US V2X PKI, and what its deviations do on the road (key `us-pki-audit`)

- **Problem.** The engine's oracle has already found real deviations in published SCMS artefacts:
  two chain-length deviations, the RA `certRequestPermissions {all,0,0}` in the test 2025 chain and
  the production RA's `minChainLength 0` (BUILD-STATUS 2026-10-18 Phase F, "Oracle"). It also found
  open conformance questions, such as application and identification certificates carrying no SSPs
  and an UNCONFIRMED US pseudonym region (same section, F2 §9). Web PKI has a decade of certificate
  ecosystem measurement; no audit of the V2X PKI was found (WebSearch "analysis of deployed V2X PKI
  certificates trust list conformance measurement": only lab studies such as arXiv:2311.03360 and
  arXiv:2501.03237).
- **Decision it informs.** SCMS Manager and the providers fix or document the deviations. OmniAir
  test plans add the checks. OEMs decide between strict and lenient receiver validation. The
  question with safety weight is whether a strict receiver from vendor A rejects a lenient signer
  from vendor B.
- **Engine evidence.** The audit is the oracle, not the simulation. The engine then measures the
  on-road effect: mixed fleets of strict and lenient receiver profiles, signed by chains that carry
  each real deviation, give the share of conflict-zone BSMs rejected per receiver/signer pair (Q6,
  Q8). This is local to each receiver, so per-unit replay is exact.
- **Expected headline.** "k of n checked fields in the production and test trust chains deviate
  from 1609.2.1/the profile; under strict validation, deviation j alone rejects X% of SPaT from
  RSUs certified under chain c."
- **Adopter.** SCMS Manager, SCMS providers, OmniAir, OEM V2X stack vendors.
- **Closest prior work.** arXiv:2501.03237 and arXiv:2311.03360 (lab implementations);
  chen2023performance. Nothing found on artefacts in the wild.
- **Engine role and size.** Mixed. The audit is the novel core and is not simulation. The engine
  provides the impact half. Most tooling exists (`tools/conformance`, `real_artefacts`).
  Extension: per-vendor receiver strictness profiles, a few days.
- **Risks.** Responsible disclosure to SCMS Manager comes first. The engine half may look bolted
  on. If the deviations are harmless (receivers ignore those fields), the impact half collapses,
  though the audit stands alone.

### D7. How many verifications must an OBU do? A measured verification budget at Plan densities (key `obu-verify-budget`)

- **Problem.** OBU and HSM sizing rests on rough numbers: patents say "over 1,000 messages per
  second" on a busy road (US 11997106, WebSearch summary). A 2026 paper sets its own 100 SPDUs/s
  receiver target and reports an Android device at about 71/s (arXiv:2606.14008). IACR ePrint
  2022/133 assumes thousands per second. None measures the real workload, which is the arrivals
  that survive the channel × the cache-miss rate under pseudonym change × the extra chain work from
  multi-provider chains, P2PCD and CRL installs that evict cached certificates.
- **Decision it informs.** HSM selection and verify-on-demand policy at OEMs and stack vendors
  (Autocrypt markets BSM verification acceleration, https://autocrypt.io/autocrypt-releases-a-major-update-to-its-bsm-verification-v2x-technology/),
  and a minimum verification rate in OmniAir or USDOT device specifications.
- **Engine evidence.** The percentile distribution of first-signature verifications per second per
  OBU, from ray-traced reachability on real maps at Plan densities, with VCOST cache statistics
  (`ee_hits`, `ee_evictions`, `invalidated`) and CRL-install storms (Q7). Per-receiver work is
  local, so a policy change (cache size, verify-on-demand) replays exactly over the same air.
- **Expected headline.** "The 99th-percentile OBU in Manhattan peak traffic needs V first-time
  verifications/s, not 1,000; a CRL install raises it by S% for a second; a 4,096-entry cache is
  oversized by a factor of F."
- **Adopter.** OEMs and V2X stack/HSM vendors, OmniAir, IEEE 1609.2 (verify-on-demand guidance).
- **Closest prior work.** Verify-on-demand (Krishnan and Weimerskirch, 2011; not in catalog,
  to verify); arXiv:2606.14008; ePrint 2022/133; catalog hits on verification load are scheme
  papers (zhang2010scalable and others), not measurements.
- **Engine role and size.** Central but narrow. The VCOST cache capacity is not calibrated and has
  no 2 s expiry (BUILD-STATUS follow-ups "Still open"), so calibration comes first, about a week.
  Large runs: ~1,000 vehicles at 86 s per simulated second (test-run-cost.md), so peak windows of
  60 s only.
- **Risks.** Owner may find it too small (like "pricing detector errors"). It is best used as a
  chapter of D1 or D4.

## 3. Ranking and fit

| Rank | Key | Novelty | Adopter changes a decision | Engine indispensable | Note |
|---|---|---|---|---|---|
| 1 | benign-mbr-load | high (nothing found) | SCMS Manager MBR parameters, 5GAA MPR | strong | Pairs naturally with the owner-liked "framing" work (same MA model) |
| 2 | cross-provider-ma | high | CP "should" → "shall", SLA | strong | Mostly built; needs per-provider MA settings |
| 3 | rsu-cert-availability | high | DOT preload/backhaul choice | strong (GLOSA loop) | GLOSA test must be fixed first |
| 4 | us-pki-audit | high (audit) | providers fix artefacts | moderate | Core is not simulation |
| 5 | scms-calendar-load | medium | preload horizon, RA sizing | moderate | Check first whether cell contention binds |
| 6 | ctl-change-propagation | high | CTL Update Process | moderate | Could merge into rank 2 |
| 7 | obu-verify-budget | medium | HSM sizing | strong but narrow | Chapter, not a paper |

D1 + D2 + D3 could form a thesis spine: "Operating the deployed US V2X trust system: what its
real rules do at city scale". Each is a decision-grade measurement that only a closed-loop,
standards-exact engine can make.
