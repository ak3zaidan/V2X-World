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
| secMark phase | normal per message, mean 0.286 ms, sd 0.254 ms about the nearest 50 ms epoch, each ID weighing one (1.3.0; v1: −0.20 / 0.41) | empirical, 10 ms bins, near-uniform over 0–100 ms |
| epoch_slip_prob (onsets per pair at v ≥ 10 m/s, Δt-corrected) | 0 (0 of 2,275) | 0.228 |
| slip size given a slip | none observed (pooled sizes written) | −1: 0.005, +1: 0.96, +2: 0.04 |
| frozen_repeat_prob (per pair at v ≥ 5 m/s) | 0 | 0.156 |
| header_jitter_ms sd | 28.3 (Wyoming; mean offset 38.7 ms; PROVISIONAL until P1-E3) | borrowed from locked |
| output_noise (v3, tools/realdata 1.2.0) | speed 0.031 m/s, heading 0.052° (≤ BSM-to-BSM change / √2) | borrowed from locked |
| position_noise (v3, 1.3.0) | random walk over fix epochs, τ 50 s (fitted in sample); increment a 4-component normal mixture, each ID weighing one: p 0.633 / 0.169 / 0.113 / 0.084 with along 0.0 / 0.0 / 5.2 / 13.3 cm and cross 0.3 / 1.2 / 3.1 / 9.8 cm | borrowed from locked |
| grid_hop (v3) | leave the home :00/:50 grid 0.113, return 0.445 per BSM (18.0 % of steps are 50 or 150 ms) | none (own BSM clock) |

- **Refit 2026-10-25b (tools/realdata 1.3.0, testing/paper1-validation.md §8).** The 1.2.0 model, simulated at the fit pairs' own speeds and quantisation, did not reproduce its own exactly-100 ms residuals (KS 0.275): a walk reverting with the assumed τ 1 s adds (ρ − 1)·e to every pair, about 1 cm per axis with the fitted tail, while the EM reads each residual as the increment alone. τ is now fitted in sample over 1–100 s (the cap is the receiver's 100 s carrier smoothing): 50 s, KS 0.083. The stamp offsets and the position-noise EM weigh each ID once, as the engine's fleet is devices (Orem's 3 locked IDs held 28 % of the pairs): the locked secMark phase is N(0.286, 0.254) ms (13 % of BSMs a whole ms early, was 21 %). Cost: the walk's stationary spread is about 0.7 m along and 0.5 m across, on top of the engine's GNSS error (open). Wyoming was not used.
- **Refit 2026-10-25 (tools/realdata 1.2.0, schema v3, testing/paper1-validation.md §8).** On the locked exactly-100 ms pairs (1,961, Tampa + Orem) the transmitted speed and heading barely change between BSMs (sd 0.045 m/s and 0.074°), so the residual body cannot be velocity noise: v2's 0.103 m/s and 0.30° would make those changes 0.15 m/s and 0.43°. What is left after each pair's own speed and heading change, the quantisation, the stamp jitter (0.28 ms × v) and that bounded output noise is a heavy-tailed, white increment of the reported position (Tampa rms 5.7 cm against an IQR-based sd of 0.7 cm; lag-1 −0.02 along, +0.05 across), fitted as a normal mixture by EM. Orem's 3 locked IDs are noisier and closer to white position noise (lag-1 −0.33); the pooled fit weights them by their pairs. The :00/:50 grid hops are a two-state process per BSM from 9,255 consecutive-msgCnt transitions. Wyoming was not used.

- **Refit 2026-10-24 (`a77aab7c`, testing/paper1-validation.md §3).** The first fit gave the locked class 0.0067: 27 onsets, every one at v ≥ 10 m/s a :00/:50 grid hop (Δt 0.05 or 0.15 s, position consistent with Δt, residual 0.03–0.35 m) that rounding Δt to whole fix intervals read as a slip. Slip size is now the displacement beyond v̄·Δt, and the probability is fitted at v ≥ 10 m/s, where a one-epoch slip is ≥ gpsDrift. In the engine the old value gave an all-locked fleet 5.5 RandomPosition triggers per ID-minute at speed, ten times the locked bound Wyoming passed.
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

## 7. Phase-locked RSU loss across sites, and the device clock behind P1-E4 (2026-10-27)

This is analysis only. No engine runs were made and no shipped numbers changed. The candidates are new files: `data/calibration/rsu-off-channel-tampa-per-rsu.yaml` and `data/calibration/bsm-stamp-epoch-candidates.yaml`.

- **Tools:** `tools/realdata/phasefetch.py` adds two side queries:
  - Tampa with `metadata_rsuid`: 34,464 rows, sha256 `e2099066…`;
  - Wyoming with its log metadata: 6,597 rows, sha256 `10dbee68…`.
  These are separate files, so the pinned manifest does not move.
- **Analysis scripts:** `phaseloss.py`, `e4clock.py`, `stampjit.py`, `kinlag.py` and `e4stampmc.py`. Each one's usage is in its docstring.
- **Sources:** the three DOT datasets `nm7w-nvbm`, `7qu7-j8rx`, `vxmm-9bi5`, and Orem `ev64-jn8c` for contrast. `secMark` is milliseconds within the UTC minute (SAE J2735 DE_DSecond).
- **Where the 100 ms grid comes from:** IEEE Std 1609.4-2016 channel-interval timing. Each 100 ms sync interval starts on the UTC second and splits into a control-channel half and a service-channel half, each opening with a guard interval. The clause number was not re-checked for this section.

### 7.1 Is the Tampa loss phase-locked at every RSU and every hour?

The counting is fitoff.py's (commit `ca0d0ec4`). Only free devices are used, 137 of 226 IDs. Each loss is a msgCnt-attested missing BSM, placed at its interpolated secMark phase. The data are sequenced per (ID, RSU), so a loss is a loss at that RSU. excess = (loss[10,30) − loss[50,100)) / (1 − loss[50,100)). The CI is a 95 % bootstrap over IDs.

| group | IDs | loss 10-30 ms | loss 50-99 ms | diff (95 % CI) | excess |
|---|---|---|---|---|---|
| all RSUs | 137 | 0.515 | 0.228 | +0.287 (+0.234..+0.337) | 0.372 |
| hour 04 UTC | 30 | 0.520 | 0.219 | +0.301 (+0.230..+0.374) | 0.385 |
| hour 05 UTC | 23 | 0.570 | 0.316 | +0.255 (+0.171..+0.364) | 0.372 |
| hour 06 UTC | 84 | 0.507 | 0.217 | +0.290 (+0.216..+0.357) | 0.370 |
| thea24 | 10 | 0.167 | 0.163 | +0.004 (−0.056..+0.089) | 0.005 |
| thea17 | 4 | 0.307 | 0.265 | +0.042 (−0.015..+0.413) | 0.058 |
| thea25 | 6 | 0.410 | 0.292 | +0.118 (+0.089..+0.215) | 0.166 |
| thea39 / thea5 / thea18 | 3 / 2 / 6 | 0.43-0.54 | 0.18-0.34 | +0.21..+0.25 | 0.29-0.31 |
| thea29 / thea10 / thea6 / thea9 | 4 / 3 / 2 / 3 | 0.46-0.55 | 0.12-0.21 | +0.27..+0.41 | 0.34-0.47 |
| thea15 / thea33 / thea4 | 3 / 2 / 2 | 0.61-0.65 | 0.15-0.20 | +0.42..+0.49 | 0.52-0.58 |
| thea1 / thea42 | 11 / 11 | 0.82-0.84 | 0.37-0.46 | +0.36..+0.47 | 0.67-0.75 |

Loss by 10 ms phase decade (all RSUs):

| phase | 0 | 10 | 20 | 30 | 40 | 50 | 60 | 70 | 80 | 90 |
|---|---|---|---|---|---|---|---|---|---|---|
| loss | 0.336 | 0.525 | 0.505 | 0.418 | 0.350 | 0.256 | 0.211 | 0.231 | 0.216 | 0.224 |

**Finding 1.** The lock is the same in all three hours (excess 0.370-0.385), so it is a fixed schedule, not a load effect. It is common to 13 of 15 RSUs at the same UTC phase, which points to a 1609.4-synchronised schedule. Two RSUs (thea24, thea17) show none, and their CIs cover 0.

- So a deployment mixes continuous-receive units with units that leave the channel.
- Among the units that leave, the excess spans 0.17-0.75.
- `rsu-off-channel-tampa-per-rsu.yaml` lists each RSU's p_off, scaled from the pooled fit (0.45). The scaling assumes every RSU shares the pooled timing.
- The per-RSU values from 2-3 IDs are wide. thea1 and thea42 also have the highest rest-of-interval loss, which fits coverage-edge units.

**Wyoming (`7qu7-j8rx`) cannot show this.** All 6,597 rows are the host OBU's own log: `metadata_bsmsource` is EV and `metadata_recordtype` is bsmLogDuringEvent or bsmTx. Every row also comes from one OBU, because every `metadata_logfilename` ends in the same link-local address while 16 temporary IDs rotate. So there is no reception and nothing for an RSU to lose. Every stream is locked, so the 10-30 ms window is empty.

- Outside an event's core the log drops to 1 Hz: 1,520 pairs have msgCnt steps of 10/11.
- Excluding those, msgCnt-attested gaps are 0.016 of slots. The :00 and :50 grids are alike (0.015 / 0.016).
- So the host OBU itself does not skip phase-dependently. Tampa's lock is on the receive side.

**NYC (`vxmm-9bi5`) cannot be put on UTC.** `T_s` is translated per event (normalise.py, per the dataset's DataProcessing.txt), so it carries no UTC phase. What can be formed is the remote-BSM loss by phase relative to the host's own BSM: 254 remote IDs, 1 Hz decimation excluded. In 10 ms bins it is 0.056 / 0.112 / 0.069 / 0.065 / 0.086 / 0.088 / 0.089 / 0.030 / 0.087 / 0.040.

- The [10,30) vs [50,100) difference against the event origin is +0.017, with CI −0.028..+0.061.
- There is no structure beyond noise, and no half-duplex dip at the host's own transmit phase at this 10 ms resolution.
- A NYC OBU receiving on a continuous channel would look like this, but the data cannot tell that apart from a UTC lock.

### 7.2 What is left of P1-E4: the stamp model, not the hops or the header clock

Pairs are resid.py's: secMark dt, judged at 0.5 s, v1 ≥ 10 m/s. Wyoming has 4,946 pairs over 12 IDs. Splits:

| Wyoming split | share | residual p50 / p90 / p99 cm |
|---|---|---|
| secMark dt 100 ms | 0.705 | 1.58 / 5.70 / 15.4 |
| :00/:50 hop (dt 50 or 150) | 0.282 | 1.75 / 6.01 / 15.5 |
| msgCnt-attested loss (dt 200-500) | 0.006 | 5.74 / 20.5 / 29.4 |
| header − secMark dt ≤ 5 ms / 5-20 / 20-50 / > 50 | 0.40 / 0.22 / 0.22 / 0.16 | p90 5.43 / 6.77 / 7.01 / 5.28 |

- **The hops carry the position at the new instant.** Recomputing with dt rounded to 100 ms turns p90 into 174 cm. Dropping all hop pairs moves Wyoming's own distribution by only KS 0.022, so the grid hops are not the gap.
- **The log clock is not the BSM clock.** Binned by header − secMark dt, the residual barely changes. `metadata_generatedat` carries nothing about the position's time.

Along-track residual / v (ms) by secMark-dt offset e = dt − nominal, with msgCnt +1:

| e (ms) | Wyoming share (100 ms) | Wyoming mean (sd) | Tampa locked mean (sd) | Orem locked mean (sd) |
|---|---|---|---|---|
| −1 | 0.195 | +1.02 (2.12) | +2.21 (7.9) | −0.23 (2.7) |
| 0 | 0.598 | −0.17 (0.49) | +0.28 (3.4) | −0.34 (2.5) |
| +1 | 0.207 | −1.20 (0.16) | −0.86 (0.56) | −0.36 (2.4) |

**Finding 2. Wyoming's position sits on the fix epoch.** Only the stamp moves: secMark = epoch + o with o ∈ {−1, 0} ms on both grids (P(o = −1) 0.46). It is a Markov chain: stay −1 0.566, stay 0 0.630.

- A position left on the epoch gives a mean of −e, which is what the table shows. A position at the jittered instant gives about 0.
- At 34 m/s each e = ±1 pair therefore carries exactly 3.4 cm. That is the 0.16 ms sd at e = +1.
- Tampa's locked IDs (a fit set) show the same signature at a lower rate: P(o = −1) 0.10.
- Orem's locked IDs are at the instant, and their −1 never persists (stay −1 0.03).
- The shipped v3 fit pools these behaviours: a normal instant 0.286 / 0.254 ms with the position at the instant (`bsm_timing.rs` "at_instant"). It smears every pair by about v × 0.36 ms, about 1.2 cm, instead of 40 % of pairs exactly ±v × 1 ms and 60 % with none.

**Finding 3. The tail is persistent position steps, tied to 0 → −1 stamp transitions.**

| transition o1 → o2 | Wyoming pairs | Wyoming p50 / p90 / p99 cm | Tampa locked pairs | Tampa p50 / p90 / p99 cm |
|---|---|---|---|---|
| −1 → −1 | 1,282 | 1.02 / 1.76 / 2.81 | 53 | 0.78 / 1.68 / 5.55 |
| 0 → 0 | 1,649 | 1.11 / 2.70 / 11.0 | 1,856 | 0.86 / 6.12 / 23.3 |
| −1 → 0 | 983 | 4.20 / 5.01 / 5.99 | 173 | 1.37 / 3.37 / 3.92 |
| 0 → −1 | 969 | 4.82 / 12.7 / 27.4 | 165 | 7.84 / 19.9 / 49.5 |

- A residual > 5 cm falls on a 0 → −1 transition with P 0.48 at Wyoming and 0.70 at Tampa. Elsewhere it is 0.05 and 0.11.
- The steps are along-track: 90 % have |along| > |cross|. Wyoming's step |along| has p50 6.3 cm and p90 13.0 cm.
- They persist. 0-2 % are followed by an opposite step of half the size, and the lag-1 along correlation is 0.00. They are not spikes.
- A step comes with a change in the reported speed: corr(along, Δv·dt) 0.69 at Wyoming, 0.59 at Tampa, with slopes 6.8 and 4.4 against 0.5 for constant-acceleration motion. That fits a positioning filter that corrects position and velocity together.
- The shipped walk draws its heavy increments at every epoch, independently of the stamp, and leaves speed alone.

**Decomposition** (`e4stampmc.py`, 20,000 synthetic pairs at Wyoming's speeds). The base and step pools both come from Wyoming, so only the stamp model varies:

| stamp model | steps | KS vs Wyoming | p50 / p90 / p99 cm |
|---|---|---|---|
| epoch chain, Wyoming rates | on | **0.078** | 1.84 / 4.71 / 14.8 |
| epoch chain, Wyoming rates | off | 0.150 | 1.61 / 4.09 / 5.2 |
| epoch chain, Tampa-locked rates | on | 0.173 | 1.23 / 4.34 / 14.9 |
| at the instant (shipped v3) | on | 0.208 | 1.52 / 3.76 / 14.1 |
| none | on | 0.274 | 1.12 / 3.26 / 13.9 |

**Verdict on P1-E4.** The remaining KS (0.165 v3, 0.105 with GNSS velocity noise off) is explained by device clock behaviour.

- The cause is the stamp model: the position goes on the epoch and the stamp offset follows a {−1, 0} chain, against the instant model's continuous smear.
- Then comes the device's offset rate: Wyoming's OBU is early on 46 % of BSMs, Tampa's locked units on 10 %.
- The grid hops and the header clock do not explain it.
- With both in, a synthetic body lands at KS 0.078, under the 0.092 tolerance.

**Caveat that changes the reading of P1-E4.** Wyoming is one OBU, so the 0.092 tolerance is a between-temp-ID spread of a single device, not a between-device spread. Matching it with the Wyoming chain rates would put the held-out set in-sample on two parameters.

- Realistic, out-of-sample form: give the engine device profiles with the epoch-chain mechanism, using `bsm-stamp-epoch-candidates.yaml`'s Tampa-locked rates as the fit.
- Report P1-E4 against Wyoming with the chain rate as the one stated device parameter: in-sample on that rate, out-of-sample on everything else.

**Expected, written before any engine run, for the engine owner:**
- epoch-chain stamps at Tampa rates, with the v3 walk unchanged: KS between 0.15 and 0.19;
- with Wyoming rates: KS ≤ 0.10;
- with steps also moved onto 0 → −1 transitions: KS ≤ 0.092.

### 7.3 Side finding: Wyoming's reported speed lags its position

`kinlag.py` compares the reported speed with the position-derived speed (1 s centred window) at shifts −0.5..+2 s.

- At Wyoming the best shift is 0.475 s on 6 of 8 IDs. All 8 are the same OBU. The rms difference drops from 0.120 to 0.096 m/s.
- The heading shift is 0.075 s.
- At Tampa the best shift is 0.05 s: the rms difference is 0.75 m/s, too noisy to read.
- Taking the speed 0.475 s later does not shrink the RandomPosition residual (p90 5.82 → 5.84 cm), so it is not part of P1-E4.
- It does matter for speed-consistency detectors (MaxAcceleration, the speed-vs-displacement checks).
- It is one device, so it is listed as an observation, not a calibration.

### 7.4 The stamp chain in the engine (2026-10-27)

The model is `v2xw_node::bsm_timing::StampChain`, selected by `actors.vehicles.bsm_timing.stamp_chain`:

```yaml
stamp_chain:
  enabled: true
  profile: tampa_locked   # the default; or wyoming_obu
  steps: true             # the default
  stay_minus1: ...        # optional overrides
  stay_zero: ...
```

- It applies to the locked class only: the position stays on the fix epoch and secMark is set to epoch + o, with o in {−1, 0} ms.
- A 0 → −1 transition can put a lasting along/cross step onto the v3 walk. Its probability is (p_on − p_else)/(1 − p_else), and its size is lognormal through the profile's p50/p90.
- The chain has its own RNG domain, so the device's other draws do not move.
- The chain is off unless enabled, so no golden moves. With it off, the record digest matches the 3cbc95a4 release binary.
- The rows are in `crates/v2xw-engine/tests/expectations/p1-stamp.yaml`: 30 rows, 26 pass, 3 are wrong expectations and 1 is open. All 26 passing rows go red under `variants_faults.py`.

Runs: E4 highway (`p1-e4-u.yaml`), the same seed for all, v p50 29.8 m/s, 10,457 pairs. Wyoming is p50 1.64, p75 4.11, p90 5.99, p99 15.7 cm.

| run | stamp | KS (at) | p50 / p75 / p90 / p99 cm | early share, stays | >5 cm on 0→−1 / other |
|---|---|---|---|---|---|
| SB (E4V, 1.3.0 fit) | instant | 0.150 (3.1 cm) | 1.50 / 2.70 / 7.0 / 23.5 | 0.128, 0.12 / 0.87 | 0.15 / 0.14 |
| CT | chain, Tampa, no steps | 0.130 (2.6 cm) | 1.28 / 2.93 / 6.9 / 23.5 | 0.097, 0.25 / 0.92 | 0.12 / 0.14 |
| CTS | chain, Tampa, steps | **0.118** | 1.29 / 3.29 / 9.4 / 25.3 | same | 0.66 / 0.14 |
| CW | chain, Wyoming, no steps | **0.062** | 1.85 / 3.76 / 7.2 / 23.3 | 0.462, 0.57 / 0.63 | 0.14 / 0.15 |
| CWS | chain, Wyoming, steps | **0.074** | 1.88 / 4.30 / 9.6 / 24.7 | same | 0.43 / 0.15 |

**Against the prediction in §7.2 (written before the runs).**
- Tampa came out at 0.130, against the predicted 0.15–0.19. The prediction was a wrong expectation: it came from e4stampmc on Wyoming's own base pool.
  - The engine's KS sits in the 3 cm shoulder, which the chain's ±v·1 ms pairs feed.
  - So the out-of-sample Tampa form improves on the instant model, 0.150 → 0.118 with steps, but still misses 0.092.
- Wyoming came out at ≤ 0.10 as predicted, at 0.062 (0.074 with steps). It is in sample on the two chain rates, and with steps on the step rate and size too.

**Open.**
- The steps make the tail heavier than Wyoming's: p90 9.4–9.6 cm against 6.0. The v3 walk (Tampa + Orem) already has p99 23 cm against 15.7.
- The step rate assumes every step clears 5 cm, so the share of 0 → −1 pairs over 5 cm comes out short:
  - CWS gives 0.43 against Wyoming's 0.48;
  - CTS gives 0.66 against Tampa's 0.70.
  - The fix is a joint fit in tools/realdata.
- The coupled speed change (corr 0.6–0.7) is not modelled.
- Which profile becomes the default is the owner's choice.

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

Written to `data/calibration/bsm-timing-cvpilot.yaml` (schema v2xw.bsm_timing.v3).

| Class | IDs | single-interval moving pairs at risk (v ≥ 5 m/s) | of them v ≥ 10 m/s | slip onsets (v ≥ 10 m/s) | frozen-repeat onsets |
|---|---|---|---|---|---|
| locked | 45 | 4031 | 2275 | 0 | 0 |
| mixed | 3 | 4 | 0 | 0 | 0 |
| free | 2754 | 562740 | 451785 | 102868 | 87609 |

```yaml
# BSM device-timing model fitted on public CV-pilot BSMs (P1-E1/P1-E2).
# GENERATED by `python3 tools/realdata/rdtool.py all` - do not edit by hand.
# Derived statistics only. Shares are over locked+free IDs of the fit sets; 3 mixed and
# unclassified IDs are left out of the shares. Phases are secMark mod 100 ms (fix epochs
# assumed on the 100 ms grid); locked devices sit on either the :00 or the :50 grid, so the
# locked phase is the distance to the nearest 50 ms epoch.
# Tampa: 41 locked, 137 free IDs (locked share 0.230); the share is deployment-specific.
# Orem: 4 locked, 2617 free IDs (locked share 0.002); the share is deployment-specific.
schema: v2xw.bsm_timing.v3
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
  tool: tools/realdata 1.3.0
fix_interval_ms: 100
classes:
  - name: locked
    share: 0.016077
    # the 100.0% of locked BSMs within 5 ms of a 50 ms epoch; whole-ms offsets -1, +0, +1, +2 with
    # shares 0.130608, 0.866902, 0.00246, 3e-05. Mean along-track residual / v by secMark Δt - 100 ms (v >= 10 m/s):
    # -1 ms: 0.41 ms (439), +0 ms: 0.11 ms (1961), +1 ms: -0.50 ms (456)
    # That is the 1 ms truncation of a generation instant that jitters about the epoch, with
    # the position at that instant (+-0.5 ms expected; +-1 ms if the position stayed on the
    # epoch). The instant is the normal whose truncation gives the shares of -1 and >= +1:
    secmark_phase_ms: {dist: normal, mean: 0.285749, sd: 0.254333}
    phase_scope: message
    # secMark Δt minus 100 ms on single-interval pairs: p10 -1, p50 0, p90 1 ms
    # slip onsets per single-interval pair at v >= 10 m/s after a normal pair (Δt-corrected
    # displacement): 2275 at risk; frozen repeats below are per pair at v >= 5 m/s, 4031 at risk
    # 0 onsets, of which 0 return at the next pair (a one-message slip, counted here); the
    # 0 that do not return are persistent phase steps, which bsm_clock reproduces
    epoch_slip_prob: 0.0
    # no slips observed in this class; slip sizes are the pooled locked+free ones
    slip_epochs: {"-1": 0.00502586, "1": 0.95677, "2": 0.0382043}
    frozen_repeat_prob: 0.0
    # from 6579 Wyoming BSMs (the only source with a header time; its class and rate results
    # stay held out). Header minus secMark also has a mean of 38.7 ms, and it is
    # persistent: it changes in steps of about 50 ms rather than per message. PROVISIONAL
    # until P1-E3: if metadata_generatedat is ODE log time, not the signed 1609.2
    # generationTime, this is not a header jitter.
    header_jitter_ms: {sd: 28.2875}
    # receiver output noise, white, bounded by the change of the transmitted speed and heading
    # between consecutive exactly-100 ms BSMs of the locked class at v >= 10 m/s (1961 pairs):
    # change sd 0.0445 m/s and 0.0741 deg (IQR/1.349), so noise <= change/sqrt(2). The v2 fit
    # read the whole residual body (along 0.0111 m, cross 0.0089 m) as this noise
    output_noise: {speed_mps: 0.0314503, heading_deg: 0.0524171}
    # the rest of the body and its tail: the reported position's error increment per fix
    # interval, a Gaussian scale mixture (EM, 4 components, log-lik 10712.6) on 1961 exactly-100 ms
    # pairs of the locked class at v >= 10 m/s with their own speed and heading change removed
    # and the quantisation, the stamp jitter (0.286 ms x v) and the output noise above
    # deconvolved. Increments are white (lag-1 ~ 0), so the error is a random walk over the
    # fix epochs, mean-reverting with tau_s: the shortest tau (of 1/2/5/10/20/50/100 s, capped at the receiver's
    # 100 s smoothing) whose simulated exactly-100 ms residual matches these pairs, each ID
    # weighing one, within KS 0.01 of the best; KS by tau: 1 s 0.275, 2 s 0.219, 5 s 0.154, 10 s 0.117, 20 s 0.094, 50 s 0.083, 100 s 0.079
    position_noise: {dist: normal_mixture, tau_s: 50.0, p: [0.633315, 0.169124, 0.113336, 0.084225], along_m: [0.0001, 0.0001, 0.0518646, 0.132925], cross_m: [0.00323245, 0.0118151, 0.0309238, 0.0982351]}
    # :00/:50 grid hops: consecutive-msgCnt BSMs (Δt 50/100/150 ms) home->home 6552, home->away 836,
    # away->home 831, away->away 1036; a hop is Δt 50 then 150 ms (or 150 then 50), the position
    # at the new instant. The engine puts the away grid half an interval late
    grid_hop: {leave_prob: 0.113156, return_prob: 0.445099}
  - name: free
    share: 0.983923
    # bins_ms are the 11 edges of 10 bins; p[i] is the share in [bins_ms[i], bins_ms[i+1])
    secmark_phase_ms: {dist: empirical, bins_ms: [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100], p: [0.088566, 0.085286, 0.096363, 0.107481, 0.121526, 0.109662, 0.095202, 0.096257, 0.09905, 0.100607]}
    # secMark Δt minus 100 ms on single-interval pairs: p10 0, p50 6, p90 36 ms
    # the free BSM period exceeds the fix interval, so the phase drifts across the fix grid:
    # the phase above is the device's first; bsm_clock moves it. drift_ms is the per-ID
    # median of secMark Δt - 100 ms over consecutive-msgCnt single-interval pairs (2460 IDs
    # with >= 20 pairs), period_dev_ms each BSM's period minus its ID's median; its tail
    # beyond +-25 ms (15.0% of 3263437 periods) is the clock's resets
    bsm_clock:
      drift_ms: {dist: empirical, bins_ms: [-10, -8, -6, -4, -2, 0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 42, 44, 46, 48, 50], p: [0.0, 0.0, 0.0, 0.0, 0.0, 0.00122, 0.049593, 0.202033, 0.503249, 0.202033, 0.026423, 0.006911, 0.004065, 0.002033, 0.001626, 0.000407, 0.000407, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]}
      period_dev_ms: {dist: empirical, bins_ms: [-50, -45, -40, -35, -30, -25, -20, -15, -10, -5, 0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50], p: [0.006313, 0.002945, 0.002925, 0.003277, 0.002669, 0.000392, 0.000491, 0.001552, 0.136333, 0.320874, 0.190404, 0.079076, 0.042915, 0.035212, 0.035461, 0.03743, 0.03894, 0.038102, 0.024172, 0.000517]}
    # slip onsets per single-interval pair at v >= 10 m/s after a normal pair (Δt-corrected
    # displacement): 451785 at risk; frozen repeats below are per pair at v >= 5 m/s, 562740 at risk
    # 102868 onsets, of which 25548 return at the next pair (a one-message slip, counted here); the
    # 77320 that do not return are persistent phase steps, which bsm_clock reproduces
    epoch_slip_prob: 0.056549
    slip_epochs: {"-1": 0.00943322, "1": 0.990567, "2": 0.0}
    frozen_repeat_prob: 0.155683
    # no free-class source has a header time: borrowed from the locked Wyoming fit
    header_jitter_ms: {sd: 28.2875}
    # receiver output noise, white, bounded by the change of the transmitted speed and heading
    # between consecutive exactly-100 ms BSMs of the locked class (borrowed) at v >= 10 m/s (1961 pairs):
    # change sd 0.0445 m/s and 0.0741 deg (IQR/1.349), so noise <= change/sqrt(2). The v2 fit
    # read the whole residual body (along 0.0111 m, cross 0.0089 m) as this noise
    output_noise: {speed_mps: 0.0314503, heading_deg: 0.0524171}
    # the rest of the body and its tail: the reported position's error increment per fix
    # interval, a Gaussian scale mixture (EM, 4 components, log-lik 10712.6) on 1961 exactly-100 ms
    # pairs of the locked class (borrowed) at v >= 10 m/s with their own speed and heading change removed
    # and the quantisation, the stamp jitter (0.286 ms x v) and the output noise above
    # deconvolved. Increments are white (lag-1 ~ 0), so the error is a random walk over the
    # fix epochs, mean-reverting with tau_s: the shortest tau (of 1/2/5/10/20/50/100 s, capped at the receiver's
    # 100 s smoothing) whose simulated exactly-100 ms residual matches these pairs, each ID
    # weighing one, within KS 0.01 of the best; KS by tau: 1 s 0.275, 2 s 0.219, 5 s 0.154, 10 s 0.117, 20 s 0.094, 50 s 0.083, 100 s 0.079
    position_noise: {dist: normal_mixture, tau_s: 50.0, p: [0.633315, 0.169124, 0.113336, 0.084225], along_m: [0.0001, 0.0001, 0.0518646, 0.132925], cross_m: [0.00323245, 0.0118151, 0.0309238, 0.0982351]}
```

<!-- END GENERATED -->
