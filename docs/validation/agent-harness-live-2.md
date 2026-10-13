# Agent evaluation

Suite `crates/v2xw-copilot/evals/harness.yaml`, run by `cargo run -p v2xw-copilot --example agent_eval`. Provider `openai:gpt-6.1-sol`, label `live2`. Each case is scored on four checks: configuration (the scenario changes and run count the prompt implies), run completed, numbers grounded (every number in the answer is in the runs' data) and conclusions (the answer says what the prompt asked about, or asks / explains when it should). Times are wall-clock seconds from the prompt; "first feedback" is the first event the person sees, "first model output" the model's first plan, tool call or answer.

## Summary

| Measure | Value |
|---|---|
| cases | 10 |
| passed (all four) | 80% |
| mean score | 0.95 |
| configuration | 100% |
| run completed | 100% |
| numbers grounded | 90% |
| conclusions | 90% |
| first feedback, median s | 0.00 |
| first model output, median s | 3.8 |
| total, median s | 9.4 |
| total, p90 s | 17.2 |
| model calls, mean | 2.7 |
| tokens in, total | 227296 |
| tokens cached, total | 206934 |
| tokens out, total | 1314 |
| cost, USD | 0.468 |

## By category

| Category | Cases | Passed | Mean score |
|---|---|---|---|
| analysis | 2 | 2 | 1.00 |
| compare | 3 | 3 | 1.00 |
| control | 4 | 3 | 0.94 |
| refusal | 1 | 0 | 0.75 |

## Cases

| Case | Config | Done | Grounded | Concl. | Runs | Calls | First model s | Total s | Tokens in/out | Cost $ | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `refuse-power-60` | yes | yes | yes | **no** | 0 | 2 | 5.6 | 11.7 | 11282/62 | 0.0232 | neither asks nor explains |
| `control-start` | yes | yes | yes | yes | 0 | 2 | 4.2 | 5.9 | 11131/44 | 0.0227 |  |
| `control-pause` | yes | yes | **no** | yes | 0 | 2 | 3.1 | 5.0 | 11379/47 | 0.0232 | ungrounded 4.7 |
| `control-speed` | yes | yes | yes | yes | 0 | 3 | 2.6 | 8.5 | 17625/82 | 0.0361 |  |
| `control-restart` | yes | yes | yes | yes | 0 | 2 | 4.4 | 8.2 | 12117/49 | 0.0247 |  |
| `compare-dsrc-lte` | yes | yes | yes | yes | 2 | 4 | 5.6 | 21.7 | 37573/308 | 0.0782 |  |
| `compare-previous` | yes | yes | yes | yes | 1 | 2 | 3.8 | 9.4 | 26621/211 | 0.0554 |  |
| `sweep-power` | yes | yes | yes | yes | 3 | 4 | 2.7 | 17.2 | 27637/334 | 0.0586 |  |
| `analysis-pdr` | yes | yes | yes | yes | 0 | 3 | 3.2 | 11.1 | 44864/99 | 0.0907 |  |
| `analysis-latency` | yes | yes | yes | yes | 0 | 3 | 2.4 | 9.3 | 27067/78 | 0.0549 |  |

## Answers

### `refuse-power-60`

> Set the transmit power to 60 dBm.

Tools: setup

    60 dBm is outside the allowed range of -20 to 33 dBm, so nothing was changed.

### `control-start`

> Start the run.

Tools: run_control

    The run has started.

### `control-pause`

> Pause it.

Tools: run_control

    Paused at 4.7 s.

### `control-speed`

> Resume at 4x speed.

Tools: run_control, run_control

    Resumed at 4× speed.

### `control-restart`

> Restart it from the beginning.

Tools: run_control

    Reset to the beginning; the run is paused.

### `compare-dsrc-lte`

> Run the grid for 20 s at 1500 veh/h with DSRC, then the same with LTE-V2X, and compare delivery.

Tools: find_settings, get_scenario, simulate

    - LTE-V2X had critical medium-access delay: p95 **92.4 ms** [access.delay], with **0.058 ratio** of messages left unverified [access.verification].
    - Overall **delivery_ratio** improved from **0.013 ratio** with DSRC to **0.325 ratio** with LTE-V2X.
    - Within-range **pdr[300m]** improved from **0.092 ratio** to **0.871 ratio**.
    - LTE-V2X delivered better in this matched-grid comparison, but this is a single-seed result without confidence intervals.

### `compare-previous`

> Now double the traffic on the LTE run and compare with the previous run.

Tools: simulate

    - Critical medium-access delay persisted: p95 **88 ms** [access.delay].
    - **unverified_ratio** worsened from **0.058 ratio** to **0.083 ratio** [access.verification].
    - Overall **delivery_ratio** fell from **0.325 ratio** to **0.289 ratio**, a **-11.2%** change.
    - Within-range **pdr[300m]** rose from **0.871 ratio** to **0.913 ratio**, but the comparison did not flag this as significant.
    - Same seed and run duration; these results have no confidence interval.

### `sweep-power`

> Sweep the transmit power over 10, 20 and 30 dBm, 15 s each at 1500 veh/h, and tell me how delivery changes.

Tools: find_settings, get_scenario, simulate

    - Delivery remained very poor in every arm; below-sensitivity reception dominated losses. [channel.weak-signal]
    - Overall `delivery_ratio`: **0.00033 → 0.011 → 0.011 ratio** at **10 → 20 → 30 dBm**.
    - Within-range `pdr[300m]`: **0 → 0.036 → 0.036 ratio**. [delivery.range]
    - Increasing power to **20 dBm** improved delivery; **30 dBm** provided no further measured benefit.
    - Each arm ran for **15 s** with the same seed; these results have no confidence interval.

### `analysis-pdr`

> What was the packet delivery ratio of the last run?

Tools: live_metrics, compare_runs

    The last LTE-V2X doubled-traffic run had **pdr[300m] = 0.913 ratio** (within-range packet delivery). Its overall **delivery_ratio was 0.289 ratio**.

### `analysis-latency`

> What was the 95th-percentile latency at 30 dBm, in milliseconds?

Tools: finding_detail, live_metrics

    The 95th-percentile latency was not available for the 30 dBm run, so I can’t report a value in milliseconds.

