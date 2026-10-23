# SUMO against this engine: what to adopt

Status: research note, 2026-10-22. Read-only study; no engine code was changed.
SUMO source read: `eclipse-sumo/sumo` main at `dc90e947` (2026-10-23), sparse
checkout of `src/microsim`, `src/mesosim`, `src/router`, `src/netbuild`,
`src/utils/{vehicle,emissions}`, `tools/{route,detector,turn-defs}`, `data/emissions`.
Engine side: `crates/v2xw-mobility`, `v2xw-demand`, `v2xw-world`, `v2xw-energy`,
`v2xw-interop`, `v2xw-engine` (`actuated.rs`, `priority.rs`, `safety_events.rs`,
`export.rs`), `docs/validation/{traffic,cross-validation,traffic-calibration}.md`,
`docs/guides/configuration-reference.md`.

## 1. Overview

SUMO (DLR, since 2001) is the reference open microscopic traffic simulator: about 20
car-following models, three lane-change models, a time-window junction model, eight
traffic-light controller families, pedestrians, meso queue model, routers, a large
Python tool set for demand and calibration, and emission/energy models. It is the
traffic half of Veins, Artery, OpenCV2X and F2MD.

The engine already studied SUMO closely: its Krauss, EIDM, W99, ACC/CACC, LC2013,
SL2015, meso queue model, routeSampler-style count matching, duaIterate-style DUE,
Gawron/logit route choice, PHEMlight, HBEFA3 and the Kurczveil battery model all cite
SUMO and were read without copying. The engine is also cross-validated against SUMO
1.27.1 over TraCI (`docs/validation/cross-validation.md`: flow -9 to +1 %, at least
97 % of edges within GEH 5, paired travel-time ratio 0.99-1.09 on the mean).

So the remaining gaps are not the core equations. They are **behaviours SUMO has and the
engine does not**: junction impatience, deliberate rule violations and crashes,
dual-ring NEMA control, emergency-vehicle yielding, grade, crossing-conflict surrogates,
zipper merges, parking, overtaking into the opposing lane, and SUMO-format input and
output. Several of these are exactly what V2X safety applications exist to detect
(RLVW, IMA, EVA, DNPW, work-zone warnings), so they matter more here than in SUMO.

## 2. Licence

`SPDX-License-Identifier: EPL-2.0 OR GPL-2.0-or-later` on every source file (see
`LICENSE`, `NOTICE.md`). EPL-2.0 is a weak, file-level copyleft. A modified EPL file stays
EPL, and EPL code cannot be pasted into an Apache-2.0 file. Rules for this engine:

- **Do not copy SUMO code** into the engine. Re-implement from the published paper or
  the documented algorithm, cite it, and state on the model card that SUMO was read but
  not vendored. That is the engine's existing practice.
- **Parameter values and documented defaults are facts.** They may be used with a
  citation (for example, `time-to-impatience` 180 s or the SSM thresholds below).
- **Coefficient tables** (HBEFA3, PHEMlight `.veh`/`.csv`): the engine already
  transcribes them as data with attribution. For new tables (HBEFA4, PHEMlight5), the
  safer route is to load them at runtime from the user's own SUMO installation, or ship
  them unmodified under EPL in a separate data directory with the notice. The full
  PHEMlight5 vehicle set is licensed by TU Graz and is not in the repository: only
  EU4 samples are.
- **Hygiene finding:** `crates/v2xw-mobility/src/lanechange/lc2013.rs:3` says
  "Ported from SUMO `MSLCM_LC2013.cpp`". Every other module says "read only, never
  vendored". Someone should check that the module is a re-implementation from Erdmann
  (2015) and not a translation, then reword the header. Effort: S.

## 3. Component table

Classification: **ENGINE BETTER**, **EQUIVALENT**, **FRAMEWORK BETTER** (we should
improve), **MISSING** (we should add), **NOT USEFUL**. Effort: S ≤ 2 days, M 3-7 days,
L ≥ 2 weeks.

### 3.1 Car following

| SUMO component (source) | Engine counterpart | Class | Note |
|---|---|---|---|
| Krauss default, `sigma` 0.5, `tau` 1 s, `dawdle2` (`MSCFModel_Krauss.cpp`) | `carfollowing/krauss.rs` | EQUIVALENT | Same equations and defaults. The engine's default IDM `UrbanHcm` discharges queues at field rates; SUMO's Krauss discharges faster than field data (cross-validation §"Why the simulators differ"). |
| KraussOrig1, KraussX | — | NOT USEFUL | Historical variants. |
| **KraussPS** (slope-aware: `aMax − g·sin(slope)`) | none: no car-following model reads grade | **MISSING** | See P6. The engine has a DEM (`v2xw-world/src/dem.rs`) but vehicles ignore it. |
| IDM, EIDM (Salles et al. 2020) | `idm.rs`, `eidm.rs` | EQUIVALENT | The engine's EIDM leaves out action points, Wiener errors and drive-off. `drivers.rs` (HDM errors, reaction time) covers the first two for every model. |
| W99, Wiedemann 74 (`MSCFModel_Wiedemann`) | `w99.rs`; W74 absent | FRAMEWORK BETTER (low) | W74 is VISSIM's urban default. US agencies hold W74 calibrations. |
| ACC, CACC (Milanés & Shladover 2014; Xiao 2017), CC/Plexe | `acc.rs`, `cacc.rs` (Milanés, PATH, Ploeg), `platoon.rs` | ENGINE BETTER | The engine's CACC is fed by real decoded V2X messages with loss, age and association. SUMO reads ground truth. |
| Kerner, PWagner2009, Daniel1, SmartSK, NaSch, Rail | Gipps in engine | NOT USEFUL | Niche or rail. |
| `MSDriverState` (OU errors, awareness, `errorTimeScaleCoefficient` 100, `headwayErrorCoefficient` 0.75) | `drivers.rs` (HDM, Treiber 2006: V_s 0.05, σ_r 0.01, τ̃ 20 s; lognormal parameter spread; reaction delay) | EQUIVALENT, with one gap | The engine has no time-varying awareness or distraction episodes. See P15. |
| `startupDelay`, `maxAccelProfile` / `desAccelProfile` (speed-dependent; SUMO ships them empty) | none | MISSING (low) | Start-up lost time is already 3.06 s against HCM 2 s (`calibration/saturation-flow` FAIL), so `startupDelay` would not help. A speed-dependent acceleration profile would help trucks. Fold it into P6. |
| `actionStepLength` (decision interval) | `drivers.rs` reaction delay | EQUIVALENT | |

### 3.2 Lane changing and lateral movement

| SUMO | Engine | Class | Note |
|---|---|---|---|
| LC2013 (strategic, cooperative, speed-gain, keep-right; `informLeader`/`informFollower`) | `lanechange/lc2013.rs`, MOBIL, strategic layer | EQUIVALENT | `informFollower` was tried and removed for cause (Manhattan lock-up). |
| `lanechange.duration` default **0** (instantaneous change) | MOBIL lateral transition with smoothstep and a heading cap | ENGINE BETTER | SUMO teleports laterally by default. |
| SL2015 sub-lane (`lcPushy`, `lcImpatience`, `lcAssertive`) | `lateral/sl2015.rs`, `SublaneFilter` | EQUIVALENT (subset) | `lcPushy`/`lcImpatience` not carried. Low value. |
| **Opposite-direction overtaking** (`MSLaneChanger::changeOpposite`, `lcOpposite`) | none | **MISSING** | See P13. It is needed for the Do-Not-Pass-Warning application and for rural two-lane roads. |
| `lcSigma` lateral imperfection | none | MISSING (low) | Lane-keeping wander. It matters for the lateral-position checks of misbehaviour detection. |

### 3.3 Junctions

| SUMO | Engine | Class | Note |
|---|---|---|---|
| `MSLink::opened`: foe arrival/leave time windows over internal lanes, `jmTimegapMinor` | `intersection/gap_acceptance.rs` (HCM t_c / t_f, conflict matrix), `two_coloring.rs` | EQUIVALENT | Different method. The engine's is field-grounded (HCM) and deterministic. |
| **Impatience**: `impatience = waiting / --time-to-impatience` (180 s). The foe's arrival time is blended toward its braking arrival time at `impatience × foe decel` (`MSLink::computeFoeArrivalTimeBraking`) | none. The card lists "impatience growth, probabilistic right-of-way violation" under `ignores` | **FRAMEWORK BETTER** | See P1. Permitted left turners are too cautious: Raff critical gap 6.8 s against HCM 4.5 s (`calibration/permitted-left-gap` FAIL). |
| `jmSigmaMinor`, per-driver gap variation | deterministic t_c | FRAMEWORK BETTER | Part of P1. |
| **`jmDriveAfterRedTime`, `jmDriveAfterYellowTime`, `jmIgnoreFoeProb`/`Speed`, `jmIgnoreJunctionFoeProb`, `--collision.action`** | none. Violations are only audited as defects (`audit.rs`), and only scripted hard-brake, breakdown and cut-in exist (`safety_events.rs`) | **MISSING** | See P2. Without it there are no red-light runners for RLVW and no failure-to-yield for IMA. |
| `jmCrossingGap` 10 m, `jmStoplineCrossingGap` (pedestrian clearance) | `vru/crosswalk.rs` yield rule | EQUIVALENT, with an open defect | `cities/manhattan` found a car crossing an occupied crosswalk. SUMO's explicit lateral-gap rule could serve as a reference test. |
| `keepClear`, `jmIgnoreKeepClearTime` | don't-block-the-box (NY VTL §1175, `network.rs`; `actuated.rs`) | EQUIVALENT | |
| **zipper** junction type | imported as plain priority (`v2xw-world/src/sumo.rs:1570`) | **MISSING** | See P10. Lane drops and work zones. |
| allway_stop (`jmAllwayStopWait`), priority_stop, right_before_left, left_before_right, right-on-red | engine handles all-way stops, give-way-to-right and RTOR | EQUIVALENT | |
| Junction shape (`NBNodeShapeComputer`, corner radius) and internal lanes inside it | convex hull of lane ends (`osm.rs` ~4698) | FRAMEWORK BETTER (small) | `cities/berlin` found connectors about 2 m outside the junction polygon. Building the hull over connector points too is enough (P16). |
| `--time-to-teleport` 300 s | `traffic.gridlock_timeout_s` 300 s | EQUIVALENT | |

### 3.4 Traffic lights

| SUMO | Engine | Class | Note |
|---|---|---|---|
| Static programmes, offsets | `signal_fixed_time.rs`, ITE yellow and all-red | EQUIVALENT | |
| Actuated (`max-gap` 3.0 s, `passing-time` 1.9 s, `detector-gap` 2.0 s, `inactive-threshold` 180 s, switching conditions) | `v2xw-engine/src/actuated.rs`: NEMA TS 2 **single-ring** basic actuated, gap-out, extension, max-out, rest-in-green, green wave, SUMO-placed detectors | EQUIVALENT for single ring | |
| **NEMA dual ring** (`NEMAController.cpp`, 1,818 lines: `ring1`/`ring2`, `barrierPhases`, `coordinate-mode`, `fixForceOff`, `minRecall`/`maxRecall`, `controllerType` TS1/TS2) | none | **FRAMEWORK BETTER** | See P3. Dual-ring 8-phase control is the standard US controller. SPaT min/max end times are only realistic on it. |
| Actuated, delay-based and NEMA programmes from `net.xml` | imported as static (`sumo.rs:3404`) | FRAMEWORK BETTER | See P4. |
| Delay-based (Oertel & Wagner 2011; `detector-range` 100 m) | none | NOT USEFUL (low) | Rare in US practice. |
| SOTL, swarm, rail signals and drive ways | none | NOT USEFUL | |
| Emergency preemption | `priority.rs`: NTCIP 1211 SRM-driven extension, truncation and recovery | ENGINE BETTER | Driven by real SRM/SSM messages. |
| GLOSA device (reads the signal directly) | GLOSA application over received SPaT, with compliance share | ENGINE BETTER | |

### 3.5 Pedestrians and VRUs

| SUMO | Engine | Class |
|---|---|---|
| Striping model (Erdmann & Krajzewicz 2014), non-interacting | `vru/social_force.rs` (Helbing & Molnár), `crosswalk.rs`, `midblock.rs` (jaywalking), compliance calibration | ENGINE BETTER |
| JuPedSim coupling (collision-free speed model) | none | NOT USEFUL now. It could serve as an optional cross-check of crowd density at a later stage. |
| Bicycles and scooters as sub-lane vehicles | `classes.rs` (bicycle, scooter, moped), SL2015 | EQUIVALENT |

### 3.6 Routing and assignment

| SUMO | Engine | Class | Note |
|---|---|---|---|
| `MSDevice_Routing`: `period`, `pre-period` 60 s, `adaptation-steps` 180, `adaptation-interval` 1 s, `probability` (equipped share) | `routing/dynamic.rs`, `traffic.reroute_period_s` over the last 3 min of lane speeds | FRAMEWORK BETTER (small) | The engine re-routes **every** vehicle. Real navigation-app use is partial. See P14. |
| Dijkstra / A* / ALT / CH routers | Dijkstra, TDSP (`v2xw-demand/src/tdsp.rs`) | EQUIVALENT | A* and CH are speed, not realism. Add them only if large-city re-routing gets slow. |
| duarouter + duaIterate (Gawron), logit, C-logit | `demand/assign.rs` (converged DUE), `choice/{gawron,logit,msa}.rs` | ENGINE BETTER | Uses a measured convergence criterion instead of a fixed iteration count. |
| marouter (static macroscopic assignment) | `loader/bpr.rs`, `loader/queue.rs` | EQUIVALENT | |

### 3.7 Demand

| SUMO tool | Engine | Class |
|---|---|---|
| randomTrips, od2trips | `demand/poisson.rs`, `demand/od.rs`, `od/gravity.rs`, `od/zones.rs` | EQUIVALENT |
| routeSampler (counts to routes) | `counts/sampler.rs` (also generates the candidate routes) | ENGINE BETTER |
| flowrouter (detector flows to routes, max-flow) | covered by `counts/` | NOT USEFUL |
| activitygen | `purpose.rs` (NCHRP 716 tours and departure profiles), `demand/daily.rs` | EQUIVALENT |
| gtfs2pt / ptlines2flows | `transit/` (GTFS, dwell, mapping) | EQUIVALENT |
| **Calibrator** (`MSCalibrator`, `METriggeredCalibrator`: inserts and removes vehicles online to match per-interval edge flow and speed) | none | **MISSING**. See P11. |
| `.rou.xml`: full `vType` attributes, `vTypeDistribution`, Bernoulli `probability` flows, `departSpeed`/`departLane` keywords | `v2xw-interop/src/routes.rs` reads only `vClass` and `maxSpeed`, makes probability flows deterministic, and sends keywords to lane 0 / position 0 | **FRAMEWORK BETTER**. See P8. |

### 3.8 Vehicles, insertion, parking, transport

| SUMO | Engine | Class | Note |
|---|---|---|---|
| vClass defaults (`SUMOVTypeParameter`), `speedFactor` `normc(1,0.1,0.2,2)` | `classes.rs` (12 classes, R10 §B4), the same speed factor | EQUIVALENT | |
| vTypeDistribution | class mix, per-class model shares, `drivers.rs` lognormal spread | EQUIVALENT | |
| `departSpeed=max/desired`, `departLane=best/free` | inserts at rest | **FRAMEWORK BETTER** | See P9. Starting from rest causes most of the engine's mid-block halting on light grids (cross-validation). |
| **Parking areas, manoeuvring, parking search** (`MSParkingArea`, parking reroute) | `kerbside.rs` (bus, taxi and delivery stops in lane, double parking) | **MISSING** (partly) | See P12. |
| Taxi/DRT dispatch (`MSDevice_Taxi`, greedy and shared dispatch) | taxis as kerbside stops only | NOT USEFUL now | |
| Bluelight device (`reactiondist` 25 m, rescue lane through lateral alignment, passing red) | signal priority only. Civilian traffic does not yield | **MISSING** | See P5. |
| ToC (take-over) device, automation levels | none | MISSING (low) | |
| Friction device | `weather.rs` friction | EQUIVALENT | |
| Rail, ship, tram signalling | out of scope (`classes.rs:74`); street rail via GTFS | NOT USEFUL | |
| Bluetooth sender/receiver device | none | NOT USEFUL | |

### 3.9 Mesoscopic

| SUMO | Engine | Class |
|---|---|---|
| Eissfeldt queue model (`MESegment`, τ_ff/τ_fj/τ_jf/τ_jj), `--meso-junction-control` | `meso/mod.rs`, same model | EQUIVALENT |
| One mode per run | `meso/tiered.rs`: micro inside the radio focus region, meso elsewhere, with identity hand-over | ENGINE BETTER (SUMO cannot do this in one run) |

### 3.10 Energy and emissions

| SUMO | Engine | Class |
|---|---|---|
| HBEFA3 polynomial fit (Krajzewicz 2015) | `hbefa3.rs` (no coasting cut-off, grade from z) | EQUIVALENT |
| **HBEFA4** (`HelpersHBEFA4.cpp`) | none | MISSING (EU scenarios only). See P17. |
| PHEMlight, **PHEMlight5** | `phemlight.rs` (PC_G_EU4, PC_D_EU4) | FRAMEWORK BETTER (small). PHEMlight5 code exists, but its full data set is licensed. |
| Battery (Kurczveil 2014), MMPEVEM | `ev.rs` (Kurczveil, VT-CPEM) | EQUIVALENT |
| no US model | `moves.rs` (US EPA MOVES) | ENGINE BETTER for the US deployment target |
| Harmonoise noise | none | NOT USEFUL |

### 3.11 Outputs, detectors, safety surrogates, calibration

| SUMO | Engine | Class | Note |
|---|---|---|---|
| **FCD, tripinfo, edgeData/laneData, summary, lanechange, queue output** | MCAP plus JSONL, Parquet and Arrow exporters (`export.rs`); no SUMO formats | **MISSING** | See P7. |
| E1/E2/E3 detectors (induction loop, lane area, multi-entry/exit) as outputs | detectors exist only inside the actuated controller; `counts/` compares against real counts | FRAMEWORK BETTER (small) | Part of P7. |
| **SSM device**: TTC (3.0 s), DRAC (3.0), MDRAC (3.4), PET (2.0 s), PPET (2.0 s), BR, SGAP (0.2 m), TGAP (0.5 s), range 50 m, extra time 5 s, with 20+ encounter types (following, merging, crossing, oncoming, collision) and trajectory extrapolation for crossing TTC | `v2xw-metrics/src/safety.rs` and `v2xw-trafficval/src/safety.rs`: same-lane TTC, DRAC, cell PET, SSAM thresholds | **FRAMEWORK BETTER** | See P7b. The engine's TTC is same-lane only, and its own table says so. |
| Calibration tools (`tools/detector/validate.py`, routeSampler) | `calibration.rs`, `v2xw-trafficval` (banded checks against field figures, fault injection) | ENGINE BETTER | SUMO has no field-validation suite. |

## 4. Prioritised list to re-implement

All items are clean-room re-implementations: cite the paper, use SUMO only to check
behaviour, and never copy code. Each should land with a held check in
`v2xw-trafficval`, and per the "checks that cannot fail" rule each check must be shown
to go red under an injected fault.

| # | What exactly | Why it improves realism / accuracy | Effort | How to bring it in |
|---|---|---|---|---|
| **P1** | Gap acceptance with **impatience and driver spread**. Each driver draws t_c from a distribution around the HCM mean (lognormal, cv about 0.15-0.2; check against field data). t_c falls with waiting time, reaching a floor after about 180 s (SUMO `time-to-impatience`). Optionally, the foe's arrival time is taken assuming it brakes at `impatience × b` (SUMO's blend). | Fixes the open `calibration/permitted-left-gap` FAIL (6.8 s against 4.5 s). Field studies find accepted gaps shrink with waiting (Polus, Lazar & Livneh 2003, *J. Transp. Eng.* 129(2); Kittelson & Vandehey 1991). It also removes an unrealistically long left-turn delay that inflates travel times. | M | Extend `intersection/gap_acceptance.rs` with a keyed per-actor draw (the existing `keyed_uniform` pattern keeps determinism). Add waiting time to `ConflictView`. Move "impatience growth" from `card.ignores` to parameters. |
| **P2** | **Stochastic rule violations and crash outcomes.** Red-light running after the red onset (SUMO `jmDriveAfterRedTime`), amber running beyond the dilemma zone, failure to yield (`jmIgnoreFoeProb` above a foe speed), pedestrian non-yield. Collisions are detected and resolved into a crash state (stopped, lane blocked, despawn after clearance). | Real traffic has violators, and V2X safety apps exist for them: RLVW, IMA, pedestrian warnings, crash and secondary-incident DENM/TIM. Today every engine driver is lawful, so these apps can only see scripted events. Violation rates must be set from field studies (for example Retting, Williams & Greene 1998, TRR 1640, red-light running counts; verify before use) and default to **off** for parity. | M | A new `violations` block in `drivers.rs` with per-driver propensity. A `DespawnCause::Collision` and a crash actor state. `audit.rs` must tell an intended violation from a model defect (tag the actor), so the invariants keep catching real bugs. |
| **P3** | **NEMA dual-ring, 8-phase controller**: two rings, barriers, concurrent phases, lead/lag lefts, protected/permissive and flashing-yellow-arrow lefts, min/max recall, ped recall, coordination with force-offs and permissive windows, TS1/TS2. | It is the standard US controller (FHWA Signal Timing Manual 2nd ed., NCHRP Report 812, 2015). SPaT `minEndTime`/`maxEndTime` and the phase sequence a US OBU receives depend on it. Single-ring control cannot show two concurrent through movements with independent gap-outs. | L | Re-implement from the Signal Timing Manual and NEMA TS 2-2016, not from `NEMAController.cpp`. Generalise `actuated.rs` so the plan stays a clock: each ring moves its own phase pointer and barriers synchronise the two. Use SUMO's NEMA parameter names for import parity. |
| **P4** | Import SUMO **actuated / NEMA / delay-based programmes** from `net.xml` (`<tlLogic type=actuated|NEMA>`, phase `minDur`/`maxDur`, `<param>`) instead of flattening them to static. | Cross-validation and imported SUMO scenarios would then run the same controller logic as SUMO. | S after P3 | `v2xw-world/src/sumo.rs` ~952/3404 maps the params onto `SignalControl` / P3's ring model. Delay-based falls back to actuated, with a counted anomaly. |
| **P5** | **Emergency-vehicle yielding**: within a reaction distance (SUMO default 25 m; siren audibility is larger, so calibrate), civilian vehicles pull to the kerb or slow, as US move-over / Uniform Vehicle Code §11-405 requires (rescue lane, Rettungsgasse, for the `urban-de` rules). Emergency vehicles may pass red after slowing. | It completes the emergency-priority scenario. EVA/EVP V2X apps and preemption effects are wrong while traffic ahead of an ambulance behaves as if it is not there. | M | `rules.rs` picks the jurisdiction behaviour. The lateral offset goes through `lateral/` (SUMO uses lateral alignment). Optionally triggered only by a *received* EVA message, which SUMO cannot do: siren-only versus V2X-alerted drivers. |
| **P6** | **Grade-aware, power-limited acceleration** for every car-following model: `a_max(v, θ) = min(a_model, (P/(m·v) − F_roll − F_aero)/m) − g·sin θ` from the class's power and mass, plus speed decay on long upgrades for trucks. | Trucks on hills set capacity on grades (HCM grade PCE tables). Speeds and accelerations then feed the BSM, emissions and misbehaviour plausibility checks. The DEM exists, but motion ignores it. SUMO only has KraussPS's `g·sin θ`. Using power and mass is better than SUMO. | M | A post-model clamp in `engine.rs`. It reuses `v2xw-energy/src/powertrain.rs` road-load terms and `dem.rs` grade. It changes goldens, so put it behind a flag first. |
| **P7** | **SUMO-format exporters**: FCD XML, tripinfo, edgeData/laneData (interval traffic measures), summary, plus E1/E2 virtual detector outputs at named positions. | Engine runs become usable by SUMO's tool chain (routeSampler, plotting, `validate.py`), by Veins/F2MD/VeReMi-style pipelines (FCD is VeReMi's ground-truth format) and by any researcher comparing with SUMO. Interoperability is part of "better than". | S | New exporter ids in `v2xw-engine/src/export.rs`, reading the recording as the existing exporters do. Formats follow SUMO's XSDs (schemas carry no licence issue). |
| **P7b** | **Full SSM**: crossing, merging and oncoming TTC by trajectory extrapolation within 50 m, MDRAC (with reaction time), PPET, encounter-type classification and conflict logs with begin/end. SUMO thresholds are defaults. | Today's TTC is same-lane only (its own doc table says so), and most urban conflicts are crossing or merging. The surrogates are the engine's measure of what V2X apps prevent. | M | Extend `v2xw-metrics/src/safety.rs`. Algorithm references: Gettman et al. 2008 (SSAM, FHWA-HRT-08-051); Mahmud et al. 2017, *IATSS Research* 41(4) (review); Fazekas et al. 2017 (MDRAC); Laureshyn et al. 2010 (extended TTC). |
| **P8** | **Complete `.rou.xml` import**: every vType attribute (`accel`, `decel`, `emergencyDecel`, `sigma`, `tau`, `minGap`, `speedFactor` distribution, `carFollowModel`, `laneChangeModel` and lc/jm params), `vTypeDistribution`, Bernoulli `probability` flows (keep the variance), and the `departSpeed`/`departLane`/`departPos` keywords. | Public calibrated SUMO scenarios (LuST Luxembourg, MoST Monaco, TAPAS Cologne, Bologna) would run with their own driver calibration instead of engine defaults. Random arrivals keep realistic platoon structure. | S-M | `v2xw-interop/src/routes.rs` maps onto `library.rs` model ids and `DriverProfile`. Unknown params are counted, not dropped silently. |
| **P9** | **Insertion at speed** at network boundaries (`departSpeed=desired`, capped by the safe speed to the leader). In-network origins stay at rest. | Vehicles entering from outside the map are already moving in reality. Inserting at rest creates false halts and false "stopped vehicle" BSMs at map edges. | S | Spawn options in `engine.rs::spawn_with_route` decide boundary versus interior. A held check: no halts within the first lane at light demand. |
| **P10** | **Zipper merge** at lane drops and work-zone tapers: alternate entry, merging vehicles adapt to the foe's arrival ahead, and late-merge behaviour. | Work-zone V2X (RSM, TIM lane closures) and lane-drop bottlenecks. Today a zipper is a priority junction, which gives one stream absolute priority. Field: MnDOT zipper-merge evaluations (late merge). | M | A conflict type in `two_coloring.rs` / `gap_acceptance.rs` with alternation, set when importing SUMO `zipper` and when a closure removes a lane. |
| **P11** | **Online calibrators** at count stations: per interval, insert vehicles (from local routes) or remove them so that edge flow and speed match observed data. | Lets a city scenario track real detector time series (PeMS, NYC DOT counts) through the day, which static routeSampler output drifts away from. | M | A new engine stage beside demand, using `counts/` stations. Calibrator actions are logged so their effect stays visible. Literature: SUMO documentation "Calibrator"; the algorithm is simple insertion/removal control. |
| **P12** | **Parking**: on-street parking lanes and off-street areas with capacity; parking-in/out manoeuvres that block the lane (duration distributions); cruising search when the destination area is full. | Downtown realism (manoeuvre delay is an HCM saturation-flow adjustment). Trips end in a parking space instead of vanishing. It also produces realistic parked-car occlusion for perception. | M | `kerbside.rs` already models in-lane stops. Add parking areas from OSM `parking:lane` / `amenity=parking`, and a final leg in the route. |
| **P13** | **Overtaking in the opposing lane** on two-lane two-way roads: gap judgement against oncoming traffic, abort rule, sight distance from the DEM and LOS. | Needed for the Do-Not-Pass Warning (DNPW, a CAMP VSC-A application) and for rural highway realism. SUMO only approximates it (`lcOpposite`). Using `v2xw-world/src/los.rs` sight distance would beat it. | L | A new strategic manoeuvre in the lane-change layer. Acceptance from passing-sight-distance literature (AASHTO Green Book §3.2.4; Harwood et al. NCHRP 605). |
| **P14** | **Rerouting share and information source**: only a fraction of drivers re-route (SUMO `device.rerouting.probability`), and the trigger is either a navigation app (network-wide, delayed) or a received V2X TIM/DENM (local). | Every car re-routing on perfect 3-minute speeds over-damps congestion. Splitting the source lets the engine measure the value of V2X routing information, which SUMO cannot do natively. | S | A `reroute_share` and a `reroute_source` in the scenario. Draw per actor with a keyed RNG. |
| **P15** | Driver **awareness / distraction episodes** (SUMO `MSDriverState` awareness; naturalistic eyes-off-road durations, e.g. SHRP2 NDS) raise perception errors and reaction time for a while. | Gives warnings (FCW, EEBL) a measurable benefit through reaction time. It is also a realistic crash precursor for P2. | M | Extend `drivers.rs` with an awareness process. Off by default. |
| **P16** | Junction polygon includes the connector geometry (as netconvert's shape computation guarantees). | Closes the open `cities/berlin` and `midtown-street-life` outside-junction defects. | S | `osm.rs` convex hull over the lane ends plus connector points. Engine-internal fix, using SUMO's behaviour only as the reference. |
| **P17** | HBEFA4 classes for EU scenarios (Berlin). | HBEFA3 is superseded. US runs use MOVES, so this only matters for EU studies. | S-M | Load from the user's SUMO install at runtime (do not vendor), or as transcribed data with the EPL notice in a separate file. Check HBEFA's own terms. |
| **P18** | Wiedemann 74 car-following. | Lets VISSIM-calibrated US agency parameter sets be reused. Optional. | S-M | A new module in `carfollowing/` plus one registry line (`library.rs`). Source: Wiedemann 1974 and PTV's published thresholds. |

Order of value per effort: **P1, P2, P7, P9, P3, P5, P7b, P6, P8, P14, P16, P10, P11,
P12, P4, P15, P13, P17, P18.**

## 5. What the engine does that SUMO cannot

- **A complete V2X stack in the same process and clock**: the radio (802.11p, LTE-V2X
  Mode 4, NR-V2X Mode 2), the messages, the SCMS and its security, and misbehaviour.
  SUMO needs Veins/Artery/OMNeT++ over TraCI, with step-lock coupling and latency.
- **Applications fed by what was received**: CACC, GLOSA and signal priority run on
  decoded messages with loss and age. SUMO's CACC, GLOSA and bluelight devices read
  ground truth.
- **Sensor truth versus belief**: GNSS error processes (`gnss/gauss_markov.rs`),
  oscillator clocks (`clock/tcxo_ocxo.rs`), perception (`v2xw-engine/src/perception.rs`).
  SUMO has none of these.
- **Micro and meso in one run** with identity hand-over (`meso/tiered.rs`).
- **Field-banded validation with fault injection** (`docs/validation/traffic.md`).
  Every held figure has a source and a fault that turns it red. SUMO's test suite is
  regression only.
- **Lateral lane-change transitions by default.** SUMO's default is an instantaneous
  change.
- **HCM-calibrated urban drivers.** Queue discharge headways match Greenshields and HCM
  figures, where SUMO's default Krauss discharges faster than field data.
- **US-specific models**: MOVES emissions, MUTCD/ITE clearance intervals, jurisdiction
  rule sets (`rules.rs`), NTCIP 1211 priority, don't-block-the-box.
- **Social-force pedestrians** with compliance and jaywalking calibration (better than
  SUMO's striping model).
- **Deterministic keyed randomness** per (seed, actor, step). Results do not depend on
  thread count or on event order.
- **Count-matching demand that generates its own candidate routes**, and DUE that stops
  on a measured convergence criterion.
