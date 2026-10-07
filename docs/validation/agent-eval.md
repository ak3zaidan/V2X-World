# Agent evaluation

Suite `crates/v2xw-copilot/evals/prompts.yaml`, run by `cargo run -p v2xw-copilot --example agent_eval`. Provider `openai:gpt-6.1-sol`, label `after-2`. Each case is scored on four checks: configuration (the scenario changes and run count the prompt implies), run completed, numbers grounded (every number in the answer is in the runs' data) and conclusions (the answer says what the prompt asked about, or asks / explains when it should). Times are wall-clock seconds from the prompt; "first feedback" is the first event the person sees, "first model output" the model's first plan, tool call or answer.

## Summary

| Measure | Before (`before`) | After (`after-2`) |
|---|---|---|
| cases | 42 | 42 |
| passed (all four) | 79% | 93% |
| mean score | 0.93 | 0.98 |
| configuration | 95% | 100% |
| run completed | 95% | 100% |
| numbers grounded | 90% | 95% |
| conclusions | 90% | 98% |
| first feedback, median s | 3.50 | 0.00 |
| first model output, median s | 3.5 | 4.6 |
| total, median s | 35.9 | 13.3 |
| total, p90 s | 60.3 | 22.6 |
| model calls, mean | 8.5 | 2.3 |
| tokens in, total | 1894650 | 520330 |
| tokens cached, total | 1538500 | 452831 |
| tokens out, total | 37638 | 14600 |
| cost, USD | 4.166 | 1.187 |

## By category

| Category | Cases | Passed | Mean score |
|---|---|---|---|
| ambiguous | 3 | 3 | 1.00 |
| attack | 7 | 6 | 0.96 |
| congestion | 1 | 1 | 1.00 |
| emergency | 1 | 1 | 1.00 |
| events | 2 | 2 | 1.00 |
| follow-up | 3 | 3 | 1.00 |
| impossible | 4 | 4 | 1.00 |
| jamming | 2 | 2 | 1.00 |
| multi-variant | 3 | 3 | 1.00 |
| normal | 3 | 2 | 0.92 |
| overhead | 3 | 3 | 1.00 |
| pedestrians | 1 | 1 | 1.00 |
| privacy | 2 | 1 | 0.88 |
| radio | 2 | 2 | 1.00 |
| revocation | 1 | 1 | 1.00 |
| vague | 2 | 2 | 1.00 |
| weather | 2 | 2 | 1.00 |

## Cases

| Case | Config | Done | Grounded | Concl. | Runs | Calls | First model s | Total s | Tokens in/out | Cost $ | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `normal-baseline` (before 1.00) | yes | yes | yes | yes | 1 | 2 | 5.7 | 12.3 | 9783/277 | 0.0223 |  |
| `normal-channel` (before 1.00) | yes | yes | **no** | yes | 1 | 2 | 5.2 | 10.6 | 10311/251 | 0.0231 | ungrounded 0.00012 |
| `normal-lte` (before 1.00) | yes | yes | yes | yes | 1 | 3 | 5.0 | 15.1 | 16168/374 | 0.0361 |  |
| `congestion-rush` (before 1.00) | yes | yes | yes | yes | 1 | 2 | 4.4 | 20.6 | 10420/419 | 0.0250 |  |
| `congestion-light-vs-rush` (before 1.00) | yes | yes | yes | yes | 2 | 2 | 4.9 | 15.9 | 9945/527 | 0.0252 |  |
| `attack-constpos` (before 1.00) | yes | yes | yes | yes | 1 | 2 | 6.2 | 15.9 | 10453/325 | 0.0242 |  |
| `attack-randompos-vs-baseline` (before 1.00) | yes | yes | yes | yes | 2 | 2 | 4.6 | 12.9 | 10016/349 | 0.0235 |  |
| `attack-speed` (before 1.00) | yes | yes | yes | yes | 2 | 2 | 5.0 | 13.3 | 10015/331 | 0.0233 |  |
| `attack-replay` (before 1.00) | yes | yes | yes | yes | 1 | 2 | 4.7 | 11.0 | 10345/328 | 0.0240 |  |
| `attack-signature` (before 1.00) | yes | yes | yes | yes | 2 | 3 | 5.2 | 16.0 | 15822/451 | 0.0362 |  |
| `attack-expired-cert` (before 1.00) | yes | yes | yes | yes | 1 | 3 | 4.9 | 13.7 | 16602/339 | 0.0366 |  |
| `attack-wave` (before 1.00) | yes | yes | yes | **no** | 1 | 2 | 5.5 | 12.1 | 10472/346 | 0.0244 | mentions none of ["20"] |
| `attack-sybil` (before 0.75) | yes | yes | yes | yes | 0 | 1 | 3.4 | 3.4 | 4379/57 | 0.0093 |  |
| `jam-constant` (before 0.75) | yes | yes | yes | yes | 1 | 2 | 4.1 | 9.9 | 10369/241 | 0.0231 |  |
| `jam-dsrc-vs-lte` (before 0.50) | yes | yes | yes | yes | 2 | 3 | 5.1 | 27.2 | 17902/881 | 0.0446 |  |
| `revocation` (before 1.00) | yes | yes | yes | yes | 1 | 4 | 3.2 | 30.7 | 23794/378 | 0.0514 |  |
| `privacy-period` (before 0.75) | yes | yes | **no** | yes | 2 | 4 | 4.5 | 48.9 | 23781/464 | 0.0522 | ungrounded 0.204, 0.26 |
| `privacy-mixzone` (before 1.00) | yes | yes | yes | yes | 2 | 4 | 3.5 | 46.3 | 21909/516 | 0.0490 |  |
| `rat-three-way` (before 0.75) | yes | yes | yes | yes | 3 | 2 | 5.4 | 16.1 | 11584/496 | 0.0281 |  |
| `radio-region-eu` (before 1.00) | yes | yes | yes | yes | 2 | 3 | 4.3 | 14.7 | 15084/463 | 0.0348 |  |
| `overhead-cert-cadence` (before 1.00) | yes | yes | yes | yes | 2 | 2 | 5.0 | 11.5 | 10167/334 | 0.0237 |  |
| `overhead-pq-mldsa` (before 0.50) | yes | yes | yes | yes | 2 | 4 | 5.5 | 22.6 | 21223/747 | 0.0499 |  |
| `overhead-falcon` (before 1.00) | yes | yes | yes | yes | 2 | 3 | 5.1 | 18.7 | 15769/644 | 0.0380 |  |
| `emergency-priority` (before 1.00) | yes | yes | yes | yes | 1 | 3 | 4.2 | 19.0 | 16725/385 | 0.0373 |  |
| `pedestrians` (before 1.00) | yes | yes | yes | yes | 1 | 2 | 3.7 | 14.4 | 10440/318 | 0.0241 |  |
| `weather-rain` (before 1.00) | yes | yes | yes | yes | 2 | 2 | 5.3 | 13.9 | 10093/376 | 0.0239 |  |
| `weather-fog-front` (before 1.00) | yes | yes | yes | yes | 1 | 2 | 4.6 | 10.2 | 10297/304 | 0.0236 |  |
| `event-closure` (before 1.00) | yes | yes | yes | yes | 1 | 5 | 4.9 | 19.0 | 29275/435 | 0.0629 |  |
| `event-hard-brake` (before 1.00) | yes | yes | yes | yes | 1 | 3 | 4.9 | 12.1 | 16176/291 | 0.0353 |  |
| `multi-four-variants` (before 0.75) | yes | yes | yes | yes | 4 | 2 | 5.7 | 16.9 | 11821/528 | 0.0289 |  |
| `multi-density-sweep` (before 1.00) | yes | yes | yes | yes | 3 | 2 | 4.4 | 14.6 | 10517/498 | 0.0260 |  |
| `followup-base` (before 1.00) | yes | yes | yes | yes | 1 | 2 | 4.5 | 10.7 | 10281/294 | 0.0235 |  |
| `followup-double` (before 1.00) | yes | yes | yes | yes | 1 | 2 | 3.5 | 10.8 | 13740/369 | 0.0312 |  |
| `followup-lte` (before 1.00) | yes | yes | yes | yes | 1 | 2 | 2.9 | 11.6 | 18189/459 | 0.0410 |  |
| `vague-healthy` (before 0.75) | yes | yes | yes | yes | 1 | 2 | 3.2 | 8.8 | 10594/286 | 0.0240 |  |
| `vague-bottleneck` (before 1.00) | yes | yes | yes | yes | 1 | 2 | 4.1 | 9.4 | 9642/238 | 0.0217 |  |
| `vague-better` (before 0.50) | yes | yes | yes | yes | 0 | 1 | 2.7 | 2.7 | 4368/29 | 0.0090 |  |
| `impossible-wifi7` (before 1.00) | yes | yes | yes | yes | 0 | 1 | 3.9 | 3.9 | 4377/64 | 0.0094 |  |
| `impossible-tokyo` (before 1.00) | yes | yes | yes | yes | 0 | 1 | 3.4 | 3.4 | 4371/35 | 0.0091 |  |
| `impossible-huge` (before 1.00) | yes | yes | yes | yes | 0 | 1 | 3.9 | 3.9 | 4376/51 | 0.0093 |  |
| `ambiguous-compare` (before 1.00) | yes | yes | yes | yes | 0 | 1 | 2.7 | 2.7 | 4367/29 | 0.0090 |  |
| `ambiguous-attack` (before 1.00) | yes | yes | yes | yes | 0 | 1 | 4.4 | 4.4 | 4368/73 | 0.0095 |  |

## Answers

### `normal-baseline`

> Run the procedural grid for 30 seconds at 1500 vehicles per hour and tell me how the V2X network performs.

Tools: simulate

    The procedural-grid run completed, but **V2X delivery failed despite no reported channel congestion**. The dominant problem was weak received signals, rather than collisions or MAC queueing.
    
    - **Weak-signal losses [channel.weak-signal]:** delivery ratio was **0**, with **216 frames** lost below receiver sensitivity. Below-sensitivity accounted for **1 ratio** of losses.
    - **No congestion indicated:** channel busy ratio was **0**, collision rate was **0**, MAC drops were **0**, and MAC queue depth was **0**. These results do not indicate a congested network—but they do not offset the failed delivery.
    - **Security overhead [security.overhead]:** security overhead was **0.518 ratio**, certificate bytes share was **0.138 ratio**, and air bytes per payload byte was **3.647 B/B**.
    
    **Not measured:** backend and privacy performance.

### `normal-channel`

> How busy is the radio channel on the Midtown grid with moderate traffic, about 2000 vehicles per hour? Keep the run to 30 s.

Tools: simulate

    The radio channel was **very lightly loaded**, not congested, during the 30 s Midtown-grid run at the requested traffic demand. Reported channel busy ratio was **0.00012**, with channel load **0.00321** and no recorded collisions or MAC drops.
    
    - **Weak signals, rather than congestion, limited reception** [channel.weak-signal]: below-sensitivity accounted for **0.998 ratio** of losses.
    - **Verification had a small backlog** [access.verification]: **0.051 ratio** of messages remained unverified; verification queue depth was **1 messages** at p95, with **0 ms** p95 wait.
    
    Backend and privacy effects were not measured.

### `normal-lte`

> Simulate the grid with LTE-V2X sidelink instead of DSRC for 30 s at 1500 veh/h and summarise delivery and latency.

Tools: simulate, query_metric, query_metric

    The grid run completed with LTE-V2X sidelink at the requested demand and duration. **Medium-access delay was the critical bottleneck**, while fading dominated delivery losses.
    
    - **Latency:** Access delay reached **77.2 ms at p95**, exceeding the configured **20 ms** allowance [access.delay]. The largest latency stage was `mac_defer`, averaging **52.6 ms**, with a **0.773 ratio** share of total latency [latency.dominant-stage].
    - **Delivery:** Reception fell below **0.9 ratio from 500 m**; PRR at **500–520 m was 0.154 ratio** [delivery.range]. Fading accounted for **1 ratio** of recorded losses [channel.weak-signal].
    
    **Limitations:** Overall delivery ratio and end-to-end latency queries returned insufficient data. No DSRC baseline was run, so this does not establish a comparative improvement.

### `congestion-rush`

> Rush hour on the grid - 20,000 vehicles per hour for 40 seconds. Find the bottlenecks.

Tools: simulate

    The rush-hour grid run points to **radio reach and security processing**, rather than channel contention, as the main communication bottlenecks. Weak signals dominate losses, while signing dominates latency and some received messages remain unverified.
    
    - **Weak-signal coverage — [channel.weak-signal], [delivery.range]:** Below-sensitivity reception accounts for **0.993 ratio** of losses. Reception drops from **0.971 ratio** at **60–80 m** to **0.861 ratio** at **80–100 m**, then **0.533 ratio** at **120–140 m**.
    - **Verification shortfall — [access.verification]:** Messages left unverified: **0.07 ratio**. The flagged interval is **5.333–6.667 s**; verification queue depth reaches **0.658 messages** at p95, although p95 verification wait is **0 ms**.
    - **Signing latency — [latency.dominant-stage]:** Signing takes **9 ms** on average and represents **0.736 ratio** of total latency. Handoff is next at **2.494 ms**; mean MAC access stages are much smaller.
    - **Security overhead — [security.overhead]:** Air bytes per payload byte are **3.603 B/B**, with security overhead of **0.516 ratio**—a substantial messaging cost, but not evidence of channel saturation in this run.
    
    **Scope:** These findings identify communication bottlenecks, not road-level traffic choke points. Backend and privacy performance were not measured.

### `congestion-light-vs-rush`

> Compare light traffic (500 veh/h) against rush hour (15,000 veh/h) on the grid, 30 s each. What changes?

Tools: simulate

    Rush hour slowed vehicles and substantially increased radio traffic, but the main radio problem was weak signal—not congestion. Both arms ran for 30 s with the same seed; these are single-realisation results without confidence intervals.
    
    ### What changed
    Values below are **light traffic → rush hour**.
    
    | Metric | Light: 500 veh/h | Rush: 15,000 veh/h | Reported change |
    |---|---:|---:|---:|
    | Mean speed | 7.128 m/s | 4.602 m/s | −2.526 m/s (−35.4%) |
    | Median speed | 9.273 m/s | 4.012 m/s | −5.261 m/s (−56.7%) |
    | Offered radio load | 31732 bit/s | 374537 bit/s | +342804 bit/s (+1080%) |
    | Air bytes | 3967 B/s | 46791 B/s | +42824 B/s (+1080%) |
    | Below-sensitivity losses | 22 frames | 72736 frames | +72714 frames (+330518%) |
    
    ### Main bottlenecks
    - **Weak signals dominated rush-hour losses:** below-sensitivity accounted for 0.994 ratio of losses, versus collision at 0.00044 ratio. The increased loss count should not be read as a comparable increase in loss *rate*. [channel.weak-signal]
    - **Reception degraded with distance:** rush-hour PRR was 1 ratio through the 60–80 m bin, then 0.814 ratio at 80–100 m and 0.362 ratio at 140–160 m. [delivery.range]
    - **Signing was the largest latency stage** in rush hour; the returned analysis did not give its duration. [latency.dominant-stage]
    
    The experiment did not flag channel-load or MAC-queue metrics as significant changes. Backend and privacy effects were not measured.

### `attack-constpos`

> Add a ConstPos attacker population (5% of vehicles) to the grid at 3000 veh/h for 60 s with misbehaviour detection on. Does detection catch them?

Tools: simulate

    **This run does not demonstrate that detection catches them.** The requested scenario ran to 60 s, but the analyser reports that attackers ran and **no detector scored them**, despite detection being configured on [security.detection-off].
    
    - **Detection outcome:** detection recall, false-positive rate, and time to detection were not reported; this is not evidence that the attackers evaded a functioning detector [security.detection-off].
    - **Reception limitation:** below-sensitivity losses accounted for **0.994 ratio** of losses, limiting the messages receivers could observe [channel.weak-signal].
    - **Range:** reception was **0.899 ratio** at **80–100 m**, falling to **0.333 ratio** at **140–160 m** [delivery.range].
    
    The unanswered question is why the enabled detector produced no scores; successful detection cannot be claimed from this run.

### `attack-randompos-vs-baseline`

> Run a baseline and the same scenario with 10% RandomPos attackers, same seed, 40 s at 3000 veh/h on the grid, and report the differences.

Tools: simulate

    Completed the same-seed comparison for **40 s at 3000 veh/h** on the grid. The **10% RandomPos** variant produced **no significant metric changes**; all reported metrics were unchanged.
    
    - **Attack detection was not evaluated:** attackers ran, but no detector scored them. This was the only new finding in the attacker variant. [security.detection-off]
    - **Weak signal remained the main bottleneck:** below-sensitivity accounted for **0.998 ratio** of losses in both arms. [channel.weak-signal]
    - **Reception range was unchanged:** reception fell below **0.9 from 140 m** in both arms. [delivery.range]
    - **Latency bottleneck was unchanged:** signing remained the largest latency stage. [latency.dominant-stage]
    
    **Limits:** this is one seed per arm, with no confidence interval—not evidence that RandomPos attacks are harmless. Backend metrics were not measured in either run; privacy was not measured in the baseline.

### `attack-speed`

> What does a ConstSpeedOffset attacker (10% of vehicles) do to the grid network at 3000 veh/h over 40 s?

Tools: simulate

    The ConstSpeedOffset attack produced **no significant change in the reported network or traffic metrics** compared with the attacker-free grid. However, attackers ran without a detector scoring them, so this result does **not** establish that the attack was detected or harmless.
    
    - **Attack detection was not evaluated** [security.detection-off]: the attack arm had attackers, but no detector scored them.
    - **Weak radio signals remained the main bottleneck** [channel.weak-signal]: below-sensitivity losses accounted for **0.998 ratio** of losses in both arms.
    - **Reception range was unchanged** [delivery.range]: reception fell below **0.9 from 140 m** in both arms.
    
    The comparison reported no significant differences, but used only one seed per arm, with no confidence interval. Backend effects were not measured; detection effectiveness was not assessed.

### `attack-replay`

> Simulate a data replay attack by 5% of vehicles on the grid for 40 s at 3000 veh/h and say whether the detectors notice.

Tools: simulate

    The grid run completed for **40 s** at **3000 veh/h**, with **5% DataReplay attackers** and the requested misbehaviour detectors enabled. **Detection was not demonstrated:** the simulator reports that attackers ran, but no detector scored them.
    
    - **Detection unavailable** [security.detection-off]: No detector scoring was reported, so this run cannot establish whether the detectors noticed the replay attack—or missed it.
    - **Weak reception** [channel.weak-signal]: Below-sensitivity reception accounted for **0.998 ratio** of losses, limiting message visibility.
    - **Reception range** [delivery.range]: PRR was **1 ratio** at **100–120 m**, falling to **0.647 ratio** at **140–160 m**.
    
    Detection accuracy and time to detect were not reported; backend metrics were not measured.

### `attack-signature`

> What happens to signature verification when 10% of vehicles on the grid send messages with invalid signatures? 40 s at 3000 veh/h.

Tools: simulate, live_metrics

    **Invalid-signature traffic produced no measured change in verification load** in this grid experiment. The attack and attack-free arms had identical verification metrics, with no significant changes reported.
    
    | Verification metric | Attack-free | Invalid-signature attackers |
    |---|---:|---:|
    | Verification cost | 0.4 ms | 0.4 ms |
    | Mean verification queue depth | 0.08 count | 0.08 count |
    | Verification wait | 0 ms | 0 ms |
    | Unverified-message ratio | 0.017 ratio | 0.017 ratio |
    
    - **Attack detection was not scored:** attackers ran, but no misbehaviour detector was enabled. This does not establish how many invalid signatures were rejected. [security.detection-off]
    - **Radio reception was the main bottleneck:** below-sensitivity reception accounted for 0.998 ratio of losses in both arms, limiting traffic reaching receivers. [channel.weak-signal]
    - **Signing, rather than verification, was the largest latency stage** in both arms. [latency.dominant-stage]
    
    The returned results do not report an invalid-signature rejection count. This was a single-seed comparison without confidence intervals, so the unchanged metrics should not be generalized to heavier loads.

### `attack-expired-cert`

> Give 10% of the grid's vehicles expired certificates for 40 s at 3000 veh/h. How many of their messages are accepted?

Tools: simulate, inspect

    The run completed, but **the simulator did not report how many expired-certificate messages were accepted**, so I cannot give that count.
    
    - **Verification:** `unverified_ratio` was **0.017 ratio**—messages delivered to applications without verification divided by all delivered messages. This is not an acceptance count for expired certificates.
    - **Detection:** Attackers ran, but no detector scored them [security.detection-off].
    - **Radio losses:** Most losses were below receiver sensitivity [channel.weak-signal], so reception and certificate acceptance must not be conflated.
    
    The missing measurement is an accepted/rejected message count specifically for the expired-certificate vehicles.

### `attack-wave`

> Schedule an attack wave on the grid - 10% StopAndGo attackers active only from 20 s to 40 s of a 60 s run at 3000 veh/h.

Tools: simulate

    Completed the grid run with the requested demand and scheduled StopAndGo attacker population. Attackers ran, but detection was disabled, so this run does not assess detection effectiveness.
    
    - **Weak signals dominated losses** [channel.weak-signal]: below-sensitivity accounted for **0.994 ratio** of losses.
    - **Reception declined with distance** [delivery.range]: PRR was **0.899 ratio** at **80–100 m** and **0.333 ratio** at **140–160 m**.
    - **Signing dominated latency** [latency.dominant-stage]: mean signing time was **9 ms**, accounting for **0.731 ratio** of total latency.
    - **No detector scoring** [security.detection-off]: attack detection effectiveness was not measured. Backend metrics were also not measured.
    
    No attack-free comparison was run, so these results do not isolate the attack wave’s impact.

### `attack-sybil`

> Simulate a Sybil attack where one car fakes twenty ghost vehicles.

Tools: none

    This simulator explicitly refuses Sybil attacks, so I can’t simulate one car impersonating twenty ghost vehicles. It supports attacks such as false-position reports and message replay, but neither is equivalent to Sybil.
    
    Would you like to simulate one of those supported attacks instead?

### `jam-constant`

> Put a constant jammer in the middle of the grid (3000 veh/h, 30 s) and tell me how many frames it kills.

Tools: simulate

    The central constant jammer caused **305 frames to be lost** during the 30 s run at 3000 veh/h [channel.jamming]. This is the simulator’s count of lost **receptions**, not necessarily unique transmitted frames.
    
    - **Weak signals dominated losses**, rather than jamming: 1920 frames were lost below receiver sensitivity [channel.weak-signal].
    - No jammer-free comparison was run, so the net additional loss versus an unjammed grid was not measured.

### `jam-dsrc-vs-lte`

> Is LTE-V2X more robust to a reactive jammer than DSRC? Compare both on the grid with the same jammer, 30 s at 3000 veh/h.

Tools: simulate

    **LTE-V2X delivered more packets than DSRC, but was not uniformly more robust:** its medium-access delay triggered a critical finding. It improved delivery while producing slower, staler information.
    
    | Metric | DSRC | LTE-V2X | Reported change |
    |---|---:|---:|---:|
    | Delivery ratio | 0.161 ratio | 0.374 ratio | +0.213 ratio |
    | Goodput | 1630 B/s | 2302 B/s | +671 B/s |
    | End-to-end latency, p95 | 14.4 ms | 91.8 ms | +77.3 ms |
    | Age of information | 72.2 ms | 420 ms | +348 ms |
    
    - **Critical access bottleneck:** LTE-V2X’s p95 medium-access delay was **94.4 ms**, exceeding the finding’s **20 ms** allocation. [access.delay]
    - **Stale information:** LTE-V2X’s information age was **420 ms**, above the **300 ms** threshold. [delivery.stale-information]
    - **Dominant losses differed:** DSRC’s were below sensitivity; LTE-V2X’s were fading. Neither finding isolates jammer-caused losses. [channel.weak-signal]
    
    Both runs ended at **30 s**. The comparison uses one seed per arm, with no confidence interval or jammer-free control, so it establishes performance with the jammer—not degradation attributable specifically to jamming. Backend and privacy were not measured.

### `revocation`

> How long does it take to revoke a misbehaving vehicle's certificates in the credential-lifecycle scenario? Run it and break the time down by stage.

Tools: simulate, query_metric, query_metric, query_metric, registry__metric

    The **300 s** credential-lifecycle run recorded a revocation sequence through issuance, but did **not** provide a measured end-to-end time from misbehaviour onset to certificate enforcement. Attackers ran without detector scoring [security.detection-off], and both detection time and decision time returned insufficient data.
    
    ### Recorded revocation stages
    
    | Transition | Time |
    |---|---:|
    | Detection → report sent | 0.009 s |
    | Report sent → report received | 0.287 s |
    | Report received → decision | 0.002 s |
    | Decision → resolved | 0.022 s |
    | Resolved → issued | 0.055 s |
    
    Each transition had one recorded sample. **Report transport was the slowest recorded stage**, at **0.287 s**.
    
    **Limit:** These measurements stop at issuance; no issuance-to-CRL-installation or per-vehicle enforcement timing was returned. They therefore do not establish when other vehicles actually start rejecting the misbehaving vehicle’s certificates.

### `privacy-period`

> How linkable are vehicles to a roadside eavesdropper with 300 s pseudonym changes versus 60 s? Use the pseudonym-strategies scenario.

Tools: simulate, find_settings, query_metric

    With **60 s pseudonym changes**, the roadside observer’s reported `linkability` values ranged from **0.204 to 0.26** at the sampled times with sufficient data. The comparison did not return linkability for the **300 s** arm, so I cannot quantify the privacy improvement between the two periods.
    
    | Result | 300 s changes | 60 s changes |
    |---|---:|---:|
    | Observed pseudonym-change rate | 0 1/(veh·h) | 66.2 1/(veh·h) |
    | Goodput | 68326 B/s | 61469 B/s |
    
    - **Important boundary:** both runs ended at **300 s**; the 300 s arm recorded no pseudonym changes. This therefore does not establish how successfully the observer links vehicles *across* a 300 s change.
    - **Communication cost:** goodput fell by **6857 B/s (10%)** with the shorter period. Security overhead was identified in both runs [security.overhead].
    - **Limitations:** the linkability metric’s definition was not supplied, so those values should not be interpreted as percentages of vehicles tracked. This was one seed per arm, with no confidence interval.

### `privacy-mixzone`

> Compare the mix-zone pseudonym strategy with the J2945/1 time-based strategy against the eavesdropper in the pseudonym-strategies scenario.

Tools: get_scenario, simulate, live_metrics

    Mix-zone produced pseudonym changes while the J2945/1 time-based strategy produced none during this **300 s** run. However, the results do **not establish better protection against the eavesdropper**: tracking success, linkability and track duration were unavailable.
    
    | Significant metric | J2945/1 time-based | Mix-zone | Reported change |
    |---|---:|---:|---:|
    | Pseudonym change rate | 0 1/(veh·h) | 27 1/(veh·h) | +27 1/(veh·h) |
    | Unverified ratio | 0.03 ratio | 0.04 ratio | +0.01 ratio (+35.1%) |
    
    The time-based configuration used a **300 s** pseudonym period. Both arms retained the passive eavesdropper with **0.25** sniffer fraction and **100 m** range.
    
    Other findings, baseline → mix-zone:
    - **Weak signals dominated losses:** below-sensitivity loss share was **0.986 ratio → 0.987 ratio**. [channel.weak-signal]
    - **Neighbour information remained stale:** age of information (p95) was **678 ms → 676 ms**, above the **300 ms** threshold. [delivery.stale-information]
    
    **Bottom line:** mix-zone increased rotation activity, with a significant increase in unverified messages, but its privacy benefit remains unmeasured. This was a same-seed comparison with no confidence interval.

### `rat-three-way`

> DSRC vs LTE-V2X vs NR-V2X on the grid at 3000 veh/h for 30 s - who delivers further?

Tools: simulate

    **DSRC and NR-V2X tie for farther reliable delivery in this run; LTE-V2X delivers more overall, but not farther reliably.** LTE-V2X also triggered a critical medium-access delay finding.
    
    - **Reliable-distance threshold:** reception first fell below the **0.9 ratio** threshold at **400 m** for DSRC and NR-V2X, versus **80 m** for LTE-V2X. All curves were non-monotone, so these are first threshold crossings—not maximum reception ranges. [delivery.range] [delivery.non-monotone]
    - **Overall delivery:** DSRC **0.161 ratio**, LTE-V2X **0.374 ratio**, NR-V2X **0.223 ratio**. Relative to DSRC, the reported changes were **+0.213 ratio (+132%)** for LTE and **+0.062 ratio (+38.3%)** for NR.
    - **Delay trade-off:** end-to-end latency p95 was **14.4 ms / 91.8 ms / 41.3 ms** for DSRC / LTE / NR, respectively. LTE’s medium-access delay p95 reached **94.4 ms**, and its information age reached **420 ms**, triggering stale-information concerns. [access.delay] [delivery.stale-information]
    - **Main loss bottleneck:** below-sensitivity losses dominated DSRC; fading dominated LTE and NR. [channel.weak-signal]
    
    This is one seed per radio, without confidence intervals. Backend and privacy were not measured.

### `radio-region-eu`

> Run the grid under European regulation instead of the US band plan, 30 s at 3000 veh/h, and compare the channel load with the US setup.

Tools: simulate, simulate

    The European setup produced **no significant change in channel load** over the 30 s comparison. The US baseline used **`us-2016`**, because the simulator rejects current-US regulation for the grid’s DSRC radio.
    
    | Channel metric | US deployment-era | Europe |
    |---|---:|---:|
    | Channel load | 0.00351 | 0.00371 |
    | Channel occupancy | 0.00313 | 0.00313 |
    
    Both arms used the requested 3000 veh/h demand and the same seed.
    
    - **Weak signals, not congestion, dominated losses** in both arms [channel.weak-signal].
    - **Latency increased in Europe:** end-to-end latency was 12.3 ms versus 14.9 ms, a reported change of +2.562 ms (+20.8%). Signing remained the largest latency stage [latency.dominant-stage].
    
    This is a single-seed comparison with no confidence interval. Backend and privacy outcomes were not measured.

### `overhead-cert-cadence`

> What share of the air bytes is security overhead on the grid (3000 veh/h, 30 s) when the full certificate is attached every 1000 ms versus every 450 ms?

Tools: simulate

    Security overhead’s share of air bytes was **0.519 ratio** with certificates every **1000 ms**, versus **0.549 ratio** every **450 ms**. The shorter interval increased the overhead share, though the simulator did not flag that metric’s change as significant.
    
    [security.overhead]
    
    | Metric | 1000 ms | 450 ms |
    |---|---:|---:|
    | Security overhead / air bytes | 0.519 ratio | 0.549 ratio |
    | Certificate bytes share | 0.14 ratio | 0.242 ratio |
    
    The certificate-byte share increase **was significant**: **+0.103 ratio (+73.5%)**, as reported by the simulator.
    
    This was a same-seed grid comparison at the requested demand and duration, with no confidence interval. Backend and privacy effects were not measured.

### `overhead-pq-mldsa`

> Classical ECDSA versus post-quantum hybrid ML-DSA-44 signatures on the grid at 3000 veh/h for 30 s - what does PQ cost in channel load and verification?

Tools: simulate, simulate, finding_detail, finding_detail

    **The hybrid run sent no frames, so this experiment cannot quantify PQ’s channel or verification cost.** Both arms used the same grid, seed and Cohda OBU; the default grid OBU lacked published costs for both hybrid components.
    
    - **Hybrid failure — [simulation.silent]:** Air traffic fell from **10153 B/s → 0 B/s** (reported change **−100%**), and offered load from **81222 bit/s → 0 bit/s** (**−100%**). These are empty results, **not PQ efficiency gains**. The finding lists possible transmission blockers but does not identify which occurred.
    - **Channel baseline:** ECDSA’s channel load was **0.0035**, versus **0** for the silent hybrid arm; CBR was **0.00043**, versus **0**. The hybrid values cannot support a meaningful load comparison.
    - **ECDSA overhead — [security.overhead]:** Air bytes per payload byte were **3.664641 B/B**, with security overhead **0.519172 ratio**.
    - **Verification:** No usable hybrid verification result was reported. ECDSA’s largest latency stage was signing [latency.dominant-stage], but the returned findings did not quantify verification time.
    
    **Bottom line:** this configuration needs its hybrid transmission blocker diagnosed before PQ overhead can be measured; the apparent load reduction is a failed transmission path.

### `overhead-falcon`

> Would hybrid Falcon-512 signatures fit on DSRC at 5000 veh/h? Check it on the grid for 30 s.

Tools: simulate, simulate

    **It fits the DSRC airtime budget in this run, but not the verification workload.** At 5000 veh/h over 30 s on the grid, hybrid Falcon-512 left all messages unverified; both arms used the Cohda MK6C OBU because the grid’s default hardware did not support the hybrid.
    
    - **Verification is the bottleneck** [access.verification]: unverified ratio rose from **0.042 to 1 ratio**, a reported **+2255%**, moving from caution to unhealthy.
    - **Much larger security overhead** [security.overhead]: air bytes per payload byte rose from **3.666 to 15.2 B/B** (**+315%**). Airtime per node increased from **3.114 to 10.3 ms/s** (**+230%**), but channel busy ratio remained **0.00267** for the hybrid, versus **0.00071** for ECDSA.
    - **Latency increased, without becoming unhealthy** [latency.dominant-stage]: p95 end-to-end latency rose from **13.8 to 17.3 ms** (**+25.3%**); signing was the largest latency stage.
    - **Radio losses were mainly weak-signal losses, not congestion** [channel.weak-signal]: below-sensitivity losses accounted for **0.99 ratio** of hybrid losses.
    
    This is one seed with no confidence interval, not a capacity guarantee. Backend and privacy were not measured.

### `emergency-priority`

> Run the connected-intersections scenario for 60 s and tell me whether emergency vehicle priority requests get through to the signals.

Tools: simulate, inspect!

    The connected-intersections run completed for **60 s**, but **I cannot confirm that emergency priority requests reached the signals**. The reported metrics do not include request receipt or priority grants, and the signal-entity inspection was unavailable.
    
    Relevant radio findings:
    - **Weak signals dominated losses**: below-sensitivity accounted for **0.978 ratio** of losses. [channel.weak-signal]
    - **Reception weakened with distance**: PRR was **0.921 ratio** at **80–100 m**, falling to **0.832 ratio** at **100–120 m**. [delivery.range]
    - **Information became stale**: age of information (p95) was **432 ms**. [delivery.stale-information]
    
    These are network-wide findings, not emergency-request-specific evidence. **SRM receipt, signal acknowledgement, and actual priority activation were not reported**, so the result is inconclusive rather than a confirmed success or failure.

### `pedestrians`

> Simulate 200 pedestrians on the VRU grid scenario for 60 s and tell me about conflicts between pedestrians and vehicles.

Tools: simulate

    **The run flagged pedestrian–vehicle conflicts, but reported no pedestrian near misses.** A critical radio finding also limits confidence in timely V2X awareness: nearby vehicles lost messages [delivery.near-range-loss].
    
    - **Pedestrian conflicts:** `0.034 count`; pedestrian near misses: `0 count` [traffic.pedestrians]. The fractional conflict value is the simulator’s reported metric—not a confirmed count of discrete incidents.
    - **Crossing behaviour:** pedestrian midblock share was `0.024 1` [traffic.pedestrians].
    - **Broader traffic conflicts:** `38.5 count` for conflicts below the TTC threshold, located at `t 4-6 s` [traffic.conflicts]. This is a general traffic metric, not specifically pedestrian–vehicle conflicts.
    - **Awareness limitations:** reception at `20-40 m` was `0.902 ratio`, and information age (p95) reached `895 ms` [delivery.near-range-loss] [delivery.stale-information].
    
    The results do not provide a pedestrian collision or injury count, or individual pedestrian–vehicle conflict locations.

### `weather-rain`

> Heavy rain (intensity 0.8) on the grid at 3000 veh/h for 30 s - how does it change delivery and traffic speed compared with clear weather?

Tools: simulate

    Heavy rain improved delivery but slowed traffic in this grid run. This is a same-seed comparison with no confidence interval, so the delivery improvement should not be treated as a general rain benefit.
    
    | Metric | Clear | Heavy rain | Change |
    |---|---:|---:|---:|
    | Delivery ratio | 0.161 | 0.191 | +0.03 (+18.6%) |
    | Mean traffic speed | 6.747 m/s | 5.925 m/s | −0.823 m/s (−12.2%) |
    | Median traffic speed | 7.778 m/s | 6.932 m/s | −0.846 m/s (−10.9%) |
    
    - **Delivery remained limited by weak signals**, not relieved enough to become reliable: below-sensitivity losses dominated both runs [channel.weak-signal].
    - **Range remained a limitation:** reception fell below 0.9 from 400 m in both weather conditions [delivery.range].
    
    The analysis does not establish why rain improved aggregate delivery in this realization. Backend and privacy effects were not measured.

### `weather-fog-front`

> Start the grid in clear weather and have a fog front arrive at 20 s in a 40 s run at 3000 veh/h.

Tools: simulate

    Completed the **40 s** grid run at **3000 veh/h**, starting clear with a fog front scheduled at **20 s**. The results identify weak signal as the main delivery bottleneck, but do not isolate the fog front’s effect.
    
    - **Weak signal:** below-sensitivity losses dominated, with a reported loss share of **0.998 ratio**. [channel.weak-signal]
    - **Reception range:** PRR was **1 ratio** at **100–120 m**, falling to **0.711 ratio** at **140–160 m**. [delivery.range]
    - **Latency:** signing was the largest stage, averaging **9 ms** and accounting for **0.732 ratio** of total latency. [latency.dominant-stage]
    
    Backend and privacy were not measured.

### `event-closure`

> Close a street on the grid at 10 s and reopen it at 30 s in a 40 s run at 4000 veh/h, and tell me the traffic impact.

Tools: simulate, query_metric, live_metrics

    The grid run finished at **40 s**. Mean traffic speed was **6.448 m/s**, median speed was **7.509 m/s**, and measured time-to-collision conflicts were **0 count** (healthy).
    
    The closure’s specific traffic impact could not be quantified: time-binned speed data were insufficient, and no no-closure baseline was run.

### `event-hard-brake`

> Script an emergency hard brake at 15 s on the grid (3000 veh/h, 30 s) and check whether forward collision warnings fire.

Tools: simulate, query_metric

    Ran the grid for **30 s** at **3000 veh/h**, with an automatic-target emergency hard brake scripted at **15 s**. **No forward collision warnings were recorded**: the application-warning query returned no rows.
    
    - **Weak reception** [channel.weak-signal]: below-sensitivity losses dominated, with a reported loss share of **0.998 ratio**.
    - **Limited reception range** [delivery.range]: PRR was **1 ratio** at **100–120 m**, but **0 ratio** at **400–420 m**.
    
    The warning result does not establish why FCW stayed silent; the returned analysis did not identify the braking vehicle or its following-vehicle geometry.

### `multi-four-variants`

> Run four variants on the grid at 3000 veh/h for 30 s, same seed: baseline, 5% ConstPos attackers, NR-V2X instead of DSRC, and heavy rain. Rank what hurts delivery most.

Tools: simulate

    **None of the changes reduced delivery in this run.** Ranked from lowest to highest delivery: baseline tied with ConstPos attackers, then heavy rain, then NR-V2X.
    
    | Variant | Delivery ratio | Change vs baseline |
    |---|---:|---:|
    | Baseline DSRC | 0.161 ratio | — |
    | 5% ConstPos attackers | 0.161 ratio | 0% |
    | Heavy rain | 0.192 ratio | +19.5% |
    | NR-V2X | 0.223 ratio | +38.3% |
    
    - **Weak signals were the main delivery bottleneck**, with below-sensitivity losses dominating DSRC and fading dominating NR-V2X. Reception fell below 0.9 from 400 m in every arm. [channel.weak-signal] [delivery.range]
    - **NR-V2X improved delivery but worsened latency:** end-to-end p95 rose from 14.4 ms to 41.3 ms (+186%); medium-access p95 was 35.8 ms. [access.delay]
    - **ConstPos had no measured delivery effect**, but no detector scored the attackers—unchanged delivery does not establish that their false positions were harmless. [security.detection-off]
    
    These are same-seed, 30 s results at 3000 veh/h, with no confidence interval. Backend effects were not measured; privacy was not measured in the baseline, NR-V2X, or rain arms.

### `multi-density-sweep`

> Sweep demand over 1000, 5000 and 10000 veh/h on the grid (30 s each) and tell me where the channel starts to saturate.

Tools: simulate

    **The channel did not reach saturation at any tested demand level.** At the highest demand, signature verification—not radio access—showed a bottleneck.
    
    | Demand (veh/h) | Channel busy ratio | Channel load | MAC drops | MAC queue depth |
    |---:|---:|---:|---:|---:|
    | 1000 | 0 | 0.00318 | 0 | 0 |
    | 5000 | 0.00071 | 0.00382 | 0 | 0 |
    | 10000 | 0.00114 | 0.00427 | 0 | 0 |
    
    - **Verification pressure at 10000 veh/h** [access.verification]: messages left unverified reached **0.059 ratio**, with verification queue depth (p95) of **0.779 messages**, flagged at **9–10 s**.
    - **Weak signals dominate losses, rather than congestion** [channel.weak-signal]: below-sensitivity loss counts were **154 / 5439 / 25936 frames**. Relative to 1000 veh/h, the increases were **+5285 frames (+3432%)** and **+25782 frames (+16742%)**.
    - **Reception range worsened** [delivery.range]: reception fell below **0.9** from **120 m** at 5000 veh/h and **100 m** at 10000 veh/h.
    
    The saturation threshold remains undetermined because none of these arms reached it. These are same-seed results with no confidence interval; backend and privacy were not measured.

### `followup-base`

> Run the grid at 2000 veh/h for 30 s.

Tools: simulate

    Completed the grid run at **2000 veh/h for 30 s**. The main problems were weak-signal losses and an unhealthy share of unverified messages.
    
    - **Weak signals dominated losses:** below-sensitivity accounted for **0.998 ratio** of losses. [channel.weak-signal]
    - **Verification warning:** messages left unverified reached **0.051 ratio**, with a verification queue depth of **1 messages (p95)**; flagged at **28–29 s**. [access.verification]
    - **Signing dominated latency:** mean signing time was **9 ms**, accounting for **0.737 ratio** of total latency. [latency.dominant-stage]
    - **Reception range:** reception fell below **0.9 ratio** from **500 m**, with **0 ratio** reception in the **500–520 m** bin. [delivery.range]
    
    Backend and privacy metrics were not measured.

### `followup-double`

> Now double the density.

Tools: simulate

    Doubled vehicle demand and reran the grid for **30 s** with the same seed. Delivery improved and the verification warning cleared, but weak signals remained the main loss cause.
    
    | Metric | Previous | Doubled demand | Change |
    |---|---:|---:|---:|
    | Delivery ratio | 0.088 | 0.166 | +88.8% |
    | Age of information | 131 ms | 71.6 ms | −45.5% |
    | Goodput | 951 B/s | 2027 B/s | +113% |
    | Unverified ratio | 0.051 | 0.022 | −56.7% |
    | Median speed | 8.74 m/s | 7.772 m/s | −11.1% |
    
    - **Weak-signal losses persisted:** below-sensitivity accounted for **0.997 ratio** of losses in the new run. [channel.weak-signal]
    - **Reception range worsened:** reception fell below the threshold from **400 m**, versus **500 m** previously. [delivery.range]
    - **Verification improved:** health moved from **unhealthy** to **caution**, and the verification finding disappeared. [access.verification]
    
    These are single-seed results, without confidence intervals. Backend and privacy were not measured.

### `followup-lte`

> Switch that to LTE-V2X and compare with the previous run.

Tools: simulate

    Switched the doubled-demand grid to **LTE-V2X** and compared the same-seed **30 s** runs. **Delivery improved, but critical access-delay, near-range-loss and latency findings appeared.**
    
    | Metric | Previous run | LTE-V2X | Change |
    |---|---:|---:|---:|
    | Delivery ratio | 0.166 | 0.372 | +123% |
    | Goodput | 2027 B/s | 3095 B/s | +52.7% |
    | End-to-end latency (p95) | 14.4 ms | 101 ms | +598% |
    | Age of information | 71.6 ms | 384 ms | +437% |
    | Unverified ratio | 0.022 | 0.078 | +250% |
    
    - **Access delay became the bottleneck:** p95 medium-access delay reached **98.2 ms**, versus an allowed budget share of **20 ms**. [access.delay]
    - **Safety-message latency exceeded budget:** p95 latency reached **101 ms** against **100 ms**; mean `mac_defer` time was **53.3 ms**. [latency.over-budget]
    - **Close neighbours lost frames:** reception was **0.5 ratio** at **40–60 m**. [delivery.near-range-loss]
    - **Information became stale:** age of information reached **384 ms**, above the **300 ms** stale threshold. [delivery.stale-information]
    - **Verification deteriorated:** unverified-message health moved from **caution** to **unhealthy**. [access.verification]
    
    Single-seed results have no confidence intervals. Backend and privacy were not measured.

### `vague-healthy`

> Is the network healthy?

Tools: analyse_current_run

    **No—the current run has critical delivery and latency problems.** Weak-signal losses and medium-access delays are the main bottlenecks.
    
    - **Poor nearby delivery:** PRR at 40–60 m is **0.5 ratio**, below the **0.95** target within 50 m. [delivery.near-range-loss]
    - **Excessive access delay:** p95 medium-access delay is **98.2 ms**, against an allowance of **20 ms**. [access.delay]
    - **Safety-message latency exceeds budget:** p95 end-to-end latency is **101 ms**, above the **100 ms** budget; `mac_defer` averages **53.3 ms**. [latency.over-budget]
    - **Weak signals dominate losses:** fading accounts for **0.894 ratio** of losses. [channel.weak-signal]
    - **Information becomes stale:** p95 age of information is **384 ms**, above the **300 ms** threshold. [delivery.stale-information]
    - **Verification is incomplete:** the unverified message share is **0.078 ratio**. [access.verification]
    
    Backend and privacy health were **not measured**.

### `vague-bottleneck`

> Where's the bottleneck at rush hour on the grid?

Tools: simulate

    **No network bottleneck appeared in this run.** I used doubled baseline vehicle demand as a rush-hour proxy, so this does not establish where a genuinely congested grid would bottleneck.
    
    - **Radio queues were empty:** MAC queue depth and its p95 were both `0`, with `0` MAC drops.
    - **Security adds overhead, but was not shown to limit capacity** [security.overhead]: security overhead was `0.486 ratio`, and air bytes per payload byte were `3.201 B/B`.
    
    The analysis did not identify a street-level traffic bottleneck; backend and privacy were not measured.

### `vague-better`

> Make it better.

Tools: none

    What would you like to improve—message delivery, congestion, safety alerts, privacy, or credential handling—and in which scenario?

### `impossible-wifi7`

> Simulate Wi-Fi 7 between vehicles at 6 GHz.

Tools: none

    This simulator does not support Wi-Fi 7 or a 6 GHz vehicle-to-vehicle radio. It supports DSRC/802.11p, LTE-V2X PC5, NR-V2X PC5, and hybrid configurations.
    
    Would you like to simulate one of those supported technologies instead?

### `impossible-tokyo`

> Run the scenario in downtown Tokyo.

Tools: none

    The simulator doesn’t have a Tokyo scenario. It supports real-city scenarios for Manhattan and downtown Portland, or a procedural grid—which would you like to run?

### `impossible-huge`

> Simulate one million vehicles in Manhattan for 24 hours.

Tools: none

    I can run a Manhattan scenario, but a run of this duration needs your explicit confirmation, and I haven’t verified that the simulator supports one million vehicles. Should I check the supported vehicle scale before proceeding with the 24-hour run?

### `ambiguous-compare`

> Compare them.

Tools: none

    Which scenarios or settings would you like me to compare—for example, radio technologies, traffic densities, or pseudonym-change strategies?

### `ambiguous-attack`

> Attack the network.

Tools: none

    What kind of simulated attack would you like: **radio jamming**, **false vehicle positions**, or **invalid signed messages**? I can run it on the default grid and compare it with an attack-free baseline.

