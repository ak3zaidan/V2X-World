# Agent evaluation

Suite `crates/v2xw-copilot/evals/harness.yaml`, run by `cargo run -p v2xw-copilot --example agent_eval`. Provider `openai:gpt-6.1-sol`, label `live1`. Each case is scored on four checks: configuration (the scenario changes and run count the prompt implies), run completed, numbers grounded (every number in the answer is in the runs' data) and conclusions (the answer says what the prompt asked about, or asks / explains when it should). Times are wall-clock seconds from the prompt; "first feedback" is the first event the person sees, "first model output" the model's first plan, tool call or answer.

## Summary

| Measure | Value |
|---|---|
| cases | 15 |
| passed (all four) | 73% |
| mean score | 0.93 |
| configuration | 87% |
| run completed | 100% |
| numbers grounded | 87% |
| conclusions | 100% |
| first feedback, median s | 0.00 |
| first model output, median s | 4.4 |
| total, median s | 8.5 |
| total, p90 s | 18.2 |
| model calls, mean | 2.3 |
| tokens in, total | 238956 |
| tokens cached, total | 218952 |
| tokens out, total | 1623 |
| cost, USD | 0.494 |

## By category

| Category | Cases | Passed | Mean score |
|---|---|---|---|
| analysis | 1 | 0 | 0.75 |
| compare | 2 | 2 | 1.00 |
| control | 3 | 3 | 1.00 |
| multi | 2 | 2 | 1.00 |
| refusal | 1 | 0 | 0.75 |
| setting | 6 | 4 | 0.92 |

## Cases

| Case | Config | Done | Grounded | Concl. | Runs | Calls | First model s | Total s | Tokens in/out | Cost $ | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `set-radio-weaker` | **no** | yes | yes | yes | 0 | 4 | 5.1 | 18.2 | 25602/129 | 0.0525 | draft not met: radio.devices.obu.tx_power_dbm in [None, Some(19.0)] (is 20.0) |
| `set-cv2x-mode4` | yes | yes | yes | yes | 0 | 2 | 5.1 | 8.5 | 10904/81 | 0.0226 |  |
| `set-cert-every-message` | **no** | yes | yes | yes | 0 | 3 | 4.1 | 12.4 | 18403/127 | 0.0381 | draft not met: security.signer_id_policy.full_cert_every_ms in [None, Some(100.0)] (is 1000) |
| `set-truck-headway` | yes | yes | yes | yes | 0 | 3 | 4.5 | 11.1 | 18499/73 | 0.0377 |  |
| `set-penetration` | yes | yes | yes | yes | 0 | 2 | 3.8 | 5.9 | 10891/50 | 0.0223 |  |
| `set-pseudonym-period` | yes | yes | yes | yes | 0 | 2 | 4.5 | 7.6 | 10915/77 | 0.0226 |  |
| `multi-portland-rush` | yes | yes | yes | yes | 0 | 2 | 8.0 | 10.7 | 11106/188 | 0.0241 |  |
| `multi-attack-detect` | yes | yes | yes | yes | 0 | 2 | 5.9 | 9.5 | 11001/114 | 0.0231 |  |
| `refuse-power-60` | yes | yes | **no** | yes | 0 | 2 | 3.7 | 6.5 | 11014/71 | 0.0227 | ungrounded 20, 33 |
| `control-start` | yes | yes | yes | yes | 0 | 2 | 4.2 | 7.4 | 10865/44 | 0.0222 |  |
| `control-pause` | yes | yes | yes | yes | 0 | 2 | 2.6 | 6.5 | 11076/51 | 0.0227 |  |
| `control-duration` | yes | yes | yes | yes | 0 | 2 | 4.5 | 7.3 | 10864/42 | 0.0221 |  |
| `compare-dsrc-lte` | yes | yes | yes | yes | 2 | 4 | 3.3 | 19.5 | 37019/333 | 0.0774 |  |
| `compare-previous` | yes | yes | yes | yes | 1 | 2 | 4.4 | 13.0 | 26396/220 | 0.0550 |  |
| `analysis-pdr` | yes | yes | **no** | yes | 0 | 1 | 2.5 | 2.5 | 14401/23 | 0.0290 | ungrounded 0.913 |

## Answers

### `set-radio-weaker`

> Make the radio weaker.

Tools: find_settings, get_scenario, setup

    Ready—press Run to use the weaker radio.

### `set-cv2x-mode4`

> Use C-V2X mode 4.

Tools: setup

    Ready with C-V2X LTE sidelink (PC5 Mode 4). Press Run to start.

### `set-cert-every-message`

> Attach the full certificate to every message.

Tools: find_settings, setup

    Ready in Settings; press Run to use full certificates on every message.

### `set-truck-headway`

> IDM headway 2 s for trucks.

Tools: find_settings, setup

    Ready in Settings; press Run to start.

### `set-penetration`

> Only 30% of the cars have a V2X radio.

Tools: setup

    Ready in Settings; press Run to start.

### `set-pseudonym-period`

> Change pseudonyms every 2 minutes.

Tools: setup

    Ready in Settings; press Run to use it.

### `multi-portland-rush`

> Set up a rush-hour run in Portland, OR at 4000 vehicles per hour, 20% trucks, on LTE-V2X, 10 minutes long.

Tools: setup

    Ready in the Settings window; press Run to start.

### `multi-attack-detect`

> Configure 5% constant-position attackers, turn the misbehaviour detectors on, and change pseudonyms every 60 s.

Tools: setup

    Ready in the Settings window; press Run to start.

### `refuse-power-60`

> Set the transmit power to 60 dBm.

Tools: setup

    60 dBm is outside the allowed range of -20 to 33 dBm, so nothing was changed. What power within that range would you like?

### `control-start`

> Start the run.

Tools: run_control

    The run has started.

### `control-pause`

> Pause it.

Tools: run_control!

    The run has already finished, so it cannot be paused.

### `control-duration`

> Make the run 90 seconds long.

Tools: setup

    Ready—press Run to start.

### `compare-dsrc-lte`

> Run the grid for 20 s at 1500 veh/h with DSRC, then the same with LTE-V2X, and compare delivery.

Tools: find_settings, get_scenario, simulate

    - LTE-V2X had critical medium-access delay: p95 **92.4 ms** [access.delay], and **0.058 ratio** of messages remained unverified [access.verification].
    - Delivery within 300 m improved from **0.092 ratio** with DSRC to **0.871 ratio** with LTE-V2X.
    - Overall delivery improved from **0.013 ratio** to **0.325 ratio**; goodput rose from **235 B/s** to **3142 B/s**.
    - DSRC losses were dominated by below-sensitivity signals; LTE-V2X losses by fading [channel.weak-signal].
    - Traffic was identical across arms. This is a single-seed comparison, with no confidence interval.

### `compare-previous`

> Now double the traffic on the LTE run and compare with the previous run.

Tools: simulate

    - Medium-access delay remains critical: p95 **88 ms** [access.delay].
    - Unverified messages increased from **0.058 ratio** to **0.083 ratio** [access.verification].
    - Overall delivery fell from **0.325 ratio** to **0.289 ratio**, a **-11.2%** change.
    - Delivery within 300 m rose from **0.871 ratio** to **0.913 ratio**, but was not flagged as significant; goodput increased from **3142 B/s** to **5704 B/s**.
    - Both runs used the same seed and duration; there is no confidence interval.

### `analysis-pdr`

> What was the packet delivery ratio of the last run?

Tools: none

    The last run’s packet delivery ratio (PDR) was **0.913 ratio**.

