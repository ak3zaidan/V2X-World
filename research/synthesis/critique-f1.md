# Hostile critique of Direction 2: "Does F1 pick the safest detector?"

Date: 2026-10-17. Target: §2 of `top-three-directions.md`. Reviewer stance: the most demanding member of the ACM CCS programme committee named there as the first-choice venue. Ids resolve to `summaries/<id>.md`. Papers marked NEW are not in `catalog.jsonl`. Engine facts come from the repository's own code and design notes, which were read but not run.

## Verdict: keep with major changes. Demote it, and do not aim it at CCS

The question is sound and useful. But it rests on four claims, and all four fail:
1. that the infrastructure path (MA aggregation and revocation) is what makes open-loop metrics fail;
2. that the engine can measure closed-loop safety;
3. that practitioners pick detectors by F1;
4. that the methodology is new to security venues.

What survives is a narrower and more useful paper: **calibrating the exchange rate between open-loop errors and closed-loop harm for V2X detectors**, testing whether a calibrated cost-weighted metric recovers the closed-loop ranking, and reporting the conditions where it does not. That result is publishable either way at VehicleSec, VNC or WiSec, with IEEE TVT or TIFS as the journal. It is not a CCS paper. It should be a chapter that serves Directions 1 and 3, not the thesis's second pillar.

---

## 1. Kill search: prior work that already does it (or most of it)

No single work does the full design: many detectors ranked by exact paired net safety, with revocation in the loop and a proxy test. But the pieces are more pre-empted than §2 admits.

| Work | What it already does | Effect on Direction 2 |
|---|---|---|
| zhang2024enhancing (VTC 2024-Fall, local PDF read) | Closed-loop IMA safety metrics with MBD on and off. It reports that "for the nominal case, the AIA with the misbehaviour detection is slightly lower than that of the nominal case due to occasional false positives" | **False-positive safety cost is already measured in closed loop.** §2 lists "false-positive harm" as one of the additions over Zhang. Only *wrongful revocation* harm remains new |
| zhang2024simulation (IV 2024, local PDF read) | Reports accuracy, precision, recall and F1 *and* safety metrics for the same detector in one framework | The two metrics already sit side by side; only the multi-detector rank comparison is missing |
| franceschini2024neural (arXiv 2411.07013, thesis, local PDF read) | VeReMi-trained detectors run in a closed-loop platoon. It reports both F1 and "a gain that represents how many accidents are saved", and notes that false positives trigger platoon dissolution | A thesis-level precedent for "F1 plus accidents prevented" for V2X detectors |
| sun2024act (IEEE TIFS 2024) | A game-theoretic choice of *detector deployment and mitigation* "even amidst possibly misleading attack detection reports", scored by closed-loop inter-vehicle distance | Choosing a detector and response by consequence, with false reports, is already at TIFS, the fallback journal named in §2 |
| kalogiannis2022attack (ACM WiSec 2022), kalogiannis2025attention (local PDF read) | Platoon attack impact plus MBD. Both stress high false-positive rates, but evaluate by F1 or ROC | Shows the community knows false positives matter; no rank test |
| wu2026bsmattacker (Zenodo 10.5281/zenodo.20754022; GitHub README v1.0.1 read) | Nine detectors trained on VeReMi, five real datasets, then "replays the surviving attacks against real safety-application warning events" (FCW, IMA, EEBL). Results "under revision" | This is the open-loop safety-weighted proxy that RQ2.2 would test. When the revision lands, it may contain the F1-versus-safety comparison on *real* data, which beats simulation |
| haidar2020dare (VTC 2020-Spring) | A dataset of misbehaviour reports for evaluating the *global* MA offline | MA aggregation can be evaluated **open loop**. This undercuts H5 (see §2.1 below) |
| Gaffney and Ulvila, IEEE S&P 2001, doi:10.1109/SECPRI.2001.924287 (NEW); Cárdenas, Baras and Seamon, IEEE S&P 2006, doi:10.1109/SP.2006.2 (NEW) | Decision-theoretic and cost-based detector evaluation: ROC and F1 are insufficient, so the costs of false positives and false negatives must be weighted | A CCS reviewer will cite these against hostile point 1 in §2. "Accuracy metrics mis-rank detectors once costs differ" has been known at top security venues for 20 years |
| Urbina et al., CCS 2016, doi:10.1145/2976749.2978388 (already in §2) | Detectors scored by attack impact at a fixed false-alarm rate | A precedent, as §2 says. But it also means "impact-based detector evaluation" is not new at CCS |
| Philion et al., "Planner-centric metrics", CVPR 2020, doi:10.1109/CVPR42600.2020.01407 (NEW); Dauner et al., arXiv 2306.07962 (NEW) | Show that open-loop task metrics (detection mAP, displacement error) mis-predict closed-loop driving outcomes, and propose task-aware replacements | **RQ2.2's question, "does an open-loop metric predict closed-loop safety?", is an established genre in autonomous driving.** Direction 2 transplants it to V2X MBD. That is legitimate, but it is incremental at a security venue |
| arXiv 2606.07804 (NEW, abstract read); arXiv 2609.31217 (NEW, already in §5) | IDS and MBD trained with cost functions that weight false negatives, false positives, delay and load | Cost-weighted training already appears; still with no closed-loop safety ranking |
| carie2026hierarchical (IASC 2026, doi:10.32604/iasc.2026.088003) | Asserts that a "2% FPR means ~40 false bans per hour", uncited and with no derivation | Shows demand for false-positive-to-sanction accounting. Not a pre-emption |
| VeReMi NextGen (Hermann et al., VNC 2026, in press; https://oparu.uni-ulm.de/items/a1a874b1-c15a-4f8d-9fae-082368353aaf, NEW) | A new Ulm dataset (15 attacks, MOSAIC/InTAS, train/validation/test splits) | Its abstract shows the reference group is still building open-loop F1 datasets. That is the target for a protocol paper, but the abstract mentions no closed-loop or safety metric |
| Qualcomm MBD patents (US11722865B2, US11937087B2, US12260747B2) | Detection checks only, from abstracts | No patent found that ties detector thresholds to safety cost |

**Searches run.** OpenAlex (6 queries), Crossref, web search (6 queries, including site:arxiv.org), the GitHub/Zenodo records for BSMAttacker, both 5GAA MBD white papers in full, and the catalog.
- **arXiv API, retried.** It first returned HTTP 429, then HTTP 200, but with implausibly small totals: `ti:misbehavior` returned 47 works and `all:misbehavior AND all:V2X` returned 10. The index or rate limiter is degraded, so the empty arXiv results are **not** negative evidence. Web search restricted to site:arxiv.org partly covered the gap.
- **Not found:** a paper that ranks several V2X detectors by closed-loop net safety and compares that ranking with F1. The narrow claim survives.

---

## 2. Do the key assumptions hold?

### 2.1 FATAL to the stated headline (H5, RQ2.3): the revocation path does not act within a simulated episode, and its harm can be computed open loop

The direction's "surprising" result is that the gap between F1 and safety "would come from the infrastructure, not the detector": "wrongful-revocation harm depends on MA aggregation across vehicles, which an open-loop metric cannot see". Real US procedures contradict both halves.

**Timescale.** The project's own sourced notes (`docs/design/scms-real/03-protocols-and-operations.md` §8) record:
- the RA shuffles reports with a threshold of "10,000 reports or one day" (CAMP 1.2.2 §5.2.8.4);
- CRLs must be published within 1 business day (SCMS Manager Provider Requirements §3.4.10);
- devices fetch CRLs "no later than once a week" (CAMP 1.2.2 §3);
- the pilots used a **manual** "minimum viable" MA, in which an operator reviewed reports and added devices to the CRL (CAMP 1.2.2 §4.2–4.3; SCMS Primer pp. 6, 8).

The 5GAA white paper says a full detection-and-remediation cycle "can take a week" (5GAA, *MBD for V2X: Operational Aspects*, May 2024, §5.2.1, https://5gaa.org/content/uploads/2024/06/5gaa-wi-mbd-op-white-paper.pdf, read in full).

A wrongful revocation therefore cannot change safety within a simulation run of minutes to hours, unless time is compressed. Time compression is exactly the modelling artefact a reviewer will attack.

**Separability.** Once revoked, an honest OBE "stops transmitting for that PSID/SSP" (CAMP 1.2.2 §5.2.10.2.3). The harm is then the lost V2X benefit of one silent vehicle for the duration of the revocation. That is
- (rate of wrongful revocations, which comes from replaying per-vehicle false-positive streams through an MA policy, which is open loop, as DARE does) ×
- (marginal safety value of one vehicle's V2X per unit time, which is one paired run).

Both factors can be estimated without the closed loop. So a "revocation-aware" open-loop proxy exists by construction, and H5 is very likely false for an uninteresting reason.

**Fix.** Drop H5's mechanism claim. The one infrastructure effect that is *not* separable is **correlated false positives**:
- GNSS multipath that makes many neighbours flag the same honest vehicle;
- an OEM-specific message quirk that a detector flags fleet-wide. 5GAA's remediation discussion explicitly considers misbehaviour that "affects a particular vehicle manufacturer, year or model".

Correlated false positives defeat the independence that MA thresholds (k distinct reporters) rely on. Test that, and report wrongful-revocation harm as an accounted term with explicit timescales, not as an in-run safety effect.

### 2.2 FATAL as written, fixable with engineering: the engine has no closed-loop safety outcome yet

- **Warnings do not move vehicles.** V2X warnings are issued and labelled `true`, `false` or `missed` against ground truth (`crates/v2xw-metrics/src/apps.rs`: `app_precision`, `app_miss_ratio`, `app_lead_time_s`). The only applications that change motion are GLOSA and emergency-vehicle priority (`crates/v2xw-engine/tests/applications.rs`). A detector that discards a true BSM changes the *warning record*, not the trajectories. That makes the "closed-loop safety" score an open-loop application metric, which is one of the proxies RQ2.2 was meant to beat.
- **No emergencies arise without a script.** The calibrated drivers produce no conflicts on their own: "no driver braked harder than 3.02 m/s² in 18,052 samples" on the dense `connected-intersections` run, so EEBL, FCW and the stationary-vehicle warning "can only be exercised by an emergency that is put there" (`crates/v2xw-engine/src/safety_events.rs`).
- **So the ranking would be set by modelling choices.** It would come from (i) the scripted event mix and (ii) a driver-reaction model still to be built (§2 "What the engine still needs" already lists it). Those are exactly the choices the reviewer in §2's point 2 says decide the result.

**Fix.**
- Build application reaction (brake on FCW, EEBL or IMA, after a reaction time drawn from a published distribution, with a compliance rate). Report every ranking as a function of compliance and reaction time.
- Use surrogate safety measures such as TTC and post-encroachment time, with stated thresholds, as well as collisions.
- Draw scripted events from a declared distribution, not chosen by hand.

Without these, Direction 2 has no dependent variable.

### 2.3 Major: "F1 is how detectors are chosen" is true only of academic leaderboards

- **Deployed checks.** The US CV-pilot report specification defines coarse, precision-first checks: MaxSpeed > 90 m/s, MaxAcceleration > 10 m/s², and ObsImplausible "default off because of relay attacks" (SCMS Manager MBR spec v1.0 §2.3, via `03-protocols-and-operations.md` §8.1). Nobody chose these by F1.
- **Industry already says "tune by consequence".** 5GAA (2022, *MBD white paper*, p. 12, https://5gaa.org/content/uploads/2022/07/5GAA-Misbehaviour-detection-Final.pdf, read in full): "MBD algorithms can be tuned to balance the overall impact of false positives and false negatives when the consequences are carefully considered in each case." Neither 5GAA paper uses precision, recall or F1 anywhere (full-text search).
- **But industry calls quantification infeasible.** 5GAA (2024, §5.2.1): "The intent is not to do a full cost-benefit analysis, which may be resource-intensive and unfeasible, but to use expert judgment and prior experience." Its impact scale (§6.3: very low, low = false warning, medium = unnecessary emergency brake, high = injury) is qualitative.

So the "F1 picks the wrong detector" framing attacks a strawman for practitioners. The real gap is that **the consequence-based tuning industry says it does has no quantitative instrument**. That gap is a better hook than the F1 rank flip, and it holds whether H4 holds or not.

**Fix.** Retitle the paper around "pricing detector errors". The F1 comparison becomes one result, aimed at academic leaderboards (VeReMi, VeReMi Extension, VeReMi NextGen).

### 2.4 Major: the statistics in H4 and H5 cannot be resolved as specified

The plan uses about ten detectors and Kendall's τ against a threshold of 0.9. With n = 10, the null standard deviation of τ is √(2(2n+5)/(9n(n−1))) ≈ 0.25, and the critical |τ| at α = 0.05 is about 0.51. On top of that, net safety is itself estimated from rare scripted conflicts with sampling noise.
- "Clearly below 1" and "τ ≥ 0.9" are not distinguishable at this scale.

**Fix.**
- Rank *operating points*, not detectors: detector × threshold × reaction, in the hundreds.
- Report pairwise disagreement rates with bootstrap confidence intervals over seeds.
- Pre-register the harm weights and the proxy family.

### 2.5 Minor: RQ2.4 (pairing validity) is not a contribution in this engine

- **The engine already does most of it.** It draws every number from counter-based ChaCha12 streams keyed by SHA-256(master seed ‖ domain ‖ entity), with per-frame keys for links (`crates/v2xw-core/src/rng.rs`, `crates/v2xw-core/src/lib.rs`).
- **One residual risk remains.** Long-lived per-entity streams are cached and "consecutive draws for one key continue the same keystream". So an intervention that changes how many draws one vehicle makes can still shift that vehicle's later draws. That is a real but narrow check.
- **The method is published.** Buffalo et al. (arXiv 2603.11084) and Kerr et al. (arXiv 2409.02086) already cover it.

**Fix.** Keep it as an injected-fault validity test inside the paper (it must be able to go red). Do not list it as a contribution.

### 2.6 Are the cited contradictions real?

- **wehmer2021practical** (eval-A, read in full): the description is accurate.
- **zhang2024enhancing** and **zhang2024simulation**: the descriptions are accurate, except that §2 understates zhang2024enhancing, which already shows false-positive harm (see §1).
- **wu2026bsmattacker**: accurately flagged as "under revision" (README v1.0.1).
- **The AV papers** (arXiv 2603.28029, 2606.18864) are cited in §2 as precedents. Philion 2020 and Dauner 2023 are the stronger and older precedents and should replace or join them.
- **Nothing in §2 shows that any published V2X result contradicts F1-based selection.** H4 is a prediction, not a reported contradiction, and §2 correctly lists "rank reversal" as not yet claimable.

---

## 3. Would the result change anyone's decision? Is a null publishable?

| Audience | Would the result change its decision? |
|---|---|
| MBD researchers | **Yes, conditionally.** If VeReMi-style F1 mis-ranks operating points, dataset papers (VeReMi NextGen, BSMAttacker) would add a cost-weighted or closed-loop track. The deliverable that would actually be adopted is a **published table of harm per false positive and per false negative, by attack type, application and context**. Anyone can apply that table to existing open-loop results |
| OEM and LMBD vendors, 5GAA | **Weakly.** They already tune for precision and by expert judgment (5GAA 2022 p. 12; 2024 §5.2.1). A quantitative exchange rate supports the 5GAA impact classification (§6.3) but will not overturn practice. Simulation-only numbers will be discounted against field data |
| SCMS Manager and MA operators | **Barely, through this direction.** The revocation decision is manual, on a timescale of days to a week. That belongs to Direction 1 |
| SAE and ETSI | **Unlikely directly.** J3287 and TS 103 759 standardise report formats, not detector selection. 5GAA says remediation conditions are left to policy bodies (2024, §1) |

**Null result (τ ≈ 1, or a simple cost-weighted metric recovers the ranking).**
- Publishable at VNC or VehicleSec as "a calibrated open-loop metric suffices: here are the weights". That is a useful, cheap-to-adopt result.
- Not publishable at CCS or NDSS: a simulation-only confirmation that cost-weighted metrics work is what Gaffney–Ulvila and Cárdenas predict.
- So the null is publishable only in the reframed version, which reports weights, not a τ.

---

## 4. Fatal versus fixable

| # | Issue | Class | Concrete fix |
|---|---|---|---|
| 1 | The H5 "infrastructure gap" mechanism is likely false by construction: the revocation timescale is days to weeks, and the harm can be separated and computed open loop | **Fatal to the headline** | Drop it. Replace it with correlated false positives (GNSS multipath, an OEM-specific quirk) defeating k-reporter MA thresholds. Account for revocation harm with explicit real-world timescales |
| 2 | No closed-loop safety dependent variable: warnings do not move vehicles, and conflicts exist only when scripted | **Fatal until built** | Add driver reaction (reaction time, compliance) to FCW, EEBL and IMA. Use surrogate safety measures. Draw events from a declared distribution. Report rankings as functions of compliance |
| 3 | Strawman: practice does not pick detectors by F1 | Major, fixable | Reframe as "pricing detector errors", the quantification 5GAA calls infeasible. Aim the F1 point at academic leaderboards only |
| 4 | Novelty at a security venue: cost-based IDS evaluation (S&P 2001, 2006), impact-based detectors (CCS 2016), planner-centric metrics (CVPR 2020), closed-loop false-positive harm (zhang2024enhancing), consequence-based detector deployment (sun2024act) | Major, fixable by venue and claim | Target VehicleSec or VNC, then TVT or TIFS. Claim only: many V2X operating points, exact pairs, the calibrated exchange-rate table, and correlated-false-positive revocation |
| 5 | The ten-detector Kendall's τ cannot resolve H4 or H5 | Major, fixable | Hundreds of operating points; pairwise disagreement with bootstrap confidence intervals; pre-registered harm weights and proxies |
| 6 | Sim-to-real | Major, partly fixable | Cross-check the exchange-rate table against BSMAttacker's real-data warning replay once its revision is out. Bound the factors with Direction 3's field-set ranges |
| 7 | BSMAttacker's revision may publish the real-data F1-versus-safety comparison first | Risk | Contact its authors or position this work as the closed-loop complement. Track the repository |
| 8 | RQ2.4 is mostly solved by the engine's counter-based RNG | Minor | Keep it as an injected-fault check; drop it from the contribution list |
| 9 | Overlap with Direction 1: the wrongful-revocation term and RQ2.3 are counted in both | Minor | Give revocation harm to Direction 1. Direction 2 cites it as an input |

## 5. The surviving claim, rewritten

> For V2X misbehaviour detection, we price each detector error. With exact paired reruns and modelled driver reaction, we estimate the harm of a false positive and of a false negative per attack type, application (FCW, EEBL, IMA) and context. We show whether a cost-weighted open-loop metric with these prices recovers the closed-loop ranking of several hundred detector-threshold-reaction operating points, and where it fails. The candidate failure is correlated false positives defeating the MA's reporter thresholds.

This version:
- is novel against the search above;
- gives MBD dataset maintainers a table they can adopt;
- gives 5GAA the quantitative impact estimate it says it lacks;
- yields a useful result whichever way the proxy test comes out.

Its venues are VehicleSec, VNC and WiSec; the journals are TVT and TIFS. Its dataset release fits the NeurIPS Datasets & Benchmarks track only if the operating-point corpus is large and documented.
