# Evaluation: candidate C (robustness of V2X security conclusions) and candidate D (threshold SCMS authorities)

Date: 2026-10-17. Method: re-read of the corpus summaries; full-text extracts of bello2026ns3learn, solida2026demystifying, xu2026cost and ajibuwa2023resilient (the last fetched from arXiv 2308.02711, because the catalog summary was abstract-only); catalog regex searches (sensitivity / Sobol / Morris / factorial / credibility / threshold / FROST / MPC); fresh searches on OpenAlex, arXiv API, Semantic Scholar (throttled after one query) and web search. baharlouei2022exploring has no open-access full text, so it is still judged from its abstract.

---

## Candidate C: systematic sensitivity of V2X security conclusions to modelling choices

### Closest works and what each already does

| Work | What it already does | Gap it leaves |
|---|---|---|
| bello2026ns3learn (full text) | Swaps only the NR Mode-2 reception model in Veins/SUMO. This reverses the simulated speed trend, more than doubles hard braking, and **"registers an adversarial flood that neither reference represents at any flood rate"**. So a single model choice decides whether an attack is visible. | One factor (the reception model); no PKI; no ranking of security mechanisms; open-loop is not the issue (coupled), but there is no screening of other factors. ns-3 reference has "no application-layer security, so cryptographic delay is zero". |
| baharlouei2022exploring (abstract) | Varies grid pattern, communication settings and mobility; DDoS anomaly-detection results change. | A handful of factors, one attack, one detector family; no variance decomposition. |
| solida2026demystifying (full text) | Veins default vs MASA living lab: RSSI overestimated, about 18% fewer messages. It argues explicitly that "promising results obtained in VeReMi ... may not be replicated in VEINS without appropriate extensions and calibrations". | Field validation of the radio only; no security result is re-run. |
| xu2026cost (full text) | Cost model and statistical-power-constrained configuration of SUMO+Veins co-simulation. It explicitly worries that conclusions are "sensitive to randomness". | It is about cost and replication count, not about which modelling factor flips a conclusion; no security outcome. |
| bloessl2019case (abstract) | Shows default PHY parameters in VANET simulators are poor and most reviewed papers copy or omit them. | Pitfall catalogue; not a sensitivity analysis of security results. |
| huisman2023impact (full text) | Shows that the controller realisation significantly changes CACC attack impact (reachable sets). | Control theory, one factor. |
| kamel2020veremi, hermann2026veremi, wu2026bsmattacker (abstract) | VeReMi Extension adds sensor-noise models. VeReMi NextGen shows attacks are harder to detect on a different generator. BSMAttacker trains nine detectors on simulation and tests them on five real datasets (sim-to-field). | Each is one dataset or generator swap. BSMAttacker's results were "under revision" in v1.0.0, so **this is the paper most likely to pre-empt the ranking-flip claim. Watch it.** |
| muzammil2026code (abstract) | Audit of structural artefacts in VeReMi benchmarks. | Dataset artefacts, not modelling factors. |
| pawlikowski2002credibility, sarkar2014revisiting (title/abstract) | The credibility crisis of telecom-network simulation (statistics, replication). | General; no security. |
| ylmaz2025simulated, bauder2023cooperative, song2024c | Field anchors (USDOT LTE-V2X PER; real CAM/pseudonym traces; NIST 1609.2 captures). | Usable as anchors; none is a sensitivity study. |

### New papers found (not in the catalog)

- Stetsko, Smolka, Jurnečka, Matyáš, "On the Credibility of Wireless Sensor Network Simulations: Evaluation of Intrusion Detection System", SIMUTOOLS 2012. https://www.muni.cz/en/research/publications/977194. **This is the closest non-vehicular precedent.** The same IDS evaluation gives different outcomes across simulators configured identically.
- Kurkowski, Camp, Colagrosso, "MANET simulation studies: the incredibles", ACM MC2R 9(4), 2005. The classic credibility audit, and a reviewer will expect it to be cited.
- Joerer, Sommer, Dressler, "Toward Reproducibility and Comparability of IVC Simulation Studies: A Literature Survey", IEEE Commun. Mag. 50(10), 2012, doi:10.1109/mcom.2012.6316780. Its companion paper is "Comparing Apples and Oranges? Trends in IVC Simulations", ACM VANET 2012.
- Ajjaj et al., full-factorial, Plackett–Burman and Taguchi DoE with ANOVA on VANET AODV, with **black-hole and wormhole attacks as factors**, Applied System Innovation, Feb 2022. https://doaj.org/article/931f5cce662149c0bfe386331ffdc305. This is the only DoE with attacks found. It ranks factor contributions to network KPIs, not the robustness of a security conclusion.
- Martinez/Fogue et al., 2^k factorial analysis of warning-message dissemination (density and road map dominate), Sensors 2013. https://zaguan.unizar.es/record/58508/files/texto_completo.pdf
- "The Impact of Mobility Patterns on MANET Vulnerability to DDoS Attacks", Elektronika ir Elektrotechnika. https://eejournal.ktu.lt/index.php/elt/article/view/1358. Mobility model changes the DDoS vulnerability conclusion (MANET, single factor).
- Carpenter and Sichitiu, ICNP 2014 (obstacle modelling: ignoring obstacles "greatly overstate[s]" VANET safety-application performance). https://computer.org/csdl/proceedings-article/icnp/2014/6204a480/12OmNBz100Z
- "Sensitivity-driven Adaptive Contention Window Optimization for IEEE 802.11 based V2I Networks", arXiv 2608.01555 (2026). Sobol indices on an **analytical** DCF model; not on a simulator; no security.
- Borg et al., "Digital Twins Are Not Monozygotic: Cross-Replicating ADAS Testing in Two Industry-Grade Automotive Simulators", ICST 2021, arXiv 2012.06822. Safety-violation verdicts differ between PreScan and Pro-SiVIC. Stocco et al., "Mind the Gap!", arXiv 2112.11255 (IEEE TSE). These are the automotive-testing analogues; a software-engineering reviewer will cite them.
- Arp et al., "Dos and Don'ts of Machine Learning in Computer Security", USENIX Security 2022. This is the template for the traction version of this paper: a pitfall taxonomy plus re-evaluation of published work.

Searches with **zero** direct hits: Sobol or Morris applied to VANET/V2X **security** results; many-factor global sensitivity analysis of misbehaviour-detection rankings, revocation latency or PQ overhead; common-random-number (same-seed) paired designs in V2X security.

### The exact claim that survives

> The first many-factor global sensitivity analysis of **security conclusions**, not of network KPIs, in V2X. It uses Morris screening over the roughly 300 settings, followed by variance-based (Sobol) analysis of the surviving factors, under common-random-number paired reruns. It reports, for detector ranking, time to neutralise and PQ safety cost, **which modelling choices flip the sign or the ranking**, not just which ones move the magnitude. Each factor range is bounded by field data (USDOT LTE-V2X PER, the MASA living lab, real CAM and pseudonym traces). It then re-tests a set of published V2X security claims against that envelope.

What cannot be claimed:
- "First to show that modelling choices change security results": ruled out by baharlouei2022, bello2026ns3learn and Stetsko 2012.
- "First DoE or factorial analysis in VANETs": ruled out by Ajjaj 2022 and Martinez 2013.
- "First VANET credibility study": ruled out by Kurkowski 2005, Joerer 2012 and Pawlikowski 2002.

### Verdict: **partly novel.** The single-factor evidence is published; the systematic, security-conclusion, ranking-flip version is novel.

Traction depends on the re-evaluation element. A sensitivity table for one's own engine is a methods chapter. "X of Y published V2X security conclusions do not survive plausible modelling ranges" is a paper in the style of Arp et al. (USENIX Security, CCS, or an ACM Computing Surveys / IEEE COMST methods piece; VNC or VehicleSec at minimum).

### Strongest reviewer objection

"You measure the fragility of **your own engine's** model family, inside ranges **you** chose. Sobol indices depend on the ranges you set, a sign flip inside an implausible corner proves nothing, and without field ground truth for the security outcomes themselves (detector rankings or revocation timing in a real deployment) you can show that a result is fragile but not which answer is right. The radio is matched to Veins, which solida2026 shows is itself biased."

Mitigations:
- Bound every range by field data or by values the standards give.
- Report flips only inside the field-plausible region.
- Show that the engine reproduces at least one published single-factor flip, for example bello2026's flood visibility.
- Make the deliverable a reusable "robustness envelope" protocol, not a one-engine table.

A second objection is cost: Morris screening needs about r(k+1), roughly 3,000 city-scale runs per scenario at k=300 and r=10. Group screening and cheaper corridor scenarios need to be justified.

---

## Candidate D: threshold signing (FROST, CGGMP21) for SCMS authorities

### Closest works and what each already does

| Work | What it already does |
|---|---|
| **ajibuwa2023resilient** (arXiv 2308.02711; read here in full text) | **Uses FROST** (t-of-n, preprocessing, single-round signing with an aggregator) to threshold-sign certificates across clustered CAs in a multi-regional VPKI explicitly positioned against "conventional SCMS and IFAL". It rejects ECDSA as lacking "the ideal algebraic structure for efficient threshold signing". Evaluation: NS3 + MIRACL operation-count cost and a cluster-size "elbow" at 20. Future work: "an extensive set of simulations using NS3 and SUMO". Assumes semi-honest parties. **This kills the corpus statement "no V2X paper uses FROST"** (novelty-check.md and claims-we-cannot-make.md need correcting). |
| stinson2001provably (Stinson & Strobl, ACISP 2001) | A robust (t,n) threshold Schnorr signature **and a threshold implicit-certificate scheme**. ECQV issuance is linear in the CA key, so a threshold PCA for 1609.2 implicit certificates is essentially this 2001 construction. |
| zhang2025decentralized (TIFS), kong2024efficient (WCNC), weerasinghe2023threshold (Future Internet; NS-3 plus a real 5G testbed), zhou2026bls (TMC; BLS threshold with DKG and proactive refresh, NS-3), yuan2026qr (TVT; lattice PQ threshold signatures), wei2026post (TDSC), adarbah2024framework, harit2012rsa, oulhaci2016secure | Threshold credential issuance, authentication or eviction for VANETs. None of them is SCMS-faithful (no butterfly keys, no LA/MA, no linkage CRLs), and none runs inside a provisioning/revocation system with outages. |
| brecht2018security, IEEE 1609.2.1 | The SCMS already distributes trust: m-of-n **electors** endorse root changes, and LA1/LA2 plus the PCA/RA separation prevent any single authority from tracking. A threshold design must beat this baseline, not a single CA. |
| Not in the catalog: Jayaraman, Li, Evans, "Decentralized Certificate Authorities", arXiv 1706.03370 (MPC CA for web PKI); Chalmers MSc thesis "Vehicle Authentication with Threshold Signatures" (VATS, 2023; threshold keys across in-vehicle ECUs), https://odr.chalmers.se/items/20a835b9-3355-457f-a847-0e2263145401 | MPC CAs in general, and threshold signing on the vehicle side. |

No paper found on threshold ECDSA (CGGMP21, DKLs) for V2X; on a threshold PCA, RA, CRL generator or linkage authority inside an SCMS; or on any measured outage or compromise behaviour of a threshold SCMS. Threshold ECDSA products from industry vendors for V2X CAs were not found either.

### The exact claim that survives

> Threshold PCA, CRL-generator and LA signing that is **wire-compatible with IEEE 1609.2 and 1609.2.1**, so unmodified OBUs verify it. That means threshold ECQV, Stinson–Strobl/FROST style, for implicit pseudonym certificates, and CGGMP21 threshold ECDSA for explicitly signed certificates and CRLs. It is evaluated inside a running SCMS (butterfly provisioning, top-ups, CRL issuance and linkage-seed release) for availability, issuance latency and compromise tolerance under share-holder outage, network partition and t−1 compromise, with the knock-on effect on provisioning failures and time to neutralise.

### Verdict: **partly novel (narrow).**

The corpus's earlier "novel" verdict is downgraded. FROST in VPKI (ajibuwa2023) and threshold implicit certificates (2001) both exist. What survives is the standard-compatible, SCMS-faithful, systems-level evaluation.

### Strongest reviewer objection

"The cryptography is off the shelf, and CA signing is batch and offline, so the latency cost is predictable and small. The SCMS already removes single points of trust through electors, LA1/LA2 and organisational separation, and HSMs cover key theft. This is engineering, not research: what security property do you get that the existing SCMS separation does not?"

To survive the objection, the thesis needs a sharp result. Two examples:
- A threshold PCA or LA removes a specific collusion or compromise path (for instance PCA+RA tracking, or forged pseudonym issuance after a single-site breach) at a measured availability cost.
- Threshold CRL signing changes time to neutralise under a regional outage.

Without such a result, the topic fits a short VNC or VehicleSec paper or a thesis chapter, not a headline.

---

## Bottom line

- **C:** partly novel, and the systematic version is novel. Its value is as a defensive methods contribution. It gains traction only if it re-tests published conclusions. Watch wu2026bsmattacker (sim-to-field detector transfer) and bello2026ns3learn.
- **D:** partly novel and narrow. The corpus's "no FROST in V2X" statement is false (ajibuwa2023resilient). Only the 1609.2-compatible, SCMS-integrated outage and compromise evaluation remains.
