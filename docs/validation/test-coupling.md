# Cross-domain coupling and statistical robustness (round 15 test track)

Release binary from main `e94e43c`. Rows: `crates/v2xw-engine/tests/expectations/interactions/coupling.yaml`
(2 x 2 checks on the catalogue families; a *robustness* row puts a second master seed
`0x5EED_0002` as the condition and expects the factor's effect to **hold** its sign).
Run: `v2xw sensitivity --expect --expectations crates/v2xw-engine/tests/expectations/coupling.yaml --interactions crates/v2xw-engine/tests/expectations/interactions/coupling.yaml` (702 s, 18 runs).

## Measured

| Setting | Change | Metric | Expected | Measured (alone / under condition) | Pass |
|---|---|---|---|---|---|
| vehicles (dsrc, 10 Hz) | 40 -> 160 | cbr_mean | up, holds across seeds | +0.233 / +0.239 | yes |
| vehicles | 40 -> 160 | prr | down, holds | -0.113 / -0.120 | yes |
| vehicles | 40 -> 160 | collision_rate | up, holds | +0.123 / +0.130 | yes |
| vehicles | 40 -> 160 | access_ms | up, holds | +0.39 / +0.27 ms | yes |
| vehicles | 40 -> 160 | aoi_ms | up, holds | +10.9 / +13.5 ms | yes |
| demand (city grid) | 1500 -> 3000 veh/h | cbr_mean | up, holds | +0.016 / +0.014 | yes |
| saturated fleet (city grid) | Poisson -> 250 at once | cbr_max | up, holds (queues at junctions) | +0.949 / +0.949 | yes |
| speed limit (NR) | 13.9 -> 33.3 m/s | prr | down (Doppler) | 0.0 / 0.0 | no: dropped, see below |
| security.signature x vehicles 160 | falcon-512 | prr, cbr_mean, aoi_ms | amplifies | refused: default OBU prices no Falcon | fixed row, not re-run |
| tx power (city grid) | 0 dBm | aoi_ms | up, holds | -18.7 % / +225 ms | no: dropped, see below |
| tx power (city grid) | 0 dBm | app_miss_ratio / app_warnings | up / down | 0 / -1 warning | no: dropped |

City choice (phase1-manhattan at 1500 veh/h, 120 s, `--city`), against Billings:

| City | delivery_ratio | PDR | channel load | AoI | mean speed |
|---|---|---|---|---|---|
| Billings | 0.240 | 0.494 | 0.0173 | 62.8 ms | 5.93 m/s |
| Manhattan | 0.120 (-50 %) | 0.424 | 0.0090 | 69.1 ms | 5.43 |
| Portland | 0.103 (-57 %) | 0.323 | 0.0071 | 81.7 ms | 3.59 |
| Ann Arbor | 0.179 (-25 %) | 0.422 | 0.0094 | 67.3 ms | 5.88 |

Plausible: high-rise Manhattan and Portland's 200 ft blocks shadow more links (lower
delivery, lower *sensed* channel load despite Manhattan's +38 % offered load), Portland's
dense signals give the lowest speed. All four are downtowns, so this is not a true
dense-vs-suburban contrast; the fleet is light (density about 0.2 veh/km).

## Findings and fixes

- **Fixed, `v2xw compare` noise test** (`crates/v2xw-server/src/compare.rs`): between two
  sets of replications it called a difference real when it exceeded 2 x the combined SE.
  With 5 + 5 replications each SE has 4 df, so the false-alarm rate per metric is about
  8 %, not 5 %. It now uses t(0.975) at the Welch-Satterthwaite df (2.306 for 5 + 5; Law,
  Simulation Modeling and Analysis 5e, 10.2.1). Single runs keep the batch-means 2.
  Test `replications_use_the_welch_t_threshold`.
- **Speed -> network is not coupled on city or procedural worlds**: NR mode-2 BLER is
  indexed by relative speed only in the highway environment (`bler_nr.rs speed_bucket_kmh`:
  urban is always 0 km/h, the only urban speed the source prints), and the 802.11p PER
  model has no Doppler (documented limitation in `per.rs`/`fading.rs`). The NR row moved
  exactly 0. `world.source.params.speed_limit_mps` is also not a published setting, so the
  single rows were unplanned. Open: needs a cited urban speed curve or a Doppler term.
- **AoI under low power falls on the city grid**: `aoi_ms` averages only over pairs that
  still exchange messages, so cutting range leaves the close (fresh) pairs: survivor bias.
  The other seed rose. The expectation was wrong for this metric definition, so the row
  was dropped. A per-neighbour-in-range AoI (awareness `nar`) is the better metric.
- **Network -> applications**: the traffic family produces almost no conflict episodes
  (miss ratio 0, one warning), so the application coupling could not be measured there.
  `connected-intersections` would be needed as a family (not in catalogue.yaml).
- **Falcon x density**: the default OBU card prices no Falcon; rows now set
  `nodes.default_obu: obu/generic-automotive-soc-no-hsm` (as interactions.yaml does). Not re-run.

## Not run

Replications (10 seeds x 3 scenarios plus 5 seeds of a real change) and the 1 vs 4
thread determinism digests: the script is ready (`seeds.sh` in the agent scratchpad) but
it never got the machine-wide heavy lock inside the 90-minute window.
