# SCMS audit: the simulation as a whole, through its outputs (sim-metrics)

Round-1b audit package `sim-metrics`, on main `8becf72a`. Every SCMS metric the engine emits
is recomputed from the raw records by an independent reader, the run report's counters are
held to conservation laws, and the run is checked for time order, cost, determinism across
thread counts and checkpoint/resume.

- **Checker:** `tools/audit/sim_metrics.py`. It reads only `recording.mcap`, `metrics.json`
  and `run-report.json` and shares no code with the engine. `check` evaluates the table.
  `selftest` re-runs every check with its injected fault and requires it to go red.
- **Table:** `crates/v2xw-engine/tests/expectations/scms-audit-sim-metrics.yaml`. 32 rows,
  committed (`aca01fcc`) before any run was read.
- **Runs:** the release binary of `8becf72a`, seed `0xBE7C_0001`, through the run locks.

| Run | Scenario | What it is | Wall / peak |
|---|---|---|---|
| S | `audit-sim/short.yaml` | realism fleet (95 devices + 30 RSUs), 3 % ConstPos from 20 s, 240 s live, PKI view, events | 138 s / 138 MB |
| X | the same, cut to 50 s | receptions level, so `node.verify` is kept | 20 s / 151 MB |
| Q | `audit-sim/quarter-attack.yaml` | quarter benchmark + 3 % ConstPos + PKI view, 172,800 s (live 0–60 s, 43,185–43,215 s) | 778 s / 311 MB |
| L | `audit-sim/lte.yaml` | LTE-V2X PC5 mode 4, quarter fleet, 90 s live | 806 s / 452 MB |
| U, T1, T4, CK | short, 120 s, `--checkpoint-every 60s` | 2, 1 and 4 rayon threads; CK stopped at 60 s and resumed | not run (§3 item 7) |

## 1. Inventory

The SCMS-related metrics come from three providers:
- `SecurityProvider` (`v2xw-metrics/src/security.rs`);
- the overhead family (`overhead.rs`);
- `PrivacyProvider` (`v2xw-engine/src/privacy_metrics.rs`).

Detection metrics come from `DetectionProvider` (`detection.rs`), and the byte buckets from
`comms.rs`. Each metric is checked over every window of its run.

Windows are (previous flush, flush]: 10 s on the grid from the end of warm-up, plus a flush
at each fast-forward edge. A rate divides by the window's own length.

| Behaviour (row) | Spec value (definition) | Measured (S / Q / L / X) | Verdict |
|---|---|---|---|
| Every `metrics.json` sample is in the recording (MI.samples-recorded) | `run.rs`: metrics.json is every `metric.sample` the run emitted; `events` keeps them | 73 / 62 / 20 samples missing from the recording | **fixed** |
| Windows on the 10 s grid or at a fast-forward edge, none inside a gap, none twice (MI.windows) | `metrics.window_s` | 2,144 / 907 / 480 samples hold; Q also flushes at 43,185 and 43,215 s | pass |
| Units (MI.units) | each `MetricDef` | 641 / 266 / 174 hold | pass |
| `full_cert_share` (MR) | full-certificate signer / messages with a signer | every window equal | pass |
| `envelope_overhead` | Σ envelope / Σ payload over `node.tx` | every window equal | pass |
| `security_overhead`, `cert_bytes_share` | Σ envelope / Σ on air; Σ cert / Σ envelope, total and per msg_type | 79 / 30 / 18 samples equal | pass |
| `p2pcd_requests` | Σ telemetry `p2pcd_requests` / window length | equal (Q: 5 s window after a fast-forward edge) | pass |
| `cert_pool_valid` | distribution of `node.security.pool_valid` | n, min, mean, max equal | pass |
| `backend_link_up` | rows with `link_up` / rows | equal | pass |
| `bytes_uu_ul`, `bytes_uu_dl`, `bytes_backhaul`, `bytes_backend` | Σ `net.bytes` per bucket / window length | 84 / 36 / 24 samples equal | pass |
| `linkability` | correct links / pseudonym changes | trials 0 on all three runs: no pseudonym change in any live window | pass (not exercised) |
| `det_tp/fp/fn/tn`, `det_recall` (report level) | gt.subject population, positive = subject in `ma.report`, truth = actor in `gt.attack.action` | 0/0/3/97 / 6/1/4/292 / 0/0/0/354 | pass |
| `verify_rate`, `unverified_ratio`, `verify_cost`, `verify_wait` (X) | `node.verify` Valid+Invalid per primitive / s; Skipped / (V+I+S); cost and wait | every window equal | pass |
| `crl_entries`, `crl_bytes` (Q) | the list as last published (`proto.revocation`) | 6 entries, 452 B | pass |
| Report fate (CV) | sent = uploaded (cellular + relay) + unsent + evicted + expired + not filed + lost + MTU | 1,783 / 2,928 / 10 | pass |
| Reports received | = pki.flow uploads = Σ `reports_by_provider` | 1,154 / 2,327 / 0 | pass |
| Report truth | about attackers + about honest = sent | 1,782+1 / 2,912+16 / 0+10 | pass |
| Verification states | Σ `verification_states` = `messages_checked` | 622,322 / 914,597 / 3,951,934 | pass |
| Privacy decisions | `privacy.link` records = `privacy_link_decisions` | 643 / 1,159 / 1,366 | pass |
| Attack onset | first change on air ≤ label onset; no report before the first change; `time_to_detect` ≥ 0 | holds; label onset lags by 0–13 s (S), up to 43,127 s (Q) | **wrong expectation** (corrected) |
| Preload | `devices_by_provider` = devices enrolled in the preload; RA→ACA = ACA→RA | 130 / 333 / 384 | pass |
| Time order (RS) | each channel in nondecreasing log time; none after the end; own `t` ≤ log time | first two hold; the third fails on four channels (§3) | **open** |
| Cost | S ≤ 300 s / 600 MB; Q, L ≤ 600 s / 1.2 GB | 138 s / 138 MB; 778 s / 311 MB; 806 s / 452 MB | S pass; Q, L **open** |
| Same records on 1, 2 and 4 threads; resume equals uninterrupted | ADR 0004; `checkpoint.rs` | queued behind another package's run on slot 2; not run | **open** |
| Server `inspect.trust` and `inspect.flows`; Studio PKI view | `v2xw-server/src/pki.rs` | not probed live | **open** |

The checker matches every SCMS metric exactly on all three diverse runs, in every window.
That includes the windows of Q that a fast-forward edge cuts short.
Every conservation law holds.

## 2. Fixes

**MI.samples-recorded** (`113b156e`). The recordings lacked metric samples that
`metrics.json` had: 73 of 13,182 on S, 62 on Q and 20 on L. Among them were the
misbehaviour-report flow's `e2e_latency` and `latency_stage` samples.

`RecordingWriter::write_record`'s D9 gate refused these samples. The CLI's sink drops a
refused write (`let _ =`), and `records_refused` stays 0, so nothing showed the loss. There
were two causes:

- **The carried `sum` of an insufficient distribution was never quantised.** Example:
  `538.5491059999999`. It now goes onto the sums' 1e-3 grid, and `graded_floats` and
  `floats` now include it.
- **`v2xw_core::math::is_on_grid` rejected some large values.** At magnitudes where
  doubles are a quantum or more apart, it called a value off its grid. Example: a
  598,195,368,093 ns `aoi` denominator on the 1e-6 grid. There, `round(x/q)*q` only
  re-rounds the product. Such magnitudes now count as on the grid.

The test `v2xw-record` `record_grid::every_metric_sample_a_provider_writes_passes_the_record_gate`
fails without each half of the fix, on the sum `0.30000000000000004` and on the denominator.
It passes with both.

Goldens: the JSON of `metric.sample` for insufficient distributions changes, so the SCMS
regression digests move. The re-bless was started (`V2XW_BLESS_SCMS_DIGESTS=1 cargo test
-p v2xw-engine --test phase2 --test scms_scale`); the merge notes give its state.

Still owed: re-run S on a binary with the fix, so that MI.samples-recorded goes green
end to end.

## 3. Open findings

1. **The privacy observer reads frames before they are on the air** (RS.time-order).
   `run.rs` calls `observe_frame` when the frame is built, with its `ready` instant.
   - On S, 219 of 643 `privacy.link` claims are stamped up to 11.7 ms after the step that
     logged them.
   - A frame the MAC later drops is still "heard".
   - Fix: observe at the transmission start (PhyEnd/tx) instead.
2. **The preload flow's `pki.flow` records are logged at t = 0, but their own stamps run
   to 0.42 s.** The pre-run provisioning clock leaks into the PKI view's flow times.
3. **Two Q channels are stamped after the backend tick that logs them:** `net.bytes`
   (8.8 µs at 43,200 s) and `proto.revocation` (118 ms at 86,420 s).
4. **Run cost on Q.** The run took 778 s against QD's 189 s. Most of the difference is the
   second fast-forward: 453 s, against QD's 9.9 s.
   - With the PKI view on, `pki.state`, `backend.state` and `pki.knowledge` are recorded
     at every one of 11,004 backend ticks: 591 MB of JSON.
   - Not profiled, so this is the likely cause, not a proven one.
   - L's 806 s ran beside a cargo build and is not a clean measurement.
5. **The two onset definitions disagree** (CV.attack-onset).
   - The run report's `onset_ns`, and with it `onset_to_detection_ns`, uses the
     07-threats §4 label rule.
   - `time_to_detect` uses the first on-air change.
   - When a report comes before the label onset, `onset_to_detection_ns` saturates to 0.
   - In Q, node 303's label onset is 43,127 s after its first change.
   - One definition should be chosen for both.
6. **Not exercised by these runs:** `linkability` and `pseudonym_change_rate` (no
   pseudonym change in any live window); `revocation_latency_stage` (no `proto.revocation`
   stage pairs were sampled on Q).
7. **Not run:** determinism across 1, 2 and 4 threads, checkpoint/resume, and the server
   and Studio comparison. The scenarios and rows are ready: `det.sh` runs U, T1, T4 and
   CK, then `sim_metrics.py check` compares them.
8. **`verify_queue_depth` mixes two sources.** It feeds on the `node.verify` depth at
   enqueue and also on the periodic `node.telemetry` depth, so it is not the
   arrival-sampled distribution its definition states. This came from reading the code;
   no check was written for it.

## 4. Fault proofs

RESULTS-SELFTEST
