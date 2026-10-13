# Agent evaluation

Suite `crates/v2xw-copilot/evals/harness.yaml`, run by `cargo run -p v2xw-copilot --example agent_eval`. Provider `openai:gpt-6.1-sol`, label `live3`. Each case is scored on four checks: configuration (the scenario changes and run count the prompt implies), run completed, numbers grounded (every number in the answer is in the runs' data) and conclusions (the answer says what the prompt asked about, or asks / explains when it should). Times are wall-clock seconds from the prompt; "first feedback" is the first event the person sees, "first model output" the model's first plan, tool call or answer.

## Summary

| Measure | Value |
|---|---|
| cases | 15 |
| passed (all four) | 73% |
| mean score | 0.93 |
| configuration | 93% |
| run completed | 100% |
| numbers grounded | 100% |
| conclusions | 80% |
| first feedback, median s | 0.00 |
| first model output, median s | 3.3 |
| total, median s | 10.3 |
| total, p90 s | 15.4 |
| model calls, mean | 3.0 |
| tokens in, total | 337765 |
| tokens cached, total | 307560 |
| tokens out, total | 2262 |
| cost, USD | 0.698 |

## By category

| Category | Cases | Passed | Mean score |
|---|---|---|---|
| control | 2 | 2 | 1.00 |
| hard | 10 | 6 | 0.90 |
| refusal | 1 | 1 | 1.00 |
| setting | 2 | 2 | 1.00 |

## Cases

| Case | Config | Done | Grounded | Concl. | Runs | Calls | First model s | Total s | Tokens in/out | Cost $ | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `set-radio-weaker` | yes | yes | yes | yes | 0 | 4 | 5.4 | 15.4 | 28317/115 | 0.0578 |  |
| `set-cert-every-message` | yes | yes | yes | yes | 0 | 2 | 4.0 | 6.6 | 12247/62 | 0.0251 |  |
| `refuse-power-60` | yes | yes | yes | yes | 0 | 2 | 3.9 | 6.6 | 12378/63 | 0.0254 |  |
| `control-pause` | yes | yes | yes | yes | 0 | 2 | 2.7 | 4.5 | 12188/44 | 0.0248 |  |
| `control-speed` | yes | yes | yes | yes | 0 | 3 | 2.9 | 8.9 | 18765/82 | 0.0384 |  |
| `hard-cautious-drivers` | yes | yes | yes | **no** | 0 | 4 | 3.3 | 13.2 | 28066/223 | 0.0584 | mentions none of ["lane", "politeness", "threshold"] |
| `hard-sidelink-reselect` | yes | yes | yes | yes | 0 | 3 | 4.2 | 10.3 | 21138/135 | 0.0436 |  |
| `hard-storm` | **no** | yes | yes | yes | 0 | 3 | 5.3 | 11.5 | 18833/190 | 0.0396 | draft not met: weather.initial = "rain" (is "clear") |
| `hard-multistep` | yes | yes | yes | yes | 2 | 4 | 3.9 | 23.3 | 40329/315 | 0.0838 |  |
| `hard-compare-again` | yes | yes | yes | yes | 0 | 2 | 2.1 | 5.4 | 26380/92 | 0.0537 |  |
| `hard-explain-bad-pdr` | yes | yes | yes | yes | 1 | 3 | 3.9 | 14.0 | 27671/256 | 0.0579 |  |
| `hard-sensitivity-power` | yes | yes | yes | yes | 0 | 2 | 2.5 | 6.7 | 12691/127 | 0.0267 |  |
| `hard-sensitivity-dead` | yes | yes | yes | yes | 0 | 2 | 3.1 | 7.9 | 13179/163 | 0.0280 |  |
| `hard-invalid-phy-on-cv2x` | yes | yes | yes | **no** | 0 | 4 | 3.1 | 11.8 | 28554/175 | 0.0589 | neither asks nor explains |
| `hard-invalid-ghosts` | yes | yes | yes | **no** | 0 | 5 | 3.1 | 15.3 | 37029/220 | 0.0763 | neither asks nor explains |

## Answers

### `set-radio-weaker`

> Make the radio weaker.

Tools: find_settings, get_scenario, setup

    Ready in Settings; press Run to start.

### `set-cert-every-message`

> Attach the full certificate to every message.

Tools: setup

    Ready in Settings; press Run to use it.

### `refuse-power-60`

> Set the transmit power to 60 dBm.

Tools: setup

    60 dBm exceeds the allowed transmit-power range of -20 to 33 dBm, so nothing was changed.

### `control-pause`

> Pause it.

Tools: run_control!

    It’s already paused.

### `control-speed`

> Resume at 4x speed.

Tools: run_control, run_control

    Resumed at 4× speed.

### `hard-cautious-drivers`

> Make the drivers more cautious - they should keep bigger gaps to the car ahead and be less eager to change lanes.

Tools: find_settings, get_scenario, setup

    Ready in the Settings window; press Run to use the more cautious drivers.

### `hard-sidelink-reselect`

> Switch to LTE C-V2X and make the vehicles reselect their sidelink resources more often than the standard does.

Tools: find_settings, setup

    Ready in Settings with fast resource reselection enabled; press Run to start.

### `hard-storm`

> Set it up like a stormy evening downtown with heavy rain.

Tools: setup!, list_cities

    The evening downtown setup is blocked because Manhattan’s map is missing. Run `worlds/fetch.sh manhattan` to download it; the heavy-rain draft has not been applied.

### `hard-multistep`

> On the grid, run 20 s at 600 veh/h, then run the same with the traffic doubled, and compare the two runs.

Tools: find_settings, get_scenario, simulate

    - Overall delivery_ratio stayed very low: 0.012 → 0.013; most losses were below receiver sensitivity, not congestion. [channel.weak-signal]
    - Doubled traffic had mean speed 5.478 m/s against posted speed 11.2 m/s. [traffic.delay]
    - Both grid runs completed at 20 s with the same seed. Air traffic rose from 8691 B/s → 12282 B/s (+41.3%).
    - The comparison’s speed metric fell from 7.122 m/s → 6.071 m/s (−14.8%); collision_rate remained 0.
    - These are single-seed results, without confidence intervals.

### `hard-compare-again`

> Compare those two again - which metric changed the most?

Tools: compare_runs

    Among the significant changes, **lost_frames[half-duplex]** changed most proportionally: **484 → 1292 frames (+167%)**. Below-sensitivity losses followed at **1165 → 2989 frames (+157%)**; absolute changes across different units aren’t directly comparable.

### `hard-explain-bad-pdr`

> Why is packet delivery below 100 percent in the busier run? Explain it from the data.

Tools: engine_runs, analyse_current_run

    - Weak signals dominate: the loss analysis reports a below-sensitivity share of **0.991 ratio**—receivers cannot reliably hear the transmissions. [channel.weak-signal]
    - Reception worsens with distance: PRR is **0.02 ratio at 220–240 m** and **0 ratio at 240–260 m**. [delivery.range]
    - Half-duplex operation also contributes: its reported loss share is **0.022 ratio**.
    - This is not evidence of congestion: the run comparison shows **collision_rate 0 ratio**, **MAC drops 0 count**, and healthy **channel_load 0.003133333333333333 ratio**.
    - Being busier did not reduce overall delivery: **delivery_ratio rose from 0.006469500924214418 ratio to 0.011828320378506252 ratio**. Delivery remains low primarily because of weak reception, not because the added traffic saturated the channel.

### `hard-sensitivity-power`

> Without running anything, how sensitive is the channel busy ratio to transmit power?

Tools: sensitivity

    Channel busy ratio is strongly sensitive to transmit power: the checked-in same-seed sweep lowered `cbr_mean` from 0.0815 to 0.0132 at lower power, because fewer nodes sensed transmissions above the carrier-sense threshold.
    
    The trade-off was reduced reception: `prr` fell from 0.9078 to 0.2940. These are catalogue results, not measurements on your current scenario; nothing was run.

### `hard-sensitivity-dead`

> Does the receiver noise figure setting actually change any result?

Tools: setting_help

    Yes—`radio.phy.noise_figure_db` affects DSRC reception: increasing it adds receiver noise and lowers signal-to-noise ratio.
    - Checked-in same-seed sweeps showed `range90_m` falling from 640.0000 m to 600.0000 m, and `prr` falling from 0.9078 to 0.8459.
    - Lowering it improved `prr` slightly, from 0.9078 to 0.9094.
    - LTE-V2X and NR-V2X refuse this setting; they use their sidelink model’s parameters. These measurements are from validation sweeps, not your current scenario.

### `hard-invalid-phy-on-cv2x`

> Use LTE-V2X and set the 802.11p data rate to 12 Mbps.

Tools: find_settings, setup, setup

    LTE-V2X is ready; press Run. The engine refuses the 802.11p data-rate setting with LTE-V2X, so that setting was not applied.

### `hard-invalid-ghosts`

> Add a constant-position attacker with 5 Sybil ghosts.

Tools: find_settings, setting_help, find_settings, setup

    The constant-position attacker is ready; press Run. The published settings do not expose a Sybil ghost count or a combined constant-position/Sybil attack, so the ghosts were not added.

