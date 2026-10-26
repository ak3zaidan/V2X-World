# Live scenario audit, 2026-10-25

Every scenario file in `scenarios/` should boot, run a short window, write its records and give
sane metrics. Each city or world then gets one base run, judged against published or physical
bounds. The rows are in `crates/v2xw-engine/tests/expectations/live-scenarios.yaml`. They were
written before the runs, and any later correction is marked in the row. The checker is
`tools/audit/live_scenarios.py`. It reads only `run-report.json`, `metrics.json` and the run's
stderr, and it shares no code with the engine.

- **Binary:** release `v2xw` built from `486c8cba`, run as a copy.
- **Run flags:** `--record-level events --build-utc 2026-01-01T00:00:00Z`, with each
  scenario's own seed. Every run went through its own run-lock call.
- **Windows:** the world base runs used 60 s. To fit the shared run slots, the bulk pass used
  10 s, or the scenario's own length when shorter. When the loader refused a window that would
  cut off a scheduled event ("lengthen the run past N s"), the run was retried once at N + 5 s.
- **Coverage:** the run slots were shared with about ten other agents, so only part of the
  catalogue ran in the three hours: 115 runs (see the last table). Nothing was skipped on
  purpose. The rest validate, and the same checker judges them once they have run:
  `python3 -I tools/audit/live_scenarios.py check <table> <runs-dir>`.

**Selftest:** `live_scenarios.py selftest` turns every row with a run red under its injected
fault: a non-zero exit, a panic line, an unverified recording, a NaN, successes over trials, a
negative latency, a speed of 80 m/s, flow ×2, no actors, no receptions, the SCMS, threat or
pedestrian records dropped, no VRU devices, a changed digest, a metric ×10, or the farthest
PDR bin set to 1.

## Verdicts

| Change / row | Predicted (source) | Measured | Verdict |
|---|---|---|---|
| Every run: exit, verified recording, no panic | exit 0 (run.rs) | every run that started exited 0 with a verified recording; no panic | open (refusals and timeouts, findings 6 and 10) |
| Every run: finite, self-consistent samples | proportion = s/t inside its interval; quantiles ordered; sums match the point | all hold once the D9 1e-3 sum grid is allowed for | open: PSM cert octets over the envelope under PQ hybrid (finding 1); otherwise holds once the D9 sum grid is allowed for (finding 7) |
| Every run: latency > 0; speed 0-45 m/s; acceleration -10 to 5 m/s² | causality; tyre friction (Wong) | all hold | pass |
| Every run: Edie q = 3.6 k v; q ≤ 2,400 veh/h/ln; k ≤ 150 veh/km/ln | Edie 1963; HCM | exact in every window | pass |
| Demand: actors > 0 and ≤ cap | Poisson | base/midtown: 52 at a Poisson mean of 50 | pass |
| Radio: frames sent > 0 and received ok > 0 | — | all except phase1-grid: 2 cars, 470 attempts, all below sensitivity | wrong-expectation (exempt with evidence) |
| SCMS records when the SCMS protocol is on | — | pki.flow and backend.state on every SCMS run | pass |
| Threat records when attackers are set | — | none in credential-lifecycle or ccms-lifecycle, where 5 % of 19 cars drew 0 attackers (P = 0.38) | wrong-expectation (row made aware of the window and the draw) |
| VRU devices = device_fraction × walkers | binomial, 4σ | 189 of 708, and 432 of 1,713, at f = 0.25 | pass |
| Determinism, same seed | byte-identical digest | midtown-normal and phase1-grid identical | pass |
| Midtown mean speed | 2-9 m/s (NYC DOT: 4.7 mph; 25 mph limit) | 4.73 m/s; max 12.4 | pass |
| Midtown headway | 1-6 s | no window reached 30 samples; base/midtown's one summary: 3.3 s | open |
| Midtown end-to-end latency | 0.5-100 ms (J2945/1) | mean 12.3 ms, p95 14.5 | pass |
| Midtown PDR against distance, monotone within 0.05 | log-distance loss plus shadowing | 1.00 to 60 m; 0.5 at about 160 m; rises from 0.26 to 0.37 at 260-280 m | wrong-expectation: one avenue spacing (about 274 m) restores cross-street line of sight. The corrected envelope row (0.15) passes on 4 runs |
| Rain / clear mean speed | 0.80-0.99 (HCM: heavy rain cuts free-flow speed 6-14 %) | 0.847 | pass |
| Rain / clear PDR | 0.95-1.05 (ITU-R P.838-3) | 0.998 | pass |
| Portland mean speed; vmax | 2-10 m/s; ≤ 17 m/s (20 mph downtown) | 3.45 m/s; 9.0 | pass |
| Portland PDR against distance | monotone within 0.05 | 19 cars give correlated bins (0.69 at 80 m, 0.85 at 100 m) | wrong-expectation (fleet too small to judge) |
| Highway, highway-congestion | 8-34 m/s; line-of-sight PDR | no run: the shortest window (80 s) took more than 600 s | open |
| Highway on p1-e4: mean speed | 20-33 m/s (LOS A free flow, minus entry) | 15.5 m/s; per-window 9.0 → 18.4 m/s, p50 20, p95 30 | open: defect candidate (finding 3) |
| Highway on p1-e4: vmax, headway, PDR | ≤ 37 m/s; 1-10 s; ≥ 0.95 near | 30.0 m/s; 3.0 s (min 1.26); 1.00 to 160 m, 0.92 at 600 m | pass |
| Grid vmax (phase1-grid, vru-grid) | ≤ 20 m/s | 11.2 and 11.6 | pass |
| Grid PDR against distance (vru-grid) | monotone within 0.05 | 1.00, then 0.48 at 120 m, 0.02 at 600 m | pass |
| pdr-vs-distance study | ≥ 0.95 near; monotone; ≤ 0.7 far | 1.00, then 0.5 at about 120 m, 0.04 at 600 m | pass |
| DSRC / LTE / NR end-to-end latency | ≤ 100 ms (J2945/1, TR 36.885, TS 22.186) | means 12.2 / 68.5 (p95 109) / 32.1 ms | pass |
| DSRC CBR | ≤ 0.6 | mean 0.007, max 0.016 | pass |
| LTE-V2X / DSRC PDR | 1.0-2.0 (3-6 dB link-budget gain) | 2.26; PDR is 0.5 at 340 m against 140 m | wrong-expectation: a pooled ratio weights the far bins, where DSRC is about 0. The range ratio of 2.4 fits an 8-11 dB gain at urban exponents of 2.7-3.5 |

## Findings

1. **Open defect: PSM certificate octets exceed the envelope under PQ hybrid.** In
   `real-world/matrix/pq-hybrid`, run as a 40 s overlay, the PSMs' `cert_bytes_share` is
   623,200 / 454,860 = 1.37. The attached-certificate octets that `node.tx` reports are larger
   than the whole security envelope that contains them; `security_overhead` for the same
   messages is 0.95. Every other run and message type stays within [0, 1].
   - Either `cert_bytes` counts the hybrid certificate (or its fragments) while
     `envelope_bytes` does not, or the reverse.
   - Not root-caused this session. Owed: trace `cert_bytes` against `envelope_bytes` for VRU
     senders under `pq-hybrid`, then a test that fails without the fix.
2. **Other environment-bound failures:**
   - `interop/sumo-grid` needs SUMO installed.
   - `portland-transit` needs `worlds/cache/portland.gtfs/` (`worlds/fetch-gtfs.sh`); the cache
     on this machine has no GTFS feed.
3. **Vehicles spawn at rest, mid-lane, even on a 29 m/s highway** (open, defect candidate).
   `spawn_with_route` sets `speed_mps: 0.0` (`crates/v2xw-mobility/src/engine.rs`). On p1-e4
   (highway-congestion's world at 3,000 veh/h, no events), mean speed per window rises from
   9 m/s at 10 s to 18 m/s by 40-60 s, with p95 at 30. The synthetic highway is also a 2 × 2
   grid ring with at-grade corners of 15 m radius, so traffic slows at every corner.
   - A real freeway entry merges at speed. SUMO's `departSpeed` is the knob for this; its
     default is 0, and highway studies set it to `max` or `desired`.
   - The fix is to depart at the lower of the desired speed and a safe speed behind the
     leader, but only on roads where a vehicle enters already moving. It moves every traffic
     golden, so it needs a flag. Owed: the flag, a test that fails without it, and the highway
     row re-run.
4. **Six stale run-cost overlays removed** (`cfb9cf6e`): `scale/cost/{falcon, mldsa87,
   pq-ecdsa, phy-high, prop-abstract, rate20}.yaml`. `v2xw validate` refuses all six, nothing
   references them, and the valid rungs that replaced them are kept: pq-falcon, pq-mldsa87,
   radio-high, radio-abstract and step50-rate20.
5. **Twelve `scenarios/sensitivity/*.yaml` files are sensitivity specs, not scenarios.**
   `v2xw validate` rejects them for having no schema, which is expected. They run through
   `v2xw sensitivity`.
6. **Three scenarios cannot finish a short window within the 10-minute run budget:**
   - `highway-congestion`: its first breakdown is at 75 s, so the shortest window is 80 s. About
     164 vehicles, all within radio range of each other, ran past 600 s of wall time. A
     `sample` profile shows only the expected reception, verify and metric work.
   - `midtown-peak-load`: 60 s took longer than 600 s, so it was re-queued at 10 s.
   - `manhattan-day`: the loader refuses any window that ends before its 6 h attack. This is
     by design: the scenario validates.
7. **The D9 sum grid loses precision on fractional sums** (no golden moves; reported, not
   fixed). A ratio of sums writes both of its sums on a 1e-3 grid:
   - `bytes_per_vehicle_hour` per 1 s window writes a denominator of 0.001 vehicle-hours for
     0.00053;
   - `sig_cbr_extra` writes a numerator of 0.001 for 0.0008.

   The points are computed before rounding and are correct, but a reader cannot recompute
   them from the sums. A finer grid for sums that are fractions would let an audit recompute
   them; that change moves every metrics digest.
8. **Identical runs worth knowing:**
   - `portland.yaml` is `portland-downtown.yaml` with only its metadata changed: same actors,
     same frames, receptions off by 1.
   - `base/city`, `midtown-normal` and `attack-replay` give the same traffic and receptions,
     because a replay attacker re-sends inside its own BSM slot.
   - `midtown-rush-hour` and `attack-fake-positions` match in the same way.
9. **Cost:**
   - `midtown-street-life` runs 7.5× slower than real time (1,751 actors, 449 s for 60 s,
     199 MB).
   - `pdr-vs-distance` takes 241 s for 60 s.
   - `midtown-rush-hour` and `attack-fake-positions` take 260 s for 95 s.
   - p1-e4 takes 57 s for 60 s.
   - Every run peaked under 200 MB.

10. **`--duration-s` cannot shorten a long scenario.**
   - The loader refuses an override that cuts a scheduled item: 40 runs on `time.fast_forward`
     past the end, 7 on `warmup_s` at or past the window, and 5 on an event or attack after it.
     This is correct for a file, but it means none of the multi-day `real-world/` and `edge/`
     scenarios can be smoke-tested from the command line.
   - Here they ran as 40-65 s overlays: `meta.base` set to the shipped file, plus
     `time.duration_s` and `time.fast_forward: []`. All 25 that ran this way pass.
   - Owner choice: when the window comes from the command line, the loader could clip what lies
     past its end and say so on stderr.
11. **Stale comment:** the header of `phase1-manhattan.yaml` says the scenario stops with "world
   source ... does not support osm-xml". It runs now: 2 vehicles in 10 s, 86 frames.
12. **Multi-week variants are identical inside a short window**, as expected because they
    differ only after the fast-forward:
    - every `edge/*` overlay gives 5 actors, 2,696 frames and 3,370 receptions;
    - the `real-world/matrix/*` DSRC variants give 133 actors and about 5,576 frames;
    - `matrix/lte` sends 11,073 frames, twice as many. This fits LTE-V2X's blind HARQ
      retransmission being counted as a frame; it was not checked against the PHY records.

## Every scenario

Status comes from the rows that cover every run (`runs: all`). "Not run" means the shared run
slots ran out within the three-hour budget; those files validate.

| Scenario | Window s | Wall s | Peak MB | Actors | Frames tx/rx ok | Status |
|---|---|---|---|---|---|---|
| `attack-fake-positions.yaml` | 95 | 262.68 | 130 | 121 | 52928/273606 | pass |
| `attack-jamming.yaml` |  | 40.01 | 100 | 52 | 15756/41055 | pass |
| `attack-replay.yaml` |  | 31.16 | 102 | 52 | 15756/41055 | pass |
| `base/city.yaml` | 60 | 36.23 | 102 | 52 | 15756/41055 | pass |
| `base/midtown.yaml` |  | 37.87 | 101 | 52 | 15756/42760 | pass |
| `base/radio-faceoff.yaml` | 10 | 2.85 | 95 | 14 | 971/492 | pass |
| `berlin.yaml` | 10 | 1.17 | 45 | 5 | 214/23 | pass |
| `ccms-lifecycle.yaml` | 65 | 3.31 | 25 | 19 | 4234/10218 | pass |
| `compare-signature-schemes.yaml` | 10 | 6.69 | 36 | 40 | 2389/9441 | pass |
| `connected-intersections.yaml` | 10 | 2.26 | 29 | 7 | 1014/2513 | pass |
| `credential-lifecycle.yaml` | 65 | 7.3 | 29 | 19 | 6956/17694 | pass |
| `credential-weeks.yaml (40 s overlay, no fast-forward)` | 65 | 32.91 | 37 | 19 | 6956/17695 | pass |
| `density-sweep-congestion.yaml` | 10 | 7.21 | 95 | 8 | 474/4 | pass |
| `edge/clock-skew-ahead.yaml (40 s overlay, no fast-forward)` | 65 | 24.67 | 29 | 5 | 2696/3383 | pass |
| `edge/many-cases.yaml (40 s overlay, no fast-forward)` | 65 | 10.52 | 30 | 5 | 2696/3370 | pass |
| `edge/multi-week-crlg-down-late.yaml (40 s overlay, no fast-forward)` | 65 | 32.99 | 30 | 5 | 2696/3370 | pass |
| `edge/multi-week-crlg-down.yaml (40 s overlay, no fast-forward)` | 65 | 8.59 | 30 | 5 | 2696/3370 | pass |
| `edge/multi-week-parked.yaml (40 s overlay, no fast-forward)` | 65 | 0.05 | 4 |  | / | did not run: v2xw: events[0].t: is 640 s, which is outside the run [0, 65] s (time.duration_s), so the event would never ha |
| `edge/multi-week.yaml (40 s overlay, no fast-forward)` | 65 | 12.62 | 30 | 5 | 2696/3370 | pass |
| `edge/no-coverage.yaml (40 s overlay, no fast-forward)` | 65 | 5.47 | 31 | 5 | 2697/1345 | pass |
| `edge/offline-weeks.yaml (40 s overlay, no fast-forward)` | 65 | 17.67 | 30 | 5 | 2696/3370 | pass |
| `edge/peer-root-removed.yaml (40 s overlay, no fast-forward)` | 65 | 8.21 | 30 | 5 | 2696/3370 | pass |
| `edge/ra-outage-topup.yaml (40 s overlay, no fast-forward)` | 65 | 15.88 | 30 | 5 | 2696/3370 | pass |
| `edge/standing-crl-10k.yaml (40 s overlay, no fast-forward)` | 65 | 22.33 | 79 | 5 | 4360/5506 | pass |
| `edge/thanksgiving.yaml (40 s overlay, no fast-forward)` | 65 | 9.06 | 30 | 5 | 2696/3370 | pass |
| `emergency-priority.yaml` | 10 | 13.82 | 97 | 13 | 1227/1692 | pass |
| `europe-credentials.yaml` | 65 | 7.3 | 54 | 12 | 2506/2057 | pass |
| `glosa-energy.yaml` | 10 | 3.61 | 26 | 7 | 1115/3630 | pass |
| `highway-congestion.yaml` | 80 |  |  |  | / | did not run: timed out at 600 s |
| `interop/sumo-grid.yaml` | 10 | 0.2 | 6 |  | / | did not run: v2xw: mobility.provider: no SUMO installation found: set mobility.sumo_home, $SUMO_HOME or $V2XW_SUMO_VENV (pi |
| `manhattan-5min.yaml` |  | 22.66 | 99 | 26 | 9166/20305 | pass |
| `manhattan-day.yaml` | 10 | 0.03 | 5 |  | / | did not run: v2xw: threats.attackers[0].schedule: starts the attack at 21600 s, but the run ends at 10 s, so no attack woul |
| `midtown-normal.yaml` | 60 | 58.4 | 97 | 52 | 15756/41055 | pass |
| `midtown-peak-load.yaml` | 10 | 78.12 | 201 | 2336 | 10611/129394 | pass |
| `midtown-pedestrians.yaml` |  | 104.33 | 133 | 746 | 23710/148559 | pass |
| `midtown-rain.yaml` | 60 | 64.72 | 96 | 52 | 15756/40426 | pass |
| `midtown-rush-hour.yaml` | 95 | 260.06 | 130 | 121 | 52928/273606 | pass |
| `midtown-street-life.yaml` | 60 | 448.64 | 199 | 1751 | 37893/400029 | pass |
| `pdr-vs-distance.yaml` | 60 | 240.86 | 107 | 251 | 78751/312837 | pass |
| `phase1-grid.yaml` |  | 0.81 | 30 | 2 | 1086/0 | pass |
| `phase1-manhattan.yaml` | 10 | 3.41 | 98 | 2 | 86/0 | pass |
| `phase2-manhattan.yaml` | 10 | 11.32 | 99 | 5 | 316/560 | pass |
| `portland-downtown.yaml` | 60 | 5.6 | 48 | 19 | 6770/3835 | pass |
| `portland-transit.yaml` | 10 | 0.88 | 37 |  | / | did not run: v2xw: actors.vehicles.demand.params: worlds/cache/portland.gtfs/stops.txt: missing |
| `portland.yaml` | 60 | 5.36 | 49 | 19 | 6770/3836 | pass |
| `post-quantum.yaml` | 10 | 8.21 | 94 | 13 | 871/1136 | pass |
| `pq-certificate-cycle.yaml` | 10 | 1.57 | 17 | 7 | 450/187 | pass |
| `pq-full-stack.yaml` | 10 | 1.55 | 18 | 7 | 1041/418 | pass |
| `pq-signatures.yaml` | 10 | 4.0 | 19 | 7 | 450/188 | pass |
| `privacy-pseudonyms.yaml` | not run (budget) | | | | | validates |
| `pseudonym-privacy.yaml` | 35 | 51.96 | 96 | 59 | 10657/21006 | pass |
| `pseudonym-strategies.yaml` | not run (budget) | | | | | validates |
| `radio-dsrc.yaml` | 60 | 44.01 | 94 | 78 | 23628/63572 | pass |
| `radio-lte.yaml` | 60 | 67.44 | 102 | 78 | 46866/167124 | pass |
| `radio-nr.yaml` | 60 | 34.34 | 95 | 78 | 22714/93224 | pass |
| `rat-comparison.yaml` | not run (budget) | | | | | validates |
| `real-world/audit-sim/lte.yaml` | 40 | 0.05 | 4 | 653 | 47261/642250 | pass |
| `real-world/audit-sim/quarter-attack.yaml (40 s overlay, no fast-forward)` | 40 | 213.02 | 191 | 653 | 23862/161523 | pass |
| `real-world/audit-sim/short.yaml` | 10 | 0.02 | 5 |  | / | did not run: v2xw: time.warmup_s: is 30 s, which leaves nothing of the 10 s run to measure; make it shorter than time.durat |
| `real-world/audit/onair-base.yaml` | 10 | 0.05 | 4 |  | / | did not run: v2xw: time.warmup_s: is 30 s, which leaves nothing of the 10 s run to measure; make it shorter than time.durat |
| `real-world/audit/onair-change.yaml` | 10 | 0.01 | 4 |  | / | did not run: v2xw: time.warmup_s: is 30 s, which leaves nothing of the 10 s run to measure; make it shorter than time.durat |
| `real-world/audit/onair-p2pcd.yaml` | 40 | 25.81 | 128 | 133 | 5584/15808 | pass |
| `real-world/matrix/att10.yaml (40 s overlay, no fast-forward)` | 40 | 56.53 | 113 | 133 | 5577/15747 | pass |
| `real-world/matrix/backhaul-cellular.yaml (40 s overlay, no fast-forward)` | 40 | 21.87 | 129 | 133 | 5576/15736 | pass |
| `real-world/matrix/camp.yaml (40 s overlay, no fast-forward)` | 40 | 41.93 | 112 | 133 | 5576/15736 | pass |
| `real-world/matrix/fleet-half.yaml (40 s overlay, no fast-forward)` | 40 | 16.08 | 126 | 122 | 3195/8410 | pass |
| `real-world/matrix/full-cert.yaml (40 s overlay, no fast-forward)` | 40 | 32.57 | 118 | 133 | 5576/15678 | pass |
| `real-world/matrix/gnss-clean.yaml (40 s overlay, no fast-forward)` | 40 | 25.98 | 130 | 133 | 5576/15736 | pass |
| `real-world/matrix/gnss-raw.yaml (40 s overlay, no fast-forward)` | 40 | 49.02 | 113 | 133 | 5630/15966 | pass |
| `real-world/matrix/gnss-rtk.yaml (40 s overlay, no fast-forward)` | 40 | 34.68 | 111 | 133 | 5576/15736 | pass |
| `real-world/matrix/lte.yaml (40 s overlay, no fast-forward)` | 40 | 31.13 | 140 | 133 | 11073/55590 | pass |
| `real-world/matrix/ma-legacy.yaml (40 s overlay, no fast-forward)` | 40 | 22.58 | 129 | 133 | 5576/15736 | pass |
| `real-world/matrix/no-hsm.yaml (40 s overlay, no fast-forward)` | 40 | 46.94 | 115 | 133 | 5576/15779 | pass |
| `real-world/matrix/nr.yaml (40 s overlay, no fast-forward)` | 40 | 28.29 | 133 | 133 | 5410/27392 | pass |
| `real-world/matrix/nyc-pilot.yaml (40 s overlay, no fast-forward)` | 40 | 46.85 | 112 | 133 | 5576/15736 | pass |
| `real-world/matrix/on-demand.yaml (40 s overlay, no fast-forward)` | 40 | 26.28 | 128 | 133 | 5576/15735 | pass |
| `real-world/matrix/pen0.yaml (40 s overlay, no fast-forward)` | 40 | 38.7 | 100 | 133 | 5636/15878 | pass |
| `real-world/matrix/pq-hybrid.yaml (40 s overlay, no fast-forward)` | 40 | 22.09 | 107 | 133 | 6830/18126 | FAIL SC.arithmetic |
| `real-world/matrix/real-crypto.yaml (40 s overlay, no fast-forward)` | 40 | 33.49 | 117 | 133 | 5576/15736 | pass |
| `real-world/matrix/rsu0.yaml (40 s overlay, no fast-forward)` | 40 | 21.28 | 115 | 133 | 5573/6552 | pass |
| `real-world/matrix/rsu5.yaml (40 s overlay, no fast-forward)` | 40 | 19.45 | 127 | 133 | 5573/7287 | pass |
| `real-world/matrix/silent.yaml (40 s overlay, no fast-forward)` | 40 |  |  | 133 | 5576/15736 | pass |
| `real-world/matrix/split-90-10.yaml (40 s overlay, no fast-forward)` | 40 | 31.26 | 116 | 133 | 5576/15736 | pass |
| `real-world/matrix/sybil.yaml` | 40 | 2.84 | 4 |  | / | did not run: v2xw: time.fast_forward[0]: spans [300, 43185] s but the run is [0, 40] s (time.duration_s); a window outside  |
| `real-world/matrix/test-ctl.yaml (40 s overlay, no fast-forward)` | 40 | 33.44 | 118 | 133 | 5701/16311 | pass |
| `real-world/matrix/tls12.yaml` | 40 | 0.56 | 4 |  | / | did not run: v2xw: time.fast_forward[0]: spans [300, 43185] s but the run is [0, 40] s (time.duration_s); a window outside  |
| `real-world/midtown-scms-benchmark-lte.yaml (40 s overlay, no fast-forward)` | 130 |  |  |  | / | did not run: timed out at 600 s |
| `real-world/midtown-scms-benchmark-quarter.yaml` | 40 | 0.51 | 4 |  | / | did not run: v2xw: time.fast_forward[0]: spans [60, 43185] s but the run is [0, 40] s (time.duration_s); a window outside t |
| `real-world/midtown-scms-benchmark.yaml` | 10 | 0.4 | 5 |  | / | did not run: v2xw: time.warmup_s: is 120 s, which leaves nothing of the 10 s run to measure; make it shorter than time.dura |
| `real-world/midtown-scms-realism.yaml` | 40 | 0.68 | 4 |  | / | did not run: v2xw: time.fast_forward[0]: spans [300, 43185] s but the run is [0, 40] s (time.duration_s); a window outside  |
| `real-world/paper1/p1-attack.yaml` | 40 | 19.64 | 151 | 23 | 4376/11250 | pass |
| `real-world/paper1/p1-base.yaml` | 40 | 20.98 | 136 | 23 | 4373/11236 | pass |
| `real-world/paper1/p1-dec-h5.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-dec-m1.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-dec.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-e4-80-wn.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-e4-80.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-e4-free.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-e4-s-free.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-e4-s-v1.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-e4-s-wn.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-e4-s.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-e4-wn.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-e4.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-m05.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-ma-l0.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-ma-l1.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-ma-off.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-off.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-q0.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-q2.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-q4.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-rp-v10-gt-01.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-rp-v10-gt-02.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-rp-v10-gt-05.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-rp-v10-sm-01.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-rp-v10-sm-02.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-rp-v11-gt-01.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-rp-v11-gt-02.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-rp-v11-gt-05.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-rp-v11-sm-01.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-rp-v11-sm-02.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-rp-v11-sm-05.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-share0.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-share100.yaml` | not run (budget) | | | | | validates |
| `real-world/paper1/p1-share50.yaml` | not run (budget) | | | | | validates |
| `real-world/portland-scms-benchmark.yaml` | not run (budget) | | | | | validates |
| `real-world/realism/bsm-timing.yaml` | not run (budget) | | | | | validates |
| `real-world/realism/cert60.yaml` | not run (budget) | | | | | validates |
| `real-world/realism/crl-on-decision.yaml` | not run (budget) | | | | | validates |
| `real-world/realism/fleet2x.yaml` | not run (budget) | | | | | validates |
| `real-world/realism/ma-auto.yaml` | not run (budget) | | | | | validates |
| `real-world/realism/no-sharing.yaml` | not run (budget) | | | | | validates |
| `real-world/realism/off-ctl.yaml` | not run (budget) | | | | | validates |
| `real-world/realism/region-west.yaml` | not run (budget) | | | | | validates |
| `real-world/realism/research-suite.yaml` | not run (budget) | | | | | validates |
| `real-world/realism/us-profile.yaml` | not run (budget) | | | | | validates |
| `real-world/stress/fs-40.yaml` | not run (budget) | | | | | validates |
| `real-world/stress/fs-det.yaml` | not run (budget) | | | | | validates |
| `real-world/stress/fs-first-window.yaml` | not run (budget) | | | | | validates |
| `real-world/stress/pq-base.yaml` | not run (budget) | | | | | validates |
| `real-world/stress/pq-hybrid-falcon512-certcycle.yaml` | not run (budget) | | | | | validates |
| `real-world/stress/pq-hybrid-falcon512.yaml` | not run (budget) | | | | | validates |
| `real-world/stress/pq-hybrid-mldsa44-certcycle.yaml` | not run (budget) | | | | | validates |
| `real-world/stress/pq-hybrid-mldsa44.yaml` | not run (budget) | | | | | validates |
| `real-world/stress/pq-mldsa44.yaml` | not run (budget) | | | | | validates |
| `real-world/variants/q60-2x.yaml` | not run (budget) | | | | | validates |
| `real-world/variants/q60-lte.yaml` | not run (budget) | | | | | validates |
| `real-world/variants/q60-portland.yaml` | not run (budget) | | | | | validates |
| `real-world/variants/q60.yaml` | not run (budget) | | | | | validates |
| `revocation-latency.yaml` | not run (budget) | | | | | validates |
| `scale/10.yaml` | not run (budget) | | | | | validates |
| `scale/100.yaml` | not run (budget) | | | | | validates |
| `scale/1000.yaml` | not run (budget) | | | | | validates |
| `scale/10000.yaml` | not run (budget) | | | | | validates |
| `scale/2.yaml` | not run (budget) | | | | | validates |
| `scale/base.yaml` | not run (budget) | | | | | validates |
| `scale/city.yaml` | not run (budget) | | | | | validates |
| `scale/cost/cam-wsmp.yaml` | not run (budget) | | | | | validates |
| `scale/cost/f10.yaml` | not run (budget) | | | | | validates |
| `scale/cost/f100-10s.yaml` | not run (budget) | | | | | validates |
| `scale/cost/f100-30s.yaml` | not run (budget) | | | | | validates |
| `scale/cost/f100.yaml` | not run (budget) | | | | | validates |
| `scale/cost/f1000-capped500-abstract.yaml` | not run (budget) | | | | | validates |
| `scale/cost/f1000-capped500.yaml` | not run (budget) | | | | | validates |
| `scale/cost/f1000-range300.yaml` | not run (budget) | | | | | validates |
| `scale/cost/f1000.yaml` | not run (budget) | | | | | validates |
| `scale/cost/f300-abstract.yaml` | not run (budget) | | | | | validates |
| `scale/cost/f300-range300.yaml` | not run (budget) | | | | | validates |
| `scale/cost/f300.yaml` | not run (budget) | | | | | validates |
| `scale/cost/f520-uncapped.yaml` | not run (budget) | | | | | validates |
| `scale/cost/gn-hop1.yaml` | not run (budget) | | | | | validates |
| `scale/cost/gn-hop10.yaml` | not run (budget) | | | | | validates |
| `scale/cost/gnbtp.yaml` | not run (budget) | | | | | validates |
| `scale/cost/mac-abstract.yaml` | not run (budget) | | | | | validates |
| `scale/cost/mac-high.yaml` | not run (budget) | | | | | validates |
| `scale/cost/margin20.yaml` | not run (budget) | | | | | validates |
| `scale/cost/margin5.yaml` | not run (budget) | | | | | validates |
| `scale/cost/meso1000-nofollow.yaml` | not run (budget) | | | | | validates |
| `scale/cost/meso1000.yaml` | not run (budget) | | | | | validates |
| `scale/cost/metrics-all.yaml` | not run (budget) | | | | | validates |
| `scale/cost/nobuild.yaml` | not run (budget) | | | | | validates |
| `scale/cost/phy-abstract.yaml` | not run (budget) | | | | | validates |
| `scale/cost/pq-falcon.yaml` | not run (budget) | | | | | validates |
| `scale/cost/pq-mldsa87.yaml` | not run (budget) | | | | | validates |
| `scale/cost/prop-high.yaml` | not run (budget) | | | | | validates |
| `scale/cost/radio-abstract.yaml` | not run (budget) | | | | | validates |
| `scale/cost/radio-high.yaml` | not run (budget) | | | | | validates |
| `scale/cost/range300.yaml` | not run (budget) | | | | | validates |
| `scale/cost/rate5.yaml` | not run (budget) | | | | | validates |
| `scale/cost/step50-rate20.yaml` | not run (budget) | | | | | validates |
| `scale/cost/step50-rate5.yaml` | not run (budget) | | | | | validates |
| `scale/cost/step50.yaml` | not run (budget) | | | | | validates |
| `scms-audit-mbpipe.yaml` | not run (budget) | | | | | validates |
| `scms-conformance.yaml` | not run (budget) | | | | | validates |
| `scms-oracle-mbr.yaml` | not run (budget) | | | | | validates |
| `scms-oracle-repro.yaml` | not run (budget) | | | | | validates |
| `scms-pq-relay-repro.yaml` | not run (budget) | | | | | validates |
| `sensitivity/_cpl_probe.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/catalogue.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/dsrc-dense.yaml` | not run (budget) | | | | | validates |
| `sensitivity/dsrc-grid.yaml` | not run (budget) | | | | | validates |
| `sensitivity/gn-grid.yaml` | not run (budget) | | | | | validates |
| `sensitivity/lte-grid.yaml` | not run (budget) | | | | | validates |
| `sensitivity/nr-grid.yaml` | not run (budget) | | | | | validates |
| `sensitivity/phy-channel.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/radio-cv2x.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/radio-dsrc-dense.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/radio-dsrc.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/radio-gn.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/test-mac-congestion-sidelink.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/test-mac-congestion.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/test-networking-dsrc.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/test-networking.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `threshold-pca.yaml` | not run (budget) | | | | | validates |
| `threshold-revocation-2of2.yaml` | not run (budget) | | | | | validates |
| `threshold-revocation-3of5.yaml` | not run (budget) | | | | | validates |
| `vru-grid.yaml` | 60 | 29.44 | 36 | 187 | 9982/43692 | pass |
