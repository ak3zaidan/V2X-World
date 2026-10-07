# Agent evaluation

Suite `crates/v2xw-copilot/evals/prompts.yaml`, run by `cargo run -p v2xw-copilot --example agent_eval`. Provider `openai:gpt-6.1-sol`, label `before`. Each case is scored on four checks: configuration (the scenario changes and run count the prompt implies), run completed, numbers grounded (every number in the answer is in the runs' data) and conclusions (the answer says what the prompt asked about, or asks / explains when it should). Times are wall-clock seconds from the prompt; "first feedback" is the first event the person sees, "first model output" the model's first plan, tool call or answer.

## Summary

| Measure | Value |
|---|---|
| cases | 42 |
| passed (all four) | 79% |
| mean score | 0.93 |
| configuration | 95% |
| run completed | 95% |
| numbers grounded | 90% |
| conclusions | 90% |
| first feedback, median s | 3.50 |
| first model output, median s | 3.5 |
| total, median s | 35.9 |
| total, p90 s | 60.3 |
| model calls, mean | 8.5 |
| tokens in, total | 1894650 |
| tokens cached, total | 1538500 |
| tokens out, total | 37638 |
| cost, USD | 4.166 |

## By category

| Category | Cases | Passed | Mean score |
|---|---|---|---|
| ambiguous | 3 | 2 | 0.83 |
| attack | 7 | 7 | 1.00 |
| congestion | 1 | 1 | 1.00 |
| emergency | 1 | 1 | 1.00 |
| events | 2 | 2 | 1.00 |
| follow-up | 3 | 3 | 1.00 |
| impossible | 4 | 3 | 0.94 |
| jamming | 2 | 0 | 0.62 |
| multi-variant | 3 | 2 | 0.92 |
| normal | 3 | 3 | 1.00 |
| overhead | 3 | 2 | 0.83 |
| pedestrians | 1 | 1 | 1.00 |
| privacy | 2 | 1 | 0.88 |
| radio | 2 | 1 | 0.88 |
| revocation | 1 | 1 | 1.00 |
| vague | 2 | 1 | 0.88 |
| weather | 2 | 2 | 1.00 |

## Cases

| Case | Config | Done | Grounded | Concl. | Runs | Calls | First model s | Total s | Tokens in/out | Cost $ | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `normal-baseline` | yes | yes | yes | yes | 1 | 5 | 3.5 | 21.4 | 17253/572 | 0.0402 |  |
| `normal-channel` | yes | yes | yes | yes | 1 | 8 | 3.1 | 32.9 | 35324/894 | 0.0796 |  |
| `normal-lte` | yes | yes | yes | yes | 1 | 7 | 3.5 | 28.1 | 27692/821 | 0.0636 |  |
| `congestion-rush` | yes | yes | yes | yes | 1 | 7 | 3.9 | 33.1 | 26502/761 | 0.0606 |  |
| `congestion-light-vs-rush` | yes | yes | yes | yes | 2 | 10 | 3.4 | 45.1 | 47914/1003 | 0.1059 |  |
| `attack-constpos` | yes | yes | yes | yes | 1 | 8 | 3.8 | 30.2 | 45240/829 | 0.0988 |  |
| `attack-randompos-vs-baseline` | yes | yes | yes | yes | 2 | 10 | 3.6 | 39.4 | 59416/945 | 0.1283 |  |
| `attack-speed` | yes | yes | yes | yes | 1 | 8 | 3.3 | 34.3 | 43971/846 | 0.0964 |  |
| `attack-replay` | yes | yes | yes | yes | 1 | 7 | 4.4 | 33.9 | 34858/799 | 0.0777 |  |
| `attack-signature` | yes | yes | yes | yes | 1 | 11 | 3.3 | 49.9 | 70474/1225 | 0.1532 |  |
| `attack-expired-cert` | yes | yes | yes | yes | 1 | 12 | 4.9 | 50.8 | 66496/1234 | 0.1453 |  |
| `attack-wave` | yes | yes | yes | yes | 1 | 7 | 4.6 | 32.7 | 34912/695 | 0.0768 |  |
| `attack-sybil` | yes | yes | yes | **no** | 0 | 11 | 3.9 | 40.6 | 51808/1022 | 0.1138 | mentions none of ["sybil"] |
| `jam-constant` | yes | yes | yes | **no** | 1 | 12 | 4.3 | 59.1 | 66399/1533 | 0.1481 | no finding among ["channel.jamming"] |
| `jam-dsrc-vs-lte` | yes | **no** | yes | **no** | 2 | 16 | 3.1 | 60.3 | 104857/1354 | 0.2233 | error: the copilot exhausted its round budget of 16; it stopped rather than continuing unsupervised; mentions none of ["jam"]; mentions none of ["lte", "sidelink", "c-v2x"]; empty answer |
| `revocation` | yes | yes | yes | yes | 1 | 6 | 4.1 | 47.2 | 23973/595 | 0.0539 |  |
| `privacy-period` | yes | yes | **no** | yes | 2 | 11 | 3.4 | 66.1 | 64776/891 | 0.1385 | ungrounded 0.25 |
| `privacy-mixzone` | yes | yes | yes | yes | 2 | 9 | 4.1 | 65.3 | 51378/877 | 0.1115 |  |
| `rat-three-way` | yes | yes | **no** | yes | 3 | 12 | 2.8 | 49.2 | 72872/1180 | 0.1575 | ungrounded 0.223 |
| `radio-region-eu` | yes | yes | yes | yes | 2 | 13 | 3.9 | 45.2 | 74045/1152 | 0.1596 |  |
| `overhead-cert-cadence` | yes | yes | yes | yes | 2 | 9 | 3.5 | 27.3 | 43198/683 | 0.0932 |  |
| `overhead-pq-mldsa` | **no** | yes | **no** | yes | 3 | 15 | 3.6 | 56.9 | 98126/1528 | 0.2115 | 3 run(s), expected 2; ungrounded 189.0, 2634.0 |
| `overhead-falcon` | yes | yes | yes | yes | 1 | 10 | 3.2 | 46.2 | 50179/1534 | 0.1157 |  |
| `emergency-priority` | yes | yes | yes | yes | 1 | 4 | 3.2 | 21.3 | 12979/525 | 0.0312 |  |
| `pedestrians` | yes | yes | yes | yes | 1 | 7 | 2.6 | 28.3 | 32788/699 | 0.0726 |  |
| `weather-rain` | yes | yes | yes | yes | 2 | 9 | 3.5 | 32.4 | 47713/818 | 0.1036 |  |
| `weather-fog-front` | yes | yes | yes | yes | 1 | 10 | 3.5 | 49.9 | 46821/1384 | 0.1075 |  |
| `event-closure` | yes | yes | yes | yes | 1 | 11 | 3.1 | 52.7 | 56492/1669 | 0.1297 |  |
| `event-hard-brake` | yes | yes | yes | yes | 1 | 10 | 4.3 | 61.6 | 47484/1284 | 0.1078 |  |
| `multi-four-variants` | yes | yes | **no** | yes | 4 | 15 | 3.8 | 61.2 | 131647/1646 | 0.2798 | ungrounded 0.062 |
| `multi-density-sweep` | yes | yes | yes | yes | 3 | 12 | 2.9 | 52.7 | 66011/1182 | 0.1438 |  |
| `followup-base` | yes | yes | yes | yes | 1 | 5 | 2.9 | 23.4 | 18022/799 | 0.0440 |  |
| `followup-double` | yes | yes | yes | yes | 1 | 5 | 3.5 | 16.1 | 32994/431 | 0.0703 |  |
| `followup-lte` | yes | yes | yes | yes | 1 | 6 | 2.3 | 25.7 | 57843/688 | 0.1226 |  |
| `vague-healthy` | yes | **no** | yes | yes | 1 | 2 | 4.7 | 9.4 | 2341/155 | 0.0062 |  |
| `vague-bottleneck` | yes | yes | yes | yes | 1 | 7 | 3.4 | 47.9 | 29325/813 | 0.0668 |  |
| `vague-better` | **no** | yes | yes | **no** | 1 | 3 | 4.3 | 14.2 | 6022/348 | 0.0155 | 1 run(s), expected 0; started 1 run(s) instead of asking |
| `impossible-wifi7` | yes | yes | yes | yes | 0 | 8 | 2.9 | 27.7 | 32609/780 | 0.0730 |  |
| `impossible-tokyo` | yes | yes | yes | yes | 0 | 5 | 2.9 | 19.7 | 16879/425 | 0.0380 |  |
| `impossible-huge` | yes | yes | yes | yes | 0 | 4 | 3.0 | 14.7 | 13179/392 | 0.0303 |  |
| `ambiguous-compare` | yes | yes | yes | yes | 0 | 2 | 4.0 | 6.8 | 1958/84 | 0.0048 |  |
| `ambiguous-attack` | yes | yes | yes | yes | 1 | 6 | 3.5 | 35.9 | 29880/543 | 0.0652 |  |

## Answers

### `normal-baseline`

> Run the procedural grid for 30 seconds at 1500 vehicles per hour and tell me how the V2X network performs.

Tools: list_scenarios, prepare_scenario, run_prepared, finding_detail!, registry__metric, registry__metric, registry__metric, registry__metric

    ## V2X network results
    
    The procedural-grid run completed for **30 seconds** at **1500 vehicles per hour**. The main limitation was weak received signals, rather than channel congestion.
    
    ### Main bottleneck
    - **Weak signal [channel.weak-signal]:** Below-sensitivity receptions accounted for a loss share of **1 ratio**. This was a run-wide finding; no specific location was supplied.
    
    ### Network performance
    | Metric | Reported result |
    |---|---:|
    | Application packet delivery | 0.0 ratio |
    | Channel busy ratio | 0.0 ratio |
    | MAC access delay | 0.00005 ms |
    | Application goodput | 366.5 B/s |
    
    The delivery and goodput headlines differ in their aggregation and should not be treated as interchangeable: the reported delivery ratio is zero, while reported goodput is nonzero.
    
    **Not measured:** credential backend and privacy.

### `normal-channel`

> How busy is the radio channel on the Midtown grid with moderate traffic, about 2000 vehicles per hour? Keep the run to 30 s.

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, run_prepared, registry__metric, registry__metric

    The radio channel was **lightly loaded** on the Midtown-shaped procedural grid: channel busy ratio was **0.000117 ratio**, and offered airtime against capacity was **0.003207 ratio**. Weak reception—not congestion—was the main limitation.
    
    ### Bottlenecks
    - **Weak signal [channel.weak-signal]:** Across the run, below-sensitivity reception accounted for **0.998 ratio** of losses.
    - **Verification [access.verification]:** In the flagged time window, messages left unverified reached **0.051 ratio**; verification queue depth at p95 was **1 messages**, with p95 wait **0 ms**.
    - **Reception range [delivery.range]:** Reception fell below the range criterion from **500 m**.
    - **Signing latency [latency.dominant-stage]:** Across the run, signing was the largest latency stage, averaging **9 ms**.
    
    Channel busy ratio excludes hidden-terminal energy. **Credential backend and privacy were not measured.**

### `normal-lte`

> Simulate the grid with LTE-V2X sidelink instead of DSRC for 30 s at 1500 veh/h and summarise delivery and latency.

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, prepare_scenario, run_prepared

    Ran the procedural grid with **LTE-V2X PC5 sidelink for 30 s at 1500 veh/h**. Medium-access waiting dominated latency, while delivery degraded at longer range; no DSRC comparison was run.
    
    ### Bottlenecks
    - **Access delay — critical** [access.delay]: Across the run, p95 access delay was **77.2 ms**, above the analyst’s **20 ms** allowance.
    - **Weak-signal losses** [channel.weak-signal]: Fading accounted for a **1 ratio** share of recorded losses.
    - **Long-range delivery** [delivery.range]: Reception fell below **0.9 ratio** from **500 m**; PRR in the **500–520 m** bin was **0.154 ratio**.
    - **Latency breakdown** [latency.dominant-stage]: Across the run, `mac_defer` was the largest stage, averaging **52.6 ms** with a **0.773 ratio** share of total latency. Signing averaged **9 ms**, handoff **5.079 ms**, airtime **1 ms**, and verification **0.4 ms**.
    
    **Not measured:** credential backend and privacy. The returned analysis did not report an overall delivery ratio or end-to-end latency percentile.

### `congestion-rush`

> Rush hour on the grid - 20,000 vehicles per hour for 40 seconds. Find the bottlenecks.

Tools: list_scenarios, prepare_scenario!, get_scenario!, get_scenario, prepare_scenario, run_prepared

    ## Rush-hour grid: bottlenecks
    
    The grid run completed at **20,000 vehicles per hour for 40 seconds**. The main bottleneck was weak-signal reception rather than collisions, with signature verification and signing latency also flagged.
    
    ### Bottlenecks, most severe first
    
    - **Weak-signal losses across the grid** [channel.weak-signal]  
      Below-sensitivity signals accounted for **0.993 ratio** of packet losses; collisions accounted for **0.00072 ratio**.
    
    - **Signature verification shortfall** [access.verification]  
      In the flagged early-run window, **0.07 ratio** of messages remained unverified. Verification queue depth was **0.658 messages** at p95, although verification wait was **0 ms** at p95—so the finding does not show sustained queueing delay.
    
    - **Reception degrades with distance** [delivery.range]  
      Reception fell below the analyst’s reliability threshold in the **80–100 m** bin: PRR was **0.861 ratio**, declining to **0.466 ratio** at **140–160 m**.
    
    - **Signing dominates latency** [latency.dominant-stage]  
      Across the run, signing took **9 ms** on average and represented **0.736 ratio** of total latency. Handoff averaged **2.494 ms**, while airtime averaged **0.328 ms**.
    
    **Not measured:** credential backend and privacy. The analyst did not flag a road-traffic bottleneck; these findings primarily describe communication and security-processing limits.

### `congestion-light-vs-rush`

> Compare light traffic (500 veh/h) against rush hour (15,000 veh/h) on the grid, 30 s each. What changes?

Tools: list_scenarios, prepare_scenario!, get_scenario, prepare_scenario, run_prepared, prepare_scenario, run_prepared, compare_runs, registry__metric, registry__metric, registry__metric

    Rush hour slowed traffic and increased network-wide offered load, but did **not** produce MAC queue congestion in these grid runs. Its main V2X limitations were weak-signal reception and signing latency; light traffic did not return delivery or latency headline measurements, so those cannot be compared directly.
    
    ### What changed
    Both runs lasted **30 s**, with only demand changed from **500 veh/h** to **15,000 veh/h**.
    
    | Measurement | Light traffic | Rush hour | Change |
    |---|---:|---:|---:|
    | Mean sampled speed | 7.128 m/s | 4.602 m/s | −2.526 m/s |
    | Network-wide offered load | 31732 bit/s | 374537 bit/s | +342804 bit/s |
    | Channel busy ratio | 0 ratio | 0.00143 ratio | +0.00143 ratio |
    
    MAC drops and queue depth remained zero in both runs. Offered load is a network-wide total, not the load on a single shared channel.
    
    ### Rush-hour bottlenecks
    - **Weak signals — [channel.weak-signal]:** Across the run, below-sensitivity reception accounted for **0.994 ratio** of packet losses; collisions accounted for **0.00044 ratio**.
    - **Limited reception range — [delivery.range]:** Reception fell below **0.9 ratio** starting in the **80–100 m** bin, where PRR was **0.814 ratio**; at **140–160 m**, it was **0.362 ratio**.
    - **Signing latency — [latency.dominant-stage]:** Across the run, signing averaged **9 ms**, accounting for **0.735 ratio** of total latency.
    
    **Not measured in either run:** credential backend and privacy.

### `attack-constpos`

> Add a ConstPos attacker population (5% of vehicles) to the grid at 3000 veh/h for 60 s with misbehaviour detection on. Does detection catch them?

Tools: list_scenarios, get_scenario!, get_scenario, get_scenario, prepare_scenario, run_prepared, registry__model_card, registry__model_card, registry__metric

    **Detection catching the attackers is not demonstrated by this run.** The grid configuration was accepted at 3000 veh/h for 60 s with a 0.05 attacker fraction and local detection enabled, but the results contain no detection counts, recall, false-positive rate, or detection timing.
    
    There is also a configuration caveat: although validation passed, the registry reports that neither `threat/attacker/legacy/ConstPos` nor `detect/legacy-12` is registered in this build. I therefore cannot confirm that the requested attackers and detector actually operated.
    
    ### Reported bottlenecks
    - **Weak signal across the grid** [channel.weak-signal]: below-sensitivity losses dominated, with a reported loss-share ratio of 0.994.
    - **Reception range** [delivery.range]: PRR was 0.899 ratio at 80–100 m, falling to 0.333 ratio at 140–160 m.
    - **Signing latency** [latency.dominant-stage]: signing was the largest latency stage, averaging 9 ms.
    
    **Measurement gaps:** the credential backend was explicitly not measured [backend.not-measured]. Detection outcomes were absent from the returned analysis, so this is inconclusive—not evidence that detection succeeded or failed.

### `attack-randompos-vs-baseline`

> Run a baseline and the same scenario with 10% RandomPos attackers, same seed, 40 s at 3000 veh/h on the grid, and report the differences.

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, run_prepared, get_scenario, prepare_scenario, run_prepared, compare_runs

    ## Summary
    Ran the procedural grid baseline and the matching RandomPos case with attacker fraction **0.1**, both for **40 s** at **3000 veh/h**, retaining seed `0x000000c0ffee5eed`. The comparison found **no change in any shared headline metric** and no added or removed substantive findings.
    
    ## Bottlenecks — unchanged in both runs
    - **Weak signal [channel.weak-signal]:** Across the grid, below-sensitivity losses dominated, with a reported loss-share ratio of **0.998**.
    - **Reception range [delivery.range]:** PRR was **1 ratio** at **100–120 m**, falling to **0.647 ratio** at **140–160 m**.
    - **Signing latency [latency.dominant-stage]:** Signing was the largest latency stage: **9 ms**, with **0.732 ratio** of total latency.
    
    ## Differences and limits
    The comparator reported change **0** for every shared metric, including delivery ratio, latency, channel load, goodput, speed, and TTC conflicts.
    
    Credential backend was **not measured in either run**; privacy was **not measured in the baseline**, while the attack run returned limited pseudonym metrics. No local detectors were configured, and attack-detection effectiveness was not reported—so unchanged performance does **not** establish that RandomPos attacks were harmless or detected.

### `attack-speed`

> What does a ConstSpeedOffset attacker (10% of vehicles) do to the grid network at 3000 veh/h over 40 s?

Tools: list_scenarios, get_scenario!, get_scenario, get_scenario, prepare_scenario, run_prepared, registry__model_card

    The grid run completed with the requested ConstSpeedOffset configuration; its main measured limitation was weak-signal delivery, not channel congestion. **The results do not isolate the attacker’s effect**: no attack-specific finding was returned, and no attack-free comparison was run.
    
    ### Bottlenecks, most severe first
    - **Weak signals across the network** [channel.weak-signal]: below-sensitivity accounted for **0.998 ratio** of losses.
    - **Limited reception range** [delivery.range]: reception was **1 ratio** at **100–120 m**, falling to **0.647 ratio** at **140–160 m**.
    - **Signing dominated latency** [latency.dominant-stage]: mean signing time was **9 ms**, representing **0.732 ratio** of total latency.
    - **Irregular distance curve** [delivery.non-monotone]: reception rose by **0.941 ratio** between **740 m** and **820 m**; this limits how cleanly the range curve can be interpreted.
    
    ### What this says about the attacker
    The scenario accepted the requested attacker fraction with default attack parameters, but the grid base had no local misbehaviour detectors configured. Consequently, this run does **not** establish detection success or quantify harm attributable specifically to falsified speed; the build also supplied no model card explaining this attacker’s implementation.
    
    **Not measured:** credential backend [backend.not-measured].

### `attack-replay`

> Simulate a data replay attack by 5% of vehicles on the grid for 40 s at 3000 veh/h and say whether the detectors notice.

Tools: list_scenarios, get_scenario!, get_scenario, get_scenario, prepare_scenario, run_prepared

    The grid replay-attack run completed with the requested configuration. **Whether the detectors noticed is inconclusive:** the local detector suite was enabled, but the returned analysis contains no detection alerts, detection rate, or attacker-level results—so it does not establish either detection or evasion.
    
    ### Main bottlenecks
    - **Weak signal** [channel.weak-signal]: Across the grid, below-sensitivity losses dominated, with a loss share of **0.998 ratio**. This limits opportunities to receive attack messages.
    - **Reception range** [delivery.range]: Reception was **1 ratio** at **100–120 m**, falling to **0.647 ratio** at **140–160 m**.
    - **Signing latency** [latency.dominant-stage]: Signing was the largest latency stage, averaging **9 ms**, with **0.732 ratio** of total latency.
    
    ### Measurement limits
    - **Credential backend was not measured** [backend.not-measured].
    - Detector outcomes were **not reported in the returned results**; enabling detectors alone is not evidence that they caught the replay.

### `attack-signature`

> What happens to signature verification when 10% of vehicles on the grid send messages with invalid signatures? 40 s at 3000 veh/h.

Tools: list_scenarios, get_scenario!, get_scenario, get_scenario, prepare_scenario, run_prepared!, registry__model_card, prepare_scenario, run_prepared, registry__metric, finding_detail!

    Signature verification did **not show a queueing bottleneck** in the requested grid run: mean verification time was **0.4 ms**, with **0 ms** mean verification-queue delay. The results do not report invalid-signature rejection counts, so they cannot confirm how many bad signatures were rejected.
    
    ### Bottlenecks and findings
    - **Weak reception** [channel.weak-signal]: across the grid, below-sensitivity losses dominated, with a reported loss share of **0.998 ratio**.
    - **Signing, not verification, dominated latency** [latency.dominant-stage]: mean signing time was **9 ms**, versus **0.4 ms** for verification; signing accounted for **0.732 ratio** of total stage latency.
    - **Reception fell with range** [delivery.range]: PRR was **1 ratio** at **100–120 m**, falling to **0.647 ratio** at **140–160 m**.
    
    The **unverified-delivery ratio was 0.017045 ratio**. This means messages delivered to applications without verification—not invalid signatures accepted—and excludes dropped messages.
    
    **Not measured:** the credential backend. No clean-signature baseline was run, so these results do not establish the attack’s incremental verification cost.

### `attack-expired-cert`

> Give 10% of the grid's vehicles expired certificates for 40 s at 3000 veh/h. How many of their messages are accepted?

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, get_scenario, prepare_scenario, run_prepared!, prepare_scenario, run_prepared, registry__metric, registry__model_card

    The grid run completed for **40 s at 3000 veh/h**, with **10% of vehicles configured with expired certificates**. **The reported results do not include an accepted-message count for those vehicles**, so I can’t tell you how many of their messages were accepted.
    
    The overall `unverified_ratio` was **0.017045 ratio**—messages delivered to applications without verification—but it does not isolate expired-certificate senders.
    
    Main bottlenecks:
    - **Weak signal** [channel.weak-signal]: below-sensitivity accounted for **0.998 ratio** of reported losses across the grid.
    - **Signing latency** [latency.dominant-stage]: mean signing time was **9 ms**, the largest latency stage.
    
    **Not measured:** credential backend. Expired-certificate-specific acceptance was not reported.

### `attack-wave`

> Schedule an attack wave on the grid - 10% StopAndGo attackers active only from 20 s to 40 s of a 60 s run at 3000 veh/h.

Tools: list_scenarios, get_scenario!, get_scenario, get_scenario, prepare_scenario, run_prepared

    Completed the procedural-grid run at **3000 veh/h** for **60 s**, with StopAndGo attackers configured at **0.1 fraction**, active only from **20 s to 40 s**. The main measured bottlenecks were weak-signal losses and limited reception range; the findings did not isolate the attack wave’s impact.
    
    ### Bottlenecks
    - **Weak signals across the grid** [channel.weak-signal]: below-sensitivity losses had a **0.994 ratio** share of losses.
    - **Reception range** [delivery.range]: PRR was **0.899 ratio** at **80–100 m**, falling to **0.333 ratio** at **140–160 m**.
    - **Signing latency** [latency.dominant-stage]: signing averaged **9 ms**, with a **0.731 ratio** share of total latency.
    
    **Other observation:** reception was non-monotonic, rising by **0.643 ratio** from **140 m to 200 m** [delivery.non-monotone].
    
    **Not measured:** the credential backend [backend.not-measured].

### `attack-sybil`

> Simulate a Sybil attack where one car fakes twenty ghost vehicles.

Tools: list_scenarios, get_scenario!, get_scenario, registry__model_card, prepare_scenario, prepare_scenario, run_prepared!, prepare_scenario, registry__model_card, prepare_scenario

    I couldn’t run the requested attack: the simulator rejected the configuration for **one car impersonating twenty ghost vehicles**. Its configuration interface did not expose a supported parameter for setting the ghost-vehicle count.
    
    **No simulation completed**, so there are no measured attack effects, detection results, or network impacts to report. I haven’t substituted a different attack or an unspecified number of ghosts.

### `jam-constant`

> Put a constant jammer in the middle of the grid (3000 veh/h, 30 s) and tell me how many frames it kills.

Tools: list_scenarios, get_scenario!, get_scenario, registry__model_card, prepare_scenario, prepare_scenario, prepare_scenario, registry__model_card, prepare_scenario, run_prepared, finding_detail

    The constant-jammer run completed at **3000 veh/h for 30 s**. **The available results do not report a jammer-attributed frame count**, so I can’t tell you how many frames it killed or attribute all losses to it.
    
    ### Main bottlenecks
    - **Weak signal [channel.weak-signal]:** Across the grid, below-sensitivity losses had a reported share of **0.998436 ratio**. The analyst attributes these to distance, fading and buildings—not load.
    - **Reception range [delivery.range]:** Reception was **1 ratio** at **100–120 m**, but **0 ratio** at **400–420 m** and **460–480 m**.
    - **Signing delay [latency.dominant-stage]:** Signing averaged **9 ms**, the largest latency stage.
    
    **Not measured:** credential backend and privacy.

### `jam-dsrc-vs-lte`

> Is LTE-V2X more robust to a reactive jammer than DSRC? Compare both on the grid with the same jammer, 30 s at 3000 veh/h.

Tools: list_scenarios, get_scenario!, get_scenario, registry__model_card, prepare_scenario, prepare_scenario, prepare_scenario, registry__model_card, prepare_scenario, run_prepared, prepare_scenario, prepare_scenario, run_prepared, compare_runs!, compare_runs, registry__metric


### `revocation`

> How long does it take to revoke a misbehaving vehicle's certificates in the credential-lifecycle scenario? Run it and break the time down by stage.

Tools: list_scenarios, prepare_scenario, run_prepared, finding_detail!, finding_detail

    The **credential-lifecycle** scenario ran unchanged, and the longest measured revocation stage was report transport: **0.287 s**. The analyst returned stage timings but no end-to-end total or CRL-installation timing, so I cannot give a measured time until every receiver rejects the vehicle’s certificates.
    
    ### Revocation breakdown
    [security.revocation-latency]
    
    | Stage | Measured time |
    |---|---:|
    | Detection → report sent | 0.009 s |
    | Report sent → report received | 0.287 s |
    | Report received → decision | 0.002 s |
    | Decision → resolved | 0.022 s |
    | Resolved → issued | 0.055 s |
    
    **Revocation bottleneck:** report transport to the authority, followed by resolution-to-issuance. These findings were run-wide; no vehicle or location breakdown was supplied.
    
    ### Other run warnings
    - **Weak signal** [channel.weak-signal]: below-sensitivity accounted for **0.988 ratio** of packet losses, run-wide.
    - **Reduced reception range** [delivery.range]: packet reception was **0.853 ratio** in the **80–100 m** distance bin.
    - **Stale neighbour information** [delivery.stale-information]: age of information at p95 was **659 ms**, against a **300 ms** threshold.
    - **Tracking persisted** [privacy.linkable]: run-wide linkability was **0.963 ratio** despite pseudonym changes.
    
    **Measurement coverage:** no layers were listed as unmeasured. However, the returned revocation breakdown did not include attack-onset-to-detection, issuance-to-CRL-installation, or an end-to-end total.

### `privacy-period`

> How linkable are vehicles to a roadside eavesdropper with 300 s pseudonym changes versus 60 s? Use the pseudonym-strategies scenario.

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, run_prepared, prepare_scenario, run_prepared, compare_runs, registry__metric, registry__model_card

    The **60 s run reported a raw `linkability` value of 0.235943**, but the **300 s run reported no linkability value**, so these runs cannot establish how much shorter periods improve privacy. Both used the requested pseudonym-strategies scenario, with only the time-based change period varied.
    
    ### Privacy comparison
    - The scenario retained its roadside observer configuration: `range_m: 100.0` and `sniffer_fraction: 0.25`.
    - The simulation duration remained **300 s**. The baseline reported `pseudonym_change_rate: 0.0`, so it provided no measured comparison across pseudonym changes.
    - The metric registry supplies **no definition or unit for `linkability`**, and the observer has no registered model card in this build. Consequently, **0.235943 should not be interpreted as a percentage of vehicles successfully tracked**.
    
    ### Other bottlenecks
    Values below are **300 s period → 60 s period**:
    - **[channel.weak-signal]** Across the network, below-sensitivity reception dominated losses: **0.986 ratio → 0.987 ratio**.
    - **[delivery.range]** In the **80–100 m** distance bin, reception was **0.833 ratio → 0.834 ratio**.
    - **[delivery.stale-information]** Network-wide information age at p95 was **678 ms → 679 ms**.
    - **[latency.dominant-stage]** Signing remained the largest latency component, averaging **9 ms** in both runs.
    
    **Measurement limits:** The analyst flagged no unmeasured layers, but the requested privacy comparison is incomplete: baseline linkability, anonymity-set size, and tracking duration were not returned. A missing baseline value is **not zero linkability**.

### `privacy-mixzone`

> Compare the mix-zone pseudonym strategy with the J2945/1 time-based strategy against the eavesdropper in the pseudonym-strategies scenario.

Tools: list_scenarios, get_scenario, prepare_scenario, run_prepared, prepare_scenario, run_prepared, compare_runs, registry__metric, registry__metric

    **Mix-zone triggered pseudonym changes, while J2945/1 time-based did not during the retained scenario duration. However, the results do not establish which better defeats the eavesdropper: linkability was returned only for mix-zone, leaving no comparable time-based value.**
    
    ### Privacy comparison
    
    Both runs used the same fleet and roadside eavesdropper settings; only the strategy changed.
    
    | Returned metric | J2945/1 time | Mix-zone |
    |---|---:|---:|
    | `pseudonym_change_rate` | 0.0 | 26.966928 |
    | `linkability` | Not returned | 0.135197 |
    
    The metric registry does not publish definitions or units for these metrics in this build, so the linkability value should **not** be interpreted as a confirmed tracking-success percentage.
    
    ### Bottlenecks, most severe first
    
    - **Weak signals [channel.weak-signal]:** Across the network, below-sensitivity reception dominated losses: **0.986 ratio** for time-based versus **0.987 ratio** for mix-zone.
    - **Limited reception range [delivery.range]:** In the **80–100 m** distance bin, PRR was **0.833 ratio** versus **0.834 ratio**, respectively.
    - **Stale neighbour information [delivery.stale-information]:** Network-wide age of information at p95 was **678 ms** versus **676 ms**, both above the **300 ms** threshold.
    - **Signing latency [latency.dominant-stage]:** Signing averaged **9 ms** in both runs and was the largest latency component.
    
    ### What changed—and what remains missing
    
    The comparison showed little delivery change; mix-zone increased security overhead and the unverified ratio. Neither strategy introduced a unique analyst finding.
    
    The analyst listed **no unmeasured layers**, but the returned results omitted **time-based linkability, anonymity-set size, and tracking duration**. Those omissions prevent a complete comparison against the eavesdropper.

### `rat-three-way`

> DSRC vs LTE-V2X vs NR-V2X on the grid at 3000 veh/h for 30 s - who delivers further?

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, run_prepared, prepare_scenario, run_prepared, compare_runs, prepare_scenario, run_prepared, compare_runs, compare_runs

    **DSRC and NR-V2X tie on the measured reliable-range criterion:** reception first fell below a PRR of 0.9 from **400 m**, versus **80 m** for LTE-V2X. That is not a clean maximum-range ranking: all runs had non-monotone delivery curves, so reception could recover farther away.
    
    ### Delivery versus distance
    Same procedural grid, **3000 veh/h**, **30 s**; only the radio technology changed.
    
    | Radio | First reported fall below PRR 0.9 | PRR at 100–120 m | PRR at 400–420 m |
    |---|---:|---:|---:|
    | DSRC | 400 m | 1 ratio | 0 ratio |
    | LTE-V2X | 80 m | 0.5 ratio | Not reported |
    | NR-V2X | 400 m | 1 ratio | 0.011 ratio |
    
    [delivery.range] NR-V2X retained some delivery in the reported distant bins where DSRC reported none, but this does **not** establish a farther reliable range.
    
    ### Bottlenecks, most severe first
    - **LTE-V2X medium access** [access.delay]: p95 access delay was **94.4 ms**; NR-V2X improved this to **35.8 ms**, still flagged as a warning.
    - **Propagation losses across the grid** [channel.weak-signal]: DSRC’s dominant loss cause was below-sensitivity, **0.998 ratio**; LTE-V2X and NR-V2X were dominated by fading, **0.699 ratio** and **0.994 ratio** respectively.
    - **LTE-V2X stale neighbour information** [delivery.stale-information]: p95 information age reached **400 ms**, above the **300 ms** threshold.
    - **Irregular distance curves** [delivery.non-monotone]: every radio showed reception rising between **740 m** and **820 m**, limiting a simple “furthest delivery” conclusion.
    
    ### What changed
    Overall delivery ratio ranked **LTE-V2X > NR-V2X > DSRC**—a different result from reliable range. The comparison reported a **0.223** increase from DSRC to LTE-V2X and a **0.077** increase from DSRC to NR-V2X.
    
    **Not measured:** credential backend and privacy in all runs.

### `radio-region-eu`

> Run the grid under European regulation instead of the US band plan, 30 s at 3000 veh/h, and compare the channel load with the US setup.

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, prepare_scenario, run_prepared, prepare_scenario, prepare_scenario, run_prepared, compare_runs!, compare_runs, registry__metric

    Completed the grid runs for **30 s at 3000 veh/h**, changing only the regulatory region between comparisons. European channel load was slightly higher, but weak signal—not channel congestion—was the main limitation.
    
    ### Channel-load comparison
    
    | Metric | US deployment-era plan | European regulation | EU change |
    |---|---:|---:|---:|
    | Channel load (ratio) | 0.00351 | 0.00371 | +0.0002 |
    
    Channel load measures offered airtime against capacity; these results do not indicate saturation. **The US comparison uses `us-2016`**, because the simulator rejects this DSRC setup under current US regulation.
    
    ### Main limitations
    - **Weak signal [channel.weak-signal]:** Across the grid, below-sensitivity losses dominated: loss-share ratio **0.996** in Europe versus **0.998** under the US plan.
    - **Reception range [delivery.range]:** In the **400–420 m** distance bin, reception ratio was **0.00515** in Europe versus **0** in the US run.
    - **Signing latency [latency.dominant-stage]:** Signing was the largest latency stage in both runs, averaging **9 ms**.
    
    **Not measured:** credential backend and privacy in both runs. Channel busy ratio was reported only for the US run, so it could not be compared directly.

### `overhead-cert-cadence`

> What share of the air bytes is security overhead on the grid (3000 veh/h, 30 s) when the full certificate is attached every 1000 ms versus every 450 ms?

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, run_prepared, prepare_scenario, run_prepared, compare_runs, registry__metric

    On the grid at **3000 veh/h over 30 s**, attaching certificates more frequently increased the security-overhead share of air bytes. The measured shares were:
    
    | Full-certificate interval | Security overhead (ratio of air bytes) |
    |---|---:|
    | 1000 ms | **0.519179** |
    | 450 ms | **0.54911** |
    
    The comparison reports an increase of **0.03 ratio**, with delivery ratio unchanged.
    
    Security overhead includes envelope headers, the signer identifier (certificate or digest), and signature—not just certificates. It excludes PHY preamble/tail and frames whose envelope size came from a table.
    
    **Other bottlenecks, unchanged in character:**
    - [channel.weak-signal] Across the grid, below-sensitivity accounted for **0.998 ratio** of losses.
    - [latency.dominant-stage] Signing dominated latency in both runs, averaging **9 ms**.
    
    **Not measured:** credential backend and privacy.

### `overhead-pq-mldsa`

> Classical ECDSA versus post-quantum hybrid ML-DSA-44 signatures on the grid at 3000 veh/h for 30 s - what does PQ cost in channel load and verification?

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, run_prepared, prepare_scenario, prepare_scenario, prepare_scenario, run_prepared, prepare_scenario, run_prepared, compare_runs, registry__metric, registry__metric, registry__metric, registry__model_card, registry__model_card, registry__model_card

    **The comparison did not yield a valid measured PQ overhead:** the hybrid run reported no offered traffic and no verification samples. Both matched runs used the same grid, 3000 veh/h, 30 s, and Cohda MK6C hardware profile, changing only ECDSA-P256 to ML-DSA-44 + ECDSA-P256.
    
    ### Channel load and verification
    
    | Measurement | ECDSA | Hybrid ML-DSA-44 |
    |---|---:|---:|
    | Network-wide offered load | 81221.517241 bit/s | 0.0 bit/s |
    | Channel load | 0.003503 ratio | 0.0 ratio |
    | Channel busy ratio | 0.000428 ratio | 0.0 ratio |
    | Mean verification stage | 1 ms | No sample reported |
    | Mean verification queue stage | 0 ms | No sample reported |
    | Mean frame airtime | 0.324 ms | No sample reported |
    
    **The hybrid zeros are not a channel-load saving:** no PSDU bits were handed to the MAC. The available analysis does not explain why, so a measured hybrid verification or airtime penalty cannot be claimed.
    
    The hardware model does publish **189.0 us** for the ML-DSA-44 verification primitive and **2634.0 us** for signing, citing NDSS measurements labeled “Dilithium.” These are model inputs—not observed hybrid service times or queueing costs in this run.
    
    ### Bottlenecks observed in the ECDSA baseline
    
    - **Weak signal across the grid** [channel.weak-signal]: below-sensitivity accounted for **0.999 ratio** of losses.
    - **Signing dominated measured latency** [latency.dominant-stage]: mean signing was **7.82 ms**, versus **1 ms** verification; signing’s latency share was **0.665 ratio**.
    - **Range limitation** [delivery.range]: reception ratio was **0** in the **400–420 m** bin.
    
    **Not measured:** credential backend and privacy in both runs; hybrid verification, delivery, and latency had no reported samples.

### `overhead-falcon`

> Would hybrid Falcon-512 signatures fit on DSRC at 5000 veh/h? Check it on the grid for 30 s.

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, prepare_scenario, prepare_scenario, run_prepared, registry__metric, finding_detail

    **Hybrid Falcon-512 + ECDSA-P256 fit the simulated DSRC channel, but verification could not keep up.** The grid run at 5000 veh/h for 30 s measured a channel busy ratio of 0.002666 ratio, so channel saturation was not the main constraint.
    
    ### Bottlenecks
    - **Verification [access.verification]:** Messages left unverified were 1 ratio; verification queue depth at p95 was 1 messages. The analyst attributed this to security-processor throughput rather than channel capacity.
    - **Weak signal [channel.weak-signal]:** Across the grid, below-sensitivity reception accounted for 0.99 ratio of losses.
    - **Signing latency [latency.dominant-stage]:** Mean signing time was 9.972 ms, contributing 0.656 ratio of total latency. Mean airtime was 1.192 ms.
    - **Range [delivery.range]:** Reception was 1 ratio at 20–40 m and fell to 0.656 ratio at 120–140 m.
    
    The run used a supported Cohda OBU because the base hardware lacked published costs for both signature halves. **The result supports radio feasibility under these conditions, not reliable verified delivery.**
    
    **Not measured:** credential backend and privacy.

### `emergency-priority`

> Run the connected-intersections scenario for 60 s and tell me whether emergency vehicle priority requests get through to the signals.

Tools: list_scenarios, prepare_scenario, run_prepared

    The connected-intersections scenario completed its **60 s** run, but **the reported results do not confirm whether emergency priority requests reached the signals**. The analysis reports aggregate radio and delivery performance, not SRM reception or SSM acknowledgements, so successful priority-request delivery cannot be established.
    
    ### Main bottlenecks
    - **Weak signals** [channel.weak-signal]: Across the run, below-sensitivity reception accounted for **0.978 ratio** of packet losses; no specific intersection was identified.
    - **Stale information** [delivery.stale-information]: Across receivers, information age reached **432 ms** at p95, exceeding the **300 ms** stale threshold.
    - **Limited reliable range** [delivery.range]: In the **100–120 m** distance bin, packet reception was **0.832 ratio**.
    - **Signing delay** [latency.dominant-stage]: Signing was the largest latency stage, averaging **25.1 ms** across the run.
    
    **Measurement gap:** Credential-backend activity was not measured. Request-specific delivery, acknowledgement, and signal-priority activation results were not reported; the aggregate findings above do not establish that emergency requests failed—or succeeded.

### `pedestrians`

> Simulate 200 pedestrians on the VRU grid scenario for 60 s and tell me about conflicts between pedestrians and vehicles.

Tools: list_scenarios, get_scenario, prepare_scenario, run_prepared, finding_detail!, registry__metric, registry__metric, finding_detail

    Ran the **VRU grid with 200 pedestrians for 60 s**, retaining the preset’s vehicle traffic and cyclists. The run flagged traffic conflicts, but **did not report pedestrian–vehicle conflicts separately**, so those cannot be quantified from these results.
    
    ### Findings, most severe first
    - **Close-range message loss** [delivery.near-range-loss]: packet reception was **0.902 ratio** at **20–40 m**, potentially limiting nearby actors’ awareness.
    - **Weak signals and stale information** [channel.weak-signal] [delivery.stale-information]: below-sensitivity signals accounted for **0.993 ratio** of losses; information age reached **895 ms** at the **p95**, exceeding the **300 ms** staleness threshold.
    - **Traffic conflicts** [traffic.conflicts]: flagged in the **4.0–6.0 s** window, with a reported **38.5 count** and **p95** crash-avoidance deceleration of **1.459 m/s²**. Crucially, the metric counts repeated **same-lane vehicle-pair steps**, not distinct pedestrian–vehicle encounters. Its reported minimum TTC also conflicts with the stated threshold, so it should not be treated as a reliable pedestrian-safety count.
    
    **Measurement gaps:** no pedestrian-specific conflict breakdown or encounter locations were returned. Credential-backend and privacy layers were not measured.

### `weather-rain`

> Heavy rain (intensity 0.8) on the grid at 3000 veh/h for 30 s - how does it change delivery and traffic speed compared with clear weather?

Tools: list_scenarios, get_scenario, prepare_scenario, run_prepared, prepare_scenario, run_prepared, compare_runs, registry__metric, registry__metric

    Heavy rain slowed traffic but **increased aggregate message delivery** in this grid comparison. Only weather changed between the clear and rain runs; demand and duration stayed fixed.
    
    ### Clear vs heavy rain
    
    | Metric | Clear | Heavy rain | Change |
    |---|---:|---:|---:|
    | Application delivery ratio | 0.161 ratio | 0.191 ratio | +0.03 ratio |
    | Mean traffic speed | 6.747 m/s | 5.925 m/s | −0.823 m/s |
    | Median traffic speed | 7.778 m/s | 6.932 m/s | −0.846 m/s |
    
    Delivery is measured across candidate-receiver attempts, not just nearby vehicles. The aggregate improvement does **not** establish that rain improved radio propagation; these results do not isolate the reason for the increase.
    
    ### Main bottlenecks
    
    - **Weak signals dominated losses** [channel.weak-signal]: across the grid, below-sensitivity losses had a reported share of **0.998 ratio** in both weather conditions.
    - **Reception remained range-limited** [delivery.range]: in both runs, reception fell below **0.9 ratio** from **400 m**; the **400–420 m** bin recorded **0 ratio**.
    - **Signing dominated latency** [latency.dominant-stage]: across the run, mean signing time was **9 ms**, accounting for **0.731 ratio** of total latency in both conditions.
    
    **Not measured:** credential backend and privacy. This is a single paired simulation, not a replicated weather study.

### `weather-fog-front`

> Start the grid in clear weather and have a fog front arrive at 20 s in a 40 s run at 3000 veh/h.

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, prepare_scenario, prepare_scenario, prepare_scenario, prepare_scenario, run_prepared

    Completed the procedural-grid run for **40 s**, starting clear with a fog front scheduled at **20 s** and demand of **3000 veh/h**. Weak-signal losses were the main bottleneck; these run-wide findings do not isolate fog’s effect.
    
    ### Bottlenecks
    - **Weak reception [channel.weak-signal]:** Across the run, below-sensitivity losses dominated, with a reported loss share of **0.998 ratio**.
    - **Reception range [delivery.range]:** PRR was **1 ratio** at **100–120 m**, falling to **0.711 ratio** at **140–160 m**.
    - **Signing latency [latency.dominant-stage]:** Across the run, signing averaged **9 ms**, accounting for **0.732 ratio** of total latency.
    
    **Caveat [delivery.non-monotone]:** Reception rose between **740 m** and **820 m**, so the distance curve was not consistently decreasing.
    
    **Not measured:** Credential backend and privacy.

### `event-closure`

> Close a street on the grid at 10 s and reopen it at 30 s in a 40 s run at 4000 veh/h, and tell me the traffic impact.

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, prepare_scenario, prepare_scenario, prepare_scenario, prepare_scenario, run_prepared, registry__metric, registry__metric

    The grid run completed with demand set to **4000 veh/h**, a street closure scheduled at **10 s**, reopening at **30 s**, and a **40 s** duration. Traffic speeds were measured, but the returned results do not isolate the closure’s effect on delay, queues, or rerouting.
    
    ### Traffic impact
    - **Speed across the run:** 6.447943 m/s; median 7.508943 m/s. These are sampled, time-weighted speeds—not a before/after closure comparison.
    - **Safety:** 0.0 count of same-lane pair-steps below the time-to-collision threshold.
    - **Attribution limit:** No closure-specific traffic finding or event-execution confirmation was returned. Without an open-street baseline, these results cannot establish how much the closure slowed traffic.
    
    ### Reported bottlenecks
    - **Weak radio signals** [channel.weak-signal]: across the run, below-sensitivity losses had a reported share of 0.995 ratio.
    - **Reception range** [delivery.range]: reception fell below the reference level in the 100–120 m distance bin, with PRR 0.765 ratio.
    - **Message signing** [latency.dominant-stage]: signing was the largest latency stage, averaging 9 ms across the run.
    
    **Not measured:** credential backend and privacy.

### `event-hard-brake`

> Script an emergency hard brake at 15 s on the grid (3000 veh/h, 30 s) and check whether forward collision warnings fire.

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, prepare_scenario, prepare_scenario, prepare_scenario, run_prepared, registry__metric, registry__metric

    The grid run completed with **3000 veh/h**, a **30 s** duration, and a scripted `safety.hard-brake` at **15 s**. **Forward collision warning activation is unconfirmed:** the analyst reported **0.0 count** TTC conflicts, but this build exposes no `fcw_warnings` metric, and the report contains no FCW firing evidence.
    
    ### Bottlenecks
    - **Weak signal** [channel.weak-signal]: Across the run, below-sensitivity losses dominated, with a loss share of **0.998 ratio**.
    - **Reception range** [delivery.range]: PRR was **1 ratio** at **100–120 m**, but **0 ratio** at **400–420 m**.
    - **Signing latency** [latency.dominant-stage]: Signing was the largest latency component, averaging **9 ms**, with **0.731 ratio** of total latency.
    
    **Measurement limits:** TTC conflicts count same-lane pair-steps below the **1.5 s** threshold; they do not directly establish whether an FCW fired. Credential backend and privacy were not measured.

### `multi-four-variants`

> Run four variants on the grid at 3000 veh/h for 30 s, same seed: baseline, 5% ConstPos attackers, NR-V2X instead of DSRC, and heavy rain. Rank what hurts delivery most.

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, run_prepared, get_scenario, get_scenario, prepare_scenario, run_prepared, prepare_scenario, run_prepared, prepare_scenario, prepare_scenario, run_prepared, compare_runs, compare_runs, compare_runs, registry__metric

    Completed the independent grid variants at **3000 veh/h for 30 s**, all using seed `0x000000c0ffee5eed`. **None reduced application delivery versus baseline:** ConstPos tied baseline, while heavy rain and NR-V2X improved delivery in this run.
    
    ### Ranking: lowest delivery first
    
    | Variant | Delivery ratio | Change vs baseline |
    |---|---:|---:|
    | Baseline — DSRC, clear | 0.161093 | — |
    | ConstPos attackers — fraction 0.05 | 0.161093 | 0 |
    | Heavy rain — rain intensity 1.0 | 0.192447 | +0.031 |
    | NR-V2X | 0.222727 | +0.062 |
    
    All values and changes are **ratios**, not percentages. Changes use the comparison tool’s reported precision.
    
    ### Main bottlenecks
    - **Weak signal across the runs [channel.weak-signal]:** below-sensitivity dominated baseline, ConstPos and rain losses, with reported loss share **0.998 ratio**. NR-V2X losses were dominated by fading, **0.994 ratio**; no specific location was identified.
    - **NR-V2X access delay [access.delay]:** despite better delivery, medium-access delay reached **35.8 ms p95**, above the analyst’s **20 ms** allowance. This was a run-wide finding.
    - **Distance-limited reception [delivery.range]:** every variant fell below the analyst’s reception threshold from **400 m**. In the **400–420 m** bin, PRR was **0 ratio** for baseline, ConstPos and rain, versus **0.01 ratio** for NR-V2X.
    - **DSRC signing latency [latency.dominant-stage]:** signing averaged **9 ms** in baseline, ConstPos and rain, accounting for **0.731 ratio** of stage latency.
    
    **Interpretation:** ConstPos showed no delivery penalty, but that does not establish that its false positions were harmless or detected. Rain also changed mobility, so its delivery improvement should not be interpreted as rain improving radio propagation.
    
    **Not measured:** credential backend in every run; privacy in baseline, NR-V2X and rain. Delivery excludes attempts still in flight at the end and receivers outside the engine’s candidate range.

### `multi-density-sweep`

> Sweep demand over 1000, 5000 and 10000 veh/h on the grid (30 s each) and tell me where the channel starts to saturate.

Tools: list_scenarios, prepare_scenario!, get_scenario, prepare_scenario, run_prepared, prepare_scenario, run_prepared, compare_runs, prepare_scenario, run_prepared, compare_runs, registry__metric, registry__metric, registry__metric

    **Channel saturation was not observed at any tested demand**, including 10000 veh/h, in these 30 s grid runs. The dominant limitation was weak signal; the highest demand also triggered a signature-verification warning, not a channel-congestion finding.
    
    | Demand | Channel busy ratio (ratio) | MAC queue depth (count) | MAC drops (count) |
    |---|---:|---:|---:|
    | 1000 veh/h | 0.0 | 0.0 | 0.0 |
    | 5000 veh/h | 0.000714 | 0.0 | 0.0 |
    | 10000 veh/h | 0.001141 | 0.0 | 0.0 |
    
    ### Bottlenecks
    - **Weak signal** [channel.weak-signal]: Across the grid, below-sensitivity losses dominated at every demand. Their reported share was 1 ratio, 0.998 ratio and 0.995 ratio, respectively; no specific hotspot was identified.
    - **Verification capacity** [access.verification]: At 10000 veh/h, during the 9.0–10.0 s window, the unverified share was 0.059 ratio. Verification queue depth was 0.779 messages at p95, triggering the analyst’s warning.
    - **Reception range** [delivery.range]: At 5000 veh/h, reception fell below 0.9 ratio in the 120–140 m bin, with PRR 0.632 ratio. At 10000 veh/h, it fell below that level in the 100–120 m bin, with PRR 0.784 ratio.
    - **Signing latency** [latency.dominant-stage]: Signing was the largest latency component in both higher-demand runs, with a mean of 9 ms.
    
    ### What changed
    Channel busy ratio rose with demand, but MAC queues and drops remained unchanged. The new finding at 10000 veh/h was verification overload; this sweep therefore **does not locate a channel-saturation threshold**.
    
    These conclusions apply only to the requested short runs. CBR does not account for hidden-terminal energy; **credential backend and privacy were not measured**.

### `followup-base`

> Run the grid at 2000 veh/h for 30 s.

Tools: list_scenarios, prepare_scenario, run_prepared

    Completed the grid run at **2000 veh/h for 30 s**. Weak signal dominated packet losses, and the analyst also flagged signature verification.
    
    ### Bottlenecks and findings
    - **Weak signal** [channel.weak-signal]: Run-wide, the below-sensitivity share of losses was **0.998 ratio**.
    - **Signature verification** [access.verification]: In the flagged verification window, messages left unverified were **0.051 ratio**; p95 queue depth was **1 messages**, and p95 wait was **0 ms**.
    - **Reception range** [delivery.range]: PRR was **0 ratio** in the **500-520 m** and **520-540 m** distance bins.
    - **Signing latency** [latency.dominant-stage]: Run-wide, signing averaged **9 ms** and contributed **0.737 ratio** of total latency.
    - **Non-monotone reception** [delivery.non-monotone]: The analyst reported a PRR rise from **900 m to 920 m** of **0.615 ratio**.
    
    **Not measured:** credential backend and privacy.

### `followup-double`

> Now double the density.

Tools: prepare_scenario, run_prepared, compare_runs!, compare_runs

    Completed the grid run with demand doubled to **4000.0 veh/h**, keeping the **30 s** duration unchanged. Delivery improved relative to the previous run, but weak signal remained the main loss mechanism.
    
    ### Bottlenecks and findings
    - **Weak signal** [channel.weak-signal]: Run-wide, below-sensitivity losses accounted for **0.997 ratio** of losses.
    - **Reception range** [delivery.range]: PRR fell below **0.9** from **400 m**, with **0 ratio** in the **400-420 m**, **420-440 m**, **440-460 m**, and **460-480 m** bins.
    - **Signing latency** [latency.dominant-stage]: Run-wide, signing averaged **9 ms**, contributing **0.733 ratio** of total latency.
    - **Non-monotone reception** [delivery.non-monotone]: PRR rose by **0.875 ratio** from **740 m to 780 m**.
    
    ### Compared with the previous run
    - Delivery ratio increased from **0.088** to **0.166**; reported change: **0.078**.
    - Unverified ratio decreased from **0.051** to **0.022**; reported change: **-0.029**. The verification warning [access.verification] no longer appeared.
    
    **Not measured:** credential backend and privacy.

### `followup-lte`

> Switch that to LTE-V2X and compare with the previous run.

Tools: get_scenario, prepare_scenario, prepare_scenario, run_prepared, compare_runs

    Completed the **LTE-V2X PC5** run at **4000.0 veh/h for 30 s**, changing only the radio technology. Compared with the previous DSRC run, overall delivery improved, but latency and information freshness worsened.
    
    ### Bottlenecks, most severe first
    - **Medium-access delay** [access.delay]: Run-wide p95 access delay was **98.2 ms**, exceeding the analyst’s **20 ms** allowance.
    - **Close-neighbour losses** [delivery.near-range-loss]: PRR was **0.5 ratio** in the **40-60 m** bin.
    - **Latency over budget** [latency.over-budget]: In the flagged window, p95 end-to-end latency was **101 ms** against a **100 ms** budget. MAC deferral dominated latency [latency.dominant-stage], averaging **53.3 ms**.
    - **Fading losses** [channel.weak-signal]: Run-wide, fading accounted for **0.894 ratio** of losses.
    - **Verification warning** [access.verification]: In the flagged verification window, messages left unverified were **0.078 ratio**.
    - **Stale information** [delivery.stale-information]: Run-wide p95 age of information was **384 ms**, above the **300 ms** stale threshold.
    
    ### Compared with the previous run
    | Metric | Previous DSRC | LTE-V2X | Reported change |
    |---|---:|---:|---:|
    | Delivery ratio | 0.166 | 0.372 | 0.206 |
    | Mean end-to-end latency | 12.3 ms | 69.7 ms | 57.4 ms |
    | p95 end-to-end latency | 14.4 ms | 101 ms | 86.2 ms |
    | Unverified ratio | 0.022 | 0.078 | 0.055 |
    
    The access-delay, near-range-loss, over-budget-latency, stale-information, and verification findings were newly flagged.
    
    **Not measured:** credential backend and privacy.

### `vague-healthy`

> Is the network healthy?

Tools: analyse_current_run

    **Network health is undetermined.** The current run has no measured results to establish whether it is healthy.
    
    - **Assessment blocker — [simulation.incomplete]:** The current simulation reached **0 s** of its planned **60 s**, so no network bottlenecks can be identified.
    - **Not measured:** Traffic, access and queues, latency, security, credential backend, and privacy.
    
    Would you like me to run a scenario to check network health?

### `vague-bottleneck`

> Where's the bottleneck at rush hour on the grid?

Tools: list_scenarios, get_scenario!, get_scenario, get_scenario, prepare_scenario, run_prepared

    **The main V2X bottleneck is weak radio reception, not channel contention.** In this rush-hour grid test, the results locate the problem by communication distance—not at a particular intersection—so they don’t identify a road-traffic choke point.
    
    ### Bottlenecks, most important first
    - **Weak signal across the grid** [channel.weak-signal]: below-sensitivity reception accounts for **0.992 ratio** of losses; collisions account for **0.0009 ratio**.
    - **Delivery deteriorates with distance** [delivery.range]: reception falls below **0.9 ratio** from **80 m**. PRR is **0.832 ratio** at **80–100 m**, falling to **0.391 ratio** at **140–160 m**.
    - **Neighbour information goes stale** [delivery.stale-information]: age of information is **320 ms** at p95, above the **300 ms** threshold. No specific location was reported.
    - **Verification briefly falls behind** [access.verification]: during **6.0–8.0 s**, the unverified share is **0.058 ratio**; p95 queue depth is **0.682 messages**.
    - **Signing dominates message latency** [latency.dominant-stage]: mean signing time is **9 ms**, with **0.735 ratio** of total latency. No specific location was reported.
    
    **Not measured:** credential backend and privacy. The rush-hour demand was a test assumption, not a calibrated local traffic estimate.

### `vague-better`

> Make it better.

Tools: get_scenario, analyse_current_run

    The current run’s main problem is weak-signal reception, with stale neighbour information and incomplete signature verification also flagged. I’d prioritize reception, but “better” could mean longer reliable range or lower latency—which matters more to you?
    
    ### Main bottlenecks
    - **Weak signal** [channel.weak-signal]: Across the run, below-sensitivity reception accounted for **0.992 ratio** of losses.
    - **Limited reliable range** [delivery.range]: In the **80–100 m** distance bin, reception was **0.832 ratio**.
    - **Stale information** [delivery.stale-information]: Across the run, information age reached **320 ms** at p95.
    - **Verification gaps** [access.verification]: Messages left unverified were **0.058 ratio**, flagged in the **6.0–8.0 s** window.
    - **Signing latency** [latency.dominant-stage]: Signing was the largest latency stage, averaging **9 ms**.
    
    **Not measured:** credential backend and privacy. No settings have been changed or new run started.

### `impossible-wifi7`

> Simulate Wi-Fi 7 between vehicles at 6 GHz.

Tools: list_scenarios, get_scenario!, get_scenario, prepare_scenario, prepare_scenario, registry__model_card, prepare_scenario

    I couldn’t run the requested simulation: this simulator build does **not support Wi-Fi 7**. Configuration validation rejected it, so no run was started and no layers were measured.
    
    The supported radio options are:
    - DSRC / IEEE 802.11p
    - LTE-V2X PC5
    - NR-V2X PC5
    - Hybrid
    
    Would you like a vehicle-to-vehicle simulation using **NR-V2X** instead? It would be an alternative, not a Wi-Fi 7 simulation.

### `impossible-tokyo`

> Run the scenario in downtown Tokyo.

Tools: list_scenarios, get_scenario!, get_scenario, get_scenario

    I couldn’t find a shipped downtown Tokyo scenario; the available city scenarios use Manhattan or Portland, and the currently loaded world is a procedural grid. I haven’t started a run rather than substitute another city.
    
    Please provide a downtown Tokyo scenario or an OSM map file accessible to the simulator. Alternatively, I can run a **generic urban-grid approximation**, but it would not represent Tokyo’s actual streets.
    
    **Measurement status:** No run was started, so no layers were measured.

### `impossible-huge`

> Simulate one million vehicles in Manhattan for 24 hours.

Tools: list_scenarios, get_scenario, registry__model_card

    Should “one million vehicles” mean **present simultaneously** or **one million trips over the 24-hour period**?
    
    The available Manhattan preset covers a **Midtown area**, not the whole island. Is that area acceptable? I haven’t started a run yet.

### `ambiguous-compare`

> Compare them.

Tools: get_scenario!

    Which two scenarios or results would you like me to compare? Nothing has been run in this conversation yet.

### `ambiguous-attack`

> Attack the network.

Tools: list_scenarios, get_scenario, get_scenario, prepare_scenario, run_prepared

    Ran a simulated **ConstPos attack** on the small procedural-grid network, keeping its existing duration and traffic settings. The analyst reported no bottleneck findings, but it did not establish whether the attack was detected or mitigated.
    
    ### Findings
    - **Credential-backend visibility missing** [backend.not-measured]: The credential backend was not measured, so this run cannot establish a backend revocation response.
    - **No reported MAC congestion:** `mac_drops` and `mac_queue_depth` were both `0.0`; the tool supplied no units for these values.
    
    **Unmeasured layer:** backend. No baseline comparison was run, so these results do not isolate the attack’s effect.

