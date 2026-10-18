# Traction and impact evaluation of the candidate directions

Date: 2026-10-17. Scope: who would use each result, who funds the area, which communities would cite it, and which 2026–2027 venues fit. The five candidates are:
- **A**: counterfactual V2X benchmark with interventional detector evaluation.
- **B**: weaponised revocation and time to neutralise.
- **C**: robustness of V2X security conclusions to modelling choices.
- **D**: threshold SCMS authorities.
- **PQ**: the narrowed PQ-SCMS migration direction.

This file complements `novelty-check.md`. It does not repeat the novelty verdicts except where a new search changed one.

**Method:**
- Catalog activity counts (keyword regex over title and abstract, 11,315 entries), normalised by catalog size per year.
- Citation counts of the closest works, taken from `catalog.jsonl` (they come from OpenAlex or Semantic Scholar at harvest time, so they lag).
- Fresh searches on OpenAlex, arXiv, Semantic Scholar (heavily throttled: 3 of 5 queries returned nothing) and the web, aimed at finding prior work that kills each claim.
- Conference, funder and standards pages, read directly where possible.

Unverified dates are marked **(unconfirmed)**.

---

## 1. Bottom line

| Rank by traction | Candidate | Who needs it, concretely | Likely 5-year citations (thesis-derived papers) | Best-fit venues 2026–27 | Main risk |
|---:|---|---|---|---|---|
| 1 | **B**: weaponised revocation + MA remediation policy + time to neutralise | 5GAA says deployed systems "have not yet specified the conditions to determine when a particular remediation technique is to be used" (MBD Operational Aspects WP, May 2024). **(corrected after critique)** The next sentence says the paper "attempts to describe what conditions might be used", and its §6–7 propose a severity rating and a remediation-to-misbehaviour mapping; it never mentions false reports or slander (`critique-framed.md` §2.5). SCMS Manager / MA operators; J3287 (Aug 2024) and ETSI TS 103 759 v2.2.1 (Jan 2026) define the reports but not the decision | 20–60 | USENIX Sec '27 cycle 2 (26 Jan 2027), VehicleSec '27, NDSS '28 summer cycle, IEEE TDSC/TIFS | Partial pre-emption at detector level (VehiGAN adversarial false positives; DRAMBR false accusations); needs a credible MA model |
| 2 | **A**: counterfactual benchmark + interventional detector ranking (with C as its validity chapter) | The MBD research community, which still ranks detectors on VeReMi (heijden2018veremi 255 and kamel2020veremi 237 citations, both datasets). Causal-ML and network-twin authors who say ground truth is missing (CausalSim, Veritas, hou2024if) | 10–30 if the benchmark is not adopted; 50–150 if it is released and adopted like VeReMi | NeurIPS '27 Evaluations & Datasets track (about May 2027), VehicleSec '27, VNC '27, IEEE TMLCN / TIFS | Large engineering and release burden; reviewers will attack simulator fidelity (hence C) |
| 3 | **PQ** (narrowed): whole-system PQ/hybrid SCMS with closed-loop safety on three radio stacks | Clearest explicit practitioner demand: an industry-submitted research idea "Impact of PQC on SCMS operation and intended protection performance", rated *Critical, High Priority*, **not funded** (June 2026). NIST IR 8547 draft (ECDSA deprecated after 2030, disallowed after 2035); 5GAA–GSMA PQ statement (Mar 2025); ETSI TR 103 949 and TR 103 971-1 (May 2026) | 20–80, spread thin across many competing papers | VehicleSec '27, IEEE TVT, VNC '27, NDSS FutureG workshop, ICMC (industry) | Most crowded and fastest-moving area. Funded competitors (TraCR, about $0.8M across two active PQ-V2X projects) can scoop the gap within 12 months |
| 4 | **C**: robustness to modelling choices | Every simulation-based V2X security author, and reviewers. USDOT field data (ylmaz2025simulated) as anchors | 10–40 as a stand-alone paper; much more valuable as a defence inside A/B/PQ | VNC, WSC '27, ACM TOMACS, SIGSIM PADS; an SoK framing at NDSS/S&P is possible but harder | Its closest works are recent and lightly cited (bello2026ns3learn 1, baharlouei2022exploring 14); it reads as "methods" |
| 5 | **D**: threshold SCMS authorities (FROST, CGGMP21) | NIST's Multi-Party Threshold call (IR 8214C, Jan 2026; MPTS 2026 workshop); RFC 9591 (FROST). No SCMS operator demand found | 5–15 | NIST MPTS workshop, ACNS/AsiaCCS/ESORICS, VehicleSec | The closest V2X work has almost no uptake (ajibuwa2023resilient: 1 citation in 3 years); low safety relevance |

**Recommended top three:** **B**, **A (with C folded in as its validity chapter)**, and **PQ (narrowed)**. D is best kept as an engine capability or a workshop paper. That ordering differs from `novelty-check.md`, which ranked A first. B moves up on traction because it has a named, unmet, documented industry need (the 5GAA gap statement) and attack-plus-defence papers suit top security venues. A has the higher citation *ceiling*, but only if the benchmark is released and adopted. PQ has the most attention and money but also the most competitors.

---

## 2. How active each area is (catalog evidence)

Hits are keyword-regex matches, so absolute numbers are noisy; the trend in share of the catalog is the useful signal. Catalog entries per year: 2022 965, 2023 1,097, 2024 1,409, 2025 1,904, 2026 1,511 (partial year).

| Area (regex) | 2022 | 2023 | 2024 | 2025 | 2026 | Share 2022 → 2026 | Reading |
|---|---:|---:|---:|---:|---:|---|---|
| Misbehaviour detection | 63 | 74 | 67 | 74 | 73 | 6.5% → 4.8% | Mature, steady and large. Many ML-detector papers, few evaluation-method papers |
| Revocation (vehicular) | 41 | 33 | 30 | 49 | 52 | 4.2% → 3.4% | Steady; renewed 2025–26 (NDSS 2024 Scopelliti, DLT schemes) |
| Post-quantum (vehicular) | 32 | 38 | 54 | 161 | 258 | 3.3% → 17% | **Surging, about 5× in share**. Crowded |
| Threshold signatures / MPC | 15 | 20 | 21 | 43 | 32 | 1.6% → 2.1% | Small. Mostly generic cryptography, not V2X |
| Counterfactual / causal | 13 | 17 | 24 | 32 | 59 | 1.3% → 3.9% | Growing about 3× in share; mostly outside V2X |
| World model / surrogate / twin | 79 | 111 | 152 | 209 | 217 | 8.2% → 14.4% | Hot; the "twin validity" hook has an audience |
| Sensitivity / fidelity / reproducibility | 18 | 26 | 29 | 89 | 130 | 1.9% → 8.6% | Rising, but the regex is broad (includes generic "fidelity") |

**Citation weight of the closest works** (catalog counts):
- **A**:
  - VeReMi datasets 255 and 237; F2MD (kamel2020simulation) 164.
  - CausalSim (NSDI) 46; Veritas (SIGCOMM) 13; hou2024if 11.
  - Mobiworld 14; zhang2026driving 0, laudy2026digital 0 (both 2026).
  - Evaluation-method papers in MBD are almost uncited: wehmer2021practical 1, wehmer2023fast 0, muzammil2026code 0.
  - *Reading:* benchmarks attract citations, but critiques of the metrics have not yet gained traction. A benchmark that operationalises the critique has a clear opening.
- **B**:
  - raya2007eviction 459; haas2011efficient 108; heijden2017analyzing 99; bimeyer2012central 79; khodaei2020scalable 66; kamel2019misbehavior 56.
  - scopelliti2024efficient (NDSS) 14 in about 2 years.
  - twardokus2021targeted (framing, the only direct precedent) 6; willecke2023a4md 6.
  - *Reading:* foundational revocation work is well cited, and recent MA/framing work is thinly populated. There is an opening, and the area has a citation-rich base to anchor on.
- **C**:
  - pegurri2026van3twin 17, baharlouei2022exploring 14, bauder2023cooperative 12.
  - bello2026ns3learn 1, solida2026demystifying 0, xu2026cost 0.
  - *Reading:* young and lightly cited; the impact is mainly defensive.
- **D**:
  - stinson2001provably 128 (old cryptography); komlo2021frost 240 (generic).
  - V2X-specific: ajibuwa2023resilient 1, zhang2025decentralized (TIFS) 5, kong2024efficient 0, wei2026post (TDSC) 0.
  - *Reading:* the V2X-threshold niche has had little uptake so far.
- **PQ**:
  - twardokus2024cryptography (NDSS) 24 in about 2 years; lohmiller2025survey 27; chen2024hybrid 19; barreto2018qscms 16; yoshizawa2023post 15; bindel2022drive 13.
  - Ten or more 2025–26 papers at 0–2 citations (abrar2026deployment, fikry2026post, sengupta2026adaptive, karinsalo2026migrating, chen2026pseudonym…).
  - *Reading:* there is attention, but it is split across many near-duplicate feasibility studies. A paper must be distinctly bigger or more system-level to stand out.

---

## 3. Per-candidate assessment

### A. Counterfactual V2X benchmark + interventional detector evaluation

**Who would use it**
- *MBD researchers.* They need a replacement for, or complement to, VeReMi-style precision, recall and F1. muzammil2026code documents artefacts in VeReMi, and heijden2018veremi says "PR curves do not give the full picture".
- *Detector vendors and the ETSI/SAE MBD groups.* J3287 and TS 103 759 standardise report formats, and 5GAA's WP explicitly leaves detector efficacy and remediation conditions to policy bodies. A standards body has no way today to compare detectors by *safety effect*. This benchmark would give it one.
- *Causal-ML and network-twin researchers* (CausalSim, Veritas, conformal counterfactual KPIs, counterfactual cross-validation under network interference, shirani2025validate). They need environments with exact counterfactual ground truth. The ML community now publishes such simulator-generated causal benchmarks at top venues. Examples not in the catalog:
  - CausalVerse, NeurIPS 2025 Datasets & Benchmarks.
  - An agent-based epidemic counterfactual benchmark (arXiv 2606.05692, 2026).

**Funders**
- NSF SaTC 2.0 (NSF 25-515; target dates 28 Sep 2026 (passed) and 25 Jan 2027, **unconfirmed**) and CPS. This would be through an advisor's proposal.
- EU Horizon CCAM topic D6-03, "generative AI for CCAM perception, decision-making and validation" (€13M, 2026 call 4 Jun – 8 Oct 2026, closed). Only the validation and world-model angle fits; the 2026 CCAM topics have no dedicated cybersecurity line.

**Kill attempts**
- OpenAlex/arXiv "counterfactual + misbehavior": 0 relevant hits on arXiv, and OpenAlex returned only generic IDS papers.
- Semantic Scholar surfaced zhang2024enhancing (VTC2024-Fall, MBD and mitigation at intersections) and hermann2025hyperparameter, both already in the catalog. Neither uses paired counterfactuals.
- **The verdict stands.**

**Traction drivers**
- A public benchmark plus leaderboard is the single biggest citation multiplier in this field (VeReMi has about 490 combined citations).
- Bridging two communities widens the citing pool.

**Traction drag**
- The value depends on release quality: data volume, documentation, licence, stable task definitions.
- Simulation-fidelity scepticism. C is the answer.

**Venue fit**
- *NeurIPS 2027 Evaluations & Datasets track.* Renamed in 2026, and evaluation is now "an object of scientific study". Benchmark audits and new evaluation protocols are in scope; the 2026 deadline was 4–6 May, so 2027 is likely similar.
- *VehicleSec '27.* The 2026 CFP lists "simulation, digital twin" among fundamental technologies, plus a Distinguished Artifact Award; the 2026 deadline was 24 Feb, so 2027 is likely similar (unconfirmed).
- *IEEE VNC '27* (unconfirmed; VNC 2026 was in June, deadline typically around March).
- *Journals:* IEEE Transactions on Machine Learning in Communications and Networking, IEEE TIFS / TDSC (detector evaluation).
- *CLeaR 2027:* deadline 16 Nov 2026, Paris, 3–5 Mar 2027. Too soon for a full benchmark, and it has no benchmark track. Not recommended.

### B. Weaponised revocation (framing) + time to neutralise + MA decision design

**Who would use it**
- *SCMS Manager and MA operators* (US) and *ETSI/C-ITS CPOC / MA designers* (EU). The 5GAA WP (v1.0, 29 May 2024) states the gap verbatim: "V2X systems being deployed have not yet specified the conditions to determine when a particular remediation technique is to be used. A lack of consistent remediation conditions can lead to outcomes that are perceived as unfair." It then asks stakeholders to "explore/finetune the processes of misbehaviour classification … and mapping remediation to misbehaviour". **(corrected after critique)** The paper does propose such a mapping itself (§6–7, with reinstatement classes RC1–RC4), so the gap is evaluating that mapping, in particular under adversarial reports, not supplying a missing one.
  - A quantitative trade-off between time to neutralise and false or framed revocation is exactly the evidence those policy bodies lack.
  - The WP's remediation ladder (R1 software update → R2 suspension through ST0/ST1 partial and ST2 full revocation) can be evaluated directly.
- *USDOT 2024 National V2X Deployment Plan.* It targets "12 interoperable, cybersecure deployments" by 2028 and 25 by 2031. No dedicated misbehaviour action item was found in the summaries, so treat this as a framing hook, not a direct requirement.
- *OEMs and OBU vendors.* A framed honest vehicle losing credentials is a warranty and liability issue. 5GAA highlights the fairness concern.

**Funders**
- USDOT TraCR (Clemson-led national UTC, June 2023 – May 2029). Its thrusts include Security and Resiliency, and it funds V2X-attack projects (for example driver response to falsified V2X, about $250k).
- NSF SaTC (CPS security).

**Kill attempts**
- Web, OpenAlex and arXiv searches for framing, false accusation, discreditation or false reports leading to revocation found only:
  - *DRAMBR* (almani2025distributed, Future Internet 2025, 2 citations). Reputation for report accuracy, including false accusations. **It is partial overlap at the reporting layer and must be cited**; it does not follow framing through MA to revocation.
  - *VehiGAN* (shahriar2025vehigan, ACM 2025, 16 citations). An adversary perturbs benign inputs to raise detector false positives. **This is the closest pre-emption of the "framing" mechanism at the detector**, but there is no MA, revocation or safety cost.
  - ansari2021v2x (CPS standardisation; warns targeted objects "may be reported by others as malicious, raising many false positives in the revocation system"). A stated concern, not a study.
  - carie2026hierarchical (FPRs of 2–8% "operationally prohibitive" at highway density). This supports the motivation.
- **The verdict stands (novel)**, but the thesis must frame its contribution as *end to end through the MA to revocation, with safety and fairness cost*, not as "first adversarial false positives".

**Traction drivers**
- An attack-plus-defence paper on deployed infrastructure (SCMS linkage-seed revocation), anchored to standards J3287 and TS 103 759. Top security venues reward this.
- The citing base is the large revocation and MBD literature.

**Traction drag**
- The MA is not standardised. The thesis must justify its MA model from the 5GAA WP, J3287 §1-2 (misbehaviour management service) and TS 103 759.

**Venue fit**
- *USENIX Security '27 cycle 2:* registration 19 Jan 2027, paper 26 Jan 2027; Denver, Aug 2027.
- *VehicleSec '27* (unconfirmed, about Feb 2027).
- *NDSS 2028 summer cycle* (unconfirmed; NDSS 2027 cycles were 6 May and 19 Aug 2026).
- *IEEE S&P 2027 cycle 2* (17 Nov 2026, likely too soon).
- *ACM WiSec '27* (unconfirmed).
- *Journals:* IEEE TDSC, TIFS, T-ITS.

### C. Robustness of security conclusions to modelling choices

**Who would use it**
- Authors and reviewers of every simulation-based V2X security paper.
- USDOT and ITS JPO, which hold field data (ylmaz2025simulated, LTE-V2X PER).
- TraCR's "collective transportation cybersecurity testbeds" foundational project, which needs testbed-validity arguments.

**Kill attempts**
- The arXiv query "V2X + sensitivity analysis + security": 0 hits.
- OpenAlex found *"A Global Sensitivity Analysis of Traffic Microsimulation Input Parameters on Performance Metrics"* (IEEE T-ITS 2024, 8 citations, doi:10.1109/tits.2024.3372334; **not in catalog**). It is global sensitivity analysis for traffic-only performance metrics, with no radio, PKI or security. It weakens any "first GSA in this space" wording, so it must be cited, but the security-conclusion version stays open.
- Also xu2026cost (statistical power for VANET co-simulation, in catalog).

**Traction**
- Modest as a stand-alone paper; methodology papers in this field are cited slowly.
- Its real value is defensive: it pre-empts the main reviewer objection to A, B and PQ.
- **Recommendation:** fold it into A as the validity chapter, publishing a short methods paper at VNC or WSC only if time allows.

**Venue fit**
- WSC 2027 (WSC 2026: Glasgow, Dec 2026, contributed papers due 5 Apr; 2027 likely similar).
- SIGSIM PADS, ACM TOMACS, VNC.

### D. Threshold SCMS authorities

**Who would use it**
- *NIST's Multi-Party Threshold Schemes project.* IR 8214C (Jan 2026) calls for threshold-scheme submissions with reference implementations and evaluation reports; preview rounds closed in May and July 2026. An SCMS case study (availability, latency and compromise tolerance in a real PKI) is the kind of application evidence that workshop values.
- *SCMS Manager and providers:* no stated demand found.

**Kill attempts**
- OpenAlex and arXiv returned only ajibuwa2023resilient (arXiv 2308.02711; threshold VPKI, in catalog) and generic certificate-revocation surveys. **(corrected after critique)** ajibuwa2023resilient itself uses FROST, so "no FROST V2X work" was wrong; no CGGMP21 V2X work was found.
- **Verdict: partly novel, narrow (corrected after critique)** (see `eval-C-robustness-and-threshold.md` §D).

**Traction**
- Low. The closest V2X work has 1 citation in 3 years, and the safety link is indirect.
- Best treated as an engine capability, used within B: for example, threshold LA/MA so that linkage-seed release survives an authority outage or compromise, which affects time to neutralise.

**Venue fit:** the NIST MPTS workshop (next edition unconfirmed), ACNS/AsiaCCS/ESORICS applied-crypto tracks, a VehicleSec poster or demo.

### PQ (narrowed): whole-system PQ/hybrid SCMS with closed-loop safety

**Who would use it**
- *SCMS Manager, IEEE 1609.2/1609.2.1 editors, CAMP/OEMs.*
  - No formal IEEE 1609.2 PQ amendment or PAR was found (searched; **unconfirmed absence**).
  - An industry-submitted research idea (transportationops.org, 26 Jun 2026; submitter at Michael Baker International) asks for "Impact of PQC on SCMS operation and intended protection performance". It is rated *Critical, High Priority*, $250–499k, 1–2 years, status **Not Funded**.
  - That is direct evidence that practitioners want this exact system-level question answered and that nobody is yet paid to do it.
- *ETSI.*
  - TR 103 949 V1.1.1 (2023-05): QSC migration study for ITS and C-ITS.
  - TR 103 971-1 V2.1.1 (2026-05): TVRA, quantum safety.
  - Neither is in the catalog. Read both before claiming a gap.
- *5GAA.* With GSMA's PQTN task force (statement of intent, Mar 2025), it emphasises crypto-agility. No 5GAA PQ technical white paper was found.
- *NIST timeline.* IR 8547 is still a draft as of mid-2026 per secondary sources (deprecate ECDSA after 2030, disallow after 2035; OMB M-26-15 reportedly uses those dates). Vehicles sold now operate about 15 years, which pushes any V2X migration into that window. This gives the topic a real deadline.

**Funders and competitors**
- USDOT TraCR is actively funding PQ-V2X at Alabama, Clemson and FIU:
  - "Towards Deployment-Ready PQC-Enabled V2X": $498,924, Jan 2025 – Dec 2026, includes certificate segmentation for 1609.2.
  - "AI-Enabled PQC for Real-World Deployment…": $304,508, Apr 2026 – Mar 2027, field-testing hybrid switching.
- **These are the most likely scoopers.** Neither is an SCMS-backend, provisioning or safety-outcome study as described.
- Also twardokus (NDSS 2024, TVT 2025, VehicleSec demo), abrar2026deployment (arXiv 2608.05087), and fikry2026post.

**Kill attempts**
- arXiv "post-quantum AND SCMS": 3 hits, all in catalog (chen2024hybrid / 2501.07028, chen2024pqcmc, chen2026pseudonym / 2606.14008). Designs only, no system simulation.
- IACR ePrint 2026/2149 (Kushwaha & Kaur, Thales; **not in catalog**): PQ TLS, secure-boot and OTA benchmarks on Raspberry Pi; V2X only at a high level, with no 1609.2 or SCMS.
- An ICMC 2026 session, "Impact of Post-Quantum Cryptography on C-ITS" (**not in catalog**, industry talk), shows industry attention.
- **The verdict stands (partly novel)**, but the gap is narrow and closing.

**Traction drivers:** the hottest growth area in the catalog, explicit practitioner demand, and a regulatory clock.

**Traction drag**
- Many 2025–26 feasibility papers sit at 0–2 citations.
- Reviewers will compare against Twardokus and abrar2026deployment. They will also cite Petho's "PKI impact is small" result (petho2024investigating).
- The paper must deliver the regime map (where the safety cost stops being small) to be distinct.

**Venue fit**
- VehicleSec '27, IEEE TVT, VNC '27.
- The NDSS FutureG workshop (sengupta2026adaptive appeared there).
- The ANTS-6G workshop at PIMRC (2026 edition listed "hybrid post-quantum cryptography for automotive PKI"; the 2027 edition is unconfirmed).
- ICMC for industry reach.

---

## 4. Venue calendar (realistic targets for a thesis finishing in 2027)

| Venue | Deadline | Fits | Status |
|---|---|---|---|
| CLeaR 2027 (Paris, 3–5 Mar 2027) | 16 Nov 2026 | A (causal-estimator validation only) | Confirmed by an event listing, not by the official site; probably too soon |
| IEEE S&P 2027 cycle 2 | 17 Nov 2026 | B | From trackers; too soon |
| USENIX Security '27 cycle 2 | Reg. 19 Jan, paper 26 Jan 2027 | B (strongest), PQ | Official preliminary CFP |
| VehicleSec '27 (with USENIX Sec, Aug 2027) | About late Feb 2027 | B, A, PQ, D (poster or demo) | Unconfirmed; inferred from 2025 and 2026 (24 Feb 2026) |
| IEEE VNC 2027 | About Mar 2027 | A, B, C, PQ | Unconfirmed |
| WSC 2027 | About Apr 2027 | C | Unconfirmed (2026: 5 Apr) |
| NeurIPS 2027 Evaluations & Datasets | About early May 2027 | A | Unconfirmed (2026: 4–6 May) |
| NDSS 2028 summer and fall cycles | About May and Aug 2027 | B, PQ | Unconfirmed (2027 cycles: 6 May and 19 Aug 2026) |
| Journals with rolling submission: IEEE TDSC, TIFS, T-ITS, TVT, TMLCN; Elsevier Vehicular Communications | Rolling | All | No V2X-security or PQ-vehicular special issue with a 2026–27 deadline was found. IEEE OJVT's digital-twin issue (quantum-safe listed) closed 8 Feb 2026 |

**Funding calendar** (for an advisor's proposals, not the thesis itself):
- NSF SaTC 2.0: 25 Jan 2027 target, **unconfirmed** (an aggregator lists it).
- USDOT TraCR: through May 2029; annual project calls.
- Horizon Europe CCAM 2027 call (HORIZON-CL5-2027-06): 3 Jun – 7 Oct 2027. Topics D6-04 ("holistic solutions for CCAM integration in critical scenarios") and D6-05 (knowledge hub and tools). There is no cyber-specific topic; the "Cyber secure and resilient CCAM" listing appears only on an aggregator and is unverified.

---

## 5. Relevant items found that are not in the catalog

| Item | Venue / year | Link | Relevance |
|---|---|---|---|
| 5GAA, *Misbehaviour Detection for V2X: Operational Aspects*, v1.0 | 5GAA White Paper, 29 May 2024 | https://5gaa.org/content/uploads/2024/06/5gaa-wi-mbd-op-white-paper.pdf | **B: the key industry-need citation.** Remediation ladder R0 evaluation, R1 update, R2 suspension with ST0–ST3 and reinstatement classes RC1–RC4; §6–7 propose a severity-to-remediation mapping (corrected after critique) |
| *A Global Sensitivity Analysis of Traffic Microsimulation Input Parameters on Performance Metrics* | IEEE T-ITS 2024 | https://doi.org/10.1109/tits.2024.3372334 | C: traffic-only global sensitivity analysis; must be cited |
| *CausalVerse: Benchmarking Causal Representation Learning with Configurable High-Fidelity Simulations* | NeurIPS 2025 D&B | https://proceedings.neurips.cc/paper_files/paper/2025/hash/05962abf920ebddac275d26f83fe118a-Abstract-Datasets_and_Benchmarks_Track.html | A: precedent that simulator-generated causal benchmarks are accepted at NeurIPS |
| Counterfactual prediction benchmark for epidemic time series with time-varying interventions | arXiv 2606.05692, 2026 | https://arxiv.org/abs/2606.05692 | A: a parallel in another domain, agent-based counterfactual benchmark |
| Kushwaha & Kaur, *Identify PQC Algorithms for Automotive Security: Secure Boot, OTA, and V2X* | IACR ePrint 2026/2149 | https://eprint.iacr.org/2026/2149 | PQ: background; no 1609.2 or SCMS |
| ETSI TR 103 949 V1.1.1, *QSC Migration; ITS and C-ITS migration study* | ETSI 2023-05 | https://etsi.org/deliver/etsi_tr/103900_103999/103949/01.01.01_60/tr_103949v010101p.pdf | PQ: **read before claiming a migration gap** |
| ETSI TR 103 971-1 V2.1.1, *TVRA Part 1: Infield Test mode, Quantum safety; Release 2* | ETSI 2026-05 | https://etsi.org/deliver/etsi_tr/103900_103999/10397101/02.01.01_60/tr_10397101v020101p.pdf | PQ: the latest ETSI ITS quantum-safety analysis |
| ETSI TS 103 759 V2.2.1, *Misbehaviour Reporting service; Release 2* | ETSI 2026-01 | https://etsi.org/deliver/etsi_ts/103700_103799/103759/02.02.01_60/ts_103759v020201p.pdf | A/B: current EU report format and MA-facing encryption |
| SAE J3287_202408, *V2X Misbehavior Reporting* | SAE 2024-08 | https://saemobilus.sae.org/standards/j3287_202408-v2x-misbehavior-reporting | B (already cited as anon2024v2x; listed for completeness) |
| "Impact of PQC on SCMS operation and intended protection performance" (research idea, Not Funded) | TRB / NOCoE research ideas, 2026-06 | https://research.transportationops.org/ideas/impact-of-pqc-on-scms-operation-and-intended-protection-performance | PQ: practitioner demand signal |
| TraCR, "Towards Deployment-Ready PQC-Enabled V2X" | USDOT UTC project 2025–26 | https://rip.trb.org/View/2531081 | PQ: funded competitor |
| TraCR, "AI-Enabled PQC for Real-World Deployment … ITS" | USDOT UTC project 2026–27 | https://rip.trb.org/View/2696971 | PQ: funded competitor |
| NIST IR 8214C, *First Call for Multi-Party Threshold Schemes* | NIST 2026-01 | https://nvlpubs.nist.gov/nistpubs/ir/2026/NIST.IR.8214C.pdf | D: the only demand-side signal |
| 5GAA–GSMA statement of intent on the quantum threat | 5GAA 2025-03 | https://5gaa.org/content/uploads/2025/03/5gaa-joint-statement.pdf | PQ: industry framing |
| ICMC 2026 session "Impact of Post-Quantum Cryptography on C-ITS" | ICMC 2026 (Toronto) | https://icmconference.org/session/impact-of-post-quantum-cryptography-on-c-its-g31a/ | PQ: industry attention (talk, not a paper) |

---

## 6. Caveats

- Citation counts come from the catalog snapshot and undercount 2025–26 work. Five-year citation estimates are judgement calls by analogy (VeReMi, Scopelliti NDSS 2024, Twardokus NDSS 2024), not forecasts.
- The catalog's year trends reflect what the review searched for. PQ, for example, had a dedicated cluster, which may inflate its 2025–26 counts.
- Semantic Scholar throttled most queries. Two of the five threshold and framing kill-searches returned nothing (no data, not a negative result). The OpenAlex and arXiv searches did run.
- The CCS 2027, VehicleSec '27, VNC 2027, WiSec 2027 and NeurIPS 2027 dates were not posted; the dates given are inferred from prior years.
- The USDOT plan was assessed from its summaries only; the full plan's security chapter was not read.
- Read the 5GAA MBD WP §5–7 and ETSI TR 103 949 in full before citing them as the gap statements.
