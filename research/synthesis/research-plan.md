# Research plan: three refined directions

Written 2026-10-18. Merges `big-three.md` with `refine-pqc.md`, `refine-bench.md` and `refine-remediation.md`; those files hold the evidence and citations for every claim here. Deadlines marked *inferred* come from the previous year's cycle and must be re-checked when the call is published.

**Order:** start with Direction 1 (quantum-safe migration). Build Direction 2 (SCMS-Bench) on the shared prerequisites as the second paper. Direction 3 (remediation) comes third, once the evidence bridge exists. §6 gives the reasons.

## 1. Direction 1: Quantum-safe migration order for the US SCMS

**Thesis.** For the US SCMS, the order of migration matters more than which algorithm is chosen. Recordings of the pseudonym-provisioning channel made today let a future quantum adversary link archived pseudonyms to enrolments. So the provisioning KEM and the infrastructure certificates must migrate first. BSM pseudonym signatures can stay classical, under a crypto-agile hybrid certificate cycle, until close to 2035. (NIST IR 8547 is still a draft, and it disallows P-256 only after 2035.)

**Headline result wanted.** On a J3161-compliant LTE Mode 4 and NR Mode 2 channel at USDOT Plan densities:
- each PQ-everywhere or hybrid-everywhere profile falls below 90% PDR at a named density and fleet share;
- the infrastructure-first, provisioning-KEM-first order stays within about 5 points of ECDSA awareness through 2035;
- that order closes retroactive-linkability exposure N years earlier;
- per-certificate PQ keys (P1) multiply provisioning upload and PCA load by a named factor. That factor is the number 1609 needs to decide whether a PQ butterfly is worth standardising.

**Contributions.**
1. The first system-scale, standards-compliant evaluation of PQ migration for the US SCMS. It covers certificates, P2PCD, provisioning, CRLs, a mixed fleet by model year, J3161 SPS+one-shot and NR Mode 2.
2. A provisioning cost and crossover model over four families: P0 ECC butterfly with PQ CA signatures (chen2026pseudonym); P1 per-certificate PQ keys (the engine's current model); P2 PQ butterfly (qSCMS; ePrint 2024/946); P3 P0 plus a hybrid KEM on the download path. The thesis evaluates these families and invents no new cryptography.
3. A harvest-now-link-later privacy analysis of SCMS provisioning. *Its novelty is unverified; search before claiming it.*
4. A migration-order profile with break-points and hybrid-verification rules, as input to the 1609 WG.

**Experiment matrix.**

| Exp | Grid | Runs | Where |
|---|---|---|---|
| E1 validity | abrar's 6 LOS levels × {LOS, NLOS} × {ECDSA, Falcon-512} × 5 seeds, plus a twardokus redundancy check | 129 | local, 20–30 h |
| E2 trajectory (= Bench T4) | PQ share {10, 30, 60, 100%} × 3 densities × {LTE-M4, NR-M2} × 6 profiles × 5 seeds | 720 | cloud, 300–600 h |
| E3 provisioning/CRL | P0–P3 × {ML-DSA-44, Falcon-512, ML-DSA-65} × batch {20, 60/wk} × {1k, 10k} × 3 seeds | 144 | local, ~12 h |
| E4 hybrid attacks | 3 verify rules × {strip, oversize flood} × 2 rates × 2 densities × 5 seeds | 120 | ~40 h |
| E5 hardware | sign/verify and KEM timings on two OBU-class ARM boards (liboqs) | n/a | ~1 week |

The six E2 profiles: ECDSA, ML-DSA-44 everywhere, hybrid everywhere, PHA cycle, infrastructure-first hybrid PC, and symmetric authentication with periodic PQ signatures (PQC-TR §5.2.4).

**Validation anchors.**
- abrar2026deployment: 1,735 B, 7 subchannels, 90% PDR only at LOS-A;
- twardokus2024cryptography: 93% certificate redundancy;
- USDOT LTE-V2X field PER (ylmaz2025simulated) and NIST 1609.2 captures (song2024c);
- FIPS 204/206 sizes and NYC weekly batch sizes.

**Figures.** F1 validity; F2 break-point map (fleet share × density, LTE vs NR); F3 trajectory 2027–2036; T1 provisioning cost; F4 retroactive linkability vs year; T2 hybrid rules: attack success vs backlog.

**Prerequisites.** Per-vehicle crypto capability; J3161 one-shot; provisioning families P0, P2, P3; a harvest-now-decrypt-later metric; the symmetric arm; hardware timings; a cloud runner (§5, steps 2, 4, 7, 12). Per-event RNG and warning-driven driving are *not* needed.

**Risks.**
- **RIT.** Rahbari's NSF CAREER award promises a migration roadmap, and Twardokus's August 2026 thesis covers hybrid C-V2X and a VPKI design. Answer: own the SCMS backend, provisioning, harvest-now-decrypt-later and the mixed-fleet calendar, which RIT does not touch.
- **Clemson/TraCR.** Their certificate-segmentation work is due by Dec 2026. Answer: include segmentation as an arm, and preprint E1+E3 first.
- **H4 already known.** Answer: run the novelty search in week 1.
- **Fallback.** A paper on provisioning cost and retroactive linkability (E3+E4+H4) for VNC, PETS or TVT.

**Venue.** VehicleSec '27 short paper, late Feb 2027 (*inferred*). Full paper to the NDSS 2028 summer cycle, about early May 2027 (*inferred*). USENIX Security '27 (26 Jan 2027, confirmed) is too early. Journal: IEEE TIFS.

---

## 2. Direction 2: SCMS-Bench

**Thesis.** The field ranks misbehaviour detectors by message-level F1. Inside the real reporting-and-revocation loop, a detector's safety value depends on which messages it catches, when it catches them, and what its false positives and wrongful revocations cost. SCMS-Bench is the first public benchmark to score the whole loop (detector → MBR → MA → CRL) by a safety-weighted counterfactual outcome. It narrows big-three's gap claim: zhang2024enhancing, kalogiannis2022attack and wehmer2021practical each do part of this, but none releases a multi-detector benchmark or includes the SCMS.

**Headline result wanted.**
- Across 10 or more detectors, the F1 rank and the Net Safety Score (NSS) rank agree only weakly: Kendall τ ≤ 0.5, with a CI excluding 0.8.
- A top-3 F1 detector falls into the bottom half.
- Second result: the local recall at a given FP rate above which the MA's "do nothing" option is within ε of an oracle. That is the MPR value MBD-Op §5.2.1 asks for.

**Contributions.**
1. An evaluation-validity finding (F1 vs NSS), attributed to catch timing, attack class and FP-harm.
2. The benchmark:
   - frozen scenarios;
   - tasks T1 detection, T2 MA policy, T3 revocation distribution, T4 PQ configuration;
   - a trace tier exported in VeReMi and NextGen formats, plus a closed-loop tier;
   - leakage-linted releases and a published RNG key contract.
3. A metric suite in the CARLA-leaderboard style: NSS, time to neutralise, wrongful-revocation exposure and robustness labels.
4. The MPR curve for "do nothing".

**Experiment matrix.**

| Tier | Grid | Runs | Where |
|---|---|---|---|
| A open-loop replay | 12 detectors × 34 + 6 NextGen attacks × 2 maps × 2 densities × 10 recordings | 40 rec. | local, ~11 h |
| B closed loop | (10 + none + oracle) × 6 attack families × 2 compliance levels × 2 maps × 10 seeds | 1,440 | 120–400 h, local or cloud |
| C scale | 3 detectors × 2 attacks × 5 seeds at ~450 vehicles | 30 | cloud, 72 h |
| D robustness | Morris screening, r = 10 over ~8 factors | ~90 | ~1 day |
| E MPR | 7 recall levels × 3 FP rates × {do-nothing, LegacyWindow, Corroborated} × 10 seeds | 630 | 2–4 days |

**Validation anchors.**
- Detector F1 order on our trace tier matches its order on the NextGen test set (comparability).
- SUMO cross-validation; Veins/OpenCV2X-matched PHY; 1609.2 test vectors.
- bauder2023cooperative pseudonym behaviour; TTC < 1.5 s.
- Swept warning-response parameters (yan2014driving).
- A placebo detector must give NSS ≈ 0 and must be shown to go red under the current keying.

**Figures.** Pipeline schematic; bump chart of F1 rank vs NSS rank (the headline); NSS decomposition; attack × detector disagreement heatmap; MPR curve; feature table against VeReMi, NextGen, DARE, MisbehaviorX and zhang2024, plus the placebo audit.

**Prerequisites.** RNG keys; `bench replay`; NextGen exporter and importer; `PyDetector` rebind; warning-response model; do-nothing MA and a synthetic detector; the six missing attacks; manifest and scorer (§5, steps 1, 3, 8–10, 12).

**Risks.**
- **Ulm closes the loop first.** VeReMi NextGen announces closed-loop evaluation, and the group is funded. Answer: make the SCMS loop and wrongful revocation the paper's identity, post the Tier-A note on arXiv about Jan 2027, and offer NextGen co-hosting.
- **Invented driver response.** Answer: sweep it and report a robustness label.
- **Adoption.** Answer: zero-install traces (Hugging Face and Zenodo), a Docker image, a scorer CLI and a leaderboard Space.
- **Fallback if τ is high.** "F1 is a valid proxy, except for classes X", plus the MPR curve.

**Venue.** NeurIPS 2027 Evaluations & Datasets track, early May 2027 (*inferred*; the 2026 deadline was 4/6 May; code and data required at submission). Backup: IEEE VNC 2027, about Feb 2027 (*inferred*). Journal: IEEE TDSC.

---

## 3. Direction 3: Evidence-grade misbehaviour remediation

**Thesis.** The evidence that TS 103 759 v2.2.1 (published Jan 2026) and the CV-pilot MBR require lets an outsider with a ~$300 SDR GNSS spoofer, or a relay, produce MA-verifiable evidence against an honest US vehicle. A footprint-aware rule that needs reporters distinct at the LA, across separate events, blocks this at a stated cost. Across two SCMS providers, that holds only if the CP's "should cooperate" becomes "shall", with an SLA. The framing changes from big-three: this work *quantifies the cost-benefit that 5GAA MBD-Op §5.2.1 replaced with expert judgement*, not something 5GAA called impossible.

**Headline result wanted.**
- Under every standards-consistent proof-based MA family, `Implausible/MaxAcceleration` and the class-2 kinematic checks get a spoofed honest vehicle suspended as fast as a real attacker.
- The footprint test plus k = 2 LA-distinct events removes ≥ 99% of framings for ≤ 1 CRL cycle (24 h) of extra time to neutralise.
- The result is a count of false suspensions and how long they last, not just that framing exists.

**Contributions.**
1. A per-check framability matrix over CV-pilot MBR types and Annex D classes 1–5: the cheapest capability whose evidence survives MA re-verification.
2. A policy frontier over the 5GAA §7 families A1, A2 and A1+A2 × corroboration rules: wrongful suspensions per 10⁶ vehicle-hours, time to neutralise, LA queries and RC1–RC4 load.
3. The cost of split evidence across two providers under "should" vs "must" (CP-TR clauses 24–26).
4. An open proof-based MA and attacker set, released with a draft "misbehaviour policy envelope" (5GAA 2022 p25).

**Experiment matrix.**
- **Attackers:** A0 benign correlated GNSS fault; A1 GNSS spoofer, footprint 10/50/200 m; A2 relay at 1 or 5 km; A3 insider with n ∈ {1, 5, 20} pseudonyms; A4 sensor slander with k ∈ {1, 3} colluders; A5 forged-MBR control.
- **MA families:** P0 count; P1 event-correlated; P2 proof-based; P3 P2 plus k LA-distinct reporters; P4 P3 plus footprint test; P5 RSU witness; P6 CATS.

| Exp | Grid | Runs | Where |
|---|---|---|---|
| E1 framability | 12 attacker cells × 2 maps × 2 densities × 10 seeds, 300 s | 480 | local, ~40 h |
| E2 policy frontier | 7 families × 3 mappings × 2 MaModes × 3 k, backend-only replay of E1/E3 MBR streams | 126 × streams | minutes |
| E3 benign FP | 3 GNSS error models × 2 maps × 5 seeds at 6,000 veh/h | 30 | cloud, ~72 h |
| E4 multi-provider | cooperation {none, should, must} × fleet split {50/50, 80/20} × {A3, A4} × 10 seeds | 120 | ~10 h |
| E5 hardware (optional) | conducted-cable GNSS spoofing, or UPM traces | n/a | RF lab |

**Validation anchors.**
- ObsImplausible must come out relay-framable, matching the CV pilot's decision to turn it off.
- A1 reproduces the ~20 m/s² acceleration peaks of arXiv 2606.20215.
- GNSS error fitted to Reid 2019.
- CP-TR SLAs: 5 days and 24 h.
- SM-MBR thresholds.

**Figures.** T1 attacker cost; F1 framability heatmap; F2 policy frontier; F3 benign wrongful suspensions vs GNSS tail; F4 multi-provider time to neutralise; T2 draft MA policy with its LA query budget.

**Prerequisites.** Report-identity RNG keys; the MBR evidence bridge and P2 MA; the LA query ledger; attackers A1–A4; remediation tiers; cross-provider MA (§5, steps 1, 3, 5, 6, 10, 11). Warning-driven driving is needed only for a net-harm appendix.

**Risks.**
- **Park 2024 is unread.** It covers slander and central-MA flooding in the K-City field test. Write claim C1 as a *measured frontier*, not "the first slander defence".
- **Intel's granted patent** describes federated local MAs, which overlaps C3 as a design.
- **Suspension may be correct.** A spoofed vehicle really did send bad data, so suspending it may already be the right 5GAA outcome. Report counts.
- **The bridge refactor is real.** Budget 4 weeks.
- **Fallback.** A paper on benign GNSS faults and wrongful suspension (E2+E3) at VNC or VehicleSec. **Legal:** over-the-air spoofing is unlawful in the US, so E5 must be conducted or shielded.

**Venue.** USENIX Security '28 cycle 1 or the NDSS 2028 fall cycle, about Aug 2027 (*inferred*). VehicleSec '27 (late Feb 2027, *inferred*) only if Direction 1 does not take that slot (D3). Journal: IEEE TIFS. Standards input to 5GAA MBD4V2X, SAE J3287 and the next TS 103 759 revision.

---

## 4. How the three share engine work and experiments

- **One MA and policy layer.**
  - Bench T2 is Direction 3's policy frontier. The P0–P6 MA families are Bench baselines.
  - Bench RQ4's "do nothing" MPR curve marks the boundary of Direction 3's R0/ST0 tier. Both read MBD-Op §5.2.1, so they should share one write-up of it.
- **One attack library.** Direction 3's A0–A4 (GNSS spoofing, relay, pseudonym conflict, slander) become Bench attack families. Bench's six NextGen kinematic attacks feed Direction 3's class-2 checks.
- **One PQ grid.** Direction 1's E2 is Bench track T4. Its frozen manifest is the first bench scenario release.
- **One replay path.** `bench replay` (Bench Tier A) is also how Direction 3 runs E2 backend-only on recorded MBR streams.
- **One keying contract.** Stable trip and event keys, plus report-identity keys, give exact pairs for Bench Tier B and Direction 3 E2/E3. The same placebo test guards both.
- **Shared infrastructure.** The cloud runner (Direction 1 E2, Bench C, Direction 3 E3, about 450–750 h in all), the sensitivity-harness robustness labels, and the leakage-linted exporters.

## 5. Combined engine prerequisites, in build order

| # | Build | Code | Serves |
|---|---|---|---|
| 1 | Stable trip/event RNG keys and report-identity keys; placebo and coupling audit | `crates/v2xw-core/src/rng.rs`; `crates/v2xw-mobility/src/engine.rs:3147`; `crates/v2xw-threat/src/poison.rs:307`; `crates/v2xw-radio/src/cellular.rs:823` | 2, 3 |
| 2 | Per-vehicle crypto capability and mixed-fleet receiver rules | `crates/v2xw-engine/src/scenario/schema.rs:2409-2412` | 1 |
| 3 | `bench replay` CLI over N detectors | `crates/v2xw-record/src/reader.rs:373`; `crates/v2xw-threat/src/detect.rs:351` | 2, 3 |
| 4 | J3161 §7.3.3 SPS+one-shot and SPS_004 rounding | `crates/v2xw-radio/src/sps.rs:173` | 1 |
| 5 | MBR evidence bridge and proof-based P2 MA | `crates/v2xw-threat/src/report.rs:15-20` → `crates/v2xw-proto/src/scms/mbr.rs` (l.207, l.399); a plug-in next to `crates/v2xw-threat/src/ma_corroborate.rs` re-running `ts103759.rs` | 3, 2 (T2) |
| 6 | LA query ledger, wiring and the LA-distinct rule | `crates/v2xw-proto/src/scms/run/revoke.rs:1-117`; `params.rs:337`; `crates/v2xw-threat/src/ma.rs:24-29` | 3, 2 |
| 7 | Provisioning families P0, P2 and P3; correct the "no PQ butterfly" comments; harvest-now-decrypt-later metric | `crates/v2xw-proto/src/hybrid.rs:25-30`; `crates/v2xw-sec/src/scheme/mod.rs:40-42` | 1 |
| 8 | Warning-response driver model (compliance, reaction time, braking), sweepable | `crates/v2xw-engine/src/run/coop.rs` | 2 (blocks Tier B), 3 (appendix) |
| 9 | NextGen exporter and importer; `PyDetector` rebound to `v2xw_threat::Detector`; six NextGen attacks | `crates/v2xw-record/src/dataset/veremi.rs`; `crates/v2xw-py/src/plugins.rs`; `crates/v2xw-threat/src/attack*.rs` | 2 |
| 10 | Attackers A1 (victim GNSS override), A2 (relay), A3 and A4 | `crates/v2xw-node/src/runtime.rs:785-787`; `crates/v2xw-mobility/src/gnss/`; `attack.rs:558` | 3, 2 |
| 11 | Remediation tiers (ST1, RC1–RC4, A1/A2 mappings); per-provider MA and cross-MA requests | RA blocklist in `revoke.rs`; `deployment.rs:300` | 3 |
| 12 | Symmetric+periodic arm; hardware timing import; short windows and cloud runner; bench manifest and scorer | `crates/v2xw-node/src/profiles.rs`; `crates/v2xw-experiment` | 1, 2 |

Steps 1–3 come first because each unblocks two directions. Steps 5 and 8 are the two large refactors, about 4 weeks each.

## 6. Twelve-month timeline (Nov 2026 – Oct 2027)

| Month | Engine | Experiments and writing | Submission |
|---|---|---|---|
| Nov 2026 | 1, 2, 3 | Direction 1 H4 novelty search; E1 reproduction; order ARM boards; Bench Tier A; request Park 2024 and J3287 | — |
| Dec | 4, 7 | Direction 1 E3, E5; first Bench F1 table | arXiv: Direction 1 E1+E3 note, before TraCR's 31 Dec end |
| Jan 2027 | 12 (cloud), 9 | Direction 1 E2 on cloud; NextGen cross-check | arXiv: Bench Tier-A rank note |
| Feb | 5 | Direction 1 E4; short paper | **VehicleSec '27** (Direction 1, *inferred* late Feb) |
| Mar | 5, 6, 8 | Direction 1 full paper; 1609 WG contribution draft; Bench Tier B pilot | — |
| Apr | 8, 10 | Bench Tier B/E; Tier C on cloud; Morris Tier D | — |
| May | — | Freeze Bench v1.0 | **NDSS 2028 summer** (Direction 1), **NeurIPS 2027 E&D** (Bench); both *inferred* early May |
| Jun | 10, 11 | Direction 3 E1 framability; E3 on cloud | Revise against VNC/VehicleSec feedback |
| Jul | 11 | Direction 3 E2 frontier, E4 multi-provider; E5 or UPM traces; contact an MA operator | — |
| Aug | — | Direction 3 write-up | **USENIX Security '28 cycle 1 or NDSS 2028 fall** (Direction 3, *inferred*) |
| Sep | — | Leaderboard v1.1 adding Direction 3 attacks and MAs; NeurIPS response | 5GAA MBD4V2X / SAE input |
| Oct 2027 | — | Thesis chapters; journal extension (TIFS) | — |

## 7. Decisions the owner must make

- **D1. Order and thesis shape.** Confirm Direction 1 first. Decide whether the thesis is three papers, or one primary direction with the other two as chapters.
- **D2. The May 2027 double deadline.** NDSS and NeurIPS fall in the same week. Options: keep both; move Bench to VNC 2027 (Feb, too early for closed loop); or move Direction 1's full paper to USENIX Security '28 (~Aug 2027).
- **D3. VehicleSec '27 slot.** Use it for Direction 1 (the plan above), or for Direction 3's framability paper, which pulls steps 5–6 forward to Nov–Jan.
- **D4. Compute budget.** Approve about 450–750 cloud hours across the three directions, or accept smaller grids.
- **D5. Hardware.** Two OBU-class ARM boards for Direction 1 E5. A conducted or shielded GNSS-spoofing setup for Direction 3 E5, or a collaboration with UPM/INSIA for their traces.
- **D6. Outreach the owner does personally:** Ulm (NextGen co-hosting), UPM (GNSS traces), an MA operator such as ISS MAS, an IEEE 1609 WG contact, and the Park authors.
- **D7. Release policy.** Hugging Face, Zenodo and licences (CC BY 4.0 for data, Apache-2.0 for code), and how much of the engine to open by May 2027.
- **D8. Driver-response realism.** Accept a swept-parameter warning model as sufficient for Bench Tier B, or seek driving-simulator data.

## 8. Sources the owner should read personally

- **Read first (each result rests on it):** the 5GAA/GSMA PQC report (6 Oct 2026), §5.2, §5.2.4 and §6; Park 2024 (IET ITS, *Enhancing…*; Cloudflare 403, so use a library or ILL); SAE J3287 (paywalled); 5GAA MBD-Op v1.0 §1, §5.2.1 (p5, p12) and §7–8.
- **Direction 1:** NIST IR 8547 ipd Table 2 (check whether a final version exists); ePrint 2024/946 and qSCMS (ePrint 2018/1247); abrar2026deployment pp. 1–3 and 18–19; chen2026pseudonym §I and arXiv 2501.07028; Twardokus's thesis (repository.rit.edu/theses/12750); Rahbari's NSF CAREER abstract; TRB RiP 2531081; a targeted search for harvest-now-link-later prior art (H4).
- **Direction 2:** hermann2026veremi §V and §VIII-B, plus whether NextGen's `sender_id` leaks the real id; zhang2024enhancing; wehmer2021practical; Bassi et al., VTC2026-Spring; the NeurIPS 2027 E&D call when posted.
- **Direction 3:** TS 103 759 v2.2.1 §4.2.4, §6.2 and Annex D; 5GAA CP-TR clauses 13 and 24–26 (p57 SLAs); Intel US12489779B2; arXiv 2606.20215 §5; A4MD (VNC 2023); DRAMBR (Future Internet 2025); Jerbi et al., VNC 2026 poster.
- **Calls to recheck:** VehicleSec '27, NDSS 2028 and USENIX Security '28.
