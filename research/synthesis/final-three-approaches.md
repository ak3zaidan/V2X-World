# The final three research approaches

Date: 2026-10-17. Role: judge of the adversarial review.

**Inputs:**
- `top-three-directions.md` (the previous recommendation);
- `critique-framed.md`, `critique-f1.md` and `critique-disagree.md` (one hostile critique per direction);
- `critique-methods.md` (whether the shared method is valid);
- `critique-blind-proposals.md` (three directions proposed without reading the synthesis);
- a fresh round of searches and engine reads made for this file (§8).

Ids resolve to `summaries/<id>.md`. **NEW** marks a source that is not in `catalog.jsonl`. Engine paths are relative to the repository root, read at commit `bfb9e237`.

## 0. The decision

**Working thesis title:** *What V2X security costs the honest vehicle: framed, discarded and followed.*

| # | Approach | Built from | Novelty after the critics' and this session's searches | First venue | Biggest risk |
|---:|---|---|---|---|---|
| 1 | **Which evidence can frame you?** How framable each standard misbehaviour detector is under a proof-based SCMS misbehaviour authority (MA), and the corroboration rule that closes each framable class | Old Direction 1, rebuilt as `critique-framed.md` §4 asks, plus the correlated-false-positive mechanism from `critique-f1.md` §2.1 | Narrow but real. No per-detector framability study under an MA that re-verifies evidence was found. The general idea of slander is old | IEEE/USENIX VehicleSec | Park 2024 (K-City SCMS slander defences) or SAE J3287 may already do part of it. Neither has been read in full |
| 2 | **Pricing V2X detector errors.** The harm of each false positive and false negative, a test of whether a cost-weighted open-loop metric recovers the closed-loop ranking, and harm-labelled episodes for existing datasets | Old Direction 2, reframed by `critique-f1.md` §5, merged with blind proposal B | Narrow. No paper ranks many V2X detector operating points by closed-loop harm against a calibrated open-loop proxy. The general cost-based method dates to 2001 | VehicleSec or IEEE VNC | The result depends on a driver-response model that does not exist yet |
| 3 | **Who can follow an honest vehicle?** The extra linkability each channel of the US stack adds over a kinematic tracker, per observer: the LTE-V2X reservation, the 1609.2 signer, misbehaviour reports, and linkage-seed certificate revocation lists (CRLs) | Blind proposal A, absorbing the privacy parts of old Direction 1 (H2 and RQ1.3) | Partly novel. The leak mechanisms are known in principle; the measurement is not. Measuring reservation continuity on broadcast BSMs, against a kinematic baseline, with SCMS insiders, was not found | PoPETs | SAE J3161/1 or chipset practice may already force reselection at an identifier change. J3161/1 has not been checked; it is paywalled |

**What happened to the old directions.** Old Direction 3, "why V2X security studies disagree", is **demoted**. It becomes the validity chapter all three approaches share (§5), plus an optional audit paper that goes ahead only if a desk audit passes. Blind proposal C (SCMS resilience and crypto-agility) is the **runner-up** (§6). Why each change was made is in §7.

**The common thread.** Each approach measures a collateral cost that a security mechanism imposes on vehicles that did nothing wrong:
- revocation that is triggered falsely (Approach 1);
- messages that are wrongly discarded or warnings that are missed (Approach 2);
- tracking enabled by the security machinery itself (Approach 3).

All three use the engine's distinctive asset: TS 103 759 detectors, a corroborating MA, SCMS linkage-authority flows, an SPS MAC matched to OpenCV2X, and a passive privacy observer, all in one process with ground truth (§1.4).

---

## 1. How the candidates were judged

### 1.1 Scores

The scale is 1 (weak) to 5 (strong). "Methods" means validity under `critique-methods.md`.

| Candidate | Novelty | Usefulness (who changes a decision) | Traction and venue fit | Engine advantage | Methods | Survived independent proposal and hostile critique? |
|---|---:|---:|---:|---:|---:|---|
| Old D1 as written (framed and revoked) | 2 | 2 | 2 | 4 | 3 | Hostile critique: "fatal as written" |
| **D1 reframed (framability)** | 3.5 | 4 | 3.5 | 5 | 4 | Hostile critique: keep, if reframed |
| Old D2 as written (F1 vs net safety) | 2 | 2 | 2 | 3 | 1.5 | Hostile critique: headline fatal; no outcome variable exists |
| **D2 reframed + blind B (pricing errors)** | 3 | 3.5 | 3.5 | 4 | 3 | Both tests: B was proposed blind, and D2 survived hostile critique as "keep, demote" |
| D3 reframed (contradiction audit) | 3.5 | 2.5 | 2.5 | 3.5 | 2.5 | Hostile critique: keep, but replace if under three real pairs. Its lead pair is not a real contradiction |
| **Blind A (privacy audit)** | 3 | 3.5 | 3.5 | 4.5 | 4.5 | Proposed blind. Hostile-checked here (§4.9) |
| Blind C (resilience and crypto-agility) | 3 | 4 | 3.5 | 4.5 | 2.5 | Proposed blind. Hostile-checked here (§6) |

### 1.2 Why these three win

**Approach 1** has the highest engine advantage and is model-light:
- The engine already has the TS 103 759 class 1–5 detectors (`crates/v2xw-threat/src/ts103759.rs`).
- It has an MA that correlates evidence before it counts (`crates/v2xw-threat/src/ma_corroborate.rs`), a GNSS belief model (`obs.rs:186`) and relay/wormhole attacks (`attack.rs:558`).
- Its outcomes (framing cost, honest revocations, linkage-authority query count) come from the backend timeline. That timeline can be paired exactly once the report streams are re-keyed (`critique-methods.md` §2).

**Approach 2** is the only candidate that both arose independently (blind B) and survived a hostile critique. Its reframed output, a table of harm prices, is publishable whichever way the proxy test comes out (`critique-f1.md` §3).

**Approach 3** has the cleanest method:
- The observer never acts on the world, so it can be run exactly on a recording.
- A tracker already exists in the engine (`crates/v2xw-threat/src/privacy.rs`).
- It also gives the privacy legs of old Direction 1 a proper baseline, as `critique-framed.md` §2.4 demands.

### 1.3 Why D3 and C lose

**D3** loses its headline:
- The lead contradiction is not real. petho2024investigating treats PDR as an input, while the post-quantum papers study payload size, which is what lowers PDR; the two results compose.
- Its hypothesis H10 ("some well-known pair flips on an unreported factor") has no verified instance.
- Field anchors exist only for radio factors.
- Sobol analysis is infeasible on this machine (`critique-disagree.md` §2 A1–A6; `critique-methods.md` §4.3).

What survives of D3 is needed by every approach, so it becomes the shared validity chapter.

**C** has the strongest practitioner demand but the weakest identification:
- Recovery timing is set mostly by policy and by how fast OEMs push updates (§6).
- Its post-quantum half is the most crowded area in the corpus.

### 1.4 What the engine joins that no other tool found does

Each item was confirmed by reading the code this session:

| Capability | Where it is |
|---|---|
| TS 103 759 observation classes 1–5, run alongside the legacy F2MD-style checks | `crates/v2xw-threat/src/ts103759.rs` |
| A legacy MA that counts reporters and a corroborated MA that correlates first | `ma.rs`, `ma_corroborate.rs` |
| Elector-signed CTL (3 of 5 electors, P-384) and composite CRL | `crates/v2xw-sec/src/ctl.rs`, `crl.rs` |
| Linkage values; threshold linkage authority (LA) and CGGMP21 | `crates/v2xw-sec/src/linkage.rs`; `crates/v2xw-proto/src/threshold/{linkage,cggmp21}.rs` |
| A passive privacy observer (Wiedersheim-style tracker, with the linkability, anonymity-set and tracking-duration metrics) | `crates/v2xw-threat/src/privacy.rs` |
| SPS and sidelink MAC | `crates/v2xw-radio/src/{sps,sidelink,cv2x}.rs` |

**Gaps that matter.** The observer's input type `ObservedMessage` (`obs.rs:116`) has no MAC fields: no resource, no reservation interval, no Layer-2 ID. A grep of `sps.rs`, `sidelink.rs` and `cv2x.rs` finds no Layer-2 source ID at all. Approach 3 needs both.

---

## 2. Approach 1: Which evidence can frame you?

### Pitch
Every normative description of the MA is proof-based: the report must carry the original signed message, so the MA can "verify the observations independently" (ETSI TS 103 759 v2.1.1 §4; C2C-CC WP 2092 p24; brecht2018security p16; all quoted in `critique-framed.md` §2.1). Counting reporters therefore does not frame anyone.

What can frame an honest vehicle is an attacker, or a benign correlated fault, that makes the victim's own validly signed messages fail a standard check. This approach measures, for each standard detector:
- the cheapest capability that produces MA-verifiable evidence against an honest vehicle;
- the corroboration rule, and the linkage-authority query budget (its privacy price), that closes that class;
- what each rule costs in time to neutralise a real attacker.

### Research questions
- **RQ1.1 Framability matrix.** The rows are detectors: TS 103 759 classes 1–5 as implemented, plus the CV-pilot report checks MaxSpeed > 90 m/s, MaxAcceleration > 10 m/s² and ObsImplausible (`docs/design/scms-real/03-protocols-and-operations.md:295`). The columns are capabilities, cheapest first:
  - (a) no attacker: a benign, spatially correlated GNSS fault;
  - (b) an outsider spoofing the victim's GNSS (jimenez2026gnss, arXiv 2606.20215, shows SDR spoofing that makes real OBUs broadcast implausible CAMs);
  - (c) an outsider relaying or replaying the victim's genuine messages out of range;
  - (d) an insider whose own pseudonyms create class 5 conflicts with the victim;
  - (e) an insider making class 4 sensor claims.

  Each cell records the minimum capability whose evidence survives MA re-verification, and whether the victim is revoked.
- **RQ1.2 Closing rules.** For each framable cell, compare corroboration rules:
  - event correlation (`ma_corroborate.rs`);
  - reporters that are distinct at the linkage authority (the Intel patent's independent-observer rule, US12489779B2, NEW);
  - diversity of witnesses in space and time;
  - a trusted RSU witness;
  - the CATS two-window rule (asavisanu2025cats).

  Each rule is scored on three costs: time to neutralise a real attacker, linkage-authority queries per revocation, and honest revocations per 10⁶ vehicle-hours.
- **RQ1.3 Benign correlated false positives.** At GNSS error distributions taken from field data, how often does each rule revoke honest vehicles?
  - The engine has already seen this: the count-based legacy gate revoked **21 honest vehicles in 300 s at 6,000 veh/h** from short GNSS degradations, in the shipped `revocation-latency` scenario with its attackers removed (`ma_corroborate.rs` header).
  - That is an internal anecdote. The approach turns it into a measured, field-anchored result. This is also the one mechanism `critique-f1.md` §2.1 found that no open-loop metric can price: correlated false positives defeat the independence that k-reporter thresholds assume.
- **RQ1.4 Remediation under adversarial evidence.** Under the 5GAA 2024 §6–7 severity-to-remediation mapping (R0/R1/R2, sub-types ST0–ST3, reinstatement classes RC1–RC4), what share of framings end in R2 suspension, and which reinstatement class undoes them?
  - Timing is a single paragraph of arithmetic from the 5GAA 2025 certificate policy: revocation processed within 5 days, CRL published within 24 hours (§4.10.3.4–5).

### Contribution claims
**Defensible:**
1. The first framability matrix of standard V2X detectors under an MA that re-runs the detectors on the report's evidence, with the minimum attacker capability per class.
2. For each framable class, the cheapest corroboration rule that closes it, and its price in linkage-authority queries and time to neutralise.
3. A field-anchored rate of honest revocations from benign correlated faults under count-based versus correlating MA rules.
4. An adversarial evaluation of the 5GAA §7 remediation mapping. The 2024 paper never mentions false reports (`critique-framed.md` §2.5).

**Claims to avoid:**
- "First framing, slander or Sybil-reporting attack". Ruled out by kamel2020detection (F2MD "MA Stress", thesis p56), the 5GAA 2022 white paper p12, bimeyer2014misbehavior, twardokus2021targeted, asavisanu2025cats and turgut2026ipek.
- "One insider defeats a w-witness rule". A tautology already in brecht2018security p7.
- "Jamming alone gets an honest vehicle revoked". Holds only for a naive counter of signature failures, which is what twardokus2021targeted attacks.
- "First evaluation of the 5GAA options" and "5GAA says the conditions are missing". The paper proposes them in §6–7.
- A stage decomposition of time to neutralise as a finding. Policy text fixes the stages.
- "Real MA behaviour", unless the MA model is confirmed with an operator.
- A victim safety cost as a headline. It is expected to be near zero (`critique-framed.md` §2.7).

### Closest prior work and the exact gap

| Work | What it does | Gap left |
|---|---|---|
| kamel2020detection / kamel2020simulation (F2MD) | Fake reports with rotating pseudonyms ("MA Stress") | A counting MA; no per-detector analysis of which evidence the MA can re-verify |
| Intel US12489779B2; Qualcomm US12260747B2 (both NEW) | MA slander defence: independent observers with different pseudonyms, LA linkage check, evidentiary threshold per misbehaviour type | Designs without measurement; no attacker-cost or query-budget numbers |
| 5GAA MBD white paper 2022 p12 (NEW) | Slander combined with Sybil can wrongfully revoke; reporter accountability must be traded against privacy | States the problem; no quantification |
| asavisanu2025cats | A central authority resolves real identities to defeat downvote collusion; time to ban | No SCMS evidence model, no TS 103 759 classes, no GNSS or relay framing |
| twardokus2021targeted | Jamming discredits an honest vehicle within 2 min under a reputation counter | No proof-based MA; never followed through to revocation |
| jimenez2026gnss | GNSS spoofing makes OBUs broadcast implausible CAMs | Not connected to reporting or revocation |
| CV-pilot report spec (via `03-protocols-and-operations.md:295`) | ObsImplausible is **off by default because of relay attacks** | Practitioners already judged one cell of the matrix by hand. The approach must reproduce that judgement (a validity check), then fill in the other cells |
| park2024enhancingb (IET ITS 2024; **not read**: HTTP 403 again this session) | Local plus global MBD with slander defences, field-tested at K-City | Unknown. This is the most dangerous unread overlap |
| haidar2020dare | Offline dataset of reports for evaluating the MA | Supports open-loop MA evaluation, which this approach uses for its policy sweeps |

### Key experiments and hypotheses
1. **Framability matrix.** Run each capability against honest victims chosen at random, across two maps and two densities.
   - **H1.1:** class 1 and 2 checks are framable by an outsider spoofing the victim's GNSS alone, with no credentials, because the victim honestly signs implausible messages.
   - *Decision-changing:* J3287 and TS 103 759 editors would mark those checks as not sufficient for revocation without a test that rules out spoofing, such as other vehicles in the same spoofing footprint. TS 103 759 is under revision (v2.2.1, 2026).
   - **Validity check that must reproduce a known result:** the ObsImplausible range check must come out relay-framable, matching the CV-pilot decision to turn it off.
2. **Closing rules.**
   - **H1.2:** requiring k reporters that are distinct at the linkage authority, over at least two disjoint events, closes the insider classes (4 and 5) at a few queries per revocation and adds less than one CRL cycle to time to neutralise.
   - *Decision-changing:* a concrete corroboration rule, with its privacy price, that an MA operator can adopt.
   - *Useful null:* if one query per accused vehicle suffices, that is still a short standards-input paper (`critique-framed.md` §3).
3. **Benign correlated faults.**
   - **H1.3:** under urban-canyon GNSS error distributions, count-based rules revoke honest vehicles at a rate a fleet operator would notice, and event correlation cuts that rate by orders of magnitude at a bounded cost in time to neutralise.
   - *Decision-changing:* this is evidence for OEM warranty decisions and for an MA rule.
4. **5GAA mapping under attack.** Feed the framings from experiments 1–3 through the §7 mapping and the RC1–RC4 reinstatement classes, and report false suspensions and reinstatement delay.

### Who uses it
- J3287 and TS 103 759 editors: which evidence classes are sufficient for revocation.
- The SCMS Manager and MA operators, such as the ISS Misbehavior Authority Service (NEW; press release 2019-01-07; TFHRC operational test 2020): corroboration rules and query budgets.
- 5GAA MBD work items: an adversarial test of their mapping.
- OEMs: the warranty exposure from framed customers.

### Venue
- First: **VehicleSec**.
- Fallback: IEEE VNC or ACM WiSec. NDSS only if the MA model is confirmed with ISS or the SCMS Manager.
- Journal: **IEEE TDSC** or **TIFS**.

### Top reviewer objections and rebuttals
1. *"A GNSS-spoofed vehicle really did broadcast bad data, so revoking it is correct."*
   - The device is honest and loses its credentials, and the attacker can choose the victim. Whether an MA can tell a spoofing victim from a liar is the decision at stake.
   - RQ1.4 measures whether the reinstatement classes repair the damage.
2. *"Your MA is still invented."*
   - It re-runs the standard detectors on the standard evidence. Only what the standards leave open (corroboration rule, query budget, how to weigh class 3 and 4 claims the MA cannot re-verify) is a parameter.
   - Results are reported for each rule. Validation is sought from ISS or the SCMS Manager.
3. *"Relay framing of ObsImplausible is already known."*
   - Agreed. It is used as the validity check. The contribution is every other cell, and the rules that close them.
4. *"Ethics."*
   - Simulation only. Findings go to the SCMS Manager and the 5GAA MBD work item before submission.

### Engine prerequisites
- A **proof-based MA plug-in** that re-runs the `ts103759.rs` checks on the report's evidence. It would sit beside `ma_corroborate.rs`.
- **Linkage-authority queries** through the `v2xw-proto` investigation flow, with a query counter. The flow is noted as absent from `ma.rs` ("What is not here").
- A GNSS-spoofing attacker that targets a victim's receiver, plus a relay attacker that targets the victim. Both need confirming; relay and wormhole are referenced at `attack.rs:558`.
- A GNSS error model anchored to field data for RQ1.3. The source is still to be chosen and cited.
- **Methods fix:** key the Report, ServiceTime and Backend streams by (stable reporter, report counter) rather than by node with a running index (`critique-methods.md` §3 item 2). Without it, even backend-only sweeps are not exactly paired.
- An open-loop backend replay of recorded report streams through each MA rule, DARE-style. This keeps the policy sweeps cheap.

### Residual risk and mitigation
- **Medium.** park2024enhancingb, SAE J3287 or TS 103 759 v2.2.1 may already rate evidence per class.
  - Mitigation: get all three through the library or interlibrary loan before building anything.
  - If Park already scores slander per class, narrow to the GNSS, relay and correlated-fault extension, with SCMS linkage-authority pricing.
- **Low to medium.** The class 1/2 spoofing result may look obvious.
  - Mitigation: the value is the rule and its price, not the existence of the attack.

---

## 3. Approach 2: Pricing V2X detector errors

### Pitch
5GAA says misbehaviour detection is tuned "to balance the overall impact of false positives and false negatives when the consequences are carefully considered" (2022 white paper p12). Yet it calls a full cost-benefit analysis "resource-intensive and unfeasible" and rates impact only qualitatively (2024 §5.2.1, §6.3; quoted in `critique-f1.md` §2.3). The consequence-based tuning that industry describes has no quantitative instrument.

This approach supplies one:
- **Prices:** estimated from paired reruns with a modelled driver response, giving the harm of one false positive and one false negative per attack type, application (CACC/platoon, FCW, EEBL, IMA) and context.
- **A proxy test:** whether a cost-weighted open-loop metric using those prices recovers the closed-loop ranking of several hundred detector × threshold × reaction operating points.
- **Labels:** harm labels per sender episode (from blind proposal B), so that VeReMi-style datasets can be rescored.

### Research questions
- **RQ2.1 Prices.** What is the harm per false positive and per false negative, and how does it vary with application, density, penetration rate, driver compliance and reaction time?
- **RQ2.2 Proxy.** Does a metric weighted by those prices recover the pairwise closed-loop ordering of operating points? Where does it fail? The candidate failures are clustering near conflicts and propagation through platoons.
- **RQ2.3 Labels.** What share of detected misbehaviour episodes were harmless, and what share of harmful episodes do the top-F1 detectors miss? The labels come from suppressing each sender episode and measuring locally.
- **RQ2.4 Adaptive attackers** (optional). How much detection survives an attacker who maximises harm in the closed loop?

### Contribution claims
**Defensible:**
1. The first published table of V2X detector error prices, with uncertainty and with the driver factors swept.
2. A proxy-recovery test over hundreds of operating points, with bootstrap intervals over seeds. It is useful whichever way it comes out.
3. Harm-labelled sender episodes released alongside the engine.

**Claims to avoid:**
- "First impact-based MBD evaluation": wehmer2021practical.
- "First closed-loop false-positive harm": zhang2024enhancing.
- "First F1 plus accidents saved": franceschini2024neural.
- "First consequence-based detector and response choice": sun2024act (TIFS 2024).
- "Accuracy metrics mis-rank detectors": Gaffney and Ulvila, IEEE S&P 2001 (doi:10.1109/SECPRI.2001.924287, NEW); Cárdenas et al., IEEE S&P 2006 (doi:10.1109/SP.2006.2, NEW); Urbina et al., CCS 2016 (doi:10.1145/2976749.2978388, NEW).
- "Open-loop metrics mis-predict closed loop" as a new genre: Philion et al., CVPR 2020 (doi:10.1109/CVPR42600.2020.01407, NEW); Dauner et al., arXiv 2306.07962 (NEW).
- "Practitioners pick detectors by F1": a strawman (`critique-f1.md` §2.3).
- "Wrongful revocation is the gap": revocation takes days. Its harm is accounted in Approach 1, not here.
- "Exact per-message harm": exact only open loop and in the near field of a discard (`critique-methods.md` §2).

### Closest prior work and the exact gap

| Work | What it does | Gap left |
|---|---|---|
| zhang2024enhancing, zhang2024simulation | IMA safety metrics with MBD on and off, including false-positive cost; F1 and safety side by side | One detector; no price table; no ranking test |
| franceschini2024neural (arXiv 2411.07013) | VeReMi detectors in a closed-loop platoon; F1 plus accidents saved | Thesis-scale; no proxy test, no prices |
| wehmer2021practical | Same-seed attack versus no-attack runs; attacker gain in travel time | One detector; no false-positive harm; no safety outcome |
| sun2024act | Game-theoretic choice of detector deployment and mitigation by closed-loop gap | No per-error prices, no open-loop proxy |
| wu2026bsmattacker (Zenodo 10.5281/zenodo.20754022) | Nine detectors; replay of surviving attacks against real FCW, IMA and EEBL warnings | Open-loop replay; results "under revision". It is the real-data cross-check for this approach |
| hermann2026veremi (VeReMi NextGen, VNC 2026) | New open-loop dataset with fixed splits | No harm labels; the natural adopter of the price table |
| muzammil2026code, skaik2026reliable | Audits finding artefacts and leakage in VeReMi | Motivates a harm-labelled track |
| IntentProv-IoV (doi:10.3390/sym18091430, NEW) | Counterfactual contribution score inside a structural causal model | Not simulation reruns; no benchmark |

**The gap:** many operating points, priced harm, and a recovery test, in V2X.

### Key experiments and hypotheses
1. **Prices.**
   - Use CACC/platoon, which closes the loop today (`v2xw-engine/src/run/coop.rs`), and FCW, EEBL and IMA, once a driver-response model exists.
   - Draw events from a declared distribution, not chosen by hand.
   - Use surrogate measures (TTC, PET, DRAC) together with Joerer's exposure-normalised collision probability (doi:10.1109/TVT.2013.2287343).
   - **H2.1:** the price of a false negative varies by more than an order of magnitude across application and context, while the price of a false positive is driven mainly by unnecessary braking and platoon dissolution.
   - *Decision-changing:* a single F1-optimal threshold is the wrong default, and thresholds should differ by application.
2. **Proxy recovery.**
   - Replay every operating point open loop over the recordings, which is exact.
   - Run closed-loop reruns only on a stratified sample.
   - **H2.2:** the price-weighted metric recovers at least 90% of pairwise orderings, except where false negatives cluster within the warning horizon of a conflict.
   - *Decision-changing:* if it recovers, dataset maintainers adopt the weights and closed loop is unnecessary. If it fails, a closed-loop track is needed, with the failure mechanism named.
3. **Sensitivity of the prices.**
   - Sweep compliance, reaction time (yan2014driving as one anchor) and penetration rate.
   - Report prices as functions of these, not as point values.
4. **Labels** (RQ2.3).
   - Suppress each sender episode and measure inside a declared radius and time window.
   - Report results as seed-level average treatment effects with cluster-robust intervals, using Holm or Benjamini–Hochberg correction.

### Who uses it
- MBD dataset maintainers (VeReMi NextGen, BSMAttacker) and detector authors: adopt the weights.
- 5GAA: quantitative support for its §6.3 impact classes.
- OEM and LMBD vendors: per-application thresholds. This is weaker, since they tune by expert judgement.

### Venue
- First: **VehicleSec** or **IEEE VNC**.
- Fallback: ACM WiSec. NeurIPS Datasets & Benchmarks only if the operating-point corpus is large and documented.
- Journal: **IEEE TVT** or **TIFS**. Not CCS (`critique-f1.md` §3).

### Top reviewer objections and rebuttals
1. *"Your driver model sets the prices."*
   - The driver factors are swept, and every price is reported as a function of them.
   - Prices are cross-checked against BSMAttacker's real-data warning replay where the two overlap.
2. *"Cost-based evaluation is 25 years old."*
   - Agreed: the framework is Gaffney–Ulvila's. The contribution is the V2X prices themselves, which 5GAA calls infeasible to obtain, and the recovery test.
3. *"Rare events: you cannot resolve the rankings."*
   - The analysis ranks hundreds of operating points, not ten detectors, using pairwise disagreement with bootstrap intervals and pre-registered weights (`critique-f1.md` §2.4).
4. *"Your pairs are not exact."*
   - Per-unit claims are made only open loop and in the near field. Everything else is a seed-level estimate, with coupling measured (§5).

### Engine prerequisites
- **A warning-response driver model** (blocking): compliance probability, a reaction-time distribution and a braking profile, all exposed as sweepable factors. Today FCW, EEBL and IMA warnings do not move vehicles (`run.rs:4324-4333`; `critique-methods.md` §2.4).
- **An open-loop replay harness** that runs N detectors over one recording (`critique-methods.md` §3 item 6).
- **Methods fixes 1–4** from `critique-methods.md` §3:
  - stable identity keys by `trip.seq`, which needs an ADR 0004 amendment and a re-bless of the golden digests;
  - event-keyed draws where the number of draws depends on state;
  - a coupling-audit recorder;
  - a placebo-intervention test shown to go red on the current keying.
- A detector library, reproduced first against published F1 on VeReMi-like traces.
- **A compute budget.**
  - Measured speed: 1.74 wall-seconds per simulated second at 145 vehicles and 14.3 at 453 (`docs/validation/test-run-cost.md`).
  - The ranking runs open loop. Closed-loop runs use near-field scenarios of 150 vehicles or fewer, or budgeted cloud runs.

### Residual risk and mitigation
- **Medium to high.** The driver-response model dominates the results, and BSMAttacker's revision may publish a real-data F1-versus-safety comparison first.
  - Mitigation: lead with the price table and its sensitivity, which BSMAttacker does not produce. Position the work as the closed-loop complement. Contact the BSMAttacker authors.
- **Medium.** Compute.
  - Mitigation: replay open loop first, then a stratified closed-loop sample.

---

## 4. Approach 3: Who can follow an honest vehicle?

### Pitch
Pseudonym-privacy studies usually test one channel, at the application layer, against one eavesdropper. A US vehicle running the full security stack leaks through several channels at once. Three of them are created by the security machinery itself: misbehaviour reports, linkage-seed CRLs and the provisioning channel.

This approach builds an observer × channel matrix. Each cell is the **marginal** linkability over a kinematic tracker, measured against ground-truth identity in the same run.
- **Observers:** a dense or sparse sniffer network, an RSU operator, the MA, the RA, a linkage authority, and coalitions of these.
- **Channels:** LTE-V2X SPS reservation continuity, the 1609.2 signer and certificate-change timing, report streams, the CRL channel after a wrongful revocation, and provisioning.

Each mitigation is priced in PDR and awareness. The lead case is whether an SPS reservation survives a pseudonym change.

### Facts established this session
- **TS 36.321 §5.14.1.1** lists the LTE sidelink reselection triggers. **None** is a change of Layer-2 ID or an upper-layer privacy request. Reselection is triggered by the counter reaching zero, by **no transmission for one second**, by `sl-ReselectAfter`, by MCS and latency limits, and by pool reconfiguration (https://itecspec.com/3gpp/36.321/s/5.14.1.1).
- **TS 33.536 §5.4.3.2** requires the UE to change and randomise its source Layer-2 ID and IP address when the application-layer ID changes. It says nothing about resource reservations (https://itecspec.com/3gpp/33.536/s/5.4.3.2).
- **TS 38.321 §5.22.1.1** (NR mode 2) showed no such trigger either. §5.22.1.2 was not checked.
- **Two consequences:**
  - A silent period of at least 1 s breaks reservation continuity by rule.
  - The leak exists exactly where kinematic defences are designed to work: synchronised changes in dense traffic with no silent period.
- **The idea is known in principle.**
  - Qualcomm patent US11265707B2 (priority 2018, NEW, https://patents.google.com/patent/US11265707B2/en) says that "if a same SPS is used before and after key/ID roll-over, an observer may infer that it is for the same UE". It proposes rolling SPS parameters over together with the identifiers, for **unicast** links only.
  - liu2018cross, liu2018mac and liu2020marp (TDSC) show MAC-layer semantic linking for slotted MACs.
  - So the novelty is measurement, not the idea: deployed broadcast SPS, against a kinematic baseline, with insiders, and with the mitigation's cost.

### Research questions
- **RQ3.1 Matrix.** For each observer and channel, what share of pseudonym changes becomes linkable beyond what the kinematic tracker in `privacy.rs` already links? What is the posterior anonymity set?
- **RQ3.2 SPS continuity.** Under US certificate-change policy (J2945/1-style, still to verify), how many changes does reservation continuity link that kinematics leave ambiguous? Consider three mitigations:
  - forced reselection at an ID change, with a random offset;
  - a silent period of at least 1 s;
  - doing nothing.

  What does each cost in PDR and collisions? The cost of reselection is known from the SPS literature.
- **RQ3.3 Backend observers.** What can the MA reconstruct about reporters from the report streams? ObsImplausible reports carry the reporter's own BSM (`03-protocols-and-operations.md:296`). And what does Approach 1's linkage-authority query budget reveal?
- **RQ3.4 The revocation channel.** After a wrongful revocation, how much extra tracking does a linkage-seed CRL give, compared with ACPC activation codes and enrolment blocking (5GAA 2025 certificate policy §4.10.3.4–5)? The question matters most across days and through coverage gaps.

### Contribution claims
**Defensible:**
1. The first measured observer × channel matrix of marginal linkability for the US V2X stack, including SCMS insiders, on city traffic with ground truth.
2. A quantified SPS-continuity leak for broadcast BSMs, with the PDR price of each mitigation.
3. The extra tracking a linkage-seed CRL gives over kinematics after a (wrongful) revocation, compared with revocation paths that publish no seeds.

**Claims to avoid:**
- "First MAC-layer pseudonym linking": liu2018cross, liu2020marp.
- "First to note that SPS persists across ID change": Qualcomm US11265707B2.
- "First PHY-layer linking": bloessl2015scrambler, vaas2018poster.
- "First multi-radio linking": yieh2019multi, zoccoli2025radar.
- "Pseudonym change alone is insufficient": wiedersheim2010privacy, escher2021well, zoccoli2023vanets.
- "Revocation makes devices linkable": the design intent, per brecht2018security p12 and whitefield2017formal. The marginal amount is what is new.
- "First issuer linkability": huang2025privacy.
- Any PHY fingerprinting claim. The engine does not model RF impairments.

### Closest prior work and the exact gap

| Work | What it does | Gap left |
|---|---|---|
| liu2018cross, liu2020marp | MAC semantic linking and a MAC-aware pseudonym scheme | Generic slotted MAC; not 3GPP SPS; no SCMS; no kinematic baseline |
| Qualcomm US11265707B2 (NEW) | Names SPS persistence as a linking risk; rolls SPS over with the IDs | Unicast only; no measurement |
| bitsikas2025security (arXiv 2502.16650) | 3GPP leaves the frequency of ID changes and cross-layer synchronisation undefined | Specification analysis; no measured linking |
| twardokus2022vehicle | SPS predictability exploited for DoS | Not used for privacy |
| escher2021well, khodaei2020cooperative, wiedersheim2010privacy | Kinematic and static-field linking | Single layer; no MAC or insider channels |
| willecke2024primire, simplicio2018privacy, correia2023range | Privacy-preserving MA aggregation; revocation without permanent linking | Proposals; no measurement of extra leakage on traffic |
| V-TRACE (JIPS, NEW) | Persistent tracking of flagged vehicles across pseudonym changes | Defensive use; no channel matrix |

### Key experiments and hypotheses
1. **SPS continuity**, on the LTE-V2X stack across densities and change policies.
   - **H3.1:** with synchronised changes and no silent period, reservation continuity links most of the changes that the kinematic tracker leaves ambiguous in dense traffic.
   - *Decision-changing:* a one-line rule for J3161/1 or 3GPP RAN2: reselect at an L2 ID change. The cost is priced.
   - *Null:* if forced reselection is already mandated, report how much an "adjacent restart" still leaks, and stop.
2. **The CRL channel.**
   - **H3.2:** a linkage-seed CRL adds little over kinematics for a dense sniffer network, but dominates linking across days and through coverage gaps (the plausible answer `critique-framed.md` §2.4 asks to measure).
   - *Decision-changing:* prefer ACPC or enrolment blocking for misbehaviour classes prone to wrongful revocation. This connects to Approach 1.
3. **MA reconstruction of reporters.**
   - **H3.3:** an MA that keeps raw report evidence can reconstruct a material share of reporters' trajectories.
   - *Decision-changing:* a data-minimisation rule for MA retention, of the kind PriMiRe proposes, made quantitative.
4. **Joint budget.** How much does combining channels beat the best single channel?

**Method note.** The observer never acts, so experiments 1–3 run exactly on recorded runs. Only the mitigation costs need reruns, and those are distribution-level, reported as average treatment effects over seeds. Methods validity is the strongest of the three approaches.

### Who uses it
- SAE (J3161/1, J2945/1) and 3GPP RAN2/SA3: identifier-change and reselection rules.
- The SCMS Manager and MA operators: data retention, and the choice of revocation path.
- IEEE 1609.2.1 editors: a privacy-driven argument for revocation that publishes no seeds.

### Venue
- First: **PoPETs**, a measurement paper with a policy recommendation.
- Fallback: ACM WiSec or VehicleSec.
- Journal: **IEEE TIFS**.

### Top reviewer objections and rebuttals
1. *"Identifiers must change together. That is known (TR 33.885; the Qualcomm patent)."*
   - The specifications treat L2 and IP IDs as identifiers, not reservations, and the reselection triggers omit an ID change (TS 36.321 §5.14.1.1).
   - The contribution is the measured marginal leak and the cost of closing it.
2. *"Kinematics already link everything."*
   - That is why every result is marginal over the kinematic tracker. It is reported in the regimes where kinematic defences work.
3. *"Chipsets already reselect."*
   - Must be checked against J3161/1 and, ideally, a capture from a commercial OBU. This is the go/no-go below.
4. *"Your MAC is a model."*
   - The SPS semantics are fixed by the specification, and the engine's SPS is matched to OpenCV2X, so the leak is structural rather than an artefact of calibration.

### Engine prerequisites
- A Layer-2 source ID in the sidelink MAC, changed together with the certificate. NR SCI format 2-A is believed to carry 8 bits of the source ID; verify in TS 38.212 §8.4.
- Reservation metadata visible to the observer through the `ObservedMessage` firewall (`obs.rs:116`): resource, reservation interval, and the time of reselection. Note that the reselection counter itself is not on the air.
- The certificate-change policy per J2945/1, to verify.
- Observer views for the MA, RA and linkage authority. ACPC and enrolment-blocking revocation paths.
- Long or stitched runs for linking across days.
- A field anchor: one LTE-V2X capture from a testbed partner, to confirm reservation behaviour at an ID change.

### Residual risk and mitigation
- **Medium.** J3161/1 or chipsets may already force reselection.
  - Mitigation (go/no-go): read J3161/1 and get one capture before building.
  - If the leak is closed, the lead case becomes the backend insiders and the CRL channel (RQ3.3, RQ3.4), which do not depend on it.
- **Medium.** PoPETs may judge the result incremental.
  - Mitigation: lead with the joint matrix and the insiders, not a single leak.

---

## 5. Shared instrument and validity chapter (what remains of old Direction 3)

### The instrument, re-worded
This replaces "exact paired counterfactual reruns" everywhere (`critique-methods.md` §5):

> Paired reruns with common random numbers, keyed by stable entity and event identity. They are exact per unit for open-loop and backend-only interventions, and for the near field of closed-loop interventions. Elsewhere they are distribution-level estimates over seeds, with coupling measured by a placebo-validated audit.

### Engine fixes, needed before any headline number (in order)
1. Stable identity keys: `trip.seq` for actors and a stable node identity, replacing `next_actor` and `next_node` (`v2xw-mobility/src/engine.rs:3147-3150`; `run.rs:3168`). This needs an ADR 0004 amendment and a re-bless.
2. Event-keyed draws for every stream whose draw count depends on state. That covers Shadow, LaneChange, MacBackoff after a grant, SPS reselection, and the Report, ServiceTime, Backend and Crypto streams, via a generic event key on the existing `LinkFrame` pattern (`rng.rs:47-58`).
3. A coupling-audit recorder, and `v2xw compare --coupling`.
4. A placebo intervention that is shown to go red on today's keying. This follows the project's rule that a check must be able to fail.
5. A warning-response driver model (Approach 2).
6. An open-loop replay harness (Approaches 1 and 2).

### The validity chapter
- Map every result against Arp et al.'s ten pitfalls (arp2020dos; USENIX Security 2022) and Kurkowski's four credibility axes (doi:10.1145/1096166.1096174).
- Sort factors into three classes:
  - environment: Morris screening with r ≈ 20, about 420 runs, then Sobol analysis on at most 4–5 factors;
  - policy: swept and mapped;
  - adversary: worst case.
- Use PRIM scenario discovery on flip indicators (`critique-disagree.md` §5).
- Start from the internal one-at-a-time sweep of 217 settings (`docs/validation/sensitivity-catalogue.md`) and cite it honestly.
- Replicate one headline result per approach in Veins or Artery.

### The optional audit paper
It goes ahead only if a desk audit finds **at least three** genuine, reconcilable pairs of disagreeing papers. Candidates:
- abrar2026deployment vs Bazzi 2021 vs Teixeira and Henriques (SBSeg 2026, NEW);
- twardokus2024cryptography vs agustina2026lightweight;
- VeReMi-Extension vs hermann2026veremi vs wu2026bsmattacker.

Venue: VNC or ACM TOMACS.

---

## 6. Not chosen, and what would change that

**Blind C, SCMS resilience and crypto-agility (runner-up).**

For it:
- strong practitioner demand: the unfunded "Impact of PQC on SCMS operation" research idea (`eval-traction.md` §PQ), and NIST IR 8547's deprecation timeline;
- a strong engine fit: an elector-signed CTL, threshold authorities, three radio stacks.

Against it:
1. **Recovery timing is set mostly by policy and by OEM push.** In the SCMS root-management procedure, OEMs push "Add Root CA" over their own transport (USDOT SCMS wiki, https://its.dot.gov/research_areas/cybersecurity/scms/attachments/27427253/27427254.pdf). That invites the same "arithmetic" objection `critique-framed.md` §2.6 raised against the time-to-neutralise decomposition.
2. **Self-healing has already been demonstrated in emulation** (mushrall2018emulab; abstract only, no numbers), and industry has run fleet-scale reissuance exercises (Keyfactor case study, NEW).
3. **The post-quantum half is the most crowded area in the corpus:**
   - chen2024hybrid (arXiv 2501.07028): hybrid certificates field-tested on a Chunghwa Telecom SCMS;
   - chen2026pseudonym, fikry2026post, twardokus2024cryptography, abrar2026deployment;
   - Teixeira and Henriques 2026;
   - two funded TraCR projects.

**It would be promoted if** a non-arithmetic mechanism proved large. The best candidate is the P2PCD and verification gap after a PCA or ICA replacement, which is a joint radio × PKI effect. The corrected PQ question (`critique-disagree.md` fix 4: scheme × certificate-inclusion policy × stack under strict J3161) can be a chapter if PQ demand is needed.

**Standalone post-quantum.** Still too crowded, and old H9 is contradicted by the engine's own `sensitivity-security.md`: ML-DSA-44 drops PDR from 0.96 to 0.62 on a 20–60 vehicle grid.

**Threshold authorities.** Narrow after ajibuwa2023resilient, which uses FROST in a VPKI.

**LLM root-cause analysis.** Its novelty depends on the agent, not on the engine.

---

## 7. What changed from the previous top three, and why

| Previous | Now | Why |
|---|---|---|
| D1 "Framed and revoked", headline H1a/H1b, USENIX Security | **Approach 1, framability under a proof-based MA**, VehicleSec | The witness-counting MA was a strawman (TS 103 759 §4). H1a is a tautology already in print. H1b attacks a naive counter. The 5GAA 2024 paper was misread. The timing stages are fixed by the 2025 certificate policy. The victim's safety cost is about zero. Kept: the SCMS-specific evidence and corroboration question, the engine's strongest fit. Added: correlated benign false positives, already seen in the engine (21 honest revocations in 300 s) |
| D1's privacy payload (H2) and reporter-privacy price (RQ1.3) | **Moved into Approach 3** | They had no kinematic baseline and may not exist on the deployed revocation path (ACPC, enrolment blocking). Measuring them properly needs the observer matrix |
| D2 "Does F1 pick the safest detector?", CCS | **Approach 2, pricing errors, merged with blind B**, VehicleSec/VNC | H5 (an infrastructure gap) is false by construction: revocation takes days and is separable. No closed-loop outcome exists for human drivers today. "F1 is how detectors are chosen" is a strawman. Novelty at security venues is thin (S&P 2001/2006, CCS 2016, CVPR 2020). Kept: the price table and the proxy test, which blind B reached independently |
| D2's RQ2.4, pairing validity, as a contribution | **A placebo test in the validity chapter** | Buffalo et al. (arXiv 2603.11084, NEW) already make the argument, and the engine is already counter-based |
| D3 "Why studies disagree", lead case PQ, USENIX Security | **Demoted** to the validity chapter, plus an optional audit paper behind a go/no-go | The lead contradiction is not one. H9 is contradicted. H10 has no instance. Anchors exist only for radio. Sobol is infeasible on 8 GB |
| No privacy direction (cross-layer linkability was judged "mostly already done" in `novelty-check.md` R1) | **Approach 3** | Blind A reached it independently. This session confirmed that the 36.321 reselection triggers omit an ID change. The direction also gives old D1's privacy claims the baseline the critic demanded. Its novelty is a measurement, as with Approach 1 |
| "Exact paired counterfactuals" as the common instrument | **The re-worded instrument (§5)** | Streams are entity-keyed, ids follow insertion order, and draw counts depend on state (`critique-methods.md` §1) |

---

## 8. Searches and checks run for this file (2026-10-17)

| Check | Source | Result |
|---|---|---|
| Linking pseudonyms through SPS reservations | WebSearch (extended); OpenAlex (4 queries); arXiv API over HTTPS (`all:sidelink AND all:privacy` gives 3 hits; `abs:V2X` gives 930, so the API works) | No paper measuring it. Found Qualcomm US11265707B2 (unicast; names the risk). Phrase-quoted arXiv queries returned 0, which may be a query-syntax effect, so they are not taken as negatives |
| LTE and NR sidelink reselection triggers | itecspec 36.321 §5.14.1.1; 38.321 §5.22.1.1 | No ID-change trigger in either. Reselection after 1 s without transmission (LTE). 38.321 §5.22.1.2 not checked |
| L2 ID change rule | itecspec 33.536 §5.4.3.2 (via WebSearch) | Change the L2 ID with the application ID; nothing on reservations. J3161/1 not reachable |
| Measured SCMS compromise recovery | WebSearch | Only design text (brecht2018security), the USDOT root-management procedure (manual in the proof of concept; OEM push), the EmuLab demonstration (mushrall2018emulab) and the Keyfactor exercise. No measured end-to-end timing |
| PQ mixed-fleet migration | WebSearch (extended); catalog | chen2024hybrid, chen2026pseudonym, abrar2026deployment, donghia2024shaping, all in the catalog. No mixed-fleet migration simulation found |
| Harm-labelled MBD datasets | WebSearch (extended) | VeReMi NextGen, SPADE, PAMPOS: none has harm labels |
| GNSS or relay framing through to revocation | WebSearch | Building blocks only (patents; whitefield2017formal). No end-to-end study |
| park2024enhancingb full text | Wiley PDF and full-text pages; OpenAlex locations | HTTP 403 again; no repository copy. **Still unread** |
| Engine facts | Code reads (§1.4) | Confirmed: TS 103 759 classes, corroborated MA, CTL with electors, threshold LA, privacy observer. Missing: MAC fields for the observer, an L2 ID in the MAC, a driver warning response |

Semantic Scholar was not queried this session.

## 9. Corrections made to earlier synthesis files

Each correction is marked "(corrected after critique)" where it was made.

| File | Error | Correction |
|---|---|---|
| `novelty-check.md` | "No V2X paper using FROST" and the verdict "novel (narrow)" for threshold authorities | ajibuwa2023resilient uses FROST in a VPKI. Verdict changed to "partly novel, narrow" |
| `claims-we-cannot-make.md` | The threshold row implied no FROST V2X work | Row rewritten to cite ajibuwa2023resilient, with the surviving 1609.2-compatible claim |
| `claims-we-cannot-make.md` | The framing row omitted F2MD's MA Stress attack, 5GAA 2022 and the patents, and implied jamming-to-revocation was open | Row extended |
| `eval-traction.md` | "No FROST or CGGMP21 V2X work"; the 5GAA 2024 "conditions not specified" claim (three places) | Corrected per `critique-framed.md` §2.5 |
| `eval-B-revocation.md` | 5GAA ladder given as ST0–ST2; "no conditions" | Ladder corrected to R0–R2, ST0–ST3 and RC1–RC4, with the §6–7 mapping noted |
| `eval-A-counterfactual.md`, `novelty-check.md`, `claims-we-cannot-make.md` | "Exact" per-unit counterfactuals | Note added on entity keying, insertion order, and the missing warning loop |
| `eval-C-robustness-and-threshold.md` | arXiv API listed as searched | Note added that the empties were likely transport failures, so not negatives |
| `top-three-directions.md` | H1a tautology; H1b strawman; 5GAA misreading; timing decomposition; per-unit exactness; the petho vs abrar "contradiction"; H9; the common-instrument wording; the arXiv-failure statement | All marked inline, with a "superseded" banner pointing here |

## 10. Read in full before stating any of this as fact
- **Blocking for Approach 1:** park2024enhancingb (doi:10.1049/itr2.12549), SAE J3287 (2024), ETSI TS 103 759 v2.2.1 (2026), and the 5GAA 2025 certificate policy §4.10.
- **Blocking for Approach 3:** SAE J3161/1 (identifier-change and reselection rules), J2945/1 (certificate-change policy), TS 38.321 §5.22.1.2 and TS 38.212 §8.4 (SCI 2-A fields).
- **For Approach 2:** wu2026bsmattacker's revised release, and VeReMi NextGen in full.
- **For the runner-up:** the mushrall2018emulab full text, and the Keyfactor case study.
