# P1 real-data baseline (P1-E1, P1-E2)

Written 2026-10-24. Experiments P1-E1 (real-data baseline) and P1-E2 (device-class test) of `paper-plan-2026-10.md` §1.3, run with the pipeline of §4.2 (`tools/realdata`, version 1.0.0). No engine code is involved.
Every number below comes from the generated tables at the end of this file. One command rebuilds them (§6).

---

## 0. Verdict

- **The falsification gate passed before any table was written (T0).**
  - A 2 m step injected into a locked Wyoming trace was found at exactly the injected pair.
  - A synthetic epoch slip fired both neighbouring pairs, with the "≈ 2 v Δt then ≈ 0" signature, and the slip estimator counted it.
  - Shuffling the class labels erased the class effect: the median shuffled effect was −0.05 per ID-minute, against a real effect of 187.6.
  - The gate itself went red when RandomPosition was deliberately broken (gpsDrift 10⁹ m) and when the real labels were replaced by shuffled ones.
- **The class result holds on all 4.1 M Orem rows, not only on the 30-minute sample of check 3.** RandomPosition v1.0 (1 m, secMark Δt, gap limit 0.5 s, v ≥ 10 m/s), in triggers per ID-minute with ID-clustered 95% CIs (T2):

  | Set | Locked devices | Free devices |
  |---|---|---|
  | Tampa | 0.2 [0.0, 0.6] (1 in 2,783 pairs) | 20.4 [18.2, 22.5] (4.7% of pairs) |
  | Orem, 7 days | 0.0 (0 in 1,279 pairs, 3 IDs) | 188.8 [187.4, 190.1] (41.7% of pairs, 2,351 IDs) |
  | Wyoming (held out) | 0.0 (0 in 4,946 pairs, 12 IDs) | no free IDs |

- **Out of sample, the class test is only half tested (T7).**
  - The thresholds and bounds were frozen in commit `0ebf53a3` before Wyoming was scored.
  - Wyoming's locked class passes: 0.0, against a frozen bound of at most 0.568.
  - Wyoming has no free IDs, so the claim "the free class trips at speed" has **no out-of-sample test yet**.
  - The in-fit transfer from Tampa to Orem passes in both classes. That is not out of sample.
  - Until a new free-class set is scored, the safe wording is "devices whose secMark is not locked to the fix epoch". "A device class that predicts the rate out of sample" must wait.
- **The time base decides the result (T3).** The same honest BSMs go from 0 to:
  - **214 per ID-minute** on Wyoming when Δt comes from the header time (`metadata_generatedat`) instead of secMark;
  - **17 per ID-minute** on Tampa and Orem locked devices when it comes from the RSU receive time.

  So a receiver that judges with the 1609.2 generation time, as the engine does, fires on locked devices too, if that header time is what P1-E3 must still confirm it is.
- **The two normative readings of RandomPosition give the same rates at 100 ms (T3).** The acceleration term ½·a·Δt² is 1.5 cm at 3 m/s² and 100 ms, and no cell of T3 moves by more than 0.21 per ID-minute. The readings differ in practice only through the pair spacing. At the v1.1 canonical 0.1 s spacing, Orem's free rate is 222.6 per ID-minute, against 188.8 at 0.5 s.
- **The fitted timing model is in `data/calibration/bsm-timing-cvpilot.yaml`** (schema `v2xw.bsm_timing.v1`, T8).

## 1. Data used

| Set | Query (pinned in `tools/realdata/sources.py`) | Rows | Role |
|---|---|---|---|
| Tampa `nm7w-nvbm` | all rows, 12 columns | 34,464 (= count(*)) | fit |
| Orem `ev64-jn8c` | **all 7 days** (2025-02-02..08), one query per day, 12 columns | 4,137,684 (= count(*)) | fit |
| Wyoming `7qu7-j8rx` | all rows, 11 columns | 6,597 (= count(*)) | **held out** (time-base test; header jitter only) |
| NYC `vxmm-9bi5` | **stated subset**: the first 600 of 10,533 events by row id (65,533 BSMs, 893 host/remote IDs) | 600 | reported separately |

- **Raw rows never enter the repository.** Tampa and Wyoming are CC BY-SA, and Orem states no licence. The raw rows live only in the scratch directory. The sha256 of each file is in `data/calibration/cvpilot-raw-manifest.json`.
- **The fetches are byte-reproducible.** Re-fetching Tampa and Wyoming gave identical sha256.
- **NYC is a subset because of its size.** Each event carries a ±10 s JSON BSM list of about 175 KB, so all 10,533 events would be about 1.8 GB. NYC is warning-selected and its positions are translated, so it is never pooled with the others.
- **Peak memory was 110 MB.** Every stage streams one temporary ID at a time.

## 2. Method (as implemented)

- **Normalise.** J2735 units are converted and every unavailable sentinel becomes missing. That covers speed 8191, heading 28800, accel 2001, semi-major 255, secMark ≥ 60000, and BrakeAppliedStatus "unavailable" (Orem strips the leading zeros of that bit string). Each source has its own unit test.
  - Wyoming is already in SI units.
  - Wyoming's brake columns are constant on all 6,597 rows (a wheel braking at 35 m/s everywhere), so they are treated as unavailable.
  - Wyoming is sequenced by its own secMark, because the header time jitters by ±50 ms and could swap neighbouring BSMs.
- **Pair.** BSMs are de-duplicated on (ID, secMark, msgCnt) and paired consecutively. Δt is computed under every time base the source has:
  - secMark (all sets);
  - header (Wyoming);
  - RSU receive time, "rx" (Tampa, Orem).

  A pair is judged under gap limit L when 0 < Δt < L + 50 ms, that is, when Δt rounded to the 100 ms BSM interval is at most L.
- **Checks** (`mbrchecks.py`), at the published defaults:
  - RandomPosition v1.0 (velocity only) and v1.1 (velocity plus accelSet.long);
  - ConstantPosition (both speeds ≥ 0.02 m/s, moved < 1 cm);
  - MaxSpeed (> 90 m/s);
  - MaxAcceleration, both as implied |Δv|/Δt and as the transmitted accelSet;
  - BrakeAcceleration (brakes applied and accelSet.long ≥ 0 while moving: an ASSUMPTION, because the v1.0 PDF's rule text could not be machine-read here).
- **Rates.** Triggers per observed ID-minute of judged pairs. The units are temporary-ID periods, not vehicles.
  - CIs come from a 1,000-replicate bootstrap that resamples IDs. Its seeds are keyed by cell, so the CIs are deterministic.
- **Class** (`devclass.py`). An ID is locked when more than 80% of its secMarks lie within 1 ms of a 50 ms epoch, and free when fewer than 30% do; IDs with fewer than 10 BSMs are unclassified.
  - These are the pre-registered thresholds of §1.3.
  - **The bound rule was changed once, before freezing and before any Wyoming number existed.** With bounds pooled over Tampa and Orem, Orem's 2,351 free IDs set the free bound at 186 per ID-minute, which would call Tampa's own free devices (20.4) "not free". The frozen rule therefore uses per-fit-set bounds: locked at most the highest locked upper 95% bound (0.568), free at least the lowest free lower 95% bound (18.2).

## 3. Results beyond the headline

1. **Speed (T4).** Free-device triggers are a high-speed effect.
   - Orem free: 9.5% of pairs at 5–10 m/s, 41% at 10–20 m/s and 49% at ≥ 20 m/s, against ≤ 1.1% below 5 m/s.
   - Tampa free: 4.8% and 4.5% at the two fast bands.
   - The residual scales with v·Δt, as an epoch-misalignment mechanism predicts.
2. **Anatomy (T6).**
   - Of Orem's 512,733 free-device triggers, 30% have |Δp| ≈ 0 (stale fix) and 49% have |Δp| ≈ 2 v Δt (fix from the next epoch). Tampa's are 4% and 46%.
   - The cross-track component dominates in at most 1% of the triggers in any set: the errors are along-track, not GNSS white noise.
   - Free devices transmit with a period above the 100 ms fix interval. secMark Δt minus 100 ms has p50 6 ms and p90 36 ms (T8 comments). So their timestamp phase drifts across the fix grid, and each wrap shows up as a skipped or a repeated fix.
3. **ConstantPosition (T4b) has two benign sources.**
   - **Creep at the 1e-7° position resolution:** 10.8% of Tampa locked pairs below 0.5 m/s, which is 56 per ID-minute.
   - **Frozen positions while moving:** 13–14% of Orem free pairs in every band from 0.5 m/s up, about 60 per ID-minute.
   - Each frozen pair is also a RandomPosition trigger at speed.
4. **The other checks (T5).**
   - MaxSpeed fires only in NYC (10.8 per ID-minute). Nine NYC IDs report impossible constant speeds (96.7 m/s on 999 BSMs): faulty devices or processing artefacts, but real benign triggers under the rule.
   - Implied MaxAcceleration is ≤ 0.3 per ID-minute everywhere, and the accelSet reading is 0.
   - BrakeAcceleration can be evaluated only on Orem and NYC. It is 0.9 [0.8, 1.0] per ID-minute on Orem free devices, under the stated ASSUMPTION.
5. **NYC (separate).**
   - RandomPosition fires at 89.1 [68.5, 110.9] per ID-minute at speed, on 16% of pairs.
   - 65% of its triggers are stale-fix pairs: the frozen runs on freeway FCW events that check 3 saw.
   - Its device class cannot be read, because secMark is replaced by T_s relative to the warning.

## 4. The fitted timing model (input for P1-E4)

`data/calibration/bsm-timing-cvpilot.yaml` holds the per-class parameters and their provenance: the queries, the manifest sha256, fitted_on, held_out and the tool version.

| Parameter | locked | free |
|---|---|---|
| ID share (pooled fit IDs) | 0.016 | 0.984 |
| secMark phase | normal, mean −0.20 ms, sd 0.41 ms (to the nearest 50 ms epoch) | empirical, 10 ms bins, near-uniform over 0–100 ms |
| epoch_slip_prob (onsets per moving pair) | 0.0067 | 0.227 |
| slip size given a slip | −1: 0.44, +1: 0.56 | +1: 0.98, +2: 0.02 |
| frozen_repeat_prob | 0 | 0.156 |
| header_jitter_ms sd | 28.3 (Wyoming; mean offset 38.7 ms; PROVISIONAL until P1-E3) | borrowed from locked |

- **The class share is deployment-specific.** Tampa has 23% locked IDs and Orem 0.2%. The engine should treat the share as a scenario knob, with these two values as anchors.
- **The phase should be drawn per device and drift, not drawn independently per message** (see the YAML comment). Independent per-message phases would add a second slip mechanism on top of the fitted slip probabilities.
- **The header offset is persistent.** It changes in steps of about 50 ms rather than per message, so an independent per-message sd overstates the pair-to-pair jitter of most pairs and understates the step.

## 5. What is still open

1. **P1-E3 needs the raw Wyoming `.bin` frames,** through the its.dot.gov data request (O1). Decode `headerInfo.generationTime` (Time64, µs) of each signed BSM, match it to the dataset rows on (ID, secMark, msgCnt), and compare it with `metadata_generatedat`.
   - If they agree, the header-base row of T3 (0 → 214 per ID-minute) and the header_jitter_ms field stand.
   - If `metadata_generatedat` is ODE log time, both must be dropped.
   - The persistent ~50 ms steps and the +38.7 ms mean offset look more like a logger than like a GNSS-disciplined 1609.2 clock, which makes the check urgent.
2. **An out-of-sample free class.** Wyoming has none. Candidates are the full Tampa raw data (another night), NYC raw data with absolute secMark, SPMD, or the Zenodo corpora, all through the §4.3 requests. Until then the claim stays in-sample for the free class.
3. **The rule text of BrakeAcceleration and the "0.1 s apart" wording.** Read both from the PDF by hand, and re-run if the BrakeAcceleration condition differs from the assumption.
4. **ID-minutes are not vehicle-minutes.** Temporary-ID periods are the unit, and IDs change every few minutes.

## 6. Rebuild

From the repository root, with RAW a directory outside the repository (here the session scratch `realdata/`). Run CPU-heavy runs through the machine's run lock:

```sh
python3 -m unittest discover tools/realdata                       # 26 unit tests, < 1 s
python3 tools/realdata/rdtool.py fetch --raw RAW                  # stage 1: ~8 min of network, ~500 MB; skips files matching the manifest
HEAVY_LOCK=/private/tmp/claude-501/v2xw-run.lock HEAVY_MEM_CAP_MB=2500 heavy.sh \
  python3 tools/realdata/rdtool.py all --raw RAW                  # stages 2-7: ~1 min CPU, 110 MB peak
```

- **What `all` checks before it writes anything:**
  - the raw files must match the committed manifest;
  - the refit of the classifier must reproduce `tools/realdata/frozen/classifier-v1.json` byte for byte;
  - that frozen file must be committed and clean (so the Wyoming score always follows a committed freeze);
  - the gate must pass.
- **What it writes:** the tables below, `research/synthesis/p1-realdata-results.json` and the YAML.
- **`rdtool.py fit` is the only way to re-freeze the classifier.** After it, the frozen file must be committed before `all` will score Wyoming.

---

## Generated tables

<!-- BEGIN GENERATED: tools/realdata rdtool.py all -->
### T0. Falsification gate (run before every table)

| Test | Result | Detail |
|---|---|---|
| G1 jump | pass | injected step at BSM 661: new triggers at pairs [660] (want [660]) |
| G2 slip | pass | slip at BSM 661: new triggers {660: 'double', 661: 'zero'}; estimator +1 slips of +1 epoch |
| G3 placebo | pass | real effect 187.57/ID-min; shuffled median -0.05, 97.5th centile 12.47 (200 shuffles) |
| G4a broken check must fail the gate | pass | gpsDrift 1e9 m: gate failed as it must; G1 jump red; G2 slip red; G3 placebo ok |
| G4b shuffled labels must fail G3 | pass | real effect -5.28/ID-min; shuffled median 0.12, 97.5th centile 12.70 (200 shuffles) |

Gate: **PASSED**.

### T1. Datasets

| Dataset | Role | Raw rows | Unique BSMs | IDs | locked / mixed / free / unclassified IDs | secMark Δt in 90-110 ms | msgCnt +1 |
|---|---|---|---|---|---|---|---|
| Tampa | fit | 34464 | 34464 | 226 | 41 / 0 / 137 / 48 | 69.9% | 78.1% |
| Orem | fit | 4137684 | 4132447 | 2951 | 4 / 3 / 2617 / 327 | 52.0% | 91.1% |
| Wyoming | held_out | 6597 | 6579 | 16 | 12 / 0 / 0 / 4 | 53.2% | 74.5% |
| NYC (warning-selected) | separate | 65533 | 65533 | 893 | n/a (no secMark phase) | 78.9% | 80.2% |

### T2. RandomPosition v1.0 (gpsDrift 1 m), secMark Δt, gap limit 0.5 s, v ≥ 10 m/s

Rate is triggers per observed ID-minute of judged pairs, with an ID-clustered bootstrap 95% CI (1000 replicates).

| Dataset | Class | IDs | Pairs | Triggers | % of pairs | Triggers per ID-minute [95% CI] |
|---|---|---|---|---|---|---|
| Tampa | locked | 37 | 2783 | 1 | 0.04 | 0.2 [0.0, 0.6] |
| Tampa | free | 121 | 7704 | 362 | 4.70 | 20.4 [18.2, 22.5] |
| Tampa | all | 193 | 10576 | 365 | 3.45 | 15.1 [12.7, 17.8] |
| Orem | locked | 3 | 1279 | 0 | 0.00 | 0.0 [0.0, 0.0] |
| Orem | mixed | 1 | 4 | 3 | 75.00 | 196.9 [196.9, 196.9] |
| Orem | free | 2351 | 1228310 | 512733 | 41.74 | 188.8 [187.4, 190.1] |
| Orem | all | 2441 | 1229865 | 512838 | 41.70 | 188.6 [187.2, 189.9] |
| Wyoming | locked | 12 | 4946 | 0 | 0.00 | 0.0 [0.0, 0.0] |
| Wyoming | all | 12 | 4946 | 0 | 0.00 | 0.0 [0.0, 0.0] |
| NYC (warning-selected) | all | 413 | 22878 | 3717 | 16.25 | 89.1 [68.5, 110.9] |

### T7. Device-class test out of sample (P1-E2)

Thresholds and predictions frozen in `tools/realdata/frozen/classifier-v1.json` (committed 0ebf53a3) before Wyoming was scored. Headline cell as T2.

| Set | Class | IDs | Rate [95% CI] | Frozen prediction | Result |
|---|---|---|---|---|---|
| Wyoming (held out) | locked | 12 | 0.0 [0.0, 0.0] | {"rate_at_most": 0.568368} | PASS |
| Wyoming (held out) | free | 0 | - | {"rate_at_least": 18.2124} | not testable: no free IDs |

Secondary (in-fit, not out of sample): bounds from Tampa alone, scored on Orem. Tampa locked 0.2 [0.0, 0.6], free 20.4 [18.2, 22.5]; Orem locked 0.0 [0.0, 0.0] (≤ 0.6: pass), free 188.8 [187.4, 190.1] (≥ 18.2: pass).

### T3. RandomPosition by reading, time base and gap limit (v ≥ 10 m/s)

v1.0 = velocity only; v1.1 = velocity and accelSet.long (ASN.1 v1.1; its canonical cell is the 0.1 s limit). Cells: triggers per ID-minute [95% CI]. Wyoming has no accelSet.long, so its v1.1 rows equal v1.0.

| Dataset | Class | Reading | Δt base | ≤ 0.1 s | ≤ 0.2 s | ≤ 0.5 s |
|---|---|---|---|---|---|---|
| Tampa | locked | v1.0 | secMark | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.2 [0.0, 0.6] |
| Tampa | locked | v1.0 | rx | 21.0 [9.1, 36.5] | 18.5 [8.4, 32.0] | 16.6 [7.5, 26.8] |
| Tampa | locked | v1.1 | secMark | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.2 [0.0, 0.6] |
| Tampa | locked | v1.1 | rx | 21.0 [8.8, 36.0] | 18.7 [8.4, 32.9] | 16.7 [8.0, 28.5] |
| Tampa | free | v1.0 | secMark | 17.9 [15.2, 20.7] | 19.7 [17.4, 22.2] | 20.4 [18.2, 22.5] |
| Tampa | free | v1.0 | rx | 45.8 [31.6, 62.4] | 44.6 [33.1, 59.8] | 45.4 [34.1, 58.6] |
| Tampa | free | v1.1 | secMark | 17.8 [15.1, 20.7] | 19.6 [17.1, 21.9] | 20.3 [18.3, 22.4] |
| Tampa | free | v1.1 | rx | 45.8 [31.0, 63.6] | 44.6 [32.2, 61.2] | 45.5 [34.2, 58.8] |
| Tampa | all | v1.0 | secMark | 13.2 [10.4, 16.3] | 14.7 [11.9, 17.5] | 15.1 [12.7, 17.8] |
| Tampa | all | v1.0 | rx | 39.5 [28.2, 53.9] | 37.9 [28.5, 49.1] | 38.0 [28.8, 48.8] |
| Tampa | all | v1.1 | secMark | 13.1 [10.4, 15.9] | 14.6 [12.0, 17.4] | 15.1 [12.7, 17.7] |
| Tampa | all | v1.1 | rx | 39.5 [27.5, 53.8] | 38.0 [28.8, 50.9] | 38.1 [29.2, 49.3] |
| Orem | locked | v1.0 | secMark | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] |
| Orem | locked | v1.0 | rx | 14.5 [12.3, 16.1] | 16.5 [14.1, 17.6] | 16.6 [15.0, 17.5] |
| Orem | locked | v1.1 | secMark | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] |
| Orem | locked | v1.1 | rx | 14.5 [12.3, 16.1] | 16.5 [14.1, 17.6] | 16.6 [15.0, 17.5] |
| Orem | free | v1.0 | secMark | 222.6 [221.3, 223.8] | 204.6 [203.3, 205.9] | 188.8 [187.4, 190.1] |
| Orem | free | v1.0 | rx | 390.1 [388.7, 391.7] | 342.1 [340.9, 343.3] | 289.3 [288.2, 290.4] |
| Orem | free | v1.1 | secMark | 222.7 [221.4, 223.9] | 204.6 [203.4, 205.9] | 188.8 [187.4, 190.2] |
| Orem | free | v1.1 | rx | 390.2 [388.8, 391.7] | 342.1 [340.9, 343.4] | 289.3 [288.2, 290.4] |
| Orem | all | v1.0 | secMark | 222.4 [221.0, 223.7] | 204.4 [203.1, 205.7] | 188.6 [187.2, 189.9] |
| Orem | all | v1.0 | rx | 389.6 [387.8, 391.2] | 341.7 [340.3, 343.1] | 289.1 [287.8, 290.3] |
| Orem | all | v1.1 | secMark | 222.4 [221.0, 223.7] | 204.4 [203.1, 205.7] | 188.6 [187.3, 190.1] |
| Orem | all | v1.1 | rx | 389.6 [388.1, 391.3] | 341.7 [340.4, 343.1] | 289.1 [288.0, 290.3] |
| Wyoming | locked | v1.0 | secMark | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] |
| Wyoming | locked | v1.0 | header | 218.4 [203.2, 232.9] | 220.5 [204.0, 235.5] | 214.2 [196.8, 228.1] |
| Wyoming | locked | v1.1 | secMark | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] |
| Wyoming | locked | v1.1 | header | 218.4 [201.5, 232.6] | 220.5 [204.0, 235.0] | 214.2 [197.8, 227.0] |
| Wyoming | all | v1.0 | secMark | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] |
| Wyoming | all | v1.0 | header | 218.4 [201.7, 233.0] | 220.5 [204.4, 233.9] | 214.2 [195.7, 227.4] |
| Wyoming | all | v1.1 | secMark | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] |
| Wyoming | all | v1.1 | header | 218.4 [203.1, 233.1] | 220.5 [204.4, 235.9] | 214.2 [196.7, 227.0] |
| NYC (warning-selected) | all | v1.0 | secMark | 94.9 [71.4, 118.7] | 92.0 [69.9, 114.3] | 89.1 [68.5, 110.9] |
| NYC (warning-selected) | all | v1.1 | secMark | 94.8 [72.2, 119.7] | 92.0 [71.1, 115.8] | 89.1 [67.7, 112.9] |

### T4. RandomPosition v1.0 by speed band (secMark, 0.5 s)

Cells: % of judged pairs over 1 m (triggers per ID-minute).

| Dataset | Class | 0-0.5 | 0.5-5 | 5-10 | 10-20 | >=20 m/s |
|---|---|---|---|---|---|---|
| Tampa | locked | 0.00% (0.0) | 0.00% (0.0) | 0.05% (0.2) | 0.00% (0.0) | 0.32% (1.5) |
| Tampa | free | 0.05% (0.2) | 0.21% (0.9) | 0.16% (0.7) | 4.83% (20.6) | 4.50% (20.0) |
| Tampa | all | 0.04% (0.2) | 0.17% (0.7) | 0.13% (0.5) | 3.15% (13.8) | 4.09% (18.1) |
| Orem | locked | 0.00% (0.0) | 0.00% (0.0) | 0.00% (0.0) | 0.00% (0.0) | - |
| Orem | free | 0.64% (2.9) | 1.10% (4.8) | 9.49% (41.7) | 41.06% (184.9) | 48.61% (229.9) |
| Orem | all | 0.64% (2.9) | 1.10% (4.8) | 9.46% (41.6) | 41.01% (184.7) | 48.61% (229.9) |
| Wyoming | locked | - | - | - | 0.00% (0.0) | 0.00% (0.0) |
| Wyoming | free | - | - | - | - | - |
| Wyoming | all | - | - | - | 0.00% (0.0) | 0.00% (0.0) |
| NYC (warning-selected) | all | 3.93% (22.0) | 0.34% (1.9) | 1.34% (7.4) | 11.51% (63.2) | 28.08% (153.7) |

### T4b. ConstantPosition by speed band (secMark, 0.5 s)

Cells: % of judged pairs frozen (< 1 cm) while both speeds >= 0.02 m/s (triggers per ID-minute).

| Dataset | Class | 0-0.5 | 0.5-5 | 5-10 | 10-20 | >=20 m/s |
|---|---|---|---|---|---|---|
| Tampa | locked | 10.84% (56.1) | 0.00% (0.0) | 0.00% (0.0) | 0.00% (0.0) | 0.00% (0.0) |
| Tampa | free | 2.44% (11.0) | 0.21% (0.9) | 0.02% (0.1) | 0.02% (0.1) | 0.00% (0.0) |
| Tampa | all | 3.81% (17.5) | 0.17% (0.7) | 0.02% (0.1) | 0.01% (0.1) | 0.00% (0.0) |
| Orem | locked | 1.17% (7.0) | 0.25% (1.4) | 0.00% (0.0) | 0.00% (0.0) | - |
| Orem | free | 0.86% (4.0) | 12.94% (56.6) | 13.47% (59.1) | 13.48% (60.7) | 14.24% (67.3) |
| Orem | all | 0.86% (4.0) | 12.93% (56.6) | 13.42% (58.9) | 13.46% (60.6) | 14.24% (67.3) |
| Wyoming | locked | - | - | - | 0.00% (0.0) | 0.00% (0.0) |
| Wyoming | free | - | - | - | - | - |
| Wyoming | all | - | - | - | 0.00% (0.0) | 0.00% (0.0) |
| NYC (warning-selected) | all | 1.30% (7.3) | 4.53% (25.2) | 4.44% (24.3) | 6.48% (35.6) | 5.53% (30.3) |

### T5. The other SM-MBR checks (secMark, 0.5 s, all speeds)

Cells: triggers per ID-minute [95% CI]. MaxAccel (implied) is |Δv|/Δt > 10 m/s² (the engine's reading); MaxAccel (accelSet) is the transmitted |accelSet.long| > 10. BrakeAccel needs brakes and accelSet.long: only Orem and NYC carry both.

| Dataset | Class | ConstantPosition | MaxSpeed | MaxAccel (implied) | MaxAccel (accelSet) | BrakeAccel |
|---|---|---|---|---|---|---|
| Tampa | locked | 13.0 [6.7, 18.5] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] |
| Tampa | free | 4.3 [3.2, 5.2] | 0.0 [0.0, 0.0] | 0.3 [0.1, 0.6] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] |
| Tampa | all | 6.1 [4.4, 7.9] | 0.0 [0.0, 0.0] | 0.2 [0.1, 0.5] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] |
| Orem | locked | 1.0 [0.0, 1.9] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] |
| Orem | free | 31.9 [29.5, 34.4] | 0.0 [0.0, 0.0] | 0.1 [0.0, 0.1] | 0.0 [0.0, 0.0] | 0.9 [0.8, 1.0] |
| Orem | all | 31.9 [29.4, 34.5] | 0.0 [0.0, 0.0] | 0.1 [0.0, 0.1] | 0.0 [0.0, 0.0] | 0.9 [0.8, 1.0] |
| Wyoming | locked | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] |
| Wyoming | all | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] |
| NYC (warning-selected) | all | 25.5 [18.9, 33.3] | 10.8 [4.2, 19.4] | 1.5 [0.2, 4.2] | 0.0 [0.0, 0.0] | 0.0 [0.0, 0.0] |

### T6. Anatomy of the RandomPosition triggers (secMark, 0.5 s, v ≥ 10 m/s)

Epoch-slip signature: |Δp| ≈ 0 (< 0.4 v Δt, stale fix) or ≈ 2 v Δt (1.6-2.4, fix from the next epoch). Residual centiles of all judged pairs, from 1 dB log bins (upper bin edge).

| Dataset | Class | Triggers | ≈ 0 | ≈ 2 v Δt | cross-track dominant | residual p50 / p99 (m) |
|---|---|---|---|---|---|---|
| Tampa | locked | 1 | 0% | 0% | 0% | 0.013 / 0.40 |
| Tampa | free | 362 | 4% | 46% | 1% | 0.100 / 2.51 |
| Tampa | all | 365 | 4% | 45% | 1% | 0.063 / 2.51 |
| Orem | locked | 0 | - | - | - | 0.040 / 0.13 |
| Orem | free | 512733 | 30% | 49% | 0% | 0.794 / 3.16 |
| Orem | all | 512838 | 30% | 49% | 0% | 0.794 / 3.16 |
| Wyoming | locked | 0 | - | - | - | 0.020 / 0.16 |
| Wyoming | all | 0 | - | - | - | 0.020 / 0.16 |
| NYC (warning-selected) | all | 3717 | 65% | 21% | 1% | 0.032 / 10.00 |

### T8. Fitted timing-model parameters (fit sets: Tampa + Orem)

Written to `data/calibration/bsm-timing-cvpilot.yaml` (schema v2xw.bsm_timing.v1).

| Class | IDs | single-interval moving pairs at risk | slip onsets | frozen-repeat onsets |
|---|---|---|---|---|
| locked | 45 | 4010 | 27 | 0 |
| mixed | 3 | 4 | 1 | 0 |
| free | 2754 | 554405 | 125843 | 86653 |

```yaml
# BSM device-timing model fitted on public CV-pilot BSMs (P1-E1/P1-E2).
# GENERATED by `python3 tools/realdata/rdtool.py all` - do not edit by hand.
# Derived statistics only. Shares are over locked+free IDs of the fit sets; 3 mixed and
# unclassified IDs are left out of the shares. Phases are secMark mod 100 ms (fix epochs
# assumed on the 100 ms grid); locked devices sit on either the :00 or the :50 grid, so the
# locked phase is the distance to the nearest 50 ms epoch.
# Tampa: 41 locked, 137 free IDs (locked share 0.230); the share is deployment-specific.
# Orem: 4 locked, 2617 free IDs (locked share 0.002); the share is deployment-specific.
schema: v2xw.bsm_timing.v1
provenance:
  sources:
    - {dataset: ev64-jn8c, rows: 295906, query: "https://data.transportation.gov/resource/ev64-jn8c.csv?%24select=tmstp_epoch,temp_id,msg_count,sec_mark,latitude,longitude,speed,heading,longitudinal_acceleration,semi_major_axis_accuracy,semi_minor_axis_accuracy,wheel_brakes_flags&%24order=temp_id,tmstp_epoch,:id&%24where=date_utc+%3D+'2025-02-02T00:00:00.000'"}
    - {dataset: ev64-jn8c, rows: 655514, query: "https://data.transportation.gov/resource/ev64-jn8c.csv?%24select=tmstp_epoch,temp_id,msg_count,sec_mark,latitude,longitude,speed,heading,longitudinal_acceleration,semi_major_axis_accuracy,semi_minor_axis_accuracy,wheel_brakes_flags&%24order=temp_id,tmstp_epoch,:id&%24where=date_utc+%3D+'2025-02-03T00:00:00.000'"}
    - {dataset: ev64-jn8c, rows: 598592, query: "https://data.transportation.gov/resource/ev64-jn8c.csv?%24select=tmstp_epoch,temp_id,msg_count,sec_mark,latitude,longitude,speed,heading,longitudinal_acceleration,semi_major_axis_accuracy,semi_minor_axis_accuracy,wheel_brakes_flags&%24order=temp_id,tmstp_epoch,:id&%24where=date_utc+%3D+'2025-02-04T00:00:00.000'"}
    - {dataset: ev64-jn8c, rows: 621138, query: "https://data.transportation.gov/resource/ev64-jn8c.csv?%24select=tmstp_epoch,temp_id,msg_count,sec_mark,latitude,longitude,speed,heading,longitudinal_acceleration,semi_major_axis_accuracy,semi_minor_axis_accuracy,wheel_brakes_flags&%24order=temp_id,tmstp_epoch,:id&%24where=date_utc+%3D+'2025-02-05T00:00:00.000'"}
    - {dataset: ev64-jn8c, rows: 706683, query: "https://data.transportation.gov/resource/ev64-jn8c.csv?%24select=tmstp_epoch,temp_id,msg_count,sec_mark,latitude,longitude,speed,heading,longitudinal_acceleration,semi_major_axis_accuracy,semi_minor_axis_accuracy,wheel_brakes_flags&%24order=temp_id,tmstp_epoch,:id&%24where=date_utc+%3D+'2025-02-06T00:00:00.000'"}
    - {dataset: ev64-jn8c, rows: 846126, query: "https://data.transportation.gov/resource/ev64-jn8c.csv?%24select=tmstp_epoch,temp_id,msg_count,sec_mark,latitude,longitude,speed,heading,longitudinal_acceleration,semi_major_axis_accuracy,semi_minor_axis_accuracy,wheel_brakes_flags&%24order=temp_id,tmstp_epoch,:id&%24where=date_utc+%3D+'2025-02-07T00:00:00.000'"}
    - {dataset: ev64-jn8c, rows: 413725, query: "https://data.transportation.gov/resource/ev64-jn8c.csv?%24select=tmstp_epoch,temp_id,msg_count,sec_mark,latitude,longitude,speed,heading,longitudinal_acceleration,semi_major_axis_accuracy,semi_minor_axis_accuracy,wheel_brakes_flags&%24order=temp_id,tmstp_epoch,:id&%24where=date_utc+%3D+'2025-02-08T00:00:00.000'"}
    - {dataset: nm7w-nvbm, rows: 34464, query: "https://data.transportation.gov/resource/nm7w-nvbm.csv?%24select=metadata_recordgeneratedat,coredata_id,coredata_secmark,coredata_msgcnt,coredata_lat,coredata_long,coredata_speed,coredata_heading,coredata_accelset_long,coredata_accuracy_semimajor,coredata_accuracy_semiminor,coredata_wheelbrakes&%24order=coredata_id,metadata_recordgeneratedat,:id"}
  manifest_sha256: ee6eeab3f926d0fefac2e790d0dea9c5411531d7823d39558620a5521189d412
  fitted_on: ["nm7w-nvbm", "ev64-jn8c", "7qu7-j8rx (header_jitter_ms only)"]
  held_out: ["7qu7-j8rx (device class and trigger rates)"]
  tool: tools/realdata 1.0.0
fix_interval_ms: 100
classes:
  - name: locked
    share: 0.016077
    # normal fitted to the 100.0% of locked BSMs within 5 ms of a 50 ms epoch
    secmark_phase_ms: {dist: normal, mean: -0.204323, sd: 0.40966}
    # secMark Δt minus 100 ms on single-interval pairs: p10 -1, p50 0, p90 1 ms
    # onsets per single-interval moving pair (v >= 5 m/s) after a normal pair: 4010 at risk
    epoch_slip_prob: 0.00673317
    slip_epochs: {"-1": 0.444444, "1": 0.555556, "2": 0.0}
    frozen_repeat_prob: 0.0
    # from 6579 Wyoming BSMs (the only source with a header time; its class and rate results
    # stay held out). Header minus secMark also has a mean of 38.7 ms, and it is
    # persistent: it changes in steps of about 50 ms rather than per message. PROVISIONAL
    # until P1-E3: if metadata_generatedat is ODE log time, not the signed 1609.2
    # generationTime, this is not a header jitter.
    header_jitter_ms: {sd: 28.2875}
  - name: free
    share: 0.983923
    # bins_ms are the 11 edges of 10 bins; p[i] is the share in [bins_ms[i], bins_ms[i+1])
    secmark_phase_ms: {dist: empirical, bins_ms: [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100], p: [0.088566, 0.085286, 0.096363, 0.107481, 0.121526, 0.109662, 0.095202, 0.096257, 0.09905, 0.100607]}
    # secMark Δt minus 100 ms on single-interval pairs: p10 0, p50 6, p90 36 ms
    # the free BSM period exceeds the fix interval, so the phase drifts across the fix grid:
    # draw the phase per device and let it drift; independent per-message draws double count
    # the slips below, which are the observed onsets of a position-epoch change
    # onsets per single-interval moving pair (v >= 5 m/s) after a normal pair: 554405 at risk
    epoch_slip_prob: 0.226987
    slip_epochs: {"-1": 0.000182767, "1": 0.978934, "2": 0.0208832}
    frozen_repeat_prob: 0.156299
    # no free-class source has a header time: borrowed from the locked Wyoming fit
    header_jitter_ms: {sd: 28.2875}
```

<!-- END GENERATED -->
