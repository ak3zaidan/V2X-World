# Landscape of the field

Scope: 11,315 deduplicated works (catalog.jsonl), each with a summary in `summaries/`. 3,430 are rated high relevance, 5,169 medium and 2,716 low. Of the 11,315 summaries, 693 were written from the full text, 8,271 from the abstract and 2,351 from the title and metadata only. Paper ids below resolve to `summaries/<id>.md`.

## 1. Taxonomy

The field splits into six layers. This engine is unusual because it spans all six in one process.

1. **Trust infrastructure (PKI).** Who issues credentials, how they are provisioned, and how they are revoked.
   - SCMS (US: RA, PCA, LA1/LA2, MA, ECA, butterfly keys, linkage-value CRLs) and its crypto analysis: brecht2018security, whyte2013security, simplicio2018unified, barreto2021schnorr, boldyreva2024provable, junior2020privacy, simplicio2019acpc.
   - CCMS / ETSI C-ITS VPKI and the KTH line: khodaei2017secmace, khodaei2023secmace, noroozi2018vpkiaas, anon2013vespa.
   - Implementations and comparisons: chen2023performance, chen2024performance, chen2026comparison, mushrall2018emulab.
   - Revocation and CRL distribution: haas2011efficient, khodaei2020scalable, eckhoff2013smartrevoc, scopelliti2024efficient, tesei2023transparent.
   - Alternatives such as blockchain, group signatures, certificateless and CPPA schemes. This is a large literature of over 1,700 papers with little system-level evaluation.
2. **Privacy.** Pseudonym change strategies, linkability and tracking attacks (buttyan2009slow, escher2021well, bloessl2015scrambler, liu2018cross, yieh2019multi), and the privacy-safety trade-off (jemaa2017study, emara2017safety, xin2019balance).
3. **Cryptographic cost and migration.** Verification overhead and policies (papadimitratos2008impact, baee2019broadcast, feiri2012evaluation, krishnan2011verify, pollicino2021hardware). Post-quantum and hybrid V2X (twardokus2024cryptography, twardokus2025quantum, abrar2026deployment, yoshizawa2023post, fikry2026post, chen2026pseudonym). Threshold and multi-party authorities (ajibuwa2023resilient, zhang2025decentralized).
4. **Misbehaviour.** Local detection, which makes up the bulk of the literature: plausibility checks, ML, trust and reputation. Reporting and the Misbehaviour Authority (kamel2018misbehavior, kamel2019misbehavior, haidar2020dare, SAE J3287 = anon2024v2x). Reaction and eviction (raya2007eviction, hasrouny2019misbehavior, willecke2023a4md). Datasets (heijden2018veremi, kamel2020veremi, hermann2026veremi).
5. **Attacks and their physical impact.** Falsification, Sybil, replay, jamming and DoS, and their effect on traffic and safety: platoons and CACC (heijden2017analyzing, amoozadeh2015security, malik2022comfase), signal control (chen2018exposing), city-scale traffic (wellensmiles2026assessing). Lower-layer C-V2X and NR sidelink attacks: trkulja2020denial, twardokus2022vehicle, twardokus2023protecting, talukder2024symjam.
6. **Evaluation infrastructure and AI.**
   - Simulators and co-simulation: Veins/Plexe, Artery/Vanetza, Eclipse MOSAIC, ms-van3t/VaN3Twin, OpenCAMS, VESNOS, F2MD.
   - Digital twins, learned surrogates and network world models: RouteNet family, chai2025mobiworld, bouton2026mobile.
   - Counterfactual and causal network evaluation: alomar2022causalsim, bothra2022veritas, hou2024if.
   - LLM agents for simulation, network operations and security: rezazadeh2024genonet, wang2025network, gao2025exploring.
   - Risk assessment (TARA, ISO/SAE 21434).

## 2. Counts by topic and year

Topics were assigned by keyword rules on title and abstract; one paper can count under several topics. "High" = rated high relevance. 1,703 papers matched no rule; most of them are adjacent background.

| Topic | All | High | <2010 | 2010-14 | 2015-18 | 2019-21 | 2022-23 | 2024-25 | 2026+ |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| PKI / SCMS / VPKI / certificate management | 828 | 314 | 54 | 79 | 118 | 170 | 126 | 161 | 106 |
| Certificate revocation / CRL | 648 | 284 | 63 | 100 | 116 | 135 | 89 | 85 | 55 |
| Pseudonyms and location privacy / linkability | 1044 | 526 | 33 | 105 | 180 | 230 | 183 | 200 | 109 |
| Anonymous authentication (CPPA, group, certificateless, ring) | 835 | 160 | 13 | 55 | 85 | 194 | 157 | 223 | 108 |
| Misbehaviour detection / IDS / trust | 2277 | 723 | 12 | 119 | 234 | 407 | 437 | 718 | 335 |
| Misbehaviour reporting / Misbehaviour Authority | 30 | 26 | 0 | 2 | 4 | 9 | 5 | 9 | 1 |
| Attacks and their traffic / safety impact | 2247 | 910 | 13 | 107 | 269 | 401 | 463 | 720 | 262 |
| Platooning / CACC security | 642 | 360 | 3 | 15 | 76 | 135 | 136 | 199 | 74 |
| Post-quantum / hybrid cryptography | 684 | 299 | 1 | 1 | 14 | 54 | 82 | 241 | 274 |
| Threshold / multi-party cryptography | 293 | 60 | 6 | 11 | 18 | 35 | 58 | 106 | 53 |
| Blockchain / distributed ledger | 894 | 199 | 0 | 0 | 18 | 159 | 216 | 338 | 159 |
| Security overhead / verification load | 172 | 48 | 4 | 11 | 21 | 29 | 28 | 42 | 37 |
| Radio: DSRC / 802.11p / ITS-G5 | 352 | 150 | 6 | 26 | 59 | 80 | 46 | 91 | 42 |
| Radio: LTE-V2X / NR-V2X / sidelink | 216 | 125 | 0 | 0 | 8 | 53 | 47 | 64 | 41 |
| Simulators, co-simulation, digital twins, testbeds | 2150 | 761 | 48 | 105 | 184 | 294 | 429 | 704 | 370 |
| Datasets and benchmarks | 1489 | 373 | 3 | 5 | 30 | 134 | 265 | 645 | 402 |
| Learned surrogates / world models / GNN network models | 599 | 97 | 0 | 0 | 2 | 37 | 100 | 244 | 211 |
| Counterfactual / causal evaluation | 147 | 23 | 1 | 1 | 5 | 12 | 31 | 43 | 54 |
| LLM agents (networks, security, simulation) | 595 | 42 | 0 | 0 | 0 | 0 | 25 | 339 | 225 |
| Risk assessment / TARA / ISO/SAE 21434 | 252 | 74 | 2 | 4 | 20 | 42 | 49 | 82 | 51 |
| Standards and deployment reports (venue type) | 66 | 10 | 13 | 9 | 3 | 6 | 5 | 10 | 2 |

All papers by year: before 2010, 317; 2010-14, 765; 2015-18, 1,287; 2019-21, 1,925; 2022-23, 2,062; 2024-25, 3,313; 2026 and later, 1,523; unknown, 123. High-relevance papers by the same bins: 93, 331, 461, 643, 658, 850, 350, 44. Year 2026 is partial (searched on 2026-10-17).

**Trends.**
- Post-quantum V2X grew from 15 papers before 2019 to 515 since 2024. It is now the fastest-growing security topic.
- LLM work appears only from 2023.
- Learned surrogates and world models took off after 2022.
- Classical PKI and revocation papers have been flat since about 2019.
- Misbehaviour **reporting** and the MA remain a niche: 30 papers in total.
- Of the 23 high-relevance counterfactual papers, none is a V2X security study with ground truth.

## 3. Venues

By venue type: 6,348 journal, 2,830 conference, 1,369 preprint, 421 book or chapter, 138 thesis, 117 workshop, 44 report, 22 standard.

Main venues (venue names normalised across editions):

| Venue | All | High |
|---|---:|---:|
| arXiv | 1077 | 146 |
| IEEE VTC (all editions) | 212 | 113 |
| IEEE Transactions on Vehicular Technology | 275 | 92 |
| IEEE Access | 312 | 86 |
| IEEE Transactions on Intelligent Transportation Systems | 258 | 82 |
| IEEE Vehicular Networking Conference (VNC) | 104 | 59 |
| IEEE Internet of Things Journal | 221 | 56 |
| IEEE ICC | 137 | 54 |
| IEEE GLOBECOM | 102 | 42 |
| Lecture Notes in Computer Science | 95 | 38 |
| Sensors | 134 | 32 |
| Vehicular Communications (Elsevier) | 93 | 31 |
| IEEE IV / IEEE ITSC | 49 / 54 | 30 / 30 |
| IEEE TDSC / IEEE TMC / IEEE TIFS | 40 / 44 / 39 | 14 / 14 / 11 |
| IACR ePrint | 56 | 13 |
| IEEE INFOCOM (incl. workshops) | 29 | 12 |

The security-flagship venues (NDSS, VehicleSec, WiSec, PETS, USENIX NSDI, SIGCOMM) hold few papers, but those few are among the closest prior works: twardokus2024cryptography, scopelliti2024efficient, chen2018exposing, khodaei2018efficient, boldyreva2024provable, alomar2022causalsim, bothra2022veritas. VNC is the venue where closed-loop misbehaviour work concentrates: willecke2023a4md, kamel2018misbehavior, heijden2017analyzing, boeira2018mitigating, hermann2026veremi, jerbi2026poster.

## 4. Main research groups

The groups are inferred from author frequency among high-relevance papers. Affiliations come from general knowledge and should be checked before citing.

| Group (lead authors) | High papers | What they own in this space | Key ids |
|---|---:|---|---|
| Ulm University (Frank Kargl, R. W. van der Heijden, Wehmer) | 27 | MBD evaluation, VeReMi, CACC attack impact, MA design (Blackchain), causal field evaluation | heijden2018veremi, heijden2017analyzing, heijden2017blackchain, wehmer2021practical, wehmer2023fast, hermann2026veremi |
| KTH (Panos Papadimitratos, Mohammad Khodaei) | 19 / 12 | Implemented VPKI (SECMACE, VPKIaaS), pseudonym policies, CRL distribution at city scale, platoon attacks | khodaei2017secmace, khodaei2020scalable, noroozi2018vpkiaas, kalogiannis2022attack, raya2007eviction |
| IRT SystemX / Télécom Paris (Joseph Kamel, Arnaud Kaiser, Pascal Urien) and Reims (Marwane Ayaida, Farah Haidar) | 9-15 | F2MD, VeReMi Extension, MA reporting protocol, DARE, field pseudonym reload | kamel2020simulation, kamel2020veremi, kamel2018misbehavior, haidar2020dare, haidar2021experimentation |
| University of Luxembourg (Abdelwahab Boualouache) | 17 | MBD and pseudonym-change surveys, ML-MBD for 5G | boualouache2022survey |
| Veins / Plexe community (Christoph Sommer, Falko Dressler) | 15 / 13 | The reference V2X simulators; cooperative-driving attacks | boeira2017impact, heijden2017analyzing (Plexe-based) |
| RIT (Hanif Rahbari, Geoff Twardokus) | 9 / 10 | PQ/hybrid V2V (NDSS 2024), CHASM, PQ-V2Verifier, NR sidelink DoS, discreditation attack, DT-CoVeSS | twardokus2024cryptography, twardokus2025quantum, twardokus2022vehicle, twardokus2023protecting, twardokus2021targeted, twardokus2025dt |
| University of São Paulo / LG (Marcos Simplicio, Paulo Barreto) | n/a | SCMS crypto: unified butterfly, ACPC, LA-free revocation, qSCMS | simplicio2018unified, simplicio2019acpc, junior2020privacy, barreto2018qscms |
| Chunghwa Telecom (Abel C. H. Chen) | 10 | SCMS/CCMS/C-SCMS implementations, hybrid PQ SCMS certificates | chen2023performance, chen2024performance, chen2026comparison, chen2026pseudonym |
| UPC Barcelona (Albert Cabellos-Aparicio, Pere Barlet-Ros) | 11 / 10 | RouteNet GNN network digital twins | ferriolgalmes2022routenet, rusek2019routenet |
| TU Dortmund (Christian Wietfeld, Benjamin Sliwa) | 11 / 10 | Data-driven predictive QoS for vehicles | sliwa2020cooperative |
| Waterloo / UNB line (Xuemin Shen, Rongxing Lu, Xiaodong Lin) | 10-14 | CPPA and revocation schemes | wasef2011managing |
| Clemson / Alabama (Mizanur Rahman and others) | 11 | Attacks on CV applications, in-vehicle DT Sybil detection | hasan2026vehicle |
| Microsec / BME (Petho and others) | n/a | PKI safety impact quantification | petho2022considering, petho2024investigating, petho2024quantifying |
| Univ. of Windsor (Arunita Jaekel, Ikjot Saini) | 10 | ML MBD on VeReMi | n/a |

There is also a long tail of authors proposing anonymous authentication and blockchain schemes (for example Al-Shareeda, Jie Cui, Hong Zhong; 13, 11 and 10 high papers), almost always evaluated analytically.

## 5. The closest prior works to this engine

No paper combines all of the engine's distinguishing features: city-scale microscopic traffic, three V2X radio stacks, a standards-validated US SCMS backend with linkage revocation, closed-loop MBD-to-revocation, per-message joint ground truth and exact counterfactual reruns. The closest works each cover a subset:

| Work | Traffic | Radio | PKI backend | Attacks + MBD in loop | Revocation in loop | Determinism / counterfactuals | Notes |
|---|---|---|---|---|---|---|---|
| abdo2024vehicular (VESNOS) | SUMO | OMNeT++ | SCMS (open source) | security focus | not stated | no | Closest on the "SCMS + traffic + network" claim (abstract only; read in full) |
| willecke2023a4md (A4MD) | SUMO | Artery ITS-G5 | ETSI (CCMS concepts) | yes (F2MD detectors) | reaction / eviction | no | Closest on the closed loop, but ETSI, not US SCMS |
| kamel2020simulation (F2MD) | SUMO | Veins 802.11p | none (future work) | yes, with reports | no | seeds only | Basis of most MBD simulation work |
| mushrall2018emulab | none | none | functional SCMS emulation | no | root-CA self-healing | no | SCMS research emulator |
| khodaei2017secmace / khodaei2023secmace | mobility traces | no | full VPKI | no | CRL distribution separately | no | EU-style VPKI at scale |
| chen2023performance / chen2026comparison | no | devices | SCMS, CCMS, C-SCMS; RCA in SCMS Manager test CTL | no | no | no | Real trust-list interoperability |
| riebl2016improved (Artery/Vanetza) | SUMO | ITS-G5 | message security only | no | no | no | Standard-compliant signing in simulation |
| twardokus2024demo / twardokus2025dt | OSM scenarios | SDR, C-V2X | 1609.2 with PQ | attacks | no | no | HIL testbed; DT-CoVeSS not yet read |
| ahmad2025end / ahmad2025opencams (OpenCAMS) | SUMO + CARLA | OMNeT++ C-V2X | PQ case study | cybersecurity use cases | no | no | Multi-process co-simulation |
| pegurri2026van3twin (VaN3Twin) | SUMO | ns-3 multi-RAT, ray tracing | no | no | no | no | Measurement-validated PHY |
| malik2022comfase / malik2025security | SUMO | Veins | no | fault/attack injection, safety outcomes | no | repeatable | Security benchmark for CDA |
| bello2026human | SUMO | OMNeT++ NR-V2X | no | DoS attack | no | matched no-attack baselines | Closest on paired attack-vs-baseline safety comparison |
| kerner2008testbed | three-phase micro | 802.11e | no | no | no | single integrated model | First single-process traffic+radio simulator (2008) |
| egealopez2019vehicular (Veneris) | Unity, IDM/MOBIL | GPU ray tracing | no | no | no | no | 3D game-engine V2X with IDM/MOBIL |
| gao2025great (Great X) | Unreal | ray tracing | no | no | no | deterministic fixed-step | Single-engine deterministic multi-modal |
| jerbi2026poster / novo2026spade / hermann2026veremi | Artery / MOSAIC | ITS-G5 | no | injection, labelled | no | seeded | Labelled attack-data generators |
| zhang2026driving | driving sim | no | no | no | no | matched factual/counterfactual benchmark | Same idea as direction 3 in driving video |

The papers that rule out "first" claims are listed in `claims-we-cannot-make.md`.

## 6. Coverage caveats

- OpenAlex (budget exhausted) and DBLP (bot challenge) were unavailable for most clusters, and Semantic Scholar was throttled. Crossref, arXiv and OpenCitations carried the search; see `search/*-queries.md`. Conference papers indexed mainly in DBLP or IEEE Xplore may be under-represented.
- Of the 192 strong-overlap summaries, many are title-only (for example haas2012global, liu2010limits, bittl2015security, krishnan2011verify, hammi2017butterfly, bindel2022drive, barreto2018qscms). These need full-text retrieval before any claim depends on them.
