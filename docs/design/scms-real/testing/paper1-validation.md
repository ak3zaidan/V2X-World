# Paper 1 features validated with real runs, and P1-E4

Date 2026-10-24. Base `e15bfec1`, using the integrator's release binary. Expectations are in
`crates/v2xw-engine/tests/expectations/scms-paper1.yaml`. Each row was committed with
`measured: pending` before the run it judges (`2de1b397`, `b37e84c9`, `ed2bd523`, `c9b739a3`
and `b50d761c`). The measured values and verdicts were filled in by `574c7d9e`. Scenarios
are in `scenarios/real-world/paper1/`. The analysis scripts are in `tools/realworld/paper1/`.

## 0. Verdict

The table has 73 rows: 55 pass, 4 fixed, 4 wrong expectations and 10 open. Fault proof
(`tools/realworld/variants_faults.py`): 58 of the 59 passing rows go red when a fault is
injected. The one that stays silent (a ratio over a zero base) went red on the real fault
(see §3).

- **What works as theory says:**
  - **The timing classes.** RandomPosition triggers scale with the free share: 7,233 / 5,097 / 4,452 / 199 at locked shares 0 / 0.25 / 0.5 / 1.
  - **The store.** P1-E6 nulls: an attack-free run loses 0 of 0 attack reports, and a zero quota loses 258 of 258. The 05:00 ET decay acts only inside the window.
  - **VeReMi.** The round trip is exact: 25,532 lines, 0 mismatches. An independent leak check finds 0 leaks, and an injected leak is caught.
  - **BeaconFrequency (WP-02).** An honest 10 Hz fleet raises 0 events, and the shortest honest received gap is 95 ms, above the 75 ms threshold. A 20 Hz injection fires.
  - **Determinism.** Same seed and 1 vs 2 threads give byte-identical runs.
- **Fixed (2 defects, each with a test that is red without the fix):**
  - **F1.** The real-data slip estimator turned the locked class's :00/:50 grid hops into epoch slips. That gave an all-locked fleet 5.5 triggers per ID-minute at speed and 35 honest devices blocked by sound MA rules. After the fix and refit these are 0.0 and 1.
  - **F2.** The VeReMi export at `--record-level receptions` labelled every attacker message genuine and put every true position at (0, 0).
- **P1-E4 (Wyoming KS) is red.**
  - Pre-registered tolerance: KS ≤ 0.092. Measured 0.355, and 0.337 after the refit.
  - **The tail now matches:** 0 triggers, and a locked rate of 0.0 against the 0.568 bound.
  - **The residual body is too tight:** engine p90 2.3 cm against Wyoming 6.0 cm.
  - **The fault proof works.** On the old white-noise GNSS model the same row reads 0.99.
  - **The run was not speed-matched:** the highway reached a median of 22 m/s against Wyoming's 34 m/s.
- **Open, timing model (TM-1).** A k > 0 slip moves secMark back instead of the position forward. As a result:
  - generationTime-based Δt trips less often than secMark-based Δt, while real data shows the opposite;
  - 17 % of a free device's consecutive BSMs share a secMark.

## 1. Method

- **Runs.** The binary is the release build of `e15bfec1`, copied to the scratchpad. Every run used the scenario's seed and `--no-verify`, went through the run lock one at a time (2,500 MB cap, 2 rayon threads) and was launched from the repository root (the calibration path is relative to it).
- **Measures.**
  - `matrix.py extract` gives the run report, the PKI view and the cost.
  - `tools/realworld/paper1/summ.py` adds the `x.*` measures from the run's jsonl and VeReMi output:
    - report fates (`pki.report_fate`, final fate per report);
    - `ma.received`;
    - `ma-replay.csv`;
    - the VeReMi round trip against `phy.rx` and `node.tx`;
    - an independent leak check;
    - the TS 103 759 BeaconFrequency rule replayed on the received VeReMi lines (75 ms, 2 in a row);
    - the E4 residuals.
- **E4 residuals.** These are recomputed from the transmitted BSMs (`node.tx` J2735 fields) by tools/realdata's own stages: `pairing.py` and `mbrchecks.py`, the code that scored Wyoming. The residual is the v1.0 RandomPosition residual on secMark Δt, for pairs judged at the 0.5 s limit with v₁ ≥ 10 m/s, which is the headline cell of `p1-realdata-baseline.md`.
- **The KS tolerance** was set before any E4 run. It is the 95th percentile of the KS distance between two halves of Wyoming's 12 IDs over all 462 splits: 0.092 (`ks.py tol`). An engine fleet must be no further from Wyoming than one half of Wyoming's own devices is from the other.
- **Disclosure.** One 60 s probe of the shipped `bsm-timing.yaml` (Midtown, built-in calibration) ran before the tolerance was written, to learn the output format. Its residual body was seen. No E4 (highway) number was seen before the tolerance was written.
- **A stated assumption (P1-E6 and P1-E7 runs).** The RA's report shuffle releases at 10,000 reports or after one day (CAMP SCMS-765), so in a 120 s run no report would reach an MA. Base B showed exactly that: 1,731 uploaded, 0 delivered. The attack and MA-replay scenarios therefore set `report_shuffle_window_s: 10`, and the 06 §9 owner decision "MA timing studies" applies. With this setting, A delivers 1,639 reports.

## 2. Results

Change, expectation (theory and source in the row), measured value and verdict. B is the
honest 120 s Midtown run with the CV-pilot calibration at locked share 0.25 and the US profile
on secMark. A is B with 3 % ConstPos attackers from 20 s.

| Change | Expected | Measured | Verdict |
|---|---|---|---|
| locked share 0 (S0) vs B | RP triggers ×1.1–1.8 (free 24 vs locked 0.17 per ID-min) | 5,097 → 7,233 (×1.42) | pass |
| locked share 0.5 | ×0.45–0.9 | ×0.87 | pass |
| locked share 1 | ×0–0.1 | ×0.039 (199) | pass |
| share 0: ConstantPosition, reports | up (free frozen repeats 0.156) | CP 6,512 → 8,956; reports 3,794 → 5,186 | pass |
| share 1: ConstantPosition vs timing off | = OFF (locked frozen 0) | 0 → 76 | **fixed** (F1; refit S100R 0) |
| share 0: frames / events / memory / wall | same / ×0.9–1.15 / ×0.8–1.25 / ×0.6–1.6 | 41,781 → 41,844 / ×1.003 / ×1.05 / ×0.94 | pass |
| share 0: honest revocations, CRLs, attack reports, separation | 0 (8 h MA review) / same / 0 / none | 0 / 0 → 0 / 0 / [] | pass (the separation row's type was corrected) |
| rerun; 1 thread | identical digest | identical (`40a5faf2…`) | pass |
| gap 0.1 vs 0.2 s (v1.0 sm, v1.0 gt, v1.1 gt) | ratio ≥ 0.5 (T3: rate flat) | 0.46 / 0.44 / 0.46 | **wrong expectation** (§4) |
| gap 0.2 vs 0.5 s (4 combinations); v1.1 sm 0.1 vs 0.2 | 0.7–1.0; 0.5–1.0 | 0.93–1.0; 0.997 | pass |
| ASN.1 v1.1 vs v1.0 (sm; 0.1, 0.2, 0.5 s) | ×0.3–1.02 (subset of pairs) | ×1.008 / 0.46 / 0.43 | pass |
| generationTime vs secMark (v1.0; 0.1, 0.2, 0.5 s) | up (header jitter 28 ms; Wyoming 0 → 214) | 2,164 → 1,731; 4,722 → 3,941; 5,097 → 4,179 | **open** TM-1 |
| attack-free: attack evidence lost (P1-E6) | 0 | 0 of 0 (and A: 258 attack reports) | pass |
| zero quota | all lost, 0 delivered, MA empty | 258 / 258 lost; 0; 0 (A: 1,639 delivered) | pass |
| quota 4,500 B, 2,300 B, 0 vs 64 KiB | evictions up; lost share monotone | evicted 0 → 87; lost 0 → 4.6 % → 5.1 % → 100 % | pass |
| multiplier 0.5, decay at 05:00 ET (outside the window) | store life identical | identical fate signature (and record digest) | pass |
| decay at 18:00 ET inside the window, ×0.5 / ×1 / at 05:00 | DecayedOut ≥ 1 / 0 / 0; ×1 = no decay | 3 / 0 / 0; identical | pass |
| MA replay, all locked (P1-E7 null) | 0 honest blocked by every sound rule | 35 → 1 after refit | **fixed** (F1) / **open** (1 left) |
| MA replay, timing off | 0 | 1 (node 262, 5 × MaxAcceleration) | **open** (06 §6 item 1) |
| MA replay, all free (fault) | ≥ 1 | 114 | pass |
| MA replay: a sound rule revokes an attacker | ≥ 1 | 5 of 10 | pass |
| VeReMi at `receptions`: lint, round trip, leaks, fault | pass / 0 / 0 / caught | pass / 0 of 25,532 / 0 / 525 | pass |
| VeReMi truth at `receptions` | ConstPos coded 1 | all coded 0; 9,035 / 9,035 true positions at (0, 0) | **fixed** (F2) |
| VeReMi truth at `full` | coded 1; no (0, 0) | 3 attackers coded 1; 26 rows at (0, 0) | pass / **open** VR-2 |
| honest 10 Hz, BeaconFrequency (WP-02) | 0 events; min gap ≥ 75 ms; 20 Hz fires | 0; 95 ms; 910 | pass |
| E4 KS vs Wyoming (≤ 0.092) | ≤ 0.092 | 0.355; after refit 0.337 | **open** E4-BODY |
| E4 white-noise GNSS; all-free class | > 0.092 | 0.992; 0.412 | pass (red as required) |
| E4 locked rate at speed | ≤ 0.568 per ID-min | 5.52 → 0.0 after refit | **fixed** (F1) |
| E4 speed match | p50 25–36 m/s | 20.6; 22.2 at 80 mph | **open** E4-SPEED |

**SCMS architecture, from the PKI records.**

- **Entities and links.** Every run has the same entity set and the same links: RA and DCM peers for vehicles, and LOP on the highway. No new channel or entity appears with the timing model.
- **Reports.** Reports leave by cellular and by RSU relay: 1,507 and 320 in A.
- **Delivery to the MA.** Reports reach the MA only through the RA shuffle.
- **CRLs.** CRL counts are unchanged in every 120 s window.
- **Revocation.** The 8 h review means that nothing is revoked inside a window.
- **What the MA decided.**
  - In A (75 % free), the engine's own corroborated MA opened 24 cases and decided to revoke 21 honest devices before review. With every OBU locked (MAL1R) it decided 0 honest.
  - In MAL0 (all free) it decided 67 honest revocations.

  This is the Paper 1 effect, measured in the closed loop.

**Performance.**

- **Midtown runs.** 30–65 s of wall time for 120 s simulated, with a peak of 250–330 MB. Event counts are within 1 % across shares.
- **The timing model's cost.** None that can be measured: OFF ran in 30.5 s and S0 in 40.9 s, a difference explained by detection and reports.
- **Quarter-fleet MA runs.** 113–174 s and 190–330 MB.
- **Highway minute.** 10–276 s, depending on lock contention from other agents. The longer wall times (Q4 at 360 s, the E4 runs) coincide with other agents' runs. The event counts are what to read.

## 3. Fixes

**F1. The tools/realdata slip estimator, with a refit of the calibration (`a77aab7c`, `df6d7a86`).**

- **The defect.** `pipeline.slip_size` rounded secMark Δt to whole fix intervals. When a locked device hops between the :00 and :50 grids, Δt becomes 0.149 s while the position stays consistent with Δt, and that hop was counted as a +1 position-epoch slip. Example: 1.474 m at a reported 9.26 m/s is 1.59 intervals, so round(1.49) − round(1.59) = −1.
- **Evidence from Tampa's locked IDs.**
  - All 9 "slips" at v ≥ 10 m/s had residuals of 0.03–0.35 m, and none fired RandomPosition.
  - The free class's 154 slips at v ≥ 10 m/s are real: 147 fire RandomPosition.
- **The fix.**
  - A slip is now the displacement beyond v̄·Δt, measured in fix intervals.
  - The slip probability is fitted at v ≥ 10 m/s, where a one-epoch slip is at least gpsDrift. Below that, locked devices show sub-metre near-stale positions that a speed-independent probability would carry to highway speed.
  - Frozen repeats keep the v ≥ 5 m/s denominator.
- **Tests.** Two new tests in `test_realdata` fail without the fix: a grid hop is not a slip, and slips are counted at speed only. The suite passes 28 of 28.
- **The refit** (`rdtool.py all` on the manifest-verified raw rows):
  - locked `epoch_slip_prob` went from 0.0067 to 0 (0 of 2,275 fast pairs);
  - free went from 0.227 to 0.228;
  - the frozen classifier, the Wyoming score and every rate table are unchanged.
- **Engine effect.**
  - All-locked ConstantPosition: 76 → 0.
  - E4 locked rate: 5.5 → 0.0 per ID-minute.
  - Honest devices blocked by sound rules on the quarter fleet: 35 → 1.
- **Goldens.** None move: the calibration file is read only when a scenario names it.

**F2. The VeReMi export refuses a recording without `gt.kinematics` (`8b5b1843`).**

- **The defect.** `from_records_with` joins the truth state and the node-to-actor map, which attack labels are keyed by, from `gt.kinematics`. Only `--record-level full` records that channel. At `receptions` the export silently wrote all true positions as (0, 0) and coded every attacker message 0.
- **The fix.**
  - The export now errors when a recording has receptions but no `gt.kinematics`.
  - `v2xw run` refuses `--veremi` below `full` before it simulates.
- **Test.** `dataset_profiles::a_recording_without_kinematics_is_refused_rather_than_labelled_genuine` is red without the fix. v2xw-record passes 254 of 254, and `cargo check -p v2xw-cli` passes.
- **The CLI refusal** is compiled but not run as a binary: no debug binary was built in this batch.

## 4. Wrong expectations, corrected with evidence

- **`P1.TS.separation`.** `separation.violations` is a list, not a count. The row now expects it to be empty.
- **The three gap 0.1 vs 0.2 s rows.** I compared counts against a real-data result that is a *rate per judged ID-minute* from logs that lose almost nothing.
  - **Loss pairs.** An engine receiver loses BSMs and forms pairs with Δt ≈ 0.2 s. The v1.1 cells, which judge only pairs 0.1 s apart, are flat at 2,181 and 2,188, so the extra v1.0 triggers at 0.2 s are those loss pairs.
  - **The gap limit.** The engine judges Δt ≤ `max_gap_s` exactly, while tools/realdata's "≤ 0.1 s" cell is Δt < 0.15 s (`judged()`, GAP_TOL 0.05). Under generationTime jitter about half of the single-interval pairs have Δt above 0.100 s. **To reproduce a real-data cell L, set `max_gap_s` = L + 0.05.**
  - **The corrected rows** check only the subset property (ratio ≤ 1).

## 5. Open items

- **TM-1 (engine, `bsm_timing.rs`): slip semantics.** Fixed in the engine by the v2 BSM clock (§7); the re-run rows are `P1.TM1.*`.
  - **What the engine does.** A k > 0 slip keeps the position and stamps the timestamp k epochs older. Two consecutive BSMs then carry the same secMark: in the E4F free class, secMark Δt is 0 ms in 17 % of consecutive pairs, 200 ms in 17 %, and 100 ms in the rest.
  - **The consequence.** The generationTime clock follows the device's own time, so it never sees the slip. generationTime-based Δt therefore trips less often than secMark-based Δt (4,179 vs 5,097).
  - **What real data shows.** Free devices advance secMark by about 100–136 ms per BSM (p50 +6 ms), and the position jumps two fix epochs (Tampa T6: 46 % of triggers are "2 v Δt"). The rx and header bases trip more than secMark: Tampa free 45 vs 20 per ID-minute, Wyoming 214 vs 0.
  - **The fix.** A drifting per-device BSM clock, which the calibration file's own comment asks for: a BSM period above the fix period, so the phase wraps. This is a model change with goldens behind the existing flag. It is not a patch.
  - **TM-2.** A k < 0 slip repeats the previous fix exactly, so ConstantPosition fires as well. It is moot for the locked class after F1, and the free class's −1 share is 0.005.
- **E4-BODY.** The v2 fit adds the locked instant jitter and the receiver output noise (§7); the re-run rows are `P1.E4S.*`.
  - **The gap.** The engine's residual body is too tight: p50 / p90 / p99 are 1.2 / 2.3 / 4.4 cm, against Wyoming's 1.6 / 6.0 / 15.7 cm. This is the epoch-to-epoch output noise of the smoothed receiver, about one J2735 position LSB.
  - **Effect on the paper.** It does not move the 1 m decision: the tail matches, with 0 triggers. It does keep the pre-registered KS row red.
  - **Next step.** Calibrate the receiver's output noise on the Tampa and Orem locked bodies (Tampa locked p50 0.013 m, p99 0.40 m), never on Wyoming, then re-run E4 once against the same tolerance.
- **E4-SPEED.** `p1-e4-s.yaml` (6 km straights, 90 s; §7). The procedural 2×2 highway stays at a median of 20–22 m/s, even posted at 80 mph with half the demand, against Wyoming's 34.4 m/s. This is probably the corner slow-downs or the desired-speed spread. A longer straight segment is needed before the KS comparison is speed-matched.
- **One honest blocked device (06 §6 item 1).**
  - **What happened.** Node 262 is accused 5 times, all by MaxAcceleration (implied |Δv|/Δt > 10 m/s²), with the timing model on or off. Five reporters are enough for k ≤ 5.
  - **What still blocks no honest device:** rules with k ≥ 10, device-k with k = 20, and CaTch with θ = 10.
  - **Conclusion.** P1-E7's null holds for the timing model and fails on the engine's honest braking and speed-snap triggers. That is the known open item, not a new one.
  - **Update 2026-10-24 (backend residuals): an engine defect, already fixed.** The trigger is a one-step stop: node 262 closes on a standing vehicle and the advance cap takes it 0.93 → 0.08 m/s in 0.1 s (ground-truth `gt.kinematics`, −8.5 m/s²), which its BSMs show as an implied deceleration over 10 m/s². That is the honest-density §2 snap, fixed by `traffic.standing_leader_stop` (`0588ae6c`, which these scenarios inherit from the benchmark since the density merge). On `8becf72a`: MALOFF draws 0 MaxAcceleration reports and blocks 0 honest devices under all 37 rules (4 of 10 attackers revoked); the same run with `standing_leader_stop` and `lane_bend_speed` off brings back node 262's 5 reports from 5 reporters (the injected fault for `P1.MA.off-sound-zero`).
- **VR-2.** 26 of 9,035 truth rows are still at (0, 0) at `full`. Each comes from a vehicle's first BSM, which is sent (for example at 9.781 s) before its first `gt.kinematics` record (at 9.9 s). The fix is a kinematics record at spawn (engine) or a bounded forward join (exporter).
- **WP-02 in the engine.** The TS 103 759 suite still cannot be selected in `detection.local`. Its BeaconFrequency rule was therefore replayed on the run's received VeReMi lines, with the engine's own `BSM_J2945_1` values, rather than run in the engine.
- **The free class is fitted mostly on Orem** (2,617 of 2,754 IDs). The engine's all-free highway reaches 204 RandomPosition triggers per ID-minute, close to Orem's 189 and ten times Tampa's 20. The share is a scenario knob, and the deployment's free-device mix is not observable from public data.

## 6. Reproduce

```
# scenario files: scenarios/real-world/paper1/*.yaml (run from the repository root)
v2xw run scenarios/real-world/paper1/p1-base.yaml -o runs/B --record-level events --no-verify
v2xw ma-replay runs/MAL1                     # MA runs only
V2XW_P1_WYOMING=<raw>/wyoming.csv python3 -I tools/realworld/paper1/summ.py runs/E4 --e4
python3 -I tools/realworld/paper1/summ.py runs/VRF --veremi    # VeReMi runs: --record-level full
python3 tools/realworld/matrix.py check crates/v2xw-engine/tests/expectations/scms-paper1.yaml runs
python3 tools/realworld/variants_faults.py crates/v2xw-engine/tests/expectations/scms-paper1.yaml runs
```

The run list with flags is in the table's `runs` section. VR was cut to 60 s at
`receptions` (F2 now refuses that combination). VRF is the same run at `full`.

## 7. TM-1, E4-BODY and E4-SPEED: the v2 timing model and the P1-E4 re-run

Commits `cca82fcd` (model, fit, tests) and `47467608` (rows written with `measured: pending`
before any run). Binary: an optimised debug build of `cca82fcd`.

**What the raw rows show** (scratch analysis of the manifest-verified CV-pilot rows, then
`tools/realdata` 1.1.0):

- **Free devices run their own BSM clock.** Tampa free IDs advance secMark by a steady
  per-ID 103 ms (per-ID median excess 3 ms, MAD 1 ms), and the RSU receive time advances
  with it, so the period itself exceeds the fix interval. At secMark Δt 100-105 ms, 2.3-3.4 %
  of fast pairs move two fixes, about 1/33, the wrap rate a 3 ms excess gives. Orem's
  per-ID median is 6 ms, with a heavy per-BSM tail (15 % of periods more than ±25 ms from
  the ID's median).
- **Slips and wraps are told apart by the next pair.** Of 102,868 free onsets, 25,548
  return at the next pair (a one-message slip: "2 v Δt then ≈ 0"); the other 77,320 stay
  (a clock wrap). v2 fits `epoch_slip_prob` on the returning ones only (0.0565, was 0.228)
  and leaves the rest to the clock.
- **Locked secMark jitter is a timing error.** At secMark Δt 99 / 100 / 101 ms the mean
  along-track residual / v is +0.41 / +0.11 / −0.50 ms (Tampa + Orem locked, v ≥ 10 m/s).
  That is the 1 ms truncation of a generation instant that jitters about the epoch with the
  position at that instant (±0.52 ms predicted), not a position left on the epoch (±1 ms).
  Fit: N(0.225, 0.275) ms, from the shares of −1 (20.7 %) and ≥ +1 (0.24 %).
- **The rest of the body is velocity-domain noise.** Consecutive residuals are uncorrelated
  (lag-1 −0.02 along, +0.03 cross on Tampa; white position noise would give −0.5). On
  exactly-100 ms locked pairs (1,961, Tampa + Orem) the along sd is 1.11 cm and the cross
  sd 0.89 cm (IQR/1.349) over a 0.42 cm quantisation: speed noise 0.103 m/s, heading
  noise 0.30° (cross attributed wholly to heading; an assumption). Wyoming was not used.

**The engine (`crates/v2xw-node/src/bsm_timing.rs`, schema `v2xw.bsm_timing.v2`):**

- `bsm_clock`: period = fix interval + per-device `drift_ms` + per-BSM `period_dev_ms`;
  secMark and generationTime are the tick; the position is the latest fix before it; a
  100 ms wake with no tick sends nothing (msgCnt stays contiguous); the BSM leaves at its
  tick (signing starts there).
- TM-1 defect fix: a slip `k > 0` moves the position k fixes later and keeps the timestamp.
- A per-message phase puts the position at the jittered instant.
- `output_noise` perturbs the transmitted speed and heading only; receivers' detectors see
  it (the frame's claimed speed and heading now come from what the BSM carries).
- v1 files still read (`bsm-timing-cvpilot-v1.yaml` is the frozen v1 fit); v1 refuses
  the v2 blocks. No golden moves: the model runs only when a scenario enables it.
- **E4-SPEED:** `p1-e4-s.yaml` is `p1-e4-80.yaml` with 6 km straights and 90 s.

**Results (one run measured; the run lock was held by other agents for about 20 minutes per
run, so the other seven runs are queued, not measured, and their rows stay `pending`).**

| Change | Expected (written before) | Measured (E4S) | Verdict |
|---|---|---|---|
| P1-E4 KS vs Wyoming, speed-matched, v2 fit | ≤ 0.092 (predicted: still above) | 0.169 (was 0.337) | **open** E4-BODY, narrowed |
| residual p50 / p90 / p99 | about 2.2 / 4 cm predicted | 2.1 / 4.1 / 6.1 cm (Wyoming 1.6 / 6.0 / 15.7) | as predicted |
| speed p50 | 25-36 m/s | 29.8 m/s (was 22.2) | pass (E4-SPEED closed) |
| locked rate at speed | ≤ 0.568 per ID-minute | 0.0 (0 of 10,449 pairs) | pass |
| honest revocations | 0 | 0 | pass |
| secMark steps (locked) | none expected to repeat | 13,434, none repeated | — |
| cost | — | 90 s simulated in 22.6 s wall; 239,272 events; 13,469 frames | — |

**What is left of E4-BODY.** The body centre now matches to within 0.5 cm and the 1 m
tail matches (0 triggers). Wyoming's tail between 5 and 16 cm is missing: the fitted noise
is Gaussian, while the real residual is heavy-tailed (Tampa locked along rms 5.7 cm
against an IQR-based sd of 1.0 cm). The next step is a heavy-tailed (mixture or Student-t)
output noise fitted on the same Tampa + Orem pairs, then one more E4 run against the same
tolerance. The locked :00/:50 grid hops (28 % of Wyoming pairs at Δt 50 / 150 ms) are not
modelled either.

**Not yet run (rows pending):** P1.TM1.* (BV2, GT05V2, OFFV2, E4SF), P1.E4S.ks-white-noise
(E4SWN), P1.E4S.body-p90, frames, events, memory and reports (E4SV1), P1.E4S.rerun (E4S2).
`tm1/runall.sh` in the scratchpad runs them in order, one run per lock call.

## 8. E4-BODY v3, the hop-speed defect and the 1.3.0 refit

Commits `ff7c9e8e`, `204d5d85` (v3 fit, GNSS velocity-noise knob), `8d3990b7` (rows measured,
E4V1 rows written before its run), `775c9cc0` (tools/realdata 1.3.0, E4V rows written before
its run) and `098c9c3a` (hop-speed fix). Wyoming was never used to fit.

**Queued §7 rows, now measured** (486c8cba release binary): P1.TM1.* all pass (positionJump
5,318 -> 5,935 at gt 0.5 s; free secMark never repeats, p50 excess 7 ms, 254 triggers per
ID-minute; frames ratio 0.928). P1.E4S.* pass except the KS (0.169, open). P1.MA.l1-attackers 4.

| Change | Expected (written before) | Measured | Verdict |
|---|---|---|---|
| E4T: v3 fit (heavy-tailed walk, hops) vs v2 | KS ≤ 0.092, down 30 % | 0.169 -> 0.165; p50 / p90 / p99 2.26 / 6.3 / 20.5 cm (Wyoming 1.64 / 6.0 / 15.7) | open: tail matches, body too wide |
| E4T reports vs E4TV2 | delta ±5 | 2 -> 43 | **fixed** (hop-speed defect) |
| E4U: GNSS white velocity noise off | KS down 20 %, ≤ 0.092 | 0.165 -> 0.105 at 1.27 cm; p50 1.97 cm | open |
| E4U MaxAcceleration verdicts | down 80 % (blamed on velocity noise) | 48 -> 61 | **fixed**: wrong cause, see below |
| E4U determinism, frames, events, memory | identical; same | identical; 13,469; ratio 1.00; 1.02 | pass |

**Defect: a hop BSM's speed.** A device on the :50 grid stamps and positions its BSM 50 ms
after the belief but sent the belief's speed, so at a hop's 50 ms secMark Δt MaxAcceleration
read twice the deceleration. Fixed in `bsm_timing.rs`; test
`a_hop_bsm_carries_the_speed_of_its_own_instant`. Rows `P1.E4V1.*` re-measure it.

**Fit defect (tools/realdata 1.3.0).** Simulated at the fit pairs' own speeds, the 1.2.0 model
missed its own exactly-100 ms residuals (KS 0.275): the walk's assumed τ 1 s adds (ρ − 1)·e,
about 1 cm per axis, to every pair, while the EM reads each residual as the increment alone.
τ is now fitted in sample (grid 1–100 s, capped at the receiver's 100 s smoothing): 50 s, KS
0.083. Stamp offsets and the position-noise EM weigh each ID once (Orem's 3 IDs held 28 % of
the pairs). Model prediction at 29.8 m/s, written before the run: p10 / p25 / p50 / p75 /
p90 / p99 0.36 / 0.82 / 1.38 / 2.46 / 6.8 / 22.7 cm. Three new tests, each red under its fault.

**Not yet run (rows pending):** E4V1 (hop fix, 1.2.0 fit; cwd a scratch root holding the
1.2.0 calibration), E4V (the final P1-E4 on 1.3.0) and E4V2 (determinism). The build lock was
held by other agents for over three hours, so the fix binary was not built. Open: the hop
test's red-under-fault run; the walk's stationary spread (about 0.7 m along, 0.5 m across)
adds to the GNSS model's absolute error.

## 9. P1 engine gaps (2026-10-27)

P1-E5 gap shape, the P1-E6 RSU-count confound, the P1-E8 attack start and L1: results,
rows and the open items are in research/synthesis/p1-experiments.md ("P1 engine gaps").
Short form: the Tampa RSU deafness locked to the first half of the UTC 100 ms interval is
modelled (`radio.devices.rsu.off_channel`) and matches every interval share; KS 0.071 and
0.076 on two seeds with Tampa's free clock and the log's stamp jitter, against 0.0747 (open).
RSU ids now start at `ROADSIDE_ID_BASE`, so a vehicle's draws do not depend on the RSU count
(golden digests of scenarios with RSUs re-blessed); `attack_start` is the first falsified
message, and the six k-rule budget rows pass.
