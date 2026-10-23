# Full-scale memory and honest reports at full density (2026-10-23)

Branch `wip/finalize4-mem` (`0fc63153`, the previous engineer's compaction commit on main
`d0e6530b`) and two commits on top (RNG `511410b9`, certificates `2b3b75d9`), plus the
benchmark's turn rule (§2.2). Every run is the full-scale benchmark
(`scenarios/real-world/midtown-scms-benchmark.yaml`, seed `0xBE7C_0001`) cut to a live
window with no fast-forward: `duration_s` 100 (or 80 or 20), `warmup_s` 10. "main" is the
release binary of `d0e6530b`; "b1", "b2" and "b3" are opt-level-2 debug builds of this
branch: `0fc63153` alone, with the RNG commit, and with both commits. Each run went through the machine's run lock under a
2.5 GB cap. The analysis scripts read the recordings' `node.tx` channel (claimed position,
speed, heading and `t_generated` of every BSM) and the run report. They are in the session
scratchpad (`mem5/hon.py`, `yaw.py`, `acc.py`, `prof.sh`), not committed.

## 1. Memory

### 1.1 Where the memory goes

These are live-heap snapshots of b1 (`MallocStackLogging=lite`, `malloc_history -callTree
-invert`, `vmmap -summary`), taken at 40 s of an 80 s window (the run was stopped there to free the machine: with stack
logging it ran at 26 wall seconds per simulated second).

| | main, 40 s | b1, 40 s |
|---|---|---|
| footprint | 860 MB | 593 MB |
| live heap (malloc zones, allocated) | — | 495 MB |
| `MALLOC_LARGE (empty)`: freed large blocks the allocator keeps | 288 MB of a 1,115 MB footprint (the previous engineer's snapshot of main) | 0 |

- **The retention was the allocator's large-block cache.** On main, freed large blocks
  (the 10 s metric windows' sample buffers, the per-step reception fan-out) stayed resident
  as `MALLOC_LARGE (empty)`. The previous engineer's `release_free_memory` (macOS
  `malloc_zone_pressure_relief`, every 10 simulated seconds, at most every 5 wall seconds)
  returns them. At 40 s the footprint falls 31 %, from 860 to 593 MB, for the same run.
- **The largest live structure is the SCMS backend's per-device state,** under
  `Phase2 → ScmsRun::preload_at`: 113 MB in `handle_provision` (RA request records, the
  butterfly cocoons, provisioning jobs, PCA issued records, RA batch files) and 58 MB in
  `handle_enrol` for 970 devices at 40 s. About 60 KB of the enrolment share is each
  device's own copy of the governance certificates (`DeviceTrust::roots` and `chain`,
  cloned from the CTL and LCCF per device). It grows with the devices provisioned (1,933
  at 240 s), not with time.
- **The per-link radio state** (the "per-frame arrival and faint maps, 53 MB" of
  `ccsig-memory.md` §2) is not the arrival maps. The 53 MB under `Engine::start_frame` were
  two per-directed-link maps that `start_frame` fills: the RNG registry's parked shadowing
  streams (`RngRegistry::checkout`) and the shadowing processes
  (`LogDistanceShadowing::shadows`). A frame's arrival and faint maps live only while the
  frame is in flight (108 frames at 230 s).

### 1.2 Changed (results unchanged)

- **Parked link streams** (`v2xw-core`, commit `511410b9`). A parked link's stream took a
  32-byte `(domain, EntityRef)` key and a 16-byte slot in its shard. It is now kept in a
  second map of the shard keyed by `(domain, packed link)`, holding the word position only:
  24 bytes an entry. The live budget counts both maps, so what is parked and when is
  unchanged, and so is every word a stream yields.
  - Tests: `a_parked_stream_continues_word_for_word` (400 rounds through the parked path);
    the `phase2-path` and `scms_scale` regression digests pass unchanged.
  - Fault: a resume that ignores the word position turns the test red.
- **Shared governance certificates** (`v2xw-proto`). Each device's trust store held its
  own decoded copy of every root and chain certificate it installed (`DeviceTrust::roots`,
  `chain`). The stores now hold an `Arc` to one copy per certificate, interned by the
  SHA-256 of its encoding (`shared_cert`), not by the 8-byte HashedId8, which a 2^32
  search can collide. What each device holds, refuses and verifies is unchanged.
  - Tests: `devices_share_one_copy_of_each_governance_certificate` (two bootstrapped
    devices hold the same `Arc` for every certificate); the 29 governance unit tests and
    the `revocation_path`, `provider_runs` and `deployment` tests pass; the `phase2-path` and `scms_scale` digests pass unchanged.
  - Fault: an interner that never reuses a copy turns the new test red.
- **The previous engineer's commit** (`0fc63153`: exact compacted metric samples, 16-byte
  parked streams, shared `Arc<HardwareProfile>`, allocator relief) compiles and keeps the
  `phase2-path` and `scms_scale` digests (b1, 13 + 1 tests).
- **End to end:** a 20 s full-scale run (`--no-recording --digest`, 783 nodes at 20 s)
  gives the same stream digest on main and on b2, `866d3101…`, so the relief slices and the
  parked-link map change no record. Its peak footprint falls from 567 to 405 MB (−29 %).
  The 30 s profile of main held 52 MB of link maps, about 0.75 million entries at an
  estimated 70 bytes each, past the 262,144 live budget, so a run of this size parks links; the digest does not show how many.
- **Not done, on purpose:** a quantile sketch for the latency windows. Every 10 s window's
  summary is min, max, p50, p95, p99 and a Neumaier sum over the sample in total order. No
  sketch gives those bits, so a sketch would move every published latency figure and every
  digest. The compacted store of `0fc63153` keeps them exact and removes the doubling and
  the clone at the window's close.

### 1.3 Measured

| Run | 40 s | 70 s | 100 s | peak |
|---|---|---|---|---|
| main, 100 s window (`base100`) | 860 MB | 1,581 MB | 1,978 MB | 2,074 MB |
| b3 (this branch) + turn window, 100 s (`b3w100`) | 509 MB | 717 MB | 979 MB | 1,027 MB |

- **The 100 s window's peak footprint halves: 2,074 → 1,027 MB** (b3 is the build of both
  commits; `/usr/bin/time -l` peak). The traffic differs slightly (the turn window, §2.2:
  1,490 against 1,491 nodes at 100 s), and b3 is an opt-level-2 debug build, not release.
- Most of it is the allocator relief of `0fc63153`: on the identical 20 s run b2 alone
  takes the peak from 567 to 405 MB with the same digest. The RNG and certificate commits
  remove live heap that grows with links and devices.
- **Against the 2.5 GB target.** main reached 2,690 MB at 150 s and 3,057 MB at 230 s of
  FB2. Scaled by the 100 s ratio this branch would sit near 1.3–1.6 GB there, but the
  150–240 s window and the resume were not run (§1.4), so the target is not yet shown
  met.

### 1.4 Still open

- **The backend's per-device state** (§1.1) is the next target: the RA's request records,
  cocoons and provisioning jobs, the PCA's issued records and the RA's encrypted batch
  files, kept for every device for the whole run. Dropping a batch file once the device
  has confirmed its download changes re-download behaviour, and a real RA's retention of
  the others is what the MA's investigation relies on, so each needs an owner decision.
- **The 240 s and resume peaks** (2,690 MB at 150 s, 3,109 MB at the 43,170 s resume on
  main) were not re-measured: one full live window takes about two hours of wall time.
  The resume rebuilds the fleet (1,833 → 1,281 nodes), and the relief call runs only at
  10 s boundaries, so a transient inside the resume step is not covered by it.

## 2. Honest reports at full density

### 2.1 What fires

The full-scale window, 0–100 s, main (`base100`):

| | value |
|---|---|
| BSMs sent (`node.tx`) | 465,937 by 894 senders |
| BSMs checked by `detect/us-sm-mbr` | 17,044,655 (942,075 unverified, not judged) |
| verdicts | 417: `implausibleAcceleration` 292, `positionJump` 123, `constantPositionFrozen` 2 |
| reports filed | 345, all about honest vehicles |

### 2.2 positionJump: receiver gaps across turns driven too fast

- **No pair as sent breaks the rule.** Over the 465,043 consecutive pairs each sender sent
  within 0.5 s, the RandomPosition residual `|p₁ + v₁·Δt − p₂|` has p99 0.064 m and a
  maximum under 1 m. So every `positionJump` verdict compares two BSMs that were not
  consecutive on the air: the receiver lost the ones between (channel load), or did not
  verify them (`prioritized`, 5.5 % of checks unverified, which the profile neither judges
  nor remembers).
- **Residual against the BSM m before, all senders:**

  | m (BSMs between, + 1) | pairs ≤ 0.5 s | over 1 m | p99 |
  |---|---|---|---|
  | 1 | 465,043 | 0 | 0.064 m |
  | 2 | 464,149 | 2 | 0.149 m |
  | 3 | 458,612 | 5 | 0.273 m |
  | 4 | 457,505 | 76 | 0.446 m |
  | 5 | 456,435 | 1,158 | 0.661 m |

- **Every pair over 1 m is a car turning.** The error is cross-track, at 7–10 m/s, with
  the heading turning 0.3–2.0 rad between the two BSMs. That is a lateral acceleration of
  8 m/s² and more: over a 0.5 s gap the straight-line prediction misses by `a·Δt²/2`, which
  passes 1 m only above 8 m/s².
- **The engine drove junction turns too fast.** Over all consecutive sent pairs, `v·|Δh|/Δt`
  has p99 4.5 m/s² and p99.9 7.6 m/s²; 329 pairs exceed 8 m/s² and 3 exceed 20 m/s². They
  cluster at 48 junction cells (20 m), and up to 72 m/s² at one (sender node 653 turns
  118° in 0.3 s at 7.2 m/s). The cause is the turn-speed rule. A connector is capped at
  `sqrt(5.5 m/s² · R)` for its *average* radius (netconvert's `--junctions.limit-turn-speed`),
  so its apex is driven at about 7.7 m/s² (85th percentile, `calibration/turning`) and a
  tight connector's apex much harder, beyond what a driver can hold on a dry road at the
  extremes. This is an engine defect.
- **Fix (scenario).** The benchmark now sets `traffic.turn_curve_window_m: 6`, the engine's
  own apex rule: the turn speed is judged over the tightest 6 m of the connector. Every
  scenario that inherits the benchmark gets it: the quarter fleet, LTE, Portland, the
  stress files and, through the quarter file, `midtown-scms-realism.yaml` and its 33
  matrix and realism children.
  - Quarter fleet, 0–40 s, main binary: lateral accelerations over 6 m/s² 187 → 29, over
    8 m/s² 90 → 17; `positionJump` verdicts 4 → 2.
  - Full fleet, 0–100 s (b3, against main): lateral accelerations over 8 m/s² 329 → 52;
    over-1 m residuals at lag 4 76 → 21 and at lag 5 1,158 → 155; **`positionJump`
    verdicts 123 → 35**; reports 345 → 174, all still about honest vehicles.
    `implausibleAcceleration` 292 → 196 (10 sender events against 12: the traffic moved),
    `constantPositionFrozen` 2 → 6.
- **Left (engine, open).** The pairs over 12 m/s² (11 of 18,152 at quarter density) are
  unchanged by the window. They are not junction turns: a through vehicle on a bent street
  (sender node 194 near Broadway, from heading −2.06 to −3.11 rad and back at 10 m/s)
  follows the lane polyline's vertices with no curvature limit. The turn-speed rule should
  also cover the vertices of a lane, or the importer should round them.

### 2.3 implausibleAcceleration: hard braking, sent

- 70 % of the verdicts. Over all sent pairs, only 12 break the 10 m/s² MaxAcceleration
  threshold (lag 1: 8, lag 2: 2, lag 3: 1, lag 4: 1), from six senders. Each is heard by
  about 25 receivers, and each receiver files its own report: one hard stop of an honest
  car puts dozens of independent reporters in front of the MA, more than the 3-reporter
  quorum.
- Two kinds:
  - **Emergency braking with GNSS speed noise.** Sender 720 brakes from 7.9 to 2.15 m/s in
    0.6 s (9.6 m/s² on average; 8.4–11.0 m/s² between consecutive BSMs). Cars brake at up to their
    class's 9 m/s² (`emergency_decel_mps2`) and mopeds and motorcycles at 10. The receiver's
    0.1 m/s-per-axis Doppler noise adds about 1.4 m/s² (one σ) to a 0.1 s difference, so an honest
    emergency stop at 9 m/s² crosses 10.
  - **A stop through the static hold.** Sender 874: 1.48, 1.39, 1.23, then 0.00 m/s 100 ms
    later. Either the car-following stopped the car in one step or the receiver's static
    hold (0.3 m/s) released the speed to zero. Both are receiver-output behaviour a real
    u-blox receiver also has.
- **Real property of the US rules (keep it measurable).** MaxAcceleration on two
  consecutive Doppler speeds, at 10 m/s², fires on an honest emergency stop. The rate in a
  run is the engine's emergency-braking rate: 12 events in about 25 vehicle-hours here. That
  rate is not calibrated against naturalistic driving data (open, engine), so the count is
  the engine's, not yet a real-world rate.

### 2.4 What is a property of the rules, and what was the engine

| Cause | Verdicts (0–100 s) | Engine defect or US rule | State |
|---|---|---|---|
| Junction apex driven at 8–45 m/s², judged across a 0.4–0.5 s receiver gap | most of 123 `positionJump` | engine (turn speed) | fixed by the scenario's apex rule: 123 → 35 |
| Through vehicles following polyline vertices | the rest of `positionJump` | engine (lane geometry) | open |
| Receiver gaps of 3–4 BSMs under channel load and `prioritized` verification | the condition for every `positionJump` | US rule: "consecutive" is read as consecutively *received* within `max_gap_s` 0.5 s (ASSUMPTION) | research: `max_gap_s` 0.15 makes it consecutive on the air |
| Emergency stops at 9–10 m/s² plus Doppler noise | 292 `implausibleAcceleration` | US rule (10 m/s² on two speeds) on an uncalibrated braking rate | research; braking rate open |
| `constantPositionFrozen` | 2 | — | R-FS40 fix holds at full density (165 in 40 s before) |

The 71 `positionJump` verdicts of the 40 s window in `measure.md` (an older binary) are
most likely this mechanism: in this run no sent pair breaks the rule.

### 2.5 Not checked

- The 240 s window's 1,935 reports (FB2) were not re-run. Density rises into gridlock after
  100 s (median speed 0 from 160 s), so more emergency braking can be expected there.
- `det.observation` and `ma.report` are not in an events-level recording, so the verdicts
  above are explained from the senders' side (no sent pair breaks RandomPosition; the
  over-1 m pairs at lag 4–5 are turns), not matched one by one to their receivers.
