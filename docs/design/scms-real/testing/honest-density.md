# Honest reports at full density: braking calibration, the speed snap and lane bends (2026-10-24)

Package: 06 §9 "Fix first" items 1 and 3 (§6 items 1 and 3). Base `e15bfec1`. "main" is the
integrator's release binary of `e15bfec1`; "b1" is an opt-level-2 debug build of this branch
(`0588ae6c`). Every run went through the run lock under the 2.5 GB cap. The window is the
quarter fleet (`real-world/midtown-scms-benchmark-quarter.yaml`, seed `0xBE7C_0001`) cut to
its live 0–60 s with no fast-forward, `--record-level events`. The census script reads the
`node.tx` BSMs (claimed speed and heading) and the run report; it is in the session
scratchpad (`hd-wf6b/brk.py`), not committed.

## 1. What the real world brakes at (calibration target)

**Source: Dingus et al. 2006, *The 100-Car Naturalistic Driving Study, Phase II – Results
of the 100-Car Field Experiment*, NHTSA DOT HS 810 593 (VTTI).** Read from the report's
text (rosap.ntl.bts.gov, dot 37370):

- about 2,000,000 vehicle miles and almost 43,000 hours of driving by 241 drivers
  (executive summary);
- 69 crashes, 761 near-crashes and 8,295 incidents fully reduced (§ "Goal 1");
- triggers (Table 2.3): longitudinal acceleration or deceleration of at least 0.6 g, or at
  least 0.5 g with a forward TTC of 4 s or less; the final longitudinal trigger
  (Table 13.9) was a deceleration below −0.52 g at any single sample, which found 44.7 % of
  the valid events, and 66.4 % of the events it found were not safety-relevant ("invalid").

What that gives per vehicle-hour (this page's arithmetic):

| Event | Count in 43,000 h | Per vehicle-hour |
|---|---|---|
| Any single-sample deceleration ≤ −0.52 g (valid + invalid, incl. sensor spikes) | 0.447 × 9,125 / (1 − 0.664) ≈ 12,100 | **≤ 0.28** |
| …of which safety-relevant | ≈ 4,080 | ≈ 0.095 |
| Crash or near-crash (where a driver brakes at the most the tyres give) | 830 | **0.019** |

SHRP 2 (about 35 million vehicle miles, more than a million hours, 1,541 crashes and 2,705
near-crashes, as the FHWA/TRB overview states it; not re-read here) gives 0.004 crash or
near-crash events per vehicle-hour, a lower rate than 100-Car's. The engine target is
therefore: decelerations of 0.52 g and more at most about 0.3 per vehicle-hour, and stops at
the US MaxAcceleration threshold (10 m/s², about 1 g) no more often than crashes and
near-crashes, 0.004–0.019 per vehicle-hour.

## 2. What the engine braked at

The census reads every BSM's claimed speed: a "hard-braking event" is a speed drop over
0.5 s at or above the threshold, merged per sender within 3 s; vehicle-hours are BSM
vehicle-hours (first to last BSM of each sender). The receiver noise is in the claimed
speed, so the 0.5 s window keeps the 0.1 m/s Doppler noise to about 0.03 g.

**Expected before the after-run (written 13:41, before b1 ran):** the snap fix removes the
quarter window's only MaxAcceleration event, so `implausibleAcceleration` 11 → 0–2 and the
honest reports 13 → 0–4 (11.3 → under 4 per vehicle-hour); `positionJump` 2 → 0–2 (the
bend rule acts on through lanes only); hard braking at 0.45 g and more stays at 0–1 events;
wall cost within ±15 % of main's (the floor adds one test per vehicle and step; b1 is an
opt-level-2 debug build, main a release build).

| Quarter fleet, 0–60 s live (1.15 BSM vehicle-hours, 141 senders) | main | b1, rules on |
|---|---|---|
| decelerations ≥ 0.3 g over 0.5 s | 13 events (11.3 /veh-h) | 12 (10.4 /veh-h) |
| decelerations ≥ 0.45 g / 0.52 g / 0.6 g / 0.8 g | 0 / 0 / 0 / 0 | 0 / 0 / 0 / 0 |
| BSM pairs over 10 m/s² (what MaxAcceleration sees, before receiver noise) | 1 (node 262, 1.10 → 0.00 m/s) | **0** |
| lateral `v·Δh/Δt` between consecutive BSMs: p99, p99.9, max | 4.86, 6.20, 72.7 m/s² | 4.78, 5.87, 27.3 m/s² |
| …pairs over 8 / over 12 m/s² (of 41,561) | 17 / 11 | **5 / 3** |
| verdicts | 13 (implausibleAcceleration 11, positionJump 2) | **0** |
| honest reports | 13 (**11.3 per vehicle-hour**), 2 honest vehicles reported, 0 cases | **0** (0 per vehicle-hour), 0 cases |
| wall, peak | 89.9 s (release), 182 MB | 112.4 s (opt-2 debug), 183 MB |

- **Against the target.** The engine's honest traffic at quarter density has no
  deceleration of 0.45 g or more in 1.15 vehicle-hours, inside 100-Car's ≤ 0.28 per
  vehicle-hour at 0.52 g (at this exposure 100-Car would expect 0–1). Its one MaxAcceleration
  event was the snap (§3), an engine artefact, now gone; the honest report rate falls from
  11.3 to 0 per vehicle-hour. The verdicts matched the expectation (0–2); the reports too
  (0–4).
- **Full density is not re-measured.** memden's 12 sender events over 10 m/s² in about 25
  vehicle-hours (0.48 per vehicle-hour, 25× 100-Car's crash and near-crash rate) include
  node 874's snap, now explained; sender 720's 9.6 m/s² stop at 7.9 m/s is real emergency
  braking whose cause was not traced. The 100 s full-fleet window that would show it costs
  about 40 min of wall time on main and was not run in this package (§5). Until then the
  full-fleet count stays the engine's, as 06 §6 item 1 says.
- **Same binary, rules off (b1 on the `e15bfec1` scenario):** 13 verdicts
  (implausibleAcceleration 11, positionJump 2), 13 honest reports, the same node-262 snap at
  46.31 s and the same braking census as main: the flags off change nothing. Wall 114.1 s
  off against 112.4 s on (−1.5 %), peak 183 MB both: the two rules cost nothing measurable.
  (Main's 89.9 s is a release build and does not compare.)

## 3. The node-874 speed snap: the standstill-gap advance cap (fixed, flag)

- **Root cause.** On main, the quarter window's only BSM pair over 10 m/s² (sender node 262,
  1.10 → 0.00 m/s in 0.1 s at 46.31 s) is the whole of its `implausibleAcceleration`
  volume: 11 receivers each judged it. A `--record-level full` re-run (same seed, same BSMs)
  gives the ground truth: actor 651, a delivery van on lane 3683, braking at −0.6 to
  −0.75 m/s² from 1.57 m/s, then 0.93 → 0.08 m/s in one step (−8.5 m/s²) with 8 mm of
  room left. That is `CAP_MAX_DECEL_MPS2`, the standstill-gap advance cap: the IDM's braking
  scales with its `a` (1.3 m/s² for a van), so the van reached its standstill gap still
  moving and the cap stopped it, harder than the class's own 7 m/s² emergency limit. The
  receiver's 0.3 m/s static hold then reported the 0.08 m/s as 0.00. It is the same
  signature as memden §2.3's node 874 (1.48, 1.39, 1.23, then 0.00 m/s 100 ms later), so
  that snap is this mechanism, not the receiver alone. Not a property of the US rules: a
  real driver creeping up on a stopped car at 1 m/s stops in about 10 cm at 2–5 m/s².
- **Fix (`traffic.standing_leader_stop`, off by default, the benchmark sets it).** A driver
  closing on a standing vehicle (or, for a heavy vehicle, one crawling under 1 m/s) brakes
  at no less than `v²/(2·(s − s0))`, the constant deceleration that stops it at its
  standstill gap, as the stop line already does (`static_obstacle_accel`, SUMO's
  `maximumSafeStopSpeed`); it binds from 0.3 m/s² and never past the class's emergency
  deceleration. The advance cap stays as the backstop.
- **Test:** `engine::tests::a_van_behind_a_standing_car_stops_at_its_gap_without_a_one_step_stop`
  (v2xw-mobility): a van at 1.5 m/s, 1.5 m short of its gap behind a stopped car, brakes at
  under 2.5 m/s² and stands within 5 cm of `s0`. With the rule off the hardest step is
  3.8 m/s² and the assertion is red.

## 4. Lane polyline bends (fixed, flag)

- **Rule (`traffic.lane_bend_speed`, off by default, the benchmark sets it).** Every interior
  vertex of a motorised through lane gets the junction connectors' apex rule: the heading
  change summed over the next `turn_curve_window_m` (6 m) of polyline gives `R = 6 m / Σθ`,
  driven at `sqrt(5.5 m/s² · R)` at most when that is under the lane's limit. The vehicle
  brakes for it at the constant deceleration that reaches the bend's speed at its first
  vertex (at most a planned stop's 3.4 m/s²), and the bend's speed also lowers its desired
  speed (`anticipated_speed`). A rule on the speed, not rounded geometry: rounding in the
  importer would move every world hash.
- Why the floor: with only the lowered desired speed, the IDM's free term tracks it from
  above by about `b / 4a`, and a 6.5 m/s kink was taken at 9.0 m/s.
- **Test:** `engine::tests::a_kink_in_a_through_lane_is_driven_at_its_turn_speed`: a square
  ring with a 45° vertex in every lane; at the vertex the car is at most the bend speed
  (6.48 m/s) + 0.3. Red without the rule (over 12 m/s) and red with the desired-speed part
  alone (9.0 m/s).

## 5. constantPositionFrozen in gridlock (not sampled)

**Not done: the sample of 20 needs gridlock, and gridlock needs the full fleet past about
160 s.** A full-fleet 0–260 s events run on main reached 40 s in 604 s of wall time
(21 wall-s per simulated s and rising with the fill; FB3's first 240 s took 8,334 s), so
the 260 s window would have taken over two hours. It was stopped, and no verdict was
sampled one by one. The quarter window has none (0 in 60 s).

What the code says (a hypothesis to check on that sample, not a finding):
- `ConstantPosition` fires when two consecutive verified BSMs are under 1 cm apart and both
  claim at least 0.02 m/s. The 0.3 m/s static hold reports zero speed for a standing car, so
  a standing car can fire only on an epoch whose raw Doppler speed is over 0.3 m/s.
- With the engine's velocity noise (σ = 0.2 / 1.96 = 0.10 m/s per axis, the GPS SPS
  standard's 95 % bound), a standing receiver's |v| passes 0.3 m/s on about 1.1 % of epochs.
  Two such epochs in a row are released from the hold; the smoothed position moves by the
  mean of the two noise velocities times 0.1 s, under 1 cm when they nearly cancel (about one
  pair in ten). That is about 0.4 sender events per standing vehicle-hour, roughly 0.2 a
  second with 1,500 vehicles standing, each judged by about 25 receivers: the order of
  FB3's 567 verdicts in gridlock.
- If the sample confirms it, the artefact is the velocity noise: an automotive receiver's
  Doppler noise is a few cm/s, at which a standing car never leaves the hold. The fix would
  then be a calibrated `sigma_velocity_mps` (a new `actors.vehicles.gnss` key), not a
  change to the rule.

## 6. The cost of the rollover window and of each fast-forward stop

Profiled on the quarter file end to end (`QD`, main, `--record-level metrics`) with macOS
`sample` (1 ms) attached for 8 s in the rollover slice at 43,200 s and for 8 s in the
second fast-forward. The full fleet was not profiled (its first window alone is over two
hours); the quarter shows the same shape as FB3 (rollover twice the first window's cost per
simulated second: 3.3 against 1.7 wall-s here, 79 against 35 in FB3).

| QD on main | wall |
|---|---|
| first window 0–60 s | 101.8 s (1.70 per simulated s) |
| first fast-forward 60–43,185 s | 0.8 s |
| rollover 43,185–43,215 s | 100.3 s (3.34 per simulated s); the 43,200–43,205 s slice 35.7 s against 10.6–14.2 s for its neighbours |
| second fast-forward | 20.6 s (11,393 stops in the run: crl-poll 11,239, max-jump 144, kernel 8) |
| whole run | 225.8 s, peak 240 MB |

**The rollover is the weekly pre-generation, and it is real cryptography on one thread.**
In the 43,200 s slice the main thread spends all of its samples in `ScmsRun::step` →
`handle_provision`: the ACA issuing (24 %), the RA opening ACA responses (21 %), signed-PDU
opening and verification, ECIES encryption (12 %) and `ScmsPki::signer_key` (12 %), almost
all of it P-256 field arithmetic; the rayon workers wait. 278 top-ups, 5,560 certificates at
quarter; FB3's 1,800 top-ups and 35,460 certificates are 6.4× the work, in one burst. That
cost is the model doing what the RA does (each certificate really issued, encrypted and
opened), so it is not removed:
- **Fixed:** `signer_key` re-verified the same explicit ACA certificate under the same ICA
  for every response the RA opened. The check is now remembered per pair of
  whole-certificate hashes (`419b8e6b`, test
  `a_component_certificate_is_verified_once_and_a_forgery_is_still_refused`): about 12 % of
  the burst, no outcome changed.
- **Open:** spreading the pre-generation as a real RA does moves the work but does not
  shrink it; what would shrink the wall time is running each device's provisioning on the
  rayon pool, merged in device order (the kernel handles one delivery at a time today).

**Each fast-forward stop was dominated by the backend view, not by the wake scan.** In the
second fast-forward, `on_backend_step` is 75 % of the main thread and `Phase2::backend_view`
45 % of that; the hottest frame is `view::ops_of` (2,527 of 5,723 main-thread samples). The
view's `ee` pool called `ops_of` once per device, and `ops_of` filtered the kernel's whole
`(node, primitive, kind)` count map each time: devices × every device's counts at every view,
which is O(fleet²) per stop and why the stop cost grew faster than the fleet (3.5 ms at 327
nodes, 29 ms at 1,945). `next_wake_cause`, the per-node wake scan ff §5 suspected, is 37
samples (0.6 %).
- **Fixed (`a66aa58c`):** `ops_of` reads `node`'s key range of the map (it is keyed by node
  first): the same entries in the same order, so no record or digest changes. Test
  `ops_of_reads_one_node_and_not_every_nodes_counts`: 20,000 devices, 0.03 s; with the old
  full scan restored, 9.6 s, red against its 2 s bound.
- The remaining per-stop work is linear in the fleet (`next_wake_cause`,
  `crl_polls_due`, the outbox and top-up scans); an event index for those is not worth its
  risk until a profile shows them.

**Not re-measured.** The build with both cost fixes (and the memo's fault proof: bypassed,
its test must go red) was queued behind another package's engine test run on the machine's
one build lock and had not run when this package stopped. Owed, on one opt-level-2 build of
`419b8e6b`: the memo fault proof, then `QD` on the `e15bfec1` scenario against the table
above (expected: the second fast-forward 20.6 s → 8–12 s, the rollover window 100 s → about
90 s, every count of the run report identical), then the full-fleet second fast-forward
(FB3: 2,014 s at 29 ms a stop).
