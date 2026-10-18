# Refining Direction 2: SCMS-Bench

Written 2026-10-18, about 25 minutes. Refines big-three.md §3, using big-needs.md, big-anatomy.md, critique-methods.md and PROTOCOL.md.
- **Full text read:** hermann2026veremi, heijden2018veremi, kamel2020simulation, xu2022opv2v and zhang2024enhancing (PDFs in `research/papers/`), plus the 5GAA MBD-Op white paper (downloaded fresh).
- **Read online:** the CARLA Leaderboard v2.1 evaluation page and the NeurIPS 2026 E&D call.
- **Abstracts only:** everything else.

## 1. Source check: claims in big-three.md §3

| Claim in big-three.md | Verdict | Evidence |
|---|---|---|
| VeReMi framed itself as "comparable evaluation" | **Confirmed.** It was also "the first such dataset publicly available": 225 simulations, 5 attackers × 3 attacker densities × 3 traffic densities × 5 repetitions, with the Maat detector runner | heijden2018veremi §1–2 |
| VeReMi NextGen: MOSAIC, 15 attacks, train/val/test splits, a generator, VNC 2026 | **Confirmed.** Also: InTAS (Ingolstadt), 4 scenarios per split, 20% attackers, 197 GB, CC BY 4.0, hosted on Zenodo 19665762 and GitHub VeReMi-dataset/VeReMi-NextGen. It evaluates **one** MDS (CaTch, with thresholds tuned for F1). **No RSSI** | hermann2026veremi §IV–VII, Table IV; https://veremi-dataset.github.io |
| "Every V2X security dataset since scores detectors as classifiers on traces" | **Confirmed, and stronger than stated.** NextGen injects attacks **in post-processing** of a baseline run, so attacks never affect any receiver's driving. The authors concede the dataset "cannot capture feedback loops", that such MDSs "must be evaluated in running simulations", and that their setup "can be easily extended… to enable such closed-loop evaluations" | hermann2026veremi §V, §VIII-B |
| "Nothing scores a detector or MA policy by what it does to safety in the loop" | **Overstated; must be narrowed.** Three works already do part of it: (1) zhang2024enhancing (VTC-Fall 2024, IRT SystemX) scores MBD by its effect on an IMA safety application, with collision risk and a kinematic reaction in the loop, for CPM attacks; (2) kalogiannis2022attack (WiSec 2022) measures attack impact plus MBD in closed-loop platoons; (3) wehmer2021practical (VEHITS 2021) shows packet-level confusion matrices "do not allow to derive valid statements about the real-world impact" and proposes a physical-impact metric | catalog summaries; https://www.scitepress.org/Papers/2021/104510 |
| Gap: no public benchmark of the credential lifecycle, or of detectors and MA policies under closed-loop counterfactual safety | **Holds** after the narrowing above. None of the three works releases a benchmark, ranks many detectors, or includes the SCMS (reports → MA → CRL/linkage revocation). The web and arXiv searches below found none | §2 below |
| 5GAA MBD-Op §5.2.1: MPR "may need to be considered" | **Quote confirmed; context corrected.** It sits under the **"do nothing" remediation option**, in the "Easily detectable" case: because device ability "can vary widely", MPR may be needed. So the MPR question is *when local detection is good enough that the MA need not act*. That makes it a better hook than a generic standard for detector quality, and it links T1 to T2 (Finalist 3) | 5GAA MBD-Op p.5 §5.2.1 (https://5gaa.org/content/uploads/2024/06/5gaa-wi-mbd-op-white-paper.pdf) |
| arXiv 2509.16489, authors "not verified" | **Resolved.** Ahmad, Abrar, Dasgupta and Rahman (the Rahman group; Abrar also wrote abrar2026deployment). It is an ITS testbed with no SCMS and no benchmark tasks | arXiv API |
| VeReMi + Extension have 492 citations; F2MD 164; OPV2V 736 | **Confirmed** against catalog counts (255 + 237; 164; 736) | catalog.jsonl |
| F2MD's stated future work is "an intelligent MA" and "a PKI module" | **Confirmed.** It reinforces why T2/T3 are open | kamel2020simulation, summary |
| "Exporters in VeReMi Extension and NextGen format" | **Partly built.** `crates/v2xw-record/src/dataset/veremi.rs` writes VeReMi (2018) field names, one JSON-lines file per receiver plus a ground-truth log, and deliberately does not leak the real sender id. **There is no NextGen exporter**: NextGen uses per-vehicle JSON with `attacker`, embedded receiver/sender noise fields and `distance_to_road_edge` (hermann2026veremi Listing 1). NextGen's `sender_id` next to `sender_alias` may repeat the F2MD real-id leak that `veremi.rs` documents. **To verify** | code; hermann2026veremi §VI-B |

## 2. Competitor watch (2025–2026)

**Ulm group (Kargl, Erb, Hermann)**
- **Recent work:** VeReMi NextGen (VNC June 2026) plus an F1-tuning framework for CaTch thresholds. Bassi, Zhang, Jemaa, Kargl and Erb's VTC2026-Spring paper, "Improving Misbehaviour Detection Through Infrastructure Support…", ties Ulm to the IRT SystemX group behind zhang2024enhancing (per a web-search snippet, unread).
- **Funding:** EU HORIZON CONNECT 101069688 and ConnRAD 16KISR036 (hermann2026veremi footnote).
- **Likely next paper:** closed-loop NextGen with VRUs on MOSAIC, which their limitations section announces. **This is the biggest new risk** (§6).

**KTH (Papadimitratos)**
- **Recent work:** PAMPOS (arXiv 2605.06833), a self-supervised trajectory-prediction MDS evaluated on all "19 attack types in VeReMi++"; AIMformer for platoons (arXiv 2512.15503); Jin and Papadimitratos on DoS-resilient authentication (arXiv 2505.22162).
- **Gap:** no "VeReMi++" paper was found. The 19 attack types match VeReMi Extension, so it is probably a renamed Extension (**inferred**).
- **Likely next paper:** more ML detectors, which are candidate **benchmark entrants**, not competitors.

**CTTC (Sedar, Kalalas):** DRL-based MBD under partial observability (arXiv 2609.31217, Sep 2026), from SUCCESS-6G (deliverable E8 DEVISE). Another detector to enter.

**IRT SystemX and Inria (Ben Jemaa, Bassi):** the closest work on closed-loop safety (zhang2024enhancing), using CPM and IMA. Their future work names trust-based MBD and more scenarios. They could become the closed-loop benchmark rival.

**Datasets in the same mould:**
- MisbehaviorX (chen2024misbehaviorx; Qualcomm and Virginia Tech; VASP on Veins, Boston);
- SPADE (arXiv 2609.02741; a MOSAIC SPaT-attack dataset).

Both are open-loop, with no PKI.

**How to differentiate**
- **Position as complementary.** Export to VeReMi and NextGen formats and score NextGen entrants unchanged.
- **Own the axes they cannot add cheaply:**
  - the real US SCMS (MA, linkage revocation, CRL distribution);
  - joint radio, PKI and traffic ground truth;
  - wrongful revocation as a measured harm.
- **Publish the rank-flip result before Ulm closes the loop.**
- **Invite Ulm to co-host the NextGen-format track** rather than race them. A shared yardstick is what drives adoption (big-anatomy §3.2).

## 3. Sharpened contribution

**Thesis.** The field ranks misbehaviour detectors by message-level F1. Once a detector runs inside the real reporting-and-revocation loop, its value to safety is set by *which* messages it catches, *when*, and what its false positives and wrongful revocations cost. SCMS-Bench is the first public benchmark that scores the whole loop (detector → MBR → MA → CRL) by a safety-weighted, counterfactual outcome.

**Headline result wanted.** Across ≥10 detectors (rule-based, CaTch, ML and self-supervised):
- the F1 ranking and the Net Safety Score ranking agree only weakly (Kendall τ ≤ 0.5, bootstrap 95% CI excluding 0.8);
- at least one top-3 F1 detector drops to the bottom half once false-positive braking and wrongful revocations are counted.

A second, decision-changing result: the minimum local detection performance (recall at a fixed FP rate) at which the MA's "do nothing" option preserves safety within ε of an oracle. This is an MPR value that 5GAA MBD-Op §5.2.1 could adopt.

**Contribution claims**
1. **An evaluation-validity finding.** Message-level F1 is a poor proxy for closed-loop safety value. The ranking disagreement is measured with confidence intervals and attributed to mechanisms: catch timing, attack class and FP-harm.
   - *Defensible:* wehmer2021practical argued this without a multi-detector ranking. NextGen concedes it cannot test it.
2. **A benchmark artefact.** It has:
   - frozen scenarios and four tasks: T1 detection, T2 MA policy, T3 revocation distribution, T4 PQ configuration (shared with Finalist 1);
   - a trace tier exportable to the VeReMi and NextGen formats;
   - a closed-loop tier;
   - leakage-linted releases;
   - a published RNG key contract.
   - *Defensible:* no prior benchmark includes the SCMS (§1).
3. **A safety-weighted metric suite** in the CARLA Leaderboard style:
   - severity-weighted infractions times a mobility term, borrowing CARLA's driving score of route completion × infraction penalty, with collision coefficients 1.0/0.70/0.60 (https://leaderboard.carla.org/evaluation_v2_1/);
   - time-to-neutralise;
   - wrongful-revocation exposure;
   - a robustness label from the sensitivity harness.
4. **An MPR curve for "do nothing" remediation,** directly answering MBD-Op §5.2.1.

## 4. Paper plan

**Research questions**
- **RQ1:** Does F1 rank detectors the same way as closed-loop Net Safety Score (NSS)?
- **RQ2:** Which mechanisms explain disagreement: catch timing, attack class (kinematic vs. Sybil/replay), FP-induced braking, wrongful revocation?
- **RQ3:** Is the ranking robust to modelling choices (warning-response parameters, propagation law, density)?
- **RQ4:** At what local recall/FP point does MA "do nothing" match an oracle MA within ε?

**Metrics**
- **Detection (open loop, exact):** confusion matrix, F1, PR-AUC, detection latency (`crates/v2xw-metrics/src/detection.rs`).
- **Safety:** severity-weighted conflicts, using TTC < 1.5 s, DRAC and PET from `crates/v2xw-metrics/src/safety.rs`, and collisions. These are paired against two arms: the same seed with no detector, and an oracle detector.
- **NSS:** NSS = (harm(no-MBD) − harm(MBD)) / (harm(no-MBD) − harm(oracle)), computed on attack runs, minus the FP-harm measured on attack-free runs. It is reported at seed level with cluster bootstrap CIs (critique-methods §4.3).
- **PKI cost:** time-to-neutralise, wrongful revocations, CRL bytes.

**Experiment matrix**

| Tier | Factors and levels | Scenarios | Reps | Runs | Cost |
|---|---|---|---|---|---|
| A, open-loop replay (exact; critique §3.6) | 12 detectors × 34 engine attacks + the 6 NextGen attacks the engine lacks (PositionMirroring, ZeroSpeed, SuddenConstantSpeed, FeignedBraking, AccelMultiplication, SuddenStop) | Midtown, highway × 2 densities | 10 recordings each | 40 recordings at ~145 veh | ~40 × 17 min ≈ 11 h local; replay is cheap |
| B, closed loop, small fleet | detector (10 + none + oracle) × attack family (6) × warning compliance (2) | Midtown and highway, ~145 veh, 600 s | 10 seeds | 12×6×2×2×10 = 2,880; cut to 1,440 by running compliance only on the top and bottom 3 | ~5–17 min each: ~120–400 h local (two to three weeks under heavy.sh), **or cloud** |
| C, scale confirmation | 3 detectors (F1-best, NSS-best, legacy) × 2 attacks | Midtown, ~450 veh | 5 seeds | 30 | 2.4 h each = 72 h: **cloud** |
| D, robustness (RQ3) | Morris screening, r = 10 trajectories over ~8 factors | small fleet | — | ~90 | ~1 day local |
| E, MPR (RQ4) | synthetic detector with tunable recall {0.2…0.95} × FP {1e-4, 1e-3, 1e-2} × MA {do-nothing, LegacyWindow, Corroborated} | Midtown, small | 10 | 7×3×3×10 = 630 | ~2–4 days local or cloud |

**Baselines and comparisons**
- **Detectors:**
  - the engine's legacy 12 plus gated checks and Kalman (F2MD-equivalent; `crates/v2xw-threat/src/detect.rs`);
  - CaTch as configured in NextGen;
  - an LSTM/transformer trained on the exported trace tier;
  - a PAMPOS-style self-supervised predictor;
  - "none" and "oracle" arms.
- **MA:** `LegacyWindow` (k = 3, 15 s window) and `CorroboratedMa` (`crates/v2xw-threat/src/ma.rs`, `ma_corroborate.rs`).
- **Cross-dataset:** run every detector on NextGen's test set as well, to show that its F1 order on our trace tier matches its order on NextGen. That establishes comparability.

**Validation anchors**
- the SUMO cross-validation of traffic;
- the Veins/OpenCV2X-matched PHY;
- official 1609.2 test vectors and the real SCMS Manager trust lists (PROTOCOL.md);
- CAM/pseudonym field behaviour from bauder2023cooperative;
- 1.5 s TTC threshold (safety.rs header);
- warning-response parameters taken from driving-simulator literature (critique-methods §2.4, e.g. yan2014driving) and **swept, not fixed**.

**Figures and tables**
1. Pipeline schematic: tiers, tasks and export formats.
2. A bump chart of F1 rank versus NSS rank per detector, with a τ CI band (the headline).
3. A decomposition of NSS per detector into prevented harm, FP-harm and wrongful-revocation harm.
4. A heatmap of attack class × detector, showing where F1 and NSS disagree.
5. The MPR curve: NSS gap to oracle versus local recall, with one line per FP rate, under MA "do nothing" versus revoke.
6. A table comparing SCMS-Bench with VeReMi, Extension, NextGen, DARE, MisbehaviorX and zhang2024enhancing on 10 features, plus a table of the placebo/coupling audit.

**Hypotheses**
- **H1, decision-changing:** τ(F1, NSS) < 0.5. If true, F1-based MPR or leaderboard choices pick the wrong detector.
- **H2:** disagreement concentrates in acceleration/braking and Sybil attacks, where NextGen F1 is near zero (Table II) but safety impact is highest.
- **H3:** FP-harm reorders the top detectors only when warning compliance is above 50%.
- **H4:** "do nothing" is within ε of oracle only above recall 0.8 at FP 1e-3.
- **H0-guard:** a placebo detector, which never flags but runs the same code path, must yield NSS ≈ 0. It is shown to go red under the current keying (critique §2.3).

## 5. Engine prerequisites mapped to code

| Need | Exists | To build | Order |
|---|---|---|---|
| Open-loop replay of N detectors over one recording | `crates/v2xw-record/src/reader.rs:373` `replay()`; detectors are behind the `Detector` trait (`detect.rs:351`) | a CLI `bench replay` that loops over detectors and writes confusion matrices | 1 (unblocks Tier A and the first result) |
| VeReMi export | `crates/v2xw-record/src/dataset/veremi.rs`, plus the leakage linter `dataset/leakage.rs` | a **NextGen exporter** (Listing 1 schema) and a NextGen *importer* to run their test set | 2 |
| Python and ML detectors in the loop | `crates/v2xw-py/src/plugins.rs` `PyDetector`; its header says the trait is declared locally because "v2xw-threat is still a stub" | rebind `PyDetector` to `v2xw_threat::Detector` and add batched windows | 3 |
| Warnings that change driving | none: apps only label warnings (critique §2.4); couplings exist only for GLOSA and CACC (`crates/v2xw-engine/src/run/coop.rs`) | a warning-response driver model with compliance, reaction time and a braking profile as sweepable cards | 4 (blocks Tier B's FP-harm leg) |
| Stable and event RNG keys | entity-keyed `crates/v2xw-core/src/rng.rs`; ids in realised order (`v2xw-mobility/src/engine.rs:3147`) | `trip.seq` keys, event keys, a coupling audit and a placebo test (critique §3.1–3.4) | 4, in parallel |
| MA plug-ins and the PKI loop | `ma.rs` `MaPipeline`, `LegacyWindow`, `CorroboratedMa`; the SCMS backend | a do-nothing MA and a synthetic tunable detector for RQ4 | 5 |
| Missing NextGen attacks | 34 attack kinds (`attack.rs`, `attack_ext.rs`, `attack_rsu.rs`) | six kinematic attacks (§4) | 5 |
| Experiment runner | `crates/v2xw-experiment` (plan, runner, aggregate, figures) | a frozen bench manifest and a scorer | 6 |

## 6. Risks and mitigations

1. **NEW, biggest: Ulm closes the loop first.**
   - *Risk:* NextGen explicitly plans closed-loop MOSAIC evaluation, and the group is funded (CONNECT, ConnRAD). A NextGen-CL paper at VNC 2027 or VTC 2027 would take the "closed-loop MBD benchmark" framing.
   - *Mitigation:*
     - make the SCMS loop (MA, revocation, wrongful revocation) the identity, not "closed loop" alone;
     - post the rank-flip result on arXiv early (around month 3);
     - offer NextGen-format interoperability and co-hosting.
2. **Prior art on impact metrics** (wehmer2021practical, zhang2024enhancing). *Mitigation:* cite them as motivation. The novelty is the multi-detector ranking study, the SCMS loop and the release.
3. **The safety leg rests on an invented driver response.** *Mitigation:* sweep compliance and reaction time and report a ranking robustness label. Also report CACC/GLOSA-only results, which need no new driver model.
4. **Compute.** *Mitigation:*
   - Tier A is exact and cheap;
   - Tier B runs on small fleets;
   - only Tier C needs cloud (~72 h; budget it now);
   - a surrogate tier comes later, not in paper 1.
5. **Adoption.** It needs:
   - zero-install traces on Hugging Face plus Zenodo DOIs (the E&D track prefers HF, Kaggle, OpenML or Dataverse, with Croissant metadata);
   - a Docker closed-loop image;
   - a scorer CLI;
   - baselines;
   - a leaderboard hosted as an HF Space with submitted detectors re-scored by CI;
   - CC BY 4.0 data (matching VeReMi) and Apache-2.0 code.

   OPV2V paired 16 implemented models with its dataset (xu2022opv2v abstract). CARLA scores submissions remotely with a monthly quota (leaderboard.carla.org). F2MD succeeded as "free and open-source" (kamel2020simulation).

**Fallback if H1 is null** (τ high): the paper becomes "F1 *is* a valid proxy for closed-loop safety, except for attack classes X" (a negative result the E&D track explicitly values as evaluation science), plus the MPR curve (RQ4) and the SCMS tasks. The benchmark release stands either way.

## 7. Six-month plan to first submission

| Month | Work |
|---|---|
| 1 (Nov 2026) | `bench replay`; NextGen exporter and importer; Tier A; a first F1 table; check the `sender_id` leak in NextGen |
| 2 (Dec) | Rebind `PyDetector`; train ML and PAMPOS-style baselines on the trace tier; cross-check against NextGen |
| 3 (Jan 2027) | Warning-response model plus stable/event RNG keys and the placebo test. Pilot Tier B to set seed counts. Post the Tier-A arXiv note |
| 4 (Feb) | Tier B and Tier E locally; start Tier C on cloud |
| 5 (Mar) | Morris robustness (Tier D); leaderboard Space; Croissant metadata; Docker image |
| 6 (Apr) | Write; freeze v1.0; submit |

**Primary venue:** NeurIPS 2027 Evaluations & Datasets track (the 2026 renaming of Datasets & Benchmarks). The 2026 deadlines were abstract 4 May and paper 6 May 2026, with code and data required at submission (https://neurips.cc/Conferences/2026/CallForEvaluationsDatasets). The **2027 date is inferred** at early May 2027.

**Backup:** IEEE VNC 2027, the home venue of VeReMi NextGen. No 2027 call was found; VNC 2026 met on 8–10 June 2026 in Montreal, so a deadline around February 2027 is **inferred**. A journal version can follow in IEEE TDSC.

## Searches run

- **arXiv** (export.arxiv.org, sorted by date, 3 s spacing): "misbehavior detection", VeReMi, "misbehaviour detection", misbehavior+V2X+dataset. Newest relevant: 2609.31217, 2609.02741, 2605.06833, 2512.15503, 2505.22162.
- **Web:** VeReMi++ (no source paper found); closed-loop MBD safety benchmarks 2025–26 (none found); Wehmer 2021; the Ulm 2026 work; VNC 2027 call (not published); NeurIPS 2026 E&D call; CARLA Leaderboard.
- **OpenAlex:** returned no abstract for Wehmer 2021.
- **Not verified:** the Bassi et al. VTC2026-Spring paper content; the origin of VeReMi++; whether NextGen's `sender_id` is the real id.
