# Build status

Living record of what is built and what has actually been *measured*, as against
the plan in `10-roadmap.md` and the decisions in `12-build-decisions.md`. Claims
here carry their evidence; anything unmeasured says so.

Last updated 2026-10-08 (the scale track section first, then the round-5 merge section, then the round-4 merge section; the stabilise track section; then the six-track merge section, then the road network and traffic behaviour sections of 2026-09-29; QA section of 2026-09-24 after them). **The crate table below is stale**: `v2xw-record`,

Last updated 2026-10-08 (the 3D scene track section first, then the round-5 merge section, then the round-4 merge section; the stabilise track section; then the six-track merge section, then the road network and traffic behaviour sections of 2026-09-29; QA section of 2026-09-24 after them). **The crate table below is stale**: `v2xw-record`,
`v2xw-metrics`, `v2xw-node` and `v2xw-engine` are no longer stubs, and the line counts
predate several waves. It is left as written rather than rewritten from memory, because a
status file whose numbers were re-estimated rather than re-measured is worse than one that
says it is out of date.

For the current release position — what must be true for a 1.0 tag, what is not true, and
what each gap would take — see [`docs/RELEASE-CHECKLIST.md`](../RELEASE-CHECKLIST.md)
(2026-09-22). The Phase 1 acceptance table below is still accurate and the checklist cites
it.

## 2026-10-11 — round-7 merge (speed, netfid, stalls)

- Merged `--no-ff` in order speed (d6dc2f3), netfid (bbe32d1), stalls (4ccb9b9) onto
  d3558de. Only conflict: this file; both track sections kept. `cargo fmt --all` as its
  own commit.
- Checked: `cargo check --workspace --tests` clean; lib tests of radio, net, node,
  mobility, metrics, engine, interop pass; `v2xw-engine --test b1_nlos --test
  unknown_signer` and `v2xw-cli` check_traffic tests pass. No UI changes. Release
  `v2xw-cli` bins built; `run-sim.sh` boots on 8758 with the agent on.
- Not run: full suites, security_lifecycle and threat suites (verify-all now drops
  unknown signers, so their counts may move), golden records with auto breakdowns.
- Still open: see the three track sections below (fleet speed ~15x slower than real
  time at 1,000 vehicles capped; short inter-signal links; `pedestrian_invariants`
  failing since d3558de).

## 2026-10-11 — large-fleet speed (fleet-speed track, branch of d3558de)

Optimised dev build (`opt-level 2`, debug assertions **on**), binaries copied out of
`target/agents` under the build lock, `v2xw run --no-recording --digest`, owner's 8 GB M2
shared with other agents (load ~3; wall times ±10 %). Scenarios: `scenarios/scale/base.yaml`
(Midtown box, DSRC, signed ECDSA, prioritized verification) with `metrics: [all]` and the
trip cap set to land the fleet: cap 560 → 520 vehicles, cap 1,260 → 1,054. "Capped" adds
`radio.range.max_m: 500`, the published cut `midtown-peak-load.yaml` already uses.

| run | vehicles | threads | wall s per sim s | peak MB | digest |
|---|---|---|---|---|---|
| uncapped | 520 | 1 / 2 | 31.9 / 21.9 | 291 / 314 | `33042a90…` both |
| uncapped | 1,054 | 2 | 71.7 | 860 | `35088f69…` |
| capped 500 m | 1,054 | 1 / 2 / 4 / 6 | 23.2 / 18.2 / 15.8 / 15.0 | 507-547 | `8661678b…` all |
| capped, after | 1,054 | 2 / 6 | 19.0 / 16.2 | 525-529 | `8661678b…` (same) |
| uncapped, after | 520 | 1 / 2 | 31.8 / 22.6 | 298-307 | `33042a90…` (same) |

**Where the time goes** (`sample`, 1,054 vehicles, two threads). Uncapped, every pair in
the box is a full link (LOS reach at 33 dBm exceeds the box's diagonal), so the
critical path is the parallel link geometry: building classification (`los_shared`,
`ring_crossing`, the R-tree walk) is 26 % of all CPU, and the main thread waits on it
67 % of the time. Capped, the critical path is the **serial main thread**, about 56 % of
it outside any parallel map, which is why 6 threads buy only 18 % over 2: per-reception
record emission and the CLI's stream digest (SHA-256, ~15 %), the metric providers'
JSON decode of node records (latency provider largest, ~7 %), one node wake per
reception (verification, ~9 %: frames arrive at distinct nanoseconds, so wakes cannot be
batched without moving them), the candidate walk (`equipped_within`, ~7 %, of which more
than half is the debug-assertion cross-check that a release build drops), and the
shadowing process (~5 %).

**Fixed:**

- perf — the log-distance shadowing draw is taken in the parallel per-link map
  (`LogDistanceShadowing::shadow_draw_shared`, the link's own keyed stream) with the
  geometry and the fade in **one** map; only the correlated state update stays serial
  (`loss_db_drawn`). Main-thread shadowing 510 → 64 samples; the wall gain is inside the
  noise at 2 threads. Same digests at 1, 2 and 6 threads. Test
  `prop.rs::a_shadowing_draw_taken_through_a_shared_borrow_is_the_one_loss_db_takes`
  (goes red when the shared draw keys the reverse link).
- perf — the CLI tally no longer allocates two `String`s per record (channel key,
  visibility); same digest bytes.

**Not reached:** 1,000 vehicles in real time. The work is quadratic by physics in a fixed
box (each of 1,054 radios hears ~200 within 500 m at 10 Hz: ~2 M reception attempts per
simulated second, each with records, metrics and a node wake), and the serial remainder
bounds the capped run at ~13-15× slower than real time on this build whatever the thread
count. Next levers, in order: move the stream digest and the metric providers' node-record
decode off the main thread (a channel to a hashing thread; the providers' predecoded path
for node records as for engine records), cache the parsed envelope per frame instead of per
receiver, and measure a release (`lto = thin`, no debug assertions) binary — the owner's
`run-sim.sh` build — which these numbers do not include.

## 2026-10-08 — round-6 merge (manual, after the spend limit)

The round's merge agent never ran (spend limit). All seven test-to-fix branches were merged by hand into main in the order scale, netsoak, trafficsoak, vrusoak, configsweep, scene, metricview. configsweep's agent stopped mid-track; its one uncommitted file (e2e-engine/owner-flow.spec.ts) was committed as a wip snapshot.

Conflicts:
- `scenario/mod.rs`: trafficsoak and configsweep both fixed trips lost on imported map boxes; connected demand now applies to a library city, a world naming a city's extract by path, and any OSM import.
- `vru/social_force.rs`: vrusoak and scene both fixed pedestrian jitter; vrusoak's smoothed per-person heading and proportional carry are kept, and a long corner carry still closes within `CARRY_CLOSE_MAX_S` (scene's rule). scene's unused facing constant was dropped.
- BUILD-STATUS sections: all kept.

Checked: release build of v2xw-cli (v2xw, v2xw-server) clean; protocol and viewer rebuilt; Studio typecheck passes; `run-sim.sh` starts with the agent on. **Not run:** any cargo test suite, vitest, Playwright, cargo fmt.

Open from the tracks: queue heads standing 8-27 s on their own green and some 160-180 s stops in Midtown rush hour (trafficsoak, not yet traced); Portland downtown spawning no vehicles (scene saw it; the demand fix in this merge may address it, unconfirmed); RSUs cannot verify post-quantum messages (no PQ cost in RSU profiles: owner's call); the agent client's /agent/* 404s fail e2e console checks against the mock server.

## 2026-10-08 — scale, speed and long runs (scale track, branch of 4986cd4)

Runs, not reading: the optimised dev build (`opt-level 2`, debug assertions on),
`v2xw run --no-recording --digest`, `RAYON_NUM_THREADS=2`, on the owner's 8 GB M2 while
other engineers' jobs and browsers kept its load average near 7, so wall times carry
±20 % noise. Ladder scenarios (scratch overlays): a 10 × 10 procedural grid with three
SPaT/MAP RSUs and every US set (`bsm, spat, map, srm, ssm`), and `scenarios/scale/base.yaml`
Manhattan; both signed (ECDSA, modeled crypto, prioritized verification), `metrics: [all]`,
the V2V apps on, fleets bulk-spawned by `max_total_vehicles`.

**A measuring pitfall, fixed for this track only.** The agents share one
`target/agents`, so `target/agents/debug/v2xw` is whichever worktree built last. Numbers
taken from it can be another branch's binary (this track's first table was, and is
discarded). Every number below comes from a binary copied out of the target dir under the
same build lock.

| run | vehicles | before: wall s per sim s | after | digest |
|---|---|---|---|---|
| grid, 10 s | 49 | 0.80 | 0.65 | same |
| Manhattan, 10 s | 49 | 0.55 | 0.49 | same |
| grid, 10 s | 197 | 4.40 | 4.44 | same |
| Manhattan, 10 s | 199 | 7.0 | 7.6 | same |
| grid, 2 s | 911 | 84.6 | 66.4 | same |
| Manhattan tiered (meso + 600 m micro box), 2 s, cap 4,800 | 1,539 | 7.9 | 8.3 | same |

Peak memory: 27 MB (grid-50), 108 MB (grid-200), 825 MB (grid-1000), 269 MB (tiered 1,539).
The 1 and 2 thread digests agree on grid-50 (`7caffe61…`); the other rungs were run at two threads only.
**Vehicles in real time:** about 100 all-microscopic in this dense, everyone-hears-everyone
ladder (wall ≈ sim at ~110 on the grid); the tiered city runs 1,539 at 8× slower than real
time with all messages and metrics, and a 6-hour tiered night (below) runs 42× faster than
real time. A 3,000-vehicle all-microscopic run was not attempted: at 1,000 it is 66× slower
than real time and the cost is quadratic in the fleet.

**Where the time goes** (`sample`, two threads). Grid-200: the engine thread was 38 %
record encoding and the metric providers' JSON decode, 36 % frame start (link geometry, in
parallel, then path loss, shadowing and fading in receiver order), 14 % node steps
(verification: COER parse, signature check); the rayon workers slept 78 % of the time.
Manhattan-1000: building shadowing is 38 % of all CPU (`ring_crossing`, `point_in_ring`
and the R-tree walk in `candidates_along`), already in the parallel half.

**Fixed:**

- `925f4ef` perf — each reception's `phy.rx`/`node.rx` records are built, encoded and
  decoded for the providers in parallel and emitted in receiver order where they were;
  Nakagami fades are drawn in parallel (`BoxedFading::shared`; single-use `(link, frame)`
  streams). Same digests. Main-thread time in `finish_phy_end` −13 % at two threads, grid-1000
  −21 % wall; more with more threads.
- `842f2c4` fix — the RA's shuffles now close at 10,000 items as well as at their window
  (CAMP-EE §2.2.7, SCMS-765; the 10,000 was published and never applied). Found on a 1-hour
  `manhattan-day`: 5,240 reports reached the proxy, **0 reached the MA**, so nobody was
  revoked, and with the one-day window a whole-day run cannot revoke anyone either. New
  lifecycle keys `cert_shuffle_threshold`, `report_shuffle_threshold`; test
  `lockstep.rs::the_report_shuffle_closes_at_its_count_before_its_window` (fails with the
  generation check removed).

**Long runs** (`--record-level metrics`, progress every 5 or 30 min):

- 1 hour of `manhattan-day` from midnight, all microscopic (attacker window moved into the
  hour): 131 vehicles, 455 s wall (8× faster than real time), peak 140 MB, 66 M records.
- 6 hours from midnight with the meso tier (`scale.traffic` tiered, 300 m radio margin):
  600 vehicles, 510 s wall, footprint 74 → 140 MB, tracking the fleet's growth towards
  06:00 rather than the clock. Stores that grow with every vehicle *ever seen*, not with the
  live fleet: the SCMS's per-device records and the engine's `p2.creds` (40 certificates a
  device, 22,800 at 6 h for ~30 live vehicles), `p2.by_digest`, `p2.privacy_*`, and the
  SCMS kernel's stage and wire logs (11,598 and 7,732 at 6 h). At a day's demand
  (~20,000 vehicles) the credential maps reach ~800,000 entries. Left as they are: the
  backend's issuance history is what a revocation needs, and the engine's copy is keyed for
  reports that arrive after a vehicle left; trimming it is a modelling decision, not a leak
  fix. `p2.in_flight` is now bounded by the 10,000-report shuffle.

**Not done:** the live server feed and the Studio at the busiest scenario (no browser run);
the shadowing process (`LogDistanceShadowing::loss_db`, 8 % serial at 200 vehicles) and
node steps are still serial; building shadowing (`ring_crossing`) was not made cheaper;
`settle`'s per-top-up scan of the whole SCMS wire log (`phase2.rs`, `kernel.steps.iter()`)
is quadratic over a long run with many top-ups, unmeasured.

## 2026-10-08 — network stack soak: run, find, fix (netsoak track, branch of 4986cd4)

Headless runs of the release engine (`v2xw run --no-recording`, optimised dev build), 15 s to
120 s each, every one re-run after its fix. Every number below is from those runs.

| Run | Configuration |
|---|---|
| soak-rush | Midtown rush hour, 30 s, DSRC; and the same on LTE-V2X and NR-V2X, and with DCC off |
| soak-peak | Midtown "busiest rush hour" (`midtown-peak-load.yaml`), 40 s, ×4 demand: 2,689 actors, 2,200 of them pedestrians and riders |
| pq-* / pqs-* | `pq-signatures.yaml`, 120 s, ECDSA P-256, ML-DSA-44, Falcon-512, hybrid ML-DSA-44 + P-256, with `metrics: [all, signing]` |
| pqmid | `post-quantum.yaml` (Midtown, hybrid Falcon-512 + P-256, 8 RSUs), 40 s |
| eu-gn | Berlin, ITS-G5, CAMs + DENMs over GeoNetworking on-air forwarding, breakdown at 10 s, ×4 demand; and with DCC off |
| att-* | fake positions (ConstPos + RandomPos, 10 %), replay, jamming at the busiest junction |

**Fixed (each its own commit, with the measurement in the message):**

1. **LTE-V2X PDR was exactly half.** Within 100 m, 0.500 in every 20 m bin, at 1-2 % CBR,
   while `nar` at 300 m was 0.97. The PRR census (TR 36.885 §A.2.1.4) was taken per copy of
   a blind-retransmitted block, so a receiver that decoded copy 1 counted as a failure of
   copy 2. A block is now censused once, at its first copy, receiver by receiver, and
   reported when it is resolved. After: 100 m 1.00, 300 m 0.90. DSRC and NR (one
   transmission) unchanged. `run.rs`, `run/sidelink.rs`.
2. **Age of information ~700-800 ms in Midtown peak on DSRC was a metric artefact, not
   the channel.** The channel was at CBR 1-2 % and PDR 0.76 within 100 m, so neither
   saturation nor DCC throttling. Three causes, measured:
   - `aoi` pooled every link out to the 1 km candidate range. Around Manhattan's corners a
     link is heard once in a while, and each gap's area grows with the square of the gap.
   - It pooled the 500 pedestrian PSM devices (1 Hz, SAE J2945/9) with the 10 Hz BSMs.
     `nar` counted pedestrians as receivers and neighbours. `pir`'s p95 of 1.0 s was their
     gap.
   - A gap's whole area was filed in the window where it closed, so window values jumped
     between 70 and 1,400 ms.

   **Chosen definition.** The headline is now each vehicle's neighbourhood age: Kaul et
   al.'s system age (SECON 2011), over the vehicle pairs truly within **100 m**. 100 m is
   SAE J2945/1's vPERRange, the range that standard judges PER and IPG over, and the same
   standard's 600 ms vMaxITT is the health band. The age is integrated exactly between
   mobility steps. It is not sampled at them, because beacons are generated on the step
   boundary and a sample would read the peak (102 ms, not 52 ms). The breakdowns are
   `300m`, `all` (the old every-link figure) and class `vru`. `nar` is vehicles only.
   `pir`'s headline excludes PSM/VAM senders, which get their own `vru` sample.

   After, on soak-peak:

   | Metric | Value |
   |---|---|
   | aoi, 100 m | 82 ms |
   | aoi, 300 m | 894 ms |
   | aoi, all | 430 ms |
   | aoi, vru | 931 ms |
   | pir p95 | 0.20 s |

   Theory for 100 m: 50 ms + 12 ms latency + losses. The 300 m figure is set by corners:
   most pairs that far apart are NLOS and heard rarely, and each adds age until the 10 s
   restart drops it. The vru figure is a 1 Hz PSM at PDR 0.76.

   On soak-rush, aoi is 67 ms on DSRC, 80 ms on NR and 108 ms on LTE-V2X. LTE's SPS waits
   about 50 ms for its reserved subframe (`mac_access_delay` 55 ms). That matches the
   J3161/1 100 ms selection window.
3. **`flow`, `density` and `mean_speed` never had a value in any run.** These are three
   of the "59 of 90 measured". Edie's aggregates need lane lengths, and nothing ever
   called `declare_lane_length`. The run now declares every Driving, Bus and Internal
   lane. The headline is the network's: the macroscopic fundamental diagram per lane-km
   (Geroliminis and Daganzo 2008). Per-lane samples are off in a run, because on they
   tripled the peak run's metrics.json from 38 MB to 108 MB.

   Pedestrians walking in groups on sidewalk lanes had filled `headway_*` and `ttc_min`
   with metre gaps. They are now left to the VRU provider. Peak headway p50 went from
   1.2 m to 11.5 m. Density is 1.7 veh/km/lane and space-mean speed 3.05 m/s.
4. **The unexplained 3.6 % of `sig_verify_dropped` under post-quantum signatures.** It
   was 4.4 % in pq-signatures and 41 % in post-quantum. It was never the verifier: queue
   depth p95 was 0 and headroom 150×. Recording node.verify showed that every drop was at
   a roadside unit. All 8 RSUs dropped 100 % of messages, as `verify_overflow`.

   The cause is the shipped RSU profile, `rsu/commsignia-its-rs4`. It publishes only an
   ECDSA P-256 accelerator, so it prices no PQ verification. Its verify queue therefore
   never drained, filled, and the policy reported "queue full" for everything after. A
   node that cannot price its verify op now drops the message as `verify_unpriced`, and
   counts it with the policy skips. `sig_verify_dropped` keeps its meaning: the verifier
   failing to keep up. `v2xw-node` `runtime.rs`, `policy.rs`.

**Checked and found plausible, no change:**

- DSRC range in Manhattan is much shorter than LTE-V2X: `nar` 300 m 0.54 against 0.97,
  with 99 % of DSRC losses below sensitivity. The link budget explains it: about 8-10 dB
  in LTE's favour (narrower subchannels and HARQ combining), against a NLOS street grid.
- `unverified_ratio` is 5 % under ECDSA and 9 % under ML-DSA-44 with digest-only signer
  ids (a full certificate every 1 s). These are receivers that have not yet heard the
  certificate. Under PQ the certificate-bearing message is larger and is lost more often
  in fragments (`frag_sdu_loss` 0.82).
- The detection metrics (`det_*`, `time_to_detect`) are empty in a 40 s attack run.
  156 verdicts fired and 102 reports were sent, but the base calendar's RA report
  shuffle is 60 s, so none reach the MA before the run ends. A run needs to be longer
  than 60 s.

**Open — not fixed:**

- **RSUs cannot verify any post-quantum message.** No RSU profile prices a PQ op, and
  nothing refuses the scenario. Their misbehaviour-report and relay roles act on nothing
  under PQ. Fixing it needs either a published (or flagged-proxy) PQ cost and
  `crypto_anchors` for the RSU profiles, or a validation refusal.
- **Verify-all delivers some messages unverified.** Under the `verify-all` policy, a
  message whose signer certificate is unknown is still delivered to applications as
  `Unverified`, although the policy card says nothing unverified reaches an application.
- **The DENM multi-hop latency looked worse with DCC on.** On eu-gn the p50 was 3.1 s
  with DCC on and 0.17 s with it off, with reach 11/11 and 10/11. This is from 11
  receivers on one seed, so it is not established. It needs several seeds before it is
  read as a DCC effect.
- **The `pir` and `aoi` health bands are J2945/1's (10 Hz BSM).** CAMs follow the
  dynamics rules (1 to 10 Hz), so on the ETSI stack a pir p95 near 0.8-0.9 s is correct
  behaviour and is still flagged.

## 2026-10-08 — pedestrians and two-wheelers: run, find, fix (branch of 4986cd4)

The owner's report: "pedestrian movement is not smooth". Measured, not read: a new
diagnostic, `crates/v2xw-engine/tests/vru_motion.rs` (`--ignored`; `V2XW_SCENARIO`,
`V2XW_SECONDS`, `V2XW_WATCH=<ids>`, `V2XW_LANES=<ids>`), runs a scenario's mobility alone and
reports, from the positions the stream would carry: jumps, published-speed mismatch, heading
turns and standing spins, reversals, accelerations over 3 m/s², lateral jerks, the
walking-speed distribution, stops, and pedestrians closer than 0.35 m (by group, lane and
activity). A 60 s Midtown run takes 1-3 s; the build is what costs.

The cause was the engine, not the viewer: the viewer's Catmull-Rom paths and 5 rad/s turn
limit were hiding what the engine sent.

| Measured (vru-grid 60 s / midtown-pedestrians 40 s / midtown-street-life 60 s) | 4986cd4 | this branch |
|---|---|---|
| heading turns over 30° in one 0.1 s step, per 1000 steps | 10.5 / 13.3 / – | 0 / 0 / 0 |
| standing spins, worst | 180° / 180° / – | 23° (the turn-rate limit) |
| published speed off the displacement, per 1000 | 0.3 / 28.1 / – | 0 / 0 / 0 |
| jumps, per 1000 steps; worst | 0 / 0.44, 8.2 m / 0.42, 8.1 m | 0 / 0 / 0.00, 0.49 m |
| worst acceleration, m/s² | 20.7 / 799 / 791 | 13.9 / 16.2 / 50.6 |
| steps over 3 m/s², per 1000 | 24 / 55 / 53 | 17 / 25 / 29 |
| pairs closer than 0.35 m (summed over steps) | 16,197 / 46,530 / 192,869 | 1,856 / 6,862 / 33,765 |
| walking speed mean, s.d. (m/s) | 1.19, 0.40 / 1.31, 1.33 / 1.25, 1.26 | 1.29, 0.31 / 1.24, 0.36 / 1.20, 0.37 |
| cyclists: worst step, acceleration | – / – / 1.06 m, 92 m/s² | – / – / 0, 6.9 m/s² |

Fixes (one commit each, symptom, cause and fix in the message):

1. **Facing and velocity** (`1ce6e1e`). The published heading was the social-force velocity,
   whose direction is the last push once a waiting walker's speed decays: people spun at the
   kerb. The stream now carries the displacement as velocity and a facing that turns at most
   4 rad/s toward where the walker actually goes, holds while standing and turns to face the
   crossing (as SUMO derives a walker's angle from its movement, `MSPModel_Striping`).
2. **Spread, groups and gaps** (`b92fba8`). The kerb potential was measured from the body's
   edge, not the walker's position (Helbing & Molnár 1995 eq. 6), 3.3× too strong, so it
   herded everyone and every group onto the centreline. Walkers now keep a place across the
   pavement (groups abreast, Moussaïd et al. 2010; lone walkers a drawn share of the width),
   moving across at 0.4 of their pace (SUMO `LATERAL_SPEED_FACTOR`). The Midtown walk network
   has 3-3.5 m gaps between sidewalk stubs; a lane is now extended back along its first
   segment so a walker walks the gap, and routes skip successors more than 5 m away.
3. **Lane-change offset** (`420b32a`, vehicle engine). A lane change started from the two
   lanes' half widths summed; a bike lane past a buffer is further away, and a cyclist leaving
   it jumped 1 m. It starts from the actual position projected across the target lane.
4. **Corners, kerbs, bends, contact** (`e7bb0dc`). Look-ahead steering 1 m before a lane's end;
   the true lateral offset kept round a corner (no carry closing at a fixed 0.7 m/s, which was
   7 m/s² at every crossing's end); a 3 m/s² stop cap at the kerb and stepping off when caught
   too close by the end of WALK; the polyline-bend projection mismatch carried instead of
   jumped; one walker's push on another capped at `V0/σ` (the elliptical potential diverges
   as `b → 0`; cf. the generalized centrifugal-force model, Chraibi et al. 2010).

**Gates.** `cargo test -p v2xw-mobility --lib vru`: 19 pass. `pedestrian_invariants`:
already failing at 4986cd4 (vehicle-vehicle `overlap = 21` in the signalised run, and the
observed run); on this branch the signalised run passes and the observed run fails on
`gap-below-minimum = 21, speed-jump = 3, accel-bound = 3`, all vehicle checks.
`traffic_invariants` pass.

**3D view check.** Both engines (4986cd4's sources and this branch, built from the same
tree) served `midtown-pedestrians.yaml` on a spare port; `scripts/capture-stream.mjs` captured
60 s of each stream (662 frames) and `test/glitch-capture.test.ts` replayed both at 60 fps
through the same tour (aerial, then chase over a car, a delivery van, a pedestrian and a
cyclist; 2,374 drawn frames each, warm-up 20 s). BEFORE: `teleport = 11` (all the engine's:
"pedestrian moved 6.14 m in one 17.0 ms frame at 1.5 m/s") and `stutter = 40` ("pedestrian
jerked 2.1 px on screen moving 2.4 px/frame"). AFTER: every class 0. A headed Studio run
(Playwright on a GPU) was not done: headless SwiftShader draws this scene at 4-8 fps, too slow
to judge motion (see the round-4 notes above).

**Open.**
- About 2-3 % of walking steps still exceed 3 m/s² with nobody within 1 m. Not traced yet;
  suspects are the crowd-speed cap (`CROWD_SPEED_CAP`, a hard clip when the Weidmann factor
  steps) and border pushes on narrow lanes.
- Groups of four on a narrow pavement still overlap: the line abreast is clamped to the
  walkable width; a V or river-like formation (Moussaïd 2010) needs a longitudinal slot.
- Pedestrians wait the whole of a 40-60 s run at some Midtown kerbs; whether that is the
  signal plan or a missing walk phase is not checked.
- Spawns are uniform over sidewalk lanes and despawns happen where a 12-lane walk ends,
  mid-pavement; the viewer fades them, but people still appear from nowhere.

## 2026-10-08 — the 3D scene, tested in the real browser (scene track, branch of 4986cd4)

Test-to-fix: the real engine (`v2xw-server`, optimised dev build) on Midtown rush hour,
pedestrians, peak load, the highway, Portland downtown and fake-position attack, with the
Studio in GPU Chromium (Playwright, `channel: "chromium"`, ANGLE Metal on the M2; the default
headless shell is SwiftShader and ran at about 1 frame a second, which made every glitch count
meaningless). Each pass screenshots Map, Chase, Driver's view and Free, runs the viewer's glitch
hunter, and an independent probe reads every drawn pose per animation frame (per-actor jerk,
jumps, motion against the drawn heading, camera jerk) and the engine's own pose stream as the
interpolator receives it. Scripts: the session scratchpad's `scene3d/` (`tour.mjs`,
`probe.mjs`, `raw.mjs`).

**Fixed, each re-measured:**

| Symptom | Cause | Fix | Before → after |
|---|---|---|---|
| pedestrians hop across corners (owner: "pedestrian movement is not smooth") | a lane change whose landing point was > 5 m away was snapped; the avenues' wide pavements put every corner at 5.4–6.9 m | `CARRY_MAX_M` 12 m; a long carry closes in at most 4 s | engine pose stream, midtown-pedestrians, 25 s, 132k steps: 65 jumps → 0 |
| pedestrians whirl on the spot at the kerb | heading = velocity direction whenever \|v\| > 1 µm/s | facing = path direction turned by `atan2(v_lat, max(v_along, 0.1))`, SUMO `MSPModel_Striping::PState::getAngle` | headings turning > 143° in a step: 940 → 106 (the rest are route-end turn-rounds) |
| black wedge over the followed car in a queue | the chase camera (≈7 m back) stood inside the car behind | a vehicle whose body box contains the camera is not drawn | rush-hour chase at a red light: subject in full view |
| street views look like night with lighting on Day | dark-theme world palette (navy sky, near-black asphalt, blue-grey facades), no light lift at street level | street daylight in the dark theme: day sky gradient, pale-sky/warm-ground hemisphere, 1.6× lift fading with the sun, warm facade gain; fog fades into the sky actually painted | chase/driver/free on rush hour, peak load, highway: blue sky, grey road and facades |
| map vehicles are blobs with no direction | round dot in a 1.55× dark halo | location-arrow pointer along the heading (SUMO GUI's triangle at overview zoom), 1.3× halo; legend matches | map on rush hour, pedestrians, attack |
| Portland's map is grey 20–30 m discs on a wire grid | junction disc sized by connector count | sized by how far the junction's own connectors reach (≤ the old size); a junction with none gets 4 m | Portland and Midtown map crops |
| floating heads | neck too short by 6 cm | neck spans torso top to head base | pedestrian chase |
| "Following vehicle 1" over a pedestrian; "No radio selected" while one is selected; "Following" in the free camera | chip and HUD assumed a vehicle with a radio | class noun from the Hello class table; "Selected" in free/roadside; HUD says "carries no radio" or "waiting for its first report" | pedestrian and highway screenshots |
| console warning on every load | three r186 removed `PCFSoftShadowMap` | `PCFShadowMap` | no warnings |

**Measured and fine (no change):** vehicle motion is smooth in every view — the drawn-pose
probe found 0 jumps, 0 backwards steps and 0 heading snaps over 51k vehicle-frames at peak
load (2,300 actors), jerk RMS 2.7 mm/frame; frame time at peak load is 0.7 ms CPU, p95
2.2 ms (M2, 1440×900); the render clock tracks the wall clock to 0.5 % with no long frames.
The chase and driver cameras are smooth once landed; the only camera jerk the probe saw is
frame-time jitter during the fly-down. The glitch hunter's own counts of "teleport" in the
first frames of a hunt come from its own start-up stall, not the scene (the independent
probe sees none).

**Not fixed, recorded:**

- **Portland downtown spawns no vehicles at all** (0 in 60 s, headless `v2xw run` and in
  the browser; the round-4 suite already recorded "at most 6"). The demand produces trips;
  they are refused at insertion, almost certainly as unroutable on the one-way grid. A
  traffic/demand fix, not attempted here.
- Pedestrian "backwards" steps (84 in 25 s, 0.06 %) and sideways ones remain in the
  engine stream: social-force sidesteps; the drawn facing no longer flips with them.
- The glitch hunter flags spawns in view at distance (`pop`, engine-caused) — vehicles
  entering at the map's edge lanes inside the frustum; not addressed.
- Not run: the Playwright e2e suites; the full viewer and Studio unit suites (the four
  viewer test files touching the changed code pass: 38 tests).

## 2026-10-07 — round-5 merge: eight tracks onto main (settings, agent, radio, network, traffic)

Merged with `--no-ff`, in this order: netconfig, radio, netstack, cellular, traffic,
trafficcal, settingsui, agent. No track's work was left unmerged. Each track's own
section follows this one.

**Overlaps reconciled into one settings surface** (the owner asked for clean, organised
settings, so no setting appears twice):

| Overlap | Kept | What changed |
|---|---|---|
| radio's `radio.tuning.*` against netconfig's `radio.phy`, `radio.channel_model`, `radio.sidelink` | netconfig's paths | `radio.channel_model` gains `nakagami_shape_far`, `nakagami_breakpoint_m`, `building_wall_loss_db`, `building_loss_db_per_m`, `vehicle_blockage`; `radio.sidelink` gains `candidate_percent`, `selection_window_ms`; the receiver numbers are `radio.phy`'s. The shadowing override reaches every law with shadowing (radio's mechanism); the exponent is netconfig's single-slope log-distance tuning; the `fading` word feeds radio's Nakagami profile. Radio's validation rules (exponent under another named law, shadowing on a deterministic law, Sommer terms with buildings off, NR-only candidate shares, LTE's 20 ms T2 floor) now name the merged paths. |
| netstack's `net.mac_queue_depth` against netconfig's `radio.mac.queue_length` | `radio.mac.queue_length` | one queue-depth setting |
| netstack's `net.access_categories` against netconfig's `radio.mac.periodic/event_access_category` | both | `etsi-profiles` maps by ETSI DCC profile; otherwise the two `radio.mac` words choose the queues, and a forwarded copy is event traffic. Naming both is refused. |
| netconfig's `actors.vehicles.driver` against trafficcal's `actors.vehicles.drivers` | `actors.vehicles.drivers` | one Drivers section: realism, spread, reaction time, perception error, truck lane restriction, motorcycle filtering, and the passenger-car IDM `a`, `b`, `T`, `s0` and MOBIL politeness and threshold |
| settingsui's BSM-only Message rate quick setting against netconfig's `messages.rate_hz` | `messages.rate_hz` | the quick setting offers Standard, 20, 10, 5, 2 and 1 Hz and now sets the CAM as well as the BSM |
| trafficcal's Drivers quick setting against settingsui's grouped quick list | both | Drivers sits in the Traffic group; scenario and city stay out of the quick list |

**Fixes made at merge:**

- **GLOSA following.** The traffic track turned strategic lane choice on by default. With
  it on, the ego's lane match wavers on the approach, the GLOSA application gives no
  advice for a step or two, and the follower's speed cap was lifted at once.
  `applications.rs` measured 0 of 77 slowed advisories followed. It passed at 7bd8d95 and
  failed on the traffic branch alone. A follower now holds the cap through up to 1 s of
  missing advice (`GLOSA_HOLD_S`, this build's choice). Advice that says "no target"
  still lifts the cap at once. After the fix, `energy_glosa` on the arterial measured
  stops/km −20.2 % and HBEFA 3 fuel −1.1 %. Before the fix fuel rose +0.5 % and the test
  failed.
- **Settings window.** The traffic track's top-level `traffic` section fell into a second
  "Traffic" group, which showed Traffic twice and caused a React duplicate-key error. It
  now files under Traffic as "Signals, rerouting and gridlock". C-V2X scheduler fields
  (`radio.sidelink.*`) and `net.access_categories` now file under Medium Access.
- **Tests.** In `radio_access`, WINNER+ B1 now ships, so the test's unshipped-law example
  is `propagation/itu-r-p1411`. In the server `rpc` test, the no-duplicates check
  compared against 33 while there are 34 distinct methods. That check already failed
  before this merge.

**Verified:**

- `cargo check --workspace --tests` is clean.
- `cargo test -p v2xw-engine`: every test passes on the merged tree. The full run had 3
  failures: `applications` GLOSA, `energy_glosa` and `radio_access`. Each was fixed as
  described above, and those three test files passed on a rerun.
- `cargo test -p v2xw-server`: one failure (`rpc`), fixed and passing on a rerun.
- Studio: protocol and viewer `tsc -b`, the `@vwp/studio` typecheck, and vitest (28
  files, 282 tests) all pass.
- `cargo fmt --all` was committed on its own.
- The release `v2xw` and `v2xw-server` built.
- `V2XW_PORT=8758 ./run-sim.sh` started. The Studio rendered against it in Chromium
  through Playwright with no console errors:
  - the header shows the Metrics button with no PDR/CBR and the city on one line;
  - the settings window shows 12 sections and 271 settings;
  - the agent panel shows its one-line prompt and three suggestions.

**Not run:** the Playwright e2e suites; the full test suites of the other crates
(`v2xw-copilot`, `v2xw-mobility`, `v2xw-radio`, `v2xw-net`); a browser pass through the
quick settings, the Undo control on the agent's setting chips, or a live run with alerts.
Each track's own "not done" list stands in its section below.

## 2026-10-07 — researcher-grade configuration, engine side

New validated, published scenario fields, each read by the run. They are listed with
defaults, ranges and sources in [`docs/guides/configuration-reference.md`](../guides/configuration-reference.md).

| Section | Fields | Wired into |
|---|---|---|
| `radio.phy` | `data_rate_mbps`, `noise_figure_db`, `sensitivity_dbm`, `cca_threshold_dbm`, `capture_threshold_db` | `wiring::build_phy`, `wiring::safety_mcs` (replaces `run.rs`'s `SAFETY_MCS` constant) |
| `radio.mac` | `periodic_access_category`, `event_access_category`, `cw_min_vo/vi`, `aifsn_vo/vi`, `queue_length` | `EdcaOcbMac::with_edca`, the frame's `ac` (replaces `SAFETY_AC`) |
| `radio.channel_model` | `path_loss_exponent`, `shadowing_sigma_db`, `shadowing_decorrelation_m`, `fading`, `nakagami_shape`, `rice_k_db` | `LogDistanceTuning`, `NakagamiFading::with_m`, the candidate-range law |
| `radio.dcc` | `enabled`, `cbr_target`, `adaptive_alpha`, `adaptive_beta`, `max_itt_ms`, `density_coefficient`, `power_control`, `min_power_dbm` | `wiring::build_dcc`, `AdaptiveDcc::with_params`, `SaeJ2945Dcc::retuned` |
| `radio.sidelink` | `reservation_interval_ms`, `resource_keep_probability`, `rsrp_threshold_dbm`, `sensing` | `SidelinkAccess::for_scenario` over the profile's `SpsParams` |
| `actors.vehicles.drivers` | `max_accel_mps2`, `comfort_decel_mps2`, `time_headway_s`, `min_gap_m`, `politeness`, `lane_change_threshold_mps2` | `NativeMobility::with_driver_override` (passenger cars), MOBIL params |
| `messages.rate_hz` | fixed BSM/CAM rate | `wiring::generator_params`, J2945/1 `vMinITT` |
| `time.warmup_s` | warm-up | `on_metric_flush` records no sample inside it |

Each field has a `BOUNDS` or `CHOICES` row, a `KEY_STATUS` row, a title, an
explanation and a recommended range in `guide.rs`. `time.warmup_s`,
`radio.phy.data_rate_mbps`, `radio.channel_model.fading`, `radio.dcc.enabled`,
`messages.rate_hz` and `actors.vehicles.drivers.time_headway_s` are `basic`; the rest
are `advanced`. The loader refuses the combinations where a setting would do nothing,
for example 802.11p numbers on a sidelink run, ETSI numbers under J2945/1, or EDCA
numbers at the abstract MAC tier. None of the new fields is serialised when unset, so
no existing scenario's hash moves.

- **Verified:** `tests/researcher_config.rs` has nine tests, all green. Every area moves
  a metric against a same-seed baseline, and the loader refusals are checked. The engine
  lib `scenario` tests have 29 green, including `every_leaf_has_a_status`,
  `every_number_publishes_a_unit` and `the_tables_name_real_fields`. The `v2xw-radio` lib
  has 326 green.
- **Shared-target pitfall:** the engine's build script writes `scenario_reflect.rs` into an
  `OUT_DIR` that every worktree sharing `target/agents` overwrites. If another worktree
  builds in between, the publish tests then see that worktree's schema. Touch
  `schema.rs` inside the heavy lock before running them.
- **Not done:** the agent's tool list and the Studio's quick settings were not changed;
  they read the published surface. 20 MHz 802.11p, NR numerology, sidelink mode 1 and a
  bounded certificate cache are not modelled; the reference lists why. The full engine
  and server suites were not run.
## 2026-10-07 — network stack on the channel (netstack track)

Closes the round-4 gap "forwarded GeoNetworking copies have no MAC".

| Piece | Where | What runs |
|---|---|---|
| On-air GN forwarding (`net.geonet.forwarding: on-air`, new default) | `engine/src/run/gn_air.rs` | an EN 302 636-4-1 router per RSU/OBU inside the run; DENMs originate as GBC to `area_radius_m`, everything else as SHB (CAMs fill the LocT, so no beacons, §10.3.6.2); every decoded frame goes to the receiver's router; a forward is a new frame (same SPDU, router's packet) in the relay's MAC pending list; a CBF candidate is asked again at its timer and cancelled if a copy was heard; then DCC gate, EDCA, collisions, CBR and `node.tx` like any frame. `overlay` keeps the old instant run. `RunReport.gn_air` totals; `RunReport.geonet` rows are filled at the end of the run from the first DENM after the hazard |
| ETSI DCC profiles on EDCA (`net.access_categories: etsi-profiles`) | `gn_air.rs::access_category`, `FrameDescriptor.ac` now honoured by the MAC and the DCC gate | TS 102 724 DP0-DP3 -> EN 302 663 AC_VO/VI/BE/BK: hazard DENM VO, CAM/BSM BE, forwarded copies and the rest BK |
| Per-AC queue limit (merged into `radio.mac.queue_length`, 1-1024, default 64) | `wiring::build_mac` | `EdcaOcbMac::with_queue_depth`; overflow is a MAC drop |
| GN-layer global CBR (`net.dcc_cbr: global`, gn-btp only) | `engine/src/run/gcbr.rs` | TS 102 636-4-2 §5: CBR_G = max(own, neighbours' R0, neighbours' R1) feeds the DCC; `RunReport.gcbr` |
| CPM figures (`RunReport.cpm`) | `engine/src/run/cpm_metrics.rs` | object awareness within 150 m (awareness messages alone vs with CPMs), redundancy, payload size p50/p95/bins, generation rate per station |
| Air ledger (`RunReport.air_bytes`) | `engine/src/run/air_bytes.rs` | octets by layer for every frame on the air, PHY preamble time, forwarded-copy octets |
| CSMA/CA in the dissemination run (`DissemConfig.mac`) | `net/src/stack/dissem.rs`, `eval.rs::compare_with` | carrier sense, AIFS + CWmin backoff, collisions, half duplex |

All new options are validated, have labels and choice words in `options.rs`, and publish
through the reflection (no Studio change needed).

**Measured** (each test run once):
* Connected-intersections, gn-btp, breakdown at 8 s, on-air GPC CBF: 11 of 11 equipped
  vehicles in the 1 km area reached, p50 / p95 101 / 167 ms, 2 hops, 71 transmissions
  (68 forwards) for 3 DENMs; 100 CBF timers armed, 32 stood down; DENMs went on the air
  from 33 stations against 1 with the overlay. Air ledger of that run: payload 18.8 % of
  PSDU octets (security 48 %), forwards 4 %.
* Same grid, 8 s, no hazard: `etsi-profiles` raises mean access delay per grant from
  2.2 µs to 5.7 µs (CAMs on AC_BE); the global CBR raised the DCC input in 370 of 691
  updates (max +1 %, a light channel).
* CPM run (40 % equipped, 40 pedestrians without devices): awareness 3.6 % from CAMs
  alone vs 5.7 % with CPMs; redundancy 1.78 reports per receiver-object-second; CPM
  payload p50 68 B, p95 159 B; 1.19 CPMs per station per second.
* Amador et al. highway, 10 veh/km/lane, seed 7: with CSMA, ETSI / DPD / GPC PDR 1.000 /
  0.667 / 1.000 (published 0.9998 / 0.9500 / 0.9917; without a MAC 0.787 / 0.667 /
  1.000); transmissions per DENM 200.8 / 6.6 / 17.2 (published ÷ 30: 275 / 16.2 / 19.9).

**Not done / caveats.** GeoUnicast is sent once with no link-layer ACK/retry; GN security
header verification at forwarders is not separate (each copy goes to the node inbox and is
verified as a frame); on-air forwarding needs the medium/high MAC tier (abstract tier and
sidelink fall back to the overlay); a breakdown's DENM waits out the 10 s stationary
trigger timer, so a short run ends before it; the DP-to-AC table and the TS 102 636-4-2
CBR rule are cited from memory of the published documents; no IVIM/TIM; the dissemination
run still has no DCC. Shared build cache: the engine's `build.rs` reflection can be stale
when another worktree built last; force it with a changed `V2XW_GIT_COMMIT`.
## 2026-10-07 — the agent panel, minimal and capable, plus header fixes (agent-panel track)

- **The Portland bug, at its cause.** The agent said Portland was not supported because
  nothing told it the city library exists: no city was in its instructions or tools, and
  Portland is not a ready-made scenario. Now the instructions list every city id with its
  short name (`catalog::cities_line`, from `worlds/cities.yaml` via `v2xw_engine::city`), a
  `list_cities` tool searches the registry and says whether the map is downloaded,
  `find_settings` adds the cities to a place query, and the prompt forbids saying a city is
  missing without checking. The quick reference also names `city`, the fleet mix
  (`actors.vehicles.classes`), pedestrians, `actors.rsu_placement` and
  `security.protocol.id`.
- **`setup` tool.** It builds a draft from what the Settings window shows (`scenario.get`, which
  returns the staged draft when there is one), validates it with the loader, and stages it with
  `scenario.set`. It does not run anything. It emits `AgentEvent::Staged {setup, chips}`, where
  each chip's label comes from the published settings (`title`) and its value is put in words
  (`options::option_row`, city short names, "5% Constant position"). `POST /agent/undo
  {setup}` restores what the window held before, and the model is told at its next prompt.
  The settings index now carries each field's `label`, `level` and `recommended` range.
- **Brief replies.** The system prompt now asks for one or two sentences, a setup by default for
  "set up / use / add", and the problems flagged first.
- **More live alerts** (`watch.rs`), each said once per run:
  - `traffic.no-vehicles` (critical): no actors after 15 s.
  - `access.verify-drops`: `verify_overflow` > 0.
  - `traffic.gridlock`: the latest bin's mean speed is under 1 m/s after 30 s (HCM 6th ed.
    Exhibit 16-8).
  - `security.attacks-missed` (det_recall < 0.5) or `security.attacks-detected`.
  - The existing analyst findings already cover channel load against the J2945/1 and ETSI
    limits, latency over 100 ms, delivery and certificate exhaustion.
  - A question asked during a run carries the watcher's warnings to the model.
- **Studio.** The agent panel is a message list and one input. A setup shows as one row of
  chips with Undo, and the Settings window refreshes to the draft. Alerts are one-line cards
  placed by arrival: the numbers are behind a click, and at the run's end so is the run
  summary. Findings, runs and comparisons are behind "Details". The status paragraphs,
  provider banner, live-notes block and quick buttons are gone. In the header:
  - The Metrics button shows only "Metrics" (`metrics/Summary.tsx` removed).
  - The city is one short line (`cityShortLabel`: "Midtown Manhattan, NY", ellipsis at 22ch,
    full name in the tooltip).
  - The primary button is sized to its label and never shrinks.
  - Under 1240 px the icon buttons drop their words first.
- **Verified:**
  - copilot lib (108) and the `control` (6, including the new Portland setup, follow-up and
    Undo test) and `agent` (5) integration tests pass, as does `v2xw-cli tests/agent_host`
    (2).
  - The Studio typecheck (app and tests) passes, eslint is clean on the changed files, and
    `test/agent.test.ts` and `test/cities.test.ts` pass (19).
  - Real model (`openai:gpt-6.1-sol`): the owner's Portland prompt staged City → Portland, OR ·
    4000 veh/h · motorcycle 10%, passenger 80%, truck 10% · 200 pedestrians · US SCMS (CAMP) ·
    DSRC · 5% Constant position attackers · detectors. The reply was one line, and
    `cities.list.staged` was `portland`. Pressing Run then showed the first alert card.
  - The header check measured scrollWidth equal to clientWidth for "Run", "Run again",
    "Reconnect" and "working…" at 1440, 1100 and 900 px. The city stayed one line.
- **Not done:** the `simulate` tool's changes are not shown as chips, only in Details. Undo was
  exercised against the fake engine, not in the browser.

## 2026-10-07 — round-4 merge: nine tracks onto main

Merged with `--no-ff`, in this order: valfix, foundations, agentcrypto, page, settings,
scale, v2xnet (networking), cellular, interop. All nine merged; none was left out.

- **Conflicts resolved:** `RunReport` keeps both `scale` (scale track) and `geonet`
  (networking track). The mobility builder in `Engine::build` uses SUMO when
  `mobility.provider` is `sumo` (interop), else the city-scale tier when `scale` asks for
  one, else the default microscopic model. BUILD-STATUS sections from every track are kept.
- **Fixes the merge needed:** the CLI longrun tests set `RunOptions.city` twice (valfix and
  scale both added it); now once. The Studio's comparison hint now starts the second
  engine with `V2XW_PORT=8788 ./run-sim.sh baseline.yaml`, because the old
  `cargo run -p v2xw-server` no longer works now that the bin is in `v2xw-cli`.
- **Test reconciliation (commit "reconcile the merged tracks' tests"):**
  - `scale.traffic.meso.jam_threshold` is declared dimensionless in the publish table.
  - The fragmentation test lets only the groups that `resolve_in_flight` closes at the
    horizon be missing from the last window, since foundations made that window close at
    the horizon.
  - The geonet test drops the RSUs' spat/map roles, which foundations' new validator rule
    refuses in a cam+denm run.
  - The unbuildable-world-source case uses `open-drive`, since interop made `sumo-net` build.
  - The server's method inventory and the TypeScript conformance sweep count 34 methods.
    `cities.list` from the cities track had never been counted, and the sweep's failed
    count assertion left the run paused, which caused three more failures after it.
  - The node-feed vector was re-blessed for agentcrypto's additive `crypto` block, and
    `@vwp/protocol`'s `feed.ts` already carries it. `FEED_VERSION` was not raised, because
    the block is additive.
- **Verified:** `cargo check --workspace --tests` is clean, with only pre-existing
  warnings. `cargo test -p v2xw-engine -p v2xw-server` ran once: 369 passed, 7 failed and
  11 were ignored. All 7 failures are fixed above, and their targets were re-run green: the
  engine lib, fragmentation, geonet and scenario, plus the server's feed, rpc and interop.
  The Studio typecheck passes after the protocol and viewer builds, and
  `cargo fmt --all` is its own commit. The release build of `v2xw-cli`, which holds the
  `v2xw` and `v2xw-server` bins, and of `v2xw-server` succeeds.
- **The engine starts with the agent on:** `V2XW_PORT=8759 ./run-sim.sh` printed `agent
  http://127.0.0.1:8759/agent/status (watching every run)` and the model banner, and
  `/agent/status` answered. It was then stopped.
- **Owner action:** the server already on port 8787 was started at 14:48 from the old
  binary, which has no agent. Stop it and start `./run-sim.sh` again. Your old untracked
  `run-sim.sh` was moved to the session scratchpad as `run-sim.sh.owner-backup`. The
  tracked one replaces it, with the same scenario and port and `.env` read by the engine.
- **Not done or not checked:**
  - The full suites of the other crates (cli, mobility, netval, interop and others) were
    not run, and neither were Studio unit tests, Playwright or e2e.
  - Each track's own "not done" list stands: the NET-03 check edit was blocked by the
    permission classifier and needs owner approval; no browser pass was made over quick
    settings or the Basic/Advanced window; the Poisson cap counts trips, not placed
    vehicles; forwarded GeoNetworking copies have no MAC; cellular values are marked
    todo-calibrate; no Studio control exists for `scale`; there is no WINNER+ B1
    calibration for interop.

## 2026-10-07 — V2X networking stacks as protocols (networking track)

New module `crates/v2xw-net/src/stack/` (Artery/Vanetza as reference); the rest of
`v2xw-net` still only counts header octets.

| Piece | Module | What runs |
|---|---|---|
| GN MIB (EN 302 636-4-1 Annex H) | `stack::mib` | hop limit 10, LocTE 20 s, beacon 3 s + 750 ms jitter, CBF 1-100 ms over `DIST_MAX` 1 000 m |
| Areas, function F (EN 302 931) | `stack::area` | circle / rectangle / ellipse |
| Location table + DPL (Annex A.2) | `stack::loct` | expiry, neighbour flag, 8-entry duplicate list |
| GeoAdhoc router (§10, Annexes E-F) | `stack::router` | beacons, SHB, TSB, GBC, GUC; greedy, non-area CBF, simple and CBF area forwarding; CBF variants `etsi` / `dpd` / `gpc` |
| BTP (EN 302 636-5-1) | `stack::btp` | BTP-A/B header, port demux |
| CA service (EN 302 637-2 §6.1.3) | `stack::ca` | T_GenCam 100-1 000 ms, 4°/4 m/0.5 m/s, N_GenCam, LF container |
| DEN service (EN 302 637-3) | `stack::den` | trigger/update/terminate, repetition, receiver new/update/repetition/termination, keep-alive forwarding |
| 1609.3 WSA, 1609.4 switching | `stack::wave` | 50/50 ms CCH/SCH, 4 ms guard, next opportunity; WSA provider/user |
| Dissemination run + metrics | `stack::dissem` | event-driven, beacons fill tables, 802.11p air time; reach, reach by 500 m bin, tx (originated/forwarded/KAF), latency p50/p95/max, hops |
| Published comparison | `stack::eval` | Amador et al., Veh. Commun. 34 (2022), arXiv:2403.05994, Tables 1-3 |

**In the scenario.** `net.geonet` (gn-btp only; validated, dropdowns and labels in
`options.rs`): when a `safety.hard-brake` or `safety.breakdown` fires, its DENM is
GeoBroadcast to a circle (`area_radius_m`) and forwarded over the equipped vehicles'
positions at that instant; the event's note says reached/in-area, p50/p95 latency, hops
and transmissions, and `RunReport::geonet` keeps the numbers. `net.channel_access:
alternating` (wsmp only) holds every BSM to the 1609.4 CCH interval.

**Measured** (each test run once):
* Amador highway, 10 veh/km/lane, 5 DENMs, seed 7 — ETSI / DPD / GPC transmissions per
  DENM 183.8 / 6.6 / 18.4 (published Table 2 ÷ 30: 275 / 16.2 / 19.9; ETSI÷DPD 27.8× vs
  17.0×); PDR 0.787 / 0.667 / 1.000 (published 0.9998 / 0.9500 / 0.9917). The orderings
  reproduce; ETSI and DPD lose more reach here because the overlay has no MAC/DCC, so two
  near-tied contenders both send and the second copy cancels every downstream contender.
  Table 2's per-DENM vs per-run meaning is not stated in the paper; ÷ 30 is our reading.
* 10-node chain, 400 m spacing, 500 m range: 9/9 reached in 8 forwarded hops; CBF hop
  delay 60.4 ms as the timer formula gives. KAF: a car arriving 66 s later is reached only
  with KAF on.
* Engine, connected-intersections, breakdown at 10 s, 200 m hops: 15 vehicles, 4 of 14 in
  the area reached (the grid is not connected at 200 m that early), flooding's p95 latency
  below CBF's. Alternating access: 100 % of BSMs start inside the CCH interval vs 20.6 %
  continuous.

**Not done.** Forwarded copies do not contend for the MAC or count in channel load; the
overlay uses a unit disk at `range_m`, not the run's link budget; no location service for
GeoUnicast; no GN security header processing; WSA is not sent as frames in the engine.
`scenario::publish` tests `every_number_publishes_a_unit` (`actors.rsu_placement.count`)
and `every_section_has_a_group` (`city`) fail on fields this track did not touch (not re-run on the base commit).

## 2026-10-07 — SUMO interoperability and cross-validation (interop track)

New crate `crates/v2xw-interop`. Every number below is from the debug build with SUMO
1.27.1 from the `eclipse-sumo` wheel; the report is the record and is regenerated, not
edited.

**One command:** `just cross-validate` (add `--quick` for shorter runs) writes
[`docs/validation/cross-validation.md`](../validation/cross-validation.md). SUMO is found
through `$SUMO_HOME`, else `$V2XW_SUMO_VENV` (a virtualenv with `pip install eclipse-sumo`).
The Manhattan study needs `worlds/cache/manhattan.osm.xml`.

What is built:

| Piece | Where | State |
|---|---|---|
| TraCI client (binary protocol, batched gets: a fixed number of round trips per step whatever the fleet) | `v2xw-interop/src/traci.rs` | used by every run below |
| `mobility/sumo-traci`: SUMO drives every vehicle; this engine publishes SUMO's states as ground truth (front bumper → rear reference point, compass angle → ENU heading, SUMO lanes → world lanes by id) and inserts its own demand's trips with this engine's routes | `v2xw-interop/src/sumo.rs` | `scenarios/interop/sumo-grid.yaml` ran 60 s through `v2xw run`: 23 SUMO vehicles, 5,763 signed BSMs sent and verified over 802.11p |
| Scenario switch `mobility: { provider: sumo, route_files, sumo_args, sumo_home }`; `world.source.kind: sumo-net` now builds (the loader refused it before) | `v2xw-engine` schema, validate, wiring | absent field = unchanged hashes |
| `.rou.xml` import (`vehicle`, `trip`, `flow` with `vehsPerHour`/`period`/`number`/`probability`, nested and referenced routes) as a `Demand` that keeps each vehicle's route | `v2xw-interop/src/routes.rs` | test on `worlds/sumo/grid3.rou.xml` |
| Export: world → plain XML (`.nod/.edg/.con.xml`) → `netconvert` → `.net.xml`; demand → `.rou.xml` with this engine's routes | `netexport.rs`, `routes.rs`, `v2xw export-sumo SCENARIO --out DIR` | sumo-grid export: all 113 trips inserted by stand-alone `sumo` |
| Cross-validation study and report | `xval.rs`, `radio.rs`, `bin/cross_validate.rs` | full run about 2 min |

Method: SUMO's own network (netgenerate 5 × 5 grid; netconvert on a 1.1 km Midtown
extract), imported into this engine, so lanes match one to one; one trip list routed once
and driven by both (native, SUMO-Krauss, SUMO-IDM); one observer measures all three (Edie
flow, density and speed, paired travel times, halting below 0.1 m/s, per-edge GEH).

Findings (see the report for the tables):

- Network flow and density agree within about ±9 %, at least 98 % of edges are within
  GEH 5 (acceptance 85 %), edge-speed correlation 0.84–0.97.
- This engine is 3–9 % slower in space-mean speed and its mean travel time is 5–16 %
  longer, but the paired **median** ratio is 1.01–1.04: the difference is a tail of slow
  trips, not a shift. SUMO-IDM against SUMO-Krauss moves results only 1–4 %, so the tail
  is not the car-following equation; the leading suspects are late strategic lane changes
  before turns and the already-recorded 5.99 s permitted-left critical gap. Not yet
  isolated by an experiment.
- Radio: LTE-V2X Mode 4 against OpenCV2X (Artery-based) is within 5–14 PDR points on
  average, against Molina-Masegosa 6–11. **802.11p against Veins is 30 points off**: this
  engine still delivers most packets at 300–400 m where the Veins curve has collapsed.
  The studies used WINNER+ B1, which this workspace has only as an uncalibrated stand-in;
  until it is calibrated this is a finding about the channel, not yet the 802.11p stack.
- Every reference curve is digitised from a published figure (Veins: colour-traced from
  Sepulcre et al., arXiv:2104.07923, Fig. 4a; OpenCV2X: markers of arXiv:2103.13212,
  Fig. 3) and the report says so.

Not done: signal states from SUMO are not published back to the engine's signal channel;
SUMO pedestrians are not read; signal programmes are not exported (netconvert regenerates
them); the round trip of a procedural world through netconvert keeps lanes and signals
but shortens lanes by about 4 % (junction shapes); the tail finding above needs its
experiment; WINNER+ B1 calibration owed before the 802.11p comparison means anything.

## 2026-10-06 — settings as dropdowns, and a ready-made list a person can read (settings track)

The owner's review: settings fields were free text nobody knew how to fill, and the
ready-made list was every file in `scenarios/` under names like `phase1-manhattan`.

### What changed

- **The engine publishes words for every known set** (`crates/v2xw-engine/src/scenario/options.rs`,
  hooked into `publish.rs`). A node whose values are known carries `x-options` (value, label,
  one sentence, available) and `x-options-open`. Sources, so the page names no model id:
  `CHOICES` and the reflected enums; the loader's own tables where it is narrower than the
  registry (demand models, OBU and RSU profiles, the three jammers, `detect/legacy-12`, the two
  authorities, backhaul and backend-net models, the eavesdropper, the compromised-RSU attack);
  `AttackKind::ALL` as `threat/attacker/legacy/<Kind>`, the six `phase2::unrendered` types listed
  but unavailable with their reason; `Apps::ALL`, the RSU roles, every channel any region's band
  plan opens. Numbers carry `x-presets` with sources (demand night / light / midday / rush hour /
  gridlock, run length, penetration, pseudonym period, transmit power); lists of model choices
  carry `x-item-template`. `enum` is unchanged, so the JSON Schema means what it meant.
- **The catalogue was missing most security-path ids.** The registry slot for
  `detection.local[]` offered nothing, `detection.ma` only `threat/ma/legacy-window` (every
  shipped scenario uses `threat/ma/corroborated`), `threats.attackers[].id` offered the three
  jammers. Each now comes from the constant the engine builds by.
- **Studio**: a dropdown with the chosen option's sentence beneath it ("Engine default" for an
  optional field, "Other…" for an open set, refused values disabled); preset chips beside a
  number box that still takes any value; checkboxes for message sets and applications; a
  row-by-row editor with Add/Remove for attackers, detectors, jammers and adjacent-channel
  emitters (fields it has no control for stay visible as one small JSON box).
- **Ready-made list**: `scenarios/ready-made.txt` is the picker's list, grouped; the server reads
  it (`preset_entries`) and sends each entry's `group`; unlisted files still load by path. The
  picker shows the groups and each scenario's first sentence. Seventeen entries — Midtown normal,
  evening rush hour, pedestrians and jaywalkers, heavy rain; a highway; Portland; Berlin on the
  European stack; DSRC / LTE / NR on identical traffic; fake positions at rush hour; replay;
  jamming; certificates and pseudonym privacy; the ETSI CCMS; post-quantum signatures; emergency
  signal priority. Each is an overlay on `scenarios/base/` or on the study file it builds on.
  `manhattan-vru.yaml` (loaded by nothing) is removed. Start the server on
  `scenarios/midtown-normal.yaml`.

### Evidence

- `v2xw validate` (release CLI from `main`): all 17 listed files and both bases pass.
- `cargo test -p v2xw-engine --lib scenario::`: 24 passed, including the two new tests in
  `options.rs` (the typed fields are choices; ids read as words).
- `cargo test -p v2xw-server --test apply -- the_ready_made_catalogue_is_the_list
  a_preset_can_be_loaded_and_run`: 2 passed.
- Studio `tsc` clean; `vitest run test/settings.test.ts` 17 passed (4 new); eslint clean on the
  changed files.
- Playwright, one pass on the debug server of this branch: the picker shows 6 groups and 17
  entries; the settings window has 53 dropdowns, 24 preset chips and 5 list editors; Add in
  Attackers appends a valid "Fake position: frozen" row at 5 %; no page errors. The text inputs
  left are names, descriptions, dates, file paths, the seed and the list editors' advanced JSON.
- Short runs, release CLI, `--no-recording`:
  - rush hour with the fake-position attackers, 200 s: 211 vehicles, 9 attackers, 9 revocation
    decisions, CRL enforced 57 s after the first detection, 1,993 receptions refused as revoked;
    **1.83 s of wall time per simulated second** — the page plays it slower than real time.
  - post-quantum, 20 s: 192 SDUs fragmented; Berlin, LTE, emergency priority, 20 s: run clean.
  - highway, 120 s at 48,000 veh/h offered: 164 vehicles admitted (about 4,900 veh/h), 0.79 s/s.

### Open

- **The highway is busy, not jammed.** The spawner admits about a tenth of the offered rate on
  the 2 × 2 grid, so per-direction flow (~1,200 veh/h) stays below what one blocked lane leaves.
  A TR 36.885 drop at 70 km/h (336 vehicles) ran at 5.1 s/s and emptied as trips completed. A
  level-of-service F highway needs more admission than this geometry gives.
- **Midtown volumes are scaled** (the presets' ratios are cited, their level is not NYC DOT's);
  the rush-hour congestion comes from the breakdown and the closure.
- No Sybil scenario: the engine refuses Sybil (no concurrent pseudonyms for an attacker); the
  replay scenario covers a validly signed lie instead. No PSM generator, so the pedestrian
  scenario's devices do not transmit.
- `radio.models.*.id` and the procedural world parameters are still free text / JSON.

## 2026-10-07 — network and traffic validation suites finished (validation track)

One engineer, owner of `crates/v2xw-netval` and `crates/v2xw-trafficval` (both merged as
early work on 2026-10-06) and of the small defects they find. Every number below comes from
the debug build in the track's worktree; the two reports are the record and are regenerated,
not edited.

**One command:** `just validation-report` (add `--quick` to leave out the whole-run and
city-scale checks) runs `netval-report` and `traffic_validation` and writes
[`docs/validation/network.md`](../validation/network.md) and
[`docs/validation/traffic.md`](../validation/traffic.md). Every check runs as shipped and
again with an injected fault that must turn it red; a check its fault cannot turn red is
reported as such (`CANNOT FAIL` / *pass, not proved*), never as a pass.

### Network: 45 checks — 44 pass and are proved able to fail; NET-03 fails (another track's defect)

New this round: **PHY-14** (the 802.11 convolutional code's distance spectrum recomputed
from its generator polynomials and puncturing patterns by a trellis search, against the
NIST union bound's coefficients), **PHY-15** (uncoded BER formulas against the exact
Gray-mapped BER), **PHY-16** (PER against a Monte-Carlo hard-decision Viterbi link written
in the suite: the NIST bound is 1.2–1.8 dB pessimistic at 10 % PER, as a bound must be),
**ACC-12** (sidelink selection window and candidate set, TS 36.213 §14.1.1.6 / TS 38.214
§8.1.4), **ACC-13** (S-RSSI ranking, step 8), **NET-06** (ETSI CAM, DENM and VAM encoders
against asn1tools compiled from the ETSI modules: 240 of 240 messages decode, re-encode to
the same octets and carry the station id and position sent).

Fixed checks: ACC-04 now holds the EDCA machine to the exact slotted broadcast process as
well as Bianchi's Eq. 7 (the approximation overstates collisions at N = 20); SEC-01 sends
OpenSSL's signatures in low-s form and confirms the high-s refusal; SEC-03 no longer panics;
E2E-01 moves attack windows inside its 3 s runs; E2E-03 runs below saturation so a power
fault can show.

Defects found and fixed:

| Where | Defect | Fix |
|---|---|---|
| `v2xw-radio` `cv2x::THERMAL_NOISE_DBM_PER_HZ` | −173.9750935 against its own formula's −173.9751872 dBm/Hz | constant corrected |
| `v2xw-metrics` channel table (outside this track's crates) | I-T2 called `backend.state`, `node.drop`, `node.security`, `privacy.link`, `privacy.track`, `scenario.event`, `sec.pseudonym` undeclared, failing conservation on 8 shipped scenarios | listed with the visibility `v2xw_record::channels` declares |

Recorded, not changed: NET-03 (every message signed under PSID 0x20, including CAMs, DENMs,
SPATEM/MAPEM; `v2xw-node`, another track's crate); the real backend refuses high-s ECDSA
signatures that FIPS 186-4 accepts (a deliberate I-S1 policy, an interop note); the
punctured-rate spectrum coefficients ns-3 publishes differ from an exhaustive search in
their high-order terms (≤ 6e-4 relative in Pe, under 0.001 dB).

### Traffic: 23 checks — 86 held figures pass, 20 pass but are not proved, 15 fail (full run, 10 min debug)

The suite's own defects were the larger share: a cached study keyed on whether a tamper was
present (so a check's second fault reused its first fault's run, and the jerk rows could
never go red); a gridlock row held on runs shorter than the auditor's 180 s limit; a closure
applied to a lane nobody used; an IDM band encoding a property the equation does not have
(the continuous IDM stops 0.084 m short of s0); a string-stability probe whose spectrum lay
outside the unstable band; a lateral-acceleration reading that turned a polyline vertex into
11 m/s². Each is fixed in the check and listed in the report's findings table. String
stability is now held to the linear transfer function |G(iω)| within 0.003 at 22 densities
(measured within 0.0005).

Model defects the suite finds and records (the report's FAIL rows, open):

| Check | Finding | Owner |
|---|---|---|
| `calibration/turning` | turns driven at the SUMO default 5.5 m/s² average lateral acceleration; 85th percentile 8.2 m/s² against AASHTO's 3.7 | mobility (changing the default moves every golden) |
| `model/sugiyama-ring` | the calibrated city drivers are too string-stable to form Sugiyama's jam (slowest car 3.4 m/s) | mobility |
| `cities/manhattan` | one car crossed an unsignalised crosswalk a pedestrian was on (5 vehicle-steps) | mobility (`vru::crosswalk`) |
| `cities/berlin` | junction areas of the OSM import smaller than their connectors (body centre ~2 m outside; 34 outside-junction and 16 step-speed steps in 180 s) | world |
| `cities/portland` | the scenario puts at most 6 vehicles on the road at its 1,000 veh/h (2 in the quick run) | demand / world |
| `invariants/peak-density` (180 s) | one lane-change overlap and one red entry at 20,000 veh/h | mobility |
| `invariants/shipped-scenarios` (120 s) | midtown-street-life: 56 outside-junction, 17 pedestrian-overlap steps; a 21 m/s published-speed spike in two Manhattan scenarios; jerk rate 0.0011 against the 0.001 gate on two | world, mobility |
| `calibration/permitted-left-gap` | Raff critical gap 5.99 s against the HCM's 4.5 s (15 turners) | mobility |

### What is not done

- 20 traffic held figures are still *pass, not proved* (the ring fundamental diagram's
  truck fault does not move its bands; several calibration rows have no fault yet).
- No J2735 SPaT/MAP independent decoder (the J2735 ASN.1 is not in the repository), no
  re-derivation of the LTE/NR BLER tables against 3GPP link-level results, no J2945/1 CAMP
  worked examples (not obtainable).
- `docs/validation/traffic.md` was generated before the three full-run findings above were
  added to the report's findings table (`report::FINDINGS`); the FAIL rows themselves are in
  it, and the next `just validation-report` adds the table rows.
- NET-06 needs `asn1tools` (`pip install asn1tools`, or `NETVAL_PYTHON` naming an
  interpreter that has it); without it the check reports a skip.

## 2026-10-06 — stabilise track: every suite run on the ten-track merge, and Midtown's evening peak

One engineer. First, run every test suite once on `main` as merged (`c4c4721`) and fix what
the merge broke; then put the page, the scene, the inspector and the metrics under a
realistic Midtown rush hour, measure them, and hold what was measured with budget tests.
Every number below comes from a command run on the owner's 8 GB laptop with seven other
engineers' builds and runs sharing it through the build lock; wall-clock figures are noisy,
counts are not. Every heavy command ran under `heavy.sh`, which sets
`RAYON_NUM_THREADS=2`: the engine's parallel link geometry had two threads, not the
machine's eight.

### The suites on the merge

| Suite | Result on `c4c4721` | Cause | Now |
|---|---|---|---|
| core, msg, sec, world, mobility, radio, net, proto, threat, node, record, metrics, experiment, cli, copilot, wasm, py | all pass (217, 220, 100, 245, 252, 317, 113, 128, 225, 209, 242, 256, 80, 17, 90, 6, 16) | — | — |
| engine | 221 pass, 1 fail: `the_five_internal_error_causes_are_refused_at_check_by_their_setting` | the mbd track's two-pipeline `MaHost::from_scenario` replaced the robustness track's `ma_params` and reported an unknown `detection.ma.params` key against the block, not the key, and took `revoke_window_s: -1` | fixed (`32cf240`): refusals name `detection.ma.params.<key>`, list the pipeline's keys, refuse negative windows |
| server | 118 pass, 1 fail: `the_feed_has_the_shape_of_the_published_vector` | two deliberate shape changes: `queues.list[].drops_note` gone (node.drop records made tx drops observable), `received[].stages_ms.handoff` new | vector re-blessed (`05be715`); `FEED_VERSION` unchanged, since a v1 reader misreads neither |
| conformance | 66 pass, 3 fail: the wall-clock and libm firewalls, and the `grid-traffic` golden | netval and trafficval (merged as early work) called `f64::exp/ln/log10/powf/sin` and read `Instant::now`; the golden was blessed on the traffic3d branch before main's schema additions reached it, plus 45 more `metric.sample` rows from later merges | netval/trafficval on `v2xw_core::math`, timings dropped from their progress lines (`5d75dbf`); golden re-blessed after the same digest at `RAYON_NUM_THREADS=1` and `2` (`66848ef`) |
| netval | 2 pass, 4 fail, 1 ignored | the suite's own findings, not the merge: NET-03 PSIDs (CAM signed under 0x20, registry 36), ACC-04 (N=20: 0.869 simulated vs 0.907 analytical), PHY-09 (802.11p floor −98.000 vs kTB+NF −97.975 dBm), SEC-01 (Rust accepted 15/32 OpenSSL signatures), SEC-03 panics (`UnknownKey { backend: "crypto-backend/real", key: 3 }`) | **open**, for the netval owner |
| trafficval | 0 pass, 1 fail: `the_model_level_checks_pass_and_each_can_fail` | the suite's findings: IDM gap at rest 1.927 m against [1.95, 2.5]; string stability, simulation and theory disagree at 6 densities | **open**, for the trafficval owner |
| UI: protocol, viewer, mock-server | 197, 194 (+1 skipped), 49 pass | — | — |
| UI: studio unit | 234 pass | the first run failed 10 files on "Failed to resolve entry for package @vwp/protocol": a fresh worktree has no `dist/` until a package is built; after the typecheck built it, all pass | — |
| UI: typecheck, all four packages | clean | — | — |

The Playwright suites were not run (they drive the page against the mock and the engine and
belong to the UI tracks' merge step), apart from the peak spec below.

### Midtown's evening rush: `scenarios/midtown-peak-load.yaml`

(Added as `midtown-rush-hour.yaml`; renamed at the merge because the settings track's
`midtown-rush-hour.yaml` is a different, lighter scenario with an incident and a closure.)

The D7 Midtown box at 17:30 on a weekday: demand from NYC DOT's peak-hour avenue counts
(1,356 veh/h PM peak on Eighth Avenue, CB4 2011; about eight avenues cross the box) with a
four-times fill for the first 90 s so the rush exists inside a five-minute run; 2,000
pedestrians and 200 cyclists who behave as observed, a quarter with a handset; 3 %
motorcycles and 4 % delivery mopeds; every motor vehicle sends signed BSMs. The scenario
file states its sources and what it does not model.

**The engine at peak** (the release `v2xw-server` built from main on 2026-10-06, unthrottled,
no page attached, `run.status` every 10 s and `footprint`):

| | uncapped | `radio.range.max_m: 500` |
|---|---|---|
| simulated ÷ wall time | 0.19 (46.5 s in 243 s, slowing as vehicles arrive) | 0.53 (58.4 s in 111 s) |
| actors / radios at the end | 2,283 / 627 | 2,300 / 644 |
| reception attempts per frame | 314 (14.2 M for 45 k frames; mean RSSI −120.8 dBm) | 110 (7.5 M for 68 k frames; −110.8 dBm) |
| engine footprint | 2.37 GB (seek history at its 1,024 MB cap from 36 s) | 1.69 GB at 111 s |

Uncapped, every frame was followed to every radio in the box, most of them 20 dB under the
noise floor; the scenario now caps full evaluation at 500 m (beyond it a line-of-sight
receiver still takes the energy as interference) and says why in the file. With the fill
the page run below reached 0.21x real time at about 250 vehicles: the engine, not the page,
sets the pace at the peak.

A 15 s `sample` of the release server: the engine thread never idled; the rayon wait for a
frame's link geometries was about 21 % of it (two threads), `ActorSnapshot::actors_within`
about 12 %, `start_frame`'s own loop about 10 %, and the metric providers decoding records
from JSON about 10 %. The candidate scan was walking every actor (about 2,300) to keep the
~600 with a radio; **`c8769ed`** gives frames an index of the equipped actors
(`run/radios.rs`). It changes no record: `grid-traffic` reproduces its golden digest, and a
debug assertion compares the index with the old query on every frame of every debug run
(the vru, reception, phases and radio_access suites pass with it; dropping one hit fails 3
of 4 vru tests). Its speed-up is **not measured**: a release build is off limits here and a
debug ratio is not a release one.

**The page at peak** (`ui/apps/studio/e2e-engine/peak.spec.ts`, release engine, Vite dev
server as the owner runs it, headless Chromium on SwiftShader, 60 s of streaming then a
vehicle selected and the metrics panel opened):

| | measured | budget |
|---|---|---|
| actors on the page | 2,337 | > 1,000 (the crowd arrived) |
| viewer CPU per frame, mean | 6.4 ms (frame 7.5 ms) | < 12 ms |
| page script share while streaming | 1.2 % (GPU waits 2.9 %) | < 50 % |
| select a vehicle: HUD shown / script / GPU wait | 111 ms / 173 ms / 6,533 ms | < 5 s / < 1 s / — |
| open metrics: first card / script / GPU wait | 1,860 ms / 672 ms (React DEV 320 ms) / 2,793 ms | < 5 s / < 1 s / — |
| longest main-thread task | 6,607 ms | < 400 ms, with a GPU only |
| tab heap after GC, peak | 146 MB | < 600 MB |
| engine footprint, peak | 1.03 GB | < 3 GB |

The first run failed on a 5,004 ms task when a vehicle was selected. Its CPU profile:
4.1 s in `WebGLRenderer.setSize` and `getExtension`, 25 ms of React. A synchronous WebGL
call waits for the GPU process to finish what is queued, and SwiftShader is a software
rasteriser, so in this harness a long task is the rasteriser. The spec therefore profiles
three windows and splits the page's own script from GPU waits; script is budgeted always,
long tasks only with `VWP_HEADED=1`. A control in the spec (a 300 ms busy loop must read as
more than 250 ms of script) keeps the script budget able to fail.

What this says about the page at the peak: its own JavaScript is light (1 % while
streaming, under 0.7 s for either interaction in DEV mode) and memory is bounded. Two
things are left for the owners of the viewer:

- **Shader compilation is synchronous.** `getProgramInfoLog` took 1.75 s during streaming
  and 2.4 s when the metrics panel opened: three.js checks every new program's link
  (`renderer.debug.checkShaderErrors`, on by default), which blocks until the GPU has
  compiled it, and new materials appear as vehicle classes and close-up models enter the
  view. On a GPU this is a hitch of tens to hundreds of milliseconds rather than seconds,
  not measured here. `renderer.compileAsync` at world load would move it off the click.
- **A selection resizes the canvas** (the inspector opens beside it): 3.4 s of
  `setSize` here, a reallocation of the drawing buffer on any GPU.

The channel at this peak is quiet: CBR 0.008 and 0.99 ms of air time per node per second
at 19 s, with about 250 vehicles. Buildings keep most neighbours under the carrier-sense
threshold; whether that matches a field measurement in Midtown is not checked here.

### Budget tests added

- `ui/apps/studio/e2e-engine/peak.spec.ts` — the table above, opt-in with the engine suite
  (`VWP_ENGINE_BIN=… pnpm exec playwright test -c playwright.engine.config.ts peak.spec.ts`;
  `VWP_PEAK_REPORT`, `VWP_PEAK_PROFILE` and `VWP_PEAK_SHOTS` save the numbers, the
  profiles and pictures).
- `crates/v2xw-engine/tests/peak_budget.rs` — the engine's work at peak as counts, on a
  procedural grid with the peak's mix: no frame is followed to anything without a radio,
  and with `radio.range.max_m` the attempts per frame fall and stay under a budget taken
  from the measured value.
- `crates/v2xw-engine/src/run/radios.rs` — the index's grid against a full scan.

## 2026-10-06 — metric health bands and the run summary (metrics track)

The owner asked two things of the measurements: "a user might see airtime per node, but they
might not really be quite sure whether or not the shown value is safe or healthy for an actual
system", and "a run summary at the end … how the run performed, any of the most important
statistics, anything that was unhealthy or had high load, because the goal of this is to
evaluate certain protocols."

- **Health bands are part of the metric definition.** `crates/v2xw-metrics/src/health.rs`
  publishes, for 21 metrics, two thresholds (healthy → caution → unhealthy), a one-line
  explanation per level, the series judged (the headline, a percentile such as
  `e2e_latency.p99`, or a breakdown value such as `pdr[100m]`), the source and whether the
  number is a standard's, a literature convention or an engineering choice.
  `MetricDef::new` looks every metric up in that one table, so the band reaches the catalogue
  whichever provider defines the metric (including `cert_pool_valid` and `linkability` in
  `v2xw-engine`, untouched). `MetricDef::validate` refuses a malformed band. The server's
  catalogue rows carry it as `health`.
  - Standards: CBR 0.50 / 0.68 (SAE J2945/1 vMinCBP; ETSI TS 102 687 adaptive CBR_target),
    airtime per node 10 / 30 ms/s (TS 102 687 reactive Relaxed 1 % duty cycle; δ_max 3 %),
    p99 end-to-end latency 50 / 100 ms (3GPP TS 22.185 §5.1, TR 22.886, SAE J2945/1), PDR
    within 100 m 0.95 / 0.90 (3GPP TR 37.885's 0.9 reception level), AoI 300 / 600 ms and
    p95 inter-reception 0.3 / 0.6 s (J2945/1 10 Hz and vMaxITT 600 ms).
  - Literature: TTC 3 / 1.5 s (Hayward 1972; FHWA SSAM), DRAC 2 / 3.35 m/s² (Archer 2005).
  - Engineering choices, said so in the band: MAC and verification queues, access delay,
    verification wait, unverified share, certificate pool, revocation stage time, detection
    recall and false-positive rate, linkability, backend reachability. Several match the run
    analyst's `Thresholds` on purpose, so a card and a finding never disagree.
- **The dashboard shows them.** A "Health at a glance" strip of every banded metric, worst
  first; each card a coloured edge and a line like "Unhealthy 0.71 — above the 0.68 ETSI
  congestion target; vehicles will throttle their messages" (the level always written, never
  colour alone); threshold lines on the sparkline; a dot on a group in the side navigation;
  in the expanded view the band shaded behind the chart, the whole run's verdict ("mean …;
  unhealthy 12 % of the run, worst … at 42 s") and the band written out with its source. The
  expanded view opens on the judged series.
- **Run summary** (`metrics/RunSummary.tsx`): a dialog over the simulator that opens by itself
  when a run this page watched finishes, and from the header's Summary button or the
  dashboard's Run summary button at any time. The scenario in one line; headline results in
  six tiles (delivery, latency, channel load, security overhead, credentials and revocation,
  traffic flow); every metric outside its band ranked by severity with how bad, when and the
  worst nodes; bottlenecks (dominant latency stage, dominant loss cause, backend entities by
  utilisation against the 0.8 planning ceiling); the agent's analysis (`AgentSlot.tsx`, the
  newest analyst findings and model overview from `state/agent.ts`, with an Analyse button);
  what stayed healthy and what was not measured.
- **Evidence.** `cargo test -p v2xw-metrics --lib`: 233 passed, including the four band tests
  (every band well formed; 0.71 CBR unhealthy with the 0.68 reason; higher-is-better bands
  turn the other way; a band out of order refused). Studio typecheck clean;
  `test/metrics-health.test.ts` 10 passed (boundaries match the engine's, judging a run's
  windows, ranking, the scenario line, reductions, dominant share, backend utilisation).
  One Playwright pass against a debug `v2xw-server` on a 25 s cut of `manhattan-5min`: the
  catalogue carried 62 series rows with a band; the summary opened by itself at the end with
  "None unhealthy on average, 1 to watch, 2 out of range at times, 12 healthy throughout"
  (unverified share caution at 0.0303; AoI 1,367 ms at 18 s; PDR within 100 m 0.933 at 15 s),
  bottlenecks signing 73 % of latency and weak signal 99 % of losses; the dashboard showed the
  health strip and the expanded CBR chart its band and source; no page errors.
- **Not done.** The run analyst (`v2xw-copilot`) still holds its own `Thresholds` rather than
  reading the bands; the two agree where they overlap but are two copies. HSM utilisation is
  not a catalogue metric (it is in node telemetry), so it has no band; the summary shows
  backend entity utilisation instead. A metric whose judged series the engine does not
  stream (a p99 on an engine without percentiles) is shown as not judged, never judged on
  another statistic. The summary's whole-run figures are means of the run's windows, not
  pooled estimates; it says so.

## 2026-10-06 — the agent, always on (agent track)

The owner asked for the agent to turn on whenever a simulation starts. What is built:

- **The watcher** (`crates/v2xw-copilot/src/watch.rs`): a pure state machine fed `run.status`
  answers and the run's evidence. It adopts a run when it moves (a server started paused at
  t = 0 is not a run yet), runs the deterministic analyst over the evidence read so far and
  posts a plain sentence when a warning-or-worse finding appears, rises a level or clears;
  names a trend when the latest bin of `cbr` or `pdr` crosses the analyst's own threshold
  (`Thresholds::cbr_target`, `prr_range_level`) before the whole-run value does; and
  announces rises in the backend's `ma.revocation_decisions` and `ee.vehicles_starved`
  counters. Every number in a note is the analyst's or the backend's. When the run ends it
  writes a `RunSummary`: a verdict (healthy / needs attention / unhealthy), each of the nine
  layers against its thresholds or "not measured", the problems most severe first, and an
  overview. With a model key the overview is Claude's (one request, no tools), and it is
  discarded for the analyst's own if it states a number the analysis does not contain.
- **The host** (`crates/v2xw-cli/src/agent_host.rs`): `v2xw serve` starts the watcher
  thread with the engine, unasked: a status read a second, an evidence read every six
  seconds while a run moves. New endpoints `GET /agent/watch?after=` (the feed, the phase,
  the progress, whether a model is attached) and `GET /agent/summary` (the last finished
  run's summary and its Markdown) — the latter is what the run-summary overlay reads.
- **`find_settings`**, a new agent tool: searches the engine's own published settings
  surface by words, so the model finds a key (attackers, demand, radio) instead of guessing.
- **The Studio**: the header's Agent button carries a live dot while a run is watched, a
  count of unseen warnings, and a short note under the header for each new warning and for
  the run's end; the agent panel opens on the live notes and the run summary
  (`AgentRunSummaryView`, exported for the overlay).

**The agent lives in `v2xw serve`, not in `v2xw-server`.** `v2xw-copilot` depends on
`v2xw-server` for the method list and the OpenRPC document, so the `v2xw-server` binary cannot
carry it without a dependency cycle. Start the engine with
`./target/release/v2xw serve --scenario <file> --port 8787 --paused --speed 0` (same flags);
the page says exactly this when the engine it is connected to has no agent.

Measured, once each: `v2xw-copilot` lib `watch::` and `agent::tests` (13 pass, among them
each live-note kind shown silent on a quiet run and firing on its constructed cause, and a
model overview with an invented number discarded), `tests/agent.rs` (5: a scripted model
configures, runs, analyses, then a follow-up compares), `v2xw-cli tests/agent_host.rs` (2:
the watcher follows a fixture run to its summary with no request), the Studio's
`test/agent.test.ts` (12) and its typecheck. One Playwright pass on a debug `v2xw serve`
with the procedural grid at 1,800 veh/h and `metrics: [all]`: the header note appeared at the
run's end, the panel showed the notes and a "Needs attention" summary (one finding,
`channel.weak-signal`). Not measured: a run with a model key (no key on this machine), and the
watcher's load on a long city run.

## 2026-10-06 — scene and cameras track: the ground is where the streets are

Owner: `ui/packages/viewer` (cameras, ground, road surfaces, clip planes). Owner's points 2
("close to the ground the ground seems to go down and there are holes in it"), 4 ("the camera
ended up below the map") and 12 ("the free camera should be easier to control").

### One cause for points 2 and 4

The viewer took `world.bbox.minZM` to be the ground. The OSM importer puts a tunnel 6 m below
ground per `layer` level (`crates/v2xw-world/src/osm.rs`, `TUNNEL_LEVEL_DEPTH_M`), and the box
encloses every lane's centreline, so a world with a tunnel has its floor under the streets. On the
real Manhattan map the page reported `bbox.minZM = −24 m`. So the ground plane was drawn 24 m under
the streets, the landuse with it, every junction disc 24 m under its junction (a hole at every
intersection), every crosswalk likewise; between the road ribbons the ground "went down". The
camera's floor was the same −24 m, so the free camera, a spring's overshoot, or a chase of a car
entering a tunnel could take the camera under the roads and the buildings.

- `src/ground.ts`: the ground level is the median building base (junctions, then 0, when there
  are none). The ground plane, landuse, RSU masts, heatmap and coverage overlays, picking and the
  glitch hunter's "under the ground" all use it; junction discs and crossings use the junction's
  own height (a bridge's junction is above grade).
- The ground quad follows the camera and spans 2.5 far planes (a corner ray of a 68°, 2.5:1 view
  reaches 2.07 far planes out before its depth passes the far plane), and is pushed behind the
  road layers by polygon-offset units, not slope.
- Clip planes: the street near plane is measured from the ground, not the box floor (which had put
  it at 1 m), and is never so far out that the near-plane rectangle reaches into a road surface —
  the limit is computed exactly from the projection's own inverse, lens shift included.
- Cameras: a hard limit applied last in every frame and every mode — above `ground + 0.75 m`
  (above every road surface: markings, sidewalk lift and a 0.15 m kerb, AASHTO Green Book §4.7),
  outside every building that is not ghosted, under the plan view's ceiling. A chase or dashboard
  subject in a tunnel is framed from the street above it. The occlusion march runs only for chase
  and dashboard (with the free camera's look point 40 m ahead and a building beyond, it pulled
  the camera through the wall in front). The plan view lifts above roofs at its 12 m minimum.

### Point 12: the free camera

It had no keys: drag turned the view and nothing moved it, and a drag released over a vehicle
was taken as a click and flew the camera into a chase. Now: W A S D or the arrows move level with
the ground, E / Space up, Q / C down, drag to look, the wheel sets the speed, Shift ×4, R resets to
a 55° view from above the skyline. Speed scales with height (1.2 m/s per metre, 8–600 m/s), eases
in over 0.12 s and stops in 0.07 s. A move into a building slides along the wall, 1.2 m off it.
The camera answers directly (no spring on top of the eased keys). The Studio shows the controls
while the free camera is in use (`FreeCameraHint.tsx`), gives the canvas the keyboard when Free is
chosen, and ignores a "click" that was a drag of more than 5 px (`Viewport.tsx`). In the plan view
W A S D pan.

### Evidence

- `test/ground-camera-geometry.test.ts` drives the real viewer (null renderer) over a grid world
  with a 6 m tunnel, through 24 random poses × 40 frames in each of map, chase, dashboard, free and
  rsu, with random drags, wheel, held keys (Q often), seeks, a subject leaving the stream and NaN
  poses, and checks every frame: camera above every road surface and outside every building; the
  near-plane rectangle above the road; every pixel ray below the horizon within the far plane
  landing on the ground quad; the ground plane at the ground level; and no street-surface vertex
  under the street except the tunnel's. The checks use three.js's camera and the scene graph only.
  **On main's viewer (`c4c4721`) every test in the file fails**, with frames failing each check
  counted separately (of 960 per mode): map — ground not at grade 960, ray misses ground 204,
  camera in a building 57; chase — near plane cuts the road 235, ray misses 151, under the ground
  61; dashboard — near cuts 200, under the ground 200, ray misses 47; free — ray misses 250, near
  cuts 80, under the ground 80; rsu — ray misses 116, near cuts 64, under the ground 19; plus
  junction discs and markings at z −6.3 to −6.5, and the chase of a car in the tunnel at z −3.4.
  On this branch: 7 of 7 pass.
- `test/free-camera.test.ts` (4): W moves forward level and stops on release; the wheel and Shift
  change the speed and Q stops at the floor; walking into a building slides along it; R returns
  to a view over the city.
- Existing suites near this change, run once on the final commit: `camera-robustness`,
  `world-cameras`, `street-cameras`, `viewer`, `scene-correctness`, `glitch-hunter` — 76 pass
  (87 with the two new files). Viewer `tsc -b` and test tsconfig clean; Studio `tsc` clean. The
  whole viewer suite was not run (build-first policy; the merge step runs it).
- The page, Manhattan (`manhattan-5min.yaml`, main's release engine, headless SwiftShader at a
  few frames a second; pictures looked at): the page reports `bbox.minZM −24`, ground level 0, the
  ground plane at −0.05. Chase with the orbit dragged to its lowest sits at the 0.75 m floor with
  the road, markings and crosswalks whole under it; dashboard and free seeded from it draw the
  crossing and the avenue; a drag in free stays in free (it used to select a vehicle); E, W and Q
  move the camera and Q stops at 0.75 m; R flies to a view over Midtown. Motion smoothness cannot
  be judged at that frame rate.

### Open

- A car in a tunnel is under the ground plane and so invisible; the chase camera follows it from
  the street above. Drawing tunnels (a cut-away, or the ground made see-through over a followed
  subject) is not done.
- A DEM-draped world is not handled: the protocol carries no terrain, so the ground is one flat
  plane at the median building base.
- Files outside `ui/packages/viewer`: `ui/apps/studio/src/components/Viewport.tsx` (the hint, the
  canvas focus, the drag-is-not-a-click guard) and the new
  `ui/apps/studio/src/components/FreeCameraHint.tsx`.

## 2026-09-30 — robustness track: a refusal names its setting, nothing flaky, nothing leaks

One engineer, owner of error handling in `crates/v2xw-server` and the engine's loader, the
fuzz and soak suites and the Studio's error presentation. Every number below comes from a
command run in the track's worktree on a shared, loaded machine (other tracks held the build
lock for most of the session); timings are wall clock and noisy, counts are not.

### User input no longer becomes "internal error"

The five causes the 2026-09-24 QA found are refused at **Check** (`scenario.validate`),
**Apply** (`scenario.set`) and **Run** (`run.start`) with `-32004` and a row naming the
setting and the fix. A Run refused before the kernel is touched leaves the run on screen as
it was (not `error`).

| Cause | Now refused by | How |
|---|---|---|
| missing map file | `world.source.path` | new `scenario::preflight` (files on disk), run by the server at Check/Apply/Run, by `Engine::build` and by `v2xw validate` |
| missing DEM | `world.terrain.dem` | the same preflight |
| RSU `site` on an OSM world | `actors.rsus[i].site` | `validate` knows the world's site count before it is built (none on an import, one per junction with `rsu_at_junctions`) |
| wrong `net.backend_net` id | `net.backend_net` | `validate` runs the run's own `apply_backend_net`, which now also refuses unknown and non-finite params |
| unknown `detection.ma.params` key | `detection.ma.params.<key>` | `validate` runs the run's own `ma_params`, which lists the keys it takes; `detection.local[i].params` the same |

Other user-reachable internal errors found and closed:

- **Every build failure was `-32603`.** The server's setup channel carried a string; it now
  carries the classified error (`setup_error`): anything the scenario caused is `-32004` with
  its row, a file is `-32013`, and only engine faults are `-32603`. Import errors from the
  map or the raster are reported against `world.source.path` / `world.terrain.dem`
  (`wiring::source_error`, `terrain_error`). A refusal that only the built world can decide
  (a closure naming an edge the grid lacks) leaves the engine in `error` with the reason in
  `run.status`, `run.step` then answers `-32002` (no run), not `-32603`, and the next Run works.
- **`time.mobility_step_ms: 30`** (inside the 10–100 ms bounds) failed Run with
  "internal error: recording": the server built its cadence as `Cadence::new(1 s, step)`. It
  now uses the kernel's own snapshot cadence.
- **`world.generate`** with lanes and widths that leave no block was `-32603`; now `-32602`
  naming the parameter.
- **World source kinds this build cannot build** (sumo-net, opendrive, json-legacy,
  osm-bbox), **grid parameters the generator refuses** (e.g. `cols: 1`), an **empty OSM path**
  and a **missing `world.highway_preset`** on an OSM world: refused at Check by field.
- **Guards** (stated as guards, not traffic figures): a procedural grid of more than 10,000
  junctions (`MAX_GRID_JUNCTIONS`), and an arrival rate above 100,000,000 veh/h, including one
  a `demand.multiplier` raises it to (`MAX_ARRIVAL_RATE_VEH_PER_H`). `rate_veh_per_h: 1e308` was
  accepted and made the thinned-Poisson process draw a candidate per nanosecond. The first
  ceiling, 1,000,000 veh/h, broke 38 engine tests and the CLI's shipped-scenario test, which
  offer `scenarios/scale/`'s 3,600,000 veh/h on purpose (fill the grid at lane capacity); the
  full engine suite caught it and the ceiling was raised.
- Refusal rows are JSON Pointers (`/actors/rsus/0/site`, was `/actors/rsus[0]/site`, which
  matched no field, so the page listed the refusal at the top instead of on the setting), and
  their hint is the field's published range or choices instead of a generic sentence.
- The server binary prints a refused scenario's rows at startup (it printed "scenario invalid").

**The page.** `lib/errors.ts` turns a refusal into "The engine refused a setting:
world.terrain.dem: … which does not exist …" and an internal error into "a fault in the
engine, not in your settings". A refused Run marks the fields in Settings and resumes the run
it paused. The header's Run button used to swallow every error (`PrimaryAction` discarded
it); it now shows it beside the button. In `error` the status says the engine's reason.
Keys the engine refuses whatever their value (`detection.responder`,
`detection.perception_tier`) and `time.des_resolution` are now under **Unsupported** with the
engine's note.

### Fuzzing the control surface

`crates/v2xw-server/tests/fuzz.rs`: seeded random sequences of every method, well-formed and
malformed, half on a connection and half over HTTP, interleaved with producer ticks, and
edits to every leaf of the published scenario surface (allowed values, bounds, past the
bounds, wrong types). After every call: no panic, no `-32603`, a §6.4 code, a refusal with
rows, a `run.status` in a known state with its clock inside its horizon, and a refused edit
changes nothing; at the end the original scenario reproduces its pre-fuzz output digest.

- `fuzz_short` (250 calls) runs in the suite. `fuzz_long` ran 2 × 20,000 and 3 × 2,000 calls
  (seeds 11, 12, 1, 2, 3): **46,000 calls, 0 internal errors, 0 panics**, digest reproduced
  each time; e.g. seed 11: 5,627 ok, 6,679 `-32602`, 508 `-32004`, 880 `-32009`.
- Shown able to fail: with `run.pause` made to answer `-32603` the fuzzer failed ("internal
  error for user input"); with the preflight removed from Check, and with `ma_params` removed
  from `validate`, the five-cause tests failed (engine and server).
- Found by the fuzzer itself: a random `world.cache` value made the server write world-cache
  folders into the working directory; the fuzzer now points it at its scratch directory.

### Soak

- **50 consecutive runs through the page** (`lifecycle.spec.ts`, `VWP_SOAK_RUNS=50`, edits and
  preset switches between runs): 50/50 finished and streamed to their end, no console error.
  Engine physical footprint 137 MB at run 10 and 139 MB at run 50 (range 117–161 MB over the
  eleven samples); tab heap 60.3 → 61.6 MB (range 43–62). A heap-snapshot diff of run 10
  against run 50: 50.0 → 51.4 MB of self size, the growth V8 code and Chrome's performance
  timeline, no application constructor growing. **No leak.**
  - The check now reads the physical footprint, not `ps` RSS: in the first 50-run soak RSS
    read 25.9 MB at run 20 and 132.9 MB at run 30 with the footprint flat — macOS compresses
    a process's pages under pressure. The bound is unchanged.
- **The long soak** (`soak.spec.ts`, `VWP_SOAK_SIM_S`): `credential-lifecycle` with 900
  veh/h, 30 pedestrians, the SCMS lifecycle, attackers, a closure, a demand surge, a weather
  front and the chase feed on a followed vehicle. Its first 240 s probe found a real defect:
  - **A run declared itself finished one step early.** `LiveEngine::step` ended the stream
    when the cursor reached the horizon's index rather than passed it, so a stream that caught
    up with a slow kernel stopped at 239.9 s of 240 s, the last step never streamed and the
    run published no digest. Fixed (`>` for `>=`).
  - With that fixed, the 240 s probe passed: run to its end, no console or page error, tab
    heap 28.5 → 44.1 MB and flat from 90 s (43.3–44.6 MB); 227 s of wall clock.
  - **The live server's seek history had no memory bound.** The long run (1,800 s asked)
    was stopped at 665 s: the engine's footprint grew linearly, 32 → 211 → 482 → 756 →
    1,084 MB at 1, 154, 306, 457 and 608 s, and 1,231 MB at 665 s, while the tab heap stayed
    at 43–45 MB. `run.status` showed why: 6,655 retained steps of a 36,000-step limit. A
    step carries every reception record, about 180–270 kB here with 42 radios under
    `verify-all`, so the step limit alone allowed about 6.5 GB for an hour. The history now
    also has a byte budget (`LiveOptions::retain_bytes`, default 1 GiB, `--retain-mb`);
    older steps are dropped and a seek before them is refused with its range
    (`the_seek_history_is_bounded_by_memory…`). On the same soak with `--retain-mb 200`
    the history held at 209.6 MB (1,988 steps at 456 s).
  - **Something else grew too, and the run slowed as it went.** With the history capped,
    that 480 s soak still failed its flatness check (footprint 132 MB at 97 s, 495 at 305 s,
    659 at 480 s, tab heap flat at 43–45 MB) and slowed from 154 simulated seconds in the
    first 61 s of wall clock to 151 s in 545 s. Located and fixed on 2026-10-06 (below).

### The long soak's growth and slowdown, located (2026-10-06)

Found with `soak_probe` (`crates/v2xw-server/tests/soak.rs`) on the soak's own scenario
(credential lifecycle, 900 veh/h, 30 pedestrians, attackers, a closure, a demand surge, a
weather front; 600 s), which prints the projector's stores, the seek history and the
process's physical footprint every 30 simulated seconds, with `sample` profiles of the
kernel thread and `heap` censuses at 300 s and 570 s. Debug build, shared machine.

| What | Cause | Fix |
|---|---|---|
| Kernel time growing with the CRL | every received certificate was checked against every linkage-CRL entry by walking its seed chain and hashing 20 indices (`CrlStore::revokes_linkage_at_period`): **30 % of the kernel's samples at 450 s** | the store expands its entries into each period's linkage values once per period and CRL version and looks certificates up (`CrlLinkageEntry::values_at`); the node model already treats CRL expansion as a background job (`crl_expansion_pm`) and charges a check by entries consulted (`CrlGate::work`), so no simulated quantity moves. Test: the expansion answers exactly what the walk answers, across new CRL versions and evicted periods; shown red with the cache not cleared on a new entry |
| Kernel time growing with protocol history | `StageLog::run`/`at` filtered the whole log on every backend step (8 % of samples at 450 s) | each run's stamps are indexed by position |
| Server footprint | each `backend.state` snapshot was kept parsed: hundreds of `serde_json` maps a second, the largest growth in the heap census (+50,000 640-byte blocks in 270 s, 32 MB) | kept as the record's bytes (17 kB each), parsed when `inspect.entity` asks; bounded at 600 snapshots and 64 MB |
| Seek history over its budget | the byte estimate counted lengths, not capacities; a step's event list runs up to twice its length | steps are shrunk before they are kept and the estimate counts capacities and the allocator's 16-byte quantum (one estimator, `StepOutput::approx_bytes`, where there were two) |
| Plot polls growing with the run | `metrics.query` scanned every sample of a metric for every bin, under the lock the producer needs | one walk over the (time-ordered) samples (`binned_means`); test against the old scan |
| Departed vehicles' detectors | a retired vehicle kept its misbehaviour-detector suite and rate-limit stamps for the rest of the run | dropped at retirement (`Phase2::retire`) |

Before → after, the same 600 s scenario through `soak_probe` (wall clock on a loaded
machine, noisy; the counts are not):

| | before | after |
|---|---|---|
| wall clock for 600 simulated s | 602–615 s | 314–373 s |
| last 30 simulated s | 92–97 s | 25–29 s |
| kernel samples in the CRL walk at 450 s | 3,410 of 11,528 | not among the top 70 frames |
| footprint at 600 s, `--retain-mb 200` | 465 MB (history 199) | — |
| footprint at 600 s, `--retain-mb 20` | 223 MB | 153 MB |
| heap growth 300 → 570 s, `--retain-mb 20` | 124 → 204 MB | 91 → 140 MB |

What still grows over that window is bounded: the grouped-metrics breakdown store (capped at
1,000,000 entries), the backend history (600 snapshots, 64 MB at most), each metric's plot
series (16 bytes a sample, for the whole run by design) and the PCA's issuance table in
`phase2` (every certificate issued, as a real PCA keeps them; it grows with certificates
issued, not with time).

**The one-hour soak through the page passed** (`soak.spec.ts`, `VWP_SOAK_SIM_S=3600`,
`--retain-mb 200`, debug engine, software-rendered headless Chromium, 1.6 h of wall clock on
the shared machine): the run reached 3,600 s with its digest, the engine never entered
`error`, no page or console error, 15 vehicles followed in turn in the chase view with
their message feed. The seek history held at 199.9–200.0 MB from 606 s. The engine's
footprint beside it: 152, 203, 209, 208, 221, 266, 274, 249, 269, 324, 329 and 249 MB at
305 … 3,600 s — the rise from 1,800 s is the demand surge (live radios 52–55 against 29–40)
and the breakdown store filling to its cap (313,495 entries at 908 s, 1,000,000 from
3,021 s); the end is 249 MB against 209 MB at the warm sample (bound 322). The projector's
stores stayed with the live fleet: 419 radios had driven by 3,021 s, the feed and security
rows held 45–46. Tab heap 44.1–47.7 MB throughout. A `heap` census of the engine at
3,322 s: 557 MB malloced, of which the seek history's events (≈2,200 steps' lists and
their payloads, ≈200 MB) and the breakdown store's deques (≈130 MB at its cap, a deque's
spare capacity included) are the two largest; the latter is the next thing to trim if the
server must fit a smaller budget.

### Resume and replay

- **Random drops** (`resume.spec.ts`, new test): five cuts at seeded random instants in one
  20 s run at real time, some while the page was still reconnecting: 5 resumed Hellos, 221
  frames, seq 0–220 with no gap or duplicate, no page error. The single-drop test: 67 frames,
  resumed at seq 19.
- **Recorded runs could not be replayed by the replay server at all**: the live server wrote
  no `Hello` into its recording ("holds no Hello frame"). It now writes the run's Hello first.
- **Open — a replay does not draw what the live page drew.** `a_real_run_replays_…` (in
  `tests/replay.rs`, ignored with the reason) compares a live run's stream with its recording
  replayed: the recording lacks the MetricSample and Provenance frames (live {Keyframe 4,
  Delta 27, MetricSample 2, Provenance 1}, replay {Keyframe 4, Delta 27}), and the pose
  bodies differ — the live stream carries each vehicle's body centre (`live.rs`
  `body_centre`) while the recording's frames come from the kernel's snapshot stream, which
  keeps the rear-bumper reference (first difference 2,500 mm, half a passenger car). The fix
  is one projection for both, which moves every recording's bytes.
  - The page's own replay (the WebAssembly reader) now applies the live stream's projection
    to what it decodes (`toBodyCentres` in `lib/replay.ts`, with the connected engine's class
    lengths, and a logged note when no engine is connected to give them), so a recording
    opened in the page draws each vehicle where the live page drew it; `replay.test.ts`
    holds it. It is not exercised end to end in the page here: the reader's WebAssembly
    build needs `wasm-bindgen` and a clang with a wasm32 target, which this machine lacks
    (`ui/apps/studio/public/wasm/README.md`). The replay *server* still forwards the
    recorded frames unchanged (§7.2), so it stays half a length off and without the
    MetricSample and Provenance frames until the recording carries the live projection —
    the integrator's decision, since it moves every recording's bytes.

### Flakiness

| Suite | Runs | Result |
|---|---|---|
| `cargo test -p v2xw-server` | 3 | all pass each time (the one failure each time was the replay test above, deterministic, now ignored) |
| `cargo test -p v2xw-server`, 2026-10-06 | 2 | 116 passed, 0 failed, 4 ignored, both times |
| `soak` churn test (`the_projector_keeps_what_is_alive…`) | 3 | 3/3 (alone and in both suite runs); red with the feed's dropping of empty logs disabled |
| vitest protocol / viewer / mock-server / studio | 3 each | 192 / 141 / 45 / 188, all pass every time |
| mock Playwright `scene-validation` (15 tests) | 2 full passes | 30/30 |
| its building test alone, old subject vs new | 8 + 8 | 8/8 and 8/8 |
| engine Playwright `resume` (2 tests) | 1 | 2/2 |
| engine Playwright `settings` (3 tests) | 3 | 3/3 on the third run; the first two failed in the new refusal test's own steps (Check read a stale form; a cleanup click on a disabled button), fixed in the test |

The wave-1 flake ("buildings fill the upper frame", signal 0) did not reproduce in 8
isolated repeats with the old subject either, so its cause is not proven. The test now
follows the equipped vehicle with the most tall buildings ahead of it, chosen from the
decoded world, so its precondition (walls in front of the camera) holds whatever the clock
is when it starts; a vehicle leaving the gridded blocks at the map's edge is the likely
cause, not a demonstrated one.

**A cause of wrong results across worktrees (2026-10-06).** The engine's lib tests failed
in this worktree with `every_number_publishes_a_unit` naming three `actors.vru` keys that
exist in no file of this worktree. They came from the pedestrians track's worktree: the
engine's build-script output (`target/agents/debug/build/v2xw-engine-8c4e…/out/
scenario_reflect.rs`, and the `V2XW_GIT_COMMIT` stamp) lives in one directory every
worktree shares — the per-worktree `codegen-units` in `.cargo/config.toml` separates the
crates' artifacts but not the build script's run — and cargo judged it fresh because this
worktree's `schema.rs` was older than the other track's last run. Any engineer's binary
could carry another track's scenario reflection. This track's builds now set
`V2XW_GIT_COMMIT` to their own commit, which the script watches
(`rerun-if-env-changed`), so cargo reruns it whenever another worktree ran it last; the
lasting fix (a per-worktree `[profile.dev.build-override]` setting, or a target directory
per worktree for build scripts) is the integrator's. With it, the lib tests pass 48/48.

### Settings the engine does not fully apply (`KEY_STATUS`)

No key is `not-implemented`. Three are `refused` and now sit under Unsupported with the
engine's reason: `time.des_resolution` (the kernel keeps nanoseconds; only an unkeepable
resolution is refused), `detection.responder` and `detection.perception_tier` (refused when
set). Thirteen are `partial`, each with its note shown on the field: `time.time_dilation`,
`actors.vru`, `actors.backend.entities`, `weather.surface`, `radio.tiers.focus`,
`messages.sets`, `security.protocol`, `security.signature`,
`security.verification_policy` (no key for its thresholds), `security.pseudonym_change.strategy`
(mix-zone behaves as silent: no world has mix zones), `nodes.compute_tier`,
`nodes.backend_tier` (high adds nothing over medium), `threats.compromised_rsu_attack`. None
was small and safe to wire in this track without touching another track's models.
`TimelineKind::Closure`'s doc now gives the three target spellings the loader reads
(`123`/`lane:123`, `edge:45`, `street:NAME`).

### Engine suite

The integrator asked for one full run after `14366f6`. At `640ad91` (before this track):
199 passed, 3 failed, 2 ignored — the three known failures (`message_sets`' DENM test and
the two `phase2` honest-revocation tests). With this track's changes the full run found a
regression of this track's own making: 38 tests in `measurement`, `propagation`,
`radio_access`, `radio_access_layers` and `reception` offer `scenarios/scale/`'s 3.6 M veh/h,
which the first arrival-rate ceiling refused. With the ceiling raised those five binaries
pass (41 passed, 1 ignored), `scenario` passes 15/15 and every other binary passed in the
full run; the same three known failures remain. `v2xw-cli`'s shipped-scenario test also
failed: `v2xw validate` refused `phase1-manhattan.yaml` because its map path is relative and
the test runs from the crate directory. A missing file is now a note from `v2xw validate`
(the command answers whether the document is valid) and still a refusal from Run.

On 2026-10-06, after the CRL, stage-log and retirement changes: every engine test binary
passes except the same three known failures (`message_sets`' DENM test, the two `phase2`
honest-revocation tests, with the same counts as before); `the_phase_2_run_is_deterministic`
passes. The first attempt failed 41 tests for two reasons of the environment, not the code:
the Manhattan map was not linked into the worktree (each refusal now names
`world.source.path`, the file and the fix — the five-cause work doing its job), and the
shared build-script output described above. `v2xw-conformance`'s golden suite passes (4/4,
the grid-traffic record unchanged). `v2xw-server`: 116 passed, 0 failed, 4 ignored.
`v2xw-sec` and `v2xw-proto`: 228 passed.
## 2026-09-30, resumed 2026-10-06 — the 3D traffic scene (traffic3d track)

Owner: `ui/packages/viewer`, plus a versioned `lamps` byte through the engine and protocol.
Branch `worktree-wf_9470e8ab-93a-8`. Every number below comes from a command run on this
machine; wall times are not claimed (several engineers share one build lock).

### A glitch hunter, and what it found

`src/glitch.ts` watches every drawn frame and counts, per class and per camera mode: pop,
teleport, stutter, heading snap, vehicle–vehicle, vehicle–building and vehicle–person
interpenetration, z-fighting, flicker, LOD pop, camera clip, empty frame, a lost chase subject
and a chase camera looking down instead of along the street. Each event is attributed to the
viewer or to the engine's own data (a slot whose snapshots jump further than it could drive, or
two agents the engine itself put in one place). `test/glitch-hunter.test.ts` injects each
defect into a clean frame and asserts the hunter counts it, and asserts a clean synthetic run
counts nothing.

`test/glitch-capture.test.ts` replays a real engine stream (`scripts/capture-stream.mjs`) at
60 fps with ±0.5 ms jitter and 1 % dropped frames, and tours aerial (20 s), chase over a car,
the largest vehicle, a person and a two-wheeler (15 s each), then dashboard (20 s). The BEFORE
viewer is `main` at `640ad91` with only the hooks the hunter reads. Both viewers replay the same
captures and are judged by the same hunter:

Two captures. **Midtown**: the real Manhattan network with 600 pedestrians, 80 cyclists and
30,000 veh/h of demand (`glitch-midtown-dense.yaml`, 150 s of sim at 10 Hz, 46 MB, from `main`'s
engine), toured from 60 s. **Grid**: a Midtown-shaped procedural grid with 300 pedestrians, 40
cyclists and 6,000 veh/h (`t3d-grid-dense.yaml`, 139 s, from this branch's engine, so with lamps),
toured from 30 s. 5,057 and 4,156 drawn frames. Counts are events; "engine" is the part the
hunter attributes to the stream's own data.

| class | Midtown before | Midtown after | grid before | grid after |
|---|---|---|---|---|
| `pop` | 127 (127 engine) | 0 | 21 (21 engine) | 0 |
| `teleport` | 62 | 359 (359 engine) | 0 | 0 |
| `stutter` | 13,855 in 206 frames | 3 in 1 frame | 6,635 in 129 frames | 72 in 2 frames |
| `heading_snap` | 2,377 | 4 (4 engine) | 1,091 | 0 |
| `overlap_vehicle` | 0 | 0 | 0 | 0 |
| `overlap_building` | 0 | 0 | 0 | 0 |
| `overlap_pedestrian` | 1 (1 engine) | 1 (1 engine) | 0 | 0 |
| `z_fighting` | 1 | 0 | 5 | 0 |
| `flicker` | 0 | 0 | 0 | 0 |
| `lod_pop` | 8 | 0 | 0 | 0 |
| `camera_clip` | 0 | 0 | 0 | 0 |
| `empty_frame` | 0 | 0 | 0 | 0 |
| `subject_lost` | 0 | 0 | 0 | 0 |
| `chase_framing` | 0 | 0 | 0 | 0 |

Reading it:

- **Pops** are actors appearing or vanishing inside the view; the engine spawns them there, so
  the hunter blames it, but the viewer drew them as a blink. They now fade in and out over 0.3 s.
- **Teleports went up, and they are all the engine's.** The Midtown stream moves pedestrians 2–4 m
  in one 0.1 s step (examples: "pedestrian moved 2.43 m in one 16.6 ms frame at 1.5 m/s"). The
  BEFORE viewer drew each as a slide at tens of m/s (62 caught as viewer teleports, the rest as
  stutter); the AFTER viewer recognises a step longer than the reported speeds allow, cuts to it,
  and the hunter attributes the cut to the data. The likely source is
  `v2xw_mobility::vru::social_force` putting a person on the centreline of the next lane of their
  route (`lateral_m = 0.0`) instead of where they stood — see Open.
- **Stutter** was every vehicle on screen jumping when the plan view re-aimed, a street camera
  passing every kink of its subject's path to the picture, and hard landings after a flight. What
  is left is 3 pedestrian events in one dashboard frame at Midtown and 2 frames of pedestrians
  jerking 1.5–1.9 px (against a 1.5 px threshold) just after a landing on the grid.
- **Heading snaps** were people spinning on the spot with the social-force heading; a person's
  drawn heading now turns at most 5 rad/s. The 4 left are cuts at the engine's own jumps.
- **Camera clips**: the hunter first counted 27 (BEFORE) and 21 (AFTER) on the grid, all while
  the camera descended from the plan view into a street. The hunter took world z as the near
  plane's up axis, which for a camera pitched steeply down put the near plane's corners far below
  where they are, inside roofs; with the camera's own axes neither viewer clips, so they are
  counted as the hunter's error, not fixed defects.

### Peak Midtown (resumed 2026-10-06)

A third capture at real peak density: `ui/packages/viewer/test/glitch/midtown-peak.yaml`, the
Manhattan network with 1,500 pedestrians, 150 cyclists and 60,000 veh/h offered in a mixed fleet
(68 % cars, 9 % delivery vans, 6 % motorcycles, 5 % trucks, 5 % buses, 4 % mopeds, 2 % emergency,
1 % coaches). Counted from the capture's own keyframes (`ui/packages/viewer/scripts/census.mjs`): on the end-of-session
capture, 2,372 live actors when the tour starts at 60 s and 3,405 at 150 s — 1,160 cars, 175
vans, 118 motorcycles, 95 trucks, 88 buses, 72 mopeds, 31 emergency vehicles, 16 coaches, 1,500
people and 150 cyclists.
150 s of sim, 1,652 frames, 82 MB; the tour as above (aerial 20 s, chase over a car, a delivery
van, a person and a motorcycle 15 s each, dashboard 20 s), 5,354 frames.

| class | start of this session | end of this session |
|---|---|---|
| `pop` | 45 (45 engine) | 45 (45 engine) |
| `teleport` | 808 (808 engine) | 41 (41 engine) |
| `stutter` | 573 in 20 frames (322 engine) | 235 in 3 frames |
| `heading_snap` | 19 (18 engine) | 0 |
| `overlap_vehicle` | 1 (1 engine) | 1 (1 engine) |
| `overlap_pedestrian` | 2 (2 engine) | 2 (2 engine) |
| `lod_pop` | 4 | 0 |
| `overlap_building`, `z_fighting`, `flicker`, `camera_clip`, `empty_frame`, `subject_lost`, `chase_framing` | 0 | 0 |

What moved them (each its own commit, each with a test shown to fail without it):

- **Pedestrian jumps, 808 → 41.** `v2xw_mobility::vru::social_force` put a person on the next
  lane's centreline at every lane change. A person is now projected onto the next lane where they
  stand, and at a corner — where centreline lanes leave someone on the outside of the turn off the
  next lane's band — an offset (`Pedestrian::carry`) keeps them where they were and closes at
  0.7 m/s, so they walk round the corner. Projection alone took the count to 704; a 3 m carry to
  439; 5 m to 41. The 41 left are 5.3–6.6 m gaps in the extract's walkable network, still drawn
  as cuts. This is the pedestrian track's file: the change is the lane-change block and one field.
- **People jerking near the camera.** Each knot of a person's Catmull-Rom path is now limited
  against both chords that meet at it, so its velocity is continuous; before, a person stepping
  back in a crowd had a zero tangent at one segment's end and a full one at the next's start.
  Stutter frames 20 → 3.
- **LOD pops on buses** (above, "Level of detail by size"): 4 → 0.

Left: 2 of the 3 stutter frames are the frame a camera flight lands (2.0 px of jerk against a
1.5 px threshold, every actor on screen counted at once, hence 235 events), the third is the
followed person at 1.8 px. The 45 pops are vehicles the engine spawns inside the view 2.7 km
away at the tour's first frame. The overlaps are the engine's own (bodies overlapping in the
snapshot).

### What changed

- **Motion.** Critically damped springs replace the exponential lerp in the plan view (it
  started every move at full speed, so every car on screen jumped with each re-aim) and follow
  the subject in chase and dashboard, with its velocity fed forward. Flights between street
  subjects climb over the roofs and descend in a time set by the height they fall. Actors fade
  in and out over 0.3 s instead of appearing; a departed actor lingers to its last pose; a slot
  whose snapshots jump further than it could have moved is snapped (and counted as the
  engine's). People's headings turn at most 5 rad/s and their paths use Catmull-Rom tangents.
- **Chase framing** (the wave 1 integrator's "too steep on `manhattan-vru.yaml`"). The camera
  used to stand 9 m back (up to 12.6 m with speed) and 16° up whatever it followed, so a person
  was a speck seen from above and a bus filled the frame. It now stands half the subject's length plus
  max(4.5 m, 1.5·H + 0.35·L) behind it (3.6 m for a person or rider), looks at 0.72 of its
  height, and sits 13° up (10° for a person or rider): a car from 7.0 m, a bus from 15.3 m, a
  person from 3.7 m (at the default zoom). The hunter's `chase_framing` class fails a chase view
  steeper than 35° or with its subject under 6 % or over 70 % of the frame's height; it counted
  none on either capture.
- **Depth.** Road layers are 4 cm apart and ranked: the surface shader pulls each rank 2 depth
  steps nearer, so a coloured bus lane over a carriageway, a kerb over a junction, and a
  crossing over both keep their order at any range from a 0.35 m near plane.
- **Road users.** Procedural models per class, three LODs each: sedan, crossover, NYC taxi
  livery (20 % of cars, a stated choice), bus, coach, box truck, semi, delivery van, ambulance,
  motorcycle, moped, delivery moped, bicycle, e-scooter and a walking person with a gait cycle.
  Wheels spin and front wheels steer. Motorcycles, mopeds, bicycles and scooters lean into a
  turn by the steady-turn angle tan φ = v²/(g·R) (Cossalter, *Motorcycle Dynamics*), capped at
  30° and settling in 0.25 s, upright below 1.5 m/s. Paint follows a published colour-popularity
  split; people take a stature from NHANES.
- **Lamps from the engine.** A new VWP v1.2 `lamps` byte (brake, left/right indicator, hazard,
  low beam, reverse, emergency beacons) is written by `v2xw-mobility` (`src/lamps.rs`: brake on
  at 1.0 m/s², off at 0.6, held at a standstill; indicators 100 ft before a turn or lane change,
  NY VTL §1163, used by 75 % / 52 % of drivers per SAE 2012-01-0261; beacons on emergency
  vehicles) and by `v2xw-engine` (`src/daylight.rs`: low beams below −6° sun elevation or
  under 304.8 m visibility, NY VTL §375). The viewer lights brake lamps, flashes indicators at
  1.5 Hz, and at night draws headlight pools and lamp points.
- **Streets.** MUTCD lane lines, edge and centre lines, stop bars, lane-use arrows,
  high-visibility crosswalks, and bus (red) and bike (green) lane colours. Signal heads face
  their approach on mast arms or poles, with visors. Lenses have the MUTCD's shapes: a head
  over a lane that only turns, on a signal group no through lane of its approach uses, is a
  separate turn face with arrow lenses (§4D.06, §4D.20) and shows a permissive turn as the
  flashing yellow arrow (§4D.18); a pedestrian head shows the UPRAISED HAND and the WALKING
  PERSON (§4E.04), not discs. The lane-movement reading behind the lane-use arrows is shared
  with the heads, so a lane's arrow on the road and its lens on the mast agree.
- **Level of detail by size.** LOD distances were a car's for every class, so the dense
  Midtown tour caught buses changing model while 40 px tall. They now stretch with the class's
  height (against 1.5 m) and length (against 5 m), at most threefold, never below a car's.

### Evidence

All from this branch at the commits named; wall times are from a loaded, shared machine.

- **Viewer suite**, `vitest run` file by file (`--maxWorkers=1`): 22 files, 187 tests pass, one
  skipped (`glitch-capture.test.ts` without a capture); `tsc -p tsconfig.test.json` clean.
  Before this resumed session the branch did not compile: `2957cc0` declared `across` twice in
  `cameras.ts` (fixed in `8b56fb4`).
- **The hunter can fail**: `test/glitch-hunter.test.ts` (17 tests) injects each class into a
  clean synthetic frame and asserts it is counted, and asserts a clean run counts nothing.
- **The lean can fail**: flipping its sign in `actors.ts` fails 2 of the 3 tests in
  `test/two-wheeler-lean.test.ts`; restored, 3 pass.
- **The lamps byte can fail**: removing `|| t.lamps != r.lamps` from the encoder's moved-row
  test fails both tests in `crates/v2xw-record/tests/lamps.rs`; restored, both pass.
- **Captures**: `test/glitch-capture.test.ts` with `VWP_GLITCH_CAPTURE` on each capture, seed
  24593, 20 s per view (the table above; BEFORE is `uibase`, `main`'s viewer with the hooks, run
  with the same `glitch.ts` and tour).
- **Per-frame budget at 5,000 actors** (`test/budget.test.ts`, `--expose-gc`, a synthetic 16×16
  block grid, 600 frames at a simulated 60 fps with 10 Hz deltas, all 5,000 in view): viewer CPU
  (interpolate, cull, LOD, matrix and animation write) mean 2.23 ms, wall mean 2.50 ms, p95 4.35,
  p99 5.06 ms; 70 draw calls; instance capacity 10,496 before and after; heap +0.14 MB after a
  forced GC. `main`'s viewer on the same test: CPU mean 1.31 ms, wall mean 1.62 ms,
  p99 3.52 ms, 67 draw calls. The animated, lit, fading models cost about 0.9 ms a frame more
  at 5,000 actors; both are far inside the test's 8 ms CPU ceiling (single runs on a loaded
  machine: the difference is indicative, not precise).
- **Golden**: `grid-traffic` re-blessed for the lamps field (`be81df7`); the new digest is
  byte-identical at `RAYON_NUM_THREADS=1` and `=2`, and only `gt.kinematics` bytes moved
  (108,943 → 111,733; 619 records either way). `cargo test -p v2xw-conformance --test kit golden`: 4 pass against
  the new record; `cargo test -p v2xw-record`: 241 pass.
- **Protocol package**: 13 files, 194 tests pass (the lamps round trip in `test/lamps.test.ts`).
- **Signal lenses**: `test/signal-lenses.test.ts` (3 tests) fails 2 of 3 with the group check
  or the flashing-yellow-arrow rule removed. **LOD by size**: `test/lod-by-size.test.ts` fails
  with the stretch removed. **People's paths**: `test/vru-path.test.ts` measures a 1.34 m/s
  velocity jump between 1 ms samples with the old one-chord limiting, under 0.15 with the new.
  **Pedestrian lane changes**: `a_pedestrian_never_jumps_at_a_lane_change` in
  `v2xw-mobility` fails on the old lane change ("a12 moved 0.27 m in one step, limit 0.24 m");
  `cargo test -p v2xw-mobility --lib vru`: 16 pass.
- **The page, dense Midtown** (`e2e-engine/traffic3d.spec.ts` against this branch's debug engine
  on a 600-pedestrian, 60-cyclist, 20,000 veh/h copy of the peak scenario, 25 s per view;
  pictures in chase over a car, a box truck, a person and a motorcycle, dashboard and aerial,
  BEFORE from `main`'s viewer source on the same engine and scenario). Headless Chromium draws
  through SwiftShader, a software rasteriser, and on this loaded machine it drew that scene at
  4 frames a second (every view's mean frame time 247–250 ms, the hunter's cap). At 4 fps the
  per-frame classes cannot be judged — a person walking 1.3 m/s moves 0.33 m between frames and
  turns up to 72° — so that run's teleport, stutter and heading-snap counts (976, 781 and 4,618
  in the worst views) say how slow the rasteriser was, not how the scene moves; the 60 fps replay
  above is the measurement of motion. The pixel class is valid at any rate: 394 frames sampled
  over six views, one empty (luminance σ 0.8, in the chase view during the fly-down from the
  plan view), and the hidden-scene control read as empty as it must. That empty frame is not
  explained yet (see Open).
- **The page, the spec's own scenario** (`vru-grid.yaml` at 6,000 veh/h, 12 s per view): the
  same rasteriser managed 4–8 frames a second here too (mean frame time 126–245 ms by view), so
  again only the pixel class is judged: 197 frames sampled over five views, none empty (lowest
  luminance σ 10.1). The motion classes it reports at that rate (e.g. "pedestrian turned 23.9° in
  one frame": the viewer's people turn at up to 5 rad/s, 36° in a 126 ms frame, against the
  hunter's 20° per-frame limit, which assumes 60 fps) are the frame rate's, and the spec fails on
  them; a headed run on a GPU (`VWP_HEADED=1`) is what the page check needs to judge motion.


### Open

- **Pedestrian jumps: 41 left on peak Midtown, and a change in the pedestrian track's file.** This
  session fixed the lane change in `v2xw_mobility::vru::social_force` (projection plus a corner
  carry, see "Peak Midtown"); the integrator should expect it to meet the pedestrian track's
  edits in the same block. What is left are 5.3–6.6 m gaps in the walkable network, above the
  5 m carry (`CARRY_MAX_M`), drawn as cuts. Raising the cap would draw a person sliding across
  what may be a road; joining those gaps belongs in the importer or the router. No golden holds
  pedestrians, so no digest moved; any pedestrian scenario's output did.
- **One empty frame in the page's chase view** (dense Midtown, SwiftShader at 4 fps): σ 0.8 on
  one of 86 sampled frames, while the camera flew down from the plan view. The 60 fps replay of
  the same kind of scene counts no `empty_frame` and no `camera_clip`, so it is either a frame
  in mid-flight looking at one flat roof, or something only real pixels show. Not resolved.
- **The allocation witness is near its bound.** End of session, `vitest run` over the whole
  viewer package (`--maxWorkers=1`): 25 files, 191 tests pass, 1 skipped (the capture test), 1
  fails — `render-perf.test.ts` "holds its per-frame allocation near zero". Run alone with
  `--expose-gc` it passes at 58.4 B/frame against its 60 B/frame bound (6.2 B/frame in the
  viewer's own sources; largest site in the protocol's compiled `messages.js`), where the test's
  own comment records 11–15 on `main`. Its deterministic guards pass (0 `addUpdateRange` calls,
  0 of 24 update ranges reallocated). Which change since `main` added the bytes — this branch's
  lamps column through the protocol is the first suspect — was not found.
- **Landing stutter.** 2 of the 3 stutter frames left on peak Midtown are the frame a camera
  flight hands over to the follow: 2.0 px of jerk against the 1.5 px threshold, counted once per
  actor on screen. The flight ends at the subject's velocity; the jerk is the follow spring
  meeting a person's own wobble.
- **Night in the Studio.** Every NYC scenario starts at `2027-03-04T08:00:00Z`, which is 03:00
  EST, so the engine's headlamp rule turns every low beam on. The Studio pins the viewer's sun at
  11:00, so the page draws lit headlamps (dimly, as by day) under a noon sky. Either the
  scenarios meant 08:00 local (`13:00:00Z`, which moves every digest) or the Studio should call
  `viewer.setSunFromRun(true)` (one line in `apps/studio/src/state/engine.ts`, the viewport
  track's file), which draws the run at the hour the engine simulates.
- **Protocol fields for the pedestrian and motorcycle track.** The viewer needs nothing beyond
  what v1.2 carries: class, pose, heading and speed drive the gait, the steer and the lean; the
  `lamps` byte covers a motorcycle's brake lamp, indicators and headlamp. A jaywalker is drawn
  where the engine puts them. If that track adds a class (e-bike, cargo bike, wheelchair), a
  model per class name in `vehicle-models.ts` `modelVariants` is the one place to add it;
  unknown names fall back by size.
- **Large vehicles in the tour.** The peak capture's tour followed a delivery van (and the page
  a box truck); a bus has been followed only by the framing's unit tests.
- **The capture test is skipped in CI**: a capture is tens of megabytes and needs a running
  engine. The hunter's own tests run everywhere; the Playwright spec
  (`e2e-engine/traffic3d.spec.ts`) runs the hunt in the real page against the real engine.

## 2026-09-30 — six tracks merged: perf, roadnet, traffic, radio, scms, shell

Six engineers worked in isolated worktrees on the owner's 2026-09-29 request (traffic and
network realism, every SCMS entity wired in, a simpler page with a settings window and a
full-screen metrics panel). The integrator merged them into `main` in the order perf
(`8c56ba3`), roadnet (`f2d8db2`), traffic (`9534729`), radio (`19ee03c`), scms (`026e2ae`)
and shell (`4d3faf6`). After each merge it built and tested the crates that merge touched,
and it fixed the seams between tracks in commits of their own. Every number below comes from
a command run on this machine. A number marked *(track)* is the track's own measurement,
repeated from its report and not re-measured here. The release binaries were built from
`ad473c8` and driven in a browser.

### What each track delivered

- **Perf** (`crates/v2xw-core`, `v2xw-radio`, `v2xw-engine`, protocol and viewer packages).
  - A frame's link geometries (antennas, focus placement, buildings, street corner, vehicles
    on the path) are computed in parallel. The stateful budget half still runs in receiver
    order, in `run/link.rs`.
  - Building queries walk the R-tree once along the segment.
  - The vehicles on a link's path come from a body grid.
  - A despawned node's per-link RNG streams are swept.
  - Link streams are parked as key and position beyond a live budget.
  - SHA-256 uses the ARMv8 instructions.
  - The page preallocates pose slots for the traffic it has, not for the 2^20 the engine
    allows.
  - No digest moved *(track)*.
  - Still open: the 20 s TR 36.885 drop (1,250 vehicles) is a batch job, about 142 s of wall
    clock per simulated second *(track)*.
- **Roadnet** (`crates/v2xw-world`).
  - Street cross-sections, including NYC's 10 ft lanes and pedestrian minimums rounded up.
  - Lane-weighted, coordinated signal plans: `world.signals.coordinate` and
    `progression_speed_mps`.
  - Portland (`urban-us-portland`, `scenarios/portland-downtown.yaml`) and German (`urban-de`)
    presets.
  - Kiosks are taken off the carriageway, turns a car cannot make are dropped, right turns
    yield to the cycle lane, and Transit Mall bus lanes are imported.
  - Validation baselines for three cities are in `worlds/validation/`.
  - Every OSM world's content hash changed.
- **Traffic** (`crates/v2xw-mobility`).
  - City drivers are calibrated to the HCM (`IdmPreset::UrbanHcm`), with reaction times and
    amber heterogeneity; a stop is planned at up to AASHTO's 3.4 m/s².
  - Rules of the road per jurisdiction (`TrafficRules`): right turn on red is prohibited in
    NYC and allowed after a stop elsewhere.
  - A calibration harness (`calibration.rs`, `examples/traffic_calibration.rs`) measures
    saturation headway, Raff critical gap and free-flow spread against published figures.
  - The auditor judges right-on-red and dilemma-zone ambers.
- **Radio** (`crates/v2xw-radio`, engine radio wiring).
  - SAE J2945/1 congestion control with its PER bookkeeping, and J3161/1 rate control on the
    sidelink.
  - Adjacent-channel interference (`radio.adjacent_channel`, ACIR at the rules' minima).
  - TR 37.885 vehicle antenna patterns (`radio.devices.obu.antenna_pattern`).
  - Region channel plans (`radio.region`, `radio.channel`).
  - Field and lab checks: the Qualcomm CBR, and a field LOS range.
  - Still open: the 5X CBR is 0.753 in the model against 0.875 measured in the lab *(track)*.
- **SCMS** (`crates/v2xw-proto`, `v2xw-node`, engine backend step, Studio Backend view).
  - Post-quantum hybrid credentials (`hybrid`), charged at liboqs costs.
  - The backend step runs over cellular and RSU access.
  - `backend.state` snapshots are served by `inspect.entity {entity: "backend"}` and drawn by
    the Backend view: every entity, with live counters and the links between them.
  - `scenarios/credential-lifecycle.yaml` (US SCMS) and `ccms-lifecycle.yaml` (ETSI CCMS).
- **Shell** (`ui/apps/studio`).
  - The left settings sidebar is gone. Settings open in a window modelled on VS Code's
    settings (gear or Cmd/Ctrl+comma), with:
    - search by name, description and path, and a group tree;
    - Modified and Unsupported filters;
    - edit, Undo and Reset to default;
    - a JSON view;
    - Check, Discard, Apply and Run;
    - a detached-window mode.
  - The header is reduced to run state, scenario, clock, the primary action, Metrics, the
    inspector, the gear and a menu.
  - Metrics is a full-screen panel.
  - 106–113 labelled elements before, 33 after; the viewport went from 37% of the window to
    86% *(track)*.

### Seams fixed at integration, each in its own commit or merge

- **Traffic × roadnet** (`9534729`). The traffic track's rules table did not know roadnet's
  new presets, so a Portland world fell back to New York's rule (no right turn on red).
  - `urban-us-portland` now permits right on red after a stop (ORS 811.360(1)(a)).
  - `urban-de` follows StVO §37(2), as `sumo-german` does.
  - The `world.highway_preset` note in KEY_STATUS carries both tracks' text.
- **Radio × perf** (`19ee03c`). Perf split `link_budget` into a parallel pure half and a
  sequential stateful half; radio added antenna-pattern gain to that function.
  - The pattern gain moved into `LinkView::geometry`, right after the endpoints. It changes
    gains only, never positions.
  - The transmitter's heading and velocity come from perf's `node_actor` lookup.
  - `link_budget` returns the LOS class the sidelink's curves need.
- **Radio × roadnet** (`9493def`). Two sidelink tests broke on the new Manhattan geometry
  with no radio file changed. Bisected by reverting merges: traffic reverted, still red;
  roadnet reverted too, green.
  - The bulk-spawned fleet now lands elsewhere: the nearest vehicle to the world's centre
    went from 71 m to 405 m.
  - The jammer test now places the jammer at the fleet's medoid: SCIs missed 0.055 quiet
    against 0.192 jammed.
  - The focus-region test now keeps buildings on, so marginal links exist.
  - No assertion changed.
- **SCMS × shell** (`4d3faf6`). The Backend button was written for the old header. It now
  sits in the new header after Metrics, as a labelled icon button.
- **Roadnet × signal streaming** (`14366f6`). On Manhattan, 35 of 1,452 head groups are red
  in every phase. For two of them the phase durations sum to 90.00000000000001 s on a 90 s
  cycle, so `signal_heads` failed. `GroupSignal::at` now caps the time to the next change at
  one cycle.
- **Golden** (`ad473c8`). `grid-traffic` was re-blessed from `edca51b8…` (15,967 records) to
  `8c94ac18…` (15,957).
  - `gt.kinematics` has the same record count and different bytes, which fits the new
    drivers. The move was not bisected to a single merge.
  - Before blessing, `v2xw-golden-digest grid-traffic --json` gave byte-identical output
    twice at `RAYON_NUM_THREADS=1` and once at 4.
- `cargo fmt --all` (`dadb1c0`): formatting only.

### Evidence

- **Rust, one crate at a time, debug, at `dadb1c0`:**

  | Crate | Passed | Failed | Ignored |
  |---|---|---|---|
  | core | 217 | 0 | 0 |
  | msg | 210 | 0 | 1 |
  | net | 113 | 0 | 0 |
  | sec | 99 | 0 | 0 |
  | proto | 128 | 0 | 0 |
  | node | 193 | 0 | 0 |
  | radio | 316 | 0 | 9 |
  | world | 242 | 0 | 4 |
  | mobility | 238 | 0 | 0 |
  | metrics | 251 | 0 | 0 |
  | record | 239 | 0 | 0 |
  | threat | 210 | 0 | 0 |
  | experiment | 80 | 0 | 0 |
  | copilot | 68 | 0 | 0 |
  | server | 107 | 0 | 1 |
  | cli | 16 | 0 | 0 |
  | wasm | 6 | 0 | 0 |
  | py | 16 | 0 | 0 |
  | conformance | 68 | 1 | 0 |
  | engine | 192 | 4 | 2 |

  - The conformance failure was the golden. After `ad473c8` the kit's golden tests pass, 4
    of 4.
  - One engine failure was `signal_heads`. After `14366f6` it passes 2 of 2, and
    `v2xw-world` passes in full again.
  - The other three engine failures are open (below). The full engine suite was not run
    again after `14366f6`.
- **UI, at `dadb1c0`:**
  - vitest: protocol 192, viewer 141, mock-server 45 and studio 183, all passing.
  - Builds and typechecks are clean for all four packages.
  - Studio eslint: 0 errors, 4 warnings, all of which predate this round.
- **Playwright, one worker:**
  - Engine suite against the release `v2xw-server`: 12 of 12 passed in 5.2 min.
  - Mock suite: 24 passed, 1 failed. The failure was "buildings fill the upper frame at
    street level" (signal 0, needs more than 0.05). Run again on its own, it passed, so it
    is flaky: it depends on which actor is followed.
- **Release build.** `cargo build --release -p v2xw-server -p v2xw-cli` at `ad473c8`
  finished in 7 min 55 s.
- **Live check**, release server on :8787 with `scenarios/manhattan-5min.yaml` and Studio dev
  on :5173, with each screenshot looked at:
  - Settings: Cmd+comma opens the window with 122 settings. The search "vehicles per hour"
    finds 3. The rate was edited from 1500 to 2400. Apply said "Applied 1 change", Run
    started generation 1, and `scenario.get` showed the running rate at 2400.
  - Aerial view: 56 vehicles moving.
  - Follow: `selectFirstActor` gave "follow: node 0". The inspector listed its BSMs with
    pseudonym and position, and the HUD was full.
  - Chase view: behind the car at a red, with the crosswalk and the signal heads drawn.
  - Metrics: a full-screen panel with five plots, delivery against distance, the delay by
    stage and the worst nodes.
  - `manhattan-vru`: pedestrians drawn alongside the vehicles.
  - Backend: `credential-lifecycle`, loaded from the settings window's ready-made list,
    drew all 18 SCMS entities with 13 links in use. Counts moved while the run went on: the
    PCA's certificates rose from 240 to 250 between 95 s and 105 s, and the RA's batches
    from 48 to 50.

### Open

- **An honest vehicle is revoked in `phase2`.** Two tests fail:
  `a_report_becomes_a_vehicle_that_cannot_sign` (`revoked_honest` 1) and
  `the_revocation_latency_is_decomposed_by_stage` (`crl_past_horizon` 1).
  - Bisected to the roadnet merge: green without it, red with or without traffic.
  - Instrumented run: node 28 is honest. Eight reporters flagged it with
    `headingInconsistency` between 257.1 and 257.9 s, then with
    `positionSpeedInconsistency`, and the MA revoked it. Its ground truth over 256–263 s is
    a straight line at 10.2 m/s with no heading change.
  - So the detectors reacted to what its messages said, not to how it moved. Likely cause:
    a GNSS outlier draw that the new world's timing moved onto this vehicle. That was not
    verified.
  - Needs a decision from the security owner: should one event heard by eight reporters
    count as eight pieces of evidence at the MA?
- **`message_sets::a_denm_is_raised_by_hard_braking_and_by_nothing_else` fails:** "no vehicle
  braked hard". With the traffic track's drivers, 40 s at 20,000 veh/h on
  `connected-intersections` has a hardest braking of −3.02 m/s², and 0 of 18,052 samples
  reach the DENM threshold of 3.92 m/s². That is realistic, so the test needs a
  deterministic emergency (a cut-in, a pedestrian stepping out), which the scenario format
  cannot express yet.
- **The owner's default scenario has no credential system.** `manhattan-5min.yaml` sets no
  `security.protocol`, so its Backend view says "No backend to show". The SCMS runs only in
  `credential-lifecycle`, `ccms-lifecycle`, `phase2-manhattan`, `pseudonym-privacy` and
  `revocation-latency`.
- **The Backend panel covers the header.** Metrics keeps the header visible; Backend hides
  it.
- 35 Manhattan head groups never show anything but red.
- The chase camera on `manhattan-vru` looked steeply down at the car. On `manhattan-5min` it
  sat behind the car.
- Each track's own open list stands, in its section below (roadnet, traffic) and in its
  report.

## 2026-09-29 — road network realism (roadnet track)

Owner: `crates/v2xw-world`. Every number below comes from a command run on the shared
machine: `world_report` (the new validation report, `crates/v2xw-world/examples/
world_report.rs`) or `traffic_audit` on debug builds. Wall times are noisy (five engineers
sharing one build lock) and none is claimed.

### What the importer now builds

- **The street's whole cross-section** (`crates/v2xw-world/src/section.rs`). Bus lanes
  (`bus:lanes`, `lanes:bus`, `psv:*`, `busway`), painted cycle lanes and parking-protected
  tracks (`cycleway*`, with `:oneway`, `:width`, `:buffer`, `:traffic_mode`), and parking
  lanes (`parking*`, `parking:lane*`), laid out kerb to kerb: the carriageway centred on the
  way, a track beyond the kerb with a 3 ft buffer, a painted lane between parking and
  traffic. NYC DOT Street Design Manual widths (11 ft bus, 8 ft parking, 5.5 ft painted,
  5 ft + 3 ft track); a `width` tag is shared by the general lanes after the parking and
  painted cycle lanes are taken out of it. Bus lanes admit buses and emergency vehicles,
  and bicycles only where there is no cycle lane (NYC's rule); parking lanes admit
  nothing. Portland's Transit Mall (`lanes=1`, `psv:lanes=designated|`) is read as two lanes.
- **Movements that respect the lanes.** A car's connector never enters a bus lane, a
  track or a parking lane; a bus lane's connector is a bus path; a cycle lane runs on into
  the next block's cycle lane through a connector (no teleport) and turns only towards its
  own kerb. A movement that turns across a lane of its own approach yields to the traffic
  going straight on in it (NY VTL §1160/§1146): a left turn off 1st Avenue gives way to the
  track beside it.
- **Signal plans shaped like Midtown's.** 90 s cycles (Manhattan's pre-timed range is
  90-150 s, NYC DCP 61st Street FEIS ch. 14). The green is shared by the lanes each phase
  serves (Webster with lanes as the flow proxy) — a four-lane avenue gets about twice the
  green of a two-lane street — but never below the time a pedestrian needs to cross the
  road that phase stops (MUTCD 2009 §4E.06: 7 s walk, 3.5 ft/s). The major phase runs first
  and each plan is offset one travel time after its upstream neighbour: a green wave along
  each avenue at the speed limit (NYC DOT's 25 MPH Signal Retiming), or at
  `world.signals.progression_speed_mps` (NYC's 15 mph cyclist Green Wave, Portland's
  11-13.5 mph downtown timing). New scenario keys `world.signals.coordinate` and
  `world.signals.progression_speed_mps`.
- **Junction geometry.** Junctions whose connecting road is shorter than both junction
  areas are joined (netconvert `--junctions.join`, decided by what is left of the road, at
  most 30 m and a 45 m spread); lanes of different roads laid over each other are moved
  apart and pavements moved to the kerb of the carriageway the map implies
  (`src/separate.rs`); fork and merge arms are cut back until they stop overlapping; dead
  ends too tight for a U-turn become trip ends; continuation nodes (a name change) keep a
  1 m area on a straight street and room for the design radius where it bends; movements
  that are really U-turns across a divided street, or that start behind the approach, are
  dropped; a junction's shape covers its connectors; two connectors closer than a car's
  width conflict.
- **Crossings and buildings.** A `highway=crossing` node with no crossing way gets a
  synthesised crosswalk between the nearest pavements; a kiosk-sized building (under
  25 m²) a car's body would enter on a drive lane is dropped (Midtown's newsstand).
- **Two more jurisdictions.** `urban-us-portland` (ORS 811.111: 20 mph business district;
  Portland's 20 mph residential streets) and `urban-de` (StVO §3: 50 km/h). A Portland
  scenario, `scenarios/portland-downtown.yaml`, and `worlds/fetch.sh` to download any box.
- **World validation** (`crates/v2xw-world/src/validate.rs`): geometry a car, a cyclist or
  a pedestrian cannot use, and every imported attribute against the source tags — lane
  counts, one-way, `turn:lanes`, bus, cycle and parking lanes, `width`, `maxspeed`,
  crossing nodes — with a baseline gate (`world_report --baseline`,
  `worlds/validation/<city>.json`) that exits 1 when a check gets worse.

### Measured

`world_report` on each city (checks count lanes, ways or nodes that fail, out of those
examined). "Before" for Manhattan is this track's own earlier state on 2026-09-29, after
the junction join and the pavement separation and before the cross-sections (the
previous run's `world_report`, same extract, same box); the QA figures of 2026-09-24 are
quoted where the QA measured the same thing.

| Check (Manhattan, D7 box) | QA 2026-09-24 | before | now |
|---|---|---|---|
| sidewalk lanes on a drive lane, > 0.3 m | 811 of 7,741 (10.7 km) | 54 of 6,233 (98 m) | 36 of 6,233 (68 m) |
| bus lanes against the tags | — | 174 of 175 wrong (none built) | 0 of 197 |
| cycle lanes tagged, none built | — | 196 of 196 | 0 of 196 |
| parking lanes tagged, none built | — | 77 of 77 | 0 of 82 |
| `width` tag against the built carriageway, > 15 % | — | 27 of 44 † | 6 of 25 |
| one-way against the tags | — | 19 | 1 |
| car's body in a building (the newsstand) | 12 audit steps | 1 lane | 0 |
| U-turn connectors tighter than a car | 118 | 0 | 0 |
| connectors a car turns through faster than it can steer | — | 9 | 6 |
| stub drive lanes (< 2 m) | — | 9 | 13 |
| `turn:lanes` entries a movement violates | — | 31 of 339 | 35 of 329 |
| pavement laid along a cycle track (new check) | — | — | 47 lanes (1.7 km) |

† measured by the check before it was fixed: it summed every piece of a way (way
458166897, three pieces of 21.3 m, "built 63.9 m").

What Manhattan now carries: 225 bus lanes (12.0 km), 275 cycle lanes and tracks
(16.3 km), 111 parking lanes (8.9 km); 269 signal plans on 90 s cycles, 213 of them
offset for progression, the first (major) phase 56 % of the cycle on average.

The other two cities, imported with nothing changed but the extract, the box and the
preset:

| | Portland (`urban-us-portland`) | Berlin-Mitte (`urban-de`) |
|---|---|---|
| extract (Overpass, 2026-09-30) | 6.8 MB | 11.4 MB |
| lanes / junctions / signal plans / buildings | 9,845 / 2,334 / 186 / 640 | 6,989 / 2,319 / 37 / 710 |
| bus, cycle, parking lanes built | 127, 315, 108 | 38, 104, 257 |
| lanes, one-way, bus, parking against the tags | 0, 0, 1, 0 wrong | 0, 0, 0, 1 wrong |
| cycle lanes tagged, none built | 0 of 135 | 2 of 91 |
| sidewalk on a drive lane | 47 of 3,037 (287 m) | 43 of 3,990 (286 m) |
| connectors turning faster than a car can | 62 of 2,363 | 10 of 1,123 |
| car's body in a building | 4 lanes | 10 lanes |
| `width` against the built carriageway | 0 of 1 | 16 of 165 |
| signal plans offset for progression | 117 of 186 | 12 of 37 |

`traffic_audit`, 300 s, debug build. Manhattan: `manhattan-5min.yaml --rate 6000
--pedestrians 200 --cyclists 50` (the QA's dense run had the same rate; its VRU counts are
not recorded, so the pedestrian classes are not strictly comparable). Portland:
`portland-downtown.yaml --pedestrians 100 --cyclists 30`. Berlin: the same scenario with
Berlin's extract, box and `urban-de`, `--rate 1500 --pedestrians 100 --cyclists 30`.

| Class (vehicle-steps) | Manhattan QA 2026-09-24 | Manhattan now | Portland | Berlin |
|---|---|---|---|---|
| vehicles (peak) | — | 428 (420) | 41 (38) | 132 (107) |
| heading jump | 147 | 3 | 5 | 6 |
| in a building | 12 | 0 | 0 | 1 |
| vehicle-vehicle overlap | 0 | 2 | 0 | 7 |
| vehicle-pedestrian overlap | 15 | 12 | 0 | 10 |
| gap below s0 | 217 | 245 | 0 | 0 |
| jerk | 37 | 206 | 4 | 61 |
| step vs reported speed | 18 | 8 | 0 | 28 |
| conflict zone | 0 | 0 | 0 | 147 |
| body outside its junction | 0 | 0 | 0 | 60 |
| occupied-crosswalk entry | 0 | 1 | 0 | 0 |
| every other class | 0 | 0 | 0 | 0 |

Two identical Manhattan audits (before and after a 0.01 s change to the pedestrian
minimum that moved no count) gave identical counts; so did every repeated Portland and
Berlin audit.

### Still open, most important first

1. **Berlin's unsignalised junctions.** 147 conflict-zone steps, 60 body-outside-junction
   steps and 7 overlap steps, at Mitte's unsignalised junctions (37 plans for the whole
   box). Not diagnosed: the junctions and movements are named in the audit's
   examples, and whether the cause is the priority rules (mobility) or the geometry of
   Mitte's skewed, joined junctions (this track) is the next question.
2. **Pavements on cycle tracks.** 47 Midtown pavement lanes (1.7 km) lie along a track.
   The track is laid 3 ft beyond the parking lane, outside a carriageway centred on the
   way, and where the mapped pavement is there the two share a band — the auditor sees
   cyclists through pedestrians. Moving the pavements clear the way they are moved clear
   of drive lanes was tried and reverted: pavements squeezed between a track and a drive
   lane stayed where they were and pavement-on-roadway went from 53 lanes (95 m) to 69
   (646 m). Counting the track inside the carriageway was tried and reverted too (648 m).
3. **Jerk and gap below s0 on dense Manhattan** (206 and 245) are higher than the QA's
   37 and 217. Both are car-following onsets (a follower creeping behind a stopped
   leader, braking hard at a queue); the network changed under them — narrower 10 ft
   lanes, bus lanes that cars cannot use, turning traffic now yielding to the cycle lane
   beside it — and no class the importer controls moved with them. The mobility track
   owns both.
4. **Vehicle-pedestrian overlaps, 12 steps.** A left turn meeting a pedestrian on the
   crosswalk it crosses (mobility's crosswalk yield) and a bus's right turn from the kerb
   bus lane cutting a 1 m pavement stub at the corner (geometry: the stub is a trim
   artefact).
5. **Portland's connectors**: 62 turn faster than a car can steer, mostly by 1-2°; the
   5 heading jumps are at driveways (`service` ways of 10-26 m) meeting the street at
   short, sharp corners.
6. **`turn:lanes` violated, 35 of 329**: the lane is tagged to turn and the junction the
   way reaches has no departure that way (the turn is at a junction further on, or the
   join moved it); the lane is given its straightest movement rather than none.
7. **Crossing nodes with no crosswalk**: 85 Manhattan, 161 Portland, 21 Berlin — a
   pavement on one side only, or none within 25 m.
8. **The engine's build script shares its run directory between worktrees.** The
   scenario-schema reflection (`crates/v2xw-engine/build.rs`) writes to one
   `target/agents/debug/build/v2xw-engine-*/out` for every worktree, so an engineer's
   engine tests can run against another worktree's schema: this track's first run of the
   publish tests saw the radio track's `radio.region` fields and not its own
   `world.signals`. Touching `src/scenario/schema.rs` before building makes it rerun.


## 2026-09-29 — traffic behaviour realism: a calibration harness, and the model held to it

The traffic is now measured the way a traffic engineer measures a street, against the
published figure for each measurement, and the car-following, junction and signal
behaviour was changed where it did not match. The measures live in
`v2xw_mobility::calibration`. Two tests in `crates/v2xw-mobility/tests/calibration.rs` hold
the model to its bands. Every figure below comes from a run on this machine;
`cargo run -p v2xw-engine --example traffic_calibration -- <scenario> --rate R --duration S
--examples N` prints the whole table and the auditor's examples for any scenario.

### What is measured

| Figure | Reference (band) |
|---|---|
| saturation headway, through, from the 5th queued car | HCM base 1,900 pc/h/ln × CBD 0.90 → 2.11 s (1.80-2.40) |
| start-up lost time over the first four | HCM 2.0 s (1-3) |
| headways by queue position | Greenshields 1947: 3.8, 3.1, 2.7, 2.4, 2.2 s (secondary) |
| queue spacing, front to front | HCM 7.6 m (6.5-8.5) |
| free-flow speed over the limit: mean, and spread (p85 − p15)/2.07 | SUMO speedFactor N(1, 0.1) (0.93-1.07; 0.05-0.15) |
| launch acceleration 0-8 m/s | Wang et al. 2004: 1.25 m/s² (0.9-2.0) |
| stop deceleration, 85th percentile | AASHTO 3.4 m/s² (1.5-3.4) |
| permitted-left critical gap, Raff's method on lags timed to the opposing car's arrival | HCM 6th ed. Ch. 31: 4.5 s (3.5-5.5) |
| pedestrian walking speed, mean and 15th percentile | Knoblauch 1996 (1.2-1.6; 0.97-1.4) |
| pedestrian signal compliance | Basch et al. 2015, Manhattan: about 89 % on Walk (secondary; reported) |
| avenue and side-street volumes over signalised stop lines | NYC DOT automated counts, Midtown 08:00-19:00: avenues 970-1,890 veh/h, streets 330-640 (reported) |
| turning speeds, travel speed | reported only |

### Before and after

**The saturation-flow field study** (`SaturationExperiment`: one signalised crossroads,
approaches kept loaded, 600 s). The harness did not exist before this track. The "before"
column is the engine's previous driver set, Kesting 2010 (T 1.5 s, a 1.4 m/s²), measured on
the same experiment.

| | Kesting 2010 drivers | city drivers (UrbanHcm) |
|---|---|---|
| saturation headway | 2.68 s (1,343 veh/h/ln) — OUT | **2.134 s** (1,687 veh/h/ln), n = 531 |
| start-up lost time | 2.80 s | **2.85 s**, n = 49 |
| headways 1-5 | 4.14, 3.43, 3.04, 2.86, 2.77 | **3.73, 2.89, 2.51, 2.34, 2.24** (10th: 2.09) |
| launch acceleration | 0.95 m/s² | **1.16 m/s²** |

**The Midtown-shaped grid test** (5 × 8 blocks of 274 × 80 m, 25 mph, 120 pedestrians, 300 s):
free-flow mean 0.970 and spread 0.098, launch 1.47 m/s², stop deceleration p85 1.74 m/s²,
walking speed 1.306 m/s mean and 0.981 m/s p15 — all in band and held by the test. Its
permitted-left critical gap is 5.96 s over 15 accepted and 15 rejected lags, above the
band; it is reported, not held (see Still open).

**phase1-grid at 6,000 veh/h for 300 s, no pedestrians.** The before run used the harness
binary built before any behaviour change.

| | before | after |
|---|---|---|
| free-flow mean / p15-p50-p85 | 0.934 / 0.81-0.98-1.00 | 0.979 / 0.88-1.00-1.09 |
| launch acceleration | 1.246 | 1.541 |
| stop deceleration p85 | 2.794 | 1.722 |
| first discharge headway (lone cars at green) | 3.03 s | 3.93 s (now includes the driver's reaction) |
| permitted-left critical gap | — | 4.98 s, 8 accepted / 10 rejected lags |
| auditor, non-zero classes | jerk 13 | jerk 2 |

**manhattan-vru at 6,000 veh/h for 300 s** (the dense QA case).

| | before (first run of this track, 10:33) | after |
|---|---|---|
| saturation headway | 2.788 s, n = 6 | 2.217 s, n = 3 |
| start-up lost time | 0.93 s, n = 5 | 3.19 s, n = 3 (OUT; 3 queues) |
| free-flow mean / spread | 0.801 / sd 0.218 | 0.957 / 0.090 |
| stop deceleration p85 | 1.465 (OUT) | 1.783 |
| launch acceleration | 1.151 | 1.454 |
| walking speed mean / p15 | 1.310 / 1.027 | 1.312 / 1.031 |
| gap below minimum | 217 | **0** |
| jerk over 30 m/s³ | 37 | **3** |
| step-speed | 18 | 11 |
| overlap, red entry, conflict zone, queue jump, occupied-crosswalk entry | 0 | 0 |
| pedestrian overlap | 15 | 29 |
| heading jump / in building | 147 / 12 | 151 / 11 |
| standstill (180 s) | 0 | 1 |

On Midtown the queue figures rest on 3 queues: dense Manhattan at this demand hardly forms
a queue of five, which is the demand gap below, not a model result.

### What changed in the model

- **City drivers** (`IdmPreset::UrbanHcm`, the native engine's set): car T = 1.0 s
  (Treiber & Kesting's city value) and a = 1.7 m/s², calibrated here against the HCM
  saturation flow and Wang's launch acceleration (swept T 1.0-1.5, a 1.0-2.0). Trucks keep
  the Kesting 2010 column.
- **Start-up reaction**: each driver draws a signal response (Taoka 1989 lognormal, median
  1.15 s) and a queue start-up delay behind a moving car (median 0.5 s, calibrated, same
  quantile). **Heterogeneity**: free speed N(1, 0.1) × limit per driver.
- **Amber**: each driver has a time-to-line threshold across the Zegeer & Deen dilemma zone
  (10 % stop at 2.5 s, 90 % at 5.5 s); a driver who could stop goes on only below it and
  only if the line is reached 0.3 s before the red. The auditor's amber rule flags going
  only from beyond 5.5 s.
- **Right turn on red** is a jurisdiction rule (`rules::TrafficRules`, from the world's
  highway preset): prohibited in New York City and by default, permitted after a full stop
  where a preset says so. No shipped preset permits it.
- **Permitted left turns** take the HCM signalised critical headway (4.5 s, was the
  two-way-stop 4.1 s), judge an opposing car's arrival with it accelerating only up to the
  25 mph limit, and yield to all opposing non-left traffic whatever the junction matrix
  says; paths that cross are foes even where a matrix leaves them unmarked.
- **Green onset**: a driver gives way to a conflicting car still entering on the end of its
  amber. **Stops** end 0.5 m short of the line (were 4 m), a stop behind a standing car ends
  exactly s0 short of it, and braking short of an emergency builds at 20 m/s³.
- **Merges**: a car already in the converging stretch goes first; a merge partner binds
  after the relaxed stop at a crosswalk. **Lane changes** need the target lane clear by
  the follower's s0. A car off the centreline on a bend advances at its own speed.
- **Closures**: the router's cost generation counts closure changes (a reopen and a close
  in one step used to go unseen); a car cut off detours to its destination road or the
  nearest reachable lane, and waits at the barrier rather than vanishing as RouteBlocked;
  a driver without navigation sees a closure through the next two junctions.
- **Harness fixes**: the procedures that measured wrongly on the first run (a free-flow
  sample whose leader was on the next lane; lags offered to the second car in a queue;
  an experiment signal with no all-red) are fixed and documented on the module.

Each new rule has a test that was shown to fail with the rule broken (right on red in the
signal model and in the auditor, amber heterogeneity and the reach-the-line margin, the
auditor's amber rule). The pedestrian, traffic-invariant and closure tests went red on the
intermediate states and pass now: `cargo test -p v2xw-mobility` — 229 unit tests and 9
integration tests pass.

### Still open

1. **Demand is far below Midtown's.** At 6,000 veh/h the median signalised avenue approach
   carries 36 veh/h and a side street 12; NYC DOT's counts are about 1,400 and 450. Matching
   them needs roughly 30-40 times the demand, thousands of vehicles on the extract at once;
   whether the engine and the page carry that is the scale track's question. No shipped
   scenario was changed.
2. **Geometry the mobility model cannot fix** (for the road-network track): sidewalks
   drawn over the carriageway (sidewalk 602 converges to 2 m from driving lane 11649 at
   (490, 1943); footway 587/511 within 1.1 m of lane 9117 at (665, 1662)), the main source
   of the pedestrian overlaps; internal connectors tighter than a car can turn (14081 has a
   1.2 m radius, 14143 2.7 m, 14276 and 14267 similar), all the heading jumps; lane 1011
   passing inside building 2956's corner at (323, 1468), all the in-building steps; a lane
   drop through junction 3591 into a 1 m exit lane (4772), and 1 m lanes between
   junctions (3930); step-speed on connectors 14075, 14690 and 14687 at the approach-to-
   connector hand-over; the procedural grid's plans have no all-red and its conflict
   matrix leaves permitted lefts and opposing through movements unmarked.
3. **One vehicle waits 180 s** on Midtown (actor 241, lane 537 → 14143): the movement is
   always green, the crosswalk right after the line is never empty, and the model yields to
   anyone on the whole crosswalk. UVC §11-502(a) asks only for the driver's half and a
   pedestrian approaching close from the other; adopting it needs the auditor's
   occupied-crosswalk rule changed with it.
4. **No strategic lane changes**: routes are lane-level from the spawn lane and lane changes
   are discretionary, so a car is always in its turn lane but never weaves to reach one,
   and a trip that would need a change is dropped as unroutable.
5. Pedestrian compliance is 100 % by default (`jaywalk_probability` 0); Manhattan's is
   about 89 %. The auditor counts every don't-walk entry as a violation, so turning it on is
   a decision about what the auditor holds the model to.
6. Turning speeds (left 21 km/h, right 21 km/h mean on Midtown) are reported, not tested:
   no single published figure fits every corner radius.
7. **Permitted lefts are more cautious than the HCM on the Midtown grid** (Raff 5.96 s over
   15 lags; 4.98 s on phase1-grid). The likely cause is the start-up reaction: a turner
   waiting for a gap takes the signal-response reaction (median 1.15 s) once the gap opens,
   where a driver watching for it anticipates. Not changed without a source for the
   anticipatory response.

## 2026-09-24 — QA of the website on the release build: final state

The QA lead drove the page as the owner would — `v2xw-server` built with
`cargo build --release -p v2xw-server -p v2xw-cli` on ports 8787 (behind a TCP proxy that can
cut every connection) and the Studio's dev server on 5173 — and fixed what could be fixed
directly. Every number below comes from a command or a Playwright script run on this
machine; the scenarios were copies of `manhattan-5min.yaml` (`qa-manhattan`: 20 s at
6,000 veh/h) and of `revocation-latency.yaml` (`qa-secure`: 40 s, one roadside unit,
attackers from 10 s, 30 s pseudonyms, a 20 s CRL cadence).

### Fixed during QA, one commit each

| Commit | What was wrong, as found on the page |
|---|---|
| `69d0c14` | Loading a ready-made scenario kept unapplied edits (a refused one included) and rebased them onto it, so the next Run was refused for a field the form no longer showed as edited. |
| `0a2466a` | `nodes.default_obu: obu/cohda-mk5`, offered by the page, ran 20 s of Manhattan with 22 vehicles and sent no frame: the profile publishes no P-256 signing cost. The loader now refuses such a profile by name and lists the ones that sign. |
| `d672594` | The chase HUD said `profile: n/a` for every vehicle, beside a state tab naming its profile: vehicles that spawn after the Hello are not in its node table. It now reads `inspect.node`. |
| `b236001` | **Honest traffic rejected at every i-period boundary.** `CrlGate` derived `Default` with `skew: 0`, and every node's gate comes from `Stores::default()`. A certificate one period away was refused, the signature detector reported it, and the authority revoked honest vehicles. Found with the lifecycle compressed to 60 s periods: 3,645 of 33,361 verifications invalid and 2 honest devices revoked, no attacker. Default is now `CrlGate::new(0)`. |
| `b0cf71e` | The `rsu` camera on a run with no roadside unit logged "needs a vehicle to follow … click a vehicle first". |
| `97f0f11` | `v2xw run --duration-s` was applied after validation: a valid override was refused and a shortening one was not checked. |
| `ac3047d`, `ce6bbb6` | Three settings change nothing on a BSM run by design (`radio.tiers.phy: abstract` under a medium MAC, `security.envelope: etsi103097`, `messages.codec_tier: size-model`), and the page gave no reason. Their engine notes now say so, and a fully applied field shows its note once edited. |

Each Rust fix has a test shown red without it. After the fixes: `v2xw-node` 192 pass,
`security_lifecycle` 9/9, engine `scenario` 13/13 and lib 46/46, `v2xw-cli` 16/16,
`v2xw-conformance` 69/69 (the `grid-traffic` golden reproduces unchanged), Studio vitest
165/165, typecheck clean. The release binaries were rebuilt at `ac3047d` (`ce6bbb6` changes only
the page) and the page re-checked.

### What was checked, and what it showed

- **Every control.** A scripted pass over the header, settings panel, transport bar,
  viewport toolbar, inspector, message panel, measurements strip and the Runs, Compare
  and Commands tabs, on a live Manhattan run. All behaved as labelled. Details:
  - The scrub bar moved 8.8 s to 30.8 s on a click at 10 %.
  - 14 of 19 overlays toggle. The other 5 are disabled and labelled "not available in
    this build".
  - The metric picker lists 223 metrics.
  - The engine-backed suite (`e2e-engine`, 9 tests, including its own 18-row control
    table) passed 9/9, twice, on the final release binary.
- **Every settings field.** 111 edits covering 106 of the page's 114 fields. For each, a
  base preset was loaded, the field was edited in the page and Run was pressed. The rest
  were covered separately: the focus-region fields through the page, `events` by
  `events.spec.ts`.
  - 54 moved the run's digest in the expected direction. Examples: OBU 10 dBm, mean RSSI
    −107.5 → −114.5 dBm; buildings off, 1,902 → 27,504 receptions; `time_dilation`, 424
    suppressed frames.
  - 25 were refused, each with the engine's reason. Five causes come back phrased
    "internal error": a missing map file, a missing DEM file, an RSU `site` on an
    imported city, a wrong `net.backend_net` id, and an unknown `detection.ma.params` key.
  - 31 left the digest unchanged, each for a stated reason: descriptive fields; the
    default value; nothing in a 40 s run reached the backend; or the three notes above.
  - `keep_holes` and `metres_per_level` changed nothing in a 20 s run and were not
    investigated further.
  - The TR 36.885 drop put 1,250 vehicles on Manhattan and did not finish 20 s within
    240 s.
- **Soak, 20 runs through the page.**
  - The runs included seed, radio and rate edits, two scenario switches, and Run pressed
    twice while a run was playing at 1×.
  - 20/20 finished and streamed to their last instant. The connection stayed streaming,
    with no page or Studio errors.
  - Engine RSS was 67 / 282 / 273 MB at runs 1 / 10 / 20. RSS is compressed on this
    machine; the physical footprint was 506 / 447 / 541 MB.
  - Tab heap was 499 / 454 / 453 MB.
- **Network drop.** All 6 connections were cut at 12.1 s into a 1× run. The page was
  streaming again 1.6 s later, with `HELLO_RESUMED` at seq 135. It applied 441 frames,
  seq 0–440, with no gap, no duplicate and no error.
- **Rendering suites against the mock.**
  - Camera fuzz: 36 steps, 0 failures. Scene validation: 15/15. Studio and regressions:
    8/8.
  - Capture tour: 0 vehicles drawn inside a building (1,600 samples), 0 wrong signal
    heads (14,400 head-checks), chase jitter RMS 7.5 px. All seven pictures were read.
- **Traffic auditor, release, Manhattan with VRUs at 6,000 veh/h for 300 s.**
  - Two runs gave identical counts. The run had 340 vehicles, 538,926 vehicle-steps and
    599,834 pedestrian-steps, 46,517 of them on crosswalks.
  - Zero counts: overlap, teleport, red and amber entry, conflict zone, illegal
    transition, lane change near a junction, queue jump, speed jump, heading flip, accel
    bound, standstill, mid-road despawn, occupied-crosswalk entry, pedestrian
    don't-walk entry, and all three world checks.
  - Non-zero: heading jump 147, gap below s0 217 (one follower creeping at 0.2 m/s to
    1.85 m behind a stopped leader, s0 2.0 m), jerk 37, step-speed 18, in-building 12
    (the newsstand), and **pedestrian overlap 15**.
  - Steps on passages through buildings: 5,834.
  - Without VRUs at the same rate, gap below s0 is 23 and heading jump 143. The
    junction track reported 0 and 177 before wave B.
- **Metrics on a dense run** (227 vehicles, 90 s).
  - Delivery: `pdr` 0.334, `pdr[100m]` 0.916; `per` = 1 − `pdr`. By distance: 0.999 at
    0–50 m, 0.864 at 50–100 m, 0.535 at 100–150 m, falling to about 0.04–0.08 beyond
    400 m. Line-of-sight avenues keep 0.072 beyond 1 km.
  - End-to-end latency: p50 14.69, p95 19.19, p99 19.60 ms. The ten stages sum to
    exactly the 14.708 ms mean, and their shares sum to 1.0000.
  - Loss causes plus delivery sum to 1.0003.
  - Bytes: the buckets sum to `bytes_total` (212,822 B/s), and the per-vehicle-hour
    buckets sum to their total. Offered load 1.703 Mbit/s against carried 1.7026.
  - Channel: CBR 0.015.
  - Overheads: security 0.564, link 0.200, full-certificate share 0.102 (one in ten, as
    J2945/1 says).
  - Awareness: time-weighted AoI 289 ms against a per-delivery peak AoI of 140 ms, which
    is consistent because the two are weighted differently. NAR 0.98 at 100 m and 0.45
    at 300 m.
- **Radio**, the same 60 s Manhattan run:

  | | mean RSSI | PDR 100 m | PDR 300 m | e2e p50 / p95 | air time |
  |---|---|---|---|---|---|
  | DSRC | −107.9 dBm | 0.922 | 0.313 | 14.7 / 19.1 ms | 0.30 ms |
  | LTE-V2X | −99.0 dBm | 0.967 | 0.532 | 63.8 / 102.8 ms | 1 ms |
  | NR-V2X | −98.9 dBm | 0.980 | 0.518 | 25.0 / 50.4 ms | 0.5 ms |

  - Transmit power 10 dBm: DSRC −114.9 dBm, LTE −111.9 dBm. LTE drops exactly the
    13 dB it lost; DSRC was at a mean of 17 dBm under J2945/1 power control.
  - Medium propagation: DSRC −109.4 dBm. Buildings off: −79.2 dBm.
- **Message sets.**
  - In 30 s: SPaT 300 (10 Hz), MAP 30 (1 Hz), PSM 1,501 from 50 VRU devices. On
    GN/BTP: CAM 2,813 and VAM 5,263.
  - DENM: 100 from hard braking in a dense 60 s run.
  - Fragmentation: 1,600 B padding with `generic-sdu` doubled the frames and gave SDU
    loss 0.81 against fragment loss 0.80.
  - A hybrid Falcon signature with no fragmenter: the certificate-carrying frames above
    the MTU were refused (2,241 → 2,008 frames).
  - Engine tests: `message_sets` 5/5 (including SPaT against the lamps), `fragmentation`
    7/7, `timeline` 9/9, `attack_wave` 1/1, `phase2` 8/8.
- **Scenario events** on Manhattan, 90 s at 15,000 veh/h.
  - Closing FDR Drive at 15 s wrote the record "43 lanes closed". Entries onto its
    busiest edge fell from 5 to 0.
  - `param.change` of the arrival rate to 0 at 45 s: vehicles first seen after 45.2 s
    fell from 113 to 1.
- **Security.**
  - Pseudonyms: 600 changes at exactly 30.000 s intervals. Certificate, temporary ID and
    link-layer address changed together 600/600, and the chain was consistent 416/416.
  - Top-up with a compressed lifecycle: 93 started and 91 completed, 455 certificates,
    312 kB up and 123 kB down over cellular Uu.
  - Revocation, seven ConstPos attackers over 180 s:
    - 6 of 7 were revoked, in 11.19 s from detection to enforcement. Stage times: report
      9 ms, shuffle 1.74 s, decision 1.76 s, issue 1.84 s, publish 11.10 s.
    - 567 CRL downloads and 24 RSU broadcasts; 24,952 receptions from revoked senders
      were rejected.
    - **1 honest vehicle was also revoked.**
- **Chase inspector.** A clicked or chase-adopted vehicle lists its sent BSMs, each
  decoded field by field with the 1609.2 header and the octets by layer. It also lists
  received messages with fate, RSSI/SINR, distance and delay by stage, and queues whose
  depths change between reads.

### Still open, most important first

1. **Honest vehicles are revoked with no attacker in dense traffic.**
   - The shipped `revocation-latency.yaml` with its attackers removed revoked 21 honest
     vehicles in 300 s. Its detectors fired 1,040 positionSpeedInconsistency, 611
     headingInconsistency and 204 positionJump verdicts, which became 1,582 reports.
   - `qa-secure` without attackers revoked 1 in 180 s on the fixed binary.
   - Mechanism: the GNSS model's outliers (1 %/s, 12 m) and 3 s bursts deliberately
     under-report their accuracy. The receiver's detector uses a constant 5 m confidence
     rather than the BSM's. At 6,000 veh/h, one burst is seen by enough neighbours to
     pass the authority's gate: 3 reporters over 4 s within 15 s.
   - `phase2.rs::with_no_attacker_nothing_is_revoked` passes only at 600 veh/h. This
     needs a calibrated decision, not a QA patch.
2. **Vehicles hit pedestrians.** There were 15 vehicle–pedestrian overlaps, down to 0.69 m
   between centres.
   - The pedestrians were on sidewalk lanes that lie on the roadway: 811 of 7,741 sidewalk
     lanes overlap a drive lane by more than 0.3 m, about 10.7 km in all.
   - Example: sidewalk 2714 runs 0.73 m from 6th Avenue's lane 1339.
   - This is importer geometry.
3. **The "sign" latency stage includes the J2945/1 hand-off jitter.**
   - `t_signed` is stamped after the jitter (`Engine::hand_down`), so `sign` reads 13.9 ms
     for a 9 ms HSM signature.
   - With `compute_tier: abstract` (1 µs signing) it still reads 4.94 ms.
   - The total is right; the split is not. The fix needs a stage and a record field of
     its own, which moves the golden.
4. **Traffic:**
   - Heading jumps: 147.
   - Gap below s0: 217. One follower creeps to 1.85 m. This was 0 in the junction
     track's report and is 23 without VRUs.
   - Jerk: 37. Step-speed: 18.
5. **Sidelink runs have no `cbr` metric.** It is empty for LTE-V2X and NR-V2X, though the
   chase view shows CBR on a sidelink frame.
6. **Page:**
   - The stats chip says "0 RSUs" on a run with a roadside unit placed by `position_m`.
   - The inspector says "radios 82" beside "106 vehicles or roadside units".
   - The HUD shows "(indices pending — node.tx)" even under the SCMS lifecycle.
   - A page reloaded after a run finished shows no measurements.
   - The tab heap is about 450 MB on Manhattan.
   - One page load in about 10 fell back to the built-in settings list under heavy CPU.
     It was not reproduced in 3 further loads.
7. **Wording:**
   - Five run-time refusals are prefixed "internal error" although the cause is the
     user's input: a missing map or DEM file, an RSU `site` on an imported city, a wrong
     `net.backend_net` id, an unknown `detection.ma.params` key.
   - `TimelineKind::Closure`'s doc says `lane` or `edge`; the loader wants
     `target: "edge:N" | "street:NAME"`.

### Completeness against the owner's request

| Asked for | State | Evidence |
|---|---|---|
| Parameters applied actually take effect | delivered | 106 fields through the page; 54 change the run, 25 refused with a reason, 31 unchanged with a stated reason; two carry-over and note fixes |
| Every button, config and feature works | delivered, with the open items above | control pass; `e2e-engine` 9/9 twice |
| Runs keep working after a while | delivered | 20-run soak, Run pressed mid-run twice, memory flat from run 10 |
| Traffic smooth and correct | partial | 0 overlaps, teleports and red entries; 147 heading jumps, 37 jerk events and 217 gap steps remain |
| Cars through buildings | partial | 12 steps at one newsstand; 0 drawn in buildings by the viewer |
| Camera blacks out or goes underground | delivered | fuzz 36/0; scene validation 15/15 |
| Cars touch each other | partial | vehicle–vehicle overlap 0; 15 vehicle–pedestrian overlaps from sidewalk geometry |
| A car goes round one stopped at the junction | delivered | queue-jump 0; conflict-zone 0 |
| Clean rendering, lights green only sometimes | delivered | 0 wrong heads of 14,400; `signal_heads.rs` |
| Network load, e2e delay, overhead metrics | delivered, one split wrong | stages tile the mean exactly; the sign stage includes the hand-off jitter; no sidelink CBR |
| Certificate lifecycle, pseudonym rotation, CRL distribution | delivered | 30.000 s rotation with every identifier; top-up 91/93; CRL by download and RSU broadcast |
| Backend over cellular, not C-V2X | delivered | reports and top-ups over Uu (2.67 MB up in the attack run); RSU relay path; backhaul |
| Appropriate to protocol and deployment | partial | honest revocations in dense traffic; ETSI butterfly and ECTL not driven |
| Chase view shows the messages, content and queue | delivered | sent and received decoded with octets; queues live |
| The engine improved end to end | partial | the open list above |

## 2026-09-24 — junction geometry, signal timing, passages (junction track)

Every number below comes from `traffic_audit` (`crates/v2xw-engine/examples/traffic_audit.rs`)
or a test run on this machine. Scenarios are run for 300 s. "Dense" means 6,000 veh/h.

| Class | dense Manhattan | manhattan-5min | dense grid |
|---|---|---|---|
| world: conflicting protected greens | 19 → 0 | 19 → 0 | 0 → 0 |
| world: lanes in a building, no passage | 64 → 0 | 64 → 0 | 0 → 0 |
| heading flip | 1 → 0 | 0 → 0 | 0 → 0 |
| heading jump, old 4 m bound | 79 → 39 | 10 → 21 | 0 → 0 |
| heading jump, AASHTO P bound (5.42 m) | — → 177 | — → 46 | — → 0 |
| in a building outside a passage | 12 → 12 | 6 → 6 | 0 → 0 |
| jerk > 30 m/s³ | 52 → 38 | 5 → 4 | 13 → 13 |
| step vs reported speed (new) | — → 7 | — → 2 | — → 0 |
| all other safety classes | 0 → 0 | 0 → 0 | 0 → 0 |

Two dense-Manhattan runs gave identical counts.

What changed:

- **Turns are drivable arcs.**
  - Junction connectors used to be quadratic Béziers. They are now AASHTO simple curves:
    the largest circular arc that fits between the two lane ends.
  - Motor lane corners are rounded the same way (`v2xw_world::curve`).
  - Lane ends are pulled back, up to 7 m, until each turn has room for the P design
    vehicle's 6.4 m centreline radius.
  - The auditor holds each class to its AASHTO minimum path radius. For a car that is
    5.42 m.
- **Forks share their lanes out in order.** Signal plans use split phasing where
  approaches conflict.
- **Change intervals follow ITE 2020, per phase group.**
  - Amber: `y = t + v/(2a + 2Gg)`, clamped to 3-6 s (MUTCD §4D.26).
  - All-red: `r = (W + L)/v`, capped at 6 s.
  - Room is left in the plan for a pedestrian walk phase, which this track does not add.
- **Passages are in the world model** (`World.passages`).
  - Each lane that runs through a building is classified by its OSM tags.
  - Of the 62 such lanes: 39 are tagged (the Helmsley Building portals, the Park Avenue
    Viaduct at Grand Central, basement ramps) and 23 are untagged driveways. The untagged
    ones are counted as the `untagged-building-passage` anomaly.
  - The auditor now accepts a vehicle inside a building only on a passage through that
    building, and it compares heights as well as footprints.
  - The viewer draws an opening in the wall where each passage lane enters a building.
- **Signals.**
  - Every producer now streams one row per head group: the live projector, the fixture
    engine and the recorder. Before, the recorder's signal block was empty, and the fixture
    engine sent only the controller's first movement.
  - Rows are evaluated with the plan's own arithmetic. Previously, at a phase boundary on
    Manhattan, a head showed green while its movements were amber.
  - All producers use one J2735 table.
  - `tests/signal_heads.rs` checks 733,764 Manhattan head-group samples over a full cycle,
    after a seek and after a rewind. All match the engine.
- **Studio.**
  - The aerial view now opens with all of the traffic in frame. The three aerial
    scene-validation tests that predated wave A now pass.
  - At 800 × 520 the viewport is 470 × 278 (it was 360 × 146).

Still open:

- **Heading jumps.** Every remaining jump is on a connector next to a 1 m lane. That lane is
  left where a segment is too short to hold both junctions' areas; it happens on divided
  avenues.
  - Joining those junctions (netconvert `--junctions.join`) cut the P-bound count from 177
    to 52 in a trial. It also produced 8 overlaps between side-by-side connectors, so it
    was not kept.
- **Jerk.** Classified onsets of hard braking in moving vehicles on dense Manhattan:
  - 23 of 30 are junction merge ordering against a vehicle on another path;
  - 4 are on a free road;
  - 3 are same-path leaders;
  - none are from a signal.

  Two merge-ordering changes each lowered the count on one scenario and raised it on
  another, so neither was kept.
- **In a building outside a passage.** All 12 are one newsstand (way 1117866998), mapped
  0.8 m from a lane centreline.
- **Shadow acne.** No pixel test was added. Bias 0, a positive bias and a 128² shadow map
  all left the headless SwiftShader frame free of measurable acne, so a test could not be
  shown to fail.

## 2026-09-23 — five tracks merged: stability, traffic, radio, metrics, rendering

Five engineers worked in isolated worktrees and the integrator merged them into `main` in
that order (merge commits `2cf3e75`, `0f1340d`, `b0ad18a`, `af00eec`, `7636985`), testing
the touched crates after each merge and fixing the seams between tracks in commits of their
own. Every number below comes from a command run on this machine; the release binaries were
built from `main` and driven in a browser.

### What each track delivered

- **Stability.** Apply reaches the next run. Every `run.start` sends a fresh `Hello`, and
  there is one kernel per run: `run.stop` joins it. A world imported once is reused exactly
  (`world.cache`, and in memory). The engine-backed Playwright suite (`e2e-engine/`, 5 tests)
  drives every control against the real server.
- **Traffic.** An invariant auditor (`v2xw_mobility::audit`) checks every vehicle at every
  step, with a fault-injection test for each class. Fixed: commitment at junctions, merges,
  deadlock at a red, insertion, smooth headings, tunnels below ground. Signal state is now
  streamed per signal group rather than per controller. Weather, fleet classes, demand
  models and VRUs now affect driving.
- **Radio.** `radio.rat` runs LTE-V2X Mode 4 and NR-V2X Mode 2 sidelinks as well as 802.11p.
  Buildings obstruct links (Sommer), and so does terrain from a DEM (knife-edge). Each
  vehicle generates at its own phase with J2945/1-style jitter. Also wired: jammers,
  `radio.models`, the focus region and `nodes.compute_tier`.
- **Metrics.** End-to-end delay is split into stages that tile each message's journey, and
  every reception attempt has exactly one recorded fate. New metrics: awareness (AoI, NAR),
  load and overhead (security, network, link, certificate share, bytes per vehicle-hour).
  The PSDU is composed layer by layer; the page offers every series through a picker.
- **Rendering.** Vehicles move along smooth curves between mobility samples. Signal lamps
  show the state for the drawn instant, and each head shows its own group's state. The
  camera stays out of walls and above the road, and the lens is shifted so the HUD does
  not cover the followed car. Plan-view markings no longer shimmer.

### Seams fixed at integration, each in its own commit

- `world.cache` key: it now includes the importer revision and the DEM bytes (`159a371`,
  `b0ad18a`).
- `messages.generator`: the radio track's timing parameters and the metrics track's rule
  parameters now share one validator (`af00eec`).
- Sidelink frames carry no 802.11 framing (`af00eec`).
- Signal group keys were ported into the rendering track's `SignalRenderer` (`7636985`).
- The stream now sends each vehicle's body centre. It used to send the rear-bumper
  reference, which drew every car 2.5 m behind itself (`c2bd4e6`).
- Tests that the merged behaviour had made vacuous were changed to measure what they
  meant, with no assertion weakened (`98fc437`, `f915539`, `894f329`, `45cd5d6`).
- `v2xw-threat` in-the-loop broke under the metrics track's continuous-time verification:
  3,604 reports and 0 of 5 attackers caught. Bisected to the metrics branch; the test host
  now joins claims across steps and gets 72,916 reports with all 5 caught (`246d6b0`).
- Conformance kit: the method count is now 33 and a hash-order false positive is gone
  (`23a81ab`). The first golden record is blessed, `grid-traffic` (`1aaa943`, re-blessed
  in `b66b3cd` and `5cac9a0`). Before each blessing the digest was shown identical twice
  at `RAYON_NUM_THREADS` = 1, 4 and 8.

### Built for the owner's requests during integration

- **Message content in the chase view.** `node.tx` names the pseudonym that signed each
  frame and carries the BSM's decoded Part I (`17f85bc`). The inspector lists the followed
  vehicle's broadcasts, marking any pseudonym change; each row expands to every field, the
  octets by layer and the signing delay. It also lists what the vehicle heard, with fate,
  RSSI, SINR, distance and end-to-end delay (`7954007`, `fdd18ca`).
- **Pseudonym rotation.** It now follows `security.pseudonym_change` (time, distance or
  silent) over a pool of 20 pseudonyms used round-robin. Before this, a period other than
  300 s was ignored, each vehicle held a single pseudonym, and the store alternated
  between two (`544a23e`).
- **Found by driving the release build in a browser:**
  - Clicking a car followed no radio when the page did not know the car's node (`fdd7cae`).
  - The kernel simulated the whole run ahead of the stream, which emptied the message log
    and inflated `run.status` (`d4fe381`).
  - The node's telemetry window, with its queues and CPU load, was never published
    (`1950d08`).
  - The fallback settings list offered radio technologies the engine refuses (`cafa7f5`).

### Evidence

- **Rust, one crate at a time, debug:**
  - core 215, proto 101, world 211 (4 ignored), mobility 214, msg 203 (1 ignored), sec 99,
    radio 263 (6 ignored), net 113, node 186, threat 210, record 239, copilot 68, py 16,
    wasm 6: all pass, at `1aaa943`.
  - metrics 235, server 77, cli 15, experiment 80, conformance 69: all pass, at `5cac9a0`.
  - engine at `5cac9a0`: 115 pass, 2 fail (below).
- **UI:**
  - Typecheck is clean for protocol, viewer, mock-server and studio.
  - vitest: protocol 186, viewer 132, mock-server 45, studio 143.
  - Studio e2e against the mock: 20 of 24 pass (below).
  - Studio e2e against the real server: 5 of 5.
- **Release build:** `cargo build --release -p v2xw-server -p v2xw-cli` succeeds.
- **Live check** (the owner's `run.txt` commands on ports 8787 and 5173, Playwright,
  screenshots read):
  - The aerial view shows 26 vehicles whose positions move between frames. A click on a
    car switches to chase view on `node 21`, which is stopped behind a red stop bar.
  - The inspector lists 50 broadcasts (BSM #116, id `ac285d32`, 40.75261°, -73.97935°,
    176 B) and 50 receptions, all delivered and verified. The queues show 0/0, 1/1, 0/0,
    1/1, 0/0 (p50/p95), and the HUD shows the pseudonym `ac28…5b`.
  - Pause stops the clock at 00:02:00.500; one step moves it to 00:02:00.599.
  - After editing the duration to 20 s and the seed to 0x2a, Apply says "Applied 2
    changes". The run finishes at 20 s, and Run again gives the same digest,
    `d0d6139e…`. No page errors.

### Still open

- **Engine `phase2.rs`, 2 of 8 fail** (they failed on `main` before this wave):
  `with_no_attacker_nothing_is_revoked` and
  `the_detector_suite_has_false_positives_on_honest_traffic`. The legacy-12 suite fires on
  3.51 % of honest messages (positionSpeedInconsistency 1.84 %, headingInconsistency
  1.74 %). Of the 3 candidate pairs, the two linkage authorities refuse 2, and 1 innocent
  device is revoked.
- **Studio mock e2e, 4 failures** (they fail on `main` too, per the rendering track):
  - The aerial view opens at a fixed 1,400 m extent: 54 of 200 vehicles are outside the
    frustum, and 0.735 of the map is populated against a 0.9 floor.
  - A vehicle mark covers 16 px against a 40 px floor.
  - The inspector's radio count reads 178 against 191 nodes in the table.
  - `the nothing in the interface covers the vehicle` test is flaky under load: 2 of 3
    passes, drift 0.066 against 0.03. Q3 scrub failed 3 times and passed 3 times across
    runs; the e2e file itself notes that a Vite hot reload produces exactly that failure.
- **Not built:**
  - The backend path over cellular (Uu). `net.uu`, `net.backhaul` and `net.backend_net`
    are read by nothing. Vehicle-to-SCMS traffic (enrolment, pseudonym top-up, misbehaviour
    reports, CRL download) uses a constant backhaul latency with no capacity limit, and
    CRL distribution is an RSU broadcast only.
  - A pseudonym certificate's expiry and re-provisioning during a run: the bootstrap pool
    is valid for the whole run.
- **From the tracks:**
  - Traffic, Manhattan at 6,000 veh/h:
    - 79 heading jumps, from OSM connectors tighter than a 4 m radius.
    - 12 in-building steps at covered ramps.
    - 64 lanes through building footprints, which are real passages in the source.
    - 19 conflicting protected greens in synthesised plans.
    - No all-red interval.
    - No VRU device (PSM/VAM).
  - Radio:
    - No sidelink congestion control is enforced, and there are no blind retransmissions.
    - The 1 km candidate range truncates reception.
    - The NR BLER is a fit, not a measured curve.
    - `phase2-manhattan.yaml` runs with buildings off, because one mast hears 2 of 30
      reports with them on.
  - Metrics:
    - Only `fragmenter/none` exists.
    - The size-model codec tier is refused.
    - Per-node breakdowns are only in the recording.
  - Stability:
    - `events` beyond outage and weather are inert.
    - A reconnect gets a resync, not a true resume.
- **Behaviour the owner will see:**
  - With buildings obstructing, the Midtown 5-minute run delivers 9,970 of 146,618
    reception attempts (PDR 0.07). The attempts include every pair within 1 km; within
    100 m, delivery is 0.85 to 1.0 (radio track, `radio_access.rs`).
  - The e2e-latency plot kept a previous run's history after Run again with a short run
    (`e2e_latency.p95` axis 50–125 s on a 20 s run). Not investigated.
  - Forward seeking now reaches only as far as the kernel's bounded lead, 12.8 s by default.

### Wave B — eight more tracks, merged 2026-09-24

Eight engineers worked in isolated worktrees; the integrator merged them into `main` in
this order: session `e6548f9`, geometry `26a0dcb`, vru `8d3ccfc`, radioprop `96703ca`,
radioaccess `724e92c`, messages `2d306c7`, security `58aa4df`, inspector `e3983cf`. The
crates each merge touched were built and tested before the next one, and every seam found
was fixed in a commit of its own. Every number below comes from a command run on this
machine. The integrator deleted the shared debug target first, because worktrees sharing one
`CARGO_TARGET_DIR` had silently linked each other's crates; every result here was built from
the merged tree alone.

#### What each track delivered

- **Session.**
  - A session outlives its socket: a reconnect resumes the stream with nothing missed
    (`HELLO_RESUMED`) instead of resyncing.
  - A seek past the kernel's lead runs the kernel there, reporting progress, and lands.
    Before, the scrub bar could reach only 12.8 s ahead.
  - A rewind keeps the run's speed, and every per-run view resets on a new run.
  - Every scenario-timeline kind now acts and writes a `scenario.event` record: closures
    with re-routing, demand multipliers, `param.change` for the live parameters, and attack
    waves. The page edits the events and draws them on the time bar.
  - Tests: server resume 7, rewind 3, seek_ahead 1, pacing 1, timeline 1; engine
    `timeline.rs` 9 and `attack_wave.rs` 1.
- **Geometry.** Junction turns are AASHTO arcs, forks share their lanes out in order,
  change intervals follow ITE, roads through buildings are passages, and every producer
  streams one signal row per head group. Its own section above (2026-09-24, junction track)
  has the before/after audit counts.
- **VRU.**
  - Crosswalk lanes and MUTCD pedestrian signal intervals, from OSM `footway=crossing` and
    on the grid.
  - Vehicles yield at crosswalks, and pedestrians obey walk signals.
  - Pedestrians and cyclists can carry VRU devices hosted in the node phase. They send real
    SAE J2735 PSMs and ETSI VAMs, the VAM generated from TS 103 300-3 V2.2.1.
  - Cyclists are drawn as riders, and VRUs have their own aerial mark.
  - Three new audit checks (`Check::ALL` is 26 after the merge);
    `pedestrian_invariants` 4/4.
- **Radioprop.**
  - The Mangel 2011 street-corner model, a geometric city-street law (the new default
    `high` tier) and ITU-R P.838-3 rain at the carrier.
  - `radio.devices` and `radio.range` reach every link. The candidate range comes from the
    link budget, not a fixed 1 km, and arrivals below the noise floor count as energy only.
  - Engine `propagation.rs` 9/9.
- **Radioaccess.**
  - SAE J3161/1 is the default LTE-V2X profile.
  - Sidelink congestion control (ETSI TS 103 574 / J3161 CR limits), blind HARQ
    retransmissions with chase combining, and SCI decoding drawn against the control BLER.
  - An honest 802.11p AIFS.
  - Jammers can ride a vehicle or drive a path.
  - `node.tx` carries a radio view: the MCS by name and, on a sidelink, the slot,
    sub-channels, HARQ attempt and CBR/CR.
  - Engine `radio_access_layers.rs` 10 pass, 1 ignored measurement.
- **Messages.**
  - Roadside units broadcast SPaT and MAP, and vehicles send DENM, SRM and SSM.
  - Every fragmentation strategy runs, with reassembly, timeouts and the loss it amplifies.
  - The headline `pdr` is the 3GPP TR 36.885 packet reception ratio within a stated range.
  - A message reaches the applications when its verification finishes, not at the next
    periodic step.
  - Breakdowns reach the page: delivery by distance, latency by stage, and per-node
    rankings.
  - Engine `message_sets.rs` 5/5 and `fragmentation.rs` 7/7.
- **Security.**
  - The SCMS runs in lockstep with the engine: a pre-run pool, pseudonym top-up over a
    backend link, proxy hand-off, single-certificate revocation and a CRL on its cadence.
  - Vehicles reach the backend over cellular Uu (`net.uu`), or relay through a roadside unit
    with a backhaul.
  - The ETSI ITS PKI runs as a second credential protocol, with passive revocation.
  - Rotation is overlap-aware, and there are no phantom pseudonym changes.
  - GNSS burst and outlier probabilities are per-second hazards.
  - Roadside units detect and report.
  - The two `phase2.rs` tests that failed on `main` now pass: phase2 8/8 (1 ignored
    diagnostic) and `security_lifecycle.rs` 8/8.
- **Inspector.**
  - `node.feed` streams the followed vehicle's sent and received messages, decoded from
    their own octets: the 1609.2 spans, signer, HashedId8 and J2735 fields. It also streams
    the vehicle's queues (receive, verification, application, transmit, CRL) with waits and
    drops.
  - The Studio has a message panel with Sent, Received and Queues tabs. The `frame_tap`
    test shows the tap changes no record digest.
  - Server `feed.rs` 6/6, and a shared vector (`node-feed-v1.json`) checked from Rust and
    TypeScript.

#### Seams fixed at integration, each in its own commit

- **`run.seek` over HTTP** (`49c93b6`). The kernel runs ahead on its own clock, so an HTTP
  seek got -32003 or -32009 depending on a race. The transport refusal now comes first. A
  new assertion is red on the old order.
- **Pedestrian heads streamed Off** (`6abb260`). Geometry's `World::group_signals` found a
  head group only through an approach lane, but vru's walk heads face their crosswalk.
  `signal_heads.rs` is red without the fix (plan sg1 group 100: Off against Red). With it,
  1,930,684 Manhattan samples agree with the engine.
- **SPaT reference** (`d97bf80`). The test's reference got the same pedestrian rule (junction
  7 group 100 read Dark against StopAndRemain).
- **Focus-region sidelink test** (`06364dc`). Radioprop's high tier is line of sight on a
  building-free extract, so the region compared two path-loss laws: 5,803 receptions
  against 5,426. Both runs now use one law. It is 5,422 against 5,426, and red (5,426
  against 5,426) with the high sidelink PHY disabled.
- **The messages merge.**
  - The node phase is a step or a wake over `HostedNode`; a VRU device has no deferred
    checks and is never woken for one.
  - MAP/SPaT payloads and the hard-braking acceleration go to OBUs only.
  - The fragmenting hand-down carries radioprop's and radioaccess's new frame fields.
- **The security merge.** The `security.signature` resize is applied before padding,
  fragmentation and the MTU check, so all three see the resized SPDU.
- **The inspector merge.**
  - The feed push in the session track's connection loop returns `Exit::Park`.
  - The projector keeps the metric breakdowns and the security panel's store beside the
    feed.
  - A resumed Hello keeps the session handling and still re-asks `view.follow`, which is
    idempotent.
- **Radio view in the chase panel** (`6e55ed0`). Radioaccess's radio view reached no client
  after the feed replaced the old message log. It is now `radio.access` on each sent frame,
  shown as an "access" line (live: `dsrc-80211p 6mbps-qpsk-1/2`). The feed vector was
  re-blessed.
- **Breakdown cards** (`8dbda63`). The cards polled metrics the run does not measure, which
  left 27 "unknown metric" page errors in `resume.spec.ts`. They now ask only for what the
  run's catalogue lists, and quietly.
- **E2E tests the merges made wrong, with no assertion weakened.**
  - `lifecycle.spec.ts` (`a3efa9d`): `nodes.backend_tier` is now partly applied, and the
    test asserts that the engine publishes no unclassified or not-implemented leaf.
  - `studio.spec.ts` (`e16f15d`): the legend's five state glyphs are counted apart from
    vru's road-user key.
- **Goldens.** `grid-traffic` was re-blessed after radioprop (`27dc00c`), radioaccess
  (`83eca76`), messages (`1d6b7d0`) and security (`50338d1`). Each merged change equals
  what that track's own branch moved. Before each blessing, the digest was identical
  twice at `RAYON_NUM_THREADS` = 1, 4 and 8. The golden is now 15,914 records, digest
  `2d37766a5fb69289…`.
- `cargo fmt --all` (`d1a4d17`).

#### Evidence

- **Rust**, at `d1a4d17`, all 20 crates one at a time, debug: 2,904 pass, 0 fail, 14
  ignored.
  - core 215, world 223 (4 ignored), mobility 228, msg 210 (1 ignored), net 113, sec 99,
    radio 287 (6 ignored), node 191, metrics 251, record 239, proto 104, threat 210.
  - engine 174 (2 ignored), experiment 80, server 106 (1 ignored), cli 15, copilot 68,
    py 16, wasm 6, conformance 69.
- **UI:**
  - Typecheck is clean for protocol, viewer, mock-server and studio.
  - vitest: protocol 191, viewer 139, mock-server 45, studio 165.
- **Studio e2e against the release server:** 9 of 9 (controls, events, 4 × lifecycle,
  messages, resume, seek-and-plots).
- **Studio e2e against the mock:** 23 of 25 on the first run. The legend test was then
  fixed and passed 2 of 2. The aerial frustum test is still flaky (below).
- **Release build:** `cargo build --release -p v2xw-server -p v2xw-cli` succeeds (8 m 28 s).
- **Live check.** The owner's `run.txt` commands ran on ports 8787 and 5173, driven by
  Playwright, and the screenshots were read.
  - Setting 40 pedestrians and 10 cyclists and pressing Apply said "Applied 2 changes".
    Run started the run.
  - About 18 s later: 84 live actors (34 passenger, 40 pedestrian, 10 bicycle), 55 of which
    moved in 3 s; the run reached 2:06 of 5:00.
  - A real mouse click on a car switched to chase view on node 6.
  - The message panel listed 60 sent BSMs with decoded position, speed and heading. The
    first row opened to: pseudonym `3a66801336da2bd9`, 40 + 93 + 5 + 38 = 176 B, 17.0 dBm
    on channel 172, the access line, and 10.9 ms signing.
  - It also showed 60 received rows and a transmit queue with 2 messages passed through.
  - No page errors and no page exceptions.

#### Closed from the 2026-09-23 list

These items were open above and wave B closed them, each by the track named:

- The two `phase2.rs` failures (security).
- The backend over cellular Uu, and relaying through a roadside unit (security).
- 19 conflicting protected greens, 64 unmarked lanes in buildings, and the missing
  all-red interval (geometry).
- No VRU device (vru).
- No sidelink congestion control or blind retransmissions (radioaccess).
- The fixed 1 km candidate range (radioprop).
- Only `fragmenter/none`, the refused size-model tier, and per-node breakdowns only in the
  recording (messages).
- Inert timeline events, a reconnect that resynced instead of resuming, and forward seeking
  limited to the kernel's lead (session).
- The mock e2e radio-count failure (inspector).

#### Still open

- **Flaky tests.**
  - The mock aerial test "draws one mark per live vehicle" sees 1 of 200 vehicles outside
    the opening frustum in 2 of 3 runs at the merged `HEAD`. Centring the
    world when the traffic needs all of it made this worse (3 of 4 runs failed), so that
    change was reverted.
  - The first engine e2e run once opened on the built-in settings list (a cold start); it
    passed on the two runs after that.
- **The chase view with fragmentation on.** The frame tap hands the whole SPDU for every
  fragment, so the feed shows each fragment as a full message. Fragmentation is off by
  default.
- **SPaT.** It is built from `SignalPlan::group_timelines`, which merges phases. The
  geometry track moved the live signal block to per-phase evaluation because merged phases
  round differently at a boundary. The SPaT test samples at 0.35 s offsets and has not been
  checked at a boundary.
- **Chase HUD.** It shows `n/a` for CPU, RAM, stored certificates, CRL entries and report
  outbox on `manhattan-5min`, which has no security path. The HUD header still says "OBU"
  for a node spawned after the Hello.
- **Settings.** No setting is left that the engine reads nothing from.
  `nodes.backend_tier` became Partial.
- **From the tracks' own reports:**
  - Geometry: 39 / 177 heading jumps, jerk 38 and 12 in-building steps on dense Manhattan
    (see its section). Viaduct ramps are about 38°.
  - VRU: the PSM codec is not oracle-validated. No mid-block jaywalking. VRU devices
    transmit at 20 dBm, not 23.
  - Radioprop: Mangel is read from a reprint. The 25 m side cap is a design choice. Rain
    has no two-run test. The medium tier has no vehicle blockage.
  - Radioaccess: the NR BLER is still a fit. The J3161 values are second-hand. The compute
    tier `high` is partial. Hybrid is refused. The Studio has no widget for
    `radio.models.sidelink`.
  - Messages: no vehicle acts on SPaT (no GLOSA or red-light warning), and SRM priority is
    never granted. CPM is refused. The SPaT/MAP encoders are not independently decoded.
    PSID 0x82 is unverified.
  - Security:
    - ETSI butterfly authorization and ECTL are not driven.
    - Post-quantum sign/verify time is not charged.
    - Several backend-access figures are uncited defaults with no model card.
    - The 55-unit Phase 2 scenario and the Manhattan pseudonym-privacy study were not run
      end to end.
    - `compromised_rsus` is refused.
  - Session:
    - A vehicle that cannot avoid a closed lane leaves as RouteBlocked.
    - Radio and model parameters are refused by `param.change`.
    - An attack wave has a single window.
    - Sessions live in memory, so a resume across an engine restart is a fresh Hello.
  - Inspector:
    - TX-overflow and CRL-backlog drops are on no record channel.
    - "Depth now" can read low when the kernel is slower than real time.
    - No feed from a recording.
    - Roadside units are missing from a live Hello's node table.


## Crates

| Crate | Lines | State | Evidence |
|---|---:|---|---|
| `v2xw-core` | 13,487 | complete | 215 tests, zero clippy warnings. Determinism kernel independently reviewed: no defect in the RNG algorithm, math routing, event ordering, manifest digest or float reduction order. 1 critical + 10 major API defects found and fixed, then 7 more in a completion pass. |
| `v2xw-world` | 20,336 | complete | Imports real Midtown Manhattan in 377 ms. Verified below. |
| `v2xw-msg` | 7,956 | ETSI done, J2735 BSM in progress | ETSI stack generates from the forge modules and compiles (9,811 lines). |
| `v2xw-mobility` | 2,409 | in progress | — |
| `v2xw-radio` | 1,866 | in progress | — |
| `v2xw-net` | 1,374 | in progress | — |
| `v2xw-sec` | 555 | in progress | — |
| `v2xw-record` | stub | in progress | — |
| `v2xw-metrics` | stub | in progress | — |
| `v2xw-node` | stub | not started | Blocked on the radio, net, msg and sec trait definitions. |
| `v2xw-engine` | absent | **not started** | Owed by build-decision D8. Gates the headless end-to-end run. |
| `v2xw-cli`, `-server`, `-proto`, `-py`, `-wasm`, `-threat` | stubs | not started | |

UI: `ui/packages/{protocol,mock-server,viewer}` and `ui/apps/studio` exist; the
conformance and quality review is in progress.

## World import — verified

The importer was checked against the real city, not only against itself.

**Correct.** Projection error is 0.110 m worst case over an 856 m baseline
(0.013 %), cross-checked against three surveyed landmarks by geodesic distance.
The modal drivable heading is 60–61° with 90.5 % of lane length within ±4° of
it, which matches Manhattan's grid being rotated about 29° from true north;
Broadway correctly falls out as the 81° diagonal. Lane ordering is right-hand
traffic in 610 of 610 multi-lane edges and 178 of 178 two-way pairs, with none
wrong. Rendering confirms per-lane centrelines, crossings with stop lines,
sidewalks, turn connections and one-way chevrons. Building heights top out at
443 m with a median of 45 m.

**Scale.** 3,421 junctions (303 signalised), 13,760 lanes (5,450 drivable,
3,029 junction connectors, 8,291 sidewalk), 27,497 connections with 78 banned by
34 turn restrictions, 7,390 buildings, 963 crossings. 295 anomalies across 18
named categories, every one counted with example way ids, no panics.

**Defects found.** Six, recorded with evidence in
`findings/world-import-defects.md`. Two are major and affect any traffic result:
the speed-limit class defaults are SUMO's German rural values, giving 16 % of
drivable lanes a 100 km/h limit on Manhattan side streets, and lane width is a
single global constant so every lane is exactly 3.50 m. A third is worse for
routing: only 52.8 % of driving lanes lie in a strongly connected component.

## Traffic — audited, 2026-09-23

`v2xw_mobility::audit` checks every vehicle at every mobility step against the traffic
invariants (footprint overlap, gap below `s0`, lateral offset, body in a building or
outside its junction, red and avoidable-amber entry, two conflicting movements in one
conflict zone, lane change near a junction or round a standing queue, unconnected lane
transition, teleport, heading jump and flip, speed jump, acceleration and jerk bounds,
gridlock, mid-road despawn) plus three world checks. Measure any scenario with
`cargo run -p v2xw-engine --example traffic_audit -- <scenario.yaml> [--rate R]`; the
gate is `crates/v2xw-mobility/tests/traffic_invariants.rs`, which also runs a control
with the junction rules off that must go red.

300 s runs, before → after (counts are vehicle-steps, or pair-steps for overlaps):

| Class | dense grid (6000 veh/h) | Manhattan (manhattan-5min) | Manhattan, 6000 veh/h |
|---|---|---|---|
| overlap | 183 → 0 | 82 → 0 | 1787 → 0 |
| gap below s0 | 256 → 0 | 96 → 0 | 1189 → 0 |
| red entry | 59 → 0 | 42 → 0 | 287 → 0 |
| conflict zone | 33 → 0 | 14 → 0 | 257 → 0 |
| illegal lane transition | 119 → 0 | 56 → 0 | 515 → 0 |
| teleport | 2503 → 0 | 1419 → 0 | 6523 → 0 |
| lane change near a junction | 39 → 0 | 18 → 0 | 269 → 0 |
| heading flip | 0 → 0 | 19 → 0 | 108 → 1 |
| heading jump | 1198 → 0 | 367 → 10 | 1629 → 79 |
| body in a building | 0 → 0 | 1351 → 6 | 4397 → 12 |
| jerk > 30 m/s³ | 231 → 13 | 122 → 5 | 784 → 52 |

What remains, honestly: heading jumps are vehicles on OSM junction connectors whose
curvature is tighter than a 4 m radius; the bodies in buildings clip the corners of two
buildings built over covered tunnel ramps (The Horizon, The Corinthian); the jerk
excursions are emergency braking the car-following model asks for. The world check still
reports 64 lanes whose centreline runs through a building footprint — Park Avenue's
portals in the Helmsley Building and covered ramps, at grade in the source — and 19
conflicting protected greens in the synthesised OSM signal plans (crossing lane
assignments within one approach). Tunnels now run below ground and bridges above it.

Mean speed 4.4–5.8 m/s with 10–15 % of vehicle-steps standing;
no vehicle stands longer than 46 s.

## Phase 1 acceptance criteria

Three of seven are now met with independently reproduced evidence. "Independent"
means a second agent re-derived the number from the standard or the literature
rather than re-running the builder's test.

| # | Criterion | Status |
|---|---|---|
| 1 | Golden determinism, identical digests on each operating system | **Gate built, never run.** CI imports a committed fixture on all three platforms and fails unless the digests agree. Determinism itself is repeatedly proven *within* a platform: a double import of the real 30 MB extract is byte-identical across all four artefacts in separate processes, and two recordings of one run have identical data sections. No CI run has executed. |
| 2 | Envelope size equals the size model, zero bytes tolerance | **Met.** Exactly 93 bytes for a digest signer and 87 plus the certificate for a certificate signer, confirmed by decomposing a real signed message octet by octet against the derivation. The derivation's own byte-count threshold was wrong and is corrected (D12.1). |
| 3 | Modelled and real crypto produce identical logs | **Not met.** Genuine but incomplete: a verifier broke equivalence three ways, through signature malleability, invalid key material and unsupported post-quantum primitives. Repairs in flight. |
| 4 | Seek at most 100 ms at the 95th percentile | **Met, 57x margin.** 1.763 ms independently measured on a 9,001-frame, 600-second recording, using a type-7 quantile rather than nearest-rank so the figure is not a ranking artefact. |
| 5 | 60 fps with the heads-up display, every value resolving to a model card | **Met and exceeded.** 604 fps on a real GPU at 5,000 actors against ADR 0011's 60 fps target. Every heads-up value is keyboard reachable and opens its provenance; the focus indicator measures 9.10:1 contrast against a 3:1 floor. |
| 6 | Manifest lists engine, plug-in, world and card hashes | Pending. The world hash exists; manifest assembly is owed by `v2xw-engine`. |
| 7 | Manual map-to-chase fly-down | The automated fly-down passes end to end against the mock engine. Needs a human to judge. |

## 2026-09-23 — the measurement layer measures a real run

What changed, with the evidence (details in `08-measurement-and-data.md` §2.7):

- **Every reception attempt has one recorded fate** on `node.rx` (delivered, lost with one
  cause, or in flight at the end), with every stamp of the message's journey. Checked on a
  real 10-vehicle Manhattan run by `crates/v2xw-engine/tests/measurement.rs`: `node.rx`
  count = `phy.rx` count = the run report's attempts, and M-RX1, M-LAT1, M-BYTE1, M-BYTE2,
  M-SHARE and M-RANGE all hold with data to check. Shown to fail with delivery stamps
  shifted by 1 ns (821 M-LAT1 violations).
- **Frames carry their headers.** A BSM on the air is the SPDU plus 43 octets (WSMP 5,
  LLC/SNAP 8, QoS Data MAC header 26, FCS 4); a CAM over GN/BTP plus 82. Air time is
  computed over the whole PSDU. `net.layer: gn-btp` and `messages.generator` now reach the
  engine (a 200 ms BSM interval halves the frames; shown to fail with the parameters
  disconnected: ratio 1.0).
- **The verification queue runs in continuous time**, so a backlog outlives a tick and a
  frame waits from the instant it arrived. `node.verify` records now decode as the channel's
  reader-side view; they did not, for any record, before.
- **Measured on a 27-vehicle procedural grid, live in the page** (debug build): e2e p50
  10.0-10.7 ms and p95 10.3-21.0 ms (10 s bins), of which the OBU's 9.0 ms HSM signature
  is 61-86 % (30 s bins);
  air time 0.30 ms; security envelope 56 % of the octets on the air.
- **Finding for the radio track, not fixed here:** on that run 23-46 % of resolved reception
  attempts were lost to collisions and the mean MAC deferral grew from 0.5 to 5.3 ms while
  each node occupied 0.3 % of the channel. Every node generates at the same engine tick and
  signs for exactly 9 ms, so every frame reaches the MAC in the same instant; the
  decomposition (`latency_stage[mac_defer]`, `collision_rate`) is what makes this visible.
- **Cost, measured:** with `metrics: [all]` the 100-vehicle Manhattan rung ran 23.6 s of
  engine time for 2 simulated seconds against 14.6 s with `metrics: [pdr]` (debug). Most of
  the difference is four providers each decoding every `node.rx` record from JSON.

## Correction, 2026-09-22 — the end-to-end run is a stub at the message layer

An earlier entry here and my report to the owner both described the Phase 1 run
as producing "970 signed messages". That was wrong, and an independent audit
caught it. Nothing is encoded and nothing is signed: the node returns a size from
a model and hands the engine a byte count. The layers below messaging are real
and verifiably deterministic; the messaging layer is a faithful size model with
no payload and no signature behind it.

Recorded rather than quietly fixed, because the reason it passed unnoticed is
instructive. Every number in the run report was plausible and self-consistent,
the recording verified, and the digest reproduced. What gave it away was one
comparison nobody had made: two different message formats came out at exactly the
same size. Full detail in `findings/slice-verification.md`.

## Verification standard used

Every crate was built, then adversarially validated by an agent told to re-derive
rather than review. That produced results worth recording, because in several
cases the independent derivation was the only thing that could have caught the
defect:

- The packet-error model was re-implemented from scratch in Python and agrees
  with the crate to 5e-10 dB across all 24 cells.
- The message encoder was checked against two independent implementations: a
  Python oracle compiled from the real standards modules, and an encoder the
  verifier wrote from the encoding rules. 235 vectors, both directions.
- Signature determinism was confirmed by reimplementing the relevant standard in
  Python and reproducing the exact 64 bytes.
- The cryptographic port was checked by re-running the legacy Python to capture
  fresh vectors rather than trusting committed ones.
- The wire specification's worked hex dumps were re-extracted from the
  specification text at run time and shown byte-identical to the checked-in
  fixture, so the golden test really is the specification's bytes.
- Timing fixes were mutation-verified: reintroducing each bug reproduced the
  original failure signature.

## Corrections made to the design during the build

The design is not treated as infallible. Where implementation disproved it, the
document was amended and the reason recorded:

- **ADR 0008's pose quantisation was impossible as written.** Int16 millimetres
  spans ±32.767 m and cannot address a square-kilometre world. Corrected to i32
  millimetre keyframes about a per-run origin with i16 millimetre deltas about
  the previously *transmitted* quantised value, which is also what stops error
  accumulating, plus an absolute escape for teleports.
- **FlatBuffers dropped for VWP v1** in favour of a flat fixed layout, so the
  recorder can store the exact bytes that went over the wire and live and replay
  are provably identical.
- **ADR 0004 gained an evidence section** from the legacy digest forensics, which
  produced build-decisions D9 and D10.
- **D11** arbitrates five places where the design and the implementation
  disagreed.
- **CI contradicted D1**, pinning Rust 1.86.0 against the file's 1.98.1. Fixed,
  with an assertion so they cannot drift again.
- **The staged Phase 0 cleanup would have deleted three files** that
  `01-inventory.md` §3.7 explicitly preserves. Rescued into `legacy/reference/`.

## 2026-10-07 — everything merged after the account's spend limit stopped all agents

Every agent stopped at once when the account hit its spend limit. Each track's work was
committed (a wip snapshot where it was mid-change) and merged into main by the PM, with
conflicts resolved by hand and the whole workspace compile-checked:

- Merged: the owner-review round (stabilise, validation, settings and scenarios, health
  bands and run summary, always-on agent, scene and cameras, vehicle overlay, UI cleanup),
  the agent specialist, the US city library with scenario/city separation (64 extracts in
  worlds/cache; `v2xw cities`; `cities.list` over the API), threshold and post-quantum,
  long runs (checkpoint and resume, record levels, progress), traffic realism, emissions
  and energy, the driving-model library (ACC/CACC, sub-lane), and demand and route choice.
- NOT merged: the city-scale track (branch wrap2/scale). It rewrote the mobility step to
  run in parallel at the same time as traffic realism changed it and long runs restructured
  the CLI runner; untangling it needs its own pass. The V2X-networking, cellular/IP and SUMO
  interop tracks never started.
- Verified: `cargo check --workspace` passes, release binaries rebuilt, Studio typecheck
  clean, default scenarios validate, the engine starts and serves `cities.list`.
- Not verified: no test suite was run after these merges. Several merged tracks were
  mid-change, so expect rough edges.
- Known issues: resuming a checkpointed run that used `--city` falls back to the scenario's
  own world (the checkpoint does not record the city yet); the network validation suite
  reported real findings for their owners (CAM PSID 0x20 vs registry 36, an 802.11p noise
  floor off by 0.025 dB, an OpenSSL signature interop check, an analytical-collision
  mismatch at N=20); the trafficval IDM rest gap is 1.927 m against [1.95, 2.5]; the
  see-through-ground fix shipped in the scene track but is unconfirmed by the owner.

## 2026-10-07 — city scale merged onto main (branch wrap2/scale)

- Merged: the mesoscopic queue tier (`v2xw-mobility::meso`, Eissfeldt 2003 / SUMO MESO),
  the meso-micro tiering and its focus-region radio gate (`v2xw-engine::scale`), and the
  ordered parallel decide pass (`v2xw-mobility::par`). Conflicts resolved: the parallel
  decide closure now carries traffic realism's `detour_ok` and the drivers track's lateral
  decision; the gate's "dormant radios do not receive" check uses the radio index's
  `(actor, node)` pairs; `scale::normalise` runs after `city::adapt_to_world`; `v2xw run
  --no-recording --digest` reports the long-runs tally's record digest as `stream_digest`.
- Default behaviour: with no `scale` section nothing tiers, the holds set stays empty, and
  the decide pass runs serially below 64 vehicles or on one thread; at or above that it is
  an indexed rayon collect, so its output is in id order whatever the thread count.
- Verified: mobility `meso`/`par`/`tiering` unit tests, `v2xw-engine --test scale`
  (1/2/4 threads identical), and release CLI runs with the same digest at 1 and 2 threads:
  `scale/1000.yaml` (1,050 vehicles) `024937e0727cf796…` both; city rung at a 1,600 cap
  (1,166 vehicles) `bd087fc4e158f570…` both.
- Measured (release, LTO off, 16 codegen units, 2 s simulated, owner's 8 GB machine,
  `RAYON_NUM_THREADS` as stated). Real-time ratio = simulated s / wall s:

  | run | vehicles | threads | real-time ratio |
  |---|---|---|---|
  | `scale/100.yaml`, all microscopic | 106 | 2 | 0.82 |
  | `scale/1000.yaml`, all microscopic (before) | 1,050 | 1 | 0.011 |
  | `scale/1000.yaml`, all microscopic | 1,050 | 2 | 0.016 |
  | city rung, cap 1,600 (after) | 1,166 | 1 | 0.20 |
  | city rung, cap 1,600 (after) | 1,166 | 2 | 0.23 |
  | `scale/city.yaml` (cap 30,000) | 1,539 | 2 | 0.21 |

  At about 0.2x real time the all-microscopic engine carries roughly 250 vehicles (the
  page measurement above) and the tiered city carries at least 1,539, the most this
  Midtown box inserts in 2 s; at about 1,000 vehicles tiering is about 15x faster. The
  remaining cost at 1,000+ vehicles is the radio, not the decide pass (two threads: 1.4x).
- Not done: a longer run where the fleet keeps growing past the 2 s insertion limit, and a
  Studio control for the `scale` section.

## 2026-10-07 — packet-level cellular and TCP/IP for backend traffic

- `crates/v2xw-radio/src/cellular_packet.rs`: `cellular/uu/packet-pf` (`PacketUu`). Base
  stations per city (`BaseStationLayout::for_city` hex grid, explicit `from_sites`, or the
  old lattice); SINR against neighbour sites mapped to spectral efficiency (Mogensen
  attenuated Shannon, CQI-table clamp); one proportional-fair scheduler per cell and
  direction over PRBs per TTI (LTE 20 MHz / 100 PRB / 1 ms, NR 100 MHz / 273 PRB / 0.5 ms);
  HARQ at 10 % first-transmission BLER, RLC AM retransmission then loss; SR wait and grant
  chain on the uplink; core-network leg from the Coll-Perales MEC placement; handover
  holds and forwards packets (lossless AM) and *measures* the gap to the first block on
  the target cell. HARQ draws are pure hashes, so the copy-rollout that gives a delivery
  instant and the real cell agree (determinism). `comparison()` evaluates the M/M/1 simple
  model on every send and says when the two differ (>25 % and >5 ms, or >1 point of loss).
- `crates/v2xw-radio/src/ip_transport.rs`: TCP (IW10, slow start, CUBIC per RFC 8312,
  fast retransmit, RFC 6298 RTO), TLS 1.2/1.3 handshakes, HTTP and TLS-record framing over
  any `PacketPath` — `PacketUu`, `WiredLink` (RSU fibre/copper backhaul FIFO), or
  `ConstantPath` (the simple model). `ScmsExchange` for provisioning, pseudonym batches and
  CRL fetches; `compare_exchange` reports the difference from `2L + bytes/rate`.
- Engine: `net.uu.id: cellular/uu/packet-pf` (params `rat` lte|nr, `background_load`,
  `neighbour_load`, `isd_m`, `sites` or `city_bbox`). Every backend message over Uu then
  rides a per-vehicle keep-alive TCP/TLS connection. `AccessReport` gains HARQ/RLC/
  handover/TCP counters, `uu_differs_from_simple` and the `uu_vs_simple` sentence.
- Calibrated: an unloaded LTE request/reply lands in 15–40 ms against the 29.2 ms measured
  first hop (Narayanan WWW'20). `todo-calibrate` on the card: retransmission BLER, RLC
  timer, TR 36.881 access/handover figures, Shannon fit, background load, NR timings.
- Not done: the RSU backhaul in the engine is still the `Backhaul` latency+rate struct;
  `WiredLink` exists for TCP but is not yet wired into `backhaul/*` ids. No Studio UI.

## 2026-10-07 — radio and MAC fidelity (radio-fidelity track)

- **802.11p against Veins: 30.0 → 5.7 PDR points.** `propagation/winner-plus-b1`
  (`v2xw-radio/src/winner.rs`; WINNER+ D5.3 Table 4-1 / TR 36.885 Table A.1.4-1) ships
  with the effective antenna heights `h' = h − 1 m` (breakpoint 19.7 m). The old stand-in
  used the actual heights (177 m). The published simulators disagree on that convention:
  OpenCV2X and Molina-Masegosa match only the actual heights, at 4.1 / 9.3 / 6.0 points.
  Both conventions are offered (`params: {heights: effective | actual}`). Tables are in
  [`docs/validation/radio-fidelity.md`](../validation/radio-fidelity.md).
- **`radio.tuning`** (`channel`, `receiver`, `sidelink`): 17 researcher overrides covering
  path-loss exponent, shadowing σ and decorrelation, the Nakagami m profile, Sommer β/γ,
  vehicle blockage under any law, 802.11p noise figure, carrier sense and capture
  threshold, and SPS sensing, RSRP threshold, candidate %, probResourceKeep and T2. Each
  is bounded, status-classified, titled, and has an `x-explain`/`x-recommended` (new
  `guide::ADVANCED_HINTS`, advanced level). The loader refuses an override the composed
  model would ignore. An empty section leaves the scenario hash unchanged.
- **Hidden terminals labelled.** An 802.11p interference loss is `hidden-terminal` when
  the later of the two transmitters heard the earlier frame below CCA. Three tests that
  counted `collision` now count both labels.
- Verified: `v2xw-radio` winner/tuning tests; `v2xw-engine --test radio_tuning` (4);
  `v2xw-engine --lib scenario::` (27); the interop WINNER measurement (ignored test).
- Integration note: the shared `target/agents` build-script output for `v2xw-engine` is
  shared across worktrees (cargo hashes path packages workspace-relatively), so another
  track's schema can leak into this one's published-schema tests. Touch
  `src/scenario/schema.rs` before running them.
- Not done: IPG validation, B1 NLOS, OpenCV2X sub-channelisation in the harness, and an
  exposed-node label. The LTE sensing ring (≈ 330 MB per 1,000 UEs) is the next memory
  target.
## 2026-10-07 — cellular backend completed: every exchange end to end, wired backhaul media, eMBMS CRL

- **Every backend message rides the selected link model.** `v2xw_proto::net::PathOracle`
  on `BackendNet`: the kernel asks it for each message's arrival. The engine's `Carrier`
  (`crates/v2xw-engine/src/backend.rs`, shared by `BackendAccess` and both kernels) carries
  every request, response, acknowledgement and poll of top-ups, re-enrolments and ETSI
  trust-list fetches over the vehicle's keep-alive TCP/TLS connection through the PF
  scheduler (`cellular/uu/packet-pf`, via the ctx-free `AtClock` path), or over the
  relaying unit's backhaul line. A connection that gives up is reopened after RFC 6298's
  1 s, three times, then the nominal link is charged (`carrier.fallbacks`). The old
  512/4096-byte probe is gone. A cloned topology drops the oracle (preload's scratch kernel).
- **Wired backhaul per RSU.** `backhaul/fibre`, `backhaul/dsl`, `backhaul/microwave`
  (`WiredLink` media: asymmetric rate, propagation per medium, hops, loss; cards from
  `ip_transport::wired_cards()` in the catalogue), selectable in `actors.rsus[].backhaul`
  with numbers in the new `actors.rsus[].backhaul_params` / `actors.rsu_placement.backhaul_params`
  (`rate_mbps`, `down_mbps`, `latency_ms`, `distance_km`, `hops`, `loss`). Each unit has
  one FIFO line per direction: relayed reports, its own reports, the CRL feed for its
  broadcast and relayed vehicles' flow messages all queue on it; a lost packet is resent
  after Linux's 200 ms `TCP_RTO_MIN`. `backhaul/cellular` under packet-pf rides the unit's
  own TCP connection through the scheduler.
- **eMBMS CRL broadcast.** `net.uu.params.crl_delivery: mbms` (default `unicast`):
  unicast polls stop; a new list and each carousel turn (`mbms_carousel_s`, 10) go once
  per cell to every cellular vehicle in coverage lacking it, after the next MCH scheduling
  period (`mbms_period_ms`, 320, TS 36.331) and the bearer's serialisation
  (`mbms_subframes` × 100 PRB × 120 RE × 1.1758 bit/RE ≈ 1.4 Mbit/s per subframe, TS
  36.211 §6.7/§6.10.2, TS 36.213 Table 7.2.3-1).
- **Metrics.** `AccessReport.exchange_latency` per type (`topup`, `reenrol`,
  `trust-fetch`, `crl-fetch`, `crl-mbms`, `report-uu`, `report-relay`; count/sum/max),
  `AccessReport.carrier` (kernel messages over Uu and backhaul, backhaul line bytes,
  retransmissions, reconnects, fallbacks), `mbms_*`; certificate availability
  `Phase2Report.cert_unavailable_ns` / `cert_unavailable_max_ns` (time with no valid
  pseudonym, between node steps). Published on the backend view (`ee`, `rsu` entities);
  the agent's watcher announces backhaul retransmissions.
- **Calibrated, markers removed:** Shannon fit 0.75/1.25 (Mogensen VTC'07 link-level
  SISO), RLC t-PollRetransmit 20 ms for LTE and NR (ns-3 `LteRlcAm`; NR changed from
  10 ms), LTE SR-based UL access chain (TR 36.881 Table 5.1-1), LTE handover interruption
  49.5 ms (TR 36.881 §5.2, inside the TS 36.133 §5.1.2.1 50 ms bound). DSL rates cited to
  the FTTC 80/20 tier. **Still `todo-calibrate`:** `bler_retx`, `background_load`, NR HO
  interruption and NR timings, LTE overheads, PF window, fibre/microwave hop delays,
  microwave rate/outage, DSL interleaving.
- Tests: `ip_transport::wired_media_differ_and_queue`, `backend::carrier_tests` (2),
  `packet_cellular` (kernel messages carried, top-up latency measured),
  `security_lifecycle::a_vehicle_without_a_modem_relays_through_a_roadside_unit`
  (backhaul line used, relay latency measured). No MBMS integration run yet.
- Not done: positions are observed at flow start and on engine sends, not per kernel
  message during a long flow; MBMS reception has no per-vehicle loss beyond coverage.
## 2026-10-07 — traffic: fleets, strategic lanes, actuated signals, gridlock (branch of 7bd8d95)

- **N vehicles is N on the road.** `actors.vehicles.demand.fleet_size` (Basic) holds a fixed
  fleet: finished trips are replaced, refused or unroutable trips redrawn (`Demand::fleet_target`,
  `top_up`). `max_total_vehicles` now counts vehicles placed (`Demand::refund`), so a cap of 2
  no longer gives 0. Poisson only; refused with `scale` (the tiered engine draws its own trips).
- **Dead ends.** The router takes a U-turn where it is a dead end's only way on (the OSM
  importer builds it only where a car fits); the OD connected core counts it.
- **Strategic lane choice** (`traffic.strategic_lane_choice`, default on): routes are planned
  from every lane of the road (4 s per lane crossed); the driver changes lane before the
  MUTCD no-change zone, and one who reaches the zone without a gap takes a route from its own
  lane. On a two-lane grid a right-lane trip turning left had no route at all before this;
  peak-density quick run places 345 vehicles against 242.
- **Gridlock** (`traffic.gridlock_timeout_s`, default 300 s = SUMO's time-to-teleport): a
  vehicle standing that long (kerb stops and scripted stops excused) is moved to the next lane
  on its route with room, or removed; counted on `traffic.control` and as `traffic_*` metrics
  (`traffic_teleports` has a health band).
- **Signals** (`traffic.signals`): `control: actuated` runs NEMA-style basic actuation (min
  green, passage-time gap-out, extension, max-out, rest in green) by moving the plan clock, so
  drivers, pedestrians, lamps and SPaT agree; the SPaT states the window (minEnd = soonest
  gap-out, maxEnd = latest extension, status trafficDependentOperation). `green_wave_speed_mps`
  and `green_wave_heading_deg` set one-way progression offsets.
- **Congestion rerouting** (`traffic.reroute_period_s`, default 0 = off as in SUMO): lane speeds
  sampled each second, smoothed over 180 samples, priced into the router; each vehicle
  re-plans once a period.
- Also published: `traffic.right_turn_on_red`, `dynamic_rerouting`, `lane_changes`,
  `keep_junctions_clear`. Absent `traffic` section = these defaults (content hash unchanged).
- Verified: mobility lib tests (284), engine scenario/actuated tests, `tests/traffic_control.rs`,
  trafficval `--quick --only saturation` (saturation headway 2.15 s; start-up lost time 3.15 s,
  out of [1, 3] — the same value at 7bd8d95, it follows the Greenshields 1947 calibration) and
  `--quick --only peak-density` (10 safety-class vehicle-steps at 7bd8d95, 11 now; all one
  speed-jump of ~10.5 m/s² at t 88.4 s on lane 119, from the standing-leader advance cap).
- Not done: permitted-left critical gap (5.99 s vs HCM 4.5 s: the opposing car's predicted
  arrival is earlier than its actual one); pedestrian calls for actuated phases; actuated
  coordination (offsets hold only under fixed-time); roundabout-specific yielding; the
  agent's watch does not yet read the traffic metrics.
## 2026-10-07 — traffic calibration and driver heterogeneity

- **Human drivers** (`v2xw_mobility::drivers`; scenario `actors.vehicles.drivers`).
  - `realism` presets: `calibrated` (the default, unchanged), `identical`,
    `heterogeneous` and `human`.
  - Overrides: `parameter_spread`, `reaction_time_s`, `perception_error`,
    `truck_lane_restriction` and `motorcycle_filtering`. New pedestrian option:
    `actors.vru.crossing_gap_margin_s`.
  - Each field is bounded, has a KEY_STATUS row, a title and choice words. Realism and
    reaction time are Basic fields, and there is a new "Drivers" quick setting.
  - Per-driver T, a, b and s0 are mean-one lognormal factors with per-class cvs.
  - Reaction time is a delayed car-following decision, with an alarm when braking past b.
  - Perception errors are the HDM's Ornstein-Uhlenbeck errors (Treiber, Kesting & Helbing
    2006).
  - The truck restriction keeps trucks out of the leftmost lane of motorway and trunk
    roads with 3 or more lanes (StVO §18(11)).
  - All noise is keyed per driver and step, so runs stay deterministic. Defaults are off,
    so calibrated runs are unchanged.
- **Traffic-calibration study**: `cargo run -p v2xw-trafficval --bin traffic_calibration`
  writes `docs/validation/traffic-calibration.md`. Full mode passes. It includes city and
  corridor fundamental diagrams with Greenshields and triangular fits, each beside SUMO's
  default Krauss driver. Results:
  - Capacity sits on the IDM's equilibrium maximum flow: 1,899 veh/h for the city ring
    (0.0 % below) and 1,737 for the corridor (0.3 % below).
  - The human population keeps 98 % of capacity.
  - Stop-and-go waves travel at -12 to -18 km/h. The triangular congested branch gives
    -13.6 km/h.
  - Human drivers jam at 5 of 11 densities where identical drivers do not.
  - No collisions.
  - The headway cv is 0.22 for human drivers against 0.006 for identical ones.
  - Against the HCM's 2,400 pc/h/ln: Treiber 2000's drivers reach 0.72 of it and SUMO's
    default reaches 1.17.
- **Open findings fixed**:
  - The IDM rest gap (1.927 m) and string stability at 6 densities already pass under
    the corrected check. Both are re-run in the study.
  - `model/sugiyama-ring` now passes (jam at -14.9 km/h) with the human population. Its
    fault is the ideal driver.
  - Fixed a bug where the jam speed was read from the least-squares intercept instead of
    the slope.
  - The jam speed is now measured from the outflow front.
- **Not done**: NGSIM trajectory import, which would let the headway distribution be held
  with a Kolmogorov-Smirnov test. Also not done: motorcycle filtering beyond the existing
  SL2015 model, and a social-force recalibration.
- **Not re-run**: `docs/validation/traffic.md`, which needs a full traffic_validation run.
- **Build hazard**: `target/agents` shares v2xw-engine's build-script output
  (`scenario_reflect.rs`) across worktrees. A stale reflection from another worktree made
  the publish tests fail. Touch `src/scenario/schema.rs` inside the heavy lock before
  testing.
## 2026-10-08 — traffic soak: run, find, fix (branch of 4986cd4)

- **`v2xw check-traffic <run dir>`** reads a run's recording through the Studio's replay
  scene and rebuilds its world, so it checks what the page shows: queue heads standing on
  their own green or at a clear unsignalised junction, mid-lane stops, stops past a
  cycle, red entries (judged where the front crossed the stop line), lamps matching no
  phase, overlaps, jumps, braking, jerk, speeding, pedestrian jumps; plus discharge
  headways by queue position, saturation headway, start-up lost time, speeds per road
  class. `--trace <actor>` prints one actor's steps with the lamps, occupants and exit of
  each movement ahead. Test: `tests/check_traffic.rs` (a clean busy grid passes; a plan
  the lamps do not match fails). Traffic-only soaks (`equipped_fraction: 0`) run 300 s of
  Midtown in 8 s; with the radio stack and full recording, 180 s took 734 s (node.rx
  writes 4 GB of JSON).
- **Fixed, each found by a soak run and re-run:**
  - Recorded lamps kept the fixed-time clock while actuated control and priority moved
    the plans: actuated grid, 446 red entries and 2 green standstills as recorded -> 0.
  - Vehicles crawled at 0.1-0.2 m/s toward a red far ahead (the stop-at-line floor bound
    at every distance) and into standing obstacles inside s0; now they drive up and stop.
  - The standstill-gap cap's -10.5 m/s² step (phase1-grid lane 119): capped at 8.5.
  - `portland-downtown.yaml`: 8 vehicles in 300 s -> 84 (connected-core OD on every
    imported map).
  - GLOSA lane match through a strategic lane change (0 of 77 -> 7 of 53 followed
    without the 1 s hold; still not the whole cause).
- **Open, seen in the soaks:** queue heads on protected green 8-27 s at a few Midtown and
  Chicago stop lines (exit room / junction-clear holds, not yet traced); stops of 160-180
  s on midtown-rush-hour; 151 long stops and 10 lateral overlaps on highway-congestion;
  start-up lost time 3.17 s (target 1-3); `check-traffic` counts lawful right turns on
  red as red entries (Chicago 31, Portland 11); phantom stops are mostly kerbside stops
  that stay on the lane centreline (no pull-over), which the recording cannot tell apart.

## 2026-10-11 — network fidelity: unknown signers, B1 NLOS, OpenCV2X pool, IPG, unicast retry (branch of d3558de)

- **verify-all held:** a message whose `digest` signer missed the certificate cache was
  delivered `unverified` under `verify-all`. `VerificationPolicy::holds_unknown_signer`
  (true for verify-all; OBU, RSU, VRU) now drops it as `node.rx` cause `unknown-signer`
  (P2PCD request still counted). Grid, 20 s: 77 of 511 app deliveries (15 %) were such
  messages; now 0 delivered unverified (`v2xw-engine --test unknown_signer`).
- **B1 NLOS:** `propagation/winner-plus-b1` `params.nlos: sommer | manhattan` (WINNER+ D5.3
  Table 4-1 two-leg law over the traced corner). Tables in `docs/validation/radio-fidelity.md` §3a.
- **OpenCV2X 2 × 12 RB pool and IPG** in the sweep harness: 120 veh/km MAD 9.3 → 6.5
  points when 190 B fits one sub-channel; 19.2 taken literally (§1).
- **GeoUnicast link-layer ACK/retry** (`DissemConfig::unicast_retry_limit`, 802.11-2020
  Annex C, limit 7): lossy 10-hop line, GBC to a remote area, PDR 0.20 → 1.00, hop ACK
  ratio 0.81 → 1.00 with 6 retries. The engine's overlay keeps 0 (no MAC, unit disk); not
  yet a scenario field.

## 2026-10-11 — traffic stalls: breakdowns, merges, right turn on red (branch of d3558de)

Traffic-only soaks (`equipped_fraction: 0`, 300 s) of midtown-rush-hour, portland-downtown
and the city base on `--city chicago`, checked with `v2xw check-traffic`.

- **Fixed:**
  - The 160-180 s stops on midtown-rush-hour (lane 10719, the cars on lane 280, actor 152)
    were the scripted `safety.breakdown`: the auto pick took any moving car, which came to
    rest inside a junction connector or just past the stop line and held everyone behind
    it for the whole event (plus 21 overlaps as cars took the next connector past it). The
    auto pick now takes a car that stops on its own lane 10 m or more short of its end, and
    a car standing 8 s with a broken-down lane on its next four route lanes replans round it.
  - Two connectors onto one exit lane that touch a few metres before it were a crossing,
    so the second car waited at its line until the first cleared (actor 193, 5 s on green);
    they now merge.
  - `check-traffic` excuses a right turn on red after a full stop where the jurisdiction
    allows it: Chicago 31 red entries -> 1, Portland 11 -> 0 (its check passes). Portland
    spawns 84 vehicles in 300 s.
- **Open, traced:**
  - Queue heads on green 9-20 s at Midtown links of 13-16 m between two signals (lanes
    2695, 3159), and a 266 s stall at Chicago junctions 324/326, 12 m apart: the short link
    is fed in two phases upstream but served in one downstream, and a car may not stop
    across the crosswalk between them. Pairing the offsets of closely spaced signals (tried,
    not committed) did not help because the phases are complementary; the fix is a joined
    junction, or one controller with a shared phase plan, for such pairs.
  - Phantom stops (kerbside stops on the centreline) block lanes for 20-57 s and queue cars
    on green behind them (Midtown lanes 1405, 718).
  - One lane-change frame jump (2.9 m in a step, lanes 2268 -> 2269, Midtown).
  - `pedestrian_invariants` fails on d3558de itself (overlap 26, in-building 6; overlap 17,
    in-building 21, speed-jump 1, accel-bound 1, all VRU actors); unchanged by this branch.
  - Start-up lost time: Midtown 2.4 s, Chicago 3.3 s (two queues only).

## 2026-10-11 — security and protocol realism: run, find, fix (branch of 500cbb0)

Short runs of the attack and credential scenarios, read from `run-report.json` and
`metrics.json`.

- **Fixed:**
  - Detection metrics were never fed in an engine run: every `det_*` sample read zero
    trials. The engine declared no subject and its `gt.attack.action` records went to a
    null recorder. New ground-truth channel `gt.subject` (digest to actor, once per
    digest); the vehicle level is one trial per true actor. attack-fake-positions 300 s:
    TP 12, FN 3, FP 0, TN 408 (recall 0.80, FPR 0); report precision 0.986. Time to
    decision p50 55 s, mostly the compressed 60 s report shuffle. CRL installed 3,624
    times; 196,093 receptions from revoked senders rejected.
  - RSUs could not verify post-quantum messages. They now use `pq_software_proxy`, the
    Cohda MK6 rows of NDSS 2024 Table V (ML-DSA-44 verify 189 µs), for post-quantum
    primitives only. The proxy is flagged and likely optimistic. `v2xw validate` notes it
    and the manifest warns (06-node-models.md §7.6).
  - Certificate-cycle fragments pushed hybrid SPDUs past the MTU: pq-certificate-cycle
    had 391 refusals in 30 s, and no fragment reached the air. Now 0.
- **Checked, plausible:**
  - Replay (90 s): 0 honest vehicles reported to the MA.
  - credential-lifecycle (150 s): top-ups 59/59, pool steady at 5, re-enrolments 18/18,
    2 of 2 attackers revoked, 0 honest.
  - Pseudonym strategies (120 s): c2c-cc 57 changes per vehicle-hour with linkability
    0.36; mix-zone 22 and 0.21. time (300 s period), distance (2 km) and silent make 0
    changes in 120 s, as specified.
- **Open:**
  - Sybil is still refused: the host hands an attacker no concurrent pseudonyms, and
    receivers drop ghosts' unknown signers.
  - 15 % of honest pseudonyms are reported at least once in 300 s of rush hour. These
    are GNSS outliers; the MA filters them (0 honest revoked).
  - 22 revoke decisions for 12 attackers: one decision per pseudonym, not per device.
  - A 300 s attack run at `--record-level events` writes 6.9 GB; use `metrics`.

## 2026-10-11 — round-8 merge

- Merged into main: protocol (detection metrics fed by gt.subject and gt.attack.action, RSU PQ software proxy, cert-cycle fragments fit the MTU) and studio (quiet agent client when no agent, Run shown when settings are staged, Undo after Run).
- **Held, not merged:** realism (`worktree-wf_defd1f50-e87-6`, f3b5cc8). It brings grid travel time against SUMO from +18 % to +6 %, but the safety suite got worse (37 FAIL against 33): connected-intersections 0 -> 17, phase1-grid 20 -> 54, peak-density 0 -> 16, closure 0 -> 7. The track asked for these to be fixed before merging. Its worktree is kept.
- Checks: cargo check --workspace --tests; tests rsu_pq_proxy, copilot control, metrics/record/cli lib; studio typecheck and vitest 284/284; release build; run-sim.sh boots on 8758 with the agent on.

## 2026-10-11 — traffic audit: important traffic, world and run parameters configurable (branch of 0964c12)

- **Exposed** (validated, published with unit, label and guide text, read at run time; defaults unchanged, so no digest moves):
  - Drivers: `speed_factor`, `speed_spread`, `start_reaction_s` and `queue_start_delay_s`.
  - Per-class `truck`/`bus`/`motorcycle`/`bicycle` a, b, T and s0.
  - Junctions: `traffic.critical_gap_factor`, `permitted_left_gap_s` and `turn_lateral_accel_mps2`.
  - Kerbside: `traffic.kerbside.*`.
  - OSM signal timing: `world.signals.{cycle_s,all_red_s,pedestrian_walk_s,pedestrian_speed_mps}`.
  - GNSS: `actors.vehicles.gnss.*`.
- The loader refuses settings that would do nothing: speed and reaction fields with identical drivers, a class missing from the fleet, kerbside fields with kerbside off, signal timing on a non-OSM world, and GNSS with no radios.
- `tests/traffic_audit_config.rs` runs one same-seed test per group; with the wiring removed, all six fail.
- Inventory: docs/guides/config-inventory-traffic.md, covering 171 constants and the model structs.
- **Open:**
  - The meso tier ignores the speed and reaction fields.
  - RTK keeps the 1.6 m white noise.
  - `gap_factor` is not on the gap-acceptance card.
  - `cargo clippy -p v2xw-engine` stops on an existing v2xw-radio error (bler_nr.rs:222).

## 2026-10-11 — realism vs SUMO, safety regressions closed (branch of wip/realismfix)

- Verified the paused claim with fresh runs of both trees' binaries (main 0964c12 built from source): connected-intersections 0 (main 0), peak-density overlaps 0, closure 0, phase1-grid at 20,000 veh/h 18 (main 20). Causes, by ablation (4e6fd61): the target-lane follower search missed a car on the connector into the target lane; give-up re-plans accepted only the destination lane; the lane-end barrier came up too late; the follower courtesy (`informFollower`) locked Manhattan at 3600 veh/h by spill-back.
- New fix: a red or amber the driver stops for binds even while a nearer car ahead goes on through it (it lost to the leader as the nearer constraint, and the follower then entered on red). Peak-density red entries 4 -> 1 (main 3); seeds 11/22/33 none (main 2).
- New fix: an insertion sees a vehicle still changing out of the origin lane (it was dropped at rest across the changer's body). Dense-grid invariant test: overlap 13 -> 0, in-building 6 -> 2 (main 9, 17; the test fails on main too).
- New fix: a lane change begun while moving is held while the vehicle stands part-way across (it slid sideways and swung its heading on the spot; a truck swept into the next lane's queue). tests/realism.rs kerbside: overlap 206 -> 47 (main 242 plus standstill 3; the test fails on both). highway-congestion 671 -> 435 (main 866).
- Traffic validation 31 FAIL (main 32). Still worse than main at the shipped seed: midtown-peak-load 269 vs 258 (pedestrian step-outs, hard braking), berlin 283 vs 280 (a merge conflict zone), permitted-left Raff gap 7.01 vs 5.98 s (n = 21; no lags between about 2 and 7 s, so Raff's crossing point is set by one or two samples). Over seeds 11/22/33 midtown-peak-load is 1288 vs main 1328 (measured before the insertion fix).
- Cross-validation regenerated: grid 3600 paired TT ratio 1.09, r 0.93 (main 1.21, 0.89); Manhattan 3600 376 trips vs SUMO 368, 24 left vs 32.

## 2026-10-12 — sensitivity: radio, MAC and congestion (branch of wip/sens-radio)

- `crates/v2xw-engine/tests/sensitivity_radio.rs`: 27 tests over four specs in
  `scenarios/sensitivity/` (`radio-dsrc`, `radio-dsrc-dense`, `radio-gn`, `radio-cv2x`) show
  that tx power, noise figure, path-loss exponent, shadowing, fading, data rate, payload,
  fleet size, message rate, CWmin, AIFSN, MAC queue length, reactive/adaptive/J2945 DCC, GN
  hop limit, sidelink MCS, transmissions, RRI, keep probability and sensing move their
  metrics the way theory says, on DSRC and LTE/NR sidelink. Table and theory:
  `docs/validation/sensitivity-radio.md`.
- Fixed: a node's first step waits for its generation phase (the mobility step no longer
  moves collisions); J2945/1's tracking error reads the navigation solution, not two noisy
  fixes (density now stretches the interval); `range90_m` ignores a one-bin dip.
- Open: saturated dense runs (150 vehicles, 40-50 Hz) are far slower than quadratic in the
  fleet; untraced.

## 2026-10-11 — sensitivity: security, message and backend settings move the metrics (branch of 0786d92)

- `tests/sensitivity_security.rs`: 14 same-seed tests, about 80 s together. Rows: signature scheme, certificate cadence, new-neighbour attach, verification policy, crypto hardware, verify queue depth, pseudonym period, message rate, envelope, Uu preset and penetration, RSU backhaul latency and bandwidth, CRL cadence and fetch interval, attacker share. Table and numbers: docs/validation/sensitivity-security.md.
- **New setting:** `security.signer_id_policy.on_new_neighbour`, the ETSI CAM rule (TS 103 097 §7.1.1). Off by default, so no digest moves. The loader refuses it when it would do nothing. A forced certificate does not restart the cadence; restarting it made unknown signers worse at a fleet-wide pseudonym change (843 -> 3,948). Now 843 -> 105. With the wiring removed, its test fails.
- **Open:**
  - The 5 Hz CBR sample reads about 0 for a fleet spawned together. The MAC CBR covers only the last 100 ms of each second, and the phase moves generation inside a step, not across the period.
  - Pseudonym changes are counted only with a backend protocol.
  - There is no Part II size knob.
- Checks: v2xw-node tests; v2xw-engine lib `scenario` tests; sensitivity_security 14/14.

## 2026-10-12 — round 10 integration (sensitivity: radio, security, traffic)

- Merged in order: sens-radio (27 rows, 4 radio specs), sens-security (16 rows; P2PCD
  metric, `messages.path_history_points`, silent-period cadence fix), sens-traffic
  (`world.source.params.ew_green_share`, default 0.5 keeps worlds and digests unchanged).
- Only conflict: this file (both sections kept). `cargo check --workspace --tests` clean;
  engine lib scenario/publish/guide 31/31; sensitivity_radio 27/27, sensitivity_security
  16/16 (1 ignored), sensitivity_traffic 14/14, network_audit_config 7/7; node and world
  lib tests pass. Release `v2xw-cli` builds; `run-sim.sh` boots with the agent on.
- Output shifts: US DSRC runs (generation-phase first step, J2945/1 nav tracking);
  `node.telemetry` gains `p2pcd_requests`, so recordings differ.
- Open: dense saturated runs scale badly (~n^3 suspected); 5 Hz CBR reads ~0 for a fleet
  spawned at once; `ew_green_share` bound not checked by validate.rs; v2xw-radio clippy
  error (bler_nr.rs FRAC_1_PI) blocks clippy on the engine.

## 2026-10-12 — speed: saturated radio runs (speed track, branch of 05599f6)

Optimised dev build (`opt-level 2`, debug assertions on), binaries copied out of
`target/agents`, `v2xw run --no-recording --digest`, owner's 8 GB M2 shared with other
agents (wall ±10 %). Saturated: `scenarios/sensitivity/dsrc-dense.yaml` with the
"queue 64->1, saturated" row's base settings (40 Hz, 25 ms step, DCC off, certificate on
every message, 1 s) and the trip cap at 60/100/150. Fleet: `scenarios/scale/base.yaml`
with `metrics: [all]`, cap 1,260 (+ `radio.range.max_m: 500`) or cap 560 uncapped.

| run | vehicles | threads | wall s per sim s before → after | peak MB before → after | digest |
|---|---|---|---|---|---|
| saturated | 49 | 2 | 1.8 → 1.8 | 45 → 46 | `5b86db93e9dc` both |
| saturated | 75 | 1 / 2 | 14.3 / 17.2 → 4.1 / 4.5 | 59 → 65 | `351c0b650895` all four |
| saturated | 99 | 2 | 32.1 → 5.8 | 74 → 82 | `7aeb1545a6d3` both |
| capped 500 m | 1,049 | 1 / 2 | 25.0 / 18.0 → 24.4 / 18.9 | 415-575 → 423-477 | `54ef582c5ba9` all |
| uncapped | 520 | 2 | 21.4 (unchanged code path) | 315 | `4ad18868be20` |

**Cause of the ~n^3 (measured, `sample`):** not a busy/idle edge fan-out — the engine
has none; carrier sense is sampled at MAC polls. Each poll called `medium_idle_at` /
`medium_busy_since`, which scanned the **whole frame table** (every queued frame of every
node: up to 64 x n when saturated) for the node's own transmission — 65 % of the main
thread at 75 vehicles — and `OfdmPhy::energy_dbm` walked every live arrival at every
receiver. Polls per second grow ~n^2, so the run grew ~n^3.

**Fixed** (`e71284d`): the engine keeps each node's on-air frames (`on_air_tx`, pruned at
the node's next poll) and the PHY reads one receiver's arrivals as a key range in the same
order. Same digests at 1 and 2 threads; radio `phy` lib 29/29, `radio_tuning` 4/4,
`sensitivity_radio` 27/27.

**Measured, not changed:**
- 93 % of saturated MAC timer events are repeat polls of one node at one instant
  (4.2 M of 4.5 M at 99 vehicles): every busy poll schedules another wake at the same
  `clear`, and the copies never merge. Dropping them is not results-neutral (a repeat
  poll after another node started a frame at that instant freezes a backoff the next poll
  might not), so it is a model decision for the owner: dedupe and accept new digests.
- A memo of building classification by exact link (46 % of uncapped classifications
  repeat, queued cars at red lights) gave 8 % wall for +650 MB peak; reverted. Uncapped,
  ring tests are 17 % and the R-tree walk 11 % of all CPU; main thread waits 73 %.
- Capped 1,000: main thread 46 % waiting on the parallel link map, then node step
  (one wake per reception: envelope parse + verify, ~8 %), metric providers (~8 %),
  candidate walk (~7 %), SHA-256 ~3.5 %. No single item above 10 %; not reached.

## 2026-10-12 — protocol and security engine fixes (branch of 05599f6)

- **Verify queue order:** the queue serves the highest policy priority first (only
  `prioritized` sets one; the others stay first in, first out, so their digests do not
  move). New `security.prioritized_horizon_m` (300 m). Near wait 0.42 ms vs far 0.92 ms;
  with the order removed, 0.87 vs 0.88.
- **On-demand relevance:** with no application score, a message is scored
  1/(1+TTC) from its own claim against the receiver's belief. New
  `security.on_demand_threshold` (0.5). Unverified share 0.992 -> 0.948 at 0.05. Both new
  keys are refused under any other policy.
- **Peer certificate cache:** eviction now removes the certificate from the verification
  store. New `nodes.peer_cert_cache` (128). Cache 4: P2PCD 373 -> 21,903, unknown-signer
  373 -> 24,424 (373 with the fix removed).
- **5 Hz CBR:** the generation phase spans the message period (`messages.rate_hz`), so 5 Hz
  CBR reads 0.048, not about 0. The 10 Hz default keeps its 100 ms window.
- **MA per device:** a Revoke about a pseudonym of a device already decided (PCA + LA
  resolution) counts as `decisions_linked`, not a decision. 30 % attackers: 7 decisions for
  8 attackers (14 before).
- **Recording:** `--record-level events` drops `node.rx`, `phy.rx` and `node.verify`. These
  were 95 % of an attack run's bytes. The old level is `receptions`. Metrics are unchanged.
- **Output shifts:** `prioritized` and `on-demand` runs change, and so do runs at a
  non-10 Hz `rate_hz`. `node.verify` records carry the served priority, and the phase-2
  report gains `decisions_linked`.
- **Open:**
  - Sybil is still refused. The ghosts need extra frames from `hand_down_app`, signed with
    concurrent pseudonyms the attacker holds. They also need the certificate attached so
    receivers can verify them.
  - Only `ObuRuntime` orders by priority (the RSU and VRU runtimes do not).

## 2026-10-12 — traffic stalls and VRU interaction (branch of 05599f6)

- **Settings wired:** `traffic.critical_gap_factor` now also scales the right-on-red gap;
  the pure queue tier (`mobility/meso/queue`) draws drivers with the scenario's speed
  factor and spread (reactions refused there: a queue discharges at `tau_jf_s`);
  `actors.vru.walking_speed_mps` (Knoblauch 1996, 1.51 m/s default under `observed`);
  `world.source.params.ew_green_share` published as a field (bound 0.1-0.9, refused on an
  unsignalised grid); `world.signals.join_m` (netconvert `--tls.join`, opt-in, 0-40 m).
  All default to today's behaviour. Engine scenario tests 27/27, world osm 19/19.
- **Measured, not fixed:** Midtown rush (traffic only, 300 s): `join_m` 20 joins nothing
  the short-lane join had not; 40 joins 4 pairs, including the lane 2698/2695 stall (a
  crosswalk signal 37.5 m from a junction), but stopped-on-green went 10 -> 13. Chicago:
  no change. Pedestrians judging stopping distance at 2.0 m/s² instead of 3.4 (tried, not
  committed): midtown-peak-load decel exceedances 46 -> 47, so the hard braking there is
  not the pedestrians' step-out test.
- **Open:** kerbside overlaps from junction connectors; drivers anticipating kerb-waiting
  pedestrians; `docs/validation/traffic.md` not regenerated (no model output changed by
  default).
- **Build hazard:** agents share the scratchpad. Keep scripts and logs in a per-worktree
  subdirectory, or another agent's `cd` runs your tests in its worktree.

## Round 11 integration (2026-10-12)

Merged with --no-ff, in this order: perf (saturated-run speedup), protofix (security and protocol engine fixes), trafficfix (traffic settings wiring). The only conflicts were in BUILD-STATUS sections, and both sides were kept. Then `cargo fmt` (publish.rs).

- Checks: `cargo check --workspace --tests` is clean. Engine `scenario::` lib passes 27/27. The new sensitivity_security rows (on_demand, peer_cert, prioritized) pass 3/3. Radio phy passes 29/29 and v2xw-node lib passes 136/136. The release CLI build passes. run-sim.sh boots on port 8758 and the agent status endpoint returns 200.
- Output changes come from protofix only. They affect prioritized and on-demand runs, runs with rate_hz other than 10, MA decisions in attack runs, and the events record level, which now omits receptions. The perf change keeps the digests identical. The trafficfix settings default to the old behaviour.
- Still open: Sybil ghost frames; priority ordering for the RSU and VRU runtimes; repeated MAC polls at the same instant (removing them needs new digests, owner to decide); join_m stops-on-green; the full sensitivity_security suite has not been run end to end.

## 2026-10-13: catalogue-wide sensitivity sweep (branch of d06658f)

- **Command:** `v2xw sensitivity --catalogue`, implemented in `crates/v2xw-engine/src/sensitivity/sweep.rs`. It moves every published numeric or choice setting (217 settings, 420 perturbations) on a same-seed family run. The families are DSRC, LTE, NR, GN, security, traffic, saturated, OSM Portland and pedestrians, all in `scenarios/sensitivity/catalogue.yaml`, which also holds the routes, prerequisite contexts and value overrides. The sweep diffs a fingerprint of about 280 metrics (radio harness, `report.*`, `metric.*`, kinematics). It has a JSON-lines `--cache`, so a stopped sweep resumes.
- **Outputs:**
  - `docs/validation/sensitivity-catalogue.md`: the matrix and three lists (89 dead, 1 contradiction, 13 refused), with an analysis of each.
  - The seed expectation table `crates/v2xw-engine/tests/expectations/catalogue.yaml`: 36 rows, each with a cited source.
  - The CI subset `tests/sensitivity_catalogue.rs`: 3 tests, about 4 s. An injected wrong-direction row turns it red.
- **Not changed (owner decision):**
  - Weather intensity is ignored on arterials. Fixing it changes the midtown-rain digest.
  - The loader does not refuse `truck`, `bus` and `motorcycle` driver parameters, or the RSU and VRU device parameters, when there is no such actor.
  - RTOR is dead on the procedural grid.
- **Cost:** a full sweep from an empty cache takes about 47 minutes on two threads, and the saturated family dominates. With the cache it takes about 8 minutes. The nakagami context fix is committed but has not been re-swept. Clippy has not been run.

## 2026-10-13 — agent harness robustness (branch of d06658f)

Track: the built-in agent (`crates/v2xw-copilot`, the agent host in `v2xw-cli`).

- **Eval suite** `crates/v2xw-copilot/evals/harness.yaml`: 48 cases. They cover setting things by plain name or synonym, several settings at once, cities, refusals, run control, A/B tests, compare-with-previous, sweeps and analysis. The offline half (`tests/harness_eval.rs`, no model) checks key resolution, value repair and refusal text. It scored 20/60 on the harness as received and scores 60/60 now. The live half is `examples/agent_eval.rs --suite …harness.yaml` with new `draft` / `calls` / `no_calls` checks. Its reports are in `docs/validation/agent-harness-live-{1,2}.md`. The live runs used 62 model calls with gpt-6.1-sol. Run 1 passed 11/15 and run 2 passed 8/10. Every remaining failure was in the scoring, not the agent: two expectations were wrong about the engine (tx default 23 dBm; `digest_otherwise: false` already means a cert on every message), one explanation phrase was missing from the vocabulary, and run status was not counted as a source. All four are fixed in the suite and runner but have not been re-run live, because the call budget was spent.
- **Fixes**:
  - The model was being served the old all-words surface search under a duplicate `find_settings` tool name. There is now one tool, with synonyms, stop words, plurals and ranges.
  - `repair.rs` now reads a change against the published index: short keys, values with units (`2 min`, `30%`, `200 ms` → 5 Hz), choice synonyms (`C-V2X mode 4`, `802.11p`, `5G NR`), on/off. It also adds companion settings: a cert on every message sets `digest_otherwise` to false, and a PQ signature switches to an OBU that prices it, as in `pq-signatures.yaml`.
  - Refusals now carry the field's choices, range, unit and recommended range.
  - `changes` is accepted in any shape models write.
  - `run_control` understands action synonyms (restart = seek 0). `start` used to start paused at maximum speed; it now plays.
  - `setup` no longer caps the duration.
  - Both providers retry timeouts, resets and 408/409/425.
  - A history budget sends earlier turns' long tool results cut once the conversation passes 120k characters.
  - Studio runs are handed to the agent (`Agent::adopt_run`), so "compare with the previous run" reaches runs the person made.
  - Numbers about earlier runs are read from a tool in the same turn.
- **Checks**: copilot lib 112/112, tests control 5/5, agent 7/7, harness_eval 1/1; `v2xw-cli` bins build. Clippy on copilot is blocked by an existing `v2xw-radio` lint error (`bler_nr.rs:222`, FRAC_1_PI). No engine output changes; digests unaffected.
- **Open**: run the full live suite (`--all`, about 110 calls). Studio-run adoption has no live end-to-end test yet: it is unit-tested, but the host wiring has not been exercised with a browser.

## 2026-10-13 — compare with the previous run (branch of d06658f)

- Engine/server: `crates/v2xw-server/src/compare.rs` reduces a run to a digest (scenario, seed, city, every headline metric pooled over the run plus a 400-point series). The live engine keeps the last eight runs when `run.start` begins the next one, but not on a seek rewind. New RPCs: `runs.list`, `runs.compare` (json/markdown/csv/text) and `runs.digest`; the agent picks them up from OpenRPC. Each comparison gives the labelled settings diff, previous/current/change/% per metric with its direction and health on both sides, a meaningful flag (crosses a band, or ≥10 % and past the unit's resolution; with different seeds it must also exceed 2× the combined batch-means SE), and a computed summary.
- CLI: `v2xw compare <A> <B>` takes run dirs, digest files, experiment dirs or comma-separated replications.
- Studio: header "Compare with previous", a run-summary button, and a dialog with sides, a seed/noise note, the summary, settings, an overlaid chart, the metrics table, CSV/Markdown export and Ask the agent. The Runs tab gets "Runs this session" (pick A/B, save a digest).
- Checked: real engine e2e `e2e-engine/compare-runs.spec.ts` (grid, and Manhattan at 23 → 10 dBm: PDR −22 %, AoI +129 %, channel load −18 %, same seed). The CLI gives the same numbers. Digests unchanged (the comparison is read-only). Guide: docs/guides/compare-runs.md.

## 2026-10-13: round-12 integration notes

- **Merged** (`--no-ff`, in order): catalogue sensitivity sweep, agent harness robustness, compare runs. The only conflicts were in this file; both sections were kept each time.
- **Checks:** `cargo check --workspace --tests` is clean. These pass: engine `scenario::`/`publish::`/`guide::` lib tests (27), `sensitivity_catalogue` (3), copilot lib 112 + control 5 + agent 7 + harness_eval 1, server `rpc` 11. Protocol and viewer `tsc -b` pass. The Studio typecheck passes, and vitest passes 288/288.
- **Playwright (mock):** `ui/packages/mock-server/dist` was stale. The agent and metrics specs failed on `/agent/status` 404s until it was rebuilt (`npx tsc -b`). `studio.spec.ts:71` (`feed-row-sent` never shows) also fails on the pre-merge Studio source, so this round did not cause it. It is still open.
- **Release:** `cargo build --release -p v2xw-cli --bins` succeeds. `V2XW_PORT=8758 ./run-sim.sh` boots with the agent available, and `runs.list` answers.
- **Open:** the items listed in the three sections above. These include weather intensity on arterials, phantom-actor settings that the loader accepts, the full live agent suite, and digests that are kept only in memory.

## 2026-10-13: network realism against the reference code (branch of 0e9d662)

- **Sybil on the air.** The attack is no longer refused. Each ghost re-signs the attacker's
  payload under one of its concurrent pseudonyms (`NodeSecurity::sign_as`) with the
  certificate attached, and is queued behind the attacker's own frame. Ghosts are
  registered for ground truth and for the MA.
  - New attacker params: `sybil_ghosts` (6, F2MD) and `sybil_jitter_m` (1 m). Both are
    refused on any other kind.
  - Measured on a 40 s grid: 2,100 ghost frames. `sybilCoLocation` fired 20 times (0 in
    the ConstPos control), and there were 15 reports about the attacker. Before this
    change, `Engine::build` refused the attack.
- **Verify priority.** `pick_by_priority` is now shared: RsuRuntime and VruDeviceRuntime
  order by it as the OBU does, and stay FIFO at priority 0.
  - VRU devices now run `security.verification_policy` (before, they always ran
    verify-all) plus the claim-based relevance used by `on-demand`. VRU checks under
    prioritized: 10,708 near and 14,505 far. Before, every check had priority 0. A
    handset is not loaded, so the waits are equal (0.021/0.019 ms).
  - Engine RSUs still run `ObuRuntime` + verify-all by design.
- **Reference comparisons** (docs/validation/radio-fidelity.md §7):
  - Veins `Decider80211p` error model: equal under constant interference, never more
    pessimistic.
  - Artery `CaService`: 0 mismatches. This needed a fix: the DCC gate is now clamped to
    `T_GenCamMax`, and at a 2 s gate the count went 30 → 60 CAMs.
  - Vanetza: TS 102 687 eq. B.2 regating was added.
  - OpenCV2X SPS: no mechanism differs. Its validation ini uses 15-RB sub-channels and 1
    sub-channel per 190 B packet.
  - 802.11p density: CBR is linear (0.00155 per veh/km), and losses to interference rise
    from 3 % to 12 % of all losses.
- **Output shifts:** Sybil runs (newly runnable). Runs with `prioritized`/`on-demand` and
  VRU devices. Adaptive-DCC runs (B.2). CAM runs whose DCC gate exceeds 1 s.
- **Checks:** engine `scenario::` 27/27, security_lifecycle sybil+refused 2/2, vru
  policy 1/1, node 34, radio dcc 30 + veins, msg generator 22 + artery_cam 1.
- **Open:** no live end-to-end Sybil run in the Studio yet. The sidelink still needs the
  LTE TBS table. Engine RSUs do not use RsuRuntime. The full catalogue sensitivity sweep
  has not been re-run.

## 2026-10-13: traffic realism against SUMO, round 13 (branch of 0e9d662)

- **Hard braking at crosswalks (midtown-peak-load), cause found and fixed.** The cause was pedestrians, but not the kerb step-out test that was ruled out on 10-12. A walker already on one crossing went straight onto the next crossing lane with no traffic check. This happens at a refuge island, or where two crossings meet. In the traced case the walker stepped out 5 m ahead of a car doing 9.3 m/s, and the car stopped inside the junction at -22 m/s². The fix: the walker now holds at the far end of the first crossing while a vehicle is too close to yield on the second (UVC §11-502(b)). While standing there it does not count as occupying the first crossing, for drivers or for the audit. This follows SUMO's `MSPModel_Interacting::blockedAtDist`, where a standing pedestrian outside a vehicle's path does not block it.
  - Results on midtown-peak-load (traffic only, 180 s, `check-traffic`): decel exceedances 15 → 0, red entries 18 → 14, stopped on green 89 → 76, jerk 556 → 491. Long stops (11) and overlaps (14) are unchanged.
  - Results in the traffic validation: safety-class violations midtown-peak-load 269 → 180, midtown-pedestrians 74 → 60, midtown-street-life 92 → 90.
  - Digests change for scenarios with pedestrians.
  - `check-traffic --trace` now also prints the pedestrians on crossings near the traced vehicle.
- **`traffic.turn_curve_window_m`** (new, opt-in; default 0 keeps today's behaviour). SUMO's netconvert caps a turn at sqrt(a·R) using the connector's average radius, so the apex is taken faster than that. Measured on the Midtown grid at a = 5.5, the 85th-percentile peak was 7.7 m/s² (AASHTO allows ≤ 3.73).
  - The setting judges the radius over the tightest arc of the given length. At 6 m the peak equals a: 5.46 at 5.5, 3.52 at 3.5.
  - Cross-validation with the 6 m window: the junction-speed gap to SUMO falls from +9..+14 % to +2..+7 % on the grid. Manhattan moves from +1..+5 % to -2..-6 %, and space-mean speed shifts by up to 2 points. The window is therefore not made the default; that is for the owner to decide.
  - It is published as Basic, and the loader refuses it when `turn_lateral_accel_mps2` is 0.
  - Test: `traffic_audit_config::the_turn_curve_window_reaches_the_turn_speeds`.
- **Permitted-left Raff estimate.** Lags rejected while a pedestrian was on the turn's crosswalk are now excluded, which is field practice: 7.01 → 6.79 s. It still FAILs. The estimate rests on 21 accepted lags, and three long rejections decide it. The decision rule itself uses 4.5 s.
- **Cross-validation:** the native defaults are unchanged, so every traffic row is as before. Grid / Manhattan vs SUMO Krauss: flow -9..+1 %, speed -5..-4 %, GEH ≥ 97 %, paired TT ratio 0.99-1.09.
  - `V2XW_XVAL_TURN_WINDOW_M` is a diagnostic knob.
  - Tooling note: the shared SUMO venv had lost `data/typemap/osmNetconvert.typ.xml`, so Manhattan was silently skipped. It has been restored from the v1_27_1 tag.
- **Open:**
  - Lane changes per vehicle-km on Manhattan are 18-41 % below SUMO (LC2013 speed-gain/keep-right not modelled).
  - Halting time is 26 % below SUMO on Manhattan (Krauss full stops against IDM creep).
  - Stopped-on-green: 76 on midtown-peak-load.
  - Kerbside overlaps from connectors, the in-building corner lane change and actuated detector placement were not reached.
  - Start-up lost time is 3.06 s, outside the HCM's ±1 s band. Greenshields' own headways imply 3.6 s, so the band itself is in question.
- **Checks:** mobility lib 289/289, engine `scenario::` 27/27, `traffic_audit_config` 9/9.

## 2026-10-13: expectation checks across the catalogue (branch of 0e9d662)

- **Table:** 151 single-setting rows over 78 settings (36 before), each with a cited source, in `crates/v2xw-engine/tests/expectations/{catalogue,radio,security-network,traffic}.yaml`. 17 interaction checks in `tests/expectations/interactions/`: 2 × 2 same-seed runs, judged as dampens / amplifies / holds / reverses (`sensitivity/expect.rs`). Rows may carry a magnitude band.
- **Run:**
  - `v2xw sensitivity --expect` plans only what the table names and gives one pass/fail line per row. It takes about 7 min from an empty cache, about 1 min with the cache.
  - `tests/sensitivity_expectations.rs` is the fast part. `-- --ignored` runs the full table.
  - Results are in `docs/validation/sensitivity-expectations.md`.
- **Result:** the first run passed 144/160; the result now is 166/168. Fixes:
  - **Metric fix:** `metric.headway_time` now counts only following pairs: the follower is at or above 5 mi/h and the headway is at most 6 s. Before, its mean was 129 s and it moved the wrong way. Metric samples change wherever safety metrics are on.
  - **Families file:**
    - `rate_veh_per_h_per_lane_km` is a rate, not a density.
    - The ML-DSA context gets a fragmenter; without one, every frame was refused at the MTU.
    - GNSS is judged on the lifecycle family.
    - 20 % trucks/buses/motorcycles for the per-class and NLOSv rows.
    - The NYC preset for kerbside.
    - Real ends for `outlier_rate_hz`.
  - **Planner:** a boolean with no published default is swept both ways. Several "dead" switches were schema-default `true`.
  - **Expectations corrected with evidence:**
    - The streak gate tolerates one-sample GNSS outliers.
    - HDM reaction time does not set the headway.
    - LIMERIC does not throttle below CBR 0.68.
    - Demand effects at X ≪ 1.
- **Still failing:** `weather.intensity` on arterials (owner decision, unchanged), and truck time headway, which is below this family's resolution.
- **Seen:** `gnss.rtk: true` gives 3 false verdicts against 0 on the lifecycle family.
- **Catalogue doc:** `docs/validation/sensitivity-catalogue.md` was regenerated from a cache. Entries carried over from the round-12 cache predate the headway metric fix.

## 2026-10-13: round-13 integration notes

- **Merged** (`--no-ff`, in order): network realism against the reference code, traffic realism against SUMO, sensitivity expectations. The only conflicts were in this file; both sections were kept each time. `cargo fmt --all` is its own commit.
- **Checks:** `cargo check --workspace --tests` is clean (three pre-existing unused-import warnings). These pass: engine `scenario::` 27, `publish::` 12, `guide::` 3; `sensitivity_expectations` 4 (+1 ignored full table), `security_lifecycle` 12, `traffic_audit_config` 9, `vru` 5; `v2xw-msg` `artery_cam` 1. The full `--expect` table, clippy and Studio/Playwright were not run this round.
- **Release:** `cargo build --release -p v2xw-cli --bins` succeeds. `V2XW_PORT=8758 ./run-sim.sh` boots and `/agent/status` reports the agent available.
- **Owner decisions owed:** the crosswalk refuge hold is on by default and changes digests of scenarios with pedestrians on crossings; the 6 m turn curve window (off by default); the `weather.intensity` arterial fix (digest change). Headway metric output changed wherever safety metrics are on.
- **Open:** the items in the three sections above (LTE TBS table, engine RSUs on the OBU runtime, permitted-left gap, lane-change/halting gaps to SUMO, truck headway family, `gnss.rtk` false verdicts).

## 2026-10-13: speed, round 14 (speed track, branch of e2b03c9)

Plain `dev` build (unoptimised, as built under `target/agents` this round, so absolute
times are higher than the 2026-10-12 opt-level-2 table), binaries copied out,
`v2xw run --no-recording`, 2 threads, owner's 8 GB machine shared with other agents
(wall ±10 %). Saturated: `dsrc-dense.yaml` with the "queue 64->1, saturated" base settings,
trip cap 150 (99 vehicles), 1 s. Capped: `scale/base.yaml`, cap 1,260, `radio.range.max_m:
500`, `metrics: [all]`, 2 s (1,049 vehicles).

| run | wall s per sim s before → after | MAC timer events before → after | peak MB before → after |
|---|---|---|---|
| saturated, 99 | 64.7 → 31.0 | 5.34 M → 0.32 M | 83 → 82 |
| capped, 1,049 | 174.2 → 158.3 | 18.7 k (unchanged) | 420 → 416 |

- **MAC timer coalescing** (`f0c987d`, digest change approved): `Engine::schedule_mac_timer`
  drops a timer for a node and channel already queued at that instant (the queued one has
  the lower sequence number, so it is the one that would have dispatched first). Saturated
  99: frames sent 1,833 → 1,951, records 521,796 → 524,843; the capped run is identical
  record for record (it never queued duplicates).
- **Digest off the main thread** (`61504a6`, `a1095a8`): `v2xw_core::hash::ThreadedSha256`
  (256 KiB batches over a channel to one hasher thread, started at the first full batch,
  inline on wasm). Used by the CLI's `Tally` and by `DigestRecorder`; same digest (unit
  test `threaded_sha256_matches_the_serial_digest`).
- **Decode once** (`8fd2a2a`): `NodeSecurity::parse` reuses the last `(profile, bytes)`
  decode per thread, so the receivers of one frame decode its envelope once; the node phase
  decodes `node.tx`/`node.rx` metric views in parallel and hands them over with
  `ProviderSet::on_event_predecoded`.
- **Determinism:** saturated 99 after: stream digest `5083b56a9632` at 1 and 2 threads, identical per-channel counts and JSON bytes.
- **Not done:** record JSON emission is still serial on the main thread. In the capped run
  most of the time is `node.rx`/`phy.rx` encoding (2.7 M records, 2 GB of JSON). The
  saturated run still has 335 k `NodeTask` events, which have not been profiled.

## 2026-10-13: settings correctness, round 14 (branch of e2b03c9)

- **`weather.intensity` on arterials** (owner-approved digest change). The FHWA arterial rows are written by pavement state only, so a drizzle and a downpour gave one number on every city street. Rain, snow and sleet now place the speed and saturation-flow cut linearly across the cited band by intensity (wet 10-25 % speed, 2-21 % flow; snowy 30-40 % / 15-30 %; FHWA Road Weather Management, HCM 7th ed. Exhibit 11-20 ordering). Intensity 0.5 keeps the old midpoint. Freeway rows already split light/heavy and are unchanged; fog stays at the midpoint. Radio rain loss already scaled with intensity (`v2xw-radio` prop.rs).
- **Loader consistency.** Truck/bus/motorcycle driver settings are refused when the fleet has no such class, now including an empty `classes` with the Poisson `cars-only` preset (`kesting2007` has trucks, `legacy-mixed` all three; other demand kinds are given the benefit of the doubt). `radio.devices.rsu` is refused with no `actors.rsus`/`rsu_placement`; `radio.devices.vru` with no pedestrian or cyclist carrying a device. Messages name what to add.
- **Truck-heavy freeway family** (`freeway` in `scenarios/sensitivity/catalogue.yaml`): the six-lane highway at 12,000 veh/h offered, 30 % trucks (FHWA Highway Statistics VM-1 rural Interstate share), 90 s. The truck time-headway row routes there first.
- **`gnss.rtk` false verdicts: cause found and fixed.** The RTK receiver kept the 1.6 m `gpsSensor` white noise, larger than the whole RTK σ, so its fixes scattered like a standard receiver's and its bursts reached ~10 m against a stated 3.9 m radius (3 `positionSpeedInconsistency` verdicts). RTK white noise now scales by the RTK/SPS fitted σ ratio (Reid 2019). Measured on the lifecycle scenario (30 s, no attackers): 3 → 0. Digests change for RTK scenarios only. New expectation row `gnss.rtk → verdicts_fired same`.
- **Clippy:** `bler_nr.rs` digitised 0.318 point allowed past `approx_constant`; `cargo clippy -p v2xw-radio -p v2xw-mobility -p v2xw-threat -p v2xw-engine --no-deps` now has warnings only, no errors.
- **Not measured:** the truck time-headway row on the new freeway family. One `--expect --only` run on a debug build had not finished its first 90 s highway run after 25 min and was stopped. Run it on the release binary: `v2xw sensitivity --expect --only actors.vehicles.drivers.truck.time_headway_s`. If it is too slow, shorten the family to 30 s.
- **Checks:** `sensitivity_expectations` 4/4 (+1 ignored). `cargo clippy` (four crates) shows warnings only. Mobility, engine `scenario::` unit tests, and the full table were not run.

## 2026-10-13: junction and signal stalls (branch of e2b03c9)

- **`traffic.signals.short_link_clearance_m`** (new, opt-in, 10-200 m): for a driving lane shorter than this between two signalised junctions on one cycle, the downstream offset makes the green that empties it lead the upstream green that fills it by 2 s (FHWA STM 2e ch. 6). Keep-clear (`traffic.keep_junctions_clear`, SUMO's MSLink keepClear) already holds a car out of a full link; this removes the cause. Not measured on midtown-peak-load this round.
- **`traffic.signals.detector_placement`** (new, `stop-line` default | `sumo`): SUMO's per-lane loop at speed limit x 2 s upstream extends the green; stop-line presence still calls a red approach. Refused under fixed-time control.
- **Lane changes between non-parallel lanes refused** (> 10 degrees at the vehicle or 15 m ahead): the corner lane change through a building. Grid lanes are parallel, so grid digests hold; imported maps may change.
- **Recommended ranges** for min green (5-15 s) and max green (30-60 s), FHWA STM 2e, so sweeps never cross the other's default.
- **Right turn on red on the grid** is live (150 s, 1500 veh/h: 6.501 vs 6.473 m/s); the catalogue's "dead" verdict came from the thin 90 s family. Test `right_turn_on_red_reaches_the_grid_signals`.
- **Not reached:** kerbside overlaps from connectors; start-up lost time calibration (3.06 s vs HCM 2 s needs a measured re-fit of the green-onset reaction, a digest change).
- **Checks:** `traffic_audit_config` 11/11, engine `scenario::` 27/27.
- **Build hazard:** the shared target dir served a stale `scenario_reflect.rs`; `touch crates/v2xw-engine/build.rs` forces it.

## 2026-10-13: LC2013 lane-change motivations and HCM heavy-vehicle gaps (branch of e2b03c9)

- **`traffic.lc_strategic`, `lc_cooperative`, `lc_speed_gain`, `lc_keep_right`** (new, opt-in). They port SUMO `MSLCM_LC2013.cpp` into `v2xw_mobility::lanechange::lc2013`:
  - **Strategic look-ahead:** a route change is due within lookAheadSpeed · 10 s · lcStrategic (×2 to the left) + 2 · (length + minGap). Further out, the vehicle changes only toward a lane that is no slower.
  - **Speed gain:** an accumulator per side with thresholds of 0.2 / lcSpeedGain on the left and 2 / lcSpeedGain on the right, plus a 20 s remain-time rule.
  - **Keep right:** its own accumulator (`KEEP_RIGHT_TIME` 5 s; acceptance time 7 · v).
  - **Cooperation:** a vehicle blocking a neighbour's urgent strategic change moves aside, unless the inconvenience is above lcCooperative.
  - MOBIL's b_safe criterion still gates every change.
  - Setting any of the four replaces MOBIL's discretionary decision. Unset fields take SUMO's default of 1. With all four unset, nothing changes and digests stay the same.
  - The loader refuses them when `lane_changes` is false, and refuses strategic/cooperative when `strategic_lane_choice` is false. Diagnostic knob: `V2XW_XVAL_LC2013=1`.
- **`traffic.heavy_vehicle_gap_terms`** (new, opt-in). Adds the HCM 6th ed. Exhibit 20-11 terms for one heavy vehicle: t_c,HV is +1.0 s, or +2.0 s with four or more major lanes; t_f,HV is +0.9 / 1.0 s. They apply at unsignalised junctions and on a permitted left.
- **Permitted-left Raff 6.79 s, reviewed and not fixed.** The decision rule judges the opposing car as if it accelerates at 1.4 m/s² up to 11.2 m/s (`time_to_stop_line_launching_s`). The tracker times the car's real arrival, from an ETA at its present speed. A car cruising below the cap is therefore rejected on a predicted lag that is shorter than the one measured. That is a likely source of the long "rejected" lags.
  - SUMO `MSLink::blockedAtTime` instead compares the foe's announced arrival window with the ego's own leave time plus a 1 s minor time gap. Impatience blends in the foe's braking arrival.
  - Aligning the two would change the defaults, so it was not done this round.
- **Cross-validation, Manhattan @1200, quick, same build.** These numbers show LC2013 is not ready as a default:

  | Run | Lane changes per vehicle-km | vs SUMO Krauss 0.67 | Halting time per trip |
  |---|---|---|---|
  | MOBIL baseline | 0.48 | -28 % | 93.8 s |
  | LC2013 on | 0.02 | -97 % | 99.0 s |

  - On the network, LC2013 almost stops lane changes, strategic ones included.
  - On a two-lane ring with no junctions it does overtake: the test `engine::tests::lc2013_overtakes_a_slow_leader` passes, and so do the 2 `lc2013` unit tests.
  - The defect is therefore in the junction-network path. Suspects to check first: `lc2013_strategic_due` gating strategic changes; `lane_route_dist` and the remain-time rule; the offer filter.
  - It is not yet traced. Keep it opt-in.

## 2026-10-13: agent harness finish (branch of e2b03c9)

- **New agent tools** (`crates/v2xw-copilot/src/agent/control.rs`, `src/help.rs`): `engine_runs` (list / compare / digest over `runs.list`, `runs.compare`, `runs.digest`, so "compare with the previous run" reaches any run the engine kept, Studio runs included; digest series dropped); `setting_help` (published row, the guide's explanation, the configuration reference and both parameter inventories' rows, a sensitivity summary); `sensitivity` (by setting or by metric, from the expectation table and the regenerated `--catalogue` / `--expect` reports, compiled in). The instructions route "what does X do / how sensitive" to them.
- **Settings surface**: the agent's catalogue is `publish::fields()` at runtime; a new test asserts it equals the published path set (at least 250) and that the new lane-change, weather, sidelink and turn-window keys repair and validate. The refused attacker list now comes from the engine (`phase2::attack_kinds`; Sybil is no longer listed as refused) and attacker searches list every `params` key with the kind that uses it (`phase2::ATTACKER_PARAMS`). The Sybil Studio label no longer says "not runnable".
- **Live eval** (`docs/validation/agent-harness-live-3.md`, gpt via `.env`, 45 model calls, $0.70): the 4 rescored cases from rounds 1-2 now pass (set-radio-weaker, set-cert-every-message, refuse-power-60, control-pause), and so does control-speed. 6 of the 10 new hard cases pass, so the raw total is 11/15 (73 %). Of the 4 failures, 2 were scoring errors, now fixed: the refusal vocabulary lacked "refuses ... not applied", and a `mentions` check contradicted the one-line setup rule. The other 2 were agent faults, now fixed but not re-run live: a preset whose map is missing blocked the rain change (the setup refusal now says to retry on base `current`), and Sybil ghosts were reported as not settable. With the scorer fixed, the same answers score 13/15.
- **Open**: re-run `hard-storm` and `hard-invalid-ghosts` live; the full `--all` suite (about 110 calls).

## 2026-10-13: round-14 integration notes

- **Merged** (`--no-ff`, in order): speed, settingsfix, netcell, junctions, lanechange, agent, studio. Conflicts: BUILD-STATUS sections (kept all) and `scenario/guide.rs` ADVANCED table (kept the lane-change rows and the `scale.*` rows).
- **Fixed after merge**: `rtk_is_an_order_of_magnitude_better` asserted RTK white noise < 0.2 × SPS, but the fix scales it by the fitted σ ratio (above 0.2); the test now checks that ratio. The copilot agent test expected a `metrics` change, but phase1-grid already records `metrics: [all]`. Studio e2e: the command list needs developer mode, the run is played on before the metrics cards, and the mock now answers `runs.list`; `studio.spec.ts` passes end to end, `feed-row-sent` included.
- **Ran**: `cargo check --workspace --tests`; engine `scenario::`/`publish`/`guide`, `traffic_audit_config`, `sensitivity_expectations` (fast), `security_lifecycle an_mbms_broadcast_delivers_the_crl`, mobility and radio libs, metrics polarity, core hash, copilot, `cli` tests: all pass. Studio typecheck and vitest (288) pass. Release CLI built; `run-sim.sh` boots on 8758 with the agent watching.
- **Open**: 4 Playwright scene-validation tests fail with "the frame never stopped changing" (not investigated; may be load-dependent). Saturated-DSRC pinned digests (speed track) and the full engine suites were not rerun. Not measured: the truck time-headway expectation row and LC2013 on Manhattan (still off by default).

## 2026-10-13: round-15 integration notes (engine settings test tracks)

- **Merged** (`--no-ff`, in order): perf, phy, mac, netmsg, security, traffic, vru, coupling. No conflicts; `validate.rs`, `run.rs`, `sweep.rs` and `catalogue.yaml` auto-merged with every track's blocks kept.
- **Engine changes that move results**: 802.11p carrier sense now waits aCCATime (8 us), so every contended DSRC result changes (more collisions, lower PDR); on-air DENM honours `net.geonet.repetition_*` (defaults unchanged); `v2xw compare` uses Welch's t; new refusals (EDCA on unused queues, compromised RSU without units, GLOSA compliance with GLOSA off, green-wave heading without speed, on-air keep-alive). Fingerprints gain `report.air_by_type`, lane-change and zeroed traffic counters, `metric[dim=value]` slices.
- **Ran**: `cargo check --workspace --tests`; engine `scenario::`/`sensitivity::` lib tests, `networking_track`, `radio_access`, `researcher_config`, `sensitivity_expectations` (fast), `sensitivity_security`, server `compare`: all pass. Release CLI built; `run-sim.sh` boots on 8758 with the agent available.
- **Open**: many corrected expectation rows were never re-run (heavy-lock contention): phy-channel, mac-congestion, security-track, run-cost, coupling Falcon rows; traffic lane-change rows stay commented until re-run on this release build. Catalogue and `--expect` reports need regenerating after the CCA fix. Dead settings listed in each `docs/validation/test-*.md`.

## 2026-10-13: speed, group 4 (speed track, branch of bbbb29a)

Optimised dev build (`CARGO_PROFILE_DEV_OPT_LEVEL=2`, debug assertions on), binaries copied
out of `target/agents`, `v2xw run --no-recording --digest`, owner's 8 GB machine with load
average 7-15 from other agents, so wall times carry **±50 %** noise (an `after` run that
overlapped a cargo test ran single-core; `user ≈ real` in its log). Overlays committed in
`scenarios/scale/cost/`: `f1000-capped500` (cap 1,260, `radio.range.max_m: 500`,
`metrics: [all]`, ~1,050 vehicles), `f520-uncapped` (cap 560, no reach cap), and
`f1000-capped500-abstract` (all three radio tiers abstract).

| run | threads | wall s per sim s before → after | attempts | digest before → after |
|---|---|---|---|---|
| capped 500 m, ~1,050 veh | 2 | 19.5 → 41.8 (after ran beside a cargo test; not comparable) | 2,732,608 | `d2490d16d431` → same |
| uncapped, ~520 veh | 2 | 54.6 (sampled) → 45.4 | 3,075,769 | `447bc6190a45` → same |
| uncapped, ~520 veh | 1 | — → 44.7 (machine-bound: 1 and 2 threads equal) | 3,075,769 | → `447bc6190a45` (same as 2 threads) |
| capped, all abstract | 2 | 26.7 (sampled) | 2,740,167 | `—` |

**Where the time goes** (`sample`, before): uncapped 520, building classification
(`ring_crossing` 15 %, R-tree selection 15 %, `point_in_ring` 9 %, footprint box 3 %) is
about 44 % of busy CPU, matching the "half the CPU" estimate. Capped 1,050 at the abstract
tier, libm `pow` is a third of busy CPU, almost all inside the NIST PER model (ten `pow`
per coded-error call, called per SINR window for the SIGNAL and DATA fields) and the
per-window dBm → mW conversion of every interferer; the medium tier runs the same PHY.

**Fixed (all bit-identical; digests above unchanged):**
- `189075e`, `3f64c31` — building rays: `segment_intersection_t` decides `t, u ∈ [0, 1]`
  without dividing (`unit_quotient`, exact by correct rounding, falls back to the division
  for non-finite operands and possible underflow); `ring_crossing`/`segment_blocked` skip
  `point_in_ring` when no edge straddles the point's y; the R-tree segment selection
  multiplies by a per-segment reciprocal (conservative under the 1 mm envelope pad). Test
  `the_division_free_crossing_test_matches_the_division` (200 k random and end-touching
  cases plus quotient edge cases; it caught a NaN-divisor ordering bug).
- `e8d48f2` — `PerModel::survival_data/survival_signal` return exactly 1 when a bound
  `4·(Σc)·d^k_min/(2b)` shows `Pe < 2^-54` (then `1 − Pe` rounds to 1), skipping the ten
  `pow`; test `the_shortcut_survival_is_the_exact_survival` sweeps every MCS, both presets.
- `e3390bd` — `sinr_windows_with` converts each interferer to mW once per arrival, not once
  per window (k² → k `pow`).
- `6dddff9` — the abstract tier is not cheap, so it is relabelled: the audit's 11 % comes from
  free space with no buildings letting 49 % more links reach (receptions, not the radio
  model, dominate cost). `radio.tiers.propagation`/`mac` abstract words now say what they
  do and point large fleets at `radio.range.max_m`. Labels are not hashed.

**Not done / open:** a quiet-machine before/after table (rerun the three overlays with
`v2xw-before`/`v2xw-after` back to back when load < 3); an abstract PHY that is actually a
table lookup (02-architecture §7) is a model change with new abstract digests, owner's call;
per-link classification cache (8 % for +650 MB in round 12) not retried; 1,000 vehicles in
real time not reached.
