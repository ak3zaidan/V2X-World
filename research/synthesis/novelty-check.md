# Novelty check of the candidate directions

Method:
- Each direction in `PROTOCOL.md` was checked against all 192 strong-overlap summaries (read in full) and the strong-overlap sections of all 30 batch notes.
- It was also checked against targeted keyword searches of the 11,315-entry catalog, one per direction (about 18 queries; hit counts are given below), and against the condensed summaries of the 135 most relevant partial-overlap papers those searches surfaced.
- Verdicts:
  - **novel**: no paper found that does it.
  - **partly novel**: the idea or parts of it exist, and a specific, defensible gap remains.
  - **already done**: the core claim is published.
- "Basis" warnings flag closest papers whose summary rests on an abstract or title only. These must be read in full before the thesis states a verdict as fact.

---

## Direction 1. Closed-loop misbehaviour, revocation and safety ("time to neutralise", MA decision design)

Search hits: revocation together with safety or timing terms, 238; MA and reporting, 30; detection-to-reaction with safety, 12.

**Closest papers and what they already do**

| Id | What it already does | Basis |
|---|---|---|
| raya2007eviction | Concept: local detection and containment, then CA eviction (2007) | abstract |
| hasrouny2019misbehavior | MDS → MA → periodic geographic CRL framework, simulated | abstract |
| willecke2023a4md | A4MD: detection, reporting **and reaction** simulated in ETSI C-ITS (Artery + F2MD) | abstract |
| kamel2020simulation | F2MD: in-simulation attacks, detectors and misbehaviour reports; future work names "an intelligent MA" and "a PKI module" | full text |
| heijden2017analyzing | Attack → crash quantification for CACC; explicitly calls for "effective detection and revocation" | full text |
| scopelliti2024efficient | Bounded revocation time (self-revocation, Tamarin-verified) vs passive CRL under network delays | full text |
| tesei2023transparent | Revocation vulnerability window below 1 s on a DLT scheme | full text |
| khodaei2018efficient, khodaei2020scalable | City-scale CRL dissemination (95% of vehicles within 15 s); "vulnerability window" named as future work | full text |
| haas2011efficient, eckhoff2013smartrevoc, robertndsegmented | CRL dissemination speed (V2V epidemic, parked vehicles, segmented viral distribution for the SCMS) | abstract |
| haas2012global | "Global revocation for the intersection collision warning application": revocation tied to a safety application | **title only** |
| lopes2025novel, wasef2011managing, didouh2022blockchain, imran2026secure | Active / local / decentralised revocation that shrinks the exposure window | abstract / full text |
| boeira2018mitigating, wolf2020securing, franceschini2024neural, ghiro2026physics, zhang2024simulation, trkulja2025vehicle, hasan2026vehicle | Detection → **local** reaction → safety outcome (collisions avoided), in the loop | mixed |
| bimeyer2012central, kamel2019misbehavior, haidar2020dare, willecke2024primire, carie2026hierarchical | MA-side report evaluation, witness thresholds, ML at the MA, a privacy-preserving aggregation and a game-theoretic "conviction gate" | abstract / full text |
| ganan2013certificate, ganan2012rar, liu2010limits | Probability of accepting a revoked certificate as a function of CRL age; risk-aware revocation; "limits on revocation" | full text / **title only** |
| kalogiannis2022attack | Insider (credentialed) attacks and MBD in a platoon co-simulation with a VPKI context | abstract |

**Already done**
- The eviction concept.
- CRL dissemination latency at city scale.
- Bounded revocation time.
- Local detection-to-reaction loops with safety outcomes.
- An ETSI-side detection–reporting–reaction simulation framework.
- MA report datasets and ML.

**Still open (no paper found)**
1. An end-to-end decomposition of time to neutralise in the US SCMS. The stages are: detection → report (J3287) upload → MA investigation and decision → LA1/LA2 linkage-seed release → CRL issuance → RSU/V2V dissemination → receiver enforcement. It must be measured in one run, with the share contributed by each stage.
2. That time mapped to **traffic-safety outcomes**, with exact paired counterfactuals: the same seed with no attacker, the same seed with no revocation, and the same seed with instant revocation. This gives "harm accrued during the window" and "harm prevented by revocation" per incident.
3. **MA decision design**: evidence thresholds, witness counts, insider vs outsider attackers. These need to be evaluated jointly for false-revocation rate, time to neutralise and safety.
4. A comparison of linkage-CRL revocation, ACPC activation codes (simplicio2019acpc, wantoro2022efficient) and certificate expiry on the same metric. wantoro2022efficient says "determine optimal settings via simulations".

**Verdict: partly novel.** The loop concept is not new, so the thesis cannot claim "first closed loop". The quantified, stage-decomposed, US-SCMS-faithful time to neutralise with counterfactual safety attribution is open. The thesis must read haas2012global, willecke2023a4md, liu2010limits and abdo2024vehicular in full before finalising this verdict.

---

## Direction 2. The safety cost of security (PQ migration of the SCMS, verification policies, detection thresholds)

Search hits: PQ in a V2X context, 304; verification policy, 12; detection threshold with safety terms, 72.

**Closest papers and what they already do**

| Id | What it already does |
|---|---|
| papadimitratos2008impact, petit2009analysis, petit2010analysis, bittl2015security | Security overhead → reliability and safety-application requirements (2008–2015) |
| baee2019broadcast | 1609.2 verification delay in dense highway traffic; argues a crash is possible from authentication delay |
| petho2022considering, petho2024investigating, petho2024quantifying | PKI overhead → safety risk. **Finding: the PKI's impact is small (at most about +0.11 risk) and negligible near 100% PDR** |
| feiri2012evaluation, feiri2012congestion, krishnan2011verify, ko2021avod, pollicino2021hardware, jha2018pseudonym, jin2025accountable | Certificate omission, verify-on-demand, selective verification, verification prioritisation under load and DoS |
| twardokus2024cryptography (NDSS) | Hybrid ECDSA+PQ V2V, learning-based certificate scheduling (93% of certificate transmissions redundant), spectrum fit, SUMO/Veins + SDR |
| twardokus2025quantum (CHASM), twardokus2024demo | Hybrid PQ over C-V2X with certificate fragmentation and adaptive modulation; PQ 1609.2 HIL testbed |
| abrar2026deployment | Full-stack C-V2X Mode 4: Falcon-512 vs ECDSA; Dilithium-2/SPHINCS+ exceed transport blocks; PDR/latency vs level of service |
| yoshizawa2023post, kim2023signature, yoshizawa2026zero | PQ certificates break the 1609.2 packet budget; signature splitting; ZK compression |
| barreto2018qscms, oliveira2019qscms, chen2024hybrid, chen2026pseudonym, chen2024pqcmc, bindel2022need, lonc2025hybrid | PQ / hybrid SCMS designs (qSCMS, hybrid certificates, PQ implicit certificates; lattice implicit certificates fail) |
| fikry2026post | PQ-aware SCMS with selective Dilithium; **DSRC capacity analysed analytically at 50 vehicles** |
| samara2025post | PQ token alternative vs pseudonym PKI on PDR, CBR, revocation overhead (NS-3 + SUMO) |
| wang2026reproducibility | PQ signature profiles → required vehicle spacing (viability theory; companion paper not in corpus) |
| dongre2025adaptive, sengupta2026adaptive | PQ-sized packets vs NR sidelink numerology; adaptive PQ selection under NR-V2X latency |
| karinsalo2026migrating, lohmiller2025survey, chouhan2026quantum | Analytical: direct PQ replacement is infeasible; they **call for system-level validation under realistic traffic** |
| jemaa2017study, emara2017safety, xin2019balance, wolf2020securing | Safety cost of privacy mechanisms and of MBD reaction (safety-efficiency trade-off) |
| bello2026ns3learn | Radio-model realism alone doubles predicted hard-braking events |

**Already done**
- The general framing "security overhead affects safety", which dates from 2008.
- PQ size and fit analyses.
- Hybrid PQ V2V protocols.
- PDR and latency of PQ over LTE-V2X Mode 4.
- Analytical PQ-SCMS capacity.
- Verification policies under load.

**Still open**
1. **Whole-system PQ migration of the SCMS**: PQ/hybrid butterfly-equivalent provisioning over cellular TCP/TLS, top-up timing and failure, CRL and certificate-chain sizes, P2PCD with PQ chains and RSU relay. It must be measured alongside the air interface on DSRC, LTE-V2X and NR-V2X, with **closed-loop traffic-safety outcomes** (conflicts, hard braking, crashes) rather than PDR thresholds. karinsalo2026migrating and abrar2026deployment name this gap as future work.
2. Verification policy × detection threshold × PQ profile as **joint** knobs, each evaluated by paired counterfactual safety cost. Prior work varies one knob at a time.
3. Reconciling the result with petho2024investigating. If the safety cost is small, say so. The contribution is then the regime map: where it stops being small, for example at high density, on NR with PQ, or under a verification-flooding attacker.

**Verdict: partly novel.** The thesis must not claim the framing or "first PQ V2X safety study". The whole-system PQ SCMS with closed-loop safety and three radio stacks is open, but the space is crowded and fast-moving (twardokus, abrar, fikry, chen and others, 2024–2026).

---

## Direction 3. Counterfactual V2X: a benchmark with per-unit counterfactual ground truth, plus a learned world model or surrogate

Search hits: counterfactual or causal with network or simulation, 185; world model, surrogate or digital twin with a network context, 972; V2X benchmark or dataset with attack labels, 635.

**Closest papers and what they already do**

| Id | What it already does |
|---|---|
| zhang2026driving | **Controlled simulation benchmark with matched factual and counterfactual outcomes for driving world models** (video). Limitation: short horizons in which the ego action does not change other agents |
| laudy2026digital | Digital Twin Counterfactual Framework: simulate potential outcomes with a twin under a fidelity-validation hierarchy (domain-general) |
| ma2026twin | Noise-coupled factual/counterfactual twin rollouts in learned video world models |
| alomar2022causalsim, bothra2022veritas | Causal trace-driven network simulation and counterfactual queries (ABR video); CausalSim needs an RCT |
| hou2024if, qchohi2026confounding, agudelo2026trustworthy, ak2024if | Counterfactual KPI estimation and what-if validation for wireless and RAN digital twins |
| cornish2023causal, shirani2025validate, thumm2026dotime | Falsifying twins, validating counterfactual estimators, synthetic counterfactual benchmark generators (generic) |
| wehmer2023fast | Proposes (does not execute) a causal design for field evaluation of MBD |
| schofield2025beyond | Causal sensitivity evaluation of driving world models (WOSAC) |
| heijden2018veremi, kamel2020veremi, hermann2026veremi, yuce2025state, novo2026spade, guven2025creating, abdelhakeem2025pq, wu2026bsmattacker | Simulation-generated labelled V2X misbehaviour datasets (seeded generators, PQ dataset, CPM, SPaT) |
| muzammil2026code | Audit: VeReMi benchmarks contain structural artefacts |
| malik2025security | Repeatable simulation-based security benchmark for CACC with safety outcomes |
| anjum2026mint, stenhammar2026advancing | Joint mobility + network V2X datasets with synchronised ground truth (no security) |
| chai2025mobiworld, bouton2026mobile, alagharu2026network, matsumoto2026neural, ranjan2026enactor, RouteNet family | Wireless or network world models, neural surrogates of V2X communication simulation, traffic-simulator surrogate world models |
| cirne2018loop, baharlouei2022exploring | Deterministic trace replay to compare security variants; simulator settings change security conclusions |

**Already done**
- Counterfactual simulation benchmarks in other domains (driving video) and the general idea of twin-simulated counterfactuals.
- Counterfactual estimation for wireless KPIs.
- Labelled V2X attack datasets.
- Wireless world models and V2X communication surrogates.

**Still open (no paper found)**
1. A V2X benchmark in which every unit (message, vehicle, incident) carries a factual outcome **and its exact counterfactual** under a single intervention: attacker present or absent, revocation on or off, PQ vs ECDSA, detector A vs B. The outcome must hold across traffic, radio and PKI layers jointly, with long-horizon multi-agent propagation. That propagation is the limitation zhang2026driving states.
2. Using those exact counterfactuals as **ground truth to validate causal estimators and digital twins** (CausalSim, Veritas, conformal counterfactual KPI methods, DTCF). Their authors name the lack of such ground truth (cornish2023causal, alomar2022causalsim, hou2024if).
3. A learned world model or surrogate of a V2X system that includes **security state** (credentials, revocation status, detector outputs), evaluated on counterfactual accuracy rather than factual fit.
4. Paired same-seed designs (common random numbers) for V2X security studies. The catalog search for "common random numbers / same seed / paired design" in V2X returned **0** papers.

> **(corrected after critique)** "Exact" paired counterfactuals are overstated. The engine keys each random stream by entity, not by event (`crates/v2xw-core/src/rng.rs:1-58`). Actor and node ids follow realised insertion order (`v2xw-mobility/src/engine.rs:3147-3150`, `v2xw-engine/src/run.rs:3168`), and several draw counts depend on simulated state. Pairs are exact per unit only for open-loop replay, backend-only changes (once report streams are re-keyed) and the near field of a closed-loop discard. Everywhere else they are coupled, distribution-level comparisons over seeds. FCW/EEBL/IMA warnings also do not change driving yet (`run.rs:4324-4333`). See `critique-methods.md` and `final-three-approaches.md` §5.

**Verdict: novel** as a V2X, cross-layer, exact-counterfactual benchmark. Partly novel for the world-model half, because network and driving world models exist. The thesis must cite zhang2026driving, laudy2026digital and ma2026twin as conceptual precedents and avoid claiming "the first counterfactual benchmark" in general.

---

## Runners-up

### R1. Cross-layer linkability
Closest:
- liu2018cross (MAC + pseudonym linking), liu2020marp, liu2018mac.
- bloessl2015scrambler (PHY scrambler seed), baldini2017analysis (RF fingerprinting), vaas2018poster (PHY through encrypted mix-zones).
- yieh2019multi and zoccoli2025radar (multi-radio, Wi-Fi probe linkage).
- huang2025privacy and huang2025interleaved (PCA-id issuer linking in a multi-PCA VPKI).
- escher2021well (static fields defeat the C-ITS pseudonym scheme), bittl2017mutual (certificate digest vs pseudonym change), miao2018vehicle and zhang2020novel (kinematic linking).

Already done: every individual cross-layer channel the runner-up named, at the PHY, MAC, multi-radio, issuer and static-field levels.

Open: only a **joint** multi-signal linker (SPS reservation persistence across pseudonym change on LTE/NR, plus issuer/PCA, plus certificate-digest timing, plus PHY) with exact ground truth at city scale. SPS-reservation linking on NR Mode 2 was not found as a stand-alone paper.

**Verdict: mostly already done. A narrow, partly novel remainder.**

### R2. Multi-provider SCMS interoperability
Closest:
- chen2024performance (implemented SCMS vs CCMS), chen2026comparison (IEEE / ETSI / YD/T 3957, RCA on the SCMS Manager test CTL and EU ECTL).
- jha2018pseudonym (SCMS cross-certification proposed).
- huang2025privacy (multi-PCA linkability), xia2026esga (multi-domain linkage-value CRL sync), khodaei2017secmace (multi-domain VPKI).
- yoshizawa2022misbehaviour (cross-authority reporting "broken"), garcia2012organizational (USDOT CME models), henry2020pseudonym (SCMS vs CCMS issuance), sarker2021blockchain and byun2025blockchain (elector and policy alternatives).

Open: the quantitative behaviour of a US SCMS with several competing RA/PCA/MA providers under one SCMS Manager. This covers CTL/CRL propagation delays, cross-provider misbehaviour investigation and revocation latency, provider-induced linkability and provider outage. None of it is simulated end to end.

**Verdict: partly novel.** Moderate impact. The question is policy-relevant to the US deployment.

### R3. NR sidelink attacks with verification load
Closest:
- twardokus2022vehicle and twardokus2023protecting (protocol-aware LTE/NR scheduling DoS with roadway impact).
- trkulja2020denial, yoon2021evasive, djaidja2022adaptive, talukder2024symjam, hossain2026sync (sync spoofing), ashik2026analyzing (SPS starvation with Age of Information), silva2025evaluation, bitsikas2025security (NR sidelink spec analysis), bello2026human.
- tine2025real (protocol-compliant UDP and oversized-BSM flooding degrading real FCW).
- Verification DoS: he2012mitigating, jin2019dos, jin2024future, jin2025accountable, ko2021avod, zhou2026efficient.

Already done: MAC-layer attacks and verification-queue DoS, each separately.

Open: a **combined** attack, in which resource-exhaustion on NR Mode 2 interacts with signature-verification exhaustion, with PQ-sized messages, in closed loop with safety outcomes. tine2025real is the nearest, but it uses real LTE R14 devices at small scale with no PQ.

**Verdict: partly novel.**

### R4. LLM agents for cross-layer root-cause analysis
Closest:
- Simulator-configuring agents: rezazadeh2024genonet and rezazadeh2025generative (LLM multi-agent + ns-3: generate, run, interpret), han2026ns3, li2024chatsumo, jeong2025agentsumo, liu2026simskill (SUMO agents), xu2026cost (configuration optimisation).
- Root-cause and troubleshooting: wang2025network (NIKA), tseng2026sade, sana2025reasoning (TeleLogs), yi2025automated (cross-layer 5G video), lu2026wirelessopsagent.
- V2X and security: gao2025exploring (LLM agents build traffic scenarios, attacks and defences), hazarika2025llm (LLM V2X telemetry and fault detection), wu2025v2x, carie2026hierarchical (LLM calibrates an MA-like conviction gate), gao2026context and gao2026hierarchical (LLM incident response).

Already done: an LLM agent that configures and analyses simulation runs. That claim is not novel.

Open: a V2X **cross-layer root-cause benchmark whose ground-truth causes are established by exact counterfactual reruns**. An agent's explanation ("the crash happened because the CRL arrived 4 s after the attacker's beacon") can then be scored causally, not by an LLM judge (tseng2026fault notes LLM-judge scoring as a limitation).

**Verdict: partly novel.** The novelty comes from the counterfactual ground truth, which is direction 3's asset.

### R5. Quantified ISO/SAE 21434 ratings
Closest:
- furtado2018threat (SCMS STRIDE threat model, qualitative).
- jayaratne2024simulation (simulation framework for automotive risk assessment), kamtam2025wolves (simulation-based attack-feasibility estimation).
- petho2024quantifying (DoS → safety risk regression), yang2021risk (reachable impact of stealthy attacks).
- marksteiner2023tara (TARA to test generation), monteuuis2022v2x (attack trees too large), chlup2022threatget.

Already done: simulation-informed TARA has started.

Open: SCMS-specific TARA ratings in which impact is computed from paired-counterfactual safety harm and feasibility from executed attack success rates, across the full SCMS threat list.

**Verdict: partly novel.** Low to medium novelty; useful as an application chapter, not a headline.

### Engine capability: threshold SCMS authorities (FROST, CGGMP21, Umbrella)
Closest: ajibuwa2023resilient (threshold Schnorr signing across VPKI CAs), stinson2001provably (threshold Schnorr implicit certificates, 2001), wasef2011managing (threshold revocation), zhang2025decentralized, kong2024efficient, wei2026post, adarbah2024framework.

**(corrected after critique)** The original line said no V2X paper uses FROST or CGGMP21. That is false: ajibuwa2023resilient (arXiv 2308.02711, read in full in `eval-C-robustness-and-threshold.md`) uses FROST to threshold-sign certificates across clustered CAs in a multi-regional VPKI, positioned against the SCMS. No V2X use of CGGMP21 was found.

Open: threshold RA/PCA/LA/MA signing inside a running SCMS, with the availability, latency and compromise-tolerance trade-off under outages and its knock-on effect on provisioning and revocation.

**Verdict: partly novel and narrow (corrected after critique).** Threshold signing in a VPKI exists (ajibuwa2023resilient; stinson2001provably for threshold implicit certificates). What survives is threshold authorities that are wire-compatible with IEEE 1609.2/1609.2.1 inside a running SCMS, evaluated for outage and compromise tolerance.

---

## New open directions the literature reveals (the engine is unusually well placed)

**N1. Interventional evaluation of misbehaviour detectors.** Detectors are ranked by precision, recall and F1 on VeReMi-type data. Several papers say this is the wrong yardstick:
- heijden2018veremi: "PR curves do not give the full picture".
- wehmer2021practical: impact-based metrics.
- muzammil2026code: benchmark artefacts.
- wehmer2023fast: the causal effect of MBD is "still elusive".
- schofield2025beyond: causal sensitivity.

The open problem is to rank detectors (and thresholds) by their **exact counterfactual safety effect** when deployed in the loop: harm prevented by true detections minus harm caused by false positives and wrongful revocations. The data would be same-seed reruns with detector A, detector B and no detector. No paper does this.

**Verdict: novel.** It is the natural flagship task of the direction 3 benchmark.

**N2. Weaponised revocation (framing attacks against the MA).** twardokus2021targeted shows a reactive jammer can get an honest vehicle discredited by MBD in two minutes. kamel2020simulation lists "false report injection" as future work. bimeyer2012central assumes a benign majority of reporters.

No paper follows such framing through the MA to **actual revocation of an honest vehicle** and its safety cost: lost awareness of a legitimate vehicle, and the vehicle's own loss of credentials until re-enrolment. Nor has anyone designed MA policies that resist it. The catalog search for wrongful revocation, framing or discreditation found only twardokus2021targeted.

**Verdict: novel.** It pairs naturally with direction 1: the MA must trade time to neutralise against framing resistance.

**N3. Robustness of security conclusions to modelling choices.** Several papers show single modelling choices flip results:
- baharlouei2022exploring: mobility and communication settings change DDoS-detection results.
- bello2026ns3learn: radio realism reverses speed trends and doubles hard braking.
- solida2026demystifying: Veins defaults deviate from a living lab.
- xu2026cost: cost and statistical-power modelling of co-simulation.

The open problem is a **systematic** global sensitivity analysis: which of the roughly 300 settings flip the sign or ranking of a security conclusion. It would use paired reruns to cut variance and be anchored to field data (ylmaz2025simulated USDOT LTE-V2X PER, bauder2023cooperative real CAM and pseudonym behaviour, song2024c NIST 1609.2 captures).

**Verdict: partly novel** (single-factor evidence exists). The systematic version is novel. It is high value as a methodological chapter and as a defence of all other results.

**N4. Calibrated validity of causal estimators and twins in V2X.** This is direction 3 used as a service. Take the published causal methods (CausalSim, Veritas, conformal counterfactual KPIs, DTCF, causal falsification of twins) and measure their counterfactual error against the engine's exact counterfactuals in a V2X security setting.

**Verdict: novel.** It is a strong bridge to the networking-ML community (NSDI and SIGCOMM venues).

---

## Ranking (novelty first, then impact)

| Rank | Direction | Verdict | Impact | Why |
|---:|---|---|---|---|
| 1 | **Direction 3 + N1 + N4**: counterfactual V2X benchmark with exact per-unit cross-layer ground truth; flagship tasks are interventional detector evaluation and validation of causal estimators and twins | novel | high | No V2X counterfactual-pair benchmark exists; it fixes acknowledged metric and benchmark flaws; it reaches both the V2X and networking-ML communities |
| 2 | **N2 + Direction 1**: MA decision design under framing attacks and time to neutralise in the US SCMS, with counterfactual safety attribution | N2 novel; D1 partly novel | high | Weaponised revocation is unstudied; time-to-neutralise decomposition with linkage CRLs is open; it is policy-relevant (J3287, SCMS) |
| 3 | **N3**: robustness of V2X security conclusions to modelling choices | partly novel (systematic form novel) | medium-high | Strengthens every other result; field anchors are available |
| 4 | **Threshold SCMS authorities** (FROST/CGGMP21) in a running system | partly novel (narrow) (corrected after critique) | medium | FROST in a VPKI already exists (ajibuwa2023resilient); only the 1609.2-compatible, SCMS-integrated evaluation remains |
| 5 | **Direction 2 (narrowed)**: whole-system PQ SCMS migration with closed-loop safety on three radio stacks | partly novel | high but crowded | Must engage Twardokus, abrar2026deployment, fikry2026post and Petho's small-effect result |
| 6 | **R3**: combined NR Mode 2 resource and verification exhaustion with PQ sizes | partly novel | medium | Each attack alone is published |
| 7 | **R4**: LLM cross-layer root-cause analysis scored by counterfactual ground truth | partly novel | medium | Configuration agents are done; novelty depends on rank 1 |
| 8 | **R2**: multi-provider US SCMS operation | partly novel | medium | Implementation comparisons exist; system dynamics do not |
| 9 | **R5**: quantified 21434 ratings | partly novel | low-medium | Simulation-based TARA has begun |
| 10 | **R1**: cross-layer linkability | mostly already done | low | Only a joint multi-signal remainder is open |

**Already done or not defensible as headlines:**
- The single-process integrated traffic and radio simulator.
- SCMS + traffic + network co-simulation.
- An LLM agent that configures simulations.
- The general framing "security costs safety".
- Individual cross-layer linkage channels.
- Closed-loop attack-to-safety studies.
- Detection-to-local-reaction loops.

Each is listed with its ruling papers in `claims-we-cannot-make.md`.

**Must read in full before submission** (closest papers whose summary rests on title or abstract only):
- haas2012global, liu2010limits, willecke2023a4md, abdo2024vehicular, twardokus2025dt.
- henry2020pseudonym, ganan2012rar, krishnan2011verify, bindel2022drive, bittl2015security.
- wang2026reproducibility (and its companion paper), jayaratne2024simulation, kamtam2025wolves.
