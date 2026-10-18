# What makes a contribution BIG in V2X security and simulation

Written 2026-10-18. Sources:
- Catalog citation counts from `research/catalog.jsonl`, cited by catalog id.
- Classics missing from the catalog, counted by Crossref or Semantic Scholar (S2) and cited by DOI.
- "Age-adj" means citations / sqrt(2027 - year).

OpenAlex was over quota and DBLP blocked scripted access, so the 2022–2026 top-venue list (§2) is built from the catalog plus web searches. It is not exhaustive.

## 1. The ~50 most-cited works, by contribution type

Excluded as off-topic, though the catalog's V2X filter picks them up: papernot2017practical, lin2022et, alrawais2017fog, mirsky2018kitsune, wang2024rcagent.

| Type | Work (id or DOI) | Year | Cites | Age-adj | Why it became influential |
|---|---|---|---:|---:|---|
| Simulator/platform | CARLA (S2, CoRL) | 2017 | 7376 | 2332 | Open, free, scriptable world that everyone else builds datasets on |
| Simulator/platform | SUMO (10.1109/ITSC.2018.8569938, S2) | 2018 | 3442 | 1147 | The default traffic model; every coupled V2X simulator cites it |
| Simulator/platform | sommer2011bidirectionally, Veins (10.1109/tmc.2010.133) | 2011 | 1580 | 395 | First bidirectional network-traffic coupling; became the VANET default |
| Simulator/platform | sommer2021veins | 2021 | 260 | 106 | Citable "book chapter" for a tool already in use |
| Simulator/platform | OpenCDA (10.1109/itsc48978.2021.9564825) | 2021 | 263 | 107 | CARLA+SUMO co-simulation; the base for OPV2V |
| Simulator/platform | gawowicz2019ns (ns3-gym) | 2019 | 212 | 75 | Turned a simulator into an RL environment |
| Simulator/platform | kamel2020simulation (F2MD) | 2020 | 164 | 62 | Open misbehaviour-detection framework on Veins |
| Dataset/benchmark | xu2022opv2v | 2022 | 736 | 329 | Dataset, benchmark and baselines in one release; a new task (V2V perception) |
| Dataset/benchmark | yu2022dair | 2022 | 681 | 305 | First real vehicle-infrastructure dataset; a leaderboard |
| Dataset/benchmark | li2022v2x (V2X-Sim) | 2022 | 419 | 187 | Simulated multi-agent benchmark |
| Dataset/benchmark | xu2023v2v4real | 2023 | 347 | 174 | Real-world counterpart to OPV2V |
| Dataset/benchmark | yu2023v2x (V2X-Seq) | 2023 | 243 | 122 | Sequential extension of DAIR |
| Dataset/benchmark | zimmer2024tumtraf | 2024 | 156 | 90 | Real roadside data |
| Dataset/benchmark | heijden2018veremi | 2018 | 255 | 85 | **"Comparable evaluation"** of misbehaviour detection; the de facto MBD benchmark |
| Dataset/benchmark | kamel2020veremi (VeReMi Extension) | 2020 | 237 | 90 | Fixed VeReMi's flaws; now the most-used MBD data |
| Architecture/protocol | raya2007securing (10.3233/jcs-2007-15103) | 2007 | 1236 | 276 | Defined the VANET threat model and the PKI and pseudonym architecture |
| Architecture/protocol | papadimitratos2008 SeVeCom (10.1109/mcom.2008.4689252) | 2008 | 408 | 94 | EU project architecture that fed the ETSI and C2C-CC designs |
| Architecture/protocol | calandriello2007efficient | 2007 | 591 | 132 | Practical pseudonym authentication |
| Architecture/protocol | raya2007eviction | 2007 | 459 | 103 | First revocation and eviction design (LEAVE/RTC) |
| Architecture/protocol | raya2008data (INFOCOM) | 2008 | 494 | 117 | Data-centric trust; vocabulary still used |
| Architecture/protocol | brecht2018security (SCMS) | 2018 | 325 | 108 | Authoritative description of the deployed US SCMS |
| Architecture/protocol | whyte2013security | 2013 | 161 | 43 | First public SCMS design |
| Architecture/protocol | sun2010efficient, studer2009tacking | 2009–10 | 382, 308 | ~90 | Early efficient-revocation and privacy schemes |
| Architecture/protocol | sampigethaya2007amoeba, lu2012pseudonym | 2007, 2012 | 392, 455 | 88, 117 | Location-privacy strategies (mix groups, social spots) |
| Architecture/protocol (trust/blockchain) | yang2019blockchain, kang2019secure, lu2018privacy, li2016art | 2016–19 | 799, 571, 340, 441 | 282, 202, 113, 133 | High counts inside the IoT-J/TVT ecosystem; **no deployment adoption found** |
| Attack | Koscher et al. S&P (10.1109/sp.2010.34) | 2010 | 1450 | 352 | Live compromise of a production car; started automotive security |
| Attack | Cao et al. CCS LiDAR spoofing (10.1145/3319535.3339815) | 2019 | 484 | 171 | First optimised physical attack on a production-grade AV perception stack |
| Attack | petit2015potential | 2015 | 803 | 232 | Hands-on attacks on real automated-vehicle sensors |
| Attack | amoozadeh2015security | 2015 | 416 | 120 | Showed V2X falsification destabilises cooperative driving |
| Attack | liu2016you, shen2020drift (USENIX), cao2022you (USENIX) | 2016–22 | 367, 160, 132 | 111, 62, 59 | Physical attacks with demos on real systems |
| Attack | chen2018exposing (NDSS) | 2018 | 102 | 34 | One spoofing vehicle congests a deployed I-SIG signal controller |
| Attack / measurement | wiedersheim2010privacy | 2010 | 258 | 63 | Showed simple pseudonym change is trackable; reframed the privacy debate |
| Attack + detection | xiao2006detection (Sybil), lo2007illusion | 2006–07 | 291, 133 | ~60 | Named attack classes that later work still uses |
| Measurement/field | enev2015automobile (PoPETs) | 2015 | 198 | 57 | Real CAN data identifies drivers |
| Measurement/field | wang2009vehicle (V2V channel) | 2009 | 285 | 67 | Field channel data that simulators adopt |
| Measurement/policy | allennddepartment (NHTSA V2V readiness report) | 2014 | 195 | n/a | Policy document; the reference for the US mandate debate |
| Detector method | kang2016intrusion, yang2021mth, gad2021intrusion, choi2018detecting (CCS) | 2016–21 | 678, 395, 325, 212 | 120–204 | First DNN in-vehicle IDS, or tied to a public dataset; later ones are incremental |
| Survey/SoK | lu2019survey, petit2015pseudonym, sakiz2017survey, kim2021cybersecurity, lin2008security, heijden2019survey, martinez2011survey | 2008–21 | 552, 434, 389, 384, 345, 282, 373 | 79–195 | Default related-work citation; rarely changes a decision |

## 2. Top-venue V2X security papers, 2022–2026, and what made them land

| Paper | Venue | Cites | What made it land |
|---|---|---:|---|
| Twardokus & Rahbari, "Vehicle-to-Nothing? Securing C-V2X against protocol-aware DoS" ([RIT news](https://www.rit.edu/news/rahbari-and-student-publish-paper-ieee-infocom)) | INFOCOM 2022 | n/a | Attack on the *standard* LTE-V2X/NR-V2X SPS scheduler, run on SDR hardware, with mitigations |
| xu2022pof, Proof-of-Following | NDSS 2022 | 11 | New primitive for a real deployment use (platoons), with physical-layer evidence |
| zhang2023data, data fabrication in collaborative perception | USENIX Sec 2023 | 37 | Attack plus defence on the hottest V2X task (cooperative perception); real-vehicle data |
| twardokus2024cryptography, PQ authentication for V2V | NDSS 2024 | 24 | Standards-compatible (IEEE 1609.2) fix for an imminent problem; hardware-validated |
| scopelliti2024efficient, timely revocation of V2X credentials ([PDF](https://www.ndss-symposium.org/wp-content/uploads/2024-17-paper.pdf)) | NDSS 2024 | 14 | Attacks a known weak point of the deployed SCMS (CRL latency); TEE-based, formally verified |
| cao2022you, LiDAR removal attacks | USENIX Sec 2022 | 132 | Physical attack on production-grade AV perception |
| Trkulja, Starobinski & Berry, C-V2X DoS ([NDSS page](https://ndss-symposium.org/ndss-paper/auto-draft-118), arXiv 2010.13725) | NDSS (workshop or main, unverified) | n/a | Analytical and simulation model of resource-selection DoS |

Earlier precursors in the same mould:
- verheul2019ifal (EuroS&P 2019), on SCMS certificate provisioning;
- camenisch2020zone (EuroS&P 2020);
- khodaei2018efficient (WiSec 2018), on CRL distribution.

All three take the deployed credential system as given and fix one measurable weakness.

What these papers share:
1. A **standardised or deployed target** (1609.2, SPS, SCMS revocation, deployed signal control), not a bespoke scheme.
2. **Real evidence**: SDR or OBU hardware, real vehicles, a formal proof, or a field dataset. Simulation alone appears only as scale-up.
3. A **concrete operational consequence** (denial of service, a revocation window, PQ packet size).
4. A **fix compatible with the standard**.

Simulation-only V2X security papers at these venues were not found.

Top venues bring legitimacy, not citation counts. Here they gather 10–40 citations in 2–4 years, far below the dataset papers in §1.

## 3. Patterns: which contribution types have outsized, lasting impact

1. **Shared infrastructure wins by an order of magnitude.**
   - CARLA, SUMO and Veins lead on both raw and age-adjusted counts. Datasets with benchmarks (OPV2V, DAIR-V2X) reached 700 citations in about 4 years, faster than any security paper.
   - The mechanism is that every later user must cite the tool or dataset they ran on.
   - The preconditions are that it is open, maintained, and cheap to run, and that it defines a task and metric others compete on.
2. **In V2X security specifically, the benchmark beats the detector.**
   - Together, VeReMi and its Extension (492 citations) outweigh any single misbehaviour detector.
   - Their framing was "comparable evaluation". The field lacked a common yardstick, not another algorithm.
   - F2MD (164) gained by being the runnable home for VeReMi-style experiments.
3. **Founding architecture papers last decades**, because standards and every later scheme cite them:
   - Raya & Hubaux 2007;
   - SeVeCom 2008;
   - the SCMS papers (whyte2013security, brecht2018security).

   For the US stack that window has closed; the SCMS is fixed. The openings left are new layers of it: PQ, revocation latency, cooperative perception, and misbehaviour authority (MA) policy.
4. **Demonstrated attacks on real systems change industry and standards decisions**, even with moderate citation counts: Koscher 2010, Cao 2019, Twardokus 2022, chen2018exposing. Their weight comes from irrefutable evidence on the real target.
5. **Citation counts mislead in two places.**
   - Blockchain and trust-scheme papers (e.g. yang2019blockchain, 799) and surveys rank high but show no deployment or standards uptake.
   - Policy-moving documents (the NHTSA report, CAMP and SCMS reports) are under-counted.

**Implication for this engine.** The type with outsized and lasting impact that is open to a master's student is the *platform plus benchmark*: a VeReMi- or OPV2V-style reference artefact for the **SCMS and misbehaviour layer at system scale**, including:
- scenarios;
- metrics;
- baselines;
- a leaderboard;
- a runnable, open engine.

No equivalent exists for the SCMS layer in the catalog (F2MD stops at local detection).

Top-venue landing additionally needs a real-target hook: the standard SCMS/1609.2.1 behaviour, plus one finding validated outside the simulator.

The adoption blockers to fix first (critique-methods.md):
- about 2.4 h per 600 s run;
- RNG keyed per entity rather than per event;
- V2X warnings that do not yet change driving.
