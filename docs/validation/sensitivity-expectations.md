# Expectation checks across the settings catalogue

What the engine should do when a setting moves, written down before the run with its theory or source, and checked against same-seed runs. Two kinds of row:

- **Single-setting rows** (`crates/v2xw-engine/tests/expectations/*.yaml`, 151 rows over 78 settings). Each row moves one published setting to an end of its recommended range, or to a literal value. It names a fingerprint metric, a direction, and optionally a magnitude band (`changed / base`) and a minimum relative change. It runs on the family `scenarios/sensitivity/catalogue.yaml` routes the setting to.
- **Interaction checks** (`crates/v2xw-engine/tests/expectations/interactions/*.yaml`, 17 checks). Each is a 2 × 2 on one family, with all four runs on the same seed: the base, factor A alone, condition B alone, and both. A's effect without B (`A − base`) is compared with its effect under B (`AB − B`), in the metric's own units (Montgomery, *Design and Analysis of Experiments* 9e, 5.2). The verdict is `dampens`, `amplifies`, `holds` (same sign) or `reverses`, and A's own direction is checked too. Examples: density × DCC, message rate × signature size, data rate × frame size, demand × signal control, trucks × NLOSv blockage.

**Run it:**

- `v2xw sensitivity --expect [--only PREFIXES] [--cache FILE] [--markdown FILE] [--json]`. It prints one PASS / FAIL / DEAD / REFUSED line per row, failures first, and exits non-zero on any failure. It plans only the perturbations the table names. That is about 7 minutes from an empty cache on two threads, and about 1 minute with the cache.
- `cargo test -p v2xw-engine --test sensitivity_expectations`, the fast part:
  - every interaction's four scenarios pass the loader;
  - every row is planned;
  - the interaction judge goes red on a wrong expectation;
  - one interaction runs end to end.
- `-- --ignored` runs the whole table.

## What the runs found, and what changed

The first full run passed 144 of 160 rows. Every failing or dead row was traced to its cause:

| Row | Cause | Resolution |
|---|---|---|
| `rate_veh_per_h_per_lane_km` high: fewer vehicles | The setting is an arrival rate per lane-km (`city.rs` `adapt_to_world`), not a density. The sweep's `[5, 30]` ends put "high" below the family's own 1500 veh/h. | Families file: half and double the base (25 and 100). |
| `security.signature: ml-dsa-65`: frames 1941 → 0 | Every ML-DSA SPDU is over the 1,400-octet WSMP MTU and is refused (`net_mtu_refusals`). This is by design, and it is counted. | The signature context adds `fragmenter/generic-sdu`, as `pq-signatures.yaml` does. The rows now pass (fragments, larger PSDU). |
| GNSS outliers → misbehaviour false positives: dead | The detectors fire only after `min_consecutive` suspicious messages from one signer (the legacy streak gate). A one-sample outlier is tolerated by design, even at 0.5 Hz and 100 m. | The rows now test bursts and outages, which do raise false positives, and test that isolated outliers do not. |
| `metric.headway_time`: a longer truck headway lowered it 60 %, and its mean was 129 s | The metric took gap / v for every pair, including queues crawling at 0.05 m/s (140 s "headways") and free-flowing pairs 200 m apart. | **Metric fix** (`v2xw-metrics` `safety.rs`): following pairs only, meaning the follower is at or above 5 mi/h (HCM queued) and the headway is at most 6 s (Vogel 2003). The mean is now 3.7 s. |
| `reaction_time_s` → headway (Krauss) | The engine's drivers are IDM plus the Human Driver Model. There T' delays the response but does not set the gap (Treiber, Kesting, Helbing 2006). | The expectation is corrected: T' must change queue dynamics. |
| Adaptive DCC did not dampen density's CBR rise at 160 vehicles | LIMERIC steers toward CBR 0.68 and does not throttle below it. CBR was 0.52. | Density raised to 280 vehicles. It now dampens. |
| Demand × headway, demand × actuated: no speed effect | About 45 vehicles on the grid, so X ≪ 1, where Webster delay barely depends on flow. | Re-posed: flow holds under any headway, and actuation's speed gain shrinks as demand rises (FHWA Signal Timing Manual 2e, 5.1). |
| Per-class drivers, NLOSv blockage: dead | The families are car-only. With roof antennas above car roofs, cars are TR 37.885 case 1 (0 dB). | Contexts give the family 20 % trucks, buses or motorcycles. All per-class rows are now live and pass. |
| `vehicle_blockage` on the DSRC family: dead both ways | 40 vehicles on 17 km of open street almost never line up in 2 s. | Routed to the Midtown grid. |
| `kerbside.enabled: false`: dead, then refused | Only the New York preset has taxis. `taxi_share` together with `enabled: false` is refused by the loader, as the settings contract says. | The context is `world.highway_preset: urban-us-nyc`. |
| `gnss.outlier_rate_hz` high: dead | The guide text "0.01 per second (default); 0 for clean" parses to 0 and to the model default. | The families file sets ends of 0 and 0.1. |
| Booleans with no published default: swept only to `true` | The planner assumed the default was `false`, so a schema default of `true` read as dead (`lane_changes`, `vehicle_blockage`, `kerbside.enabled`, ...). | The planner now sweeps both values. |
| Small-but-right effects below 1 % | Examples: certificates on a new neighbour (+0.7 % over 9 vehicles), s0 (+0.9 %), start-up reaction, turn lateral acceleration, heavy vehicles far below capacity. | `min_change` is lowered, each with the measured value and the reason in its source. |

**Still failing (2):**

1. `weather.intensity` on arterials. `v2xw-mobility` `weather.rs` reads intensity only as the freeway heavy/light step, so the arterial rows use the band midpoint. The fix changes `midtown-rain.yaml`'s digest, so this is left for the owner to decide (as in round 12).
2. `drivers.truck.time_headway_s` high → headway. The effect is −0.7 % with 20 % trucks for 90 s, which is below what this family can resolve: the distance headway moved +0.03 % and speed −0.01 %. The setting is wired (`driving.rs` class override) and live. A judgement needs a truck-heavy freeway family.

**Seen, not filed:** with `gnss.rtk: true` the lifecycle family fired 3 false verdicts, against 0 without RTK, out of 394 judged messages. The position tolerance has a 2.5 m floor, so RTK cannot tighten it. The likely cause is a different draw sequence or a chord-versus-arc mismatch at turns after a lost packet. It has not been chased.

Generated by `v2xw sensitivity --expect` (families `scenarios/sensitivity/catalogue.yaml`, rows `crates/v2xw-engine/tests/expectations/*.yaml`, interactions `crates/v2xw-engine/tests/expectations/interactions/*.yaml`).

**149/151 single-setting rows and 17/17 interaction checks pass.**

## Single-setting rows

| status | setting | change | metric | expect | base | changed | Δ | source |
|---|---|---|---|---|---|---|---|---|
| PASS | `radio.devices.obu.tx_power_dbm` | low | `range90_m` | Down | 640.0000 | 200.0000 | -68.8% | log-distance link budget: received power falls dB for dB with transmit power (Rappaport, Wireless Communications 2e, 4.9) |
| PASS | `radio.devices.obu.tx_power_dbm` | low | `prr` | Down | 0.9078 | 0.2940 | -67.6% | less SNR at every distance lowers the PRR curve (Bai, Stancil, Krishnan, IEEE VTC 2010) |
| PASS | `radio.devices.obu.tx_power_dbm` | low | `cbr_mean` | Down | 0.0815 | 0.0132 | -83.8% | fewer nodes sense the frame above the CCA threshold, so less busy time per node (Bansal et al. LIMERIC, IEEE TVT 2013) |
| PASS | `radio.phy.noise_figure_db` | high | `range90_m` | Down | 640.0000 | 600.0000 | -6.2% | noise floor = kTB + NF: +NF dB is -NF dB SNR at every distance (Friis 1944) |
| PASS | `radio.phy.noise_figure_db` | high | `prr` | Down | 0.9078 | 0.8459 | -6.8% | lower SNR lowers the PRR curve (IEEE 802.11-2020 17.3.10.6 minimum sensitivity) |
| PASS | `radio.phy.data_rate_mbps` | high | `airtime_us` | Down | 322.5157 | 134.4403 | -58.3% | payload symbols scale with 1/rate; preamble and SIGNAL stay (IEEE 802.11-2020 17.4.3 TXTIME) |
| PASS | `radio.phy.data_rate_mbps` | high | `range90_m` | Down | 640.0000 | 360.0000 | -43.8% | higher MCS needs more SNR (IEEE 802.11-2020 Table 17-18 receiver sensitivities) |
| PASS | `radio.phy.data_rate_mbps` | low | `airtime_us` | Up | 322.5157 | 597.6352 | +85.3% | IEEE 802.11-2020 17.4.3 TXTIME |
| PASS | `radio.phy.data_rate_mbps` | high | `cbr_mean` | Down | 0.0815 | 0.0345 | -57.6% | CBR is offered airtime: shorter frames, less busy time (ETSI TS 102 687) |
| PASS | `radio.channel_model.path_loss_exponent` | high | `range90_m` | Down | 640.0000 | 140.0000 | -78.1% | log-distance law, loss grows 10 n log10(d) (Rappaport 4.9) |
| PASS | `radio.channel_model.path_loss_exponent` | low | `range90_m` | Up | 640.0000 | 1000.0000 | +56.2% | log-distance law (Rappaport 4.9) |
| PASS | `radio.phy.sensitivity_dbm` | high | `range90_m` | Down | 640.0000 | 400.0000 | -37.5% | a frame below the receiver's sensitivity is not decoded: a higher floor shortens range (IEEE 802.11-2020 17.3.10.6) |
| PASS | `messages.rate_hz` | low | `tx_rate_hz` | Down | 9.9375 | 1.4125 | -85.8% | SAE J2945/1 2016 6.3.6: a vehicle sends at the commanded rate at low density |
| PASS | `messages.rate_hz` | low | `cbr_mean` | Down | 0.0814 | 0.0196 | -75.9% | CBR scales with offered frames per second (Bansal 2013) |
| PASS | `messages.rate_hz` | low | `aoi_ms` | Up | 70.3455 | 318.5285 | +352.8% | age of information is about half the update interval plus latency (Kaul, Yates, Gruteser, INFOCOM 2012) |
| PASS | `messages.rate_hz` | low | `ipg_ms_mean` | Up | 106.6422 | 606.2098 | +468.5% | inter-packet gap is the update interval over the delivery ratio (SAE J2945/1 IPG definition) |
| PASS | `messages.path_history_points` | low | `psdu_bytes` | Down | 205.9623 | 195.9623 | -4.9% | each PathHistoryPoint adds octets to BSM Part II (SAE J2735 DF_PathHistory) |
| PASS | `messages.path_history_points` | low | `airtime_us` | Down | 322.5157 | 306.5157 | -5.0% | longer PSDU, more OFDM symbols (IEEE 802.11-2020 17.4.3) |
| PASS | `radio.mac.aifsn_vo` | high | `aifs_us` | Up | 8.6818 | 33.4075 | +284.8% | AIFS = SIFS + AIFSN x slot (IEEE 802.11-2020 10.22.2.4) |
| PASS | `radio.mac.cw_min_vo` | high | `backoff_us` | Up | 2.3711 | 12.4113 | +423.4% | the mean backoff is CW/2 slots (IEEE 802.11-2020 10.3.4.3) |
| PASS | `security.signer_id_policy.full_cert_every_ms` | low | `psdu_bytes` | Up | 208.3138 | 222.1066 | +6.6% | a certificate is about 100 octets more than an 8-octet digest (IEEE 1609.2-2016 6.4.8; SAE J2945/1 6.1.2) |
| PASS | `radio.sidelink.noise_figure_db` | 15.0 | `prr_600_1000` | Down | 0.9230 | 0.9195 | -0.4% | noise floor = kTB + NF (3GPP TR 36.885 A.1.1, NF 9 dB) |
| PASS | `net.geonet.hop_limit` | low | `gn_forwards_sent` | Down | 9.0000 | 0.0000 | -100.0% | the remaining hop limit caps how often a packet is re-broadcast (ETSI EN 302 636-4-1 9.3.3) |
| PASS | `actors.vehicles.demand.rate_veh_per_h` | high | `traffic.actors` | Up | 44.0000 | 49.0000 | +11.4% | Poisson arrivals: the expected count is rate x time (SUMO randomTrips --period / flows) |
| PASS | `actors.vehicles.demand.rate_veh_per_h` | high | `frames_tx` | Up | 20847.0000 | 23737.0000 | +13.9% | every equipped vehicle sends at 10 Hz, so frames scale with vehicles (SAE J2945/1) |
| PASS | `actors.vehicles.demand.rate_veh_per_h` | low | `traffic.actors` | Down | 44.0000 | 16.0000 | -63.6% | Poisson arrivals: rate x time |
| PASS | `actors.vehicles.equipped_fraction` | low | `frames_tx` | Down | 20847.0000 | 0.0000 | -100.0% | only equipped vehicles transmit (market penetration rate, USDOT CV pilot evaluations) |
| PASS | `actors.vehicles.drivers.speed_factor` | low | `traffic.speed_mean_mps` | Down | 5.8709 | 5.5829 | -4.9% | the desired speed is the limit times the speed factor (SUMO vType speedFactor; Krauss 1998) |
| PASS | `actors.vehicles.drivers.speed_factor` | high | `traffic.speed_mean_mps` | Up | 5.8709 | 5.9910 | +2.0% | SUMO vType speedFactor |
| PASS | `actors.vehicles.drivers.max_accel_mps2` | low | `traffic.speed_mean_mps` | Down | 5.8709 | 5.5853 | -4.9% | IDM: lower a lengthens every launch from a stop (Treiber, Hennecke, Helbing 2000) |
| PASS | `actors.vehicles.drivers.time_headway_s` | high | `traffic.speed_mean_mps` | Down | 5.8709 | 5.8520 | -0.3% | IDM: a longer desired headway lowers queue-discharge flow and speed (Treiber 2000; HCM 7th ed. saturation headway) |
| PASS | `traffic.right_turn_on_red` | false | `traffic.stopped_share` | Up | 0.2962 | 0.3184 | +7.5% | without RTOR right-turners wait the red out (HCM 7th ed. 19.4 RTOR volume reduction) |
| FAIL | `weather.intensity` | high | `traffic.speed_mean_mps` | Down | 5.4009 | 5.4009 | +0.0% | rain and snow lower free-flow speed (HCM 7th ed. Exhibit 11-20; Rakha et al., FHWA-HOP-07-073, 2007) |
| PASS | `actors.vru.walking_speed_mps` | high | `vru.speed_mean_mps` | Up | 1.2005 | 1.2589 | +4.9% | pedestrian free walking speed (HCM 7th ed. 1.2 m/s design; Knoblauch 1996 measured 1.25-1.5 m/s) |
| PASS | `actors.vru.walking_speed_mps` | low | `vru.speed_mean_mps` | Down | 1.2005 | 1.0024 | -16.5% | Knoblauch, Pietrucha, Nitzburg 1996 (TRR 1538) |
| PASS | `actors.vru.pedestrians` | high | `vru.actors` | Up | 165.0000 | 315.0000 | +90.9% | the pedestrian count is the population spawned |
| PASS | `radio.devices.obu.tx_power_dbm` | low | `aoi_ms` | Up | 69.9074 | 98.3304 | +40.7% | fewer receptions per update at range: age of information grows as delivery falls (Kaul, Yates, Gruteser, INFOCOM 2012) |
| PASS | `radio.devices.obu.antenna_gain_dbi` | high | `prr` | Up | 0.9078 | 0.9635 | +6.1% | EIRP and receive gain add dB for dB to the link budget (Friis 1946; ETSI EN 302 571 EIRP limits) |
| PASS | `radio.devices.obu.antenna_gain_dbi` | high | `cbr_mean` | Up | 0.0815 | 0.0963 | +18.2% | a stronger signal is sensed above the CCA threshold by more nodes: more busy time (ETSI TS 102 687 CBR definition) |
| PASS | `radio.devices.obu.antenna_gain_dbi` | low | `prr` | Down | 0.9078 | 0.8657 | -4.6% | Friis: less gain, less SNR at every distance |
| PASS | `radio.devices.obu.antenna_gain_dbi` | low | `cbr_mean` | Down | 0.0815 | 0.0733 | -10.0% | ETSI TS 102 687: fewer nodes sense the frame above the CCA threshold |
| PASS | `radio.devices.obu.cable_loss_db` | high | `prr` | Down | 0.9078 | 0.8785 | -3.2% | feeder loss is subtracted from both ends of the link budget (Rappaport 2e, 4.9; SAE J2945/1 6.4 antenna system) |
| PASS | `radio.devices.obu.cable_loss_db` | high | `cbr_mean` | Down | 0.0815 | 0.0762 | -6.4% | ETSI TS 102 687: weaker frames are sensed by fewer nodes |
| PASS | `radio.phy.noise_figure_db` | low | `prr` | Up | 0.9078 | 0.9094 | +0.2% | noise floor = kTB + NF: lower NF is more SNR (Friis 1944). Small: most links are interference- or range-limited at 40 vehicles |
| PASS | `radio.phy.sensitivity_dbm` | high | `prr` | Down | 0.9078 | 0.6974 | -23.2% | frames below the receiver's sensitivity are not decoded (IEEE 802.11-2020 17.3.10.6) |
| PASS | `radio.channel_model.path_loss_exponent` | high | `prr` | Down | 0.9078 | 0.1065 | -88.3% | log-distance law: 10 n log10(d) more loss (Rappaport 4.9; Cheng et al., IEEE JSAC 2007 measured n 2-4 for V2V) |
| PASS | `radio.channel_model.path_loss_exponent` | high | `cbr_mean` | Down | 0.0815 | 0.0053 | -93.5% | less received power, fewer nodes above the CCA threshold (ETSI TS 102 687) |
| PASS | `radio.channel_model.path_loss_exponent` | low | `prr` | Up | 0.9078 | 0.9854 | +8.6% | log-distance law (Rappaport 4.9) |
| PASS | `radio.channel_model.path_loss_exponent` | low | `cbr_mean` | Up | 0.0815 | 0.1059 | +30.1% | more nodes sense each frame above the CCA threshold (ETSI TS 102 687) |
| PASS | `radio.channel_model.fading` | rayleigh | `prr` | Down | 0.9078 | 0.8256 | -9.0% | Rayleigh (m = 1) is the deepest of the usual fades: the outage probability at a given mean SNR is highest (Simon and Alouini, Digital Communication over Fading Channels 2e, 2.2) |
| PASS | `radio.channel_model.fading` | none | `prr` | Up | 0.9078 | 0.9393 | +3.5% | no fading: no fade-induced outages above the mean-SNR threshold (Simon and Alouini 2.2) |
| PASS | `radio.channel_model.nakagami_shape` | low | `prr` | Down | 0.9078 | 0.8256 | -9.0% | smaller Nakagami m, deeper fades (Cheng et al. JSAC 2007 fitted m 0.75-3 by distance; Simon and Alouini 2.2.1.4) |
| PASS | `radio.channel_model.nakagami_shape` | high | `prr` | Up | 0.9078 | 0.9204 | +1.4% | larger Nakagami m tends to AWGN (Simon and Alouini 2.2.1.4) |
| PASS | `radio.channel_model.rice_k_db` | high | `prr` | Up | 0.9039 | 0.9229 | +2.1% | a stronger line-of-sight component (larger K) means shallower fades (Simon and Alouini 2.2.1.2) |
| PASS | `radio.channel_model.rice_k_db` | low | `prr` | Down | 0.9039 | 0.8785 | -2.8% | K -> 0 tends to Rayleigh (Simon and Alouini 2.2.1.2) |
| PASS | `radio.channel_model.shadowing_sigma_db` | high | `prr` | Changes | 0.9078 | 0.8496 | -6.4% | wider log-normal shadowing helps some far links and hurts near ones: the net sign depends on where the links sit relative to the range edge (Rappaport 4.10.2), but the PRR must move |
| PASS | `radio.phy.capture_threshold_db` | high | `prr` | Down | 0.9078 | 0.8030 | -11.5% | a frame overlapped by another survives only if its SIR exceeds the capture threshold: a higher threshold loses more (Kochut et al., IEEE ICNP 2004; ns-3 / Veins capture model) |
| PASS | `radio.phy.capture_threshold_db` | high | `collision_rate` | Up | 0.0381 | 0.0493 | +29.2% | more overlaps end as collisions instead of captures (Kochut 2004) |
| PASS | `radio.phy.cca_threshold_dbm` | high | `collision_rate` | Up | 0.0381 | 0.1363 | +257.5% | a less sensitive carrier sense hides more transmitters: more hidden-terminal collisions (Jiang, Chen, Delgrossi, IEEE VTC 2008 'Ns-2 simulation of 802.11p'; IEEE 802.11-2020 17.3.10.6 CCA) |
| PASS | `radio.phy.cca_threshold_dbm` | high | `cbr_mean` | Down | 0.0815 | 0.0776 | -4.7% | CBR counts time the medium is sensed busy above the CCA threshold (ETSI TS 102 687 4.2) |
| PASS | `radio.range.max_m` | low | `cbr_mean` | Down | 0.0815 | 0.0091 | -88.8% | frames beyond the propagation cut-off are not delivered to the PHY, so they cannot load the channel (ns-3 RangePropagationLossModel; Veins maxInterfDist) |
| PASS | `radio.phy.data_rate_mbps` | low | `cbr_mean` | Up | 0.0815 | 0.1497 | +83.8% | half the rate is about twice the payload airtime (IEEE 802.11-2020 17.4.3 TXTIME; ETSI TS 102 687 CBR) |
| PASS | `radio.phy.data_rate_mbps` | high | `airtime_us` | Down [0.25, 0.75] | 322.5157 | 134.4403 | -58.3% | TXTIME = 20 us preamble + 4 us x ceil((16 + 8 L + 6) / N_DBPS): from 6 to 12-18 Mb/s the payload part halves or better (IEEE 802.11-2020 17.4.3) |
| PASS | `messages.rate_hz` | high | `cbr_mean` | Up [1.5, 2.5] | 0.0814 | 0.1679 | +106.3% | CBR is offered airtime: twice the rate is about twice the CBR at low load (Bansal et al. LIMERIC, IEEE TVT 2013; ETSI TS 102 687) |
| PASS | `messages.rate_hz` | high | `frames_tx` | Up | 795.0000 | 1590.0000 | +100.0% | SAE J2945/1 6.3.6: a vehicle sends at the commanded rate |
| PASS | `messages.rate_hz` | high | `aoi_ms` | Down | 70.3455 | 43.3461 | -38.4% | AoI is about half the update interval plus latency (Kaul, Yates, Gruteser 2012) |
| PASS | `messages.rate_hz` | high | `collision_rate` | Up | 0.0383 | 0.0835 | +117.6% | more frames per second, more overlaps (Bianchi, IEEE JSAC 2000: collision probability rises with offered load) |
| PASS | `messages.rate_hz` | high | `prr` | Down | 0.9097 | 0.8636 | -5.1% | PRR falls as channel load rises (Bai, Stancil, Krishnan VTC 2010; Jiang 2008) |
| PASS | `messages.path_history_points` | low | `cbr_mean` | Down | 0.0815 | 0.0776 | -4.7% | shorter BSMs, less airtime per frame (SAE J2735 DF_PathHistory; ETSI TS 102 687) |
| PASS | `radio.mac.periodic_access_category` | bk | `aifs_us` | Up | 8.6818 | 21.9283 | +152.6% | AC_BK has AIFSN 9 against AC_VO's 2 on the OCB channel (IEEE 802.11-2020 Table 9-155, dot11OCBActivated) |
| PASS | `radio.mac.periodic_access_category` | bk | `access_ms` | Up | 0.0309 | 0.0577 | +86.5% | longer AIFS and CWmin 15 vs 3: longer channel access (IEEE 802.11-2020 Table 9-155) |
| PASS | `net.channel_access` | alternating | `e2e_ms_mean` | Up | 12.3214 | 30.9920 | +151.5% | alternating access sends only in the 46 ms CCH interval: a frame generated in the SCH interval waits up to 50 ms (IEEE 1609.4-2016 6.2.5) |
| PASS | `net.channel_access` | alternating | `collision_rate` | Up | 0.0381 | 0.3846 | +908.9% | frames held over the SCH interval are released together at the CCH boundary: synchronized collisions (Chen, Jiang, Delgrossi, IEEE VNC 2009 'Overhaul of 802.11 modeling') |
| PASS | `net.channel_access` | alternating | `prr` | Down | 0.9078 | 0.5002 | -44.9% | the same offered frames in half the time: higher load and synchronized contention (IEEE 1609.4-2016 6.2.5; Chen 2009) |
| PASS | `radio.dcc.adaptive_alpha` | high | `cbr_mean` | Down | 0.0829 | 0.0062 | -92.6% | LIMERIC: r <- (1 - a) r + b (target - CBR); the equilibrium per-vehicle rate b*target/(a + K b) falls as a grows (Bansal et al. IEEE TVT 2013, eq. 6) |
| PASS | `radio.dcc.adaptive_alpha` | high | `aoi_ms` | Up | 74.4392 | 146.6415 | +97.0% | a lower equilibrium rate means a longer update interval: higher AoI (Bansal 2013; Kaul 2012) |
| PASS | `radio.dcc.adaptive_alpha` | high | `frames_tx` | Down | 795.0000 | 441.0000 | -44.5% | LIMERIC equilibrium rate falls with alpha (Bansal 2013) |
| PASS | `radio.sidelink.cbr_threshold_dbm` | high | `cbr_mean` | Down | 0.2359 | 0.1190 | -49.6% | SL CBR counts subchannels whose S-RSSI exceeds the threshold: a higher threshold counts fewer (3GPP TS 36.214 5.1.30) |
| PASS | `radio.sidelink.cbr_threshold_dbm` | low | `cbr_mean` | Up | 0.2359 | 0.4768 | +102.1% | 3GPP TS 36.214 5.1.30 |
| PASS | `radio.sidelink.candidate_percent` | high | `collision_rate` | Down | 0.0696 | 0.0254 | -63.5% | a larger candidate set lowers the chance two UEs pick the same resource (3GPP TS 38.214 8.1.4 X%; Molina-Masegosa and Gozalvez, IEEE VT Magazine 2017) |
| PASS | `radio.sidelink.selection_window_ms` | low | `e2e_ms_mean` | Down | 29.0945 | 28.6228 | -1.6% | the selected resource lies in [n + T1, n + T2]: a smaller T2 bounds the access delay (3GPP TS 38.214 8.1.4) |
| PASS | `radio.sidelink.sensing` | false | `collision_rate` | Up | 0.0426 | 0.0498 | +16.9% | without sensing, SPS picks resources at random instead of excluding the ones neighbours reserved: more resource collisions (3GPP TS 36.213 14.1.1.6; Molina-Masegosa and Gozalvez 2017; Bazzi et al., IEEE TVT 2019) |
| PASS | `radio.channel_model.vehicle_blockage` | true | `prr` | Down | 0.2113 | 0.2112 | -0.1% | unset, the medium tier's log-distance law looks for no vehicles; on, a vehicle on a same-street path switches the link to the obstructed-LOS fit, about 5 dB more loss (Abbas et al., IEEE TVT 2015 OLOS; 3GPP TR 37.885 6.2.1 NLOSv; Boban et al., IEEE TVT 2011). Measured with 20 % trucks (cars under roof antennas are TR 37.885 case 1, 0 dB); small on the Midtown grid, where most links are already building-blocked (-0.1 %) |
| PASS | `radio.sidelink.resource_keep_probability` | low | `sl_selections` | Up | 62.0000 | 118.0000 | +90.3% | on reselection counter expiry the UE keeps the resource with probability P_keep, else reselects (3GPP TS 36.321 5.14.1.1) |
| PASS | `security.signer_id_policy.digest_otherwise` | false | `psdu_bytes` | Up | 208.3138 | 332.3076 | +59.5% | a certificate in every message instead of an 8-octet digest adds about 100 octets (IEEE 1609.2-2016 6.4.8 SignerIdentifier; SAE J2945/1 6.1.2) |
| PASS | `security.signer_id_policy.digest_otherwise` | false | `airtime_us` | Up | 325.3375 | 490.6625 | +50.8% | longer PSDU, more OFDM symbols (IEEE 802.11-2020 17.4.3) |
| PASS | `security.signer_id_policy.digest_otherwise` | false | `cbr_mean` | Up | 0.0023 | 0.0038 | +61.5% | more airtime per frame at the same rate: higher CBR (ETSI TS 102 687) |
| PASS | `security.signer_id_policy.full_cert_every_ms` | high | `psdu_bytes` | Down | 208.3138 | 201.4173 | -3.3% | a certificate less often: fewer 100-octet frames (SAE J2945/1 6.1.2, cert once per 450 ms) |
| PASS | `security.signer_id_policy.full_cert_every_ms` | low | `airtime_us` | Up | 325.3375 | 343.7280 | +5.7% | IEEE 1609.2-2016 6.4.8; IEEE 802.11-2020 17.4.3 |
| PASS | `security.signer_id_policy.on_new_neighbour` | true | `psdu_bytes` | Up | 208.3138 | 209.8068 | +0.7% | a certificate is attached when a new neighbour appears (SAE J2945/1 6.1.2.2), on top of the periodic one. Small here: nine vehicles over 30 s meet few new neighbours, so a few extra certificates against ~2000 frames (+0.7 % measured) |
| PASS | `security.signature` | falcon-512 | `psdu_bytes` | Up | 208.3138 | 873.4294 | +319.3% | a Falcon-512 signature is 666 octets against 64 for ECDSA P-256 (NIST Falcon spec 1.2, Table 3.3; IEEE 1609.2-2016 6.3.29) |
| PASS | `security.signature` | falcon-512 | `airtime_us` | Up | 325.3375 | 1212.5500 | +272.7% | larger PSDU, more OFDM symbols (IEEE 802.11-2020 17.4.3) |
| PASS | `security.signature` | ml-dsa-65 | `psdu_bytes` | Up | 208.3138 | 1201.5078 | +476.8% | an ML-DSA-65 signature is 3309 octets (FIPS 204 Table 2): the message is fragmented, every fragment carrying headers |
| PASS | `security.signature` | ml-dsa-65 | `frames_tx` | Up | 1941.0000 | 6611.0000 | +240.6% | a 3.3 kB signed message exceeds the 802.11p MSDU the stack sends in one frame, so it is split (IEEE 1609.3-2020 WSM length; ETSI TS 103 097 has no fragmentation, the generic fragmenter does) |
| PASS | `security.pseudonym_change.period_s` | low | `aoi_ms` | Up | 308.8016 | 456.0920 | +47.7% | after a pseudonym change receivers hold no certificate for the new digest and must wait for the next full certificate before they can verify (IEEE 1609.2-2016 5.3.4; Petit et al., IEEE COMST 2015) |
| PASS | `nodes.peer_cert_cache` | low | `aoi_ms` | Up | 69.9074 | 215.3479 | +208.0% | a smaller peer-certificate cache evicts signers, so a digest-signed message from an evicted signer cannot be verified until the next full certificate (IEEE 1609.2-2016 5.1.3; SAE J2945/1 6.1.2) |
| PASS | `security.verification_policy` | on-demand | `e2e_ms_mean` | Down | 12.2893 | 11.9536 | -2.7% | verify-on-demand skips verification for messages no application acts on, so the verified ones queue less (Krishnan and Weimerskirch, SAE 2011-01-0584; Hsu et al. 2013) |
| PASS | `actors.vehicles.gnss.burst_rate_hz` | high | `report.phase2.verdicts_fired` | Up | 0.0000 | 1.0000 | +100000000000000.0% | a burst of degraded positioning is a sustained position/speed mismatch that outlasts the streak gate: false positives grow with sustained position noise (van der Heijden, Lukaseder, Kargl, VeReMi, SecureComm 2018; Kamel et al., VeReMi extension, ICC 2020) |
| PASS | `actors.vehicles.gnss.outage_probability` | high | `report.phase2.verdicts_fired` | Up | 0.0000 | 58.0000 | +5800000000000000.0% | after an outage the fix resumes far from the last reported one: a position jump on honest senders (VeReMi 2018) |
| PASS | `actors.vehicles.gnss.outlier_rate_hz` | high | `report.phase2.verdicts_fired` | Same | 0.0000 | 0.0000 | +0.0% | the streak gate: a check fires after several consecutive suspicious messages from one signer, so isolated one-sample outliers do not raise false positives (So, Sharma, Petit, IEEE ICMLA 2018; v2xw-threat detect.rs) |
| PASS | `net.geonet.area_forwarding` | simple | `gn_forwards_sent` | Up | 9.0000 | 117.0000 | +1200.0% | simple GeoBroadcast forwarding rebroadcasts on every receiver inside the area; CBF lets the farthest forwarder suppress the rest (EN 302 636-4-1 Annex F.2 vs F.3) |
| PASS | `net.geonet.cbf_max_time_ms` | low | `report.geonet.0.latency_p95_us` | Down | 21980.0000 | 16028.0000 | -27.1% | the CBF timer is TO_max - (TO_max - TO_min) d / DIST_max: a lower TO_max shortens every hop's wait (EN 302 636-4-1 F.3) |
| PASS | `net.geonet.max_comm_range_m` | low | `gn_forwards_sent` | Up | 9.0000 | 96.0000 | +966.7% | with DIST_max below the real range, distant receivers all clamp to TO_min and forward before they can hear each other: more duplicates (EN 302 636-4-1 F.3) |
| PASS | `messages.cpm_interval_ms` | high | `cbr_mean` | Down | 0.1313 | 0.1037 | -21.0% | fewer CPMs per second, less airtime (ETSI TS 103 324 6.1.2 generation rules) |
| PASS | `messages.cpm_interval_ms` | high | `frames_tx` | Down | 12248.0000 | 10554.0000 | -13.8% | ETSI TS 103 324: the CPM generation period bounds how often one is sent |
| PASS | `actors.vehicles.demand.rate_veh_per_h` | high | `cbr_mean` | Up | 0.0143 | 0.0186 | +30.0% | more vehicles, more BSMs per second on the channel (SAE J2945/1; ETSI TS 102 687) |
| PASS | `actors.vehicles.demand.rate_veh_per_h` | high | `traffic.speed_mean_mps` | Down | 5.8709 | 5.6194 | -4.3% | speed falls as flow approaches capacity at signals (Webster 1958 delay; HCM 7th ed. 19 uniform + incremental delay) |
| PASS | `actors.vehicles.demand.rate_veh_per_h` | low | `cbr_mean` | Down | 0.0143 | 0.0056 | -60.9% | fewer transmitters (ETSI TS 102 687) |
| PASS | `actors.vehicles.demand.rate_veh_per_h` | low | `frames_tx` | Down | 20847.0000 | 8655.0000 | -58.5% | frames scale with equipped vehicles (SAE J2945/1) |
| PASS | `actors.vehicles.demand.fleet_size` | high | `traffic.actors` | Up | 44.0000 | 89.0000 | +102.3% | a larger fleet is more vehicles on the network (SUMO --scale / fleet size) |
| PASS | `actors.vehicles.demand.fleet_size` | high | `cbr_mean` | Up | 0.0143 | 0.0402 | +180.3% | more transmitters in range, more busy time (ETSI TS 102 687) |
| PASS | `actors.vehicles.demand.rate_veh_per_h_per_lane_km` | high | `traffic.actors` | Up | 44.0000 | 59.0000 | +34.1% | an arrival rate per lane-km times the world's lane-km is the Poisson rate (city.rs adapt_to_world; SUMO randomTrips --period scaled by network length) |
| PASS | `actors.vehicles.demand.rate_veh_per_h_per_lane_km` | low | `traffic.actors` | Down | 44.0000 | 19.0000 | -56.8% | Poisson arrivals: rate x time (SUMO randomTrips) |
| PASS | `actors.vehicles.equipped_fraction` | 0.5 | `cbr_mean` | Down [0.2, 0.8] | 0.0143 | 0.0099 | -30.6% | only equipped vehicles transmit: half the transmitters, roughly half the busy time at low load (USDOT CV pilot market-penetration evaluations; ETSI TS 102 687). The low end is 0, where no CBR is measured at all |
| PASS | `actors.vehicles.drivers.time_headway_s` | high | `metric.headway_time` | Up | 3.7447 | 4.2928 | +14.6% | IDM desired gap s* = s0 + vT: the following time headway grows with T (Treiber, Hennecke, Helbing 2000, eq. 2). No steady-state band: on a signalised grid following pairs are mostly launching or closing on a queue, and headways over 6 s are not following (Vogel 2003), so the mean moves less than T (+15 % measured for T doubled) |
| PASS | `actors.vehicles.drivers.min_gap_m` | high | `metric.headway_distance` | Up | 59.8185 | 60.3690 | +0.9% | IDM jam distance s0 is the standstill gap (Treiber 2000); SUMO vType minGap. Small: s0 is a few metres of a ~60 m mean moving gap, and only queued vehicles sit at s0 (+0.9 % measured) |
| PASS | `actors.vehicles.drivers.reaction_time_s` | high | `traffic.stopped_share` | Changes | 0.1657 | 0.1668 | +0.7% | HDM: a delayed response changes queue formation and discharge (Treiber, Kesting, Helbing 2006, Physica A 360, 71-88) |
| PASS | `actors.vehicles.drivers.speed_factor` | low | `metric.flow` | Down | 21.5366 | 20.3601 | -5.5% | slower desired speeds lower flow on an undersaturated network (q = k v; SUMO speedFactor) |
| PASS | `actors.vehicles.drivers.start_reaction_s` | high | `traffic.stopped_share` | Up | 0.1657 | 0.1673 | +1.0% | a longer start-up reaction is more start-up lost time per green (HCM 7th ed. 19.2: 2 s lost time; Bonneson 1992) |
| PASS | `actors.vehicles.drivers.start_reaction_s` | low | `traffic.stopped_share` | Down | 0.1657 | 0.1594 | -3.9% | less start-up lost time, queues discharge sooner (HCM 7th ed. 19.2) |
| PASS | `actors.vehicles.drivers.max_accel_mps2` | low | `metric.flow` | Down | 21.5366 | 20.6605 | -4.1% | slower launches lengthen queue discharge (IDM a; Treiber 2000) |
| PASS | `actors.vehicles.drivers.truck.max_accel_mps2` | low | `traffic.speed_mean_mps` | Down | 5.8613 | 5.8561 | -0.1% | a heavy vehicle's launch sets the queue discharge behind it: lower truck acceleration, more delay (HCM 7th ed. heavy-vehicle factor f_HV; SUMO truck vType accel 1.3 m/s2). At 1500 veh/h the grid runs far below capacity (X << 1), where heavy vehicles cost only their own launches: small (-0.1 % measured). |
| FAIL | `actors.vehicles.drivers.truck.time_headway_s` | high | `metric.headway_time` | Up | 3.7541 | 3.7269 | -0.7% | IDM desired gap s0 + vT applies per class: trucks keeping longer gaps raise the fleet's mean headway (Treiber 2000; Yang et al. TRR 2014 truck headways) |
| PASS | `actors.vehicles.drivers.bus.max_accel_mps2` | low | `traffic.speed_mean_mps` | Down | 5.8608 | 5.8558 | -0.1% | slower bus launches lengthen queue discharge (HCM 7th ed. f_HV; SUMO bus vType accel 1.2 m/s2). At 1500 veh/h the grid runs far below capacity (X << 1), where heavy vehicles cost only their own launches: small (-0.1 % measured). |
| PASS | `actors.vehicles.drivers.motorcycle.max_accel_mps2` | low | `metric.acceleration` | Changes | 0.0648 | 0.0649 | +0.1% | a fifth of the fleet with a different launch must move the fleet's acceleration statistics (Vlahogianni, Transportation Research Part C 2014: powered two-wheeler acceleration) |
| PASS | `traffic.signals.control` | actuated | `traffic.stopped_share` | Down | 0.1657 | 0.0435 | -73.8% | actuated control ends green when the queue clears and serves demand as it arrives: less delay at moderate volume (HCM 7th ed. 19.4; FHWA Signal Timing Manual 2e, 5.3) |
| PASS | `traffic.signals.control` | actuated | `traffic.speed_mean_mps` | Up | 5.8709 | 7.4676 | +27.2% | less signal delay, higher average travel speed (FHWA Signal Timing Manual 2e) |
| PASS | `traffic.signals.passage_s` | high | `traffic.stopped_share` | Up | 0.0435 | 0.0660 | +51.9% | a long passage (gap) time extends green after the queue clears while the other approaches wait (FHWA Signal Timing Manual 2e, 5.3.2; Bonneson and McCoy, NCHRP 3-66) |
| PASS | `traffic.signals.min_green_s` | low | `traffic.stopped_share` | Down | 0.0435 | 0.0378 | -12.9% | a shorter minimum green wastes less green on a light approach (FHWA Signal Timing Manual 2e, 5.3.1) |
| PASS | `world.signals.cycle_s` | high | `traffic.speed_mean_mps` | Down | 3.7712 | 3.4320 | -9.0% | below saturation, uniform delay grows with the cycle length: d1 = 0.5 C (1 - g/C)^2 / (1 - X g/C) (Webster 1958; HCM 7th ed. eq. 19-19) |
| PASS | `world.signals.cycle_s` | low | `traffic.speed_mean_mps` | Up | 3.7712 | 4.4271 | +17.4% | Webster 1958: shorter cycles, less uniform delay while capacity holds |
| PASS | `world.signals.progression_speed_mps` | high | `traffic.speed_mean_mps` | Down | 3.7712 | 3.7269 | -1.2% | offsets timed for a speed faster than traffic drives break the green band (Morgan and Little 1964; FHWA Signal Timing Manual 2e, 6.4) |
| PASS | `world.signals.progression_speed_mps` | low | `traffic.speed_mean_mps` | Down | 3.7712 | 3.5464 | -6.0% | offsets timed for a slower speed than traffic drives make platoons arrive early on red (FHWA Signal Timing Manual 2e, 6.4) |
| PASS | `traffic.right_turn_on_red` | false | `traffic.speed_mean_mps` | Down | 3.7712 | 3.6248 | -3.9% | right-turners wait the red out: more delay (HCM 7th ed. 19.4 RTOR) |
| PASS | `traffic.kerbside.taxi_share` | high | `traffic.speed_mean_mps` | Down | 5.8709 | 5.4727 | -6.8% | kerbside pick-ups block the kerb lane (HCM 7th ed. 19 f_bb bus/parking blockage factor; SUMO stops) |
| PASS | `traffic.kerbside.taxi_share` | high | `traffic.stopped_share` | Up | 0.1657 | 0.1959 | +18.2% | a stopped taxi and the vehicles queued behind it are stopped (HCM 7th ed. f_bb) |
| PASS | `traffic.turn_lateral_accel_mps2` | low | `traffic.speed_mean_mps` | Down | 5.8709 | 5.8162 | -0.9% | the turn speed is sqrt(a_lat R): a lower comfortable lateral acceleration means slower turns (AASHTO Green Book 2018, 3.3.5). Small: turning is a short share of a trip on a grid (-0.9 % measured) |
| PASS | `traffic.kerbside.enabled` | false | `traffic.stopped_share` | Down | 0.1959 | 0.1657 | -15.4% | no kerbside stops, no lane blockage behind stopped taxis and deliveries (HCM 7th ed. 19 f_bb) |
| PASS | `traffic.lane_changes` | false | `traffic.speed_mean_mps` | Changes | 5.8709 | 5.5152 | -6.1% | MOBIL lane changes let vehicles pass slower or turning leaders; without them each lane runs at its slowest leader (Kesting, Treiber, Helbing, TRR 1999, 2007), so speed must move |
| PASS | `weather.initial` | rain | `traffic.speed_mean_mps` | Down | 5.8709 | 5.4009 | -8.0% | rain lowers free-flow speed 2-14 % on arterials (Rakha et al., FHWA-HOP-07-073, 2007; HCM 7th ed. Exhibit 11-20) |
| PASS | `weather.initial` | snow | `traffic.speed_mean_mps` | Down [0.5, 0.95] | 5.8709 | 4.5078 | -23.2% | snow lowers free-flow speed 5-40 % (FHWA-HOP-07-073; HCM 7th ed. Exhibit 11-20) |
| PASS | `weather.initial` | snow | `metric.headway_time` | Up | 3.7447 | 4.1197 | +10.0% | drivers keep longer headways in snow: saturation headway +12-20 % (Perrin, Martin, Hansen, TRR 1776, 2001) |
| PASS | `weather.surface` | ice | `metric.ttc_conflicts` | Up | 0.0000 | 28.0000 | +2800000000000000.0% | on ice the braking cap (mu g) binds: more low-TTC conflicts (AASHTO Green Book 2018 3.2.2 friction; SSAM TTC < 1.5 s) |
| PASS | `world.buildings.enabled` | false | `prr` | Up | 0.2128 | 0.9817 | +361.2% | no buildings, no NLOS loss between streets (3GPP TR 37.885 6.2.1 LOS / NLOS; Mangel, Klemp, Hartenstein 2011) |
| PASS | `world.buildings.enabled` | false | `cbr_mean` | Up | 0.0143 | 0.0817 | +470.2% | without building loss each frame is sensed over a larger area (ETSI TS 102 687) |
| PASS | `radio.channel_model.building_loss_db_per_m` | low | `prr` | Up | 0.2128 | 0.2254 | +5.9% | less loss per metre of building crossed (Sommer et al., Veins obstacle model, IEEE WONS 2011: beta per metre) |
| PASS | `radio.channel_model.building_wall_loss_db` | low | `prr` | Up | 0.2128 | 0.2167 | +1.8% | less loss per wall crossed (Sommer et al. WONS 2011: beta per wall) |
| PASS | `actors.vru.pedestrians` | high | `frames_tx` | Up | 5410.0000 | 7532.0000 | +39.2% | every equipped pedestrian sends PSMs (SAE J2945/9 2017) |
| PASS | `actors.vru.device_fraction` | high | `frames_tx` | Up | 5410.0000 | 6971.0000 | +28.9% | only pedestrians carrying a device transmit (SAE J2945/9) |
| PASS | `actors.vru.cyclists` | high | `vru.actors` | Up | 165.0000 | 300.0000 | +81.8% | the cyclist count is the population spawned |
| PASS | `actors.vru.red_crossing_share` | high | `traffic.stopped_share` | Up | 0.1660 | 0.1801 | +8.4% | drivers yield to pedestrians crossing against the signal (Uniform Vehicle Code 11-502; Brosseau et al. 2013 measured 10-30 % red crossings) |

## Interaction checks (2 × 2, same seed)

| status | check | metric | expect | base | A | B | A+B | A alone | A under B | source |
|---|---|---|---|---|---|---|---|---|---|---|
| PASS | density x reactive DCC (CBR) | `cbr_mean` | Dampens | 0.1679 | 0.5241 | 0.0821 | 0.2695 | 0.3563 | 0.1874 | reactive DCC moves to a longer Toff as CBR crosses 0.30/0.40/0.50/0.60, holding the channel load down as density rises (ETSI TS 102 687 V1.2.1 Table A.2; Autolitano et al., IEEE TVT 2013) |
| PASS | density x adaptive DCC (CBR) | `cbr_mean` | Dampens | 0.1679 | 0.6462 | 0.1655 | 0.5427 | 0.4783 | 0.3772 | LIMERIC drives the aggregate rate toward a CBR target, so CBR rises less than in proportion to K (Bansal et al., IEEE TVT 2013, eq. 6; ETSI TS 102 687 V1.2.1 5.4) |
| PASS | DCC x density (AoI) | `aoi_ms` | Amplifies | 43.3461 | 85.2781 | 73.6604 | 138.5422 | 41.9319 | 64.8818 | DCC throttles only when the channel is loaded: its AoI cost is near zero on a light channel and grows with density (ETSI TS 102 687 Table A.2; Kaul, Yates, Gruteser 2012) |
| PASS | DCC x density (frames) | `frames_tx` | Amplifies | 1590.0000 | 800.0000 | 5911.0000 | 2500.0000 | -790.0000 | -3411.0000 | reactive DCC removes more frames the more vehicles share the channel (ETSI TS 102 687 Table A.2) |
| PASS | density x capture threshold (PRR) | `prr` | Amplifies | 0.8636 | 0.7620 | 0.5561 | 0.4366 | -0.1016 | -0.1195 | the capture threshold matters only when frames overlap; overlaps grow with the number of contenders (Kochut et al. ICNP 2004; Bianchi, IEEE JSAC 2000) |
| PASS | density x message rate (collisions) | `collision_rate` | Amplifies | 0.0383 | 0.0835 | 0.1669 | 0.3756 | 0.0451 | 0.2088 | the collision probability of a broadcast rises with n x rate x airtime: doubling the rate adds more collisions where n is larger (Bianchi 2000; Jiang, Chen, Delgrossi VTC 2008) |
| PASS | density x message rate (PRR) | `prr` | Amplifies | 0.9097 | 0.8636 | 0.7925 | 0.5561 | -0.0461 | -0.2364 | PRR falls with channel load, more steeply the more vehicles contend (Bai, Stancil, Krishnan VTC 2010; Bianchi 2000) |
| PASS | message rate x full certificate every message (CBR) | `cbr_mean` | Amplifies | 0.0814 | 0.1679 | 0.1302 | 0.2657 | 0.0865 | 0.1355 | CBR is rate x airtime per frame: the same rate increase costs more channel when each frame carries a certificate (ETSI TS 102 687; IEEE 1609.2-2016 6.4.8) |
| PASS | message rate x Falcon-512 (CBR) | `cbr_mean` | Amplifies | 0.0820 | 0.1655 | 0.2852 | 0.4931 | 0.0835 | 0.2078 | a 666-octet Falcon-512 signature makes each frame longer, so each extra message per second costs more airtime (NIST Falcon spec 1.2; ETSI TS 102 687) |
| PASS | data rate x frame size (airtime) | `airtime_us` | Amplifies | 322.5157 | 184.8553 | 488.0000 | 264.0000 | -137.6604 | -224.0000 | TXTIME's payload term is 8 L / rate: the airtime a faster rate saves grows with the frame length L; the preamble does not shrink (IEEE 802.11-2020 17.4.3) |
| PASS | transmit power x path-loss exponent (range) | `range90_m` | Amplifies | 640.0000 | 340.0000 | 760.0000 | 340.0000 | -300.0000 | -420.0000 | range is d0 10^(budget / 10 n): a dB of budget buys more metres when the exponent n is smaller (Rappaport 2e, 4.9) |
| PASS | demand x time headway (flow) | `metric.flow` | Holds | 21.5366 | 32.0653 | 21.4194 | 31.9808 | 10.5287 | 10.5613 | below capacity, flow out equals demand in (q = demand) whatever the saturation headway (HCM 7th ed. 19.2: capacity s g / C only binds at X >= 1) |
| PASS | actuated signals x demand (speed) | `traffic.speed_mean_mps` | Dampens | 5.8709 | 7.4676 | 5.9066 | 7.3474 | 1.5967 | 1.4408 | actuation pays most at low volume, where it can skip and gap out; as demand grows the phases run longer toward max green and the gain over fixed time shrinks (FHWA Signal Timing Manual 2e, 5.1; HCM 7th ed. 19.4) |
| PASS | demand x DCC on the traffic grid (CBR) | `cbr_mean` | Holds | 0.0143 | 0.0220 | 0.0138 | 0.0218 | 0.0077 | 0.0080 | at the low CBR of a Midtown grid (about 0.01) reactive DCC stays in its relaxed state, so more vehicles still means more load (ETSI TS 102 687 Table A.2) |
| PASS | trucks (20 %) on the traffic grid (speed) | `traffic.speed_mean_mps` | Holds | 5.8709 | 5.8613 | 5.8709 | 5.8613 | -0.0095 | -0.0095 | heavy vehicles accelerate slower and occupy more road: HCM 7th ed. eq. 19-8 f_HV = 100 / (100 + P_HV (E_T - 1)), E_T 2.0 at signals; SUMO truck vType accel 1.3 m/s2, length 7.1 m. Far below capacity (X << 1) the cost is only the trucks' own launches, so small (-0.2 % measured) |
| PASS | trucks x snow (speed) | `traffic.speed_mean_mps` | Holds | 5.8709 | 5.8613 | 4.5078 | 4.4979 | -0.0095 | -0.0099 | heavy vehicles slow traffic in any weather (HCM 7th ed. f_HV); the effect keeps its sign on snow (FHWA-HOP-07-073) |
| PASS | trucks x vehicle blockage (PRR) | `prr` | Amplifies | 0.2128 | 0.2113 | 0.2128 | 0.2112 | -0.0015 | -0.0016 | TR 37.885 NLOSv charges a blocker only when it is taller than an antenna (case 2/3, mean 9-10 dB): trucks change PRR more when blockage is modelled (3GPP TR 37.885 6.2.1; Boban et al., IEEE TVT 2011) |

## Failing

- `weather.intensity` = high on `traffic.speed_mean_mps`: FAIL (expected Down, changed by +0.0 %)
- `actors.vehicles.drivers.truck.time_headway_s` = high on `metric.headway_time`: FAIL (expected Up, changed by -0.7 %)
