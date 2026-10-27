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

## 2026-10-26 continuation

- **Binary:** release `v2xw` built from `938fb16a`, run as a copy, with the same flags and
  each scenario's own seed. Each run went through its own run-slot call: a tiny slot first,
  moving up to slot 2 and then the main slot when the memory cap stopped it.
- **Coverage:** 115 of the 117 scenarios the 10-25 pass had not run now have a run
  (shortest window first: the scenario's own length up to 10 s, or warm-up + 10 s). The
  multi-day files ran as 40 s overlays without fast-forward, as on 10-25.
  - `stress/fs-first-window` (the full fleet, 130 s) timed out at 600 s. Its 5 s sibling
    `fs-det` took 133 s, so the 130 s run needs about an hour: it belongs to the full-scale
    package.
  - `portland-scms-benchmark` (overlay; a 120 s warm-up plus 10 s on the Portland fleet) timed out at 600 s too.
  - Three 10-25 refusals now run at warm-up + 10 s = 40 s and pass: `audit-sim/short`,
    `audit/onair-base` and `audit/onair-change`.
  - Total for the new runs: 4,690 s of wall time; the highest peak was 681 MB
    (`scale/cost/f1000`).
- **Checker:** `live_scenarios.py` reads the 10-25 runs overlaid by the 10-26 ones (the
  10-25 overlays now point at this worktree). Selftest: every 10-26 row that has a run goes red under its fault; the nine fix rows (PQH.*, HWY3.*) have no run to fault yet.

### Verdicts, 2026-10-26

| Change / row | Predicted (source) | Measured | Verdict |
|---|---|---|---|
| Every new run: exit 0, verified recording, no panic | as SC.boots | 113 of 115 exited 0 with a verified recording; no panic; the 2 timeouts are above | pass |
| Every new run: arithmetic (part-of-whole ratios in [0, 1]) | SC.arithmetic | `cert_bytes_share{psm}` is over 1 on every PQ run that fragments: stress/pq-hybrid-falcon512 1.37, pq-hybrid-mldsa44 3.01, pq-mldsa44 3.02 (all messages 1.30), scms-pq-relay-repro 1.37 | engine defect, fixed (finding 1 below) |
| `stress/fs-det`: metric samples and `ped_*` samples | SC.records, SC.peds: samples > 0 | 1.12 M records and no metric sample. `metric_period_s` is 10 s, the warm-up is 2 s and the window is 5 s, so no period closes after the warm-up (run.rs `on_metric_flush`) | wrong-expectation. Corrected rows SC.records-b and SC.peds-b pass on all runs |
| `scale/2`, `scale/base`: receptions | SC.radio: receptions ok > 0 | scale/2: 5 cars on Midtown with buildings, 295 attempts, 291 below sensitivity and 4 half-duplex. scale/base: 1 car, so no receiver | wrong-expectation. Corrected row SC.radio-b (attempts = below-sensitivity + half-duplex) passes |
| `scale/cost/pq-mldsa87`: frames | frames > 0 | 0 frames and 2,850 `net_mtu_refusals`: a 4,627 B ML-DSA-87 signature does not fit the WSMP MTU, and `fragmenter/none` refuses the message before the MAC | wrong-expectation for the engine (IEEE 1609.3 has no fragmentation). Scenario finding: this cost rung measures no radio work, so it needs `fragmenter/generic-sdu` to price PQ traffic. Exempt in SC.radio-b |
| Kinematics, Edie, demand, latency, SCMS, threat and VRU rows on the new runs | as 10-25 | all hold | pass |
| PQH.psm-cert-share (pq-hybrid, fixed binary) | 1.37 / 2 pieces = 0.69, band [0.55, 0.80] | — | pending |
| PQH.psm-overhead | 0.95 ± 0.01 (envelope unchanged) | — | pending |
| PQH.arithmetic | every part-of-whole ratio in [0, 1] | — | pending |
| HWY3.off-unchanged (`depart_speed_max` off) | record digest identical to release | — | pending |
| HWY3.first-window (flag on, first 10 s) | ≥ 15 m/s (9.0 at rest) | — | pending |
| HWY3.speed (60 s) | [16.5, 23] m/s: the 15 m ring corners cap it at √(5.5 × 15) = 9 m/s | — | pending |
| HWY3.vs-base | 1.05-1.5 | — | pending |
| HWY3.kinematics, HWY3.headway | a ≥ -9 m/s², v ≤ 37; 1-10 s | — | pending |
| Run cost: `depart_speed_max` | wall within ± 20 % (same fleet, same frames ± a few %) | — | pending |

### Fixes, 2026-10-26

pending: not yet run. The build queue (one build lock for the whole machine, with 8 jobs waiting) did not reach this branch's `cargo check` in 2.5 h, so there is no fixed binary yet. The fixes and their tests are committed but have not been compiled; the binary that judges the PQH.* and HWY3.* rows is owed.

1. **Finding 1, fixed: each fragment repeated the whole certificate.**
   - Cause: `Engine::hand_down` (run.rs) splits a message into generic-sdu pieces. It sets
     each piece's `payload_bytes` and `envelope_bytes` to that piece's share, but cloned the
     whole SPDU's `cert_bytes` onto every piece.
   - Effect: a certificate-bearing SPDU in n pieces counted n certificates against one
     envelope, so `cert_bytes_share` reached 1.37 (2 pieces) and 3.0 (ML-DSA-44, about 4
     pieces).
   - Fix: `frag::fragment_cert_share` gives each piece its share of the certificate, by
     cumulative integer division. The shares sum to the certificate exactly, and no share
     exceeds its piece's envelope.
   - Tests: `tests/fragmentation.rs::a_split_certificate_is_counted_once_across_its_pieces`
     (fails without the fix: the piece's certificate is larger than its envelope) and a
     unit test in `frag.rs`.
   - Goldens: only `node.tx` `cert_bytes` and `cert_bytes_share` move, and only on runs
     that fragment certificate-bearing SPDUs (PQ with generic-sdu). No P-256 golden moves.
2. **Finding 3, behind a flag: `traffic.depart_speed_max`** (SUMO `departSpeed="max"`; off by
   default, which is SUMO's own default of 0).
   - Integration note (2026-10-26): the flag was committed as `traffic.depart_at_speed`, the name WP-05 (P9) took at the same time for map-edge entrants only; it is renamed `traffic.depart_speed_max` so both behaviours keep their own switch.
   - With the flag on, a trip enters the road at the lowest of three speeds:
     - its desired speed on the lane (its limit × speed factor, and the slower lane or turn
       it anticipates);
     - the speed it can stop from at its comfortable deceleration before the lane ends;
     - the speed that keeps the IDM's `s0 + vT` gap to the vehicle ahead and can brake to
       that vehicle's speed within the gap.
   - Test: `engine.rs::a_trip_departs_at_the_highest_safe_speed_when_asked`. Without the
     flag, both trips depart at 0 m/s; with it, the trip on the clear lane departs moving,
     and the trip 8 m behind is capped by the headway.
   - Wired through schema, validate (Wired), guide and options. No golden moves while the
     flag is off.
   - Owner choice: setting it in `highway-congestion` and `p1-e4*` moves their goldens.

### New findings, 2026-10-26

13. **A metric period that is still open at the end of a run is dropped.** Windows close
    only on the `metric_period_s` grid, at or before the horizon (`next_metric_flush`).
    - A run that ends mid-period loses its tail. A run shorter than the period after its
      warm-up (`stress/fs-det`) records no sample at all.
    - SUMO's interval outputs write the final partial interval at the end of the run.
    - Closing a partial window at the horizon would move the goldens of any run whose length
      is not a whole number of periods. Reported, not changed: owner choice.
14. **`scale/cost/pq-mldsa87` transmits nothing** (see the table above). `pq-falcon` loses
    each car's once-a-second certificate-bearing BSM to the same MTU rule (290
    refusals). These rungs price signing but not PQ air time.
15. **The finding 11 comment is in `phase1-grid.yaml`, not in `phase1-manhattan.yaml`.**
    Lines 6-9 still say that the OSM world is refused. It was left alone, because a
    scenario-file edit can move hashed goldens.
16. **Cost:**
    - `scale/1000` and `scale/cost/f1000` (1,267 cars, 2 s): about 300 s and about 670 MB,
      with 11.5 M reception attempts in 2 s.
    - `stress/fs-40` (2,408 actors, 20 s): 375 s.
    - `stress/pq-mldsa44` (40 s): 349 s, against 241 s for its hybrid sibling.
    - `real-world/variants/q60-2x` (40 s): 199 s and 304 MB.

### Still open after 10-26
W.HWY.* (`highway-congestion`, 80 s run, past the budget), W.MID.headway (needs a longer or
denser Midtown run), W.GRID.pdr-distance (2 cars), SC.boots for the refusals of finding 10
(owner choice) and the environment-bound runs (SUMO, GTFS). Owed: the PQH.* and HWY3.* runs on a binary built from this branch, then the highway-congestion row with the flag.

## Verdicts (2026-10-25)

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

Status comes from the rows that cover every run (`runs: all`; for 2026-10-26 rows, SC.records-b and SC.radio-b in place of SC.records and SC.radio). "Not run" meant, on 10-25, the shared run
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
| `privacy-pseudonyms.yaml` | 65 | 17.86 | 96 | 28 | 10583/8172 | pass (2026-10-26) |
| `pseudonym-privacy.yaml` | 35 | 51.96 | 96 | 59 | 10657/21006 | pass |
| `pseudonym-strategies.yaml` | 10 | 1.56 | 43 | 7 | 450/46 | pass (2026-10-26) |
| `radio-dsrc.yaml` | 60 | 44.01 | 94 | 78 | 23628/63572 | pass |
| `radio-lte.yaml` | 60 | 67.44 | 102 | 78 | 46866/167124 | pass |
| `radio-nr.yaml` | 60 | 34.34 | 95 | 78 | 22714/93224 | pass |
| `rat-comparison.yaml` | 10 | 2.33 | 48 | 47 | 2221/862 | pass (2026-10-26) |
| `real-world/audit-sim/lte.yaml` | 40 | 0.05 | 4 | 653 | 47261/642250 | pass |
| `real-world/audit-sim/quarter-attack.yaml (40 s overlay, no fast-forward)` | 40 | 213.02 | 191 | 653 | 23862/161523 | pass |
| `real-world/audit-sim/short.yaml` | 40 | 16.90 | 112 | 133 | 5576/15736 | pass (2026-10-26) |
| `real-world/audit/onair-base.yaml` | 40 | 16.41 | 110 | 133 | 5576/15736 | pass (2026-10-26) |
| `real-world/audit/onair-change.yaml` | 40 | 15.21 | 111 | 133 | 5576/15736 | pass (2026-10-26) |
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
| `real-world/paper1/p1-dec-h5.yaml` | 40 | 10.15 | 168 | 23 | 4402/11430 | pass (2026-10-26) |
| `real-world/paper1/p1-dec-m1.yaml` | 40 | 16.38 | 101 | 23 | 4402/11430 | pass (2026-10-26) |
| `real-world/paper1/p1-dec.yaml` | 40 | 9.81 | 140 | 23 | 4402/11430 | pass (2026-10-26) |
| `real-world/paper1/p1-e4-80-wn.yaml` | 10 | 1.59 | 47 | 3 | 283/515 | pass (2026-10-26) |
| `real-world/paper1/p1-e4-80.yaml` | 10 | 1.00 | 47 | 3 | 283/511 | pass (2026-10-26) |
| `real-world/paper1/p1-e4-free.yaml` | 10 | 1.93 | 55 | 6 | 305/788 | pass (2026-10-26) |
| `real-world/paper1/p1-e4-s-free.yaml` | 10 | 1.32 | 46 | 3 | 247/150 | pass (2026-10-26) |
| `real-world/paper1/p1-e4-s-v1.yaml` | 10 | 0.75 | 49 | 3 | 283/171 | pass (2026-10-26) |
| `real-world/paper1/p1-e4-s-wn.yaml` | 10 | 1.32 | 47 | 3 | 283/173 | pass (2026-10-26) |
| `real-world/paper1/p1-e4-s.yaml` | 10 | 0.74 | 48 | 3 | 283/172 | pass (2026-10-26) |
| `real-world/paper1/p1-e4-wn.yaml` | 10 | 3.00 | 53 | 6 | 349/901 | pass (2026-10-26) |
| `real-world/paper1/p1-e4.yaml` | 10 | 1.34 | 52 | 6 | 349/899 | pass (2026-10-26) |
| `real-world/paper1/p1-m05.yaml` | 40 | 14.18 | 106 | 23 | 4402/11430 | pass (2026-10-26) |
| `real-world/paper1/p1-ma-l0.yaml` | 40 | 115.84 | 221 | 653 | 23027/153480 | pass (2026-10-26) |
| `real-world/paper1/p1-ma-l1.yaml` | 40 | 125.61 | 134 | 653 | 23858/161070 | pass (2026-10-26) |
| `real-world/paper1/p1-ma-off.yaml` | 40 | 86.23 | 191 | 653 | 23824/161264 | pass (2026-10-26) |
| `real-world/paper1/p1-off.yaml` | 40 | 13.75 | 100 | 23 | 4813/12005 | pass (2026-10-26) |
| `real-world/paper1/p1-q0.yaml` | 40 | 9.36 | 160 | 23 | 4381/11270 | pass (2026-10-26) |
| `real-world/paper1/p1-q2.yaml` | 40 | 16.38 | 104 | 23 | 4402/11430 | pass (2026-10-26) |
| `real-world/paper1/p1-q4.yaml` | 40 | 9.90 | 179 | 23 | 4402/11430 | pass (2026-10-26) |
| `real-world/paper1/p1-rp-v10-gt-01.yaml` | 40 | 13.04 | 98 | 23 | 4391/11324 | pass (2026-10-26) |
| `real-world/paper1/p1-rp-v10-gt-02.yaml` | 40 | 10.34 | 136 | 23 | 4398/11398 | pass (2026-10-26) |
| `real-world/paper1/p1-rp-v10-gt-05.yaml` | 40 | 14.02 | 100 | 23 | 4398/11398 | pass (2026-10-26) |
| `real-world/paper1/p1-rp-v10-sm-01.yaml` | 40 | 13.48 | 110 | 23 | 4384/11284 | pass (2026-10-26) |
| `real-world/paper1/p1-rp-v10-sm-02.yaml` | 40 | 14.09 | 107 | 23 | 4397/11403 | pass (2026-10-26) |
| `real-world/paper1/p1-rp-v11-gt-01.yaml` | 40 | 12.63 | 108 | 23 | 4391/11322 | pass (2026-10-26) |
| `real-world/paper1/p1-rp-v11-gt-02.yaml` | 40 | 17.82 | 106 | 23 | 4398/11398 | pass (2026-10-26) |
| `real-world/paper1/p1-rp-v11-gt-05.yaml` | 40 | 13.49 | 109 | 23 | 4398/11398 | pass (2026-10-26) |
| `real-world/paper1/p1-rp-v11-sm-01.yaml` | 40 | 11.59 | 107 | 23 | 4384/11284 | pass (2026-10-26) |
| `real-world/paper1/p1-rp-v11-sm-02.yaml` | 40 | 12.21 | 112 | 23 | 4397/11403 | pass (2026-10-26) |
| `real-world/paper1/p1-rp-v11-sm-05.yaml` | 40 | 14.57 | 105 | 23 | 4397/11403 | pass (2026-10-26) |
| `real-world/paper1/p1-share0.yaml` | 40 | 12.62 | 108 | 23 | 4309/11035 | pass (2026-10-26) |
| `real-world/paper1/p1-share100.yaml` | 40 | 15.28 | 107 | 23 | 4813/12169 | pass (2026-10-26) |
| `real-world/paper1/p1-share50.yaml` | 40 | 10.95 | 180 | 23 | 4501/11537 | pass (2026-10-26) |
| `real-world/portland-scms-benchmark.yaml (40 s overlay, no fast-forward)` | 130 |  |  |  | / | did not run: timed out at 600 s (2026-10-26) |
| `real-world/realism/bsm-timing.yaml` | 40 | 14.22 | 106 | 23 | 4815/12131 | pass (2026-10-26) |
| `real-world/realism/cert60.yaml (40 s overlay, no fast-forward)` | 40 | 25.43 | 161 | 133 | 5576/15736 | pass (2026-10-26) |
| `real-world/realism/crl-on-decision.yaml (40 s overlay, no fast-forward)` | 40 | 23.37 | 89 | 133 | 5576/15736 | pass (2026-10-26) |
| `real-world/realism/fleet2x.yaml (40 s overlay, no fast-forward)` | 40 | 22.17 | 159 | 158 | 10205/32230 | pass (2026-10-26) |
| `real-world/realism/ma-auto.yaml (40 s overlay, no fast-forward)` | 40 | 23.35 | 109 | 133 | 5576/15736 | pass (2026-10-26) |
| `real-world/realism/no-sharing.yaml (40 s overlay, no fast-forward)` | 40 | 12.91 | 163 | 133 | 5576/15736 | pass (2026-10-26) |
| `real-world/realism/off-ctl.yaml (40 s overlay, no fast-forward)` | 40 | 18.81 | 98 | 133 | 5724/16476 | pass (2026-10-26) |
| `real-world/realism/region-west.yaml (40 s overlay, no fast-forward)` | 40 | 14.54 | 148 | 133 | 5576/15736 | pass (2026-10-26) |
| `real-world/realism/research-suite.yaml (40 s overlay, no fast-forward)` | 40 | 19.87 | 90 | 133 | 5576/15736 | pass (2026-10-26) |
| `real-world/realism/us-profile.yaml (40 s overlay, no fast-forward)` | 40 | 14.22 | 162 | 133 | 5576/15736 | pass (2026-10-26) |
| `real-world/stress/fs-40.yaml` | 20 | 375.19 | 201 | 2408 | 29896/524286 | pass (2026-10-26) |
| `real-world/stress/fs-det.yaml` | 5 | 133.23 | 269 | 2254 | 3952/47939 | FAIL SC.peds (2026-10-26) |
| `real-world/stress/fs-first-window.yaml` | 130 |  |  |  | / | did not run: timed out at 600 s (2026-10-26) |
| `real-world/stress/pq-base.yaml` | 40 | 80.32 | 163 | 653 | 23773/160731 | pass (2026-10-26) |
| `real-world/stress/pq-hybrid-falcon512-certcycle.yaml` | 40 | 75.79 | 138 | 653 | 23773/158850 | pass (2026-10-26) |
| `real-world/stress/pq-hybrid-falcon512.yaml` | 40 | 82.36 | 165 | 653 | 31159/181989 | FAIL SC.arithmetic (2026-10-26) |
| `real-world/stress/pq-hybrid-mldsa44-certcycle.yaml` | 40 | 76.40 | 123 | 653 | 23773/154214 | pass (2026-10-26) |
| `real-world/stress/pq-hybrid-mldsa44.yaml` | 40 | 241.05 | 199 | 653 | 69694/356811 | FAIL SC.arithmetic (2026-10-26) |
| `real-world/stress/pq-mldsa44.yaml` | 40 | 349.32 | 149 | 653 | 69699/358531 | FAIL SC.arithmetic (2026-10-26) |
| `real-world/variants/q60-2x.yaml` | 40 | 198.66 | 304 | 1288 | 48094/569742 | pass (2026-10-26) |
| `real-world/variants/q60-lte.yaml` | 40 | 146.12 | 200 | 653 | 47261/642250 | pass (2026-10-26) |
| `real-world/variants/q60-portland.yaml` | 40 | 20.31 | 88 | 203 | 7352/19887 | pass (2026-10-26) |
| `real-world/variants/q60.yaml` | 40 | 118.25 | 147 | 653 | 23766/160808 | pass (2026-10-26) |
| `revocation-latency.yaml` | 65 | 98.80 | 112 | 104 | 35914/117248 | pass (2026-10-26) |
| `scale/10.yaml` | 2 | 2.39 | 113 | 15 | 299/276 | pass (2026-10-26) |
| `scale/100.yaml` | 2 | 10.97 | 137 | 145 | 2834/18961 | pass (2026-10-26) |
| `scale/1000.yaml` | 2 | 306.08 | 661 | 1267 | 16650/676764 | pass (2026-10-26) |
| `scale/10000.yaml` | 0.3 | 16.70 | 98 | 388 | 562/5520 | pass (2026-10-26) |
| `scale/2.yaml` | 2 | 1.28 | 111 | 5 | 100/0 | pass (2026-10-26) |
| `scale/base.yaml` | 2 | 1.25 | 108 | 1 | 20/0 | pass (2026-10-26) |
| `scale/city.yaml` | 2 | 39.24 | 203 | 2052 | 8820/142069 | pass (2026-10-26) |
| `scale/cost/cam-wsmp.yaml` | 2 | 8.60 | 119 | 145 | 1454/9876 | pass (2026-10-26) |
| `scale/cost/f10.yaml` | 2 | 1.59 | 113 | 15 | 299/276 | pass (2026-10-26) |
| `scale/cost/f100-10s.yaml` | 10 | 43.04 | 122 | 150 | 14696/107636 | pass (2026-10-26) |
| `scale/cost/f100-30s.yaml` | 10 | 57.30 | 113 | 150 | 14696/107636 | pass (2026-10-26) |
| `scale/cost/f100.yaml` | 2 | 11.26 | 146 | 145 | 2834/18961 | pass (2026-10-26) |
| `scale/cost/f1000-capped500-abstract.yaml` | 2 | 106.77 | 345 | 1049 | 15282/517312 | pass (2026-10-26) |
| `scale/cost/f1000-capped500.yaml` | 2 | 76.32 | 353 | 1049 | 15304/421840 | pass (2026-10-26) |
| `scale/cost/f1000-range300.yaml` | 2 | 82.75 | 333 | 1267 | 16656/405304 | pass (2026-10-26) |
| `scale/cost/f1000.yaml` | 2 | 290.04 | 681 | 1267 | 16650/676764 | pass (2026-10-26) |
| `scale/cost/f300-abstract.yaml` | 2 | 72.61 | 213 | 453 | 8162/570508 | pass (2026-10-26) |
| `scale/cost/f300-range300.yaml` | 2 | 26.20 | 136 | 453 | 8171/93418 | pass (2026-10-26) |
| `scale/cost/f300.yaml` | 2 | 83.66 | 173 | 453 | 8171/155621 | pass (2026-10-26) |
| `scale/cost/f520-uncapped.yaml` | 2 | 124.43 | 221 | 520 | 9195/199944 | pass (2026-10-26) |
| `scale/cost/gn-hop1.yaml` | 10 | 2.39 | 44 | 40 | 2602/43984 | pass (2026-10-26) |
| `scale/cost/gn-hop10.yaml` | 10 | 3.20 | 43 | 40 | 2602/43984 | pass (2026-10-26) |
| `scale/cost/gnbtp.yaml` | 2 | 7.17 | 115 | 145 | 1454/9840 | pass (2026-10-26) |
| `scale/cost/mac-abstract.yaml` | 2 | 15.40 | 112 | 145 | 2833/18855 | pass (2026-10-26) |
| `scale/cost/mac-high.yaml` | 2 | 9.36 | 112 | 145 | 2834/18961 | pass (2026-10-26) |
| `scale/cost/margin20.yaml` | 2 | 13.80 | 148 | 145 | 2834/18961 | pass (2026-10-26) |
| `scale/cost/margin5.yaml` | 2 | 8.74 | 112 | 145 | 2834/18961 | pass (2026-10-26) |
| `scale/cost/meso1000-nofollow.yaml` | 2 | 105.29 | 235 | 1555 | 18650/51009 | pass (2026-10-26) |
| `scale/cost/meso1000.yaml` | 2 | 32.08 | 188 | 1555 | 8395/144327 | pass (2026-10-26) |
| `scale/cost/metrics-all.yaml` | 2 | 13.19 | 127 | 145 | 2834/18961 | pass (2026-10-26) |
| `scale/cost/nobuild.yaml` | 2 | 10.53 | 156 | 145 | 2832/225054 | pass (2026-10-26) |
| `scale/cost/phy-abstract.yaml` | 2 | 12.98 | 129 | 145 | 2834/18961 | pass (2026-10-26) |
| `scale/cost/pq-falcon.yaml` | 2 | 10.87 | 136 | 145 | 2561/16277 | pass (2026-10-26) |
| `scale/cost/pq-mldsa87.yaml` | 2 | 4.07 | 122 | 145 | 0/0 | pass (2026-10-26) |
| `scale/cost/prop-high.yaml` | 2 | 13.06 | 106 | 145 | 2834/22434 | pass (2026-10-26) |
| `scale/cost/radio-abstract.yaml` | 2 | 11.40 | 140 | 145 | 2833/209928 | pass (2026-10-26) |
| `scale/cost/radio-high.yaml` | 2 | 14.02 | 128 | 145 | 2834/22418 | pass (2026-10-26) |
| `scale/cost/range300.yaml` | 2 | 7.81 | 106 | 145 | 2834/11709 | pass (2026-10-26) |
| `scale/cost/rate5.yaml` | 2 | 7.97 | 136 | 145 | 1427/9718 | pass (2026-10-26) |
| `scale/cost/step50-rate20.yaml` | 2 | 27.43 | 106 | 145 | 5626/37115 | pass (2026-10-26) |
| `scale/cost/step50-rate5.yaml` | 2 | 7.56 | 139 | 145 | 1417/9694 | pass (2026-10-26) |
| `scale/cost/step50.yaml` | 2 | 17.41 | 106 | 145 | 2808/19055 | pass (2026-10-26) |
| `scms-audit-mbpipe.yaml (40 s overlay, no fast-forward)` | 65 | 4.57 | 45 | 5 | 2696/3370 | pass (2026-10-26) |
| `scms-conformance.yaml (40 s overlay, no fast-forward)` | 65 | 3.19 | 49 | 5 | 2696/3370 | pass (2026-10-26) |
| `scms-oracle-mbr.yaml (40 s overlay, no fast-forward)` | 65 | 5.18 | 45 | 5 | 2696/3370 | pass (2026-10-26) |
| `scms-oracle-repro.yaml` | 65 | 3.06 | 49 | 5 | 2696/3370 | pass (2026-10-26) |
| `scms-pq-relay-repro.yaml` | 40 | 10.59 | 117 | 133 | 6845/18194 | FAIL SC.arithmetic (2026-10-26) |
| `sensitivity/_cpl_probe.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/catalogue.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/dsrc-dense.yaml` | 3 | 8.58 | 81 | 107 | 2918/280747 | pass (2026-10-26) |
| `sensitivity/dsrc-grid.yaml` | 3 | 3.63 | 37 | 40 | 1195/41846 | pass (2026-10-26) |
| `sensitivity/gn-grid.yaml` | 10 | 4.22 | 45 | 40 | 2602/92675 | pass (2026-10-26) |
| `sensitivity/lte-grid.yaml` | 3 | 2.14 | 54 | 40 | 2308/44411 | pass (2026-10-26) |
| `sensitivity/nr-grid.yaml` | 3 | 2.50 | 39 | 40 | 1144/44536 | pass (2026-10-26) |
| `sensitivity/phy-channel.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/radio-cv2x.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/radio-dsrc-dense.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/radio-dsrc.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/radio-gn.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/test-mac-congestion-sidelink.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/test-mac-congestion.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/test-networking-dsrc.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `sensitivity/test-networking.yaml` | sensitivity spec (`v2xw sensitivity`), not a scenario | | | | | spec |
| `threshold-pca.yaml` | 65 | 4.13 | 52 | 19 | 6956/17687 | pass (2026-10-26) |
| `threshold-revocation-2of2.yaml` | 65 | 7.55 | 52 | 19 | 6956/17687 | pass (2026-10-26) |
| `threshold-revocation-3of5.yaml` | 65 | 4.23 | 52 | 19 | 6956/17687 | pass (2026-10-26) |
| `vru-grid.yaml` | 60 | 29.44 | 36 | 187 | 9982/43692 | pass |
