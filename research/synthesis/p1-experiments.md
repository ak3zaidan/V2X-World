# Paper 1 experiments (results by experiment)

Each section belongs to one experiment of `paper-plan-2026-10.md` §1.3. It states its
falsification test before its runs and reports pass or fail as measured. The qualifiers of
§1.1 apply to every sentence: "published"/"CV-Pilot", never "deployed"; "would revoke under
MA rule R, given trigger rate T"; "first for the US rule set / the US SCMS chain".

## P1-E10: multi-day projection (item G), ANALYTICAL

**What it is.** `tools/surrogate` (v1.0.0, shared with Paper 3): an open-loop surrogate of
the honest misbehaviour-report flow. It is fed by per-class, per-corridor trigger rates and
per-class report conversion measured from an engine run, and it simulates no traffic, radio
or cryptography. Every number it produces is labelled analytical. Class is the secMark-phase
class of P1-E2 (`tools/realdata/devclass.py`, thresholds frozen), read from the BSMs on the
air. A corridor is an avenue band 250 m wide across Manhattan's 29° grid (an ASSUMPTION of
granularity, not a measured unit). Stages: verdicts (a Poisson stream per class, corridor
and detector, per second of the subject's exposure, with a per-device gamma multiplier
fitted to the per-subject counts); stored, uploaded, lost and relayed reports (the class's
measured probabilities); the RA report shuffle (CAMP-EE SCMS-765: one day after the
batch's first report, or 10,000 reports); and in the projection, the k-threshold MA rules
over LA-linked receipts in a rolling 7 days (P1-E7's per-device family).

**The comparison run.** FB3's records are gone from the scratchpad and FB4 is not on
`main`, so the full engine run is the quarter-fleet two-day benchmark
(`scenarios/real-world/paper1/p1-e10-day.yaml`: `midtown-scms-benchmark-quarter.yaml` with
the CV-pilot BSM timing classes at 25 % locked, the US profile on secMark and the report
records; live 0-60 s and 43,185-43,215 s, fast-forward otherwise). The rates come only from
a second seed of the same file (`p1-e10-cal.yaml`, 0xBE7C_0002). The target run's verdicts,
reports and MA receipts are never read for the inputs. Its BSM census (seconds of exposure
per window, class and corridor) is read for the conditional prediction E10S. E10SU is fully
open loop: it uses the calibration's own exposure.

**Falsification test (§1.3: "must reproduce a full engine run's first day within
tolerance").** Tolerance T-E10 was written before either run was read
(`crates/v2xw-engine/tests/expectations/scms-paper1.yaml`, rows `P1.E10.*`, committed with
`measured: pending`). The surrogate fails P1-E10 if any of these is outside its band:

| Measure (day 1) | Band | Why this width |
|---|---|---|
| detector verdicts; reports stored; uploaded; received by the MAs in the first release | engine / surrogate in [0.75, 1.33] | about 2σ of seed-to-seed spread for verdicts clustered on about 100 triggering devices (CV about 14 %) |
| verdicts in each live window | [0.6, 1.67] | fewer devices per window |
| free-class share of verdicts; RSU-relay share of uploads | ±0.10 | binomial spread at a few hundred reports, plus the class boundary |
| verdicts by corridor | total-variation distance ≤ 0.15 | |
| the MA's first receipt | ±120 s | the first upload lands inside 0-60 s; the shuffle adds exactly a day |
| MA cases | exact | |

Each row names the injected surrogate faults that must turn it red (`surrogate.py
selftest`): rates ×1.5, no shuffle, classes pooled, corridors pooled or shifted one band,
the rollover window dropped, the store stage removed, relay share flipped, and 1 % cases.

**Amendments, made before either run was read (2026-10-27).**

1. This build's events-level recording has no `det.observation` channel; a format check on
   the calibration run's first window found none. So the per-class, per-corridor trigger is
   the report put in the device's store (`pki.report_fate` `stored`). The verdict total is
   the run report's `verdicts_fired`, predicted as triggers × the calibration's verdicts
   per trigger. The window, share and corridor rows now read triggers. The bands are
   unchanged.
2. Both runs stop at 87,300 s. Day 2 of the file is fast-forward only, and on the
   shared machine the first attempt ran at 88-128 wall-s per simulated hour. Day 1 and the
   RA's release of its reports (about 86,430 s) are inside it. The first attempt on the
   450 MB tiny slots was stopped by the slot's memory cap at the rollover window (462 MB);
   the runs were repeated on the 1.2 GB slot.

3. One model change was made after reading the calibration run and before reading the
   target. In the calibration, the RA shuffle released at its 10,000-report threshold
   (two RAs, 20,000 receipts at 43,195 s, inside the rollover window), not on the one-day
   timer. So the prediction now applies the SCMS-765 release rule (timer or threshold,
   per RA) to the predicted upload timeline. It no longer copies the calibration's
   first-receipt offset.

**Result: P1-E10 passes, 15 of 15 rows within T-E10.** Release binary from `f4982788`.
Seed 0xBE7C_0001 for the target and 0xBE7C_0002 for the calibration. `--record-level
events`, slot 2, peak 233 MB RSS. Each run took about 40-60 min wall, almost all of it
fast-forward on a loaded machine.

| Day-1 measure | Engine (target) | Surrogate E10S | Ratio / Δ | Fully open loop E10SU |
|---|---|---|---|---|
| detector verdicts (`verdicts_fired`) | 83,797 | 77,740 | 1.078 | 99,212 (0.845) |
| reports stored (triggers) | 27,398 | 24,828 | 1.104 | 31,686 |
| of them in 0-60 s / 43,185-43,215 s | 10,072 / 17,326 | 11,735 / 13,093 | 0.858 / **1.323** | |
| free-class share | 0.983 | 0.990 | −0.007 | 0.994 (−0.011) |
| corridor profile, TVD | | | **0.130** (≤ 0.15) | |
| uploaded by 86,400 s | 25,569 | 23,314 | 1.097 | |
| RSU-relay share of uploads | 0.138 | 0.133 | +0.005 | |
| MA receipts, first release | 20,000 | 20,000 | 1.000 | 20,000 |
| first MA receipt | 43,203 s | 43,207 s | −3.5 s | |
| MA cases | 0 | 0 | exact | |

What the pass does and does not show:

- **It is a seed-transfer test at one density and one schedule.** The trigger rates are
  rates per second of the accused device's exposure (every reporter's report counts),
  so they carry the quarter fleet's density and its gridlock.
- **The rollover window is the weakest row: 1.323 against a 1.33 edge.** One pooled rate
  per class and corridor does not capture the higher per-exposure rate after the 12 h
  fast-forward. That is visible in-sample too: the calibration's own windows give
  13,392 / 18,284 against a pooled 15,790 / 15,904.
- **The corridor dimension earned nothing here.** With corridors pooled, the TVD is
  0.107. With every corridor's rates shifted one band, it is 0.084. Both are better than
  the per-corridor 0.130. The profile is set by where the traffic is, not by
  per-corridor rates fitted on one seed. The row goes red only when the exposure layout
  is flattened (`flat_corridor`).
- **The MA batch is set by the shuffle threshold, not by the rates.** Two RAs × 10,000
  is reached in both runs. Rates ×1.5 leave the row green. Dropping the rollover window
  turns it red.
- **Red-under-fault (`surrogate.py selftest`).** Each of the 15 rows goes red under at
  least one named fault, and each of the 11 faults turns at least one row red. Four
  faults turn only the determinism row red: corridors pooled, corridors shifted, store
  stage removed, threshold ×2. They change the prediction, but by less than the
  tolerance rows can detect.
- **Reproducibility.** Same inputs give a byte-identical prediction (row
  `P1.E10.rerun`), and the projection is byte-identical on a rerun. Engine-run
  determinism was not re-checked here.

**Projection, a week (ANALYTICAL; `profiles/midtown-week.json`, `surrogate.py project`).**
Assumptions: 10,000 devices; one hour of driving a day at the calibration's class mix and
trigger rates; k LA-linked MA receipts in a rolling 7 days. Under these assumptions about
1.2 × 10⁸ reports reach the MAs each day, and the shuffle releases at its threshold the
same day. Under the k-threshold rule the MA would block 9,974 of 10,000 honest devices
at k = 1 and 9,691 at k = 100, given trigger rate T = 4.45 stored reports per free-device
exposure-second and 0.087 per locked one. An all-locked fleet would still have 8,674
blocked at k = 100. Read this as: at Midtown rush-hour density, a per-device count rule
cannot separate honest devices under the published US rule set. It is not a deployed
outcome. The rates were measured in quarter-fleet gridlock and held constant all day,
which overstates free-flow hours.

**Open.** Rates by window or time of day (the rollover row). A second density, to make
the rates transferable. A rate per receiver-pair rather than per accused-exposure-second.
FB4's full-scale records, once on `main`. Engine determinism at this scenario.
