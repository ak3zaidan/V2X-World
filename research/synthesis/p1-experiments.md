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

**Result.** Pending: the runs had not been read when this was committed.
