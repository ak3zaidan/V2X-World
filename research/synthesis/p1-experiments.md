# Paper 1 experiments (results by experiment)

Each section belongs to one experiment of `paper-plan-2026-10.md` §1.3. It states its
falsification test before its runs and reports pass or fail as measured. The qualifiers of
§1.1 apply to every sentence: "published"/"CV-Pilot", never "deployed"; "would revoke under
MA rule R, given trigger rate T"; "first for the US rule set / the US SCMS chain".

## P1-E8. Evasion: hiding inside the benign device class (item F)

### Pre-registration (written before the runs)

**Question.** The `v2xw ma-replay` MA rule family can read three device-class features
from fields the MA already receives: the evidence BSM's secMark phase, the accused's
stated PositionalAccuracy, and the issuing CA. An attacker writes the first two itself.
What attack budget does it gain by writing them to look like the benign (free, low
accuracy) class? Budget is defined per attacker as the seconds from attack start to the
first revocation decision. If there is no decision, the budget runs to the attacker's
exit and is censored there. It is summed over attackers, per rule and parameter
(`attack_budget_s` in `ma-replay.csv`, per attacker in `ma-replay-attackers.csv`).

**Engine work before the runs.**
- Item F defect: `ma.received.secmark_phase_ms` was computed from the report's evidence
  time, which is the BSM's 1609.2 `generationTime`, not from its `secMark`. On the CV-pilot
  fit the header clock jitters with sd 28 ms, so a locked device's phase landed off the
  epoch on most reports. The `cluster` rule's phase feature was therefore noise. It now
  reads the judged BSM's `secMark`, which the evidence carries.
- New attack `threat/attacker/evasion/BenignClassMimic`. Its payload is a legacy kind,
  unchanged (here ConstPos). Its disguise adds a constant `secmark_shift_ms` to secMark,
  which leaves every Δt unchanged, and states `stated_accuracy_m`. The legacy catalogue
  is untouched.

**Runs.** Quarter fleet, 60 s live after a 30 s warm-up (cut from 120 s before any run
finished: 120 s peaked at 483 MB, over the small run slot), locked share 0.25 (Tampa), 3 % attackers from 30 s, US
profile on secMark, report shuffle 10 s (the stated P1-E7 assumption). One seed. Runs:
EV0 plain ConstPos; EVN mimic with no disguise; EVP 25 ms phase shift; EVA stated 9 m;
EVB both; EVP2 EVP again; L1 and L1B are EV0 and EVB with every OBU locked.

**Falsification tests, fixed before the runs:**
1. **Null.** EVN's `ma.received` stream must be byte-identical to EV0's (digest), so its
   budget gain is 0 under every rule. Shown red by EVB, whose `ma.received` must differ.
2. **No side effect.** A secMark shift that leaves Δt unchanged, and an accuracy claim
   that SM-MBR ignores, must not change which reports are filed. For EVP, EVA and EVB
   against EV0: the same number of reports at the MA. Under `pseudonym-k` and `device-k`,
   which read no class feature, the budget must be identical. Any difference falsifies
   the implementation.
3. **Phase disguise works.** In EVP, the share of attack reports the MA reads as
   "locked" must be 0. In EV0 it must be ≥ 0.15, since about 25 % of attackers are
   locked.
4. **Reported, not predicted in sign:** the budget gain (EVx − EV0, summed over
   attackers, and the count of attackers whose revocation is lost) under `catch` and
   `cluster`. Theory: `cluster` gains only from attackers that are locked in EV0, and
   only while the free class is crowded (≥ c accused devices). `catch` gains only on
   positional subtypes, and ConstPos often leads with `constantPositionFrozen`, which
   weighs 1. If the gain is 0, that is the result.
5. **Determinism.** EVP2 must be byte-identical to EVP.

### Results (as measured, 2026-10-27)

Item-F binary: optimised debug build of this branch. One seed. Quarter fleet, 60 s live,
10 attackers, 5 of them accused at the MA, 2 of those 5 locked. 6,755 reports at the MA
per run. Scripts are in the package scratch folder (`dump.py`, `analyze.py`, `pdc.py`).

| Test | Measured | Verdict |
|---|---|---|
| 1. Null: EVN `ma.received` = EV0 | digest f41deaa2 = f41deaa2; red on EVB (e4026ee1) | pass |
| 2a. Same reports filed (EVP, EVA, EVB) | 6,755 = 6,755 | pass |
| 2b. `pseudonym-k`, `device-k` budget unchanged | 573.7 → 616.8 s and 3,210.8 → 3,426.4 s | **fail as measured** (accounting, below) |
| 3. Phase disguise: EV0 attack reports read locked | 0.248 | pass |
| 3. Phase disguise: EVP attack reports read locked | 0.039, expected 0 | wrong expectation (below) |
| Item F: all-locked fleet's honest reports read locked (L1) | 1.000; **0.091 on the pre-fix release binary** | pass, and the fault goes red |
| 5. Determinism: EVP2 = EVP | 72b8acb2 = 72b8acb2 | pass |

- **2b, root cause.** No MA decision moved: 0 of 370 rule × attacker decision times
  differ between EV0 and EVP, EVA or EVB. The budget's start, `gt.device_exposure`
  `attack_start`, is the attacker's first on-air edit (phase2 `first_on_air`). The
  disguise restamps secMark from the window start, 0.1–4 s before ConstPos first changes
  a claim, so every budget starts earlier. The record's own doc says "first falsified
  message". The definition needs fixing (open). Every "gain" below is measured on
  decision times instead.
- **3, the wrong expectation.** The two locked attackers read 0.0 locked under the
  disguise (85 and 19 reports). The free attackers' phase is uniform and stays uniform
  under a constant shift, so about 2 ms in 50 lands in the locked window by chance.

**The attack budget the disguise buys under the four `ma-replay` families:** none, for
either disguise.

| Family | What the disguise changes | Decisions moved |
|---|---|---|
| `pseudonym-k`, `device-k` | nothing they read | 0 |
| `catch` | the stated accuracy, which only weighs positional subtypes. 99 % of ConstPos attack reports lead with `constantPositionFrozen` (weight 1). Honest devices state 6.5 m, so 9 m stays in the same 3–10 m band | 0 |
| `cluster` (per-report class) | the class, but both classes are crowded. About 4 % of free devices' reports land in the locked window (honest reports read locked: 0.078), so locked becomes a crowded class at c = 3 and c = 10 alike | 0 |

**What a per-device class would change.** This is an offline analysis (`pdc.py`), not in
the engine family. It classifies each device with the real-data rule (locked when ≥ 80 %
of its reports so far are within 1 ms) and keeps everything else as in `cluster`.
- **c = 3:** nothing moves.
- **c = 10 (k = 3):** the disguise delays both locked attackers' revocation, by 10.1 s
  and 0.3 s.
- **c = 10 (k = 5):** both revocations are lost inside the run, a budget of ≥ 57 s and
  ≥ 37 s, censored at the run's end.
- **Honest devices blocked:** 31–69, the same with and without the disguise (68 → 69 in
  one cell).

So the phase disguise buys budget only against an MA that classifies per device and
relaxes thresholds for a crowded free class. That is the MA a benign-trigger-aware
operator would build.

Qualifiers:
- This is one seed, 60 s, 2 locked attackers. It shows the mechanism, not a rate.
- "Would revoke under MA rule R" applies throughout; no US MA rule is published.
- The issuer feature cannot be faked and was not exercised: all accused were in
  2 providers.
- In the all-locked L1, sound rules block up to 31 honest devices. The pre-fix binary
  gives the same 31, so it is not this change. It disagrees with MAL1R's 0 (8becf72a) and
  is open.


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
