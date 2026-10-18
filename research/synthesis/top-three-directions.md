# The three best research directions

Date: 2026-10-17. Inputs: `eval-A-counterfactual.md`, `eval-B-revocation.md`, `eval-C-robustness-and-threshold.md`, `eval-traction.md`, `novelty-check.md`, `open-questions.md`, `claims-we-cannot-make.md`, plus a fresh round of kill searches made for this file (listed in §5). Ids resolve to `summaries/<id>.md`. Papers marked NEW are not in `catalog.jsonl`.

Criteria, in order: (1) novel after the full-text checks; (2) useful, meaning a standards body, an operator or other researchers would change what they do because of the result; (3) traction and venue fit; (4) the engine is unusually well placed to do it. Time and effort are ignored.

## 0. The decision in one table

| Rank | Direction | Built from | Novelty after the checks | First-choice venue | Biggest risk |
|---:|---|---|---|---|---|
| 1 | **Framed and revoked: the attacker's cost of getting an honest vehicle revoked in the US SCMS, and MA remediation policies that resist it** | B (N2 and Direction 1), with the multi-provider SCMS (R2) and the threshold LA (D) as optional sub-experiments | Partly novel. The idea of framing is old; following it end to end through the SCMS pipeline and its cost, privacy and safety consequences is not published | USENIX Security | The MA is not standardised, so a reviewer can call the numbers artefacts of the assumed MA |
| 2 | **Does F1 pick the safest detector? Net-safety evaluation of misbehaviour detection and response, with exact paired counterfactuals** | A (N1), narrowed; the causal-estimator half is demoted | Partly novel. Impact-based and safety-based MBD evaluation exists; ranking by net harm, including wrongful infrastructure revocation, and testing whether any open-loop metric predicts it, does not | ACM CCS | The result may be "F1 is roughly fine"; plus the sim-to-real objection |
| 3 | **Why V2X security studies disagree: a global sensitivity analysis of security conclusions, led by the safety cost of PKI and post-quantum signatures** | C (N3), with the narrowed PQ direction as its lead case | Systematic version novel; single-factor evidence published | USENIX Security (Arp et al. format) | "You measured your own engine's fragility inside ranges you chose" |

Directions considered and not in the top three:
- **PQ as a stand-alone direction.** It has the clearest practitioner demand (the unfunded "Impact of PQC on SCMS operation" research idea, NIST IR 8547). But it is the most crowded area: two USDOT TraCR projects are funded on it, and twardokus, abrar2026deployment, fikry2026post and chen2026pseudonym are all active. On criterion 1 it loses to C. Its demand is kept by making it the lead case of Direction 3, where the contribution is the regime map that explains why petho2024investigating finds a small effect and abrar2026deployment finds that Dilithium does not fit.
- **D (threshold authorities).** Partly novel and narrow after ajibuwa2023resilient. It survives as a sub-experiment in Direction 1 (a threshold LA pair whose linkage-seed release survives an outage or a t−1 compromise).
- **R2 (multi-provider SCMS).** Folded into Direction 1 as cross-provider framing, which uses the engine's newly merged multi-provider runs with shared reports and CRLs.
- **R4 (LLM root-cause analysis scored by counterfactuals).** Its novelty depends entirely on Direction 2's counterfactual machinery, and the LLM-agent field moves too fast to hold a claim.

---

## 1. Direction 1: Framed and revoked

### Pitch
Measure what it costs an attacker to get an honest vehicle revoked through the real US SCMS pipeline (J3287 report → MA → LA1/LA2 linkage-seed release → CRL → dissemination → enforcement), what the victim and its neighbours lose, and which MA remediation policies keep the time to neutralise real attackers short while making framing expensive.

### Research questions
- **RQ1.1 Attacker cost.** What is the minimum attacker resource that gets an honest vehicle onto the CRL, under each MA policy? The resources compared are: credentialed insiders; one insider's concurrently valid pseudonyms used as Sybil reporters; reactive jamming minutes that cause signature failures with no insider at all (the attack of twardokus2021targeted carried through to revocation); and reports filed through a different provider's RA or MA.
- **RQ1.2 Victim cost.** What does a wrongful revocation cost? The parts are the credential outage until re-enrolment, the lost awareness for the victim's neighbours, the safety cost of both (from paired same-seed counterfactuals), and a privacy cost. A CRL entry publishes linkage seeds, so the victim's remaining pseudonyms become linkable by any listener. Framing therefore doubles as a targeted tracking attack.
- **RQ1.3 Reporter privacy.** Stopping Sybil reports needs the MA to establish that reports come from different devices. In the SCMS that requires LA or PCA cooperation. How much reporter linkage must the MA buy to raise the framing cost by a given factor?
- **RQ1.4 Policy frontier.** Over the policy families in use or proposed (witness count, witness diversity in space and time, report-rate limits, the 5GAA graded options ST0 issuance pause, ST1 partial and ST2 full revocation, and reinstatement), what is the Pareto frontier of time to neutralise a real attacker against framing cost against safety harm? Which stage dominates time to neutralise?

### Contribution claims
**Defensible:**
1. The first execution of framing attacks through a US-SCMS-faithful revocation pipeline, measured to the CRL and to enforcement by receivers, rather than stopping at a reputation score (DRAMBR, IPEK) or a detector false positive (VehiGAN, twardokus2021targeted).
2. Attacker-cost curves per SCMS parameter: concurrent pseudonyms per device, report-rate limits, provider count. Includes a policy-independent bound, stated as a hypothesis until proved: a witness-count rule of w distinct certificates is defeated by one insider whenever w is at most the number of pseudonyms it holds concurrently, unless the MA pays for reporter linkage.
3. Wrongful revocation as a privacy attack: linkage seeds on the CRL make the victim trackable. The linkability of revoked devices is a known design property (brecht2018security). Its use as the payload of a framing attack was not found in any search (§5).
4. The first quantitative evaluation of the 5GAA remediation options, scored jointly on false revocation, time to neutralise, safety and fairness. The 5GAA operational white paper (2024) says the conditions for choosing among them are not specified.
5. A stage-by-stage decomposition of time to neutralise in the US SCMS, including the business-day service level for CRL issuance.

**Not defensible:**
- "First framing or false-accusation attack": ruled out by bimeyer2014misbehavior, Moore et al. SECON 2008 (NEW), Clulow and Moore 2006 (NEW), the 5GAA 2022 white paper (NEW), asavisanu2025cats, turgut2026ipek and park2024enhancingb.
- "First closed loop from report to MA to revocation": ruled out by kamel2020simulation (F2MD) and willecke2023a4md.
- "First witness or threshold defence": ruled out by raya2007eviction, bimeyer2012central and CATS.
- "Real MA behaviour": real MA procedures are unpublished and partly manual. Every MA result is conditional on the policy family.

### Closest prior work and the exact difference

| Work | What it does | What this direction adds |
|---|---|---|
| bimeyer2014misbehavior (PhD, 2014) | A central evaluation authority filters fake reports and resolves reporter pseudonyms through the PKI; simulated against colluders | US SCMS rather than an EU-style PKI: butterfly pseudonyms, LA-mediated linkage, linkage-seed CRLs. The reporter-privacy cost of resolution is measured, not assumed. Safety and tracking costs to the victim |
| asavisanu2025cats (TVT 2025) | A central authority bans a vehicle; defends against downvote collusion; measures time to ban | No SCMS, no stage decomposition, no safety or privacy cost of a wrongful ban, no comparison of remediation options |
| turgut2026ipek (arXiv 2026) | False-revocation rates under 10–40% colluding reporters | A generic trust model; no PKI, no Sybil reporting through pseudonyms, no safety outcome |
| twardokus2021targeted (ICC 2021) | Jamming discredits an honest vehicle within two minutes | Argues that revocation follows, but never runs it; this direction runs it |
| Moore et al. SECON 2008 (NEW), Clulow and Moore 2006 (NEW) | Count honest devices excluded by colluding accusers under LEAVE and Stinger; self-sacrificing accusation | Local eviction schemes with no central MA or CRL; useful as baseline policies to port |
| scopelliti2024efficient (NDSS 2024) | Bounded revocation time from the revocation authority's decision onward | Its clock starts after the MA decision; this direction measures the decision itself and what the decision can be tricked into |
| almani2025distributed (DRAMBR), shahriar2025vehigan | Report-accuracy reputation; adversarial false positives at the detector | Both stop before the MA and revocation |

### Key experiments and hypotheses
1. **Cost of framing per attack resource and MA policy.** City-scale runs with honest victims chosen at random, framed by k insiders, by one insider with p concurrent pseudonyms, by jamming, or across providers.
   - H1a: under any witness-count rule without reporter linkage, one insider suffices. *Decision-changing if true:* the MA must link reporters, which is a privacy cost the SCMS was designed to avoid. This would inform the SCMS Manager and the J3287 editors.
   - H1b: jamming alone (no credentials) gets an honest vehicle revoked within one CRL cycle under the simplest plausible MA rule. *Surprising if true:* revocation of an honest vehicle would need no credentials at all.
2. **Victim cost.** Paired counterfactuals: framed versus not framed, with all else equal. Report the outage length, conflicts and hard braking near the victim, and the fraction of the victim's subsequent trajectory linkable from the CRL.
   - H2: the privacy loss (tracking) outlasts the safety loss. *Surprising:* a revocation designed as a safety measure becomes mainly a privacy weapon.
3. **Policy frontier.** Sweep the policy families, and report results as ranges over them rather than point estimates.
   - H3: the ST0 issuance pause with reinstatement gets most of ST2's safety benefit against persistent insiders at a small fraction of ST2's framing harm, because time to neutralise is dominated by the MA and CRL stages and not by the remediation type. *Decision-changing:* it would give 5GAA and SCMS operators a condition for choosing a rung, which their white paper says is missing.
   - H3′ (null that is still useful): if the business-day CRL service level dominates time to neutralise, revocation contributes almost nothing to safety against transient attacks, and its value lies only in removing persistent insiders. That would tell operators to invest in local reaction rather than in a faster MA.
4. **Optional sub-experiments.**
   - Cross-provider framing: a report filed through provider A's RA against a vehicle enrolled with provider B.
   - Threshold LA pair: linkage-seed release under outage or t−1 compromise (the surviving part of D).

### Who uses the result
- **SCMS Manager and MA operators:** an evidence-based condition for revoking.
- **5GAA MBD work items:** the remediation conditions their 2024 paper asks for.
- **SAE J3287 and ETSI TS 103 759 editors:** whether reports need reporter-linkage evidence or rate limits.
- **OEMs:** the warranty and liability cost of a framed customer vehicle.
- **Researchers:** the first SCMS-faithful baseline for any MA design.

### Venues
- First choice: **USENIX Security** (attack plus defence on deployed infrastructure, anchored to standards).
- Fallbacks: NDSS; VehicleSec (co-located with USENIX Security) for a shorter version.
- Journal: **IEEE TDSC** or **IEEE TIFS**.

### Hostile review (USENIX Security programme committee) and rebuttals
1. *"False-accusation-resistant revocation is a 2006–2014 topic: LEAVE, Stinger, suicide, Bißmeyer, and CATS more recently. This is old defences in a more detailed simulator."*
   - Rebuttal: the new results are SCMS-specific mechanisms that none of those papers can express: Sybil reporting through unlinkable butterfly pseudonyms, the reporter-linkage price, linkage-seed CRLs as a tracking payload, and cross-provider reporting.
   - The old schemes become baselines that are ported and measured on the same frontier.
   - **Revision forced:** the headline must be an SCMS-specific result (H1a, H1b or H2), not "framing is possible".
2. *"Your MA is invented. Real MAs are partly manual and unpublished."*
   - Rebuttal: parameterise the MA by the 5GAA ladder, J3287 and TS 103 759. Report frontiers over policy families. Lead with the policy-independent bound (H1a), which holds for every witness-count rule. State every MA delay as a range.
3. *"Ethics and disclosure: a revocation attack on a deployed system."*
   - Rebuttal: simulation only. Disclose to the SCMS Manager and the 5GAA MBD work item before submission. USENIX requires this discussion anyway.
4. *"The safety numbers depend on your drivers and radio model."*
   - Rebuttal: report safety deltas from paired runs, across Direction 3's sensitivity envelope. The cost and privacy results (RQ1.1, RQ1.3, the CRL-linkability share in RQ1.2) do not depend on the driver model.

### What the engine still needs
Confirm each item before committing; this file was written without reading the code.
- An MA policy module with the 5GAA rungs (ST0, ST1, ST2), reinstatement, witness and diversity rules, rate limits and parameterised human or business-day delays.
- Attacker roles:
  - a Sybil reporter using its own concurrent pseudonyms;
  - a reactive jammer that targets a victim's signatures;
  - a cross-provider reporter.
- The victim's re-enrolment path, and an OBU check for its own presence on the CRL.
- A "listener" adversary that links pseudonyms from CRL linkage seeds, for the privacy metric.
- A safety-harm metric suite (time to collision, post-encroachment time, hard braking, crashes), plus paired-run tooling.

### Residual risk
- Medium. If real MAs already require multi-provider, multi-day human review, H1 holds only for automated MAs, and the paper becomes a design paper about automation.
- The 5GAA white papers and J3287 §misbehaviour management must be read in full first.
- Also read in full: park2024enhancingb (a K-City field test with slander defences), haas2012global and liu2010limits.

---

## 2. Direction 2: Does F1 pick the safest detector?

### Pitch
With exact paired counterfactuals across traffic, radio and the SCMS, rank misbehaviour detectors, their thresholds and the reaction they trigger (local discard, report, revocation) by net safety effect: harm prevented by true detections minus harm caused by false positives and wrongful revocations. Then test whether F1, or any cheaper open-loop metric, predicts that ranking.

### Research questions
- **RQ2.1** Does the net-safety ranking of published detectors (F2MD plausibility checks, VeReMi baselines, ML detectors) agree with their F1 ranking on the same traffic? Where and by how much does it disagree?
- **RQ2.2** Is there an open-loop proxy that recovers the closed-loop ranking? Candidates: application-weighted F1, cost-weighted confusion matrices, attacker gain as in wehmer2021practical, and replay-based evaluation as in BSMAttacker. If none does, closed-loop evaluation is necessary, not just nicer.
- **RQ2.3** How does the optimal threshold move once the false positives feed an MA and become revocations (the coupling to Direction 1)?
- **RQ2.4** Do same-seed comparisons stay valid when an intervention changes the event order? The test compares stateful seeding with event-keyed common random numbers (Buffalo et al., arXiv 2603.11084, NEW).

### Contribution claims
**Defensible:**
1. Net-harm decomposition per unit (message, vehicle, incident) that includes the cost of false positives and of wrongful linkage-CRL revocation, from exact paired reruns. wehmer2021practical measures only the attacker's aggregate gain, for one detector, with no false-positive harm and no PKI. zhang2024enhancing and zhang2024simulation use safety metrics with local filtering only.
2. A test of whether open-loop metrics predict closed-loop safety. This is a falsifiable question with a useful answer either way.
3. A released benchmark with credentialed insiders and counterfactual pairs.
4. A validity check of pairing under event-order changes, a methods point every same-seed V2X study inherits.

**Not defensible:**
- "First impact-based or paired MBD evaluation": wehmer2021practical (read in full).
- "First safety-metric MBD evaluation": zhang2024enhancing, zhang2024simulation.
- "First detector benchmark with counterfactual ground truth": IoMT-SecAlarmBench (arXiv 2608.26416, NEW; open loop, medical).
- "First to evaluate detectors on safety applications": wu2026bsmattacker (FCW and IMA replay, sim-to-field). It is open-loop replay with no reaction or revocation, and its results are under revision.
- "Ground truth that causal estimators lack": they already validate on simulated truth (alomar2022causalsim, bothra2022veritas, shirani2025validate). That half survives only as a stress test where vehicles interfere with each other, as a side task.
- "Rank reversal" before it is measured.

### Closest prior work and the exact difference

| Work | What it does | What this direction adds |
|---|---|---|
| wehmer2021practical (VEHITS 2021) | Same-seed attack versus no-attack runs; detector scored by attacker advantage in travel time | Many detectors and thresholds; false-positive harm; MA and revocation in the loop; safety outcome; per-unit decomposition |
| zhang2024enhancing, zhang2024simulation | Closed-loop CPM detector evaluation by intersection safety metrics | Exact paired counterfactuals; infrastructure reaction; wrongful-revocation cost; BSMs and insiders |
| wu2026bsmattacker | Nine detectors, sim-trained, tested on five real datasets with FCW and IMA replay | Closed-loop reaction (replay cannot show what a discarded message changes downstream); the MA path |
| Urbina et al., CCS 2016 (NEW) | Industrial-control detectors scored by attack impact under a false-alarm bound | The V2X multi-agent setting, where false alarms harm third parties and trigger revocation |
| IoMT-SecAlarmBench (NEW) | Per-window counterfactual clean signal for detector benchmarking | Closed loop, multi-agent, cross-layer |
| arXiv 2603.28029, 2606.18864 (NEW) | Separate the closed-loop severity of false positives and false negatives (perception, AEB) | The same principle applied to V2X MBD with PKI reaction |

### Key experiments and hypotheses
1. **Ranking agreement.** Around ten detectors × several thresholds × attack mixes, each run paired with a no-detector run and a no-attacker run.
   - H4: Kendall's τ between the F1 ranking and the net-safety ranking is clearly below 1, because high-recall detectors with moderate false-positive rates cause wrongful discards and revocations near conflicts.
   - *Decision-changing:* if true, VeReMi-style leaderboards select the wrong detectors. The MBD community and ETSI/SAE detector work need a closed-loop protocol.
   - Useful null: if τ is near 1, F1 is vindicated as a cheap proxy, and the paper becomes the first validation of the VeReMi methodology. That is less exciting, but still citable.
2. **Proxy search.** Fit each open-loop proxy to the closed-loop ranking.
   - H5: no open-loop proxy reaches τ ≥ 0.9 once revocation is in the loop, because wrongful-revocation harm depends on MA aggregation across vehicles, which an open-loop metric cannot see.
   - *Surprising:* the gap would come from the infrastructure, not the detector.
3. **Threshold shift.** The optimal threshold with local reaction only, compared with reaction through the MA.
   - H6: the optimum moves toward precision by a margin that matters operationally.
4. **Pairing validity.** Inject an intervention known to change the event order; compare stateful seeding with event-keyed random numbers.
   - H7: stateful seeding produces spurious differences in a measurable share of units. This must be an injected-fault check that can fail, not one assumed to pass.

### Who uses the result
- **MBD researchers:** a benchmark and an evaluation protocol.
- **ETSI ITS WG5 and SAE MBD committees, and detector vendors:** a way to compare detectors by safety effect, which no standard has today.
- **5GAA:** evidence for detector-efficacy requirements.
- **Causal-ML and network-twin researchers:** a hard interference-heavy testbed (secondary).

### Venues
- First choice: **ACM CCS**, where Urbina et al. is the precedent for impact-based detector evaluation.
- Fallbacks: NDSS; VehicleSec or VNC.
- Dataset release: **NeurIPS Evaluations & Datasets track**, as a separate paper.
- Journal: **IEEE TIFS**.

### Hostile review (CCS programme committee) and rebuttals
1. *"Any cost-sensitive metric reorders detectors. A rank flip is expected, not a finding."*
   - Rebuttal, and **revision forced**: the headline is RQ2.2, whether any open-loop proxy recovers the closed-loop ranking, not the flip itself. The analysis attributes the residual to specific mechanisms (MA aggregation, multi-agent propagation) using the per-unit decomposition.
2. *"Exact only for your simulator. IDM drivers and your harm proxy decide the ranking."*
   - Rebuttal: report rankings across several harm proxies and across Direction 3's field-bounded sensitivity envelope. Claim only rankings that hold across the envelope.
   - Cross-check one result against BSMAttacker's real-data replay where the two overlap.
3. *"Your vehicles do not act on V2X the way real ADAS does."*
   - Rebuttal: use standard application logic (FCW, IMA, EEBL) with published driver-reaction distributions, varied as factors.
   - Concede that fully automated reaction is out of scope.
4. *"Wehmer did paired impact evaluation in 2021."*
   - Rebuttal: cite it as the precedent. The differences are many detectors, false-positive and revocation harm, safety rather than travel time, and the proxy test.

### What the engine still needs
Confirm each item; written without reading the code.
- **Event-keyed (counter-based) random-number streams** across traffic, radio and the SCMS, plus the drift check.
- A detector library with the published baselines, reproduced against their reported F1 on VeReMi-like traces first.
- Application-layer reaction (FCW, IMA, EEBL) with a driver-reaction model.
- Per-unit harm attribution across paired runs.
- A release pipeline: data schema, licence and a leaderboard. muzammil2026code's VeReMi artefacts must be avoided.

### Residual risk
- Medium to high on the headline: H4 and H5 may fail. The null is publishable, but at a lower tier.
- BSMAttacker's revised release could take the safety-application framing. Track it.
- The engineering for the release is heavy, but effort is out of scope here.

---

## 3. Direction 3: Why V2X security studies disagree

### Pitch
A many-factor global sensitivity analysis (Morris screening, then Sobol indices) of V2X *security conclusions*, using paired reruns with every factor range bounded by field data. It identifies which modelling choices flip the sign or the ranking of published claims, and uses that to reconcile contradictory results in the literature. The lead case is the safety cost of PKI and post-quantum signatures.

### Research questions
- **RQ3.1** Which of the roughly 300 settings move, and which flip, four families of conclusions?
  - (a) The safety cost of PKI, PQ and hybrid signatures: small (petho2024investigating) or prohibitive (abrar2026deployment, yoshizawa2023post)?
  - (b) Detector rankings (Direction 2).
  - (c) Revocation and CRL dissemination latency (khodaei2020scalable reports 95% within 15 s).
  - (d) Attack visibility (bello2026ns3learn: the reception model decides whether a flood shows up at all).
- **RQ3.2** Can the disagreements between published papers be explained by the factors each paper fixed differently? The method reproduces each paper's configuration inside the engine, then moves one factor at a time from theirs to the other's.
- **RQ3.3** What is the regime map for PQ: at what density, radio stack, PQ profile and verification policy does the safety cost stop being small? This is the system-level question that the unfunded "Impact of PQC on SCMS operation" research idea asks.
- **RQ3.4** What minimum set of modelling choices must a V2X security paper report for its conclusion to be checkable? This is the reporting checklist.

### Contribution claims
**Defensible:**
1. The first many-factor global sensitivity analysis of V2X security conclusions, as opposed to network KPIs (Ajjaj 2022, NEW; Martinez 2013, NEW) or traffic-only metrics (IEEE T-ITS 2024 GSA, doi:10.1109/tits.2024.3372334, NEW).
2. Reconciling specific published contradictions by naming the factor responsible.
3. A PQ regime map with closed-loop safety on three radio stacks and the SCMS back end, which no single PQ paper offers.
4. A reporting checklist grounded in measured flips, not opinion.

**Not defensible:**
- "First to show modelling choices change security results": baharlouei2022exploring, bello2026ns3learn, Stetsko et al. SIMUTOOLS 2012 (NEW).
- "First DoE in VANETs": Ajjaj 2022, Martinez 2013.
- "First credibility audit": Kurkowski 2005 (NEW), Joerer, Sommer and Dressler 2012 (NEW), pawlikowski2002credibility.
- "First PQ V2X safety study": twardokus2024cryptography, abrar2026deployment, fikry2026post, petho2024investigating.
- "The engine's answer is the right one": the analysis can show that a conclusion is fragile, not which value is true, except where field data pin the factor.

### Closest prior work and the exact difference

| Work | What it does | What this direction adds |
|---|---|---|
| bello2026ns3learn | One factor (the NR reception model) decides whether a flood is visible and doubles hard braking | Hundreds of factors screened; several conclusion families; PKI in the loop |
| baharlouei2022exploring | A few settings change DDoS-detection results | Variance decomposition; ranking flips; field-bounded ranges |
| Arp et al., USENIX Security 2022 (NEW) | Pitfalls of ML in security, with re-evaluation of published work | The V2X simulation analogue, with measured rather than argued pitfalls |
| Borg et al. ICST 2021 (NEW); Stocco et al. TSE (NEW) | ADAS test verdicts differ across simulators | Security conclusions; factors inside one deterministic engine, so differences are attributable |
| petho2024investigating versus abrar2026deployment | Contradictory conclusions on PKI and PQ cost | Explains the contradiction by factor |
| T-ITS 2024 traffic GSA (NEW) | Sobol analysis of traffic-microsimulation outputs | Radio, PKI and security outcomes |

### Key experiments and hypotheses
1. **Field-bounded factor registry.** Ranges come from:
   - ylmaz2025simulated (USDOT LTE-V2X packet error rate);
   - solida2026demystifying (MASA living lab versus Veins);
   - bauder2023cooperative (real CAM and pseudonym behaviour);
   - song2024c (NIST 1609.2 captures);
   - the standards' own values.
   Factors without an anchor are flagged and reported separately.
2. **Reproduce first.** The engine must reproduce at least one published single-factor flip (bello2026's flood visibility) and one published magnitude (khodaei2020scalable's dissemination time) before any new claim.
3. **Morris screening with group screening**, then Sobol analysis on the factors that survive, for each conclusion family. Paired reruns reduce variance.
   - H8: fewer than about ten factors account for most of the variance in every conclusion, and those factors are mostly radio and density factors, not security factors.
   - *Decision-changing:* reviewers and authors would get a short list of factors to report.
4. **PQ regime map.**
   - H9: the safety cost of PQ is negligible below a density threshold on DSRC and LTE-V2X, and stops being negligible on NR Mode 2 under high load with Dilithium-sized chains and P2PCD. The boundary moves more with the verification policy than with the signature scheme.
   - *Decision-changing:* it would tell the IEEE 1609.2 editors and OEMs that the verification policy, not the scheme choice, is the migration lever.
5. **Contradiction reconciliation.** For each pair of contradicting papers, show the one-factor path between their configurations.
   - H10: at least one well-known pair flips on a factor neither paper reports. *Surprising*, and the main traction driver.

### Who uses the result
- **Every V2X security author and reviewer:** the reporting checklist.
- **USDOT, ITS JPO and TraCR testbeds:** a validity argument and a sense of which field data matter most.
- **IEEE 1609.2 and ETSI QSC-migration editors:** the PQ regime map.
- **The two funded TraCR PQ projects:** a system-level complement to their device-level results.

### Venues
- First choice: **USENIX Security** (Arp et al. format), or an **IEEE S&P SoK** framing.
- Fallbacks: VNC; WSC or SIGSIM PADS for the methods part.
- Journal: **ACM TOMACS** for the methods, or **IEEE TVT** for the PQ regime map on its own.

### Hostile review (USENIX Security, or a WSC methods reviewer) and rebuttals
1. *"You measure the fragility of your own model family inside ranges you chose."*
   - Rebuttal: bound every range by field data or standards. Count only flips inside the field-plausible region. Reproduce published flips and magnitudes first.
   - **Revision forced:** the deliverable must include re-tests of other authors' published conclusions, not only the engine's own sensitivity table.
2. *"Sobol indices depend on the ranges and on the distributions over them."*
   - Rebuttal: report sign and ranking flips (robust to the distribution within a range) as the primary result, with Sobol indices secondary. Run a range-widening check.
3. *"Reproducing another paper's configuration in your engine isn't reproducing their paper."*
   - Rebuttal: agreed. The claim is that the conclusion is sensitive to factor X across implementations, supported by reproducing their headline number first. Where that reproduction fails, report the failure.
4. *"The cost: 3,000 city-scale runs per scenario."*
   - Rebuttal: effort is out of scope here, but use group screening and corridor scenarios, validated against city runs, to answer it.

### What the engine still needs
Confirm each item; written without reading the code.
- A machine-readable factor registry, with ranges and anchor sources, over the roughly 300 settings.
- A Morris and Sobol harness, for example with SALib run outside the engine.
- Configuration presets reproducing the published setups (petho, abrar, khodaei, bello).
- PQ and hybrid profiles wired through provisioning, P2PCD, CRLs and the air interface on all three stacks.
- Verification-policy factors.

### Residual risk
- Medium. If H8 holds and H10 fails, the result is a solid methods paper (VNC, WSC or TOMACS) rather than a USENIX paper.
- wu2026bsmattacker's revision could take the detector-ranking flip family. The PQ and revocation families do not depend on it.
- Read ETSI TR 103 949 and TR 103 971-1 before claiming the PQ gap.

---

## 4. How the three form one thesis

**Working title:** *Judging V2X security by its consequences: framing, detection and the validity of simulated evidence.*

- **The common instrument.** Exact paired counterfactual reruns across traffic, radio and the SCMS, with event-keyed random numbers. Direction 2 builds and validates the instrument (RQ2.4), and Directions 1 and 3 use it.
- **The common question.** What a security mechanism does to safety and privacy once it acts, rather than how accurately it classifies.
  - Direction 2 asks this of detectors.
  - Direction 1 asks it of the MA and revocation, including how the mechanism can be turned against honest vehicles.
  - Direction 3 asks how much of either answer is an artefact of the modelling.
- **The dependencies.**
  - Direction 1's wrongful-revocation harm is a term in Direction 2's net-safety score.
  - Direction 2's threshold shift (RQ2.3) depends on Direction 1's MA policy.
  - Direction 3 supplies the robustness envelope that rebuts the main reviewer objection to both, and carries the PQ regime map, the strongest industry-demand signal found.
- **Suggested order.**
  1. Direction 2's pairing-validity check and detector reproductions (needed by everything).
  2. Direction 1, the highest-traction paper.
  3. Direction 2's ranking study.
  4. Direction 3 as the closing methods and SoK paper.

## 5. Kill searches run for this file (2026-10-17)

The arXiv API returned empty for every query in this session, so it gave no data, which is not a negative result. OpenAlex, web search and the VehicleSec 2026 technical-sessions page did run.

| Query (paraphrased) | Source | Result |
|---|---|---|
| False misbehaviour reports leading to revocation of honest vehicles; slander; Sybil reporting; framing plus revocation | OpenAlex, web | Only DRAMBR, scopelliti2024efficient, twardokus2021targeted, park2024enhancingb, PriMiRe (all in the catalog). No end-to-end SCMS framing study |
| False reports used to de-anonymise or track a victim through the MA investigation or CRL linkage | web | Not found. Nearest: whitefield2017formal (revocation versus linkability, in the catalog) and V-TRACE (JIPS, NEW: defensive persistent tracking after a flag) |
| Evaluations of MA remediation (partial revocation, suspension, reinstatement) | web | Only the 5GAA white papers, ETSI TS 103 759 and the C2C-CC MBD white paper (NEW: C2CCC_WP_2092), which describe and do not evaluate |
| MBD evaluated beyond F1 by safety effect, counterfactual or revocation-aware | OpenAlex, web | Nothing new beyond eval-A. A DRL MBD paper (arXiv 2609.31217, NEW) weights false negatives above false positives in its reward but still reports F1 |
| SoK or re-evaluation of V2X MBD simulations; Sobol, Morris or sensitivity analysis on V2X security | OpenAlex, web | Only an FL-IDS SoK (arXiv 2607.10914, in the catalog), which covers a different question. No security-conclusion GSA |
| VehicleSec 2026 programme | usenix.org | MIRAGE (EEBL attack detection, NEW), CARdea (in the catalog), "WIP: A Benchmark for Security Evaluation of Adversarial Manoeuvre Attacks in Autonomous Driving" (NEW: a benchmark, but for autonomous-driving manoeuvres, not V2X MBD or revocation). Nothing on SCMS, revocation or sensitivity |
| PQ SCMS system-level provisioning or CRL simulation 2026 | web | Only qSCMS, chen2026pseudonym (both in the catalog) and arXiv 2601.11104 (NEW: general PQ-PKI strategy, which says PQ CRL size is unlikely to be a problem; no SCMS) |

**Must read in full before any claim above is stated as fact:**
- 5GAA MBD white papers (2022, 2024), SAE J3287, ETSI TS 103 759 v2.2.1.
- park2024enhancingb, haas2012global, liu2010limits, willecke2023a4md.
- wu2026bsmattacker (revised release), ETSI TR 103 949 and TR 103 971-1.
- brecht2018security §revocation, to confirm exactly how CRL linkage seeds make a revoked device trackable and for how long.
