# Paper plan, October 2026

Written 2026-10-23 for the owner. Lead-researcher synthesis of the three checks run today:

- `check-datasets-2026.md` (check 1: competing datasets and benchmarks, 2025–26);
- `check-paper1-preemption.md` (check 2: pre-emption risk for Paper 1);
- `check-realdata-and-vesnos.md` (check 3: public CV-pilot BSMs, VESNOS full text);

plus `research/explore/final.md` (S1/S2, 2026-10-18) and `vesnos-full-read.md`. Those files hold the citations. This file holds no new evidence. It decides what to write, what to claim and in what order.

Deadlines are marked **confirmed** only where a call is published. Everything else is *inferred* from the previous cycle. A web search on 2026-10-23 found no VehicleSec '27 call and no NeurIPS 2027 call.

---

## 0. Summary

| # | Paper | Status | Target | Deadline |
|---|---|---|---|---|
| 1 | **Honest but Wrong**: benign triggers of the published US misbehaviour-reporting rules, from pilot BSMs to revocation | Recommended. Start now | USENIX VehicleSec '27 | *inferred* late Feb 2027 (not announced) |
| 2 | **SCMS-Bench**: a closed-loop V2X misbehaviour benchmark with SCMS-layer and counterfactual impact labels | Recommended. Engine-heavy, second | NeurIPS 2027 Evaluations & Datasets | *inferred* May 2027 (not announced) |
| 3 | **Sound and Live** (S2): MA evidence policies under Sybil reporters, sparse witnesses and two providers | Deferred. Thesis anchor; reuses Paper 1's MA model | VehicleSec '28 or IEEE TDSC | *inferred* Feb 2028 / rolling |

Three things changed since `final.md`:

1. **Paper 1 is now real-data-first.** Honest pilot devices trip the 1 m RandomPosition check at 0%, 4.5% or 44% of fast pairs depending on device class (check 3). Before, the engine's "26 honest vehicles revoked" was the lead. It came from a GNSS-model artefact and is retired.
2. **Two Paper 2 selling points are gone**: "same-seed" pairing and "artefact-free" design (check 1).
3. **The biggest gap between claim and code is Paper 2's**: warnings do not yet change how anyone drives, so its "impact" labels are still open-loop.

The PQ-migration direction (`pqc-direction-final.md`) is not on this plan's critical path. It is parked unless the owner wants it back (decision O6).

---

## 1. Paper 1: Honest but Wrong

**Working title.** *Honest but Wrong: How Often the Published US Misbehaviour-Reporting Rules Fire on Honest Vehicles, from CV-Pilot BSMs to Revocation*

**One-sentence contribution.** The first measurement of how often the only published US rule set (SCMS Manager CV-Pilot MBR v1.0 and its ASN.1 v1.1) fires on honest vehicles. It is measured on public CV-pilot BSMs by device class and time base, and then carried through a city-scale report → RA → MA → LA → CRL loop to a wrongful-revocation surface over a family of MA rules.

### 1.1 The exact safe novelty claim

Use this wording (check 2 §4, tightened by check 3):

> We give the first measurement of the benign trigger rate of the only published US misbehaviour-reporting rule set, SCMS Manager's CV-Pilot MBR v1.0 and ASN.1 v1.1. (i) On public CV-pilot BSMs, honest devices whose BSM timestamp is not locked to the GNSS fix epoch trip RandomPosition at 20–195 triggers per ID-minute at speed, while epoch-locked devices trip it at about 0. The rate depends on which clock the receiver uses for Δt. (ii) In a city-scale closed loop with standards-exact formats, we show how those triggers fill the on-device store, which of them reach the MA, and how many honest devices a family of MA decision rules would revoke. We report a surface because no US MA rule is published and the pilots used human review. (iii) We show what the MA can recover from fields it already receives, and what an attacker gains by imitating the benign device class.

Mandatory qualifiers:

- **"published" or "CV-Pilot", never "deployed".** SM-MBR is a 2019 pilot specification. The pilot MA was a human operator (FHWA-JPO-19-775 p.8).
- **"would revoke under MA rule R, given trigger rate T", never "revokes honest vehicles".**
- **"first for the US rule set"**, never "first real-data benign rate" (Bißmeyer 2014, simTD FOT, ≈9.25% message-level FPR).
- **"first for the US SCMS chain"**, never "first city-scale false positives at the MA" (Kamel 2020, Fig. 3.12, F2MD on LuST).
- **Report both normative readings of RandomPosition.** v1.0 uses velocity only. The ASN.1 v1.1 uses velocity and acceleration with a 0.1 s spacing. The engine's 0.5 s gap limit appears in neither, so treat it as a swept parameter.
- **The policy hook** is 5GAA proposed CP 2025, (259) p.57: an EE "not working correctly" is blocked. This makes benign triggers a policy problem, not a hypothetical one.

### 1.2 Ruled out (do not claim)

| Claim | Why not | Source |
|---|---|---|
| "The deployed US system revokes honest vehicles" | Pilot MA was human; no production MA rule is public | check 2 §2.2, §2.3 |
| The engine's 26 honest revocations | GNSS white-noise artefact; 0 with a realistic receiver | `usfp.md` §3 |
| "Doppler smoothing fixes it" | The engine's raw case has the wrong error shape (isotropic vs along-track epoch slips); its smoothed case matches only the locked class | check 3 §3.2 |
| "Radio gaps in turns and braking drive the triggers" (the `final.md` hypothesis) | At 100 ms the trapezoid residual equals the SM-MBR residual; turning and braking explain nothing | check 3 §2.3 |
| "The only new field the MA needs is a cohort token" | Device class is visible from the secMark phase in the evidence BSMs the MA already receives | check 3 §3.4 |
| "First real-data FP rate of plausibility checks"; "transmission effects dominate" (generic) | Bißmeyer 2014 | check 2 §2.7 |
| "First genuine/faulty/malicious classification at the MA" | Kamel 2020 §6.1; Rathore 2023; Das 2026 | check 2 §3 |
| "First slander defence at a global MBD with an SCMS" | Park 2024 (K-City) | check 2 §2.5 |
| "First simulator with misbehaviour reporting in the loop" | VESNOS's paper text claims it (excerpts, unverified), although its code lacks it | check 3 §5 |
| A novel *mechanism* (secMark/fix-epoch misalignment) | Not novelty-checked; one search found nothing, which is not a check | check 3 §2.6 |

### 1.3 Experiments

Ordered so that each step can fail before the next one costs anything.

| Id | Experiment | Engine? | What would make it fail (and so must be tested) |
|---|---|---|---|
| P1-E1 | **Real-data baseline.** All SM-MBR checks (RandomPosition in both readings, ConstantPosition, MaxSpeed, MaxAcceleration, BrakeAcceleration) on Tampa, Orem (all 7 days, 4.1 M rows), NYC (reported separately: warning-selected) and Wyoming. Factors: time base (secMark vs header/generationTime), gap limit (0.1, 0.2, 0.5 s), speed band. Rates per ID-minute with ID-clustered bootstrap CIs. | no | Inject a 2 m jump and a synthetic epoch slip into a locked Wyoming trace; the pipeline must report them. Shuffle-placebo: randomising secMark phase must destroy the class effect |
| P1-E2 | **Device-class test.** secMark phase classifier (locked >80% within 1 ms of a 50 ms epoch, free <30%), fitted on Tampa and Orem, then **frozen and scored on held-out Wyoming** and on any newly obtained data | no | If the classifier does not predict trigger rate out of sample, the headline is "some devices", not "a device class" |
| P1-E3 | **Time base.** Decide whether Wyoming's `metadata_generatedat` is the signed 1609.2 generationTime, using raw `.bin` frames from the data request | no | If it is ODE log time, the 0 → 37% result must be dropped |
| P1-E4 | **Engine recalibration.** Per-device timing model fitted to E1/E2 (see §1.4). Nulls: a locked device with a perfect receiver must give zero reports. Injected epoch slips must fire. The held-out Wyoming residual distribution must match within a stated tolerance (KS distance set before the run) | yes | The tolerance row must be shown to go red on the old white-noise model (memory: checks that cannot fail) |
| P1-E5 | **Receiver-side gaps at density.** Which receiver decoded which BSM under channel load, and how gaps change the pairs judged (city map, 3 loads, 3 RATs, small fleets) | yes | Compare the gap distribution with Tampa's RSU-received gaps (70% at 100 ms, 95% ≤ 0.5 s) |
| P1-E6 | **Store and upload crowd-out.** SM-MBR §2.2 weights, ×0.9 daily decay, quota and RSU-gated upload, with injected attackers carrying ground truth; offline replay over quota, gpsDrift, free-class share, RSU density, cellular share | yes | An attack-free run must give zero attack evidence lost; a zero-quota run must lose all of it |
| P1-E7 | **MA rule family → revocation surface.** Per-pseudonym k-threshold, LA-linked per-device, CaTch-style accuracy-aware, cross-vehicle clustering on fields already received (secMark phase, PositionalAccuracy, issuer CA). Report honest devices blocked per 10⁶ device-hours against attacker time to act | yes (replay) | With the free class set to 0% share, every sound rule must revoke zero honest devices |
| P1-E8 | **Evasion.** An attacker that fakes the free-class secMark phase or claims poor accuracy to hide inside the benign class | yes | Report the attack budget the disguise buys; if it buys nothing, say so |
| P1-E9 | **Side finding.** The US profile defines reports for PSID 0x20 (BSM) only, so a PSM-sending handset attacker goes unreported (`usfp.md` §3) | done | Keep it to one paragraph |
| P1-E10 | **Multi-day projection.** An open-loop surrogate fed by per-class, per-corridor rates, labelled analytical | yes | Must reproduce a full engine run's first day within tolerance |

Sizing: about 10–15 full-scale runs plus small-fleet runs and offline replay (from `final.md`), once the full-scale benchmark runs end to end (§1.4, item A).

### 1.4 Engine work for Paper 1

| Item | What | Size (estimate) | Blocks |
|---|---|---|---|
| A | **Finish the full-scale benchmark** (the current engine wave): fast-forward speed (6.7 sim-s per s at full scale), reports held through fast-forward and arriving 12 h late, the 4,876-report honest burst after fast-forward (71 cases against honest cars), memory peak 3.1 GB against the 2.5 GB cap, and the two failing engine tests | in flight | E5–E8, E10 |
| B | **Per-device BSM timing model**: P(position epoch ≠ timestamp epoch), the duplicate/skip pattern (frozen positions while moving, as in Orem), header-clock jitter. Assigned per device class at a swept share. It replaces the "raw GNSS vs smoothed" factor as the honest-trigger knob | 200–400 LOC | E4 onward |
| C | **RandomPosition variants**: v1.0 velocity-only and ASN.1 v1.1 velocity+acceleration with 0.1 s spacing; configurable time base (secMark vs generationTime); gap limit as a parameter. `crates/v2xw-threat/src/us_mbr.rs` today implements v1.0 with generationTime and 0.5 s | 100–200 LOC | E4 onward |
| D | **SM-MBR §2.2 store policy**: verify weights, ×0.9 decay at 05:00 Eastern, device quota and upload order against the spec. A quick grep of `us_mbr.rs` found none of these, so they may live elsewhere or be missing | 0–300 LOC | E6 |
| E | **MA rule family and offline replay binary**, shared with Paper 3 | 400–800 LOC | E7, E8 |
| F | **MA-side features**: secMark phase, PositionalAccuracy and issuer clustering; the evasion attacker | 200–400 LOC | E7, E8 |
| G | **Multi-day surrogate**, shared with Paper 3 | ~300 LOC | E10 |

Status 2026-10-24 (merged onto `main`, not pushed):
- **B, delivered:** `crates/v2xw-node/src/bsm_timing.rs`, per-device timing classes read from `data/calibration/bsm-timing-cvpilot.yaml` (v2xw.bsm_timing.v1), off by default (`actors.vehicles.bsm_timing`). Built-in values are provisional until they are refit against the data package's file; the off and `enabled: false` runs reproduce the pinned digest.
- **C, delivered:** `us_mbr.rs` reading `v1_0 | asn1_v1_1`, time base `generation_time | secmark`, `max_gap_s`. Defaults keep the old behaviour. Free class at 20 m/s: 24.1 triggers per ID-minute (locked 0.17), inside the real-data 20-195 band.
- **D, delivered:** the store was checked against the SM-MBR v1.0 text and four defects were fixed; 8 knobs added, plus `security.scms.report_records`. Still open: no test of the Lost-to-retry path, and `NotSent` relays still drop the report.
- **E, delivered:** `v2xw ma-replay` runs 37 settings in 4 families. A 300 s run is too short for the 05:00 decay or the cluster rule.
- **Real-data baseline (feeds E1/E2), delivered:** `tools/realdata` v1.0.0 and `research/synthesis/p1-realdata-baseline.md`. Wyoming (held out) passes the frozen locked bound; the free class has no out-of-sample test yet, and P1-E3 needs the raw Wyoming frames.
- A: not part of this wave (see the SCMS benchmark sections of `docs/design/BUILD-STATUS.md`). F, G: not started.

Status 2026-10-27 (merged onto `main`, not pushed; results in `research/synthesis/p1-experiments.md`):
- **A, in progress:** FB4 rows written before the run, and `tools/realworld/fullbench.py` judges them. FB4 (full fleet) did not finish: about 61 wall-s per simulated second under load, stopped at 96 s. It needs one unbroken run of about 5 h. QD4 (quarter fleet) passes 6 of 7 rows. QD4.cost is open: peak memory is +12.5 % over QD25, with no cause found yet.
- **B, fix:** `ffaebbf9`. Before it, every stop and start jumped the reported position by the error walk. Only runs with timing on are affected. Paper 1 rows that ran with timing on (E4 etc.) may move when re-run.
- **F, delivered:** `ma.received` secMark phase now comes from the judged BSM. New evasion attacker `BenignClassMimic`. P1-E8: the disguise buys 0 s under all four rule families. Open: budget start should be `first_falsified`, and L1B was not run.
- **G, delivered (analytical):** `tools/surrogate` v1.0.0. P1-E10 passes 15 of 15 rows on a quarter-fleet day. The rollover window sits at the band edge. The rates must be re-measured on FB4.
- **P1-E5 fails** its KS test (0.129 vs 0.0747): the engine's RSU gaps are too clean. **P1-E6 passes** both nulls. **P1-E7:** after the B fix, only the k=2 rules block an honest device (1 per seed).

Total: about 3–5 weeks of engine work after item A, in line with `final.md`'s estimate. Items B and C are the critical path: until they exist, no engine number in Paper 1 means anything.

### 1.5 Venue and deadline

- **Primary: USENIX VehicleSec '27.** *Inferred* late Feb 2027 (VehicleSec '26 was 24 Feb 2026, co-located with USENIX Security; USENIX Security '27 is 11–13 Aug 2027 in Denver, **confirmed**). The VehicleSec '27 call was not found on 2026-10-23.
- **Not recommended: USENIX Security '27 cycle 2**, 26 Jan 2027 (**confirmed**). It is too early for E4–E8 and the venue is harder for a measurement paper.
- **Fallbacks:** IEEE VNC 2027 (*inferred* about March), IEEE T-ITS (rolling).
- **Pre-print option (O2):** post an arXiv note with the real-data result (E1–E3) in mid-December 2026. It needs no engine work and establishes priority. The risk is that it invites a scoop of the engine half, so it is the owner's call.

### 1.6 Risks, honestly

1. **The mechanism may be a pilot-era conformance defect.** If SAE J2945/1 requires secMark to equal the position time (unverified, paywalled), the free class are non-conformant devices. The paper still stands ("the rule cannot tell a non-conformant clock from a position attacker, and the CP blocks devices not working correctly"), but a reviewer can say production devices are fixed. Mitigation: read J2945/1. Frame the result as a conformance hazard and test production-era data (Orem is 2025) separately.
2. **Small samples.** Tampa is one night, Orem 30 minutes so far, Wyoming 16 IDs, NYC 12 events. Mitigation: all of Orem now, plus the full Tampa and NYC raw data by request (no stated turnaround). If the request does not arrive by mid-January, the paper rests on public samples only.
3. **Tampa is RSU-received**, so reception gaps mix with sender behaviour. Units are temp-ID periods, not vehicles. Say so.
4. **Unread texts that could redefine "the US rule set":** Park 2024 IET ITS (open access, bot-walled; read in a browser), SAE J3287 (paywalled), SAE technical papers and CAMP reports (not searched). None can be a city-scale benign-rate study, but any of them could carry thresholds.
5. **"Why simulate?"** The real-data half may read as the whole paper. The engine half must deliver what logs cannot (E5–E8). If it slips, cut to a real-data short paper with an analytical crowd-out model rather than miss the deadline.
6. **Scoop.** Das, Griffith and Rathore already work on Tampa BSMs (CCNC 2026) and could run the standard checks on clean data. This is the argument for the December arXiv note.
7. **Engine artefacts at full scale.** The post-fast-forward burst (71 honest cases) is exactly the kind of artefact that produced the retired "26". No full-scale MA number is reported until item A is fixed and a check that the burst is gone has been shown to go red on the current build.
8. **The deadline is not announced.** If VehicleSec '27 is not co-located or moves, fall back to VNC or T-ITS.

---

## 2. Paper 2: SCMS-Bench

**Working title.** *SCMS-Bench: A Closed-Loop V2X Misbehaviour Benchmark with SCMS-Layer and Counterfactual Impact Labels*

**One-sentence contribution.** The first V2X misbehaviour benchmark whose labels reach the PKI layer (misbehaviour reports, MA decisions, CRL and linkage revocation, per-message certificate state), with per-unit counterfactual impact labels produced by receivers that act on the data. It shows that message-level F1 ranks detectors differently from the outcome of the full detector → MBR → MA → CRL loop.

### 2.1 The exact safe novelty claim

> SCMS-Bench is the first V2X misbehaviour benchmark with SCMS-layer labels (misbehaviour reports, MA decisions, CRL/linkage revocation and per-message certificate state) and per-unit counterfactual impact labels whose divergence comes from receivers reacting in closed loop. It also carries radio-reception labels for the same messages, all in encoded J2735 and IEEE 1609.2. Its releases pass the published artefact audit (Muzammil's S0–S6 cascade, single-scalar baselines, grouped-split leakage protocol), which ships as a CI check shown to go red on injected artefacts.

Scope every exactness statement to the regimes in `critique-methods.md`: open-loop replay, backend-only changes, and the near field of a closed-loop discard.

The headline result wanted is unchanged from `research-plan.md` §2: F1 rank vs Net Safety Score rank across ≥10 detectors (Kendall τ), plus the MA "do nothing" recall curve. If τ is high, the fallback is "F1 is a valid proxy except for classes X".

### 2.2 Ruled out (do not claim)

| Claim | Why not | Source |
|---|---|---|
| "First same-seed / paired counterfactual runs" | SPADE runs each seeded scenario once per class with conditions held constant (§IV-B); MisbehaviorX has benign/attack runs; bello2026 has matched baselines | check 1 §0.2 |
| "Artefact-free" | Unfalsifiable as worded; Muzammil's released audit accepts any detector, so anyone can run it on us | check 1 §0.4 |
| "First false-positive / wrongful-revocation labels at an MA" (unqualified) | Kamel 2020 counts genuine-vehicle reports at an MA; the label is new only because it is tied to the SCMS chain | check 2 §5 |
| "First simulator integrating an SCMS"; "first misbehaviour reporting in a co-simulator" | VESNOS's title and (unverified) paper text claim both. Safe: "first open simulator in which the SCMS backend itself (issuance, linkage, reporting, MA decision, linkage-seed revocation, trust lists) runs inside a traffic and radio loop", cited with VESNOS commit `997913b5` | `vesnos-full-read.md` §0, §7.1 |
| Closed-loop impact labels, **today** | Warnings (FCW, EEBL, IMA, …) change nobody's driving yet (`critique-methods.md`), so current labels are as open-loop as MisbehaviorX's warning flags | check 1 §3 item 13 |
| Guven 2025 as an available comparator; PQ-V2X as a peer benchmark | Guven is withdrawn ("Do not use or cite"); PQ-V2X rows are i.i.d. label-conditioned draws | check 1 §1.4, §1.9 |
| "Real-data labels" | Real streams give calibration priors, not labels | check 3 §4 |

### 2.3 Experiments

From `research-plan.md` §2 (tiers A–E), with these changes:

- **Tier B (closed loop) cannot start until the warning-response driver model exists.** Compliance, reaction time and braking are swept and reported as a robustness label.
- **Add an audit tier.** Run Muzammil's S0–S6 cascade, single-scalar baselines (message count, median inter-arrival) and grouped vs random splits on our export. Publish the numbers whatever they are.
- **Add a real-data calibration split.** Fit the honest-device timing model on Tampa and Orem, hold Wyoming out, and report the honest-trigger rate per device class (shared with Paper 1, P1-E2/E4). This answers "your benign class is synthetic noise".
- **Add a real-data anchor.** Replay our detectors on BSMAttacker's real corpora (SPMD Ann Arbor, AMCD Tysons, V2AIX). BSMAttacker v1.0.0 results are "under revision", so pin the version.
- **Prevalence sweep down to ≤1% of vehicles**, with average precision, not only AUC.
- **Label disagreement.** Ship a manipulation label and a measured-impact label, and report how often they disagree.

### 2.4 Engine work for Paper 2

Mandatory before submission (check 1 §4 and `research-plan.md` §5):

| Item | Why | Where | Size |
|---|---|---|---|
| Warning-response driver model, sweepable | The biggest gap: without it there are no closed-loop impact labels | `crates/v2xw-engine/src/run/coop.rs` | ~4 weeks (largest refactor) |
| messageID adjacency check | `messageID` is the global SDU arena index, so one Sybil's ghosts may get consecutive IDs. Unverified linkage shortcut | `run.rs` (`desc.sdu.index()`), `veremi.rs` | small; the check must go red on a deliberately leaky export |
| Pseudonym rotation on in every released scenario | Benign identities that never rotate are a known artefact | `pseudonym_policy.rs`, scenarios | config + check |
| Value-level leakage test | `leakage.rs` checks key names and true-identity values only; it misses label-derived features and structural encodings | `leakage.rs` | mutual information on placebo runs ≈ 0 |
| NextGen exporter/importer; `PyDetector` rebind; six NextGen attacks | Comparability with the field's current benchmark | `v2xw-record/src/dataset/veremi.rs`, `v2xw-py`, `v2xw-threat` | 2–3 weeks |
| Ground-truth log of every certificate digest with validity intervals | VeReMi Ext omits up to 27.6× of identities | recorder | small |
| Attacker transmit scheduling through the same MAC and jitter as benign traffic | Avoids the phase-offset linkage shortcut | node runtime | check: phase-residual linkage AUC ≈ 0.5 |
| Bench manifest, scorer, open hosting (Zenodo, hashes; CC BY 4.0 data, Apache-2.0 code) | Adoption; avoids subscription walls | `v2xw-experiment` | 1–2 weeks |

### 2.5 Venue and deadline

- **Primary: NeurIPS 2027 Evaluations & Datasets.** *Inferred* May 2027; the 2026 cycle followed the main track. Code and data are required at submission. One unofficial post gives a later 2026 date, so recheck when the 2027 call appears.
- **Backups:** VehicleSec '28 (*inferred* Feb 2028), IEEE TDSC (rolling). IEEE VNC 2027 (about March) is too early for Tier B.

### 2.6 Risks, honestly

1. **The driver model is invented.** Mitigation: sweep it, report a robustness label, and cite yan2014driving. A reviewer can still reject the impact labels as assumption-driven.
2. **The audit can fail us.** This is now the main risk (check 1 §0): someone runs Muzammil's cascade and finds a shortcut, for example the messageID adjacency. Mitigation: run it ourselves first and publish the result.
3. **Ulm closes the loop first.** VeReMi NextGen announced closed-loop evaluation and is funded. Mitigation: make the SCMS layer and wrongful revocation the paper's identity, not "closed loop".
4. **Engineering burden and compute.** Tier B is 1,440 runs (120–400 h) on an 8 GB machine where the full-scale benchmark already exceeds its memory cap. Tier B must use small maps; Tier C needs cloud hours (decision O4).
5. **An ML venue.** Reviewers will judge the evaluation science (F1 vs NSS), not the SCMS fidelity. If τ is high, the paper rests on the fallback.
6. **Watch list:** BSMAttacker v2 (real data plus safety replay), the V2X-PAD companion paper, Jerbi/Artery CPM datasets.

---

## 3. Paper 3 (deferred): Sound and Live

As in `final.md` S2: which MA evidence policies are both sound (one device cannot frame an honest vehicle) and live (a persistent attacker is acted on within the cycle), under 20 concurrent pseudonyms, sparse witnesses and two providers. It reuses Paper 1's MA rule family (item E) and surrogate (item G), so its cost falls after Paper 1. Its review status is "merged design unreviewed". Its main risk is an obvious headline ("de-duplicate reporters"). It needs a non-obvious boundary to be worth writing. Target VehicleSec '28 or TDSC. No work on it before Paper 1 is submitted.

---

## 4. Real-data pipeline

Shared by Paper 1 (headline) and Paper 2 (calibration split). It starts from the scripts in `research/synthesis/check-realdata/` (pure Python, standard library). They graduate into a versioned tool with tests, so the paper's numbers are rebuilt by one command.

### 4.1 Sources

| Dataset | Id | Use | Licence | Caveat |
|---|---|---|---|---|
| Tampa CV Pilot BSM Sample | `nm7w-nvbm` (34,464 rows) | fit (locked and free classes) | CC BY-SA 4.0 | One night; RSU-received |
| UDOT/Panasonic Orem | `ev64-jn8c` (4.1 M rows, 7 days, 2025) | fit; production-era devices | none stated: ask ITS JPO before redistributing rows | Frozen positions in 13% of moving pairs |
| Wyoming CV Pilot BSM One Day | `7qu7-j8rx` (6,597 rows) | **held out**; time-base test | CC BY-SA 4.0 | Sanitised, I-80 only, 25% of gaps > 0.5 s |
| NYC CV Pilot EVENT | `vxmm-9bi5` (10,533 events) | reported separately | NYC terms + CC BY 4.0 | Selected around warnings; obfuscated positions |
| Full raw Tampa and NYC, raw `.bin` frames | its.dot.gov data-request form | scale-up; settles the time base | per request | No stated turnaround |
| BSMAttacker corpora (SPMD, AMCD, V2AIX) | Zenodo 23089747 | Paper 2 real-data anchor | per corpus | Check down-sampling and filtering first |
| Utah Pooled Fund | `e9cn-jdnu` | **excluded** | none stated | Speed 0 while moving, duplicates |

The public S3 sandbox returned AccessDenied on 2026-10-23. Do not plan on it.

### 4.2 Stages

1. **Fetch.** Pinned SODA queries (dataset id, `$where`, `$order`, row count checked against `count(*)`), stored outside the repository (share-alike applies to raw rows), with a sha256 manifest committed.
2. **Normalise** to one schema: temp ID, secMark, msgCnt, wall time, header time, ENU position, speed, heading, acceleration, accuracy ellipse. Units and heading convention follow J2735, with a unit test per source.
3. **De-duplicate and pair** on (ID, secMark, msgCnt). Δt from each time base. Gap limit as a parameter.
4. **Checks.** All SM-MBR checks in both readings; residual decomposition along-track and cross-track; epoch-slip signature (≈0 or ≈2·v·Δt).
5. **Device class** from secMark phase, with thresholds frozen before Wyoming is scored.
6. **Falsification gate** that runs before any table is produced: injected jumps and slips must be found, the shuffle placebo must erase the class effect, and the gate must fail on a deliberately broken check.
7. **Outputs.** Derived statistics only (rates, CIs, distributions) plus the fitted timing-model parameters as an engine input file with provenance.

### 4.3 Data and texts to request now

- File the its.dot.gov data request for full Tampa and NYC raw data and Wyoming raw frames (O1).
- Buy SAE J3287 and J2945/1, or get them through an SAE member.
- Read Park 2024 IET ITS in a browser (open access).
- Get VESNOS through interlibrary loan or from the authors.
- Ask ITS JPO about redistribution terms for the Utah sets.

---

## 5. Timeline

Starts 2026-10-26. One heavy command at a time on this machine (memory: lean agent builds), so engine and real-data work run in parallel only because the real-data work needs no builds.

| Weeks | Engine | Real data and reading | Writing and submission |
|---|---|---|---|
| 26 Oct – 6 Nov | Item A: full-scale benchmark fixes, two failing tests | Data request filed; J3287, J2945/1, Park IET, VESNOS requested; pipeline stages 1–3 on all of Orem | — |
| 9 – 20 Nov | Items B, C (timing model, RandomPosition variants) | P1-E1, E2 with the falsification gate; mechanism novelty search | Paper 1 outline |
| 23 Nov – 4 Dec | P1-E4 recalibration, Wyoming held-out test; item D | P1-E3 if raw frames arrived | — |
| 7 – 18 Dec | Items E, F | — | **arXiv real-data note (O2)** |
| Jan 2027 | P1-E5–E8 full-scale runs and replay; item G and E10 | Fold in requested raw data if it arrived | Paper 1 draft; decide the fallback by 15 Jan (§1.6 risk 5) |
| Feb | — | — | **Paper 1 to VehicleSec '27** (*inferred* late Feb) |
| Mar – Apr | Warning-response driver model; NextGen exporter; leakage, messageID and rotation checks; scorer | BSMAttacker replay; calibration split | Bench Tier A note |
| Apr – May | Tier B on small maps; Tier C on cloud; audit tier | — | **Paper 2 to NeurIPS 2027 E&D** (*inferred* May) |
| Jun onward | Paper 3 extensions (witness metric, Sybil reporter, cooperation modes) | — | Paper 1 revision or resubmission |

**Slack:** none in Jan–Feb for Paper 1, and little for Paper 2. If item A takes longer than two weeks, Paper 1 goes to VehicleSec as a real-data paper with an analytical crowd-out model, and the engine half moves to a T-ITS extension.

---

## 6. Decisions for the owner

- **O1.** File the its.dot.gov data request and buy J3287 and J2945/1 (the owner's identity, the owner's money).
- **O2.** Post the real-data result on arXiv in December (priority), or hold it until the full paper (avoids tipping off a faster group).
- **O3.** Approve the VehicleSec '27 slot for Paper 1, which moves Paper 3 to 2028.
- **O4.** Approve cloud hours for Paper 2 Tier C and part of Tier B, or accept smaller grids.
- **O5.** Accept a swept-parameter driver model for Paper 2, or seek driving-simulator data.
- **O6.** Keep the PQ-migration direction parked, or schedule it after May 2027.
- **O7.** Contact the Park authors (K-City numbers), Ulm (NextGen exporter, co-hosting) and the VESNOS authors (full text).
