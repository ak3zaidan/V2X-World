# Open questions in the literature

Every stated limitation and future-work item in the 11,315 summaries (`summaries/*.md`, field *Limitations and future work*) was extracted: **2,828 items from 2,051 papers** (high relevance 975 items, medium 1,317, low 536). Items were assigned to the open problems below by keyword rules (`clus.py` logic: one item can fall in several problems). 1,181 items (987 papers) matched no problem; nearly all are generic sentences lifted from abstracts ("security remains a challenge") or say only that future work exists. The batch notes (`batches/batch-*-notes.md`, section 1) list the same items per batch and were used to check the clustering.

Counts are papers, not items. *High* = papers rated high relevance in the catalog. Ordered by how directly each problem bears on this engine, not by size. Paper ids resolve to `summaries/<id>.md`.

Caveat: 73% of summaries rest on the abstract only and 21% on title and metadata only, so many papers' real limitations sections were not seen. Treat counts as lower bounds and as a ranking of attention, not of importance.

## Summary table

| # | Open problem | Papers | High |
|---:|---|---:|---:|
| 1 | From detection to reaction: mitigation, eviction, revocation and graceful degradation | 83 | 23 |
| 2 | Revocation latency, vulnerability window and CRL distribution | 34 | 13 |
| 3 | Global misbehaviour authority, reporting policies and report evaluation | 1 | 1 |
| 4 | Richer attacker models: colluding, adaptive, insider, multi-field, new attacks | 59 | 24 |
| 5 | Robustness of ML detectors to adversarial/evasion attacks | 21 | 8 |
| 6 | Evaluation metrics and benchmarks: comparability, impact-based metrics | 66 | 25 |
| 7 | Training/evaluation data scarcity, realism and dataset artefacts | 98 | 42 |
| 8 | Generalisation of learned models beyond the training distribution | 41 | 15 |
| 9 | Fidelity, uncertainty and guarantees of learned surrogates / world models / digital twins | 81 | 22 |
| 10 | Causal and counterfactual validity: assumptions, RCTs, validating estimates | 9 | 6 |
| 11 | Simulator coupling, fidelity, determinism and reproducibility | 82 | 35 |
| 12 | Radio realism and newer radio technologies (NR sidelink, 5G/6G, channel models) | 127 | 39 |
| 13 | Post-quantum migration: size, packet limits, hybrid schemes and crypto agility | 71 | 24 |
| 14 | Cryptographic overhead: computation, verification load, latency, energy | 162 | 35 |
| 15 | Pseudonym change strategies, linkability and the privacy-safety trade-off | 196 | 43 |
| 16 | Standards gaps, interoperability and multi-domain / cross-border operation | 74 | 23 |
| 17 | Trust-model parameterisation: thresholds, weights, bootstrap | 96 | 20 |
| 18 | Quantified risk assessment and TARA (ISO/SAE 21434) | 7 | 3 |
| 19 | LLM / agent reliability: hallucination, human oversight, contamination, root cause | 51 | 13 |
| 20 | Field validation, real-world deployment and hardware testing | 264 | 90 |
| 21 | Preliminary, small-scale or single-scenario evaluation | 35 | 26 |
| 22 | Scalability to city scale, large fleets and long horizons | 201 | 50 |
| 23 | Scenario diversity: urban vs highway, mixed traffic, VRUs, weather | 127 | 50 |
| 24 | Controller-specific results: platoons/CACC and other automation functions | 39 | 21 |
| 25 | Collective perception and sensor-data misbehaviour | 66 | 16 |
| 26 | Real-time operation and edge/OBU resource constraints | 227 | 54 |
| 27 | Formal security analysis and verification | 29 | 9 |
| 28 | Explainability and interpretability of detectors and models | 36 | 5 |
| 29 | Federated / privacy-preserving / Byzantine-robust learning | 47 | 7 |
| 30 | Blockchain overhead, consensus and practicality | 64 | 12 |

## Future work stated by the closest papers (hand-curated)

These are the items that matter most for the candidate directions, taken from the strong-overlap and near-strong summaries.

- **Detection is not enough; couple it with revocation.** heijden2017analyzing: "detection alone is likely insufficient"; open "how to use detection beyond invalidating the vehicle in the SCMS; combine graceful degradation with effective revocation". heijden2016enhanced: "study impact of intrusion response". ercan2022misbehavior: "add a reaction mechanism (notifications, passive or active revocation)".
- **An intelligent MA and a PKI module.** kamel2020simulation (F2MD): future work "an intelligent Misbehavior Authority to evaluate global investigation, reporting policies and report format", "colluding attackers and false report injection", "add a PKI module".
- **Narrow the vulnerability window.** khodaei2018efficient: "optimal CRL interval based on revocation frequency to narrow the vulnerability window". scopelliti2024efficient: revocation-time parameter trades speed against resources; "privacy-preserving blocking of re-joining after revocation". wantoro2022efficient: "determine optimal management and settings via simulations". mushrall2018emulab: "efficient CRL distribution" and "butterfly key expansion not post-quantum".
- **PQ at system level.** abrar2026deployment: Falcon-512 on NR-V2X Rel-16/17, congestion-adaptive hybrid switching, multi-intersection corridors, hardware profiling. karinsalo2026migrating: "system-level validation of PQC-enabled PKIs under realistic traffic conditions"; multi-algorithm standards. boldyreva2024provable: "achieving post-quantum security of the [butterfly key] protocol". bindel2022need: no lattice implicit certificate construction. twardokus2024cryptography: first messages of each certificate cycle only ECDSA-protected; fully PQ V2V left for after transition. sengupta2026adaptive: no live validation; trace-based only.
- **Counterfactual ground truth.** zhang2026driving: their benchmark covers only short horizons "where alternative ego action does not alter other agents". hou2024if: generalise to "distributed systems and temporal variability"; "incorporate digital twins". alomar2022causalsim: needs an initial randomised control trial; brittle low-rank assumption. wehmer2023fast: quantification of MBD effect in real systems "still elusive". cornish2023causal: observational data cannot certify a twin. qchohi2026confounding: randomised telemetry is scarce.
- **Benchmarks and metrics.** heijden2018veremi: "metrics need more study from a security perspective"; attacker and benign deviations unknowable in advance. hermann2026veremi: no RSSI; VRUs future. muzammil2026code: structural artefacts in VeReMi. malik2025security: benchmark covers jamming against CACC only.
- **Simulator realism.** solida2026demystifying: Veins overestimates RSSI and loses about 18% of messages vs a living lab; tune Veins with real data. bello2026ns3learn: offline traces cannot capture communication-to-driving feedback. bello2026human: analytical channel surrogate needs field validation; single stationary attacker. egealopez2019vehicular: evaluate influence of physically accurate models on traffic results. xu2026cost: one urban scenario; adaptive configuration across stacks. ahmad2025opencams: NR sidelink, GNSS spoofing, real-data DT validation. pegurri2026predicting: higher-layer metrics with full-stack simulation.
- **Interoperability.** chen2026comparison: implicit certificates in CCMS and C-SCMS to compare all three standards. jha2018pseudonym: SCMS cross-certification across trust roots. yoshizawa2022misbehaviour: gaps in cross-authority reporting.
- **LLM agents.** wang2025network: agents "struggle with fault localisation and root cause". protogeros2026benchmarking: agentic systems left for future; formal verification needed. twabi2026netagentbench: golden-configuration benchmarks risk contamination. mondal2023llms: human input still required.

## 1. From detection to reaction: mitigation, eviction, revocation and graceful degradation

**Papers raising it:** 83 (high relevance: 23)

**Open problem:** Detection is evaluated in isolation; what happens after detection (local reaction, MA investigation, eviction or revocation, graceful degradation) and how its timing changes safety is rarely measured. Several authors list the reaction step as explicit future work.

**Bearing on the engine:** Core of direction 1; the engine closes detection, MA, linkage-seed release, CRL and enforcement in one run.

**Representative stated items:**

- [heijden2017analyzing] Detection alone is likely insufficient given the volume of crashes; need prevention via resilient controllers and effective detection and revocation.
- [twardokus2021targeted] Mitigation is challenging
- [wolf2020securing] Fundamental trade-off between safety and efficiency of the mitigation.
- [agustina2026lightweight] Future work: side-channel countermeasures for production.
- [agyapong2026evaluating] Develop detection and countermeasures.
- [ercan2022misbehavior] Future work: add a reaction mechanism (notifications, passive or active revocation).

<details><summary>All 83 source ids</summary>

abenathar2025network, agustina2026lightweight, agyapong2026evaluating, ahmad2025historical, ajjaj2023incremental, albalasmeh2026blockchain, b2024enhancing, bangui2021fasterb, basnet2023deep, benfriha2023survey, bhargava2022duel, borah2025false, borah2025intrusion, buruaga2025benchmarking, chen2025secure, colhak2025accelerating, comert2018change, comert2020change, dawson2025zero, dong2025improved, eldalahmeh2026eliminating, elshafai2025ai, emran2026battlefield, ercan2022misbehavior, fan2025securing, heijden2016enhanced, heijden2017analyzing, hermann2025demo, houmer2020risk, huang2011reputation, huang2021impact, iqbal2022simulating, irfan2022reinforcement, ishtiaq2025intrusion, jan2021survey, jedh2024improvement, jostmann2026digital, khan2020cyber, kiruthika2024simuhack, kumar2026gpu, kumbhare2026comprehensive, kuntegowda2026quantum, li2023v2v, li2024air, li2024cyberattacks, li2024security, manale2025enhancing, medam2026autonomous, miller2019implementation, mokari2024vehicular, morgan2025digital, muller2019trust, nabil2023game, petrov2021framework, pott2021firmware, prabha2024cognitive, rahimi2021evaluation, riad2022blockchainbased, s2026integrated, sbai2020data, sedar2023comprehensive, sedar2024deep, sedjelmaci2025robust, shahariar2024need, shawky2023efficient, song2026lightweight, su2024cooperative, twardokus2021targeted, u2026energy, u2026green, wan2023imct, wang2019spreading, wang2021algorithms, wang2022impact, wang2024denial, wang2024llm, wellensmiles2025systematic, wijesekara2026bcrlsecurelink, wolf2020securing, xia2026generating, zhao2024mika, zheng2020service, zhou2022efficient

</details>

## 2. Revocation latency, vulnerability window and CRL distribution

**Papers raising it:** 34 (high relevance: 13)

**Open problem:** Revocation latency and the vulnerability window: how long a misbehaving credential stays usable, how CRL interval, size and dissemination path set it, and how re-joining after revocation is blocked.

**Bearing on the engine:** Direction 1 'time to neutralise'; the engine has linkage-based CRLs, RSU relay and broadcast.

**Representative stated items:**

- [heijden2017analyzing] Detection alone is likely insufficient given the volume of crashes; need prevention via resilient controllers and effective detection and revocation.
- [mushrall2018emulab] Open challenges: efficient CRL distribution
- [scopelliti2024efficient] Choice of Tv trades revocation speed vs resources and desynchronisation
- [akil2023non] Notes infrastructure-based schemes need RSUs, CRLs via RSUs and pseudonym reuse in rural areas.
- [ercan2022misbehavior] Future work: add a reaction mechanism (notifications, passive or active revocation).
- [khodaei2018efficient] Future: optimal CRL interval based on revocation frequency to narrow the vulnerability window.

<details><summary>All 34 source ids</summary>

abdelmaguid2026beyond, abdullayev2025novel, ahmad2022central, akil2023non, alzamili2026afog, byeon2026v2x, cebe2020communication, dickinson2019leveraging, ercan2022misbehavior, fan2025securing, gu2020efficient, heijden2017analyzing, jena2025privacy, kaur2026zero, khodaei2018efficient, kumar2025centralised, li2025lightweightb, liu2026rat, mahmood2021digital, muid2025accurevoke, mushrall2018emulab, premont2025novel, premont2026leveraging, qi2020privacy, robertndsegmented, ruan2023private, salin2023gap, scopelliti2024efficient, sharma2026transitioning, silva2022acpc, sun2026revocable, tartan2021scalable, whitefield2017formal, zhi2025multi

</details>

## 3. Global misbehaviour authority, reporting policies and report evaluation

**Papers raising it:** 1 (high relevance: 1)

**Open problem:** A global Misbehaviour Authority that evaluates reports, chooses reporting policies and formats, and decides on revocation is named as future work but not built or evaluated end to end.

**Bearing on the engine:** Direction 1 (MA decision design); SAE J3287 report path.

**Representative stated items:**

- [kamel2020simulation] Future work: an intelligent Misbehavior Authority to evaluate global investigation, reporting policies and report format.

<details><summary>All 1 source ids</summary>

kamel2020simulation

</details>

## 4. Richer attacker models: colluding, adaptive, insider, multi-field, new attacks

**Papers raising it:** 59 (high relevance: 24)

**Open problem:** Attacker models are too simple: colluding, adaptive, insider (credentialed), multi-field falsification and false-report injection are called for.

**Bearing on the engine:** Engine can execute insider attacks with valid SCMS credentials in the loop.

**Representative stated items:**

- [heijden2017analyzing] Future work: other controllers; falsifying multiple beacon fields simultaneously (evades typical MBD).
- [heijden2018veremi] A dataset has an inherent limitation: attacker and benign deviations are unknowable in advance (e.g. urban GPS errors, adaptive attackers).
- [kamel2020simulation] Future work: colluding attackers and false report injection.
- [ardakani2022fuzzy] Evaluate with different scenarios and attacker models.
- [ercan2022misbehavior] Future work: other attack types (DoS, DDoS, Sybil, black hole).
- [li2025detecting] New attack forms may emerge

<details><summary>All 59 source ids</summary>

adom2025leveraging, alsamiri2025mccs, ardakani2022fuzzy, baccari2021secure, balaji2024metaheuristic, ceviz2024survey, colhak2025accelerating, corser2013endpoint, dang2025sdn, demir2025targeted, ercan2022misbehavior, farivar2021covert, heijden2017analyzing, heijden2018veremi, huang2024aill, ji2025outline, ka2026day, kamel2020simulation, kang2019secure, korba2024federated, kumbhare2026comprehensive, li2019speed, li2021unlinkable, li2024cyberattacks, li2025detecting, lilhore2026hybrid, mamun2025adversarial, manaseer2024secure, maranur2021arpvp, mohcine2025hidden, oli2026deep, oza2020secureb, placzek2017detection, rai2026detection, ren2024stealthy, richeson2025ensemble, roongpraiwan2024digital, saharuna2025leveraging, saranya2015reputation, saranya2026optimized, sarker2020advanced, sarker2021context, sarker2021efficient, sarker2021suspicion, sarsam2023cybersecurity, saudagar2023attack, sedar2024deep, sen2024adversarial, shagor2026post, sharshembiev2019fail, shuvo2025phantom, suganyadevi2023efficient, teli2021resilience, turgut2026ipek, twardokus2021intelligent, voon2026trust, yang2021risk, yang2026insider, zuo2026lightweight

</details>

## 5. Robustness of ML detectors to adversarial/evasion attacks

**Papers raising it:** 21 (high relevance: 8)

**Open problem:** Robustness of ML detectors against evasion and poisoning is under-explored.

**Bearing on the engine:** Closed-loop adaptive attackers against detectors; benchmark task.

**Representative stated items:**

- [ajibuwa2023survey] vulnerability to adversarial ML
- [alkifaee2026machine] Adversarial robustness under-explored; evaluation fragmented across datasets; proxy IT datasets not valid evidence.
- [demir2025distributed] Future: targeted poisoning, backdoor attacks, defence mechanisms.
- [hakeem2025advancing] Adversarial robustness and computational cost remain challenges.
- [sedar2026deep] DRL vulnerable to strategic missingness-based evasion.
- [youness2025vemisnet] potential adversarial vulnerability

<details><summary>All 21 source ids</summary>

ajibuwa2023survey, alkifaee2026machine, bataineh2024detecting, demir2025distributed, fatima2026privacy, hakeem2025advancing, hassan2026ai, kumbhare2026comprehensive, mansourian2025enhancing, marchiori2024canederli, mazouzi2026block, medam2026autonomous, nakayiza2026purefl, oli2026deep, ranjan20266g, sarker2021suspicion, sedar2026deep, taheri2025enhancing, wang2024data, youness2025vemisnet, zhong2023tackling

</details>

## 6. Evaluation metrics and benchmarks: comparability, impact-based metrics

**Papers raising it:** 66 (high relevance: 25)

**Open problem:** Evaluation metrics are not comparable or not meaningful: precision/recall do not capture physical impact; heterogeneous evaluations; aggregate errors hide what-if bias.

**Bearing on the engine:** Impact-based and counterfactual metrics (new direction N1).

**Representative stated items:**

- [heijden2017analyzing] Metrics bound to the leader's behaviour.
- [heijden2018veremi] Metrics need more study from a security perspective; PR curves do not give the full picture.
- [malik2025security] benchmark covers jamming against CACC only.
- [agudelo2026trustworthy] Aggregate error metrics hide biases under what-if scenarios.
- [ajibuwa2023survey] Lack of objective evaluation and feasibility metrics
- [dewangan2026misbehavior] Need Byzantine-resilient FL, XAI, interoperable standards-aligned detection, benchmarks with latency and robustness metrics.

<details><summary>All 66 source ids</summary>

agudelo2026trustworthy, aji2025implementation, ajibuwa2023survey, alalwany2024security, alshareeda2025efficient, alwash2026fl, ayyamgari2026authentication, aziz2023reduction, bastola2025anomaly, beltiukov2025demystifying, bharanidharan2024detection, debnath2025kedennet, dewangan2026misbehavior, diop2025hybrid, eziama2026enhanced, fatima2026privacy, fei2024privacy, george2026driveface, guemespalau2026simulation, gundeti2026security, hao2024rcooper, happe2025benchmarking, hasan2026digital, he2019indraj, he2025survey, heijden2017analyzing, heijden2018veremi, hu2026llm, huisman2023impact, ilic2026fto, khan2021security, khodaei2020cooperative, korade2026sumo, kuntegowda2026quantum, li2025largeb, li2026transformer, lin2025fte, linh2026datz, lu2026wirelessopsagent, m2025lightweight, malik2025security, n2025quantum, patel2026predictive, pegurri2026predicting, pokhrel2026quantum, queiros2023raterl, quraishi2025dynamic, ropitault2025enabling, rusek2019routenet, saharuna2025leveraging, schofield2025beyond, sharmin2024benchmarking, silva2025machine, solida2026demystifying, stenhammar2026advancing, t2026quantum, tang2025i2nav, thirumalaisamy2023unlocking, torresfigueroa2020qos, turgut2026ipek, turnip20266g, twabi2026netagentbench, ukkusuri2025cybersecurity, wang2026empirical, yelure2024impact, zhou2025netarena

</details>

## 7. Training/evaluation data scarcity, realism and dataset artefacts

**Papers raising it:** 98 (high relevance: 42)

**Open problem:** Data for training and evaluation is scarce, synthetic, imbalanced or flawed (leakage, shortcuts); sim-to-real gap.

**Bearing on the engine:** Benchmark with per-message joint ground truth and sender-disjoint splits (direction 3).

**Representative stated items:**

- [heijden2018veremi] A dataset has an inherent limitation: attacker and benign deviations are unknowable in advance (e.g. urban GPS errors, adaptive attackers).
- [hermann2026veremi] No RSSI in dataset.
- [abdelhakeem2026pq] Mostly synthetic data (stated composition).
- [alkifaee2026machine] Adversarial robustness under-explored; evaluation fragmented across datasets; proxy IT datasets not valid evidence.
- [almajed2025securing] Training data imbalance; few real-world applications; need datasets reflecting real situations.
- [almekhlafi2024integrating] Veins constraints: simulation-to-reality gap, computational complexity, integration with emerging technologies

<details><summary>All 98 source ids</summary>

abdelhakeem2026pq, abdullahi2024distributed, abrar2024gps, ajjaj2023incremental, alayedi2026cross, alekszejenko2024v2x, alkifaee2026machine, alluhaibi2025quantum, almajed2025securing, almekhlafi2024integrating, amanullah2022burst, amaouche2023fscb, babbar2024effective, bataineh2024detecting, bekele2025deep, beltiukov2023netfound, beltiukov2025demystifying, bouton2026mobile, cui2026fedauth, dewangan2026misbehavior, dina2025fsprototransfer, eziama2026enhanced, ferriolgalmes2022routenet, ferriolgalmes2022routenetb, g2024hybrid, george2026driveface, ghaleb2020misbehavior, gioacchini2024cross, goncalves2021evaluation, guo2025largeb, hakeem2025advancing, hamhoum2024mistralbsm, hammi2022it, heijden2018veremi, helm2023data, hermann2026veremi, hernangomez2022berlin, houidi2022neural, houts2020ford, iandolo2026camasa, idris2024machine, iqbal2022simulating, jang2026detecting, kapoor2025improved, kapoor2026class, karyemsetty2023intrusion, khalil2024artificial, khan2025context, kidmose2025sleuth, kim2024verifying, kovacs2023implementation, kures2025cesnet, li2025advancing, li2025dmtas, li2026blichain, liu2026spatio, mahmud2025integrating, mane2025securing, muktar2026reproducible, n2025misbehavior, najem2025review, narayan2026z, patel2026predictive, pavithra2025intrusion, pinto2023ciciot2023, ranjan20266g, ravikumar2026advanced, ricard2019autobot, ruah2023bayesian, sarker2020advanced, sarker2021efficient, saudagar2023attack, sbai2026addressing, slama2022misbehavior, slama2023one, solida2026demystifying, tang2025i2nav, tassi2019dataset, wang2025collaborative, wang2026vehdyn, wei2025survey, wu2024smart, xiao2025lsha, xie2023efficient, xu2023cgan, xu2023traffnet, yakan20235g, yang2022deepqueuenet, yang2025uniplanner, yazgan2024collaborative, yin2025btrformer, yuan2025generative, zhao2022mt, zhao2025language, zheng2026prism, zhi2025multi, zhou2019evaluation, zhu2024sybil

</details>

## 8. Generalisation of learned models beyond the training distribution

**Papers raising it:** 41 (high relevance: 15)

**Open problem:** Learned models (detectors, network surrogates such as RouteNet) do not generalise to unseen topologies, loads or environments.

**Bearing on the engine:** Surrogate/world model trained across counterfactual sweeps (direction 3).

**Representative stated items:**

- [yoshizawa2026zero] Experiments on a single vendor's development platform; cannot generalise to other OBUs.
- [amro2018protecting] Generalise model to other location-based services.
- [dhamala2024performance] RouteNet's accuracy drops on divergent distributions and larger networks.
- [farreras2023improving] Generalisation to larger topologies remains the core limitation of GNN models.
- [happ2022exploring] New methods needed to cope with traffic loads and network sizes significantly different from training.
- [heijden2019survey] Future work: exploit orthogonality of the four detection classes; generalise to CPS.

<details><summary>All 41 source ids</summary>

alagappan2026cradle, alwash2026fl, amro2018protecting, azzam2025ddos, baccari2021secure, baek2025deep, baharlouei2026generalization, bindu2024federated, c2025anomaly, cai2025tutorial, dhamala2024performance, elsayed2020machine, farreras2023improving, happ2022exploring, heijden2019survey, hou2024if, kapoor2025improved, kumar2026gpu, li2020traffic, li2023among, mahmoudi2025generative, mareta2025beyond, muller2019trust, palaios2023machine, patel2026predictive, pavlitska2026systematic, rajesh2024efficient, rusek2019routenet, rusek2019unveiling, sinha2025targeting, thomas2026passive, tomas2026road, voon2026trust, wei2025survey, xia2026generating, yoshizawa2026zero, zaidi2018overview, zhong2023tackling, zhou2025reducing, zhou2026trafficb, ziazet2022addressing

</details>

## 9. Fidelity, uncertainty and guarantees of learned surrogates / world models / digital twins

**Papers raising it:** 81 (high relevance: 22)

**Open problem:** Learned surrogates, world models and digital twins lack fidelity guarantees, uncertainty handling and validation; prediction horizons are short.

**Bearing on the engine:** Exact counterfactual reruns as validation ground truth for surrogates and twins.

**Representative stated items:**

- [bello2026human] Channel is an analytical surrogate whose absolute delivery ratios need field validation.
- [heijden2017blackchain] Guarantees of hierarchical versus full consensus.
- [ahmad2025opencams] Future: 5G NR sidelink, ROS2 stacks, real-time GNSS spoofing models, MEC, real-world data streaming for DT validation.
- [bouton2026mobile] Open-loop emulation on real data rather than live deployment.
- [escher2021well] Results suggest C-ITS pseudonym scheme alone does not guarantee location privacy
- [eslami2026resilient] Asynchronous switching, delays and packet loss, actuator saturation, heterogeneous parameters, stochastic noise, CBF collision guarantees, higher-fidelity and hardware validation.

<details><summary>All 81 source ids</summary>

abdelsalam2024digital, adom2025leveraging, ahmad2025opencams, alamoudi2026digital, aljanabi2024d, almeaibed2021digital, anisi2025distributed, anonndprivacyb, bande2026ai, bao2021review, bello2026human, bhatti2021future, blasco2023predictive, bouton2026mobile, brinzea2026blockchain, campbell2025hybrid, ceapa2026iot, challoumis2026economic, cheng2022stc, cui2025false, dawson2025zero, elsayed2021edge, escher2021well, eslami2026resilient, ezhilarasu2019understanding, fan2024trust, faye2025functional, feng2025survey, ferriolgalmes2022routenet, gu2026digital, guo20213d, hager2015literature, hasan2026digital, he2020secure, heijden2017blackchain, hong2025digital, hou2024if, khosravirad2026tiers, kousaridas2021qos, kuruvatti2022empowering, li2020deep, li2025intelligentb, liang2018filter, liang2019filter, liang2024physically, liu2025dwm, lu2022secure, lv2021artificial, mohanraj2024digital, moreira2020qos, oli2026deep, pegurri2026predicting, perrone2025generative, pourkeshavarz2026autoworld, rai2026detection, rezaei2023digital, roongpraiwan2024digital, ruah2023bayesian, safaei2026detecting, sengupta2026adaptive, shah2021rve, shi2023constructing, song2025traffic, tan2023digital, tang2025digital, thomas2026passive, trantruong2026systematic, vuorinen2025multimodal, wang2022next, wang2026emaca, wu2024digital, xie2022digital, xiong2025enhancingb, yang2021robust, yang2026near, yu2025digitalb, zhang2022trust, zhang2026topology, zhao2025edge, zhao2026edge, zhou2026study

</details>

## 10. Causal and counterfactual validity: assumptions, RCTs, validating estimates

**Papers raising it:** 9 (high relevance: 6)

**Open problem:** Causal and counterfactual estimates need randomised data or strong assumptions and cannot be validated without ground truth; the causal effect of MBD in the field is 'still elusive'.

**Bearing on the engine:** Direction 3; exact same-seed reruns give per-unit counterfactual truth.

**Representative stated items:**

- [wehmer2023fast] Quantification of MBD effect in real systems is 'still elusive'; the causal design is a proposal, not executed.
- [agudelo2026trustworthy] Aggregate error metrics hide biases under what-if scenarios.
- [alomar2022causalsim] Analytical recovery method relies on exact low rank, is brittle with approximate rank, applies only to discrete actions, and gives sufficient but not necessary conditions.
- [qchohi2026confounding] Randomised telemetry is scarce because it disrupts operation.
- [schofield2025beyond] Standard metrics do not capture causal sensitivity.
- [abenathar2025network] Future: compare algorithms under congestion and variable traffic, include response time, precompute what-if analyses.

<details><summary>All 9 source ids</summary>

abenathar2025network, agudelo2026trustworthy, alomar2022causalsim, bai2025privacy, qchohi2026confounding, rahimi2023sim, schofield2025beyond, wehmer2023fast, yokoyama2024causal

</details>

## 11. Simulator coupling, fidelity, determinism and reproducibility

**Papers raising it:** 82 (high relevance: 35)

**Open problem:** Simulator coupling, fidelity, scalability and reproducibility: co-simulation latency and synchronisation, Veins biases against field data, cost of replications, realism changing conclusions.

**Bearing on the engine:** Single-process deterministic engine; new direction N3 (conclusion robustness).

**Representative stated items:**

- [baharlouei2022exploring] Future research directions arise from their observations on simulation realism.
- [ming2018general] Challenges of combining packet simulators into one framework and other limitations reported.
- [scopelliti2024efficient] Choice of Tv trades revocation speed vs resources and desynchronisation
- [abdulhussain2013enhanced] Requires a vehicle movement simulator for results; cooperation protocol needs development and testing.
- [agustina2026lightweight] Future work: NS-3/SUMO-coupled realistic traffic scenarios (current evaluation lacks them).
- [almekhlafi2024integrating] Veins constraints: simulation-to-reality gap, computational complexity, integration with emerging technologies

<details><summary>All 82 source ids</summary>

abdulhussain2013enhanced, agustina2026lightweight, ahmed2025qsafe, aledhari2025safeguarding, almekhlafi2024integrating, alshareeda2025efficient, altmemi2025software, anisi2025distributed, arabi2021reinforcement, ashayer2025co, ashayer2026corridor, baharlouei2022exploring, bande2026ai, barthauer2018coupling, benfriha2023survey, bharanidharan2024detection, bharathi2023intensive, boban2011modeling, buse2019real, choi2026differentiable, dai2025lightweight, debnath2025kedennet, ferriolgalmes2022routenet, ghafarian2023review, gularte2024integrating, hu2026llm, ilic2026fto, ito2026vast, jamsahar2026modular, jat2024current, jena2025privacy, kamal2026deep, kang2016survey, khalil2024artificial, khattak2023evaluation, lee2020integrating, lee2026design, li2025m4, li2026transformerb, linh2026datz, liu2020blockchain, lu2019survey, luo2023srcon, mekonen2025detection, menal2025analyzing, meyer2021simulator, ming2018general, mohajer2026semantic, mohan2019ad, muktar2026reproducible, mundhenk2016open, murshed2024vehicular, nampally2017simulators, nardini2017simulating, nithiyanandam2023geometric, pan2025trustworthy, pechinger2024sumonity, queiros2023raterl, rahman2026digital, ranjan20266g, saghir2024comparative, schrader2023comparing, scopelliti2024efficient, sedjelmaci2025applied, selvakumar2026digital, sengupta2026adaptive, seymour2021empirical, sikarwar2026dybatch, silva2024realistic, simohammed2024ns, solida2026demystifying, stoner2019developing, tam2024survey, tang2025digital, tang2025i2nav, tomas2026road, twardokus2026cross, veras2026implementing, vieira2019realistic, vielberth2024elevating, zhang2024testing, zhu2026scenefactory

</details>

## 12. Radio realism and newer radio technologies (NR sidelink, 5G/6G, channel models)

**Papers raising it:** 127 (high relevance: 39)

**Open problem:** Radio realism and newer radio technologies: NR sidelink Rel-16/17, PSFCH/PSCCH, 5G/6G, channel models, MCS ranges; many security results are only for 802.11p.

**Bearing on the engine:** Engine has DSRC, LTE-V2X and NR-V2X stacks; runner-up R3.

**Representative stated items:**

- [abrar2026deployment] Future work: Falcon-512 on NR-V2X Rel-16/17.
- [bello2026human] Channel is an analytical surrogate whose absolute delivery ratios need field validation.
- [guven2025creating] Future work on building and landscape data effects on propagation.
- [agustina2026lightweight] Future work: standardisation for 5G NR-V2X.
- [ahmad2025opencams] Future: 5G NR sidelink, ROS2 stacks, real-time GNSS spoofing models, MEC, real-world data streaming for DT validation.
- [almekhlafi2024integrating] future: Beyond-5G, AI, blockchain security, semi-virtual/hybrid simulation, wider V2X.

<details><summary>All 127 source ids</summary>

abdullahi2025interplay, abrar2026deployment, abreu2025cybersecurity, agustina2026lightweight, ahmad2025opencams, albalasmeh2026blockchain, albeyar2025ai, ali2026collaborative, almarshoud2024security, almekhlafi2024integrating, altmemi2025software, alzamili2026afog, arfvidsson2024small, astorga2022revisiting, bappy2026sword, bello2026human, benyamina2019anel, bintoro2025overview, bitsikas2025security, braeken20266g, cai2024task, chandolia2026ecc, chen2018security, chu2026map, dey2025secure, digregorio2025evaluating, ding2022digital, ding2022fully, eldalahmeh2025schnorr, eldowek2021survey, elshafai2025ai, emran2026battlefield, eziama2026enhanced, faye2025functional, fu2025generalized, fu2025key, gaba2020robust, ghelani2024gradient, gorbenko2024evaluation, govindam2026deepprobboost, guo2025largeb, guven2025creating, h2025resilient, habib2025hierarchical, hager2015literature, henderson2025link, hsu2022practical, huang2025llm, imran2020ieee, ismail2026design, j2025post, jiao2024advanced, kamal2026deep, kaur2024security, khan2024ai, khosravirad2026tiers, khuwuthyakorn2025federated, kousaridas2021qos, krivokapic2023authenticated, kuruvatti2022empowering, lautenbach2019preliminary, lee2024autonomous, li2022improving, li2023v2v, li2026blichain, liu2025foundation, lochbihler2023misbehaviour, luo2022scalacert, ma2026semantic, mafakheri2021optimizations, makvandi2026adaptwin, malhi2020security, maller2022cloud, mamun2026post, manale2022security, mazouzi2026block, mekala2026secured, mohajer2026semantic, muktar2025machine, muktar2026reproducible, palash2024benchmarking, patel2024xai, paula2015performance, penell2026gross, pirayesh2021jamming, pott2021firmware, qu2025llm, r2024cross, rezazadeh2024intelligible, rkhami2021mongnn, ropitault2025enabling, salehi2024multiverse, schwarzmann2022ml, sedjelmaci2025robust, sharma2020security, shaw2026quantum, shawky2023efficient, sliwa2020cooperative, solida2026demystifying, song2025hybrid, song2026lightweight, sowa2024post, stenhammar2026advancing, suleman2026simulated, tassi2019dataset, tran2025network, turnip20266g, twardokus2021intelligent, twardokus2026cross, uler2022efficient, urquizaaguiar2015impact, vinita2026hierarchical, wang2021deep, wang2022privacy, wang2023blockchainenhanced, wang2025efficient, wang2025observer, xiao2025lsha, xiong2025enhancingb, yang2026ai, yuan2026zero, zhan2024dynamic, zhang2026theoretical, zhao2025privacy, zhao2025unified, zhou2025ai, zhou2025reducing

</details>

## 13. Post-quantum migration: size, packet limits, hybrid schemes and crypto agility

**Papers raising it:** 71 (high relevance: 24)

**Open problem:** Post-quantum migration: certificates and signatures do not fit packet limits; hybrid and congestion-adaptive schemes need standard changes; crypto agility; PQ butterfly keys and implicit certificates remain unsolved; system-level validation under realistic traffic is missing.

**Bearing on the engine:** Direction 2; engine has PQ and hybrid signatures in the SCMS.

**Representative stated items:**

- [abrar2026deployment] Future work: Falcon-512 on NR-V2X Rel-16/17.
- [boldyreva2024provable] Achieving post-quantum security of the protocol is future work
- [mushrall2018emulab] butterfly key expansion not post-quantum
- [twardokus2024cryptography] embedding PQ certificate and signature in one frame is too large
- [yoshizawa2026zero] Hybrid mode (ECDSA + PQC in certificate and message) has significant size implications.
- [bindel2022need] No successful lattice implicit certificate construction

<details><summary>All 71 source ids</summary>

abdullahi2025interplay, abrar2026deployment, agarwal2026hybrid, ahmed2025qsafe, alayedi2026cross, ali2026collaborative, alkhalaf2025hmac, almarshoud2024security, alrashdi2025enhanced, amadi2026symmetricasymmetric, bindel2022need, boldyreva2024provable, buruaga2025benchmarking, datt2025q, diazgorrin2026enhancing, eichen2026practical, enden2025quantum, fatima2026privacy, fezeu2025non, godara2026quantumresilient, gullu2026post, gupta2026hybrid, h2025resilient, han2020privacy, hildebrand2022comprehensive, j2025post, jena2025privacy, jiao2021anti, karinsalo2026migrating, kumar2026gpu, kuntegowda2026quantum, li2024optimization, lonc2023feasibility, mamun2026post, marchsreiter2026improving, medam2026automotive, mohamed2025post, mumtaz2026comprehensive, mushrall2018emulab, narkedimilli2026advances, okafor2026quantum, ouyang2026application, pasupuleti2026quantum, pokhrel2026quantum, r2024cross, r2024elliptic, rahoof2026ai, rakhmanovich2025quantum, roy2026security, sadat2025secure, setyowati2025performance, shaw2026quantum, sowa2024post, tiwar2025pqc, trantruong2026systematic, tsai2025lattice, turaev2026adaptive, turnip20266g, twardokus2024cryptography, twardokus2026cross, wang2025efficient, wei2025lattice, yang2026ai, yoganand2026enhancing, yoshizawa2026zero, zeydan2022post, zeydan2022postb, zhang2026theoretical, zhao2024mika, zhao2026puf, zhou2025ai

</details>

## 14. Cryptographic overhead: computation, verification load, latency, energy

**Papers raising it:** 162 (high relevance: 35)

**Open problem:** Cryptographic overhead (computation, verification load, latency, energy) on constrained OBUs and its effect on timeliness.

**Bearing on the engine:** Direction 2 verification policies; R3 verification-load attacks.

**Representative stated items:**

- [petho2024investigating] Increased processing time is unsafe only if QoS drops below acceptable levels.
- [almutairi2024systematic] Gaps: renewable energy, recharging, encryption-effect simulation.
- [altmemi2025software] Gaps: scalability, computational overhead, limited 5G/blockchain/AI integration
- [chandolia2026ecc] Excludes physical layer, end-to-end latency and transmission delay from analysis.
- [dewangan2026misbehavior] Need Byzantine-resilient FL, XAI, interoperable standards-aligned detection, benchmarks with latency and robustness metrics.
- [hakeem2025advancing] Adversarial robustness and computational cost remain challenges.

<details><summary>All 162 source ids</summary>

abdelmaguid2026beyond, abhishek2021drive, ahasan2025securing, alam2026daire, alamoudi2026digital, alayedi2026cross, albalasmeh2026blockchain, alluhaibi2025quantum, almansour2025adaptive, almekhlafi2024cla, almuhaideb2022efficient, almutairi2024systematic, alrashdi2025enhanced, alsaadi2026privacy, altmemi2025software, anonndprivacyb, arshad2018review, attiaoui2025blockchained, awan2022blockchain, ayyamgari2026authentication, aziz2020secureais, badhesha2025efficient, bai2026deep, bappy2026sword, benguenane2023mitigating, benyamina2019anel, bharanidharan2024detection, bordin2021international, buhk2026simulation, byeon2026v2x, cebe2020communication, chandolia2026ecc, chiang2026lightweight, choi2025puf, chu2026map, cui2026fedauth, demir2025targeted, dewangan2026misbehavior, ehsani2025vehicle, eichen2026practical, eldalahmeh2025schnorr, eldalahmeh2026eliminating, eldalahmeh2026lightweight, elsayed2021edge, erskine2026machine, eziama2026enhanced, farzanullah2026pera, feng2026efficient, gantayat2026enhancing, gao2025efficient, ghosh2026ep, gullu2026post, hakeem2025advancing, halgamuge2024malicious, hamdaoui2025h, huang2026enhancing, hurl2020trupercept, iftikhar2026b, ismail2026design, ito2026vast, javaid2025leveraging, jeffrey2023review, jeyaram2026lightweight, jiang2024personalized, k2026lightweight, kamal2026deep, kapoor2025machine, khalid2025safe, khodaei2020cooperative, kumar2025centralised, kumar2026gpu, kunimoto2026v2x, kuntegowda2026quantum, lal2018secured, lashari2025lightweight, lekidis2024anomaly, li2025attentionguard, li2025lightweightb, li2026blichain, liang2025traceable, liu2026ffha, mahmood2021digital, mahmud2025integrating, makkar2025quantum, malina2021post, mamun2024enhancing, marojevic2018c, mazouzi2026block, mohajer2026semantic, mumtaz2026comprehensive, murugan2025study, mutalik2023trust, nampally2017simulators, nedungadi2026fedx, patel2026predictive, pavuluri2025enhancing, petho2024investigating, pokhrel2026quantum, pougajendy2016cdai, qaisar2026robust, quan2026closed, qureshi2022authentication, qureshi2026blockchain, ranjan20266g, raut2021seek, ravikumar2026advanced, roongpraiwan2024digital, s2026integrated, sadiq2020blockchain, salehi2024multiverse, salem2025reputation, selvakumar2026digital, senthil2025blockchainenhanced, setyowati2025performance, shi2026lifecycle, silva2022acpc, sinha2025targeting, sliwa2019lightweight, sonker2026differentially, stenhammar2025clustering, tam2024survey, tangade2018decentralized, tiwar2025pqc, tiwari2024beyond, trabucco2025network, trantruong2026systematic, tsai2025lattice, tuncel2026budacvab, turnip20266g, twardokus2026cross, u2026energy, u2026green, ullah2025decentralized, veras2026implementing, wan2023imct, wan2025pbru, wang2024lightweightb, wang2025research, wang2026hrc, wang2026seba, wang2026withdrawn, weerasinghe2023threshold, wickramasurendra2025bert, wong2025sustainable, xia2022data, xiao2025lsha, xing2026enhanced, yang2026near, ye2025reputation, youness2025vemisnet, younis2022evaluation, yu2024ensuring, yuan2025generative, zhang2023traffic, zhang2026online, zhao2025privacy, zhao2026eaba, zheng2026prism, zhou2025ai, zhou2025reducing, zhou2026efficient, zhou2026trafficb

</details>

## 15. Pseudonym change strategies, linkability and the privacy-safety trade-off

**Papers raising it:** 196 (high relevance: 43)

**Open problem:** Pseudonym change strategies, linkability and the privacy-safety trade-off are 'still an open issue'; how many pseudonyms to store; multi-authority PKI privacy.

**Bearing on the engine:** Runner-up R1; engine has pseudonym provisioning and ground truth.

**Representative stated items:**

- [anon2013vespa] Integrate privacy-preserving methods and anonymous authentication into protocols.
- [eckhoff2016marrying] Existing privacy measures often conflict with the requirements of future vehicular networks.
- [kamel2020simulation] Future work: more pseudonym change strategies (silent periods), attacks and detectors.
- [khodaei2016evaluating] Investigate pseudonym utilisation with various refill intervals and lifetimes versus server workload and privacy.
- [scopelliti2024efficient] future: privacy-preserving blocking of re-joining after revocation without hurting honest vehicles that lose connectivity.
- [akil2023non] Notes infrastructure-based schemes need RSUs, CRLs via RSUs and pseudonym reuse in rural areas.

<details><summary>All 196 source ids</summary>

abdullahi2025interplay, adom2025leveraging, agarwal2026hybrid, ahsan2024privacy, akil2023non, aladwan2020truste, alayedi2026cross, albeyar2025ai, alhassan2025intrusion, ali2026collaborative, ali2026deep, almohtaseb2024comprehensive, alwash2025effective, alzamili2026afog, anjali2025x, anon2013vespa, anon2018blockchain, anon2025enhanced, anonndprivacyb, anwar2024dynamic, arif2019survey, arshad2023blockchain, asaju2024addressing, asim2026vae, attiaoui2025blockchained, babaghayou2021whisper, badhesha2025efficient, bai2025privacy, bekele2025deep, bindu2024federated, bintoro2025overview, boualouache2017survey, boudagdigue2020cluster, brinzea2026blockchain, byeon2026v2x, byun2025blockchain, c2025anomaly, cavus2025advances, chelghoum2023btv2p, chen2025anonymous, chen2025secure, cui2026fedauth, d2026efficient, deng2022pcp, diazsanchez2025beyond, didouh2022novel, duan2025simulation, eckhoff2010strong, eckhoff2011slotswap, eckhoff2014open, eckhoff2016marrying, eldalahmeh2025schnorr, eldalahmeh2026lightweight, eldefrawy2019key, emara2013location, emara2016context, enden2025quantum, escher2021well, eziama2020detection, farivar2021security, fatima2026privacy, fayi2023survey, fezeu2025non, forster2018evaluation, gantayat2026enhancing, gao2025efficient, giannaros2023autonomous, gong2025lightweight, guo2024adaptive, gupta2025extensive, hassan2026ai, he2025dino, helal2024smart, herbke2024decentralized, huang2020secure, idris2024novel, iftikhar2026b, jan2021survey, jena2025secure, jia2020survey, jin2023enhancing, k2026lightweight, kamel2020simulation, kandhro2023artificial, kang2016location, kawale2026dandelion, khodaei2016evaluating, khodaei2020cooperative, khodaei2020key, kong2017achieve, lashari2025lightweight, leaby2022secure, li2021unlinkable, li2022vehicular, li2025dmtas, li2026blichain, li2026privacyb, liang2025traceable, lin2024efficient, liu2020blockchain, liu2025efficientb, liu2026enhanced, liu2026ffha, liu2026lsans, liu2026spatio, lu2018bars, lu2018privacy, lu2019survey, luo2025privacy, luo2026traceable, m2023optimized, ma2023location, ma2026privacy, mareta2025beyond, mazouzi2026block, meshram2026bsrp, mianji2025enhancing, mittal2025ai, mohajer2026semantic, muid2025accurevoke, nakayiza2026purefl, nedungadi2026fedx, noh2020distributed, pham2018adaptive, priyanka2023signature, qi2020privacy, qureshi2026blockchain, ranjan20266g, rashmi2025beyond, riad2022blockchainbased, roy2015survey, roy2026security, s2025optimizing, s2025survey, sadiq2020blockchain, safavat2021elliptic, salem2025reputation, saravanakumar2025beetleoptimized, sathyan2025fedsecureformer, satya2026federated, saxena2017network, scopelliti2024efficient, sedar2022multi, sengupta2025privacy, senthil2025blockchainenhanced, sernaolvera2013trust, shi2026lifecycle, silva2022acpc, sonker2026differentially, suganyadevi2023efficient, sui2026eclas, sun2026revocable, tan2023digital, tangade2018decentralized, thirumalaisamy2023unlocking, tiwari2024beyond, trantruong2026systematic, turnip20266g, ullah2022hdl, ullah2025location, wan2025pbru, wang2016tracking, wang2022privacy, wang2024privacyoracle, wang2025collaborative, wang2026certificateless, wang2026efficientb, whitefield2017formal, wu2025certificateless, wu2026look, wyk2020real, xie2023efficient, yadav2026privacy, yadav2026trust, yadavndintegrated, yang2010location, yang2023trustworthy, yang2026certificateless, yang2026insider, yao2023vdkms, yu2024efficientb, yu2024efficientc, yuan2024real, yuan2025generative, zehra2026building, zhai2025blockchain, zhang2023generative, zhang2023traffic, zhang2026edge, zhang2026pidtree, zhang2026privacy, zhao2022pbtm, zhao2023imf, zheng2020service, zhou2024federated, zhou2025clogging

</details>

## 16. Standards gaps, interoperability and multi-domain / cross-border operation

**Papers raising it:** 74 (high relevance: 23)

**Open problem:** Standards gaps and interoperability: cross-OEM and cross-border operation, multi-domain PKI, standardised MDS, NR-V2X security standardisation, implicit certificates across IEEE/ETSI/C-SCMS.

**Bearing on the engine:** Runner-up R2 (multi-provider SCMS).

**Representative stated items:**

- [chen2026comparison] Future work: implement implicit certificates in CCMS and C-SCMS to compare all three standards with implicit certificates.
- [jha2018pseudonym] Proposes extending SCMS with cross-certification for validation across individualised trust roots
- [robertndsegmented] Future work includes experiments in V2V test sites beforeintroducing to the standard.
- [agustina2026lightweight] Future work: standardisation for 5G NR-V2X.
- [ansari2021v2x] Open challenges for standardisation and research to achieve secure CPS (discussed).
- [boualouache2020software] ETSI technical report highlights absence of standardised and efficient PCS.

<details><summary>All 74 source ids</summary>

agustina2026lightweight, alayedi2026cross, altimemi2025fs, an2024malicious, ansari2021v2x, aziz2020secureais, azogu2013anti, azzam2025ddos, bande2026ai, borges2025self, boualouache2020software, boualouache2022survey, braiteh2025platooning, brinzea2026blockchain, cavus2025advances, chen2026comparison, dewangan2026misbehavior, dias2025have, eldalahmeh2025secure, emran2026battlefield, faye2025functional, fezeu2025non, garlichs2018realizing, guo2025largeb, hammi2022it, hassan2026ai, he2025survey, ito2026vast, jeffrey2023review, jha2018pseudonym, kaiser2021white, kaniwa2024exploring, karinsalo2026migrating, kheireddine2024securing, kumar2025explainable, kuntegowda2026quantum, li2025edge, li2025largeb, li2026blichain, liang2025traceable, liu2018achieving, lonc2023feasibility, mamun2026post, menal2025analyzing, mohamed2025post, monteuuis2022v2x, naeem2026adaptive, narkedimilli2026advances, pokhrel2026quantum, queiros2023raterl, qureshi2026blockchain, ranjan20266g, robertndsegmented, ropitault2025enabling, roy2026experimental, s2026integrated, sadat2025secure, saharsh2025survey, schofield2025beyond, schuette2005hardware, shagor2026post, struder2018flexible, suleman2026simulated, tam2024survey, turaev2026adaptive, twardokus2026cross, u2026energy, u2026green, ukkusuri2025cybersecurity, wang2025collaborative, yavuz2023beyond, yoshizawa2024viable, zeydan2022post, zhou2025clogging

</details>

## 17. Trust-model parameterisation: thresholds, weights, bootstrap

**Papers raising it:** 96 (high relevance: 20)

**Open problem:** Trust and detection thresholds, weights and calibration are static or hand-tuned; adaptive thresholds are proposed without safety evaluation.

**Bearing on the engine:** Direction 2 (detection thresholds as a safety knob).

**Representative stated items:**

- [kerner2008testbed] Accuracy/time trade-off via reception threshold.
- [ghaleb2019ensemble] Static thresholds are a recognised limitation of prior work (addressed here).
- [ghaleb2022fuzzy] Future: ML to adapt detection threshold to context
- [haddadou2015job] Nodes have limited reputation information beyond direct neighbours (noted).
- [hamdan2019detecting] Speed threshold not fixed; depends on street nature, nodes and lanes.
- [hurl2020trupercept] future: role-based trust (e.g. police cars), incentives for honest reporting, clustering of objects to reduce information, central server reputation, Bayesian/POMDP trust models.

<details><summary>All 96 source ids</summary>

abdelmaguid2026beyond, ahmad2020trust, ahmed2022privacy, aladwan2020truste, alam2026daire, ali2026enhancing, almansour2025adaptive, alwash2026fl, attiaoui2025blockchained, badhesha2025efficient, benfriha2023survey, che2022trust, chiang2026lightweight, chin2025technical, choi2025puf, chu2026map, cui2026fedauth, dahiya2020feedback, ehsani2025vehicle, ekpo2026hybrid, eldalahmeh2025schnorr, ghaleb2019ensemble, ghaleb2022fuzzy, guleng2019decentralized, gullu2026post, gupta2025extensive, haddadou2015job, hamdan2019detecting, han2024trust, han2026gnn, he2026glst, huang2022detection, hurl2020trupercept, ilic2026fto, iqbal2019trust, jasti2024multi, jeyaram2026lightweight, ka2026day, kerner2008testbed, khalid2025fedcrossvan, khan2019secure, kuang2022gtms, lashari2025lightweight, lekidis2024anomaly, li2025lightweightb, li2025task, li2026transformer, lomte2026intelligent, lotfi2024enhancing, lu2019survey, malhi2020security, mavromatis2017agile, mignone2024distributed, mohajer2026semantic, mumtaz2026comprehensive, mwanje2025misbehaviour, nafea2025blockchain, nagaraju2025trust, nakayiza2026puretrust, placzek2017detection, qaisar2026robust, quraishi2025dynamic, rana2025lightweight, rathore2023distinguishing, schrader2023comparing, sen2011reputation, sengupta2025privacy, shagor2026post, shahariar2024need, shahariar2026survey, shaikh2013intrusionaware, shen2025robust, shi2026lifecycle, siddiqui2021survey, sliwa2019lightweight, tsai2025lattice, uler2022efficient, ullah2025decentralized, voon2026trust, wang2023blockchainenhanced, wang2026withdrawn, wei2025data, wei2026game, wickramasurendra2025bert, wong2023real, xiao2025lsha, yang2026insider, yuan2025generative, zhang2012trust, zhang2021ait, zhang2025machine, zhang2025vehicle, zhang2026online, zheng2026prism, zhou2020distributed, zhou2026efficient

</details>

## 18. Quantified risk assessment and TARA (ISO/SAE 21434)

**Papers raising it:** 7 (high relevance: 3)

**Open problem:** TARA / ISO/SAE 21434 ratings are qualitative; attack trees become unmanageable; quantified feasibility and impact are called for.

**Bearing on the engine:** Runner-up R5.

**Representative stated items:**

- [luo2021threat] Discusses future development directions for automotive TARA.
- [monteuuis2022v2x] Notes attack trees become too large for this many attacks.
- [sheik2023securing] Need for future TARA work in the broader CCAV ecosystem

<details><summary>All 7 source ids</summary>

luo2021threat, marchiori2024canederli, marchiori2024work, monteuuis2022v2x, pavlitska2026systematic, sheik2023securing, xie2021threat

</details>

## 19. LLM / agent reliability: hallucination, human oversight, contamination, root cause

**Papers raising it:** 51 (high relevance: 13)

**Open problem:** LLM agents: reliability, need for human input, benchmark contamination, poor fault localisation and root-cause identification, scalability.

**Bearing on the engine:** Runner-up R4 (LLM cross-layer root-cause analysis).

**Representative stated items:**

- [zhang2026driving] Short horizon where alternative ego action does not alter other agents.
- [gao2026context] LLM-based simulation is time-consuming in complex environments (scalability).
- [hamhoum2024mistralbsm] Notes prior LLM MDS works risk training data leakage via message ID or pseudo-ID features.
- [mondal2023llms] Human input still required.
- [protogeros2026benchmarking] Evaluates intrinsic LLM capability only; RAG, prompting strategies and agentic systems left for future evaluation.
- [tseng2026fault] LLM-judge scoring of free-text diagnoses.

<details><summary>All 51 source ids</summary>

acharya2026ndt, alalwany2024security, ali2026enhancing, baimbetova2026pedestrian, chang2023survey, cheng2021general, chu2026map, ding2026routingbench, emami2025prompts, fan2025learning, farzanullah2026pera, feng2025network, gao2026context, gutl2026editorial, habib2025hierarchical, hamhoum2024mistralbsm, javaid2025leveraging, jiang2024personalized, khoshnevisan2025resilient, li2024security, li2025agentic, li2026agentic, liu2025model, mahmud2025integrating, mhamdi2022trust, mondal2023llms, muller2019trust, ouyang2026application, pourkeshavarz2026autoworld, protogeros2026benchmarking, rahimi2023sim, rezazadeh2024intelligible, sleiman2025impact, tseng2026fault, twabi2026netagentbench, wang2024llm, wang2025collaborative, wang2025network, wang2025robustness, wang2026large, wen2024defending, yan2026ctbench, yang2026ai, yokoyama2024causal, yuan2025llm, zhang2023cooperative, zhang2026autoonbench, zhang2026driving, zhang2026netinspector, zhao2025privacy, zhu2026scenefactory

</details>

## 20. Field validation, real-world deployment and hardware testing

**Papers raising it:** 264 (high relevance: 90)

**Open problem:** Results lack field validation, hardware profiling or real deployment.

**Bearing on the engine:** Engine validates against test vectors and SCMS Manager trust lists; field data (USDOT, NIST) available for calibration.

**Representative stated items:**

- [abrar2026deployment] Future work: extend J3161 MCS range; multi-intersection corridors; hardware profiling on automotive platforms.
- [bello2026human] Channel is an analytical surrogate whose absolute delivery ratios need field validation.
- [didouh2022blockchain] Future: proof of concept as a real traffic management server.
- [franceschini2024neural] More data and refinement needed before real-world use.
- [heijden2017analyzing] Future work: other controllers; falsifying multiple beacon fields simultaneously (evades typical MBD).
- [robertndsegmented] Future work includes experiments in V2V test sites beforeintroducing to the standard.

<details><summary>All 264 source ids</summary>

abasikelesturgut2024recent, abdullahi2025interplay, abdulmalik2025review, abrar2026deployment, adhiarja2022privacy, ahmad2025opencams, ahmed2025detection, ahmed2025qsafe, ajibuwa2023survey, alagappan2026cradle, aledhari2025safeguarding, alekszejenko2024v2x, alkifaee2026machine, alladi2022comprehensive, alluhaibi2025quantum, almajed2025securing, almarshoud2024security, altimemi2025fs, altmemi2025software, alwash2025effective, amaouche2025comparative, an2025adversarial, anjum2026mint, anon2025enhanced, anon2025securing, arshad2023blockchain, ashayer2025co, ashayer2026corridor, ayyamgari2026authentication, azfar2024traffic, azogu2013anti, azzam2025ddos, bajpai2023intrusion, balaji2024metaheuristic, barthauer2018coupling, basiri2019security, bastola2025anomaly, bello2026human, benfriha2023survey, bintoro2025overview, boualouache2017survey, boualouache2022survey, bourouis2026temporal, bouton2026mobile, braiteh2025platooning, brinzea2026blockchain, c2025anomaly, canada2019systeme, ceviz2024survey, charissis2008artificial, chaves2020emucd, chin2025technical, choi2025two, chowdhary2025comprehensive, chu2026map, cui2025fg, cui2026fedauth, cultice2022puf, dahiya2020feedback, dai2025lightweight, david2024innovative, dawson2025zero, desai2026multi, dewangan2026misbehavior, dickinson2019leveraging, didouh2022blockchain, digregorio2025evaluating, diop2025hybrid, dong2020impact, eldefrawy2019key, ercan2023enhanced, eslami2026resilient, eziama2026enhanced, faye2025functional, feng2025survey, ferriolgalmes2022routenet, foltz2020public, franceschini2024neural, ftaimi2025applying, gantayat2026enhancing, gelbal2020hardware, george2026driveface, ghafarian2023review, gomez2023survey, gullu2026post, h2025resilient, hakeem2025advancing, hamdaoui2025h, han2026gnn, hapanchak2022emulation, happe2025benchmarking, hassan2026ai, he2025dino, he2025survey, heijden2017analyzing, hermann2025demo, hildebrand2022comprehensive, hu2026multi, huang2025llm, huisman2025self, iandolo2026camasa, iqbal2025seeing, irfan2022reinforcement, j2025post, jan2021survey, jang2026detecting, jasti2024multi, javaid2025leveraging, jeffrey2023review, jena2025secure, ji2025enhanced, jiang2024graph, jiao2024advanced, jin2021emulating, jornod2019prediction, kamal2026deep, kamel2024blockchain, kanumalli2020secure, kapoor2025machine, kateshiya2015review, kayondo2026secv, khattak2023evaluation, khodaei2015key, kim2016effective, kim2024survey, kiruthika2024simuhack, kong2026ctng, kovacs2023implementation, kudva2021scalable, kumar2019detailed, kumar2026design, kumar2026q, kumbhare2026comprehensive, kuntegowda2026quantum, kuruvatti2022empowering, lecci2021ns, lee2023simulation, li2025robust, lin2021bcppa, liu2025foundation, liu2026lsans, lonc2023feasibility, lu2026wirelessopsagent, lv2021artificial, m2025lightweight, madhushri2025securing, makkar2025quantum, malina2021post, mamun2024enhancing, mamun2025adversarial, mamun2025experimental, mamun2026post, masaracchia2022digital, medam2026automotive, mekala2026secured, mekonen2025detection, meng2023advances, mesci2024blockchain, mianji2025enhancing, mihai2022digital, mohajer2026semantic, mundra2025advancing, nampally2017simulators, naqvi2024comparative, narkedimilli2026advances, nazat2024evaluating, nazat2025ensemble, nguyenminh2023machine, palaios2023machine, pan2025trustworthy, papanikolaountais2026agentic, park2009defense, park2013defense, patounas2015denial, pavlitska2026systematic, pechinger2024sumonity, qadir2025digital, ravi2021block, robertndsegmented, roy2026experimental, safavat2021elliptic, saleh2025agent, salehi2024multiverse, sbai2020data, schrader2023comparing, sedar2023comprehensive, sedar2024knowledge, sengupta2026adaptive, setyowati2025performance, shagor2026post, shah2021rve, shahariar2026survey, sharma2022cybersecurity, sharmin2025digital, shen2025robust, shim2022survey, silva2025machine, simohammed2025per, solaas2024systematic, solaas2024systematicb, song2025hybrid, stenhammar2026advancing, stenhammar2026joint, stute2020availability, suganyadevi2023efficient, sun2010mix, t2026quantum, tadesse2025sybil, tang2021fooling, tesei2018iota, trantruong2026systematic, tsai2025lattice, tuncel2026budacvab, turcanu2024integration, turgut2026ipek, twardokus2021targeted, tyagi2020trust, ukkusuri2025cybersecurity, v2026post, vaibhav2017security, veettili2024rule, vinita2026hierarchical, voon2026trust, wan2023imct, wang2017deep, wang2020physical, wang2022next, wang2023survey, wang2024digitalb, wang2025robustness, wang2026large, weerasinghe2023threshold, wellensmiles2025systematic, wen2024ddos, wickramasurendra2025bert, wu2024digital, wu2024smart, xia2026generating, xing2023survey, xing2026enhanced, xu2021opencda, xu2026sybil, yang2023discussion, yang2025uniplanner, yang2026ai, yin2025btrformer, ylmaz2025simulated, yoshizawa2026zero, youness2025vemisnet, yuan2025generative, zappone2019wireless, zhang2019uqsim, zhang2023learn, zhang2024deterministic, zhang2026securing, zhao2025communication, zhao2025edge, zhao2025unified, zhao2026edge, zhao2026puf, zhong2018simulation, zhong2023tackling, zhou2025stacked, zia2022ai

</details>

## 21. Preliminary, small-scale or single-scenario evaluation

**Papers raising it:** 35 (high relevance: 26)

**Open problem:** Evaluations are preliminary, small-scale or single-scenario.

**Bearing on the engine:** City-scale maps and sweeps.

**Representative stated items:**

- [bello2026human] Single stationary flooder attacker.
- [yoshizawa2026zero] Experiments on a single vendor's development platform; cannot generalise to other OBUs.
- [amanullah2022burst] Preliminary dataset.
- [chulerttiyawong2021blockchain] Small-scale simulation only.
- [dearing2024deep] Preliminary results.
- [ge2025iridescence] Trials preliminary.

<details><summary>All 35 source ids</summary>

amanullah2022burst, amirat2018fuzzy, bello2026human, bhupathiraju2024vulnerability, choi2025two, chulerttiyawong2021blockchain, dearing2024deep, ge2025iridescence, guo2012misbehavior, hagiya2021acceptability, helm2023data, hernangomez2022berlin, houssel2024explainable, jornod2020packet, lee2025convergence, li2025detecting, mohan2019ad, mondal2023llms, murthy2025detecting, oliveira2024maneuver, panchal2017segregation, protogeros2025continual, rusek2019unveiling, simohammed2025per, sliwa2020cooperative, su2024assessing, trabelsi2022multi, vogt2025poster, wang2025llmb, wang2025playground, xiang2023reproducing, yoshizawa2026zero, zhang2023learn, zhao2013simulation, zhou2026demo

</details>

## 22. Scalability to city scale, large fleets and long horizons

**Papers raising it:** 201 (high relevance: 50)

**Open problem:** Scalability: city scale, dense traffic, long horizons, computational cost.

**Bearing on the engine:** City-scale single-process engine.

**Representative stated items:**

- [malik2025security] Consensus reduces traffic density and relies on faultable sensors
- [zhang2024misbehavior] Future: runtime trust assessment simulation, realistic sensor data, ML MBD, edge trust MBD.
- [zhang2026driving] Short horizon where alternative ego action does not alter other agents.
- [agustina2026lightweight] Scales only to about 50 vehicles.
- [agyapong2026evaluating] Evaluate in large-scale and geographically diverse networks.
- [ahmad2026routenet] Designed for short horizons t+1 to t+3.

<details><summary>All 201 source ids</summary>

abdelmaguid2026beyond, abreu2025cybersecurity, abubakar2026securing, agustina2026lightweight, agyapong2026evaluating, ahmad2026routenet, alamoudi2026digital, alayedi2026cross, albalasmeh2026blockchain, alekszejenko2022validating, alluhaibi2025quantum, almekhlafi2024integrating, alshareeda2024ai, altmemi2025software, amadi2026symmetricasymmetric, anon2020modeling, anon2025enhanced, attiaoui2025blockchained, awan2025trustworthy, ayyamgari2026authentication, babbar2024effective, bekele2025deep, bhaskarsv2026distributed, bindu2024federated, blasco2023predictive, bordin2021international, brehongrataloup2025predictive, brinzea2026blockchain, cao2020network, chandolia2026ecc, chiang2026lightweight, chin2025technical, choi2025two, chu2026map, chulerttiyawong2021blockchain, correia2025performance, d2026efficient, dai2019big, dai2025lightweight, desai2026multi, diazsanchez2025beyond, dickinson2019leveraging, ding2022digital, diop2025hybrid, ekpo2026explainable, eldalahmeh2026eliminating, eldalahmeh2026lightweight, elsayed2021edge, farivar2021security, farokhi2017private, fathima2026comprehensive, gantayat2026enhancing, gao2019distributed, gao2025efficient, gao2026context, ge2023resilient, gelbal2020hardware, geller2024carlos, ghaleb2023intelligent, giannaros2023autonomous, gomez2023survey, greco2025detection, gullu2026post, gupta2025evaluation, gupta2025extensive, h2025resilient, hakeem2025advancing, halgamuge2024malicious, hamdaoui2025h, han2024trust, hassan2026ai, he2019indraj, he2025survey, hoque2018parallel, huang2024distributed, huang2025llm, ibrahim2026spatiotemporal, iftikhar2026b, ikram2025digital, ilic2026fto, islam2017cybersecurity, ito2026vast, j2025post, jasti2024multi, javaid2020scalable, javaid2025leveraging, jena2025secure, jiang2024graph, jiang2024personalized, jolfaei2019privacy, k2026lightweight, kamal2026deep, kaur2024security, kaur2026zero, khan2025enhancing, korade2026sumo, kousaridas2021qos, kumari2016enhanced, kumbhare2026comprehensive, lin2024efficient, liu2025efficientb, liu2026ffha, lu2025graph, luo2022scalacert, luo2023srcon, m2023optimized, ma2026semantic, mafakheri2021optimizations, malik2025security, medam2026automotive, mekala2026secured, mignone2024distributed, mirsky2018vesper, mohajer2026semantic, morgan2025digital, muid2025accurevoke, mumtaz2026comprehensive, mundra2025advancing, murshed2024vehicular, nageswaramma2025detection, nakayiza2026purefl, naqvi2024comparative, nguyenminh2023machine, otieno2024review, p2026federated, panchal2017segregation, pavuluri2025enhancing, pegurri2026predicting, penell2026gross, phil2025revolutionizing, qaisar2026robust, qureshi2022authentication, qureshi2026blockchain, rahman2026digital, rahoof2026ai, ranjan20266g, roy2026experimental, roy2026security, sahu2026assistants, salin2023gap, sardana2024denial, sathyan2025fedsecureformer, schippers2022system, sedjelmaci2025applied, sehgal2024virtual, selvakumar2026digital, senthil2025blockchainenhanced, shahid2023hybrid, sharma2026digital, shen2024blockchain, shi2023constructing, shivsankar2026cryptographically, simohammed2025per, song2025hybrid, sonker2025energy, sui2026eclas, sun2024interleaved, syedsabirmohamed2024dynamic, t2026quantum, tan2025design, tangade2026zk, tao2023zero, tao2024zero, thomas2026passive, tomas2026road, trabucco2025network, tuncel2026budacvab, turaev2026adaptive, ullah2021distributed, ullah2025decentralized, vandenberghe2011suitability, veras2026implementing, voon2026trust, wang2022strengthening, wang2023federated, wang2024lightweightb, wang2025application, wang2025collaborative, wang2026certificateless, wang2026trustb, wong2025sustainable, wu2025trust, xiang2023reproducing, xie2022digital, xing2026enhanced, xu2026sdds, yadav2026privacy, yadav2026trust, yousseef2025zero, yuan2026zero, zhang2013security, zhang2020high, zhang2022demo, zhang2023learn, zhang2024misbehavior, zhang2026driving, zhao2026eaba, zheng2023security, zheng2026digital, zhi2025multi, zhou2026trafficb

</details>

## 23. Scenario diversity: urban vs highway, mixed traffic, VRUs, weather

**Papers raising it:** 127 (high relevance: 50)

**Open problem:** Scenario diversity: urban vs highway, multi-intersection corridors, mixed traffic, VRUs, weather, rural.

**Bearing on the engine:** 50+ city maps, pedestrians and cyclists.

**Representative stated items:**

- [abrar2026deployment] Future work: extend J3161 MCS range; multi-intersection corridors; hardware profiling on automotive platforms.
- [baee2019broadcast] Assumed traffic scenario rather than closed-loop traffic simulation (as described).
- [heijden2018veremi] A dataset has an inherent limitation: attacker and benign deviations are unknowable in advance (e.g. urban GPS errors, adaptive attackers).
- [scopelliti2024efficient] small Tv impractical in rural areas
- [zhang2024enhancing] multiple traffic scenarios.
- [adigun2013protocol] Evaluate bit error rate and highway scenario; design a dissemination routing protocol.

<details><summary>All 127 source ids</summary>

abrar2026deployment, adigun2013protocol, agudelo2026trustworthy, agustina2026lightweight, aji2025implementation, akil2023non, alamoudi2026digital, almeida2023position, andert2024conclave, ardakani2022fuzzy, arfvidsson2024small, arshad2023blockchain, ashayer2025co, ashayer2026corridor, astorga2022revisiting, baccari2024anomaly, baee2019broadcast, bai2026dynamic, baimbetova2026pedestrian, bastola2025anomaly, bemani2020cyber, bhupathiraju2024vulnerability, blazek2018millimeter, bogdoll2024hybrid, chelghoum2023btv2p, chu2026map, dewangan2026misbehavior, diazsanchez2019tls, dong2020impact, duan2025simulation, dubovyk2026nonlinear, farman2021privacy, feng2025survey, forster2018evaluation, geiler2020plausibility, gioacchini2024cross, goncalves2026post, guemespalau2023building, guo2024adaptive, hagiya2021acceptability, hakeem2025advancing, hamdaoui2025h, he2025dino, heijden2018veremi, henderson2025link, hermann2026veremi, ibrahim2026spatiotemporal, ikram2025digital, ilic2026fto, inag2026adaptive, iqbal2025seeing, irfan2022reinforcement, islam2026trust, jornod2019prediction, jornod2020packet, karinsalo2026migrating, khalid2025safe, khattak2023evaluation, kim2021study, lee2024autonomous, li2026agentic, li2026transformerb, liang2025traceable, liu2023time, malik2018blockchain, malik2023carla, mekala2026secured, mohajer2026semantic, najafi2022decentralized, naqvi2024comparative, ouyang2026application, palaios2023machine, patel2026predictive, perrone2025generative, placzek2017detection, pougajendy2016cdai, ramamoorthy2024enhanced, ren2025cot, rizal2026exploring, sargolzaei2026reinforcement, sarker2020advanced, sarker2021context, sarker2021efficient, sarker2021suspicion, schuette2005hardware, scopelliti2024efficient, sehgal2024virtual, sen2024adversarial, shakir2024systematic, silva2025evaluation, stepanyants2025integrating, sultana2022data, tang2021fooling, tang2025i2nav, tao2023deep, thomas2026passive, tyagi2025building, urquizaaguiar2015impact, vandenberghe2011suitability, voon2026trust, vv2025lane, wang2022next, wang2023federated, wang2025application, weerasinghe2023threshold, wickramasurendra2025bert, wu2024smart, xu2022analyzing, xu2026cost, yao2023vdkms, ye2025reputation, yu2025digitalb, zahra2018mnp, zhang2022trust, zhang2022trustb, zhang2024deterministic, zhang2024enhancing, zhang2024inscope, zhang2024open, zhang2024testing, zhang2026edge, zhao2025communication, zhao2025unified, zheng2024game, zheng2025muse, zhou2019double, zhou2024federated

</details>

## 24. Controller-specific results: platoons/CACC and other automation functions

**Papers raising it:** 39 (high relevance: 21)

**Open problem:** Results are controller-specific (platoon/CACC); other automation functions are untested.

**Bearing on the engine:** Engine covers general urban traffic, not only platoons.

**Representative stated items:**

- [heijden2017analyzing] Detection alone is likely insufficient given the volume of crashes; need prevention via resilient controllers and effective detection and revocation.
- [kalogiannis2022attack] Framework can be extended to other driving automation functions.
- [malik2025security] benchmark covers jamming against CACC only.
- [almutaz2014detecting] detection weaker when natural dispersion is low (vehicles platooning).
- [basiri2019security] "FUTURE WORK In this research, we have focused on security and robust- ness analysis of vehicle platoons based on a graph-theoretic approach."
- [koley2022framework] Homogeneous platoons only.

<details><summary>All 39 source ids</summary>

ahmed2025detection, alam2026daire, alemu2023controller, almutaz2014detecting, bakhshi2022driving, basiri2019security, braiteh2025platooning, castiglione2025lightweight, farivar2021covert, ge2023resilient, habib2025hierarchical, hassine2023vehicular, heijden2017analyzing, holland2024testing, huisman2023impact, hussein2024da, kalogiannis2022attack, koley2022framework, li2023exploring, li2024mixed, li2025attentionguard, malik2025security, mohammed2024resilient, mundra2025advancing, nafea2025blockchain, patounas2015denial, petrillo2021secure, sargolzaei2026reinforcement, sarker2020advanced, schuette2005hardware, sen2024adversarial, taylor2022vehicular, ucar2016security, vieira2019realistic, wang2025observer, wang2026efficientb, wang2026vehdyn, yang2021risk, zhou2022robust

</details>

## 25. Collective perception and sensor-data misbehaviour

**Papers raising it:** 66 (high relevance: 16)

**Open problem:** Collective perception and sensor-data misbehaviour (ghost objects, fusion) need detection and evaluation.

**Bearing on the engine:** Partly outside the engine (no perception sensors).

**Representative stated items:**

- [malik2025security] Consensus reduces traffic density and relies on faultable sensors
- [zhang2024misbehavior] Future: runtime trust assessment simulation, realistic sensor data, ML MBD, edge trust MBD.
- [zhang2024simulation] Future: subjective logic for omission and ghost attacks.
- [allig2019extending] Apply to cooperative perception investigation
- [greco2025detection] future multi-feature fusion and explainable AI.
- [huisman2025self] Robustness under non-ideal conditions; sensor noise may cause Zeno switching; detection and transmission delays may cause late switching; minimise required sensors.

<details><summary>All 66 source ids</summary>

allig2019extending, amini2022vista, anon2020modeling, asok2024vulnerability, azfar2024traffic, baccari2024anomaly, bai2025privacy, bastola2025anomaly, chang2023safe, eziama2026enhanced, feng2024semantic, feng2025survey, garlichs2018realizing, ghelani2024gradient, greco2025detection, gupta2025optimized, hao2024rcooper, he2021distributed, hernandez2025software, huisman2025self, jasti2024detecting, jeon2021simultaneous, ju2020deception, kiruthika2024simuhack, kovacs2023implementation, krayani2023integrated, kumar2026design, li2026agentic, luo2025mixed, malik2025security, mohajer2026semantic, molloy2026g, morra2020building, okafor2026quantum, otieno2024review, pavlitska2026systematic, piperigkos2023robust, pourkeshavarz2026autoworld, predd2006distributed, rashid2026field, ren2024review, sarsam2023cybersecurity, shit2025scalable, sluis2025quality, tang2025i2nav, tao2023flowsim, tao2023zero, tao2026learning, teper2022auna, teufel2024collective, wan2025systematic, wang2021deep, wang2021i, wang2021trusted, wu2025synthetic, xu2023opencda, yazgan2024collaborative, yu2022dair, zhang2022trust, zhang2024inscope, zhang2024misbehavior, zhang2024simulation, zhang2025adaptive, zhang2026stealthy, zhao2025communication, zhao2026eaba

</details>

## 26. Real-time operation and edge/OBU resource constraints

**Papers raising it:** 227 (high relevance: 54)

**Open problem:** Real-time operation at the edge/RSU and on OBUs; sparse RSU coverage.

**Bearing on the engine:** RSU relay and edge placement are configurable.

**Representative stated items:**

- [heijden2017blackchain] Guarantees of hierarchical versus full consensus.
- [khodaei2016evaluating] Investigate pseudonym utilisation with various refill intervals and lifetimes versus server workload and privacy.
- [scopelliti2024efficient] Choice of Tv trades revocation speed vs resources and desynchronisation
- [zhang2024misbehavior] Future: runtime trust assessment simulation, realistic sensor data, ML MBD, edge trust MBD.
- [ahmad2025opencams] Future: 5G NR sidelink, ROS2 stacks, real-time GNSS spoofing models, MEC, real-world data streaming for DT validation.
- [akil2023non] Notes infrastructure-based schemes need RSUs, CRLs via RSUs and pseudonym reuse in rural areas.

<details><summary>All 227 source ids</summary>

abdullahi2025robust, abhishek2022trust, abood2023lstm, abrar2024gps, adom2025leveraging, agarwal2022p2pcpm, ahmad2025opencams, ahmadpanah2025semantic, ahmed2025qsafe, aji2025implementation, ajjaj2023incremental, akil2023non, akwirry2022multi, alam2026daire, alamoudi2026digital, alhassan2025intrusion, ali2026collaborative, aljanabi2024d, allafi2025explainable, anisi2025distributed, anon2025enhanced, arshad2018review, ashik2026analyzing, astorga2022revisiting, ayyamgari2026authentication, azzam2025ddos, babbar2024effective, babu2026hybrid, badhesha2025efficient, bai2025privacy, bai2026deep, bajpai2023intrusion, balaji2024metaheuristic, bangui2021fasterb, bintoro2025overview, buse2019real, caballerogil2014revocation, caballerogil2022query, ceapa2026iot, chandolia2026ecc, cheng2022stc, chiang2026lightweight, chowdhary2025comprehensive, comert2018change, comert2020change, cui2025false, cui2026fedauth, dai2019big, dai2025lightweight, dean2022mitigating, depp2023enhanced, diazsanchez2025beyond, dong2020impact, ekpo2026explainable, eldalahmeh2026eliminating, emara2016context, emran2026battlefield, eziama2026enhanced, fan2025learning, farzanullah2026pera, fathima2026comprehensive, fatima2026privacy, feng2016method, gomez2023survey, guo2025min, ha2023energy, hager2015literature, hallyburton2025trust, hallyburton2025trusted, hamdaoui2025h, hamdi2026decentralized, hankins2023eyes, hassan2026ai, heijden2017blackchain, hernandez2025software, hong2025digital, hu2026llm, iandolo2026camasa, iftikhar2026b, imran2020ieee, j2025post, jain2024iot, jedh2024improvement, jeffrey2023review, jeyaram2026lightweight, jiang2024personalized, jolfaei2019privacy, jostmann2026digital, kamal2026deep, kapoor2025machine, kaur2026zero, kayondo2026secv, khalid2025safe, khan2021efficacy, khan2026robust, khodaei2016evaluating, khodaei2020cooperative, khuwuthyakorn2025federated, kiruthika2024simuhack, korba2025zero, koursioumpas2024distinqt, krayani2023integrated, kumar2019detailed, lecci2021ns, lee2024autonomous, lekidis2024anomaly, li2019multi, li2021unlinkable, li2025attentionguard, li2025two, li2026blockchainb, lin2015security, liu2025digital, liu2025foundation, ma2026privacy, makkar2025quantum, maller2022cloud, mamun2024enhancing, manale2025enhancing, mao2022reliable, marojevic2018c, mavikumbure2025v2xformer, mccarthy2021opencv2x, medam2026autonomous, mekala2026secured, meshram2026bsrp, meyer2021simulator, mohajer2026semantic, molloy2026g, morgan2025digital, mouatassim2026enhanced, muid2025accurevoke, muller2019trust, mundra2025advancing, murugan2025study, nandy2020t, nedungadi2026fedx, ouyang2026application, park2024enhancing, pasupuleti2026quantum, pegurri2026predicting, phil2025revolutionizing, pokhrel2026quantum, preetha2020survey, qi2020privacy, qiu2020optimal, qureshi2026blockchain, r2024elliptic, rana2025lightweight, ranjan20266g, rizal2026exploring, saharsh2025survey, sahlabadi2025advancements, sajini2026blockchain, saleh2025agent, sangve2017intrusion, sanzgomez2025cybersecurity, sarker2021suspicion, sathyan2025fedsecureformer, scopelliti2024efficient, sen2024adversarial, senthil2025blockchainenhanced, setyowati2025performance, shagor2026post, shahariar2026survey, shahriar2025intelligent, shakir2024systematic, shankar2026trust, shi2023constructing, shit2025scalable, siddiqua2022trust, sinha2025targeting, sonker2025energy, sri2024review, ssaritha2024vanilla, struder2018flexible, syedsabirmohamed2024dynamic, tan2023efficient, tang2025digital, tao2023zero, trantruong2026systematic, turnip20266g, verma2024enhancing, vv2025lane, wan2025pbru, wang2021trusted, wang2023blockchainenhanced, wang2024denial, wang2024digitalb, wang2025dynamic, wang2025edgeb, wang2025research, wang2026certificateless, wang2026hrc, wellensmiles2025systematic, wu2024digital, xie2023efficient, xing2026fine, xiong2025enhancingb, yadav2026privacy, yang2025dynamic, yang2025painet, yang2025verifiable, yang2026near, ye2025reputation, yoshizawa2026security, youness2025vemisnet, yu2024efficientb, yu2024ensuring, yuan2024real, yuan2025llm, zhang2010scalable, zhang2022trust, zhang2023generative, zhang2024misbehavior, zhang2025forensics, zhang2026edge, zhang2026theoretical, zhao2025privacy, zhao2025unified, zhao2026eaba, zheng2020service, zheng2023security, zheng2026digital, zheng2026prism, zhong2023tackling, zhou2025ai

</details>

## 27. Formal security analysis and verification

**Papers raising it:** 29 (high relevance: 9)

**Open problem:** Formal verification of protocols and adaptive schemes is missing or informal.

**Bearing on the engine:** Outside the engine's core; test vectors complement formal proofs.

**Representative stated items:**

- [didouh2022blockchain] Future: proof of concept as a real traffic management server.
- [gong2024quantum] Security against attacks analysed only informally
- [khan2025decentralized] Since the limitation of zk-SNARK is it takes time to generate a proof and this has been also the focus of this study.
- [kovacs2023implementation] Parameters chosen as proof of concept, not real implementation
- [protogeros2026benchmarking] Reliable automation requires iterative workflows guided by formal verification.
- [sengupta2026adaptive] Future work: formal verification of RL-assisted adaptation, hardware-in-the-loop testbeds.

<details><summary>All 29 source ids</summary>

ahmed2026quantum, buinevich2019forecasting, chiang2026lightweight, didouh2022blockchain, diop2025hybrid, faye2025functional, feng2026efficient, gong2024quantum, hakim2025internet, jena2025privacy, khan2025decentralized, kosari2022modelling, kovacs2023implementation, li2026federated, meshram2026bsrp, pokhrel2026quantum, protogeros2026benchmarking, r2024cross, rai2026detection, sadat2025secure, sengupta2026adaptive, ssaritha2024vanilla, tao2023zero, tiwar2025pqc, twardokus2026cross, yadav2026privacy, yekta2023vatt, zhang2026online, zhao2025unified

</details>

## 28. Explainability and interpretability of detectors and models

**Papers raising it:** 36 (high relevance: 5)

**Open problem:** Explainability of detectors and models.

**Bearing on the engine:** LLM analyst over ground truth could supply explanations.

**Representative stated items:**

- [dewangan2026misbehavior] Need Byzantine-resilient FL, XAI, interoperable standards-aligned detection, benchmarks with latency and robustness metrics.
- [greco2025detection] future multi-feature fusion and explainable AI.
- [wang2025cp] Future: query-based black-box attacks
- [patel2026predictive] Computational overhead and weak interpretability.
- [zhong2023tackling] future: adaptive/transfer learning, FL, adversarial robustness, interpretability.

<details><summary>All 36 source ids</summary>

alan2026enhancing, allafi2025explainable, alshareeda2024ai, cai2025tutorial, dewangan2026misbehavior, fatima2026privacy, gorbenko2024evaluation, greco2025detection, guo2025largeb, jiang2024graph, kumbhare2026comprehensive, liu2023time, nazat2024evaluating, nazat2024xai, nedungadi2026fedx, p2026federated, patel2026predictive, pavithra2025intrusion, pavlitska2026systematic, ranjan20266g, rezazadeh2024intelligible, s2026integrated, sarker2020advanced, sarker2021context, sarker2021efficient, sarker2021suspicion, sen2024adversarial, senevirathna2022survey, tam2024survey, u2026energy, u2026green, wang2025cp, wu2025trust, xing2026enhanced, zhong2023tackling, zhou2025ai

</details>

## 29. Federated / privacy-preserving / Byzantine-robust learning

**Papers raising it:** 47 (high relevance: 7)

**Open problem:** Federated, privacy-preserving and Byzantine-robust learning for detection.

**Bearing on the engine:** Outside the engine's core.

**Representative stated items:**

- [akwirry2022multi] RSU-based federated model fails where RSUs are sparse
- [dewangan2026misbehavior] Need Byzantine-resilient FL, XAI, interoperable standards-aligned detection, benchmarks with latency and robustness metrics.
- [idris2024novel] Future: FL or differential privacy.
- [oli2026deep] Guarantees hold only within the Byzantine bound.
- [vinita2026hierarchical] Future work: personalised FL, asynchronous aggregation, digital-twin-assisted mobility, RL client scheduling.
- [yoganand2026enhancing] Future work: graph/identity-aware Sybil detection; full lattice PQC; DNN-based FL.

<details><summary>All 47 source ids</summary>

akwirry2022multi, ali2026collaborative, almansour2025adaptive, alsamiri2025mccs, althunayyan2025survey, alwash2025effective, attiaoui2025blockchained, bekele2025deep, ch2025novel, chen2024fed, cui2026fedauth, demir2025targeted, dewangan2026misbehavior, digregorio2025evaluating, fatima2026privacy, gayathri2025machine, haddaji2025blockchainbased, haghighifard2024secure, hossain2023flid, idris2024novel, jiang2024graph, jiang2024personalized, khalid2025fedcrossvan, kumbhare2026comprehensive, luo2025privacy, mamun2025adversarial, mareta2025beyond, mazouzi2026block, naeem2026adaptive, nakayiza2026purefl, nedungadi2026fedx, oli2026deep, rai2026detection, s2025survey, safaei2026detecting, satya2026federated, sengupta2025privacy, senthil2025blockchainenhanced, shahsavari2026sok, talpur2022gfcl, turaev2026adaptive, vinita2026hierarchical, wang2026withdrawn, yang2026ai, yoganand2026enhancing, zheng2026prism, zhong2023tackling

</details>

## 30. Blockchain overhead, consensus and practicality

**Papers raising it:** 64 (high relevance: 12)

**Open problem:** Blockchain-based schemes have unresolved overhead, consensus stability and privacy problems.

**Bearing on the engine:** Outside the engine's core (alternative to SCMS).

**Representative stated items:**

- [heijden2017blackchain] Whether vehicle clusters are stable enough for consensus.
- [malik2025security] Consensus reduces traffic density and relies on faultable sensors
- [almekhlafi2024integrating] future: Beyond-5G, AI, blockchain security, semi-virtual/hybrid simulation, wider V2X.
- [altmemi2025software] Gaps: scalability, computational overhead, limited 5G/blockchain/AI integration
- [eldalahmeh2025secure] Open challenges: adaptive security management, blockchain integration, compatibility with emerging communication standards.
- [shagor2026post] Ledger tamper evidence only within permissioned governance; malicious coalition can rewrite policy.

<details><summary>All 64 source ids</summary>

agarwal2026hybrid, ahmed2022privacy, alayedi2026cross, ali2026collaborative, almekhlafi2024integrating, almuhaideb2022efficient, altmemi2025software, anon2018blockchain, attiaoui2025blockchained, azzam2025ddos, bekele2025deep, brinzea2026blockchain, chen2025anonymous, d2026efficient, dorri2017blockchain, eldalahmeh2025secure, eldalahmeh2026eliminating, gantayat2026enhancing, giannaros2023autonomous, gu2020efficient, gupta2020blockchain, haddaji2025blockchainbased, han2024trust, heijden2017blackchain, jeffrey2023review, jena2025privacy, kumbhare2026comprehensive, li2021unlinkable, li2026blockchainb, liang2024trustworthy, liang2025traceable, luo2022scalacert, m2023optimized, malik2018blockchain, malik2025security, mesci2024blockchain, meshram2026bsrp, mhamdi2022trust, nafea2025blockchain, narkedimilli2026advances, qureshi2026blockchain, rahman2023blockchain, rana2025lightweight, ratnayake2023trust, ruan2023private, s2026integrated, sadiq2020blockchain, sajini2026blockchain, senthil2025blockchainenhanced, shagor2026post, tan2025design, u2026energy, u2026green, ullah2025decentralized, wang2023blockchainenhanced, wijesekara2026bcrlsecurelink, wong2025sustainable, yadavndintegrated, yan2021decentralized, yang2025verifiable, zeydan2022postb, zhang2019data, zhang2021ait, zhou2022efficient

</details>

## Not clustered

987 papers whose items matched no problem. Their ids are not listed; they can be regenerated from the summaries with the same rules. Almost all are generic motivation sentences or 'future work in conclusion (not extracted)'.
