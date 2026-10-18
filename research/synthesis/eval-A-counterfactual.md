# Evaluation of candidate A: counterfactual V2X benchmark

Date: 2026-10-17. Sceptical re-check of the "novel" verdicts that `novelty-check.md` gives to Direction 3, N1 and N4. Ids resolve to `summaries/<id>.md`. Papers marked NEW are not in `catalog.jsonl`.

**Bottom line: partly novel. Both flagship claims are narrower than `novelty-check.md` states.**
- **(i) Interventional detector evaluation** is partly novel. Same-scenario paired impact evaluation of misbehaviour detection (MBD) already exists (wehmer2021practical, read in full here). So does closed-loop safety-metric evaluation of MBD (zhang2024enhancing).
- **(ii) Validating causal estimators and twins against exact truth** is partly novel and weak. Every method named already validates itself against simulator or emulator ground truth. Only the V2X testbed is new.

## What was read

| Paper | How it was read |
|---|---|
| wehmer2021practical | Full text, fetched from SCITEPRESS. It was abstract-only in the corpus. |
| heijden2018veremi, zhang2026driving, alomar2022causalsim, bothra2022veritas, hou2024if, qchohi2026confounding, agudelo2026trustworthy, shirani2025validate, malik2025security, kamel2020simulation, zhang2024enhancing | Targeted sections of the local PDFs |
| laudy2026digital | Full arXiv HTML, queried for specific points |
| willecke2023a4md, wehmer2023fast, muzammil2026code | Summaries only (no PDF). They remain unverified. |

Sources searched: arXiv API, OpenAlex, web search. Semantic Scholar returned HTTP 429 on every query.

## Closest works and what they already do

### Claim (i): rank detectors by the harm they prevent, net of false-positive harm

**wehmer2021practical (VEHITS 2021), the strongest prior.** It ran a closed loop in which detection feeds vehicle routing. It used deterministic, seed-controlled runs. For each seed it compared the attacker's travel time with the same scenario without the attack ("measure t_a,fair under the exact same conditions but without the attack"), giving the attacker's advantage q_a.
- This is a same-seed paired counterfactual impact metric for MBD. It explicitly replaces confusion-matrix metrics.
- Limits:
  - selfish attackers and travel time only;
  - one detector (Petit et al.);
  - no harm from false positives to benign vehicles;
  - no revocation or PKI;
  - no safety outcome;
  - no ranking of several detectors or thresholds.

**zhang2024enhancing (VTC 2024-Fall) and zhang2024simulation (IV 2024).** Both use Artery and CARLA in closed loop with an intersection collision-risk application (IMA). They "assess the performance of misbehavior detection through its ability to overcome the impact of several types of attacks", using safety metrics instead of FPR or FNR.
- So safety-outcome MBD evaluation already exists.
- Limits: collective perception messages (CPM) only; local filtering; no exact per-unit counterfactuals; no wrongful-revocation cost.

**Other prior work:**
- **heijden2018veremi:** says intrusion-response evaluation "must be part of the system execution". Its own dataset is non-interactive.
- **wehmer2023fast:** a causal design for field studies, proposed but not executed (from the summary).
- **kamel2020simulation (F2MD):** has detection, reporting to the misbehaviour authority (MA), and revocation, but no evaluation of impact on applications.
- **malik2025security:** a repeatable security benchmark with safety outcomes. It ranks CACC controllers, not detectors.
- **abdelmaguid2026beyond (TVT 2026, abstract only):** "attack endgame" mitigation reduces hazardous events by over 40%.
- **muzammil2026code:** shows VeReMi has structural artefacts.

### Claim (ii): measure the counterfactual error of causal estimators and twins against exact truth

**The named methods already check themselves against known truth:**
- **alomar2022causalsim:** a synthetic load-balancing environment with known counterfactuals, plus held-out arms of a randomised controlled trial (RCT).
- **bothra2022veritas:** an emulation testbed with ground-truth bandwidth traces.
- **hou2024if and qchohi2026confounding:** simulated OFDM scheduling and backlog with ground truth.
- **agudelo2026trustworthy:** "ground-truth matched pairs".

**Generic simulated-truth benchmarks also exist:**
- **shirani2025validate:** six simulated environments "with known ground truth values" for causal estimators under network interference.
- **thumm2026dotime:** a synthetic counterfactual benchmark generator.
- **zhang2026driving:** a matched factual and counterfactual driving benchmark.

**The concept is formalised.** laudy2026digital (DTCF) is purely methodological, with no experiments. It already defines potential outcomes coupled by "the same noise draw" and names "fix the random seed across calls". ma2026twin does noise-coupled branching in learned world models.

## New papers found (not in the catalog)

| Paper | Why it matters |
|---|---|
| Urbina et al., "Limiting the Impact of Stealthy Attacks on Industrial Control Systems", ACM CCS 2016. https://users.soe.ucsc.edu/~alacarde/papers/ccs16.pdf | Evaluates detectors by attack impact at a fixed false-alarm bound, not by detection rate. It is the cyber-physical systems precedent for (i). |
| Ugwuabonyi and Perkins, "IoMT-SecAlarmBench: A Counterfactual Benchmark for Integrity Attacks in IoMT", arXiv 2608.26416, 2026 (submitted to IEEE BigData 2026) | Gives detector benchmarks with per-window counterfactual ground truth ("the clean signal that would have been observed"). It is open loop and covers medical IoT, not V2X. |
| Buffalo, Pearson and Klein, "Realizing Common Random Numbers: Event-Keyed Hashing for Causally Valid Stochastic Models", arXiv 2603.11084, 2026 | Shows that reusing a seed with a stateful random-number generator gives invalid counterfactuals once an intervention changes the control flow. Event-keyed, counter-based generators are needed. This bears directly on "same seed, one change". |
| Kerr et al., "Noise-free comparison of stochastic agent-based simulations using common random numbers", arXiv 2409.02086 | Common random numbers (CRN) for counterfactual agent-based model runs. It is prior art for the paired design. |
| "Effort-Based Criticality Metrics for Evaluating 3D Perception Errors in Autonomous Driving", arXiv 2603.28029 | Separates the closed-loop consequences of false positives and false negatives. |
| "Scaling Learning-based AEB with Massive Unlabeled Data", arXiv 2606.18864 | Scores collision mitigation and false-activation severity separately, in replay. |
| Neal et al., "RealCause: Realistic Causal Inference Benchmarking", arXiv 2011.15007 | Benchmarks causal estimators against known truth. |
| "Benchmarking Counterfactual Prediction in Epidemic Time Series with Time-Varying Interventions", ACM KDD 2026 | A simulation-based counterfactual benchmark in another domain. |
| "Should I Have Expressed a Different Intent?", arXiv 2601.20090 | An ns-3 "real" environment versus an ns-3 twin for counterfactuals. |

## The exact claim that survives

> A V2X benchmark with credentialed insiders, in which every message, vehicle and incident carries its exact paired outcome under single interventions on detector, threshold and MA/revocation policy. Pairing uses event-keyed common random numbers, and outcomes propagate jointly through traffic, radio and the SCMS. With it, detectors and thresholds are ranked by net safety effect: harm prevented by true detections minus harm from false positives and wrongful linkage-CRL revocations. The benchmark shows when and how much that ranking departs from the F1 ranking.

The new elements:
1. **Net harm per unit.** It decomposes harm from true positives against harm from false positives and wrongful revocations, unit by unit. wehmer2021practical only measures the attacker's aggregate gain.
2. **Infrastructure reaction (MA and CRL) in the loop.** zhang2024enhancing and wehmer2021practical only have local reaction.
3. **An empirical rank reversal against F1** across many detectors. Nobody has shown this, but it is a result still to be obtained, not yet a contribution.

Claim (ii) survives only as an application: a hard, multi-agent, interference-heavy V2X testbed in which CausalSim, Veritas and the conformal KPI estimators break their assumptions (exogenous traces, no interference). It is not "the ground truth they lack".

## Verdict

| Part | Verdict |
|---|---|
| (i) Interventional detector evaluation | Partly novel (downgraded from "novel") |
| (ii) Validating estimators and twins | Partly novel, weak (downgraded from "novel") |
| Benchmark as a whole | Partly novel. Defensible for VNC, VehicleSec or WiSec if (i) is the headline and the rank-reversal result materialises. |

**Corrections owed to the other synthesis files:**
- `claims-we-cannot-make.md` should add these rows:
  - "first same-scenario paired impact evaluation of MBD" (wehmer2021practical, now read in full);
  - "first safety-metric MBD evaluation" (zhang2024enhancing);
  - "first detector benchmark with counterfactual ground truth" (IoMT-SecAlarmBench).
- The N4 sentence in `novelty-check.md` ("their authors name the lack of such ground truth") must be qualified. Those authors lack *real-world* truth; they already use simulated truth.

## Strongest objection a reviewer would raise

"Exact counterfactuals from your own simulator are exact only with respect to your simulator's model. The net-harm ranking of detectors is then a property of your IDM/MOBIL drivers, your radio abstraction and your harm proxy, not of the real world. That is the sim-to-real gap that CausalSim and the twin papers exist to address. Wehmer and Baumgart (2021) already made paired same-scenario impact evaluation of MBD, and Zhang et al. (2024) safety-based MBD evaluation. Show that your ranking is robust to modelling choices, and that your same-seed pairing stays valid when an intervention changes the event order."

Mitigations:
- Pair the work with N3 (robustness of security conclusions to modelling choices).
- Use event-keyed common random numbers and report a check that pairing cannot drift.
- Report the rank reversal across several harm proxies and settings.
