# Networking and messages: do the settings move the metrics? (2026-10-13)

Track: GeoNetworking forwarding, DENM repetition, access categories, 1609.4 channel
access, BSM/CAM rates and triggers, path history, message sets, codec tier.

How to rerun (debug binary of this branch; the release binary on main predates the fixes):

- `v2xw sensitivity scenarios/sensitivity/test-networking.yaml` (gn-grid: 40 vehicles,
  802.11p, GN/BTP, CAM + a breakdown DENM; multi-hop rows at 5 dBm, where one hop no longer
  covers the 1 000 m area: base `max_hops` 1 at default power, 2 at 5 dBm)
- `v2xw sensitivity scenarios/sensitivity/test-networking-dsrc.yaml` (dsrc-grid, BSMs)
- `v2xw sensitivity --expect --expectations crates/v2xw-engine/tests/expectations/networking.yaml`

Run cost is read from `report.events_by_class.PhyEnd` (one event per frame; deterministic,
unlike wall time).

## Results

| Setting | Change | Metric | Expected | Measured | Pass |
|---|---|---|---|---|---|
| `net.geonet.hop_limit` (5 dBm) | 10 -> 1 | gn_reach | down | 1.000 -> 0.743 | yes |
| | 10 -> 1 | gn_forwards_sent | down | 30 -> 0 | yes |
| | 10 -> 1 | forwarded octets | down | 11 896 -> 0 | yes |
| | 10 -> 1 / 2 | gn_max_hops | down | 2 -> 0 / 2 -> 1 | yes |
| `net.geonet.hop_limit` (default power) | low | forwards_sent | down | 9 -> 0 | yes |
| `net.geonet.area_forwarding` | simple | forward air time | up | 17.3 -> 67.4 ms | yes |
| | simple | DENM latency p95 | down | 67.2 -> 16.8 ms | yes (p50 is a direct receiver and cannot move: first run used p50) |
| `net.geonet.cbf_variant` | etsi -> dpd | forwards_sent | down | 30 -> 20 | yes |
| `net.geonet.cbf_max_time_ms` | 100 -> 20 | latency p95 | down | 67.2 -> 26.4 ms | yes |
| `net.geonet.cbf_min_time_ms` | 1 -> 50 | latency p95 | up | 67.2 -> 75.0 ms | yes (p50 unchanged, as above) |
| `net.geonet.max_comm_range_m` | 1000 -> 300 | CBF cancellations | down | 87 -> 54 | yes |
| `net.geonet.area_radius_m` | 1000 -> 300 | vehicles in area | down | 39 -> 5 | yes |
| | 1000 -> 300 | forwards | down | 9 -> 3 | yes |
| | high (1784) | vehicles in area | up | 39 -> 39 | expectation corrected: 1 000 m already holds all 39 |
| `net.geonet.repetition_interval_ms` | 1000 -> 0 | GBC originated | down | 3 -> 3 (main), 3 -> 2 (fixed) | **dead, fixed** |
| | 1000 -> 200 | DENM air time | up | 6.9 -> 26.4 ms | yes (fixed) |
| | 1000 -> 200 | PhyEnd events (cost) | up | 6603 -> 6637 | yes at 0.2 % band (34 extra frames) |
| `net.geonet.repetition_duration_ms` | 5 s -> 0.5 s | DENM air time | down | 6.94 -> 5.16 ms | yes (fixed). The DENM lives about 3 s here (raised ~19.5 s, cancelled 23 s), so 2 s moved nothing; `gbc_originated` counts new packets, not repeats |
| `net.geonet.keep_alive_forwarding` | on (overlay) | kept_alive | up | 0 -> 22 | yes; **on-air now refused** |
| `net.geonet.forwarding` | overlay | forwarded octets | down | 11 896 -> 0 | yes |
| `net.access_categories` | etsi-profiles | mean AIFS | up | 4.5 -> 8.6 us | yes |
| `net.channel_access` | alternating | e2e latency | up | 12.3 -> 29.3 ms | yes |
| | alternating | collision rate | up | 0.034 -> 0.395 | yes |
| `messages.sets` | + cpm | CBR | up | 0.074 -> 0.101 | yes |
| | + cpm | payload octets | up | 316 k -> 513 k | yes |
| | + cpm | frames | up | 6603 -> 9255 (CPM air time 0 -> 1.18 s) | yes |
| | + cpm on WSMP | - | - | refused by the loader (CPM needs gn-btp) | correct |
| `messages.path_history_points` | 15 -> 0 | BSM payload | down | 67 019 -> 55 069 | yes |
| | 15 -> 1 (20 s) | mean BSM size | down | 211 -> 206 | yes |
| | 15 -> 23 | mean BSM size | up | 205 -> 205 | expectation corrected: concise history stops at 300 m |
| `messages.rate_hz` | 10 -> 20 | BSM frames | up x2 | 1195 -> 2388 | yes |
| | 10 -> 20 | CBR | up | 0.083 -> 0.166 | yes |
| | 10 -> 20 | PhyEnd events (cost) | up | 1195 -> 2388 | yes |
| `messages.generator` (BSM) | ITT 100 -> 300 ms | BSM frames | down | 1195 -> 400 | yes |
| `messages.codec_tier` | size-model | PSDU mean | same | 205.96 -> 205.96 | yes (only SRM/SSM lack an encoder) |

| `messages.sets` | + cpm | PhyEnd events (cost) | up | 6603 -> 9255 | yes |
| `messages.path_history_points` (CAM) | 15 -> 0 | mean CAM size | down | 242 -> 238 | yes |
| | 15 -> 23 | mean CAM size | same | 242 -> 242 | yes |
| `messages.generator` (CAM) | T_GenCamMax 1000 -> 200 ms | CAM frames | up | 6591 -> 7443 | yes |
| | thresholds x10 | CAM frames | down | 6591 -> 1027 | yes |
| | T_GenCamMin 100 -> 500 ms | CAM frames | down | 6591 -> 1901 | yes |

## What was fixed

1. **DENM repetition did nothing on the air** (the default forwarding). The on-air
   breakdown DENM came from the node's DEN service with a fixed 1 s / 15 s repetition
   (`v2xw-node/src/events.rs`); only the overlay read `net.geonet.repetition_*`. The engine
   now hands both to the originator at the breakdown (`run/gn_air.rs`
   `on_air_repetition`). The schema default duration (5 000 ms, the overlay's) keeps the
   on-air 15 s so existing runs do not change; any other value is used as given. Caveat:
   an explicit 5 000 ms on the air also reads as 15 s.
2. **Keep-alive forwarding on the air had no model**: the loader now refuses
   `keep_alive_forwarding: true` with `forwarding: on-air`.
3. **No per-message-type ledger**: `RunReport.air_by_type` gives frames, octets, mean
   size and air time per message type (each type's share of the CBR), and spec rows can
   now name any `report.*` field (`sensitivity.rs`).

## Not exposed / not tested

Part II content choice, unicast retry limit (fixed at 0 in the overlay; the on-air router
has no unicast), VAM interval, and the certificate attach policy (security track).
SPaT/MAP intervals need an RSU (`tests/network_audit_config.rs`). `net.dcc_cbr` needs a
congested DCC run; not measured.
