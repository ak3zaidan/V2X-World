# Live Studio track, 2026-10-25

The engine as the owner uses it: `v2xw-server` with the Studio, driven by Playwright. Each
session starts the server on one base scenario, runs it from the Studio, then for each variant
types the edits into the settings window, runs, and reads every metric card, the single-metric
view, the PKI workspace's health strip and "compare with the previous run". The server's API
answers sit beside them in the evidence, and each run's recording is copied aside.

- Rows: `crates/v2xw-engine/tests/expectations/live-studio.yaml` (51 rows, written before
  their sessions; 5 rows added before `s3-run` after `s1-radio` showed 20 Hz is refused).
- Collector: `ui/apps/studio/e2e-engine/live-studio.spec.ts`.
- Checker: `tools/audit/studio_live.py`. It re-derives the Studio's number format, the run
  pooling and the statistics from the recordings' `metric.sample` records and shares no code
  with the engine or the Studio. `selftest` applies each row's fault: 146 of 147 evaluated
  (row, run) pairs go red. The one that does not is `SV.compare-ui-api [s3-run:dcc]`, which
  is already red.
- Base: `scenarios/credential-lifecycle.yaml` cut to 120 s with `security.scms.pki_view` on.
  That is the full US SCMS loop on a 5 x 8 grid with 1,500 veh/h, one RSU, 5 % ConstPos
  from 60 s and the compressed calendar. Seed `0xC0FFEE_5EED`, release binaries from
  `486c8cba`.
- Sessions: `s1-radio` and `s3-run` ran. `s2-scms` failed at its base run: the settings
  window did not close within 30 s with the machine at load average 21, so its 10 rows are
  open. Wall times in `s3-run` are about 2.4 x those in `s1-radio` (the same A run took
  114 s against 48 s) because of other agents' load. Wall rows compare runs within one
  session only.

## What the page shows against the API and the records

| Check | Measured | Verdict |
|---|---|---|
| Cards = newest API window (`SV.cards-api`) | 56 cards in each of 13 runs | pass |
| API series = recording (`SV.api-records`) | about 6,300 windows x 56 metrics per run | pass |
| Single-metric view stats = records (`SV.expanded-stats`) | pdr, pdr[100m]: min/mean/p50/p95/p99/max/n | pass |
| PKI strip = `inspect.pki` / `inspect.trust` (`SV.pki-strip`) | 460 certificates, 0 of 47 revoked | pass |
| Compare dialog = `runs.compare` (`SV.compare-ui-api`) | 10 of 11 comparisons. dcc read 0 rows; runs.compare has 66, none meaningful | open (load timing, not reproduced) |
| `runs.compare` pooled value = records (`SV.compare-records`) | 100 values per comparison | **fixed** (agg-max counts), one open (insufficient windows) |
| Same-seed flag (`SV.same-seed`) | true x10, false for the seed variant | pass |
| Edits named in the dialog (`SV.settings-named`) | every edit. The seed is in the summary, not the table, by design | wrong expectation |
| Edit reached the run (`SV.edit-landed`), only the edits differ (`SV.only-edits`) | every variant | pass |
| Determinism (`SV.deterministic`) | A digest `f935c0ce…` in both sessions; counters equal the CLI run of the same file | pass |

## What each change moved (predicted before the run)

| Change | Predicted | Measured | Verdict |
|---|---|---|---|
| OBU tx power 23 -> 10 dBm | RSSI -6 to -13.5 dB, PDR down, attempts and frames the same | -6.99 dB, PDR 0.361 -> 0.245, attempts 576,245 -> 576,240, frames equal | pass |
| tx power 23 -> 30 dBm | RSSI +3 to +7.5 dB, PDR up | byte-identical run | wrong expectation: the US OBU EIRP cap (`run.rs` `tx_power_dbm`, `radio/regulation.rs`) already clamps 23 (mean conducted 16.99 dBm). The Studio does not say the value is clamped (open) |
| BSM rate 10 -> 5 Hz | frames x0.45–0.56, CBR x0.35–0.7, attempts x0.45–0.6, events down | x0.500, x0.53, x0.500; events_processed is not recorded (diagnostic); wall 48 -> 31 s | pass, one wrong expectation |
| BSM rate 10 -> 20 Hz at 100 ms step | x2 frames | refused by validation: 20 Hz needs `mobility_step_ms` <= 50 | wrong expectation (the engine is right) |
| Mobility step 100 -> 50 ms | frames the same, wall up | 23,425 -> 23,415; wall +17 % | pass |
| 20 Hz at 50 ms (against 50 ms) | frames, attempts x1.85–2.1, CBR x1.5–2.2 | x2.000, x2.00, x2.01 | pass |
| Buildings off | PDR up >= 5 %, decoded up | 0.361 -> 0.994; decoded 92,411 -> 571,248; wall x2, footprint 219 -> 318 MB | pass |
| Seed + 2 | PDR within 5 % | -2.5 % (63 vehicles against 46) | pass |
| Metric period 1 -> 5 s | windows /5, pooled PDR identical, latency identical | 118 -> 24 windows; latency equal; PDR +0.012 % | **open, server defect**: the pool drops insufficient windows |
| Demand 1,500 -> 750 veh/h | vehicles x0.3–0.8, attempts x0.1–0.55, wall down | 46 -> 18, x0.21, 114 -> 47 s | pass |
| CWmin(VO) 3 -> 15 | access delay up, PDR the same | +34 % (only busy-medium frames back off), PDR equal | pass |
| DCC on | BSMs the same (CBR 0.009) | equal | pass |
| SCMS, security, attacker, backend knobs (s2-scms) | in the rows | not run | open |

## Fixes

1. **Compare-runs pooled max-aggregated counts as a sum** (`ce532c6e`, `crates/v2xw-server/src/compare.rs`).
   `Pool::add` summed every `Count` sample. `crl_entries`, `crl_bytes` and
   `memory_high_water_mark` aggregate by their maximum: they are levels carried from window
   to window. So a CRL's entries were reported as entries x windows in the compare dialog,
   `runs.compare` and `v2xw compare`. The fix keeps the largest window. Test:
   `compare::tests::a_max_aggregated_count_pools_to_its_peak_and_a_window_count_to_its_sum`,
   which is red with the arm disabled (16 against 5). No recorded output or golden moves.
   These runs published no CRL entry, so the live rows could not show the old behaviour.

## Open

- **Insufficient windows dropped from the run pool** (`LV.period5.pdr`). Both runs record
  69,514 / 192,357. `runs.compare` returns from `Pool::add` on `is_insufficient()` before
  it pools `RatioEstimate::Insufficient { successes: Some(..), trials }`. That variant
  carries these counts "so that windows too thin to estimate alone can still be pooled"
  (`stats.rs`). The result is that a run's value depends on the metric period. The bias is
  small here and grows with sparse fleets. Fix: pool the insufficient proportion's counts.
  It needs a test and a build, both owed.
- tx power above the region cap is silently clamped. The settings window and the run
  status give no warning.
- `s2-scms` (certs per period, full-certificate period, verification policy, attacker
  fraction, download poll) and the lifecycle checks (pause, resume, speed, reset, scenario
  switch, long-run memory soak) were not reached in the time budget. The collector supports
  them as more variants. The existing `soak.spec.ts` and `lifecycle.spec.ts` take
  `VWP_ENGINE_PORT`/`VWP_STUDIO_PORT`.
- `SV.compare-ui-api [s3-run:dcc]`: 0 rows were read. This is likely the 20 s wait at load 21.
- Locks: a Studio session needs the build lock (Playwright) and starts the engine.
  Nesting the run lock outside the build lock held the run slot for over 18 minutes while
  waiting for builds, so sessions took the build lock only, capped at 2.5 GB.
