# 06 — How close the engine's SCMS is to the real US deployment: validation summary

First written 2026-10-18 at `8245ef81`. Rewritten 2026-10-22 at `0f7eda5c` and at
`bc3bca1b`, and 2026-10-23 at `a4982f9f` (fix wave 3 and final runs part 2). **Rewritten
again 2026-10-23 at `d90a6298`** (main), after:
- finalize 4 (TESTS, FF and MEM, merged at `2f6c664d`, `66d65542` and `58e0b270` onto
  `747a0773`; `cargo fmt` `4a1d014e`; docs `fe3757b9`);
- the full-scale benchmark to the end (BENCH, merged at `d90a6298`: FB3, docs and the
  `scms-full-bench.yaml` expectations only).

**Updated again 2026-10-24** after the backend, density and Paper 1 wave (BACKEND,
DENSITY, PAPER1VAL), merged onto `e15bfec1` (BUILD-STATUS "SCMS backend, density and
Paper 1 wave integration"). §2.2, §6 and §9 state its results: R-FFWIN fixed; the lost
relay uploads traced and fixed; `MWP.off-no-backend`, the 47 `Invalid` states, VRU PQ
pricing and the `node.rx` refusal label fixed; the 100-Car braking target, the node-874
snap and the lane-bend speed (both opt-in, the benchmark sets them); the rollover and
fast-forward-stop costs profiled and cut; and a new 73-row table of the Paper 1 features in
real runs (`scms-paper1.yaml`). Sources: `testing/honest-density.md` (**density**) and
`testing/paper1-validation.md` (**paper1**); the backend wave's results are in §6 items
2, 5, 8, 11 and 13.

**Updated 2026-10-24** after the re-run and oracle wave (ORACLE, RERUN-A, RERUN-B), merged
onto `81a12183` (BUILD-STATUS "SCMS re-run and oracle wave integration"). §2.2, §6 and §9
state its results: the R-JOINCRL fix (`2a14d6c4`, the only engine change), the oracle on
fresh records with the new OF-3 rules, the matrix and realism re-runs on the 300 s realism
base, 16 new realism rows, and 60 rows of re-runs and benchmark variants
(`scms-variants.yaml`). Sources: `testing/oracle-2026-10-24.md` (**oracle-24**),
`testing/rerun-2026-10-24-a.md` (**rerun-a**) and `testing/rerun-2026-10-24-b.md`
(**rerun-b**).

No Rust or `ui/` code changed after `4a1d014e`, so every test result of the finalize 4
integration is a result for HEAD's code. The one thing `d90a6298` adds that a test reads is
`scms-full-bench.yaml`, which the expectation-loading tests parse; their run on HEAD did not
finish (§2.2).

This page states the final state of the realism round: the verdict on every real-world rule,
the test totals across every configuration that ran, what is confirmed working, and what
still deviates or cannot be checked, with its impact.

It is for the owner and for a reviewer from outside the project. It goes no further than its
sources. The sources are the four validation checklists, the testing reports, the benchmark
and the merge notes in `docs/design/BUILD-STATUS.md`. Every claim names its row or report.

**How to read the row references:**
- **crypto #n**: `validation/crypto-formats.md` §1, row n.
- **proto Xn**: `validation/protocols.md`, item Xn.
- **gov X-n**: `validation/governance-deployment.md` §1, row X-n.
- **veh Xn**: `validation/vehicle-and-roadside.md` §1, row Xn.
- **07 §n**: `07-config-testing-report.md`.
- **matrix**, **stress**, **oracle-final**, **final-bench**, **final-run**, **usfp**,
  **pq-profile**, **knobs**, **effects**, **edge**, **realism**, **ccsig-memory**,
  **measure**, **rerun**, **ff**, **memden**, **full-bench**: the reports in `testing/`
  (`rerun` is `rerun-2026-10-22.md`, `ff` is `fast-forward.md`, `memden` is
  `memory-density.md`).
- **bench §n**: `docs/benchmark/scms-real-benchmark.md`.
- **BS wave 2**, **BS final**, **BS wave 3**, **BS final 2** and **BS fin4**: the
  BUILD-STATUS sections "SCMS realism fix wave 2 integration", "SCMS realism final test wave
  integration" (both 2026-10-22), "SCMS realism fix wave 3 integration", "SCMS realism final
  runs part 2 integration" and "SCMS realism finalize 4 integration" (all 2026-10-23). The
  full-bench merge (`d90a6298`) has no BUILD-STATUS section yet (§9).
- **04 §n** and **RT Rn**: `04-validation-report.md` and `docs/redteam/pki-privacy.md`.

The checklist files keep the verdicts their validators recorded on 2026-10-18. The only
exceptions are proto A16, D18 and I10, which were updated in `898c077a`. **Where a verdict
here differs from its checklist, this page is the later one.** §4 lists every such change
with its evidence.

## 1. What was checked, and how

Four validators each took one part of the SCMS. Each one listed every rule that the real US
system follows in that part, from primary sources:
- the IEEE 1609.2 / 1609.2.1 ASN.1 modules;
- the SCMS Manager documents (1609.2.1 Profile, Provider Requirements, Elector Policy and
  Technical Specification, EAC Requirements, re-enrolment specification, misbehaviour report
  specification);
- CAMP's EE Requirements 1.2.2;
- the USDOT primer, the USDOT DSRC RSU Specification v4.1 and the CV-pilot reports;
- the CTLs and certificate chains that SCMS Manager actually publishes.

Code comments were never used as a source. Each rule's evidence is one of five kinds:
- an official test vector;
- the independent Python oracle, which was written without reading the Rust;
- a byte-for-byte comparison with a published real artefact;
- a Rust test;
- **runtime evidence**: the oracle checking the PDUs that real runs recorded, and
  configuration tables comparing same-seed runs. Each expectation was committed before the
  run it judges.

Verdicts:
- **Matches.** The engine does what the source says, and the evidence is named.
- **Fixed.** The engine did not match. It was corrected in one of the rounds, and the new
  check was shown to fail on the old behaviour (the old code put back, or the old run's
  recording).
- **Documented deviation.** The engine does not match. The row gives the reason and what the
  difference changes.
- **Unverifiable.** No public source fixes the behaviour or value. The row says how the engine
  models it. Each such value is labelled ASSUMPTION in `constants.md`.

A row with two verdicts is counted under its first.

## 2. Final totals

### 2.1 Rules

| Area | Rules | Matches | Fixed | Documented deviation | Unverifiable |
|---|---|---|---|---|---|
| Cryptography and data formats | 80 | 61 | 10 | 4 | 5 |
| Protocols and backend flows | 109 | 87 | 12 | 3 | 7 |
| Governance, deployment and calendar | 70 | 44 | 10 | 10 | 6 |
| On-vehicle and roadside behaviour | 67 | 40 | 11 | 5 | 11 |
| **All** | **326** | **232 (71.2 %)** | **43 (13.2 %)** | **22 (6.7 %)** | **29 (8.9 %)** |

**Final match: 275 of 326 rules (84.4 %) do what the real system does** (matches plus fixed).
It was 264 (81.0 %) on 2026-10-18, 270 (82.8 %) at `0f7eda5c` and 275 at `bc3bca1b`.

**What moved in the 2026-10-24 backend, density and Paper 1 wave: no rule verdict.**
Every fix was a runtime defect with no checklist row (§6 items 1–3, 5, 8, 11, 13, 21).
The largest runtime items now open are the honest report volume at full density (the
braking target is calibrated and two traffic fixes are in, opt-in; the full-fleet re-run
is owed), the full-scale cost (profiled and cut at quarter fleet, not re-measured at full
fleet), P1-E4 against Wyoming, and the CRL gate of a car that powers on (§6 item 22).

**What moved since `bc3bca1b`: no rule verdict.** Fix wave 3, finalize 4 and the full-scale
benchmark fixed or settled runtime defects that have no checklist row (§4.2). None of them
reopened a rule. The full-scale run to the end exercised the specified report shuffle, the
hourly conditional CRL poll, the weekly top-up at the i-period boundary and the one-hour
certificate overlap at full fleet, and each behaved as its row says (§3 item 15). One row
could move on an owner reading: veh M13 (do RSUs report misbehaviour?) is still counted
unverifiable, although the engine follows the reading that the US profile has no roadside
reporter (§8 item 6). If the owner accepts that reading, the match is 276 (84.7 %).

Of the 51 rules that do not match:
- 29 cannot be checked against any public source (§7);
- 22 are known differences (§5). 14 of them change nothing a device trusts or does.

The runtime defects that testing found and that have no checklist row are in §6. Since the
previous version of this page, the full-scale benchmark has run to the end (172,800 s, rc 0,
peak 2,105 MB), R-FFBURST is fixed, and R-FFHOLD, R-FFSLOW and the rollover's pseudonym
count turned out not to be engine defects. Honest traffic at full density opens no case and
revokes nobody. The largest items still open are the honest report volume at full density
(2,001 reports in 300 s, explained but partly uncalibrated), R-FFWIN, the cost of the
rollover window and of each fast-forward stop, and the re-runs that the new benchmark turn
rule makes owed.

### 2.2 Tests across all configurations

**Configuration tables.** In each table the expectations were written before the runs, and
each run was compared with its base on the same seed. The counts are the verdicts recorded
in each table's expectations file.

| Table | Rows | Pass | Fixed (bug, then pass) | Expectation wrong, corrected | Open |
|---|---|---|---|---|---|
| Matrix: 34 runs (32 one-change variants of the Midtown realism base, plus `gnss-raw` and `us-profile`); 44 rows re-measured on `81a12183` with the 300 s base (rerun-a, 0 fail), 55 still carry their `77cffb5b` values (`scms-matrix.yaml`) | 99 | 80 | 7 | 11 | 1 |
| Realism rows on the matrix runs (`runtime_expectations.yaml`): 59 of the 62 old rows re-run on `81a12183` with the 300 s base, plus 16 new rows N1–N16 (rerun-a); V7 ×3 (`fleet2x`) not re-run and counted at their `77cffb5b` pass | 78 | 62 | 0 | 13 | 3 |
| Settings knobs, same-seed pairs (knobs) | 21 | 15 | 3 | 2 | 1 |
| Effects table (effects) | 88 | 77 | 7 | 4 | 0 |
| Edge cases (edge; 3d fixed in 07 §7; 11 measured by matrix and stress) | 12 | 7 | 5 | 0 | 0 |
| Runtime realism, base (realism) | 27 | 22 | 5 | 0 | 0 |
| Runtime realism, changes (realism) | 8 | 6 | 2 | 0 | 0 |
| Stress: multi-week, post-quantum, first full-fleet seconds; R-NOCRL rows re-run on `77cffb5b` (`scms-stress.yaml`) | 35 | 17 | 1 | 2 | 15 |
| Final bench: full fleet, hybrid ML-DSA-44, certificate cycle, determinism (`scms-final-bench.yaml`) | 28 | 15 | 0 | 3 | 10 |
| Final run: certificate cycle after R-CCSIG, full-scale benchmark (`scms-final-run.yaml`; FB2.privacy corrected in rerun-b) | 27 | 16 | 2 | 3 | 6 |
| Oracle-final on `88e24b09`: expectation rows, conf and mbr runs (oracle-final §8.1) | 60 | 60 | 0 | 0 | 0 |
| Oracle-final: `88e24b09` against `4924a5d2` and base against variant (oracle-final §8.2) | 10 | 9 | 0 | 1 | 0 |
| Full bench on `fe3757b9`: R-FFSLOW on the conformance and quarter files, the quarter fleet end to end, the full-scale file end to end (`scms-full-bench.yaml`, full-bench) | 10 | 6 | 0 | 2 | 2 |
| Re-runs and benchmark variants on `81a12183`: PQ cycle and hybrid, relay repro, R-FS40, `gnss-raw`, MW/MWC/MWC2, Q60 LTE, 2× density and Portland (`scms-variants.yaml`, rerun-b) | 60 | 51 | 0 | 5 | 4 |
| Oracle on fresh conf and mbr records of `81a12183`, 32 rows each with OF-3 and OF-3-JOIN (oracle-24 §1–§3); 3 rows red on R-JOINCRL, green after its fix | 64 | 61 | 3 | 0 | 0 |
| Paper 1 features in 41 real runs on `e15bfec1`: timing classes, MA store, VeReMi export, BeaconFrequency, P1-E4 against Wyoming, determinism (`scms-paper1.yaml`, paper1) | 73 | 55 | 4 | 4 | 10 |
| **All** | **700** | **559** | **39** | **50** | **52** |

The realism file records no per-row "fixed" verdict. The rows that the R-USFP fix turned
green are counted as pass, and the 6 rows corrected in the re-run (Z1, Z3, Z4, A3,
V8-us-catches-constpos and V8-us-checks-every-bsm, rerun §6) as corrected. Each corrected
row was shown to fail with a value outside its new band injected. In the 2026-10-24 re-run
(rerun-a) P3, P4 and V3 were corrected (per-device forms; V3 counts from the case's first
detection), and N6, N13, N11 and N15 were corrected before the results commit; all 16 new
rows and P3, P4, V3 went red with a bad value injected. In `scms-variants.yaml` all 51
passing rows go red under `variants_faults.py` (0 silent). In `scms-paper1.yaml` 58 of the
59 passing rows go red under it; the silent one (a ratio over a zero base) went red on the
real pre-refit fault (0 to 76, paper1 §3).

**What the 52 open rows are.** Most are not engine problems:
- **28 are superseded by a later table, or carried into one.**
  - Stress, 12: the 10 full-fleet rows (`FS.*`) were never run, and the final bench, final
    run and full bench measured them as `FB.*`, `FB2.*` and `FB3.*`. `PQ.certcycle` found
    R-CCSIG, which is now fixed (`PQHFC.sizes` and `PQHMC.sizes`, final-run).
    `PQ.determinism` passes in the final bench.
  - Final bench, 10. Six are closed by the final run: `FB.devices` and
    `FB.topups-at-rollover` pass as `FB2.*`, and `PQHMC.*` ×3 and `PQHFC.ccsig` are fixed
    (R-CCSIG). Four are carried into the full bench: `FB.ff-ticks` (69,130 stops, each with a
    real cause), `FB.cost` (into `FB3.completes`, still open), the honest false positives
    (`FB3.honest-reports`, pass) and the privacy link claims (`FB3.privacy`, pass).
  - Final run, 6, all closed by FB3: `FB2.completes` and `FB2.cost` (FB3 completed; its
    cost is `FB3.completes`, open), `FB2.ff-ticks` (R-FFSLOW does not reproduce,
    full-bench §2), `FB2.crl` and `FB2.backend-load` (`FB3.pki`, pass, and bench §000's
    per-entity table), `FB2.honest-reports` (`FB3.honest-reports`, pass) and `FB2.privacy`
    (`FB3.privacy`, pass). `FB2.privacy` itself was a wrong expectation (no change is due
    at the boundary under CERTCHG with a one-hour overlap, ff §3) and is now corrected
    (rerun-b, `7c0ace43`).
- **1 is open by design:** the knobs row for the request shuffle, published as partial,
  because the shuffle has no in-run effect (knobs). Its test passes (`scms_knobs_runs` 18/18).
- **10 are the Paper 1 table's open rows** (paper1 §5, §6 item 21): P1-E4 against Wyoming
  (KS 0.337 against a 0.092 tolerance), the residual body (engine p90 2.3 cm against 6.0
  cm), the highway speed (median 22 against 34 m/s), the k>0 secMark slip (TM-1), one
  honest device blocked through MaxAcceleration, the 26 VeReMi true positions at (0, 0)
  (VR-2), and the TS 103 759 suite that `detection.local` cannot select.
- **13 are open items** (counted as they stand in their files; the 2026-10-24 backend
  wave fixed `FB3.metric-window-after-ff` and `MWP.off-no-backend` in code, but neither
  run is re-measured yet).
  - Full bench, 2: `FB3.completes` (the rollover window took 4,767 s for 60 s and the
    second fast-forward 2,014 s, both slower than expected; §6 item 3) and
    `FB3.metric-window-after-ff` (R-FFWIN, fixed in code, §6 item 2; FB3 not re-run).
  - Stress, 3: `MWC.recovery` (R-JOINCRL, fixed in code at `2a14d6c4`; the stress run is
    owed again, §6 item 7), `MWP.off-no-backend` (fixed in code,
    §6 item 8; the PQ/MW re-measure gives 0 records while off, the stress row is not
    re-judged) and `PQ.backend-bytes` (the per-certificate factor was not isolated; final-run
    §2 shows the bytes are reports, not provisioning).
  - Matrix, 1: `lte-latency-budget`, outside the SCMS (§6 item 16).
  - Realism, 3: L5, the case-board review time (§6 item 15), T5, the US component set,
    which carries `status: open` in its file, and A1, report airtime 1.25 % against a 1 %
    bound (§6 item 18).
  - Variants, 4: `RR.MW.no-overdue` (R-JOINCRL in the base MW run on `81a12183`, 16,618
    refusals from 33 joiners; fixed in code since, re-run owed), `VB.LTE.latency-budget`
    (p95 110 ms, §6 item 16), `VB.2X.honest-reports` (13 against 20 cannot be told from
    noise on one seed) and `VB.PDX.speed` (no published Portland speed to judge 3.2 m/s).

**Three notes on the table:**
- **The realism base now runs its first live window to 300 s** (`b645dd99`, rerun-a §2).
  With RSUs filing no report (R-UNSEC), the 0–60 s window gave the most-heard attacker only
  2 vehicle reporters against the MA's quorum of 3, so the base opened no case (rerun-b
  open item 1 saw the same on the 60 s base). On 300 s: 2 of 3 attackers revoked, 0 honest,
  decision at 115,220 s and CRL at 147,600 s as before; wall 33 → 185 s. Every number
  measured on the realism base moves (rerun-a §3); 55 matrix rows and V7 ×3 still carry
  `77cffb5b` values from before wave 3 and the 300 s base (§9).
- **The benchmark's apex turn rule (`traffic.turn_curve_window_m: 6`, memden §2.2) changes
  the traffic of every scenario that inherits the benchmark**: the quarter fleet, LTE,
  Portland, the stress files and, through the quarter file, the realism base and its 33
  matrix and realism children. The matrix, realism, final-bench and final-run expectations
  were measured without it. Only the full bench (FB3, QD, D1) ran with it.
- **Not in the table:** the policy package's before/after runs (07 §8), and usfp's same-seed
  before/after table (§3). Neither is a row. On `edge/clock-skew-ahead`, honest vehicles
  revoked fell from 19 to 0. On the realism base under the US profile, honest vehicles
  revoked fell from 26 to 0.

**Independent checks**

| Check | Result |
|---|---|
| Oracle self-test (official vectors), HEAD | 412/412 |
| Oracle mutation run, HEAD | 24 faults, all red, 0 silent |
| Oracle `verify-emitted`, HEAD | 10/10 |
| Oracle `records` on fresh conf and mbr captures, with OF-3 and OF-3-JOIN (oracle-24 §2) | On `81a12183`: conf 221,735 / 221,770, mbr 582,679 / 583,831, every red check R-JOINCRL. **With the fix (`2a14d6c4`): conf 220,684 / 220,684, mbr 583,831 / 583,831, 32/32 rows each.** Engine fault "skip `note_prerun_crl`" built and run: OF-3 red (130). Earlier, on `88e24b09`: 279,236 of 279,236 |
| Record faults (`--mutate`) on the fresh captures | conf 38 red, 0 silent, 1 n/a; mbr 39 red, 0 silent (4 new OF-3 faults included) |
| Conformance run on the fixed code (oracle-24) | `conformance.rs` 2/2; `check_run.py` PASS 22,982 / 22,982, 0 disagreements; `--mutate` 18/18 red |
| Fault proofs of the re-run and oracle wave | R-TRUSTLEAK OBU and VRU fallback red, each alone; F6 and F7 red, one test at a time; `timeline` red under its fault; all restored (oracle-24 §4) |
| Record faults injected into those runs | 69 of 69 applicable faults red (conf 34, 1 not applicable; mbr 35), 0 silent, after oracle fix 2 (oracle-final §8.5) |
| `component_certs.py`: every component certificate of the M0 run against the published chain of its form (rerun §8) | **10/10 roles** match field for field (ISS pre-production chain for provider-a, SAESOL production chain for provider-b); every SPDU re-encodes byte for byte; `--mutate` 2 of 2 roles red |
| `ctl-files` on the engine's own CTLs (F1 `gov.json`) | PASS: CTL-FILE 16, CTL-ORDER 4, CTL-QUORUM 6, CTL-SIG 140; 6/6 faults red after oracle fix 1 |
| `ctl-files` on the six CTLs SCMS Manager publishes | 30/30 elector signatures valid; 5/5 faults red |
| `check_run.py` on the F1 conformance capture of `bc3bca1b`'s code | 18,910/18,910, 0 disagreements; `--mutate` 18/18 faults red |
| `oracle_final.rs` reproductions (OF-1, OF-2) and `conformance.rs` | 2/2 and 2/2 |
| `check_run_crl.py` on a newly generated CRL corpus (`2004bb87`) | 19/19 |
| Published artefacts; `real_artefacts`; `profiles.py`; PDU checkers; conformance run (`8245ef81`, not re-run since) | 164/164; 7/7; 24/24; 24/24, 25/25, 18/18, 4/4; 19,183/19,183 |
| Fault proofs of fix wave 3 | PQMEM F1–F5 red on main (BS wave 3); DETPRIV 6 of 6 injected faults red (measure §6); RERUNS: 3 sensitivity faults, the M-RX1 filter, the 5 s CRL poll and 4 realism size/count injections red (rerun §1–§3, §6) |
| Fault proofs of finalize 4 | FF: 4 of 4 red (GNSS resume a no-op, `ped_wait` pause a no-op, privacy pause a no-op, traffic not paused; ff §6). MEM: 2 of 2 red (a parked stream that ignores its word position, an interner that never reuses a copy; memden §1.2). TESTS: 2 of 2 red (a unit that drops every reassembled report, judged evidence from another signer; §6 item 9) |

**Rust, UI and build checks**

| Suite | Result |
|---|---|
| **2026-10-24 backend, density and Paper 1 integration (`c2447283`)**: `cargo check --workspace --tests` | Passes, known v2xw-msg warnings only |
| 2026-10-24 (`c2447283`): v2xw-engine, mobility, cli, proto, node, metrics, record, 161 targets one at a time (`energy_glosa` skipped at the 4.5 GB cap) | **1,960 pass, 8 fail, 12 ignored**, all 8 known: engine 498/2/10 (`foundations` cellular bytes, `applications` GLOSA), mobility 311/5/2 (§6 item 16), cli 32/1 (`longrun_budget`); proto 322, node 268, metrics 275, record 254 pass. `sensitivity_expectations` 4 + 1 ignored and `sensitivity_catalogue` 3/3 parse `scms-paper1.yaml`; `scms_knobs_runs` 18/18; tools/realdata 28/28 |
| Release build and boot (`c2447283`) | Release build OK (9 m 10 s); the simulator listens on 8758 within 4 s, `/agent/status` and `/rpc/schema` 200 |
| 2026-10-24 re-run and oracle integration: `cargo check --workspace --tests` | Passes, known v2xw-msg warnings only |
| 2026-10-24: v2xw-engine lib and 71 of 72 test binaries, one at a time (`energy_glosa` skipped at the 4.5 GB cap, as before) | **493 pass, 2 fail, 10 ignored.** The 2 failures are the known `foundations` cellular bytes (3114 against 3574) and `applications` GLOSA. `timeline` passes (fixed in oracle-24). `sensitivity_catalogue` 3/3 and `sensitivity_expectations` 4/4 parse every expectations yaml, `scms-variants.yaml` and `scms-full-bench.yaml` included. `scms_knobs_runs` 18/18 (2,235 s). `phase2` 13/13 after one merge fix (`da643bfc`, below); `phase2-path` passes on the re-blessed digest |
| 2026-10-24: v2xw-node | **266 pass, 0 fail** (the new OBU and VRU R-TRUSTLEAK tests included) |
| 2026-10-24 merge fix: `phase2::without_either_distribution_path_the_revocation_never_reaches_a_vehicle` | Failed after the R-JOINCRL fix: 14 vehicles switched on after the publication hold the list from their RA connection (`crls_installed` 14 against 0; vehicle revoked receptions 116). Phase2Report now counts join installations and joiners' revoked receptions apart (`crls_installed_at_join`, `revoked_receptions_at_join`); the test asserts that every vehicle install and refusal is a joiner's and that the join path ran. No record changes |
| 2026-10-24 (oracle-24, on the branch): v2xw-threat, v2xw-server, Studio e2e | threat 241/241 (`in_the_loop` with one test thread); server 140 pass, 4 ignored, `pacing` hang 2 of 6 runs (§6 item 20); Studio e2e 32/32 |
| HEAD's code (`4a1d014e`, run at the finalize 4 integration, BS fin4): `cargo check --workspace --tests` | Passes, with the known warnings (v2xw-msg unused imports). Passes again on `d90a6298` |
| HEAD's code: v2xw-sec, msg, proto, node, core, metrics, demand, mobility, cli | **1,882 pass, 7 fail, 3 ignored.** All 7 predate the merges: the 5 known mobility traffic tests (§6 item 16), `v2xw-demand transit::a_bus_serves_its_stops` and `v2xw-cli longrun_budget`. The last two fail identically on `747a0773` before the merges, and neither was in the earlier baselines |
| HEAD's code: engine lib plus 31 SCMS, phase2, PQ, sensitivity, calendar and VRU binaries (`scms_knobs_runs` 18/18 in 2,108 s) | **301 pass, 0 fail, 8 ignored.** `scms_wiring` and `pq_relay` pass again (§6 item 9); the new fast-forward, GNSS-resume, `ped_wait`, privacy-rate and memory tests pass (ff §6, memden §1.2) |
| `d90a6298`'s expectation file: `sensitivity_catalogue` and `sensitivity_expectations`, which parse every yaml in `tests/expectations` | **Not finished.** The run (with phase2 and `scms_*`) was started at the full-bench integration and stopped before it reported. `scms-full-bench.yaml` uses the schema and the four verdict words of `scms-final-run.yaml`, which those tests already load |
| v2xw-threat (`calibrate`, `in_the_loop`, `legacy_engine_compare`) | Last run on `09e55393` (wave 3): pass. Not in the finalize 4 run |
| The 6 sensitivity failures of `bc3bca1b` | Root-caused and re-stated (rerun §1–§2): `sensitivity_radio` 27/27 (the real 80 B certificate changed frame sizes), `sensitivity_security` 20/20 (a 5 s CRL poll refuses the next pseudonym before it is reported, so nothing is left to link). CLI rows 28/28, 13/13, 3/3, 24/24 |
| `fragmentation::a_padded_message_…` (M-RX1) | Passes. M-RX1 blamed an SDU's fate on a fragment the PHY lost after it; the engine was right (rerun §3) |
| Regression digests | `phase2-path` and `i-s1-phase2-60s` re-blessed for wave 3's record changes; the phase2 and `scms_scale` digests are unchanged by finalize 4's memory and fast-forward changes. A 20 s full-scale run gives the same stream digest before and after the memory commits (`866d3101…`, memden §1.2) |
| `longrun_budget` (v2xw-cli) | Fails. At `2004bb87` it was 24.9 MB against a 24 MB budget; at `747a0773` and HEAD the slice cost grows from 2.26 s to 10.7 s. Pre-existing; not root-caused |
| v2xw-server (`2004bb87`, not re-run since) | Pass |
| `timeline::a_param_change_of_the_arrival_rate_takes_effect_at_its_instant` | Failed when last run (5.29×, band 2.5–5.0). Traffic-only, no SCMS, predates this round; not in the finalize 4 run |
| UI (`4a1d014e`) | protocol and viewer `tsc -b` build; Studio typecheck passes; Studio 326/326 unit tests, with the PKI view fixtures re-recorded from `d0e6530b` (`aaa2f354`). Studio e2e was last run at `88e24b09` (32/32) |
| Release build and boot (`4a1d014e`) | Release build OK (8 m 11 s); the simulator boots on port 8758, and `/agent/status` and `/rpc/schema` both answer 200. Not rebuilt for `d90a6298`, which changes no code |

## 3. What is confirmed working

Each item below was confirmed by an independent check or by every configuration that tested it.

**1. Formats and cryptography.** Wherever a public test vector or real artefact exists, the
engine reproduces it byte for byte:
- the COER encoding, re-encoding every published file (crypto #1);
- HashedIds and Time32 with leap seconds (#5–8);
- ECDSA P-256/P-384, ECIES, AES-CCM, KDF2 and MAC1 (#52–61);
- butterfly expansion and linkage values (#63–70);
- the six published CTLs (#78).

On `88e24b09` the oracle re-encoded all 54,446 recorded PDUs canonically (DECODE: 16,131 on
conf, 38,315 on mbr) and verified every signature with its own ECDSA.

**2. Certificate profiles per provider, now checked against the published chains.**
- provider-a takes ISS's published form: a 70-year root, the root as CRACA, the CRL signer
  root-issued on series 256, and the region on sub-CAs.
- provider-b takes SAESOL's production form.
- `component_certs.py` decodes every component certificate from a run's provisioning SPDUs
  with the oracle's codec and compares it with the published certificate of its role: 10 of
  10 roles have the same fields and the same encoded size of every field but the name (rerun
  §8).
- The oracle's CRACA-CERT and CRL-SSP rules pass on both forms.
- A governance CRL can revoke a governance sub-CA.

**3. Enrolment (fixed in wave 2, OF-1).** Each device has one enrolment certificate. Its own
ECA exchange issues it, with a back-dated start and the full 6-year lifetime; the DCM
registers it; and it signs every Download-Req. On `88e24b09`: DL-SIGN 2,892 and 10,898 checks
pass, ENR-LIFE 58 and 99 pass, and 0 devices hold a foreign certificate (oracle-final §8.1,
§8.2). The F1 capture has 62 EeRaCertRequests, against 118 when each device had two.

**4. SCMS architecture, in all 34 matrix runs.**
- No separation-of-duties violation, including under the CAMP topology with its LOP.
- Sealed messages are opened only by their recipient.
- A device talks only to its RA and the DCM.
- One provisioning request per OBU over its life (0.54 requests/s scaled to 17 M vehicles,
  against CAMP's 0.5). At full fleet: 40.0 certificates for each device's request, and 1,933
  devices provisioned in 240 s, 48.8 % by the home provider (final-run FB2.devices,
  FB2.provisioning).
- 20 certificates per device at the weekly top-off, and exactly ×3 under the NYC 60-a-week
  profile.
- **The rollover top-up at full fleet:** the pseudonym pool goes from 20 to 39.8 within 10 s
  of the 43,200 s i-period boundary; the home provider issued 12,580 certificates (629
  devices × 20) in those 10 s (FB2.topups-at-rollover).
- I-S1: real and modelled cryptography give identical records.

**5. Message sizes.** provider-b matches CAMP Table 3 exactly: response 454/470, ack 292/294,
ECA response 958. provider-a's sizes (501/517, 304/306, 1,064) are now explained to the byte
by the published ISS certificates: a 12 B `identifiedRegion` on the ICA and every component,
and a 35 B `encryptionKey` on the PCA and the ECA (rerun §8). CRL sizes are 328 B with one
entry and about 33 B for each further entry.

**6. Trust list (fixed in wave 2, R-TRUSTLEAK).** The two kinds of frame the engine sizes
instead of encoding (the RSU's CRL broadcast and a report relayed over the air) carry their
signer's certificate. Every receiver holds them to its installed CTL roots. With provider-b
off the trust list, provider-a receivers verify 0 of its messages (280 before; 0 again in the
`77cffb5b` re-run, realism V6). At full fleet, 2,019,081 SPDUs were checked with 0 refused
(final-bench FB.trust), and the second full-scale run shows no signature, PSID, CTL or CRL
loss cause (FB2.trust). The engine's own CTL bytes pass the oracle's `ctl-files`, with a
3-of-5 quorum (oracle-final §8.4).

**7. Revocation path on the real calendar.** Every case went through the steps the deployment
prescribes:
- the shuffle (at most one day);
- the 8 h review;
- the next business-day 10:00 ET slot, with federal holidays skipped (the Thanksgiving week
  passes);
- the first fetch after publication.

The oracle confirms the CRL rules independently on `88e24b09`: CRL-SEQ, CRL-KEEP, CRL-SLA,
CRL-PUB, CRL-MATCH and RA-BLOCK. Every device the oracle finds on a CRL is an attacker in the
ground truth: 4 of 4 decided on conf, 12 of 12 on mbr.

**8. CRLs from t0, full lists only (fixed in wave 2, R-NOCRL and O-1), and now on the
record (OF-3, fixed in wave 3).**
- Every CRACA signs an empty full CRL at t0 and renews it by `nextCrl`, as a deployment
  running since 2015 does. On conf: `F614/0` at t0, then `F615/0` with the revocations.
- Full CRLs only, with no gap in `indexWithinI`: on mbr 614/0, 615/0, 615/1, 615/2, 616/0,
  each `nextCrl` exactly 7 days on.
- **The records now show what a device holds** (measure §3): each `node.security` row lists
  the signed CRLs the node's receiver verified and holds (craca, series, counter, issue date,
  `nextCrl`), and `pki.flow` carries each device's pre-run CRL download, marked `preload`.
  The engine was right before; `crl_version` counted entries, which is 0 for an empty list.
- **The 2-week overdue rule acts from t0 at runtime.** In the multi-week run with the CRL
  Generator down, the home list issued at t0 goes overdue at exactly t0 + 21 d = 1,814,400 s:
  0 refusals before it, then 45 % of W3's last minute and 48 % of W4 refused, the home
  provider's share (stress `MWC.overdue-onset`, fixed; rerun §7).
- At full fleet, 2,000 receiver installs of the t0 CRLs (2 per device) and 60 RSU installs
  (FB.crl).

**9. CRL freshness over five weeks** (stress multi-week, on `2004bb87`).
- Five i-period rollovers, each signing under the right period.
- 10 CRL versions in 33 days, reissued by `nextCrl`.
- 12,592 conditional polls answered 412, and 0 refusals.
- With the CRL Generator down after its first CRL, every message under that CRACA was
  refused once the list became overdue (87,823 refusals), and traffic recovered after the
  Generator came back. A parked car that returns after four weeks refuses one message at
  power-on and then none (`MWP.overdue-on-return`, pass on `77cffb5b`).
- Re-enrolment: 1 of 75 devices, the expected 1.1.

**10. Downloads and report uploads (OF-2 fixed in wave 2; R-UNSEC fixed in wave 3).**
- Every download rule passes the oracle on both `88e24b09` runs: DL-ACK, DL-ENC, DL-SIGN,
  DL-TIME, DL-NAME, DL-AFTER and DL-WAIT. Conditional GET with 412 is in force, and so is
  `Range` resume.
- Every cellular report upload carries its SPDU. MBR-UP passes 3,930 and 18,618 checks; the
  1,695 uploads with no bytes are gone. A detection that cannot be built as an SM-MBR v1.1
  upload is not filed (`reports_not_filed`).
- **A roadside unit's own detection is not filed under the US profile** (measure §4):
  [SM-MBR] §3 signs a report with the end-entity's pseudonym certificate, its evidence is
  "a BSM from the sender and a BSM from the receiver", and the USDOT RSU Specification v4.1
  gives a unit no detection or reporting function. `reports_from_rsus` is 0 on P-256, and
  the sized-only backhaul reports (583 on conf, 1,114 on mbr) are gone. A unit still relays
  vehicles' reports. `modelled_reports: 1` keeps the old path for research. Under PQ this
  is still wrong (§6 item 10).
- **A report carries the BSMs its reporter judged** ([SM-MBR] §2.3.1.2.1, §2.3.1.3.1): the
  US profile's consecutive pair, matched by generation time. Before, it attached the
  accused's last two BSMs on the air when the report was built.

**11. Retries and outages.**
- The EE retries after 10 s, at most 10 times an hour. With an RA outage across the top-ups,
  12 of 12 complete.
- A message between components waits for a component that is down, and the component's timers
  run when it restarts.
- A device that is switched off reaches no part of the SCMS (F-OFF), with one exception at a
  fast-forward boundary that is not root-caused (§6 item 8).

**12. Receiver rules.**
- 1609.2 receiver validation rule by rule (veh R2–R9, R12–R18), and the CTL chain (R1).
- The device checks the CTL's elector quorum.
- A composite CRL is kept when one of its CRLs is unverifiable.
- Uncompressed explicit keys are accepted.
- The `generationTime` relevance window is enforced, and a handset now applies it when it
  takes a frame, not when the frame arrived (measure §5). That was the cause of the 43,125 s
  old evidence: frames that waited in a handset's inbox across a fast-forward were judged
  12 h later.
- One car with a 5,000 s clock error no longer gets anyone honest revoked (19 → 0).

**13. The US misbehaviour profile at quarter density (R-USFP fixed in wave 2, R-FS40 in
wave 3).**
- **R-USFP.** The raw 10 Hz GNSS fixes carried fresh 1.6 m white noise each. The US presets'
  receivers now output a smoothed navigation solution (τ 100 s) with a 0.3 m/s static hold.
  On the realism base, same seed (usfp §3): honest vehicles revoked 26 → 0; attackers revoked
  1 → 2 of 3; reports at the MA 7,488 → 235. `gnss-raw` reproduces the old run to the report.
- **R-FS40** (measure §1). The receiver carried its solution across an epoch by the later
  velocity alone, while the OBU dead-reckons each BSM to its own phase; near a stop two
  consecutive BSMs could then coincide, which ConstantPosition rightly flags. The solution
  now moves by the mean of the two measured velocities (Hatch 1982's carrier-phase delta,
  `gnss/error/gauss-markov` 1.2.0). Monte Carlo of stop-and-go creep: 509 frozen pairs in
  396,000 before, 4 after. At quarter density `constantPositionFrozen` fell 28 → 0 and
  reports 54 → 15 (final-run §2).
- In all 34 matrix runs on `77cffb5b` no honest device is revoked, except `gnss-raw` (26, the
  fault by construction). att10 revokes 4 of 9 attackers and sybil 2; every vehicle attacker
  heard by at least 3 reporters is revoked.
- The ConstantPosition check is as SM-MBR v1.0 §2.3.1.2.3 specifies, and the static hold
  cannot trigger it (measure §1).

**14. Metrics on the realism base.** All are inside their sourced bands:
- PDR 0.364;
- BSM latency p95 14.6 ms (budget 100 ms);
- CBR p95 1.4 %;
- AoI 82 ms;
- security at 46 % of BSM bytes;
- full-certificate share 17 % (re-measures veh S6);
- certificate pool mean 25.7;
- no signature, parse or backend failures, and no starvation.

**15. The full-scale benchmark, run to the end** (FB3 on `fe3757b9`; bench §000,
full-bench §4). The committed file (full fleet, 172,800 s, seed `0xBE7C_0001`) completed
with rc 0 for the first time: 15,255 s of wall time awake, 24,786 CPU-s, **peak 2,105 MB**
(FB2 reached 3,109 MB and was stopped at 47,340 s).
- **Radio and latency.** PDR within 100 m 0.829–0.845 in both windows (quarter fleet
  0.858); BSM latency mean 12.39–12.48 ms, p99 14.94–15.07 ms; CBR mean 0.072–0.088 and p95
  0.14–0.18, below J2945/1's `vMinCU` 0.5; 245–279 verifications per node per second, wait
  p99 0.28–0.52 ms.
- **Pseudonyms.** 20 valid at every census until the boundary, 39.9 at 43,210 s (week 615
  valid from the boundary), 0 starved. 1,773 of 1,800 top-ups completed (35,460
  certificates; 27 incomplete, not traced, §6 item 5).
- **Revocation path.** 70,946 hourly conditional CRL polls, all answered 412; 60 RSU
  installs of the t0 lists; 0 CRL entries, because nothing was revoked.
- **Honest traffic at full density opens no case.** 2,083 detector verdicts in 300 s of
  live traffic, 2,001 reports decided, 1,447 real SM-MBR at the two MAs, **0 vehicles
  accused, 0 cases, 0 revocations**. FB2's 71 honest cases were R-FFBURST artefacts, now
  fixed (§4.2).
- **The report shuffle acts as specified.** Reports filed at 130–240 s reached the home MA
  at 86,430 s: the RA's shuffle (CAMP-EE SCMS-765, 10,000 reports or one day) never fills
  in a 2 km² box. A revocation therefore takes at least the day of shuffle, the manual
  review and the business-day CRL. This is a property of the specified system.
- **The fleet is kept across the resume** (1,830 → 1,891; FB2 fell to 1,281), with 0
  teleports, removals or jams, and no report burst (about 67 per 10 s; FB2 had 4,876 in
  10 s).
- **Every fast-forward stop has a real-world cause** (full-bench §2): 69,130 stops at full
  fleet, almost all hourly CRL polls; 11,393 on the quarter file (1,119,908 before) and
  8,310 on the conformance file (2,415,612 before).

**16. Determinism.** Record digests are identical at 1 and 4 threads. That holds on the 33-day
run, on the first 5 s of the full fleet, and on the hybrid ML-DSA-44 run, where
`metrics.json` and `run-report.json` are byte-identical too. The check can fail: three runs
that differ in one setting each give three digests, and so do PQ0, PQHFC and PQHMC
(`099bed7a…`, `a73101dd…`, `b33458e9…`, final-run PQ.digests).

**17. Post-quantum at density** (quarter fleet). These are modelled sizes and costs, since no
standard defines PQ encodings (pq-profile).
- **The Partially-Hybrid certificate cycle (R-CCSIG fixed in wave 3).** A node's own
  messages are signed, sized and priced with ECDSA, as Twardokus et al. (NDSS 2024) design
  it. A receiver pays the PQ check once for each certificate it learns
  (`cert_pq_verifications`). The validator refuses the cycle with a purely PQ scheme
  (ccsig-memory §1). Measured on current code (final-run §2):

  | | ECDSA | Falcon-512 cycle | ML-DSA-44 cycle | ML-DSA-44 on every SPDU |
  |---|---|---|---|---|
  | BSM PSDU mean | 207 B | 380 B (was 1,045) | 814 B (was nothing sent) | 1,332 B, 2.30 frames per BSM |
  | BSM airtime, 60 s | 13.6 s | 23.2 s | 47.5 s | 176.2 s |
  | Verified messages | 380,664 | −2.1 % | −5.7 % | −33.8 % |
  | BSM e2e mean | 3.72 ms | 3.96 ms | 4.57 ms | 12.06 ms |

- Hybrid ML-DSA-44 on every SPDU: CBR 0.125. Hybrid Falcon-512 on every SPDU: airtime ×4.4.
- **RSUs learn the CRL under PQ** (R-PQRSUCRL, fixed in wave 3): 60 RSU installs in both
  cycle runs (0 before).
- R-PQMBR is fixed. An over-MTU frame is refused and counted as refused, not as uploaded.
  Under `generic-sdu` it is fragmented.
- Hybrid top-ups complete after the RA's request shuffle. Nobody starves.

**18. Speed.** One check redid a P-256 multiplication for every pseudonym at every backend
step. With a memo, the realism base went from 2,074 s to 164 s with identical results
(31–41 s on `77cffb5b`). `pq-base` went from 1,175 to 125 CPU-s.

**19. Leaving a fast-forward** (finalize 4, ff §1–§2). The first live step after a frozen
window no longer reads the frozen hours as elapsed time: the traffic, the demand and every
GNSS receiver resume as if no time had passed (tested state for state, and to 1e-9 m for
50 receivers), and `ped_wait` and `pseudonym_change_rate` leave the pause out.

**20. Memory.** Exact compacted metric samples, a 24-byte parked link-stream map, one
shared decoded copy of each governance certificate (interned by SHA-256) and allocator
relief halve the 100 s full-fleet peak (2,074 → 1,027 MB) with an unchanged stream digest
(memden §1). The whole two-day run peaks at 2,105 MB, under the 2.5 GB target.

## 4. Verdicts changed in this round

### 4.1 Fixed (16 rules since 2026-10-18)

No rule verdict changed in fix wave 3, final runs part 2, finalize 4 or the full bench.

**Fixed in fix wave 2.** These are the five rules reopened at `0f7eda5c`.

| Row | Before | Now | Evidence |
|---|---|---|---|
| proto C6 (the RA serves only registered enrolment certificates) | OF-1: the RA answered Download-Reqs signed by an enrolment certificate it never registered (112 on conf) | One certificate: the ECA exchange's own, registered by the DCM, signs every Download-Req | `d7b64fae`, `89f54fe4`, `3471f2fc`; oracle DL-SIGN on `88e24b09` 2,892 / 10,898 pass (was 1,320 / 5,245 red); `oracle_final.rs::one_enrolment_certificate_…` passes, and its logic is red on the `4924a5d2` recording |
| proto C8 and gov E-1 (enrolment certificate valid 6 years) | OF-1: a second certificate lasting 0.9–5.5 years; age modelled as a short lifetime | Back-dated start, full 6-year duration, clamped to the ECA's start. A late joiner's pre-run exchange runs at that start (merge fix `3471f2fc`) | ENR-LIFE 58 / 99 pass on `88e24b09` (was 56 / 97 red); `enrolment::an_aged_enrolment_is_the_one_certificate_its_eca_exchange_issued` |
| proto F6 (each report is POSTed through the RA with its Download-Req) | OF-2: 1,695 of 2,991 cellular uploads on conf carried no SPDU | A detection that cannot be built as an SM-MBR v1.1 upload is not filed. Every upload carries its SPDU | `e8475281`; MBR-UP 3,930 / 18,618 pass (was 1,695 / 49 red); `oracle_final.rs::every_cellular_report_upload_carries_its_spdus`, red in Python on the `4924a5d2` recording |
| veh R1 (the signer chains to a root on the installed CTL) | R-TRUSTLEAK: 280 provider-b messages verified with provider-b off the CTL | Sized frames carry the signer certificate, and OBU, RSU and VRU receivers validate it to the installed roots | `89f54fe4`; off-ctl 280 → 0 (BS wave 2; 0 again on `77cffb5b`); `d18a1e80` proved red at the engine (45 unchecked) and at the RSU. No fault was injected into the OBU or VRU fallback |

**Fixed at `2004bb87`** (the PKI, flows and policy wave):

| Row | Before | Now | Evidence |
|---|---|---|---|
| crypto #4 (CF-UNCOMP) | Explicit certificate with an uncompressed key refused on the node path | Accepted after an on-curve check, then stored in compressed form | `22f7cf13` |
| crypto #36 (CF-CRACA; also #24's issuer) | ICA as CRACA for every provider; the governance CRL could not revoke a sub-CA | Per-provider profile: ISS uses the root as CRACA and a root-issued CRL signer on series 256; S2X uses the ICA. Governance follows the profile | `pki_profile::per_provider`; oracle CRL-SSP and CRACA-CERT on both forms |
| crypto #29 (was unverifiable) | Enrolment `cracaId` 000000: a series-4 CRL could never match | The ECA names the provider's CRACA, so a series-4 hash CRL revokes the certificate ([SMP] Table 1) | `22f7cf13`; oracle CRACA-CERT series 4 |
| proto B6 | No EE retry | 10 s, at most 10 an hour (SCMS-522/523); the RA re-acknowledges a byte-identical retry | `5d44674b`; edge 3c; RA outage 12/12 |
| proto D18 | No `Range` resume | A cut batch download resumes; the retry is answered 206 with the rest | `ee_retry.rs::an_interrupted_batch_download_resumes_with_range_for_the_rest` (proved red) |
| proto I7 | 40 B per CRL entry; CTL left out | The encoded `CompositeCrlSpdu`, home CTL included | `5d44674b`; edge: about 33 B an entry, 336 kB at 10,000 entries |
| proto I10 | No `If-Modified-Since` | Every due poll is a conditional GET, answered 412 (92 B, ASSUMPTION) when the copy is current | `report_signing.rs::every_due_crl_poll_asks_and_a_current_copy_is_answered_412` |
| gov L-1 | 30-year root for every provider | ISS form: a 70-year root `v2xrootca.ghsiss.com`. The 20-year key cryptoperiod is still not enforced; no run reaches it | `22f7cf13` |
| veh M2 | Detections with no SM-MBR v1.1 subtype filed | `detect/us-sm-mbr` files v1.1 subtypes only, and it is the default of every US real-world preset | `c5d5c5b7`, `7aa31661` |
| veh M3 | Research thresholds (12 m/s²; no MaxSpeed) | US profile: MaxSpeed 90 m/s, MaxAcceleration 10 m/s², RandomPosition 1 m. With the smoothed receiver (R-USFP), it revokes no honest vehicle on the realism base | `c5d5c5b7`; usfp §3 |
| veh M6 | Reports about messages that failed verification | The US profile judges only BSMs this receiver verified (SM-MBR §2.3.1) | `us_mbr.rs::only_verified_bsms_are_judged_or_remembered` |

Narrowed but not fixed: **proto A16.** The poll is on the air; the `infoStatus` SPDU body is
still not encoded.

### 4.2 Fixes that change runtime behaviour but no checklist row

Each was fixed and proved red, as its report describes.

**In finalize 4** (BS fin4):
- **R-FFBURST** (ff §1): both causes fixed. Mobility clocks (standstill, lane change, entry
  queue, stop end, the next arrival) counted the frozen window, so a vehicle standing at a
  red lamp had "stood" 42,930 s and the gridlock resolution teleported or removed it; and
  the first GNSS epoch after the window had `dt` 42,930 s, so the per-second outlier and
  burst hazards were 1 and every vehicle jumped 12 m. `Mobility::resume_after_pause` and
  `GnssModel::resume_after_pause` move those clocks on by the pause.
- **`ped_wait` and `pseudonym_change_rate`** across the window (ff §2): `MetricProvider::on_pause`
  removes the pause from the wait and from the observed vehicle-hours.
- **`FastForwardReport::stops_by_cause`** and `traffic_paused_ns`: every stop is labelled by
  its wake source. This is what showed R-FFSLOW gone (full-bench §2).
- **Memory** (memden §1.2): compacted metric samples, parked link streams, shared governance
  certificates and hardware profiles, allocator relief. Results and digests unchanged.
- **The benchmark's apex turn rule** (memden §2.2): junction connectors were capped for
  their average radius, so apexes were driven at 8–45 m/s² and receivers saw position
  jumps across turns. The benchmark now judges the tightest 6 m. `positionJump` 123 → 35 in
  the first 100 s at full fleet. This is a scenario setting and changes inherited traffic
  (§2.2).
- **The two engine tests** `scms_wiring` and `pq_relay` (§6 item 9).

**Settled as not engine defects** (ff §3–§5, full-bench §2):
- **R-FFHOLD.** The reports held across the first fast-forward were in the RA's report
  shuffle, whose one-day timer is a kernel delivery that the fast-forward does wake for. A
  frozen road files no reports, so the shuffle waits; in FB3 it released at 86,430 s.
- **The rollover's pseudonym count.** A certificate is valid for the week plus one hour,
  and CERTCHG needs 5 minutes and 2 km; a gridlocked vehicle in a 60 s window changes
  nothing before 46,800 s. FB3 counted 0, as expected.
- **R-FFSLOW.** The dense stepping does not reproduce on `fe3757b9` (8,310 stops on the
  conformance file, 2,415,612 before). Which commit removed it was not bisected.

**In fix wave 3** (BS wave 3):
- **R-CCSIG** (ccsig-memory §1): the Partially-Hybrid cycle signs a node's messages with
  ECDSA and checks the PQ signature once per certificate. It reverses `45f0942f` (owner
  decision applied). Faults F1 and F2 red.
- **R-PQRSUCRL** (ccsig-memory §3): RSUs fetch and install the CRL under PQ. Fault F3 red.
- **Per-node memory de-duplication** (ccsig-memory §2): shared ACA and pseudonym certificate
  `Arc`s, `checked_at` not stored under the US profile, and UsSmMbr evicts senders after
  30 s (ASSUMPTION). About 10–15 % of the live heap; results unchanged. Faults F4 and F5 red.
- **R-FS40** (measure §1): the smoothing receiver propagates by the mean velocity (§3
  item 13).
- **The privacy run report** (measure §2): `privacy_links_claimed` counts only claims across
  a pseudonym change; new `privacy_changes_heard` and `privacy_links_spurious`. The
  `linkability` metric was already right.
- **OF-3** (measure §3): held CRLs and the pre-run CRL fetch are recorded (§3 item 8).
- **R-UNSEC** (measure §4): an RSU's own reports are not filed under the US profile (§3
  item 10).
- **The old evidence** (measure §5): a handset applies the relevance window at processing
  time, and a report carries the BSMs its reporter judged.
- **M-RX1** (rerun §3): the metric no longer blames an SDU's fate on a fragment lost after
  it. The engine was right; the check was wrong.
- **Six sensitivity rows re-stated** on their measured causes (rerun §1–§2).

**In fix wave 2** (BS wave 2):
- **R-USFP**: the smoothed GNSS receiver in every US preset (§3 item 13).
- **R-NOCRL**: every CRACA publishes at t0, and devices join holding those lists.
- **O-1**: full CRLs only; the deltas were signed and never sent.
- **R-PQMBR**: over-MTU application frames are refused and counted, or fragmented under
  `generic-sdu`.
- **The late joiner's pre-run exchange** (merge fix `3471f2fc`). Before it, the RA refused
  34 of 125 joiners' aged certificates as not yet valid.

**At `0f7eda5c` and before:**
- The MA ran one case at a time. Its cases now run side by side (R-SERIAL, gap row 27).
- The CRL Generator reissues by `nextCrl` when no entry has been added.
- Devices install a newer CTL.
- The node clock reaches security.
- No signing with an expired pseudonym across the rollover (edge 1b/1c).
- F-OFF: a switched-off device reaches no part of the SCMS.
- The AoI fast-forward charge (matrix fix 2).
- The pseudonym usability memo (matrix fix 1).
- Skewed dead-reckoning (07 §8.1).
- Fast-forward stops: 255,658 → 4,290 (R-FF).
- The PKI-record defects R-LOP, R-SHARE, R-CASE, R-CORR and R-OFFCTL, and the two merge
  fixes in 07 §5.

**On the oracle side** (`1637acb3`). Two oracle defects were fixed. The `ctl-files` faults
could never fail on the engine's capture, and the CRL faults could not reach the t0 CRLs
(oracle-final §8.5).

## 5. Every documented deviation and what it changes (22)

### Cryptography and data formats (4)

| Row | Difference from the real system | Impact |
|---|---|---|
| crypto #10 (CF-IPERIOD); also gov K-5 | i-periods start on the UTC 09:00 grid, not on a uniform Time32 grid from the epoch | 2 s on every pseudonym `validityPeriod.start`. The two validators read the sources in opposite directions. **Owner decision** |
| crypto #30 (CF-ENR-REQ) | The enrolment request carries `crlSeries` 4, which breaks the ASN.1 `(0)` | Follows [SMP] §6.3.1, which overrides the ASN.1. Same bytes on the wire. No trust effect |
| crypto #35 (CF-RSU-SSP); also proto E7 | RSU application certificates carry no SSP | Probably not a deviation: gov E-8 finds no SPaT, MAP or TIM SSP in CAMP Table 1 (§8 item 1) |
| crypto #38; also veh R11 | The published RA permission group breaks the strict chain-length rule. The engine accepts it, as deployed devices do, and counts each acceptance | A real-world quirk the engine reproduces on purpose |

### Protocols and backend flows (3)

| Row | Difference | Impact |
|---|---|---|
| proto A16 (narrowed) | The `infoStatus` SPDU body (crl/ctl/caCcf/ma/ra dates) and its RA signature are not encoded. The device polls the composite CRL conditionally instead | A change costs one exchange instead of two. Poll bytes are close to real |
| proto E7 | Same as crypto #35 | §8 item 1 |
| proto H10 | `indexWithinI` saturates at 255; a real CRL Generator would have to stop issuing | Only reached with more than 127 publications in one week |

### Governance, deployment and calendar (10)

| Row | Difference | Impact |
|---|---|---|
| gov G-2 | The board's review and investigation delays before a vote are not modelled | None on CTL rules, which start at the vote |
| gov G-10 | No scenario key schedules the yearly Test Ballot | A run longer than a year has no test-ballot record |
| gov G-11 | Post-quantum elector key replacement is not modelled | None today. No method has been chosen |
| gov C-9 | CTL `effectiveDate` is the publication deadline, not the ballot date | 0–28 days late. No trust effect: devices do not check it |
| gov C-10 | A run's first CTL is sequence 0; the live production CTL is sequence 1 | Sequence numbers run one lower. No trust effect |
| gov C-11 | A CTL that drops a root does not also carry the dropped root's certificate | Bytes only |
| gov C-14 | The couriered flash drive and the video-call key confirmation are not modelled | Not on the network |
| gov L-8 | Root termination (365 days' notice) is not modelled; removal is | Only a run of a year or more |
| gov L-11 | The LA SSP is version 2 with `laId` 1 and 2; ISS's LAs carry version 1 with ids 4 and 5 | `laId` is the provider's own choice |
| gov K-5 | Same as crypto #10 | 2 s on validity starts |

### On-vehicle and roadside behaviour (5)

| Row | Difference | Impact |
|---|---|---|
| veh R10 | Country regions are not checked against a location; they are only counted | None for US pseudonyms, which carry no region (`region_unchecked` = 0) |
| veh R11 | Same as crypto #38 | Counted |
| veh C6 | "2 km since the last change" is measured as distance travelled, not straight-line distance | On a winding route a change can come slightly early |
| veh M11 | Relayed report uploads go on safety channel 172, not on a service channel advertised by WSA | Report airtime on 172 is an upper bound (0.21 % at full fleet in the first run). The radio has no per-channel MAC queues and no channel switch (07 §7 item 4) |
| veh U5 | No WSA encoder | The SCMS service cannot be advertised, so M11 cannot be fixed yet |

Fourteen of these change nothing a device trusts or does: crypto #30, #35 and proto E7, crypto
#38 and veh R11, crypto #10 and gov K-5 (2 s), and gov G-2, G-11, C-9, C-10, C-11, C-14 and
L-11. The other eight are proto A16 and H10, gov G-10 and L-8, and veh R10, C6, M11 and U5.
Each of them matters only to long runs, to bytes on a poll, or to where report airtime is
counted.

## 6. Runtime defects and findings with no checklist row

In order of impact. Each has its evidence in the named report. Items fixed or settled in
this round are in §4.2. None of the open items below changes a rule verdict. Item 1 decides
whether a full-fleet misbehaviour rate is a real-world number.

1. **Honest reports at full density** (full-bench §4.1, memden §2). Open; explained in
   part.
   - FB3 filed 2,001 reports in 300 s of live traffic, all about honest vehicles, against 50
     on the quarter fleet: 40× the reports for 6.3× the devices and 3.3× the live time.
     None became a case (0 vehicles accused; 67 report-level false positives), so no honest
     device is revoked, but the volume is super-linear in density.
   - **implausibleAcceleration** (1,249). Honest emergency stops at 9–10 m/s² plus the
     receiver's 0.1 m/s Doppler noise cross the US profile's 10 m/s² MaxAcceleration on two
     consecutive speeds, and each stop is heard by about 25 receivers that each file. That
     is a property of the US rule, but the rate is the engine's emergency-braking rate,
     which is not calibrated against naturalistic driving data. One sender (node 874)
     snaps from 1.23 to 0.00 m/s in 100 ms: car-following or the 0.3 m/s static hold, not
     root-caused.
   - **constantPositionFrozen** (567, against 1 on the quarter fleet). full-bench reads it
     as standing vehicles in gridlock (median speed 0 from 200 s). At 0–100 s it was 2–6
     (memden §2.1–§2.2), and measure §1 found the static hold cannot trigger the check, so
     why it rises in gridlock has not been checked one verdict at a time.
   - **positionJump** (267). No pair as sent breaks RandomPosition: every verdict compares
     BSMs that were not consecutive on the air (receiver gaps under load and `prioritized`
     verification, with `max_gap_s` 0.5 s an ASSUMPTION). The junction-apex cause is fixed by
     the turn rule (§4.2); through vehicles following lane polyline vertices with no
     curvature limit (near Broadway) are still open.
   - **Impact:** a full-fleet false-positive count is the engine's, not yet a real-world
     rate, until the braking rate is calibrated and the polyline turns are fixed.
   - **2026-10-24 (honest-density.md):** the braking target is calibrated against the
     100-Car study (DOT HS 810 593: ≤ 0.28 decelerations of 0.52 g a vehicle-hour, 0.019
     crashes and near-crashes). The node-874 snap is the standstill-gap advance cap
     (−8.5 m/s² on a van, over its class limit), fixed behind `traffic.standing_leader_stop`;
     lane-polyline bends get the turn-speed rule behind `traffic.lane_bend_speed`; the
     benchmark sets both. The `constantPositionFrozen` sample is still owed (it needs the
     full fleet past 160 s); the velocity-noise hypothesis is written there.
2. **R-FFWIN: a window that ends where a fast-forward starts is reported late** (FB3
   `metric-window-after-ff`). **Fixed 2026-10-24 (backend wave):** `Engine::next_metric_flush`
   closes a live window where a fast-forward begins, the frozen span as a window of its
   own where it ends, and the next live window on the period grid; `mac.cbr` spans are
   each window's own. Test `calendar.rs`
   `a_metric_window_that_reaches_a_fast_forward_is_reported_at_its_own_end` (red on
   `e15bfec1`: no samples at 25 and 45 s). The original description: The 230–240 s window is reported at 43,170 s and the
   43,220–43,230 s window at 172,800 s, with rates divided over the frozen hours
   (`verify_rate` 115.7/s, CBR 0). Those two samples are left out of every table. An
   analyst reading `metrics.json` directly would see them.
3. **Full-scale run cost** (`FB3.completes`, open).
   - The 60 s rollover window took 4,767 s (79 wall-s per simulated second, against 35 in
     the first window). Not profiled. The weekly pre-generation at 43,200 s is one
     synchronous burst (57.8 MB of backend traffic in 10 s; home kernel steps 1,266 →
     13,854 in that minute), which a real RA spreads; that is the likely cost, not traced.
   - Each fast-forward stop costs O(fleet): 29 ms at 1,945 nodes against 3.5 ms at 327.
     The stops are real work (hourly polls), so the remaining fix is the event-indexed
     wake scan (ff §5, part 2).
   - **Impact:** a two-day full-fleet run takes about 4.2 h awake on the owner's laptop.
     The 2×/4× fleet, LTE and Portland variants are still not run.
   - **2026-10-24 (honest-density.md §6), profiled at quarter fleet:** the rollover is the
     weekly pre-generation's real P-256 work on one thread (the explicit-certificate
     re-verification, 12 % of it, is now remembered, `419b8e6b`); each fast-forward stop was
     dominated by the backend view's O(fleet²) `ops_of`, now a key-range read (`a66aa58c`).
     Not re-measured at full fleet.
4. **Full-scale memory: what is left.** The target is met at 2,105 MB. The largest live
   structure is the SCMS backend's per-device state (113 MB of RA, LA and PCA records and
   batch files plus 58 MB of enrolment for 970 devices at 40 s, memden §1.1). Dropping it
   changes what the MA's investigation and re-downloads rely on, so each part needs an
   owner decision on what the RA and PCA keep.
5. **Untraced counts in FB3.** 268 of 1,715 uploaded reports never reached an MA (1,447
   received), possibly because `reports_uploaded_relay` counts a report put on the air, not
   one delivered (item 9). **Traced and fixed 2026-10-24 (backend wave)** at its cause: a
   relayed report was counted and dropped from the store when it went on the air, heard or
   not. On the 60 s relay grid of `security_lifecycle.rs` that lost 32 of 89 (`e15bfec1`:
   89 counted, 57 at the proxy). Now a relayed upload is confirmed by the
   `report-forward` unit that takes it onto its backhaul (`Phase2::relay_delivered`), an
   unconfirmed one stays stored and goes again on an RFC 6298 timer (1 s doubling to 64 s),
   and the forward dedup is per frame: 88 of 89 delivered, 88 counted, 1 stored at the end.
   FB3 itself (4.2 h) is not re-run. The 27 top-ups are not traced (they need the
   full-scale rollover window); QD completes 278 of 278. 27 of 1,800 top-ups did not complete. 255 reports were not filed
   (212 with too few BSMs on hand, 43 by RSUs that hold no pseudonym certificate, which is
   the R-UNSEC rule acting).
6. **Privacy at full fleet is still unmeasured.** FB3 counted 0 pseudonym changes and 0 link
   claims, as expected for a 60 s gridlocked window (ff §3). A live window that reaches
   46,800 s, or the NYC pilot rule, is needed for a full-fleet linkability number.
7. **R-JOINCRL: a car that joins mid-run holds only the t0 CRLs** (rerun §7, §9 item 1).
   **Fixed 2026-10-24** (`2a14d6c4`, oracle-24 §3): `phase2.rs` `prerun_crl` gives a joiner
   the newest list each provider published before it joined, with a unit test. Found
   independently by the new oracle rule OF-3-JOIN (35 stale joiner fetches on conf, 108 on
   mbr); after the fix both captures pass every check. The `phase2-path` golden moved
   (record `e54f1c23` → `fd19a3ed`, records 6,372,698 → 6,372,470); reverting the fix
   reproduces the old digest. The multi-week runs that showed it (MWC W5, MWP, and the base
   MW run of rerun-b with 16,618 refusals from 33 joiners) are owed again on the fixed code.
   The original description:
   Late in a long run those lists are overdue, so the joiner refuses its home provider's
   messages until its first download (2–49 s): 755 refusals at 4 joiners in MWC's W5, and
   18,617 at 27 joiners in MWP. It should hold the newest list each CRACA published before
   it joined, as `preload_at` does for certificates.
8. **A switched-off car records CRL downloads** (`MWP.off-no-backend`, reopened; rerun §7).
   Node 20 is off from 645 s to 2,419,510 s, but `proto.revocation` records it downloading,
   processing and enforcing three CRLs at 604,740.2 s, the first live step after a
   fast-forward. **Fixed 2026-10-24 (backend wave):** the roadside CRL broadcast's
   payload was installed by `Engine::on_app_message` on a receiver whose node was off
   (the node itself refused the frame as `receiver-off`); a CRL download landing on a
   device switched off since it asked is now also not received. Test `backend_entities.rs`
   `a_switched_off_vehicle_installs_no_list_a_unit_broadcasts` (red on `e15bfec1`).
9. **The two engine tests that failed after BS wave 3 pass again** (finalize 4 TESTS,
   merge `2f6c664d`; owner decisions applied; no assertion changed, no rule changed). Both
   pass on HEAD's code (§2.2).
   - `scms_wiring::detectors_file_real_us_reports_that_the_ma_opens_and_verifies` runs the
     scenario's own 300 s instead of 120 s. With RSUs filing no report (R-UNSEC), the first
     vehicle report is filed at 108 s by one car; in 300 s 105 real reports from 12 cars
     reach the MA, every one opens and verifies, and the MA revokes at 186.8 s. The test
     uses a `NullRecorder` (300 s in memory passed the 2.5 GB cap).
   - `pq_relay::an_oversized_relayed_report_or_crl_frame_is_fragmented_and_acted_on_whole`:
     `scms-pq-relay-repro.yaml` adds one ConstPos attacker (actor 112, after the base's 3 %,
     whose draws are unchanged) on the road of relay-only node 56, 24 m from a unit: 36
     relay uploads, 30 vehicle reports at the proxy through a unit. Moving a unit next to
     the old reporter (node 82) did not work: the unit's CRL broadcast reached the car
     first, and it stopped reporting the revoked attacker.
   - Both proved red (one build): the unit dropping every reassembled report fails the
     relay test ("no vehicle report reached the proxy through a unit"), and the judged
     evidence taken from another signer fails the wiring test ("no real US report was
     filed"). pq_relay 3/3 and scms_wiring 4/4 pass with the faults removed.
   - Finding, **fixed 2026-10-24 (item 5)**: a relay-only vehicle counted a report as
     uploaded once it was on the air to a unit within the geometric 300 m
     `relay_range_m`, whether or not any unit heard it. Node 82's 4 "uploads" (12 fragments) all fell below sensitivity at units 212
     and 218 m away behind buildings. The same car, out of the units' radio range, got its
     first CRL at 95.2 s, 60 s after the MA revoked its target (35.6 s), and reported the
     revoked attacker from 65.9 s to 72.8 s. `reports_uploaded_relay` is "put on the air",
     not "delivered". **Impact:** relay upload counts are an upper bound (FB3's 268 lost
     uploads may be this, item 5), and a relay-only car out of every unit's radio range
     learns a revocation late.
   - The relay repro now revokes 3, not 2. `pq-profile.md` still quotes the old numbers and
     is stale.
10. **R-PQUNSEC: under PQ an RSU's own reports are still filed** (final-run §3). PQHFC files
    7 and PQHMC 9. `Phase2::real_mbr` returns `Off` when `pki` is `None`, before the R-UNSEC
    roadside check. The fix is one line. It was held because it could leave the `pq_relay`
    test's MA without a case; that test now has a vehicle reporter of its own (actor 112,
    item 9), so the fix may no longer empty it. Not tried; owner decision.
11. **Post-quantum path, modelled only** (pq-profile §3; final-run §3).
    - RSUs have no application-certificate cycle under PQ.
    - Receivers check CRL entries against the Generator's list, not a signed CRL.
    - Every top-up is a fresh request (there is no PQ butterfly).
    - VRU handsets priced ECDSA whatever the scheme. **Fixed 2026-10-24 (backend wave):** a
      handset signs and verifies with the scenario's scheme, checks each learned
      certificate's PQ signature in the Partially-Hybrid mode, and its profile carries the
      Cortex-A72 liboqs anchor its ECDSA figures are proxied from (tests
      `pq_receivers.rs`, `rx_validation.rs`).
    - Both cycle runs showed 47 `Invalid` verification states. **Root-caused and fixed
      2026-10-24:** they were the first PSM of each handset, judged by roadside units
      before their first own step. A unit with no application certificate (the PQ profile
      gives it none) held its CRL gate at i-period 0 until that step and refused each
      linked certificate as an implausible period. The gate is now set at the unit's
      creation. PQHFC re-run: 0 `Invalid` (Verified 373,181 to 373,228). Test
      `pq_receivers.rs`, red with the fix removed (47).
    - No standard defines PQ encodings, so no PQ number is a real-world number.
12. **The enrolment certificate name** (rerun §8). Every enrolment certificate is named
    `ee-<device>.provider-a`, including the 46 that provider-b's ECA issues. The name is
    still unique per device, so crypto #28, proto C3 and gov E-3 hold, and the size is
    unchanged. Fixing it moves certificate bytes and the regression digests.
13. **An overdue-CRL refusal is labelled `signature-invalid` on `node.rx`** (rerun §9 item
    3). In MWC's W4 the run report counts 97,679 `crl-overdue` refusals and `node.rx` has
    exactly 97,679 `signature-invalid` fates. An analyst reading `node.rx` would see forgery
    where there is a stale list. **Fixed 2026-10-24 (backend wave):** a receiver-rule
    refusal is `RxDisposition::Refused(rule)`, and `node.rx` names it `crl-overdue` or
    `receiver-rule` (chain, validity, region, PSID); the message stays `Invalid` to the
    applications. Test `rx_validation.rs`
    `a_receiver_rule_refusal_is_reported_as_the_rule_not_as_a_bad_signature`.
14. **The US profile cannot report a handset** (usfp §3). SM-MBR v1.0 defines reports for
    the BSM (PSID 0x20) only, so a PSM-sending attacker goes unreported. This is a gap of
    the real profile, which the engine reproduces faithfully. The research suite, with
    `modelled_reports: 1`, catches it: 3 of 3 revoked.
15. **Unexplained, not yet shown to be defects:**
    - **Fast-forward ticks.** 865,653 in the edge peer-root-removed run (an older binary).
      The dense stepping of the same class (R-FFSLOW) no longer reproduces on `fe3757b9`
      (§4.2); this run was not repeated with `stops_by_cause`.
    - **Case-board review time** (realism L5). The board shows 28,799.95 s on `77cffb5b`,
      against the expected ≥ 28,800 s. The review clock starts at report receipt and the
      case's corroborated stamp comes up to a second later.
    - **O-2.** Four Saturday decisions on mbr are late under the strict reading of [SM-PROV]
      3.4.10, which does not say which reading applies.
    - **The generic reassembler's 64-set buffer** fills in the dense padded fragmentation
      run and refuses new sets (11,607). This is a real effect of the fragmenter's default,
      not a defect (rerun §3).
16. **Outside the SCMS:**
    - LTE-V2X BSM p95 latency is 112 ms, over the 100 ms budget, because the BSM is not
      aligned with its SPS reservation (matrix `lte-latency-budget`).
    - Rebroadcasting a 336 kB standing CRL every 5 s on channel 172 costs about 0.54 Mbit/s per
      RSU (07 §7 item 5). This is a configured behaviour, not a defect.
    - **The 5 failing mobility traffic tests** (re-run on `d0e6530b`, finalize 4 TESTS; none uses
      GNSS or the SCMS, and none is weakened). Two are calibration bands, three are real
      mobility defects.
      - `calibration::a_saturated_stop_line_discharges_at_the_published_rate`: start-up lost
        time 3.011 s over 49 queues against the HCM band 1.0–3.0 s. A model gap, not a
        bug: the green-onset reaction (1.08 s median) and launch give about 1 s more than
        HCM's 2 s default; a measured re-fit is owed and moves every digest.
      - `calibration::drivers_and_pedestrians_on_a_midtown_grid_move_as_published`:
        pedestrian speed p15 0.925 m/s against Knoblauch's 0.97–1.40 (mean 1.246, in band).
        The metric samples every walking step (pavements, crowding, starts and stops)
        against crossing speeds measured on crosswalks, and the default desired speed is
        Helbing's 1.34 ± 0.26 m/s (p15 about 1.07). A measurement-population mismatch to
        settle (sample crosswalk steps only, or take Knoblauch's population), not shown to
        be a model defect.
      - `pedestrian_invariants::pedestrians_and_cyclists_in_signalised_traffic_hold_every_invariant`:
        overlap 42 (two actors on adjacent lanes 207/208 closing from 4.8 to 2.65 m
        centre to centre) and in-building 4 (a walker inside building 12's corner). Real
        defects.
      - `pedestrian_invariants::observed_pedestrians_and_two_wheelers_hold_every_invariant`:
        overlap 14 (lanes 398/399, the same closing pattern), one speed jump (4.68 to
        3.77 m/s in 0.1 s) and one acceleration bound (−9.15 m/s²). Real defects.
      - `realism::buses_taxis_and_deliveries_stop_at_the_kerb_and_the_traffic_goes_round`:
        standstill 1 with kerbside stops against 0 without (actor 79 on lane 39 stands
        180 s). Overlaps now pass. A real defect; the cause recorded on 2026-10-11 (a car
        held part-way through a lane change beside a standing delivery van blocks the lane
        behind it) was not re-checked for actor 79.
    - **Two more pre-existing failures, not in the earlier baselines** (BS fin4; both fail
      identically on `747a0773`, before the finalize 4 merges; neither touches the SCMS):
      `v2xw-demand transit::a_bus_serves_its_stops`, and `v2xw-cli longrun_budget`, whose
      slice cost grows from 2.26 s to 10.7 s. Neither is root-caused or bisected.
17. **Red-team finding.** The `generationTime` window makes a replay older than 2 s harmless,
    but the replayer is never reported or punished: 11 of 11 replayers were revoked before the
    window, and 0 after it (07 §8.1).
18. **Report airtime over its bound** (realism A1, open; rerun-a §3). On the 300 s base
    misbehaviour reports are 1.25 % of the airtime against a 1 % bound: 587 relayed uploads,
    all about attackers, from 66 reporters, about one report every 6 s per reporter per
    pseudonym. No public source sets an SM-MBR v1.0 report rate, so the bound itself is
    uncalibrated.
19. **Findings of the 2026-10-24 re-runs** (rerun-b §6), none an engine defect yet shown:
    - MWC2 now refuses exactly what MWC does (both runs' revocations are provider-b's), so it
      no longer tests a late outage, and `MWC.revocation-blocked` has nothing to block.
    - LTE-V2X PDR 0.90 against DSRC 0.30 on the Q60 window: the direction matches published
      tests, the size is unchecked. LTE sends each packet twice under J3161/1.
    - Per-entity backend loads grow linearly with device count (40 messages per device from
      each LA). R-FS40 on current code: `constantPositionFrozen` 165 → 2, `positionJump`
      71 → 7, 0 links claimed. `gnss-raw`: 0 implausible evidence; honest revocations rise
      from 26 to 87 on the 300 s base, by construction of the fault.
20. **Test-harness findings** (oracle-24 §8, rerun-a §5). v2xw-server
    `pacing::a_client_synchronised_run_sheds_nothing_for_a_slow_reader` hangs intermittently
    (2 of 6 runs) on an await with no timeout; not root-caused. The `.mcap` manifest stores
    the wall-clock `build_utc`, so two same-seed recordings differ byte for byte while every
    message is identical (N15 compares messages). `in_the_loop` needs one test thread to stay
    under the 2.5 GB cap.
21. **Paper 1 features in real runs** (paper1; 73 rows: 55 pass, 4 fixed, 4 corrected, 10
    open). Fixed: **F1** (`a77aab7c`, refit `df6d7a86`): the real-data slip estimator read
    the locked class's :00/:50 grid hops as epoch slips; the refit takes locked
    `epoch_slip_prob` from 0.0067 to 0, the E4 locked rate from 5.5 to 0.0 per ID-minute,
    and honest devices blocked by sound MA rules from 35 to 1. **F2** (`8b5b1843`): the
    VeReMi export below `--record-level full` coded every attacker message 0 and put every
    true position at (0, 0); it now refuses, and the CLI refuses up front. Open:
    - **P1-E4 fails against Wyoming:** KS 0.337 against 0.092 (the white-noise GNSS fault
      row reads 0.99, as required). The tail matches; the residual body is too tight (p90
      2.3 cm against 6.0 cm, receiver output noise), and the highway runs at a median 22
      m/s against 34.
    - **TM-1:** a k>0 slip in `bsm_timing.rs` moves secMark back and keeps the position,
      so 17 % of free BSMs repeat a secMark and `generationTime` trips less often than
      secMark, the opposite of the data. Suggested fix: a drifting per-device BSM clock.
    - One honest device is still blocked by sound rules (k up to 5) through
      MaxAcceleration, timing on or off (item 1).
    - **VR-2:** 26 VeReMi true positions at (0, 0), from a vehicle's first BSM sent before
      its first `gt.kinematics` record.
    - The TS 103 759 suite cannot be selected in `detection.local`; it was replayed offline.
    - Assumption: the RA shuffle window was cut to 10 s in the Paper 1 scenarios.
22. **A car that powers on uses its expired pool's i-period at its CRL gate** (backend
    wave, found, not fixed). When MWP's node 20 powers on, its gate keeps the expired
    pool's i-period until its first step: it refuses one honest message as an implausible
    period, and `node.rx` labels it `signature-invalid` (the CRL gate, not a receiver rule,
    refuses it). Suggested fix: compute the gate period from the device clock in
    `classify`. MWC W4's `node.rx` labels were not re-run on the item 13 fix.

## 7. Every unverifiable item and how it is modelled (29)

### Cryptography and data formats (5)

| Row | Unknown | How the engine models it |
|---|---|---|
| crypto #26 | The DCM certificate (none published) | The RA's lifetime and the 1609.2.1 `DcmSsp` |
| crypto #32 | Whether pseudonyms carry a region | None. Gov F-2 decided this from [SMP] §6.3.3 (§8 item 2) |
| crypto #59 | The CCM nonce rule (the body text is paywalled) | A fresh random 12 bytes each time, never reused under one key |
| crypto #71 | The group linkage `jValue` | Group linkage is off (US practice). CAMP's `glv` vectors pass |
| crypto #73 | The LCI, PLV-request and LA key formats (NULL interfaces) | Modelled formats. The LCI is a 16 B ASSUMPTION |

### Protocols and backend flows (7)

| Row | Unknown | How the engine models it |
|---|---|---|
| proto B5 | The RA's 1 s TCP ACK as a bound | An M/M/c queue whose delay is measured |
| proto B9 | 1609.2.1 URL paths and HTTP headers | CAMP REST paths and ports. Request lines are not counted |
| proto C13 | The pilots' manual re-enrolment | SCMS Manager's online process through the RA |
| proto D11 | `download-maxAge`, `ra-maxAge`, `eca-*` (paywalled tables) | Not enforced. The engine never breaks them on the maximum-age reading |
| proto D17 | The first-batch delay | 60 s after the shuffle releases |
| proto F9 | J3287 / ETSI TS 103 759 report content | The deployed SM-MBR format |
| proto H8 | `nextCrl − issueDate` | 7 days |

Each of these MATCHES rows also contains one unpublished number:
- proto D4: the LA–ACA key rotation, 7 d.
- proto E6: "1 month" read as 30 d.
- proto G7: the MA case cap, 1,000.
- proto G8: the manual review, 8 h.
- proto H7: the CRL slot, 10:00 ET.

This round added more labelled ASSUMPTIONs:
- the 412 answer's size, 92 B;
- the retry between components, 10 s;
- the `generationTime` window. Its values are C2C-CC's (2 s, 600 s, 40 ms), because the US
  J2945/1 values are not public;
- the GNSS receiver's smoothing constant (100 s, after DO-229's carrier smoothing), static
  hold (0.3 m/s, after u-blox) and velocity σ;
- the US profile's sender eviction after 30 s without a message (at least 10× `max_gap_s`).

### Governance, deployment and calendar (6)

| Row | Unknown | How the engine models it |
|---|---|---|
| gov G-1 | EAC votes and membership | A decision is an instant the scenario gives |
| gov C-4 | What an elector removal looks like | `electorRemove`, the standard's form |
| gov D-9 | Data-centre regions | Assumed public-cloud metros |
| gov D-10 | Backend latency | Distance over fibre at about 2/3 c, plus an overhead per link |
| gov D-11 | The fleet split between providers | Seeded, stable shares |
| gov E-2 | How "6 years" of enrolment is encoded | `hours(52560)` |

### On-vehicle and roadside behaviour (11)

| Row | Unknown | How the engine models it |
|---|---|---|
| veh S1 | Deployed OBUs' verification policy | Verify-all, on-demand and prioritised. On-demand with detection now warns (07 §8.2): reports fall from 897 to 79 |
| veh S6 | The full-certificate share | Re-measured in steady state at 17 % (matrix) and 16.1–19.8 % at full fleet, inside the sourced band of 5–30 % |
| veh P7 | P2PCD concurrency limits | 8 responses, 8 inline entries |
| veh P8 | Which P2PCD flavour US devices run | Out-of-band |
| veh C10 | Whether J2945/1 clears the path history at a change | The code clears it (§8 item 4) |
| veh C11 | The pseudonym-change rule for J2945/9 handsets | The scenario's rule |
| veh M9 | The device report store quota | 64 KiB |
| veh M10 | The report rate limit | At most one per second per (reporter, accused) |
| veh M13 | Whether RSUs run misbehaviour detection | Under the US profile a unit's own detection is not filed, on the reading of [SM-MBR] §3 and the USDOT RSU Specification v4.1 (measure §4; §8 item 6). `modelled_reports: 1` files them sized-only for research. Under PQ they are still filed (§6 item 10) |
| veh V3 | The PSM rate | 1 Hz with a 100 ms floor |
| veh U3 | RSU verification policy | Verify-all |

## 8. Disagreements between the checklists

The totals count each row as its own checklist records it, with the changes in §4 applied.

1. **RSU SSPs: open.** crypto #35 and proto E7 record a deviation. Gov E-8 and F-3 read CAMP
   Table 1 in full and found no SPaT, MAP or TIM SSP in it. On that reading both rows match,
   and the deviations would be 20.
2. **Pseudonym region: open.** crypto #32 says unverifiable. Gov F-2 says "no region", from
   [SMP] §6.3.3. The published ISS and SAESOL component certificates carry regions; the
   pseudonym profile's does not (rerun §8).
3. **i-period time base: owner decision** (crypto #10 against gov K-5).
4. **Path history at a change: the checklist text is out of date.** veh C10 says the engine
   does not clear it. `vehicle.rs` `set_identity` does clear it.
5. **MA concurrency: settled.** The MA ran one case at a time; it now runs cases side by side
   (R-SERIAL, 07 §5 bug 1). The "27 decisions in every variant" were the R-USFP false
   positives, and are gone (usfp §4; att10 now 4 decisions).
6. **RSU reporting: open.** veh M13 says unverifiable: no source states whether deployed RSUs
   run detection. The DETPRIV package read [SM-MBR] §3 (a report is signed with the
   end-entity certificate, a pseudonym in CAMP's design), §2.3.1.4.1 (the reporter is itself
   a BSM sender) and the USDOT RSU Specification v4.1 (no detection or reporting function)
   as "the US profile has no roadside reporter". If the owner accepts that reading, M13
   becomes a match and the total 276 of 326 (84.7 %).

## 9. What remains

**Fix first (engine defects, in order of impact)**
1. **The honest reports at full density** (§6 item 1). Done 2026-10-24 (density): the
   braking target from the 100-Car study, the node-874 snap fixed
   (`traffic.standing_leader_stop`) and lane-polyline bends given a turn speed
   (`traffic.lane_bend_speed`), both opt-in and set by the benchmark; quarter fleet 0–60 s
   honest reports 11.3 → 0 per vehicle-hour. Left: the 20-verdict `constantPositionFrozen`
   sample (full fleet past 160 s), sender 720's braking cause, and the full-fleet re-run.
2. ~~**R-FFWIN**~~: fixed 2026-10-24 (backend wave, §6 item 2).
3. **The full-scale cost** (§6 item 3). Profiled 2026-10-24 (density §6): the rollover is
   the weekly pre-generation's P-256 work on one thread, and each stop was dominated by an
   O(fleet²) `ops_of`, now a key-range read; QD's second fast-forward 20.6 → 9.9 s and
   rollover 100.3 → 84.3 s, record digest unchanged. Left: parallel provisioning
   cryptography, the event index for the per-node wake scans (0.6 % of samples), and the
   full-fleet re-measure.
4. ~~R-JOINCRL~~: fixed 2026-10-24 (`2a14d6c4`, §6 item 7); the multi-week re-runs are
   owed (below).
5. ~~FB3's lost uploads and `reports_uploaded_relay`~~: traced and fixed 2026-10-24 (§6
   item 5); the 27 incomplete top-ups are still untraced (full-scale only).
6. ~~MWP.off-no-backend, the `crl-overdue` label, the 47 `Invalid` states, VRU PQ
   pricing~~: fixed 2026-10-24 (backend wave, §6 items 8, 11, 13).
7. **The enrolment certificate name** (moves the digests).
8. **Mobility**: the pedestrian-invariant and kerbside defects, and the `transit` and
   `longrun_budget` failures (§6 item 16).
9. **The power-on CRL gate period** (§6 item 22): compute it from the device clock.
10. **Paper 1** (§6 item 21): a drifting per-device BSM clock for TM-1; the GNSS residual
    body and the highway speed for P1-E4; VR-2; the MaxAcceleration block; TS 103 759
    selectable in `detection.local`.

**Owner decisions**
- **What the RA and PCA keep for each device** (§6 item 4): request records, cocoons,
  provisioning jobs, PCA issued records and batch files after a confirmed download.
- **R-PQUNSEC** (§6 item 10): the one-line fix, now that `pq_relay` has its own reporter.
- The RSU-reporting reading (§8 item 6).
- CF-IPERIOD (2 s).
- The US J2945/1 window values, and `max_gap_s` 0.5 s (consecutively received) against
  0.15 s (consecutive on the air) for the US profile's pair rules (memden §2.4).
- Whether SCMS Manager's artefacts may be vendored.
- **MA timing studies** (ff §4): keep the report-collection period live, or set
  `report_shuffle_threshold` / `report_shuffle_window_s` for the volume modelled, stated as
  an assumption. A frozen road holds the shuffle.
- The US profile stays the default of the US presets. It revokes no honest vehicle on any
  quarter-fleet run, nor on the full fleet over two days (FB3). It misses handset
  attackers, as the real profile does (§6 item 14).

Decided in this round: R-CCSIG's reversal of `45f0942f` was applied; the two failing
engine tests were fixed by giving each scenario what its test needs (§6 item 9).

**Engine work for the remaining deviations**
- The `infoStatus` body (A16).
- Per-channel MAC queues, a service channel and WSA bytes (M11, U5).
- Country polygons (R10).
- Governance scenario keys (G-10, C-9, C-10).
- Root termination (L-8).
- Stop issuing at 255 CRLs a week (H10).
- The post-quantum RSU certificate cycle.
- SPS-aligned BSM generation for LTE-V2X.

**Testing still owed**
- **From the backend, density and Paper 1 wave:** FB3 and QD on the R-FFWIN and relay
  fixes; MWC W4's `node.rx` labels; the full-fleet braking re-run and second fast-forward
  re-measure; P1-E4 on speed-matched runs after the residual fix.
- **From the 2026-10-24 wave:** the 55 matrix rows of 19 variants (`on-demand` …
  `real-crypto`) and `fleet2x` (realism V7 ×3) on the 300 s base; MW, MWC, MWP and the MW
  determinism pair on the R-JOINCRL fix (`RR.MW.no-overdue`, `MWC.recovery`); MWC2 redesigned
  so it tests a late outage; the full-fleet end-to-end run on current code (4.2 h); the
  2× density honest verdicts on more seeds; the v2xw-server `pacing` hang.
- **The integration of `d90a6298` is unfinished** (no code changed): the
  `sensitivity_catalogue` and `sensitivity_expectations` run that parses
  `scms-full-bench.yaml`, the oracle selftest, mutate and verify-emitted, the release build
  and boot check, and a BUILD-STATUS section with the plan §4.3 BENCH edit, which is in the
  working tree but not committed.
- **A full-fleet privacy number**: a live window that reaches 46,800 s, or the NYC pilot
  rule (§6 item 6).
- **The `d90a6298` plan §4.3 BENCH edit**, if it is still wanted (it was never committed).

Done in the backend, density and Paper 1 wave (2026-10-24): R-FFWIN, the lost relay
uploads, `MWP.off-no-backend`, the 47 `Invalid` states, VRU PQ pricing and the `node.rx`
label fixed; the braking target calibrated, the snap and bend fixes in (opt-in); the
rollover and stop costs cut at quarter fleet; Paper 1 F1 and F2 fixed and 73 Paper 1 rows
measured.

Done on 2026-10-24: R-JOINCRL fixed; the oracle on fresh conf and mbr records with OF-3 and
OF-3-JOIN; the conformance run on fixed code; the OBU/VRU R-TRUSTLEAK, F6 and F7 fault
proofs; v2xw-threat, v2xw-server and `timeline` (fixed at its cause: Poisson bands); Studio
e2e 32/32; the matrix and realism re-runs on the 300 s base (44 + 59 rows) and 16 new
realism rows; the R-FS40 window and `gnss-raw`; the PQ, relay and multi-week re-runs;
FB2.privacy corrected and `pq-profile.md` current; the LTE, 2× and Portland variants at
quarter fleet over a 60 s window; the `sensitivity_*` tests over every expectations file
(§2.2).

Done before that: the full-scale benchmark to the end (FB3) and the
quarter fleet end to end on current code (QD); R-FFBURST, `ped_wait` and the privacy rate
fixed; R-FFHOLD, R-FFSLOW and the rollover count settled; the full-scale memory target met;
`scms_wiring` and `pq_relay` passing; the PKI Studio fixtures re-recorded; the 5 mobility
failures classified.

**Paywalled standards: buying them would turn unverifiable rows into checked ones.** These
are the IEEE 1609.2.1 body text (crypto #10, proto D11, B9), IEEE 1609.2 §5.3.8 (crypto
#59), SAE J2945/1 (veh S2, C10, the window values), SAE J2945/9 (V3, C11, V1) and SAE J3287
(proto F9).

**Values that no purchase can supply.** None of these is published anywhere, so the engine
will always model them, each labelled as a setting:
- data-centre placement and fleet shares;
- EAC vote records;
- the deployed OBU and RSU verification policies;
- live CRL sizes.

"Identical" can therefore only ever mean identical wherever the real system is observable. On
that measure, 275 of 326 rules (84.4 %) match. Every rule the runtime oracle checks passed on
the last recorded runs (fresh conf and mbr captures of `81a12183` with the R-JOINCRL fix):
804,515 of 804,515 checks. Across every configuration table, 700 rows: 559 pass, 39 fixed,
50 corrected expectations and 52 open, of which 13 are open items and 10 the Paper 1
table's open rows.
- **What is left on the checklists:** 22 known deviations, 14 of which have no trust effect,
  and 29 rules no public source can settle.
- **What is left at runtime:** the honest report volume at full density (no case, but not
  yet a calibrated rate at full fleet), the cost of the rollover window and of each
  fast-forward stop at full fleet, report airtime (A1), the 27 untraced top-ups, the
  power-on CRL gate period, P1-E4 against Wyoming and TM-1, R-PQUNSEC and the
  backend retention, both owner decisions, and the re-runs that the turn rule makes owed.
  The full-scale two-day run completes under the memory target.
