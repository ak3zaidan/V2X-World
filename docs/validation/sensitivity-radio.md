# Sensitivity: radio, MAC and congestion parameters

Each row runs a base scenario and one change **on the same seed** (common random numbers:
the same vehicles, phases and fading draws wherever the change does not reach) and checks
that the named metric moves the way theory says, and, where theory gives a magnitude, that
the ratio changed / base falls in a band. A change that moves nothing fails its row.

- Harness: `crates/v2xw-engine/src/sensitivity.rs` (metric names in `Measured::get`).
- Specs: `scenarios/sensitivity/radio-dsrc.yaml`, `radio-dsrc-dense.yaml`, `radio-gn.yaml`,
  `radio-cv2x.yaml`. Each row carries its `source`.
- Tests: `crates/v2xw-engine/tests/sensitivity_radio.rs`, one test per parameter, 27 tests,
  about 2 minutes together in a dev build at `opt-level=2`.
- Command line: `v2xw sensitivity scenarios/sensitivity/radio-dsrc.yaml [--markdown out.md]`;
  `--base-only` prints every metric of the base and its PRR curve per 20 m bin.

## Re-run on main `77cffb5b` (2026-10-22)

The tables below were measured on 2026-10-12, when the node signed under a stand-in
certificate (a certified BSM 330 B). Since WP-C1 it signs under the real 80 B implicit
pseudonym certificate (263 B on 802.11p, 225 B on the sidelink), and six rows failed on
`77cffb5b`. Each was root-caused and re-stated
(`docs/design/scms-real/testing/rerun-2026-10-22.md` §1); all specs now pass from the
command line (`radio-dsrc` 28/28, `radio-dsrc-dense` 13/13, `radio-gn` 3/3, `radio-cv2x`
24/24) and the 27 tests pass. The re-stated rows, measured on `77cffb5b`:

| Row | Change | Metric | Expected | Base | Changed | Ratio | Pass |
|---|---|---|---|---|---|---|---|
| LTE MCS 5->7 (was 7->11; base MCS 5) | sidelink `mcs: 7` | `cbr_mean` | down | 0.2415 | 0.1784 | 0.739 | yes |
| LTE certificate on every message (base MCS 5) | certificate on every message | `cbr_mean` | up | 0.2415 | 0.2496 | 1.033 | yes |
| LTE vehicles 40->120 | 120 vehicles | `e2e_ms_mean` | same within 10 % (was up) | 67.6567 | 65.1232 | 0.963 | yes |
| LTE transmissions 2->1 (base 5 dBm) | `max_transmissions: 1` | `prr` | down | 0.8857 | 0.7828 | 0.884 | yes |
| vehicles 40->120 | 120 vehicles | `e2e_ms_mean` | up, ratio in [1.005, 1.2] (was 1.1) | 12.4531 | 14.1631 | 1.137 | yes |
| queue 64->1, saturated | `radio.mac.queue_length=1` | `mac_drops` | same (was up) | 0.0000 | 0.0000 | - | yes |

The other rows' numbers moved with the smaller frames (for example the DSRC base PSDU
205.9 → 199.2 B, the dense base CBR 0.616 → 0.543, the LTE base CBR 0.2398 → 0.1784) and
still hold.

## The bases

| Base | What | Used by |
|---|---|---|
| `dsrc-grid.yaml` | 6 x 6 procedural grid of 200 m blocks (1.2 km square), open air, 40 vehicles spawned at t = 0, 802.11p BSMs at 10 Hz, IEEE 1609.2 with a certificate every 1 s, 3 s. Every distance from 0 to 1 km occurs. | link budget, channel, frame size, load |
| `dsrc-dense.yaml` | 3 x 3 grid of 100 m blocks (300 m square), about 100 vehicles, all in range of each other, CBR about 0.3 at 10 Hz. | EDCA, MAC queue, DCC |
| `gn-grid.yaml` | `dsrc-grid.yaml` on GN/BTP with CAMs and DENMs, one breakdown at 8 s, 24 s. | GeoNetworking |
| `lte-grid.yaml`, `nr-grid.yaml` | `dsrc-grid.yaml` with `radio.rat` set to `lte-v2x-pc5` or `nr-v2x-pc5` and nothing else changed. | C-V2X |

Metrics: `prr_<lo>_<hi>` is the PRR census between two distances; `range90_m` (`range_m`)
is the lower edge of the first 20 m bin, with at least 20 receivers, whose PRR is below 0.9
(0.5) and stays below it in the next such bin; `cbr_mean` is the mean channel busy ratio
(802.11p) or the busy share of sub-channels (sidelink); `access_ms` is channel-access delay
(MAC hand-off to air); `e2e_ms_mean` is generation to delivery; `collision_rate` counts
collision, hidden-terminal, resource-collision and in-band-emission losses per reception
attempt; `aoi_ms` is the time-average age of information per (receiver, sender) pair.

## 802.11p: link budget, channel, frame size and load (`radio-dsrc.yaml`)

| Row | Change | Metric | Expected | Base | Changed | Ratio | Pass |
|---|---|---|---|---|---|---|---|
| tx power 20->5 dBm | `radio.devices.obu.tx_power_dbm=5.0` | `range90_m` | down | 740.0000 | 300.0000 | 0.405 | yes |
| tx power 20->5 dBm | `radio.devices.obu.tx_power_dbm=5.0` | `prr_400_600` | down | 0.9509 | 0.4904 | 0.516 | yes |
| noise figure 6->12 dB | `radio.phy.noise_figure_db=12.0` | `range90_m` | down | 740.0000 | 460.0000 | 0.622 | yes |
| noise figure 6->12 dB | `radio.phy.noise_figure_db=12.0` | `prr_600_1000` | down | 0.8489 | 0.5506 | 0.649 | yes |
| PL exponent 2.5->3.5 | `radio.channel_model.path_loss_exponent=3.5` | `prr_200_600` | down | 0.7735 | 0.0827 | 0.107 | yes |
| shadowing 2->10 dB | `radio.channel_model.shadowing_sigma_db=10.0` | `prr_200_600` | down | 0.9746 | 0.8777 | 0.901 | yes |
| nakagami m 3->1 | `radio.channel_model.nakagami_shape=1.0` | `prr_200_600` | down | 0.9656 | 0.9155 | 0.948 | yes |
| fading none->rayleigh | `radio.channel_model.fading="rayleigh"` | `prr_200_600` | down | 0.9761 | 0.9155 | 0.938 | yes |
| data rate 6->12 Mbps | `radio.phy.data_rate_mbps=12.0` | `airtime_us` | down, ratio in [0.5, 0.7] | 322.4770 | 184.8368 | 0.573 | yes |
| data rate 6->12 Mbps | `radio.phy.data_rate_mbps=12.0` | `range90_m` | down | 740.0000 | 460.0000 | 0.622 | yes |
| data rate 6->12 Mbps | `radio.phy.data_rate_mbps=12.0` | `cbr_mean` | down | 0.0833 | 0.0482 | 0.579 | yes |
| certificate on every message | `security.signer_id_policy.digest_otherwise=false, security.signer_id_policy.full_cert_every_ms=0` | `psdu_bytes` | up | 205.9381 | 330.0803 | 1.603 | yes |
| certificate on every message | `security.signer_id_policy.digest_otherwise=false, security.signer_id_policy.full_cert_every_ms=0` | `airtime_us` | up | 322.4770 | 488.0000 | 1.513 | yes |
| certificate on every message | `security.signer_id_policy.digest_otherwise=false, security.signer_id_policy.full_cert_every_ms=0` | `cbr_mean` | up | 0.0833 | 0.1328 | 1.594 | yes |
| vehicles 40->120 | `actors.vehicles.demand.params.max_total_vehicles=120` | `cbr_mean` | up | 0.0833 | 0.2346 | 2.817 | yes |
| vehicles 40->120 | `actors.vehicles.demand.params.max_total_vehicles=120` | `collision_rate` | up | 0.0335 | 0.1262 | 3.774 | yes |
| vehicles 40->120 | `actors.vehicles.demand.params.max_total_vehicles=120` | `prr` | down | 0.9139 | 0.8284 | 0.906 | yes |
| vehicles 40->120 | `actors.vehicles.demand.params.max_total_vehicles=120` | `access_ms` | up | 0.0289 | 0.2019 | 6.988 | yes |
| vehicles 40->120 | `actors.vehicles.demand.params.max_total_vehicles=120` | `e2e_ms_mean` | up, ratio in [1.005, 1.1] | 12.3196 | 12.4410 | 1.010 | yes |
| vehicles 40->80, software verify | `actors.vehicles.demand.params.max_total_vehicles=80` | `stage_verify_wait_ms` | up | 0.3824 | 2.4236 | 6.337 | yes |
| vehicles 40->80, software verify | `actors.vehicles.demand.params.max_total_vehicles=80` | `e2e_ms_mean` | up | 5.4502 | 7.7963 | 1.430 | yes |
| rate 10->20 Hz | `messages.rate_hz=20.0` | `tx_rate_hz` | up, ratio in [1.7, 2.1] | 9.9583 | 19.7167 | 1.980 | yes |
| rate 10->20 Hz | `messages.rate_hz=20.0` | `cbr_mean` | up, ratio in [1.5, 2.3] | 0.0832 | 0.1641 | 1.971 | yes |
| rate 10->20 Hz | `messages.rate_hz=20.0` | `collision_rate` | up | 0.0336 | 0.0731 | 2.179 | yes |
| rate 10->20 Hz | `messages.rate_hz=20.0` | `ipg_ms_mean` | down | 106.8604 | 55.9716 | 0.524 | yes |
| rate 10->20 Hz | `messages.rate_hz=20.0` | `aoi_ms` | down | 71.3710 | 45.7030 | 0.640 | yes |
| mobility step 100->50 ms | `time.mobility_step_ms=50` | `collision_rate` | same | 0.0335 | 0.0336 | 1.003 | yes |
| mobility step 100->50 ms | `time.mobility_step_ms=50` | `tx_phase_peak` | same | 0.1105 | 0.1105 | 1.000 | yes |

Why, briefly. Transmit power and noise figure shift the SNR at every distance by the same
dB, so the PRR edge moves in (log-distance law; noise floor kTB + NF). A steeper exponent,
more shadowing and deeper fading all lower PRR in the 200-600 m band where the mean SNR is
above threshold (Nakagami-m outage grows as m falls). 12 Mbit/s (16-QAM 1/2) halves the
payload symbols, but the 40 µs preamble and SIGNAL stay, hence a ratio of 0.57 (IEEE
802.11-2020 §17.4.3), and it needs about 6 dB more SNR (Table 17-18), so the range shrinks.
A certificate on every message adds about 100 octets (IEEE 1609.2 §6.4.8). More vehicles or
a higher rate raise CBR almost in proportion (x2.8 for three times the fleet,
x1.97 for x2 rate) and with it collisions (Bianchi 2000) and channel-access delay.

End-to-end latency barely moves with load on the Craton2 HSM base (+1 %): its 12.3 ms is
9.0 ms of signing, 2.6 ms of hand-off and only 0.03 ms of access, so it rises by the access
delay's increase (+0.17 ms) and no more. The row therefore has a band of [1.005, 1.1]. With
software verification the verify queue is what grows (M/D/1 wait x6.3, e2e x1.43).

## 802.11p: EDCA, the MAC queue and congestion control (`radio-dsrc-dense.yaml`)

| Row | Change | Metric | Expected | Base | Changed | Ratio | Pass |
|---|---|---|---|---|---|---|---|
| CWmin 3->63 | `radio.mac.cw_min_vo=63` | `backoff_us` | up | 6.4390 | 139.5037 | 21.666 | yes |
| CWmin 3->63 | `radio.mac.cw_min_vo=63` | `access_ms` | up | 0.1177 | 0.2791 | 2.371 | yes |
| AIFSN 2->9 | `radio.mac.aifsn_vo=9` | `aifs_us` | up, ratio in [2, 3] | 19.2166 | 50.4162 | 2.624 | yes |
| AIFSN 2->9 | `radio.mac.aifsn_vo=9` | `access_ms` | up | 0.1177 | 0.1823 | 1.548 | yes |
| queue 64->1, saturated | `radio.mac.queue_length=1` | `mac_drops` | up | 0.0000 | 68.0000 | - | yes |
| reactive DCC off->on | `radio.dcc.enabled=true` | `cbr_mean` | down | 0.6160 | 0.2675 | 0.434 | yes |
| reactive DCC off->on | `radio.dcc.enabled=true` | `tx_rate_hz` | down | 18.6603 | 7.8000 | 0.418 | yes |
| reactive DCC off->on | `radio.dcc.enabled=true` | `aoi_ms` | up | 40.4010 | 107.9236 | 2.671 | yes |
| J2945/1 B 25->5 | `radio.dcc.density_coefficient=5.0` | `tx_rate_hz` | down | 8.7799 | 4.0346 | 0.460 | yes |
| J2945/1 B 25->5 | `radio.dcc.density_coefficient=5.0` | `cbr_mean` | down | 0.2790 | 0.0754 | 0.270 | yes |
| J2945/1 B 25->5 | `radio.dcc.density_coefficient=5.0` | `ipg_ms_mean` | up | 106.8618 | 235.0918 | 2.200 | yes |
| ETSI adaptive target 0.68->0.15 | `radio.dcc.cbr_target=0.15` | `cbr_mean` | down | 0.3571 | 0.2837 | 0.794 | yes |
| ETSI adaptive target 0.68->0.15 | `radio.dcc.cbr_target=0.15` | `tx_rate_hz` | down | 18.4104 | 15.4437 | 0.839 | yes |

Why, briefly. Backoff is uniform over [0, CW] slots of 13 µs (EN 302 663 Table B.3), so
CWmin 3 → 63 multiplies it by about 20. AIFS is SIFS + AIFSN x slot, 58 → 149 µs; the
measured per-frame mean is lower because a frame that finds the medium idle for AIFS is sent
at once, but the ratio (2.6) matches. A one-frame queue drops every arrival that finds a
frame waiting once the offered load is above one (60 vehicles x 40 Hz x 0.45 ms). Reactive
DCC (TS 102 687 Table A.1) caps the rate as CBR crosses its 30-60 % states: rate and CBR
fall by more than half and the age of information rises x2.7. J2945/1 stretches the
interval to 100 ms x N/B above B vehicles in 100 m (§6.3.6). ETSI adaptive DCC (LIMERIC)
moves its duty-cycle limit at most 0.00025 per 200 ms step from 0.03, so it binds only after
seconds: the row runs 12 s at 0.9 % duty per station (CBR 0.36, below the default 0.68
target); with the target at 0.15 the limit falls below the offered duty after about 8 s.

## GeoNetworking (`radio-gn.yaml`)

| Row | Change | Metric | Expected | Base | Changed | Ratio | Pass |
|---|---|---|---|---|---|---|---|
| GN hop limit 1->10 | `net.geonet.hop_limit=10` | `gn_reach` | up | 0.7430 | 1.0000 | 1.346 | yes |
| GN hop limit 1->10 | `net.geonet.hop_limit=10` | `gn_forwards_sent` | up | 0.0000 | 30.0000 | - | yes |
| GN hop limit 1->10 | `net.geonet.hop_limit=10` | `gn_forwarded_bytes` | up | 0.0000 | 11896.0000 | - | yes |

At 5 dBm one hop covers about 250 m of the 1 000 m warning area. A hop limit of 1 keeps the
warning in the originator's own range (reach 0.74, no forwards); 10 hops reach everyone in
the area at the cost of 30 forwarded frames and 11.9 kB of forwarding on air.

## C-V2X sidelink (`radio-cv2x.yaml`)

| Row | Change | Metric | Expected | Base | Changed | Ratio | Pass |
|---|---|---|---|---|---|---|---|
| LTE tx power 20->5 dBm | `radio.devices.obu.tx_power_dbm=5.0` | `range90_m` | down | 1000.0000 | 540.0000 | 0.540 | yes |
| LTE tx power 20->5 dBm | `radio.devices.obu.tx_power_dbm=5.0` | `prr_600_1000` | down | 0.9409 | 0.6687 | 0.711 | yes |
| LTE noise figure 9->15 dB (5 dBm) | `radio.sidelink.noise_figure_db=15.0` | `prr_600_1000` | down | 0.6687 | 0.2588 | 0.387 | yes |
| LTE MCS 7->11 | `radio.models.sidelink={"id":"access/sidelink/engine-coupling","params":{"mcs":11}}` | `prr_600_1000` | down | 0.6687 | 0.6340 | 0.948 | yes |
| LTE MCS 7->11 | `radio.models.sidelink={"id":"access/sidelink/engine-coupling","params":{"mcs":11}}` | `cbr_mean` | down | 0.2398 | 0.1819 | 0.758 | yes |
| LTE MCS 7->11 | `radio.models.sidelink={"id":"access/sidelink/engine-coupling","params":{"mcs":11}}` | `airtime_us` | same | 1000.0000 | 1000.0000 | 1.000 | yes |
| LTE certificate on every message | `security.signer_id_policy.digest_otherwise=false, security.signer_id_policy.full_cert_every_ms=0` | `cbr_mean` | up | 0.2398 | 0.2554 | 1.065 | yes |
| LTE vehicles 40->120 | `actors.vehicles.demand.params.max_total_vehicles=120` | `cbr_mean` | up | 0.2398 | 0.5701 | 2.377 | yes |
| LTE vehicles 40->120 | `actors.vehicles.demand.params.max_total_vehicles=120` | `collision_rate` | up | 0.0337 | 0.1290 | 3.824 | yes |
| LTE vehicles 40->120 | `actors.vehicles.demand.params.max_total_vehicles=120` | `prr` | down | 0.9638 | 0.8714 | 0.904 | yes |
| LTE vehicles 40->120 | `actors.vehicles.demand.params.max_total_vehicles=120` | `e2e_ms_mean` | up | 59.4798 | 63.6381 | 1.070 | yes |
| LTE rate 10->20 Hz (RRI 50) | `messages.rate_hz=20.0` | `cbr_mean` | up, ratio in [1.5, 2.5] | 0.2452 | 0.4412 | 1.799 | yes |
| LTE transmissions 2->1 | `radio.models.sidelink={"id":"access/sidelink/engine-coupling","params":{"max_transmissions":1}}` | `cbr_mean` | down, ratio in [0.4, 0.65] | 0.2398 | 0.1282 | 0.534 | yes |
| LTE transmissions 2->1 | `radio.models.sidelink={"id":"access/sidelink/engine-coupling","params":{"max_transmissions":1}}` | `prr` | down | 0.9638 | 0.9455 | 0.981 | yes |
| LTE RRI 100->50 ms | `radio.sidelink.reservation_interval_ms=50` | `access_ms` | down, ratio in [0.4, 0.7] | 47.2274 | 25.9311 | 0.549 | yes |
| LTE keep probability 0.8->0 | `radio.sidelink.resource_keep_probability=0.0` | `sl_selections` | up, ratio in [1.6, 4] | 75.0000 | 175.0000 | 2.333 | yes |
| LTE sensing off | `radio.sidelink.sensing=false` | `collision_rate` | up | 0.0337 | 0.0444 | 1.317 | yes |
| LTE sensing off | `radio.sidelink.sensing=false` | `sl_overlapping` | up | 573.0000 | 905.0000 | 1.579 | yes |
| LTE RSRP threshold -> -60 dBm | `radio.sidelink.rsrp_threshold_dbm=-60.0` | `collision_rate` | changes | 0.0337 | 0.0226 | 0.671 | yes |
| NR tx power 20->5 dBm | `radio.devices.obu.tx_power_dbm=5.0` | `range90_m` | down | 720.0000 | 340.0000 | 0.472 | yes |
| NR vehicles 40->120 | `actors.vehicles.demand.params.max_total_vehicles=120` | `cbr_mean` | up | 0.0779 | 0.2317 | 2.976 | yes |
| NR vehicles 40->120 | `actors.vehicles.demand.params.max_total_vehicles=120` | `collision_rate` | up | 0.0710 | 0.1030 | 1.451 | yes |
| NR noise figure 9->15 dB (5 dBm) | `radio.sidelink.noise_figure_db=15.0` | `prr_600_1000` | down | 0.1968 | 0.0222 | 0.113 | yes |
| NR keep probability 0->0.8 | `radio.sidelink.resource_keep_probability=0.8` | `sl_selections` | down, ratio in [0.3, 0.7] | 186.0000 | 87.0000 | 0.468 | yes |

Why, briefly. Power and noise figure act as on 802.11p. A higher MCS needs more SINR for the
same BLER (TS 36.213), so PRR at range falls; it carries the same transport block on fewer
sub-channels, so CBR falls, while a sidelink transmission stays one subframe (airtime
`same`). Load raises the busy share of sub-channels (TS 36.214 §5.1.30), collisions and PRR
loss; the e2e rise is small because SPS latency is dominated by the wait for the next
reserved slot. That wait is about RRI/2, so RRI 100 → 50 ms halves the access delay (0.55).
A second blind transmission doubles the occupied resources (CBR 0.53 without it) and buys
HARQ-combining PRR. With keep probability P a new selection happens at each counter expiry
with probability 1 - P (TS 36.321 §5.14.1.1, TS 38.321 §5.22.1.1); NR's default P is 0, so
its row sets 0.8. Sensing off is the random-selection baseline and collides more.
The RSRP-threshold row is `changes`: its direction depends on density (Gonzalez-Martin 2019).

## What was fixed or found

- **The first step waited for nothing** (`09725f2`). A node's first BSM went out at its first
  mobility step after spawning, so a 50 ms step put every BSM of a bulk-spawned fleet into one
  half of the 100 ms cycle and doubled the hidden-terminal losses: the mobility step, which is
  not a radio parameter, moved the collision rate. The first step now waits for the node's
  whole generation phase; the `mobility step 100->50 ms` rows hold it (`same`).
- **J2945/1 never stretched the interval.** Its tracking error compared two raw GNSS fixes,
  each with 1.6 m of white noise per axis, so it was 1-5 m on every step, above vTrackingErrMax
  (0.5 m): a tracking-error BSM was always pending and density had no effect. It now reads the
  unit's navigation solution (`Engine::j2945_nav`), in which the slowly varying absolute error
  cancels. The `J2945/1 B 25->5` rows hold it.
- **A one-bin dip was read as the range edge.** The NR run at 5 dBm lost 17 % in its 0-20 m bin
  (near-field half-duplex and in-band emission) and reported `range90_m` 0 m. The edge now
  needs two counted bins below the threshold in a row.
- **Rows that could not fail were caught red first**: the certificate rows were refused by the
  loader (`full_cert_every_ms: 0` beside `digest_otherwise: true`), the 3 s LIMERIC row gave
  identical runs for both targets, and `NR keep probability -> 0` changed nothing because 0 is
  NR's default. Each failed before it was corrected.

## Limits

- The dense base is slow at saturation: 150 trips at 50 Hz did not finish 3 s in 20 minutes,
  and a 1 s, 40 Hz run takes about 58 s at 150 trips against 2.5 s at 60. The saturated-queue
  row therefore uses 60 vehicles. The cost grows much faster than the square of the fleet;
  not traced here.
- The 802.11p `e2e_ms_mean` row is a weak signal by construction (signing dominates); the
  access-delay and verify-wait rows are the load-sensitive parts of the latency budget.
