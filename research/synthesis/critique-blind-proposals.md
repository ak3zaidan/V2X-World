# Blind proposals: three research directions for the engine

Written 2026-10-17 as one part of an adversarial review. The rule was to propose directions without reading the existing synthesis.

**How blind this really is.** I did not open any other file in `research/synthesis/`, or any file whose name contains "direction", "novelty" or "eval". Two things leaked through, though:
- When I printed the "What the engine does" section of `PROTOCOL.md`, the next lines came out too. They list the current candidate directions and runners-up.
- The relayed task context summarised the earlier proposal: wrongful revocation, net-safety, and a sim-to-real methodology study.

So this proposal is blind to the synthesis files' arguments, not to their headlines. Where my picks overlap the earlier ones, I say so.

**Sources used:**
- `catalog.jsonl`, searched by regex over titles and abstracts;
- `summaries/`;
- `batches/batch-*-notes.md`, the "limitations and future work" lines;
- three PDFs;
- fresh searches on OpenAlex, WebSearch and WebFetch.

**Search gaps:**
- The arXiv API returned HTTP 429 or an empty feed on every call, so arXiv was searched only through `site:arxiv.org` web queries and through arXiv records in the catalog and on OpenAlex.
- Semantic Scholar returned 429 on every call.
- Standards (SAE J2945/1, J3287, IEEE 1609.2.1, 3GPP TS 33.185/33.536, 36.321/38.321) were **not opened**. Every claim about them below is marked as a claim to verify.

**How I picked the three.** I asked what the engine can measure that no other single tool can. My answer is one causal chain, observed jointly and re-run exactly with one thing changed:
- the radio MAC state (SPS reservations, identifiers);
- the full US SCMS (RA, PCA, LA, MA, CRL, CTL with electors, cellular provisioning);
- city-scale traffic.

I then searched for directions where that chain is the only way to answer the question. That ruled out the following:
- **Pure SCMS operations** (provisioning and CRL dimensioning) is crowded: Khodaei's SECMACE/SECMACE+, on-demand pseudonym acquisition and CRL distribution work (`khodaei2023secmace`, `khodaei2016evaluating`, `khodaei2018efficient`), plus 57 CRL-distribution papers in the catalog.
- **Standalone post-quantum** is crowded: `twardokus2024cryptography` (NDSS 2024), `lonc2023feasibility`, `yoshizawa2023post`, `barreto2018qscms`, and 119 PQ-V2X hits in the catalog.
- **LLM agents for V2X**: novelty depends on the agent, not on the engine, and the area is noisy (`hu2025llm`, `hamhoum2024mistralbsm`, `carie2026hierarchical`).

---

## Direction A: a privacy audit across layers and observers for pseudonymous V2X (the lead case is C-V2X scheduling)

**Pitch.** Every privacy study of pseudonym change looks at one layer and one observer. Most use kinematics at the application layer with an eavesdropper. A few use the 802.11p physical layer.

A real US vehicle leaks across at least five channels at once:
1. kinematics and static fields in the BSM;
2. the 1609.2 signer (digest versus full certificate, and when the certificate changes);
3. the radio MAC (in LTE-V2X and NR Mode 2, a semi-persistent reservation announced in SCI, with its reservation interval, subchannels and reselection counter);
4. the provisioning channel (cellular TLS sessions to the RA, behind or without the location obscurer proxy);
5. misbehaviour reports, which hand the MA pseudonyms and locations of both the reporter and the reported vehicle.

The direction builds a **linkability matrix of observer × channel**. The observers are an eavesdropper network, an RSU operator, the cellular operator, the RA, the MA, and coalitions of these. Each cell is measured against per-message ground truth of the true vehicle identity, in the same run, for DSRC, LTE-V2X and NR-V2X.

**Lead case.** Does an SPS reservation outlive a pseudonym change? Even when it does not, does "one reservation stops, another starts next to it at the same instant" re-link the vehicle? If so, a mix zone fails at the MAC layer however well the application-layer identifiers are synchronised.

**Research questions.**
1. For each channel and each observer, what fraction of pseudonym changes can be linked under US policy (SAE J2945/1-style certificate change) at city scale? What is the posterior entropy?
2. Does sidelink resource continuity (reservation persistence, reselection timing, reservation-interval fingerprint) link pseudonyms that application-layer synchronisation would protect? Which MAC rule closes the gap, for example forced reselection with a random gap at ID change? What does that rule cost in packet delivery and safety?
3. What does each SCMS insider, or pair of insiders, learn from protocol traffic it legitimately sees? This covers RA timing with and without the proxy, and MA report streams. How does that compare with the non-collusion assumption in the SCMS design?
4. What is the joint "privacy budget": how much does combining channels beat the best single channel?

**Closest prior work and how this differs.**
- **Application-layer tracking:**
  - Escher et al., European C-ITS pseudonym scheme on LuST, kinematics plus static attributes (`escher2021well`, doi 10.1109/VTC2021-Spring51267.2021.9449078).
  - Khodaei, about 62–73 % of pseudonyms linked across mix zones (`khodaei2020cooperative`).
  - PREXT and F2MD pseudonym strategies (`emara2016poster`, `kamel2020simulation`).
  - Strategy surveys (`boualouache2017survey`).
- **Physical-layer linking, 802.11p only:**
  - the scrambler attack (`bloessl2015scrambler`);
  - PHY properties across encrypted mix zones (`vaas2018poster`);
  - a second radio's long-term ID (`yieh2019multi`).
- **C-V2X:**
  - Twardokus and Rahbari show that SPS makes transmissions predictable, but use this only for DoS (`twardokus2022vehicle`, INFOCOM 2022).
  - Bitsikas et al. note that 3GPP requires Layer-2 ID and IP randomisation for broadcast and groupcast, but leaves the frequency and cross-layer synchronisation undefined (`bitsikas2025security`, arXiv 2502.16650, §7.1.1, read this session). They do not measure reservation-based linking.
- **Insiders:** the SCMS design argues privacy against single insiders qualitatively (`brecht2018security`). PriMiRe proposes MA aggregation that preserves privacy for ETSI (`willecke2024primire`). Neither quantifies leakage on realistic traffic.

**Searches for linking through SPS or reservations found nothing.** This covered the catalog regex, OpenAlex, two extended web searches and a `site:arxiv.org` query. That is absence of evidence, not proof. Before claiming novelty:
- Ask whether 36.321 §5.14 or 38.321 §5.22 already forces reselection on an L2 ID change. Even if they do, research question 2's "adjacent restart" question stands.
- Run a full arXiv pass.

**Why the engine is placed to do this.**
- It runs MAC state matched to OpenCV2X (`crates/v2xw-netval/src/sps.rs`), the 1609.2 signer, the SCMS backend (including the location obscurer proxy, electors and the MA) and traffic in one process, with a digest-to-actor ground-truth map.
- A passive privacy observer already exists (`crates/v2xw-threat/src/privacy.rs`: kinematic best hypothesis, msgCnt option, silent periods). Adding channels extends that observer rather than starting from scratch.
- Exact counterfactual reruns isolate the cost of each fix: the same seed, with only the reselection rule changed.
- No other public tool joins a US SCMS insider view with C-V2X MAC state.

**Useful to:**
- 3GPP SA3/RAN2 (identifier-change and reselection rules);
- the SAE V2X committees (certificate-change rules in J2945/1 and J3161/1, to verify);
- IEEE 1609 (provisioning privacy);
- the SCMS Manager (assumptions about insiders).

**Venue:** PoPETs/PETS or USENIX Security; ACM WiSec or IEEE VNC for a short version; IEEE TIFS or TDSC for the journal.

**Biggest risk.** The core MAC finding could already be closed by a spec rule or by chipset practice, or a reviewer could say "identifiers must change together, already known". **Mitigation:**
- Lead with the measured joint matrix and the insider coalitions, not with any single leak.
- Verify the MAC rules first.
- Treat the PHY results as conditional on the MAC model, since the engine does not model RF impairments, so fingerprinting work à la `vaas2018poster` stays out of scope.

---

## Direction B: a benchmark that labels misbehaviour by its causal harm

**Pitch.** Misbehaviour detection is scored as message classification on VeReMi and its extension (`heijden2018veremi`, `kamel2020veremi`). Those labels say "this message is false", never "this message mattered".

Audits now show the benchmark measures artefacts:
- `muzammil2026code` is the replication package for "What VeReMi Measures: Structural Artefacts…" (Zenodo 10.5281/zenodo.22397724).
- Sender identity leaks into the evaluation (`skaik2026reliable`, SSRN 10.2139/ssrn.7411187).

This direction would release a benchmark whose label for each message, or each sender-episode, is **interventional**: rerun the world with that message suppressed or replaced by the honest value, and record the change in conflicts, hard braking and crashes. Detectors are then ranked by harm averted per false alarm, not by F1.

The benchmark adds PKI ground truth (which pseudonyms belong to one device, and when revocation lands) and all three radios. A second track uses adaptive attackers, who maximise harm while evading a given detector inside the closed loop.

**Research questions.**
1. How do detector rankings change when F1 is replaced by counterfactual harm averted? What share of "detected" misbehaviour was harmless, and what share of harmful misbehaviour do top-F1 detectors miss?
2. Can harm be predicted online from features a vehicle can see? That would make it a learnable target for prioritising detection and reporting.
3. Against adaptive attackers who maximise harm in the closed loop, how much detection survives? Compare with offline adversarial examples on fixed datasets.
4. How sensitive are the harm labels to the driver model and to penetration rate? These are bounds the benchmark must publish.

**Closest prior work and how this differs.**
- **Impact over confusion matrices, but aggregate and not per message:** Wehmer et al. argue for physical-impact evaluation (`wehmer2021practical`, doi 10.5220/0010451000002932). Heijden et al. count crashes under CACC attacks (`heijden2017analyzing`).
- **Closed loop with outcomes, but no interventional labels:**
  - neural detection with accident outcomes (`franceschini2024neural`, arXiv 2411.07013);
  - CPM attacks with application impact, plus released datasets (`zhang2024simulation`, doi 10.1109/IV55156.2024.10588853);
  - a radio-realistic flood with safety outcomes (`bello2026human`, arXiv 2609.21206).
- **Counterfactuals in IoV:** IntentProv-IoV defines a counterfactual contribution score inside a structural causal model, not exact simulation reruns, and releases no benchmark (doi 10.3390/sym18091430, Symmetry 2026).
- **Adversarial misbehaviour detection, offline or on fixed data:** `shahriar2025vehigan` (doi 10.1145/3745787), `buch2026adversarial`, `zoccoli2022sixpack`.

**Why the engine is placed to do this.** Interventional labels need three things together:
- bit-exact reruns where only one thing changes;
- ground truth that joins messages, receivers and outcomes;
- detection, reporting and revocation in the loop.

The engine has all three. Replay datasets cannot produce such labels.

**Useful to:** ML and misbehaviour-detection researchers (a replacement benchmark); 5GAA, ETSI and SAE work on misbehaviour reporting (which reports deserve MA bandwidth).

**Venue:** NeurIPS Datasets & Benchmarks, or NDSS/CCS framed as an evaluation-methodology paper; journals IEEE TIFS or TDSC.

**Biggest risk.**
- **Label validity:** "harm" inherits the driver and traffic model, so a reviewer may call it simulator-defined.
- **Cost:** one rerun per message is infeasible, so sampling or group attribution is needed.

**Mitigation:**
- Publish label sensitivity across driver models.
- Restrict the labels to sender-episodes.
- Validate that the ranking flips are robust where real VeReMi-style features exist.

**Overlap note:** this is close to candidate 3 in `PROTOCOL.md`. I reached it independently from the 2026 VeReMi audits, but the overlap is real.

---

## Direction C: resilience and crypto-agility of the US SCMS, from recovering after an authority compromise to migrating algorithms

**Pitch.** The SCMS design states that the system should "heal itself" after a component is compromised, and it specifies root management by electors and re-enrolment (`brecht2018security`; arXiv 1802.05323). Nobody has measured what healing costs on a road network. The same machinery governs the planned "compromise" of a post-quantum migration.

The direction simulates incidents end to end and measures the window to restore trust and the window to restore function. The incidents:
- compromise of a PCA, ICA or ECA;
- an elector or root replacement through CTL ballots;
- a staged move from ECDSA to hybrid to PQ.

The **window to restore trust** is how long until no vehicle accepts the compromised chain, via CRL and CTL spread over RSUs, cellular and P2P. The **window to restore function** is how long until vehicles are re-provisioned through RA throughput over real cellular coverage. The **safety cost** is what happens between the two: vehicles whose credentials are untrusted go unheard. A second axis covers prevention: threshold authorities (FROST, CGGMP21) as a control on compromise probability, against their cost in latency.

**Research questions.**
1. After each authority compromise, how long do the trust window and the function window last at city scale? How many vehicle-hours of unverified or unheard BSMs build up?
2. Which recovery policy minimises the safety cost?
   - hard revoke-and-reissue;
   - a grace period with parallel chains;
   - priority re-provisioning by exposure;
   - pushing the trust list through RSUs.
3. For a PQ migration, which schedule keeps every pair of vehicles mutually verifiable and keeps the channel load safe in the mixed fleet? The schedule covers when issuance flips, when receivers start requiring PQ, and whether to issue hybrid certificates. How does the answer differ across DSRC, LTE-V2X and NR-V2X?
4. Measured against these recovery costs, when are threshold authorities worth their overhead?

**Closest prior work and how this differs.**
- **Design-level recovery and root management, no measurement:** `brecht2018security`; blockchain root of trust (`sarker2021blockchain`, doi 10.1145/3412841.3441905); an SCMS emulation testbed (`mushrall2018emulab`).
- **Resilience of a vehicular PKI to load and DDoS, European-style:** `khodaei2023secmace` (doi 10.1109/TCC.2023.3250584).
- **Revocation timing for single vehicles, not authority compromise:** `scopelliti2024efficient` (NDSS 2024, doi 10.14722/ndss.2024.24017).
- **Post-quantum migration:**
  - qualitative analysis of migrating C-ITS and SWIM PKIs (doi 10.34190/iccws.21.1.4479, ICCWS 2026);
  - PQ feasibility and certificate sizes (`lonc2023feasibility`, `yoshizawa2023post`);
  - PQ authentication over LTE-V2X (`twardokus2024cryptography`, doi 10.14722/ndss.2024.24267);
  - hybrid SCMS certificates (doi 10.1109/ICJECE.2026.3692881).
  - None of these simulate a mixed fleet through a staged migration with live CTL and CRL.
- **Measured reload over 4G:** `haidar2021performance` (doi 10.1109/NTMS49979.2021.9432646). It provides calibration data, not a competitor.
- **Background:** Ott and Peikert's research challenges in crypto-agility (arXiv 1909.07353).

**Why the engine is placed to do this.**
- In the repository it has elector-signed CTLs, elector replacement, the CRL for a revoked ACA, PQ and hybrid signatures, FROST and CGGMP21 authorities, and re-enrolment, with tests in the `crates/` tree.
- It is validated against the real trust lists of the SCMS Manager.
- It runs cellular provisioning over TCP/TLS with city traffic and three radios.
- Exact reruns compare recovery policies on the same incident.
- The combination of real trust lists, a full backend and radio-coupled traffic is rare. It appears in no catalog entry I found: the catalog regex returned 4 hits for SCMS compromise, recovery or trust lists, none of them quantitative.

**Useful to:** the SCMS Manager and USDOT (incident-response and PQ transition planning); the IEEE 1609.2.1 and ETSI TS 102 941 trust-list mechanisms; OEM security operations.

**Venue:** NDSS (or VehicleSec), ACSAC, or IEEE S&P if the result is a clear policy surprise; journals IEEE TDSC or TITS.

**Biggest risk.**
- **No public data on real incidents:** RA throughput, fleet connectivity, and how often vehicles check the CTL are assumptions.
- **It may read as "what-if engineering":** a reviewer may say the outcome is driven by the parameters chosen.

**Mitigation:**
- Calibrate on field reload measurements (`haidar2021performance`) and published SCMS proof-of-concept figures.
- Report results as phase diagrams over the uncertain parameters, not as point estimates.
- Look for one robust policy inversion, such as "priority re-provisioning beats a grace period whenever X".

**Overlap note:** this is the least overlapping of the three with the `PROTOCOL.md` list. It absorbs the PQ and threshold pieces that were judged crowded or narrow on their own.

---

## How the three relate

A is privacy, B is detection and its consequences, and C is trust infrastructure. All three rest on the same two engine properties: ground truth joined across radio, PKI and traffic, and exact single-change reruns.

They are independent theses. A could also feed a section of C, because re-provisioning traffic is itself a channel for linking.

My ranking, for a master's student:
1. **A** has the sharpest novelty and the cleanest experiments, with an existing observer to extend.
2. **B** has high impact, but labelling it is costly.
3. **C** is the most useful to standards bodies, but has the biggest problem with calibration data.
