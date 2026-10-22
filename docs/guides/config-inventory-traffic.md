# Traffic, world and run parameter inventory

The traffic audit of 2026-10-11 went through `crates/v2xw-mobility`, `crates/v2xw-demand`
and the world's signal synthesis. It looked for hard-coded numbers and for model fields
that no scenario could reach. Each one is either **exposed**, as a validated scenario field
the Studio settings window and the agent pick up from `settings.describe`, or **kept**,
with the reason. Every exposed field defaults to the value the engine used before, and the
default serialises as absent, so no existing scenario digest moves. The loader refuses
combinations where a field would do nothing (see `traffic_audit_checks` in
`crates/v2xw-engine/src/scenario/validate.rs`).

Tests: `crates/v2xw-engine/tests/traffic_audit_config.rs` runs one same-seed test per
group (driver speed and reaction, per-class drivers, junctions, kerbside, GNSS, signal
timing), plus the loader refusals and the digest check. With the wiring removed, all six
group tests fail.

## Exposed by this audit

| Scenario path | Default | Range | What it is and the source |
|---|---|---|---|
| `actors.vehicles.drivers.speed_factor` | 1.0 | 0.5-1.5 | Mean free speed / limit (SUMO `speedFactor`); `DriverTraits::draw_law` |
| `actors.vehicles.drivers.speed_spread` | 1 | 0-3 | Multiplier on class `speedDev` (SUMO: 0.1 car, 0.05 truck/bus) |
| `actors.vehicles.drivers.start_reaction_s` | 1.08 s | 0.3-3 s | Median reaction to a green (Bonneson 1992, TRR 1365: 1.22 s mean; was Taoka 1989's amber-onset 1.15 s) |
| `actors.vehicles.drivers.queue_start_delay_s` | 0.5 s | 0.05-3 s | Median queue start-up delay (calibrated to Greenshields 1947 headways); sets saturation flow |
| `actors.vehicles.drivers.{truck,bus,motorcycle,bicycle}.{max_accel_mps2,comfort_decel_mps2,time_headway_s,min_gap_m}` | class's own | as passenger rows | Per-class IDM a, b, T, s0 over the model calibration (SUMO vTypes; Treiber & Kesting 2013 Table 11.2); truck = truck+trailer, bus = bus+coach, motorcycle = motorcycle+moped |
| `traffic.critical_gap_factor` | 1 | 0.5-2 | Scales the HCM 6th ed. ch. 20 critical gaps and follow-up times (unsignalised junctions) |
| `traffic.permitted_left_gap_s` | 4.5 s | 2.5-8 s | Permitted left-turn critical headway (HCM 6th ed. ch. 31) |
| `traffic.turn_lateral_accel_mps2` | 5.5 m/s² | 0-10 | Turn-speed cap sqrt(a·R) (SUMO `--junctions.limit-turn-speed`) |
| `traffic.turn_curve_window_m` | 0 (average radius) | 0-30 m | Radius over the tightest arc of this length; 6 m suggested (`DEFAULT_TURN_CURVE_WINDOW_M`, this crate's choice) |
| `traffic.kerbside.enabled` | on | bool | Kerbside stops on/off |
| `traffic.kerbside.bus_stop_spacing_m` | 260 m | 100-2000 m | Bus stop spacing (TCRP Report 19) |
| `traffic.kerbside.bus_dwell_mean_s` | 60 s | 5-300 s | Mean bus dwell (TCQSM 3rd ed. ch. 6) |
| `traffic.kerbside.taxi_share` | jurisdiction (0; 0.3 NYC) | 0-1 | Share of cars making a taxi stop |
| `traffic.kerbside.taxi_dwell_median_s` | 30 s | 5-600 s | Median taxi dwell |
| `traffic.kerbside.delivery_stop_share` | 0.6 | 0-1 | Share of vans/trucks making a delivery stop |
| `traffic.kerbside.delivery_dwell_median_s` | 600 s | 30-3600 s | Median delivery dwell |
| `world.signals.cycle_s` | 90 s | 30-180 s | Synthesised plan cycle (netconvert `--tls.cycle.time`; FHWA STM ch. 5); OSM worlds only |
| `world.signals.all_red_s` | 0 s | 0-6 s | All-red floor (netconvert `--tls.allred.time`; MUTCD §4D.26 max 6 s) |
| `world.signals.pedestrian_walk_s` | 7 s | 4-30 s | Walk interval (MUTCD 2009 §4E.06) |
| `world.signals.pedestrian_speed_mps` | 1.067 m/s | 0.8-1.5 m/s | Clearance walking speed (MUTCD 2009 §4E.06, 3.5 ft/s) |
| `world.source.params.ew_green_share` | 0.5 | 0.1-0.9 | Procedural grids: share of the green given to the east-west street (Webster 1958; HCM 6th ed. ch. 19, split by critical flow ratio) |
| `actors.vehicles.gnss.rtk` | off | bool | Receiver grade: Reid 2019 standard vs RTK percentile rows |
| `actors.vehicles.gnss.outlier_rate_hz` | 0.01/s | 0-0.5 | Outlier rate (legacy engine draw, 04-models.md §3.8) |
| `actors.vehicles.gnss.outlier_magnitude_m` | 12 m | 1-200 m | Outlier size |
| `actors.vehicles.gnss.burst_rate_hz` | 0.006/s | 0-0.5 | Degraded-burst rate |
| `actors.vehicles.gnss.outage_probability` | 0 | 0-1 | Probability each fix is lost |
| `actors.vehicles.gnss.smoothing_tau_s` | 0 (raw fixes) | 0-600 s | Receiver's Doppler-aided position smoothing (Hatch 1982; RTCA DO-229 100 s); the US real-world presets set 100 s (R-USFP) |
| `actors.vehicles.gnss.static_hold_mps` | 0 (off) | 0-1.5 m/s | Receiver static hold: below it, position held and speed 0 (u-blox M8 Receiver Description); presets 0.3 m/s; needs the smoothing |

Limits:

- The meso tier draws its drivers with the default law, so it ignores the speed and
  reaction fields. Micro vehicles in a tiered run read them.
- RTK lowers only the correlated part of the error. The 1.6 m white share stays, so the
  mean error falls by about 40 %, not to RTK's 0.26 m. To make RTK precise, the white share
  needs a figure per receiver grade.
- The right-on-red gap is not scaled by `critical_gap_factor`.
- `gap_factor` is not on the gap-acceptance model card yet.

## Already exposed before this audit

| Parameter | Scenario path |
|---|---|
| Demand level, fleet size, OD, time-of-day profile | `actors.vehicles.demand.{rate_veh_per_h,rate_veh_per_h_per_lane_km,fleet_size,kind,params}` |
| Vehicle mix | `actors.vehicles.classes.*.fraction` |
| Equipped fraction | `actors.vehicles.equipped_fraction`, `actors.vru.device_fraction` |
| Car-following model and its card parameters (IDM, EIDM, Krauss, Gipps, W99, ACC, CACC) | `actors.vehicles.driving.car_following` |
| Sub-lane / SL2015 lateral model | `actors.vehicles.driving.lateral` |
| Passenger IDM a, b, T, s0 | `actors.vehicles.drivers.{max_accel_mps2,comfort_decel_mps2,time_headway_s,min_gap_m}` |
| MOBIL politeness and threshold (all classes) | `actors.vehicles.drivers.{politeness,lane_change_threshold_mps2}` |
| Driver heterogeneity preset, spread, reaction, perception | `actors.vehicles.drivers.{realism,parameter_spread,reaction_time_s,perception_error}` |
| Truck lane restriction, motorcycle filtering | `actors.vehicles.drivers.{truck_lane_restriction,motorcycle_filtering}` |
| Right turn on red, rerouting, lane changes, strategic lane choice, keep junctions clear, gridlock teleport | `traffic.*` |
| Actuated signals: min/max green, passage (gap-out), detector, green wave | `traffic.signals.*` |
| Signal coordination (OSM) | `world.signals.{coordinate,progression_speed_mps}` |
| Pedestrians: count, behaviour preset, mid-block rate, red crossing share, gap margin; cyclists, e-bike share | `actors.vru.*` |
| Weather kind, intensity, visibility, surface (drives FHWA speed/headway effects) | `weather.*` |
| Meso / tiered city scale | `scale.traffic.*` |
| Duration, warm-up, seed, replications, metric period, mobility step | `time.*`, `seed`, `experiment.*` |
| Metric selection, exporters | `metrics`, `exporters` |

## Model structs and settings not exposed

| Parameter | Where | Verdict |
|---|---|---|
| `TwoWheelerParams` lean angle, lane position | mobility/engine.rs | kept because they are rendering/lateral geometry; motorcycle a, b, T, s0 are exposed via `drivers.motorcycle` |
| `CyclistSpeeds` conventional/e-bike speeds, spread, shared-lane cost | mobility/engine.rs | kept because they are field-measured (Copenhagen/Netherlands speed studies cited on the struct); e-bike share is exposed |
| `SocialForceParams` (Helbing 1995 forces, field of view, age-group walking speeds) | mobility/vru/social_force.rs | kept because they are model internals or field fits; behaviour preset and compliance numbers are exposed |
| `SignalDefaults` yellow decel/reaction/range, ITE red clearance, head height, split by lanes | world/osm.rs | kept because ITE/MUTCD formulas fix them; cycle, all-red and pedestrian timing are exposed |
| `SignalPlanParams` defaults for procedural/legacy plans | mobility/intersection/signal_fixed_time.rs | kept because procedural grids take `world.source.params.cycle_s`/`amber_s` and OSM plans come from `SignalDefaults` |
| `GapAcceptanceParams` stop-line offset, queue follow-up, legacy fallback, standing speed | mobility/intersection/gap_acceptance.rs | kept because they are geometry and model switches; the gap scale is exposed |
| Right-on-red critical gap (HCM minor right, 6.2/6.9 s) | mobility/intersection/signal_fixed_time.rs | kept because it is the HCM table value; not scaled by `traffic.critical_gap_factor` (stated in its note) |
| Weather effect coefficients (FHWA speed/headway reductions, sight distance) | mobility/weather.rs | kept because they are the FHWA tables (`fhwa_parameters` publishes them on the card); weather inputs are exposed |
| GNSS bias correlation, vertical sigma, burst factor/duration, velocity sigma | mobility/gnss/gauss_markov.rs | kept because they are card constants tied to the measured fits |
| Meso tier internals (cell length, depart delay) | mobility/meso/ | kept because the meso drivers draw default traits (they do not read the new speed/reaction fields) and the tier is chosen in `scale.traffic` |
| Insertion gap, lookahead, max lifetime | mobility/engine.rs `EngineParams` | kept because they are numerical insertion/neighbour-query settings |
| Recorder level | CLI `--record-level` | kept because it is a CLI run option, not a scenario field |

## Every named numeric constant in `v2xw-mobility` and `v2xw-demand`

Extracted with `grep 'const NAME: f64|u32|u64|usize|Duration'` from the sources at this
commit.

| Name | Where | Value | Verdict |
|---|---|---|---|
| `FREEWAY_MIN_LIMIT_MPS` | mobility/weather.rs:67 | `22.352` | kept: the FHWA freeway/arterial split (50 mph), a definition |
| `G_MPS2` | mobility/weather.rs:105 | `9.806_65` | kept: physical constant |
| `T_S` | mobility/weather.rs:113 | `2.5` | kept: AASHTO stopping-sight-distance perception-reaction time (2.5 s) in the visibility speed limit; visibility itself is `weather.visibility_m` |
| `A_MPS2` | mobility/weather.rs:114 | `3.4` | kept: AASHTO stopping-sight-distance deceleration (3.4 m/s²) in the visibility speed limit |
| `HEAVY_INTENSITY_THRESHOLD` | mobility/weather.rs:129 | `0.5` | kept: a weather-intensity breakpoint; intensity itself is `weather.intensity` |
| `AT_STOP_M` | mobility/kerbside.rs:70 | `1.5` | kept: kerbside stop geometry |
| `OVERRUN_M` | mobility/kerbside.rs:73 | `1.5` | kept: kerbside stop geometry |
| `RETRY_M` | mobility/kerbside.rs:76 | `40.0` | kept: kerbside retry distance |
| `LOOK_LANES` | mobility/kerbside.rs:79 | `5` | kept: kerbside search depth |
| `PASS_ROOM_M` | mobility/kerbside.rs:84 | `10.0` | kept: kerbside passing geometry |
| `MIDTOWN_TAXI_SHARE` | mobility/kerbside.rs:164 | `0.3` | exposed as `traffic.kerbside.taxi_share` (new; the Midtown preset value) |
| `UNDERGROUND_Z_M` | mobility/audit.rs:77 | `3.0` | kept because it is a check or calibration reference, not a model input |
| `PED_CONFLICT_TTC_S` | mobility/audit.rs:84 | `3.0` | kept because it is a check or calibration reference, not a model input |
| `TTC_NEAR_MISS_S` | mobility/audit.rs:89 | `1.5` | kept because it is a check or calibration reference, not a model input |
| `RIGHT_ON_RED_STOOD_M` | mobility/audit.rs:377 | `6.0` | kept because it is a check or calibration reference, not a model input |
| `HDM_DISTANCE_ERROR` | mobility/drivers.rs:70 | `0.05` | exposed as `actors.vehicles.drivers.perception_error` (multiplier) |
| `HDM_RATE_ERROR_PER_S` | mobility/drivers.rs:72 | `0.01` | exposed as `actors.vehicles.drivers.perception_error` (multiplier) |
| `HDM_ERROR_CORRELATION_S` | mobility/drivers.rs:74 | `20.0` | kept: a published HDM constant (Treiber 2006); its magnitude is exposed via perception_error |
| `HUMAN_REACTION_S` | mobility/drivers.rs:77 | `0.8` | exposed as `actors.vehicles.drivers.reaction_time_s` (the `human` preset value) |
| `MAX_REACTION_S` | mobility/drivers.rs:80 | `2.0` | kept: a guard on the exposed reaction time |
| `STYLE_CUT_Z` | mobility/style.rs:93 | `1.036_433` | kept: the tercile cut of the style split, a definition |
| `HEAVY_TURN_SPEED_MPS` | mobility/style.rs:173 | `10.0 * 0.447_04` | kept: AASHTO design turning speed for heavy vehicles |
| `INTENT_HORIZON_M` | mobility/views.rs:622 | `150.0` | kept: how far ahead a published intent looks; affects messages, not driving |
| `EMERGENCY_BRAKE_ONSET_JERK_MPS3` | mobility/views.rs:715 | `25.0` | kept: jerk limit on emergency-brake onset, a vehicle-dynamics bound |
| `PAR_MIN_ITEMS` | mobility/par.rs:14 | `64` | kept: a parallelism threshold; the result is bit-identical either way |
| `STANDING_MPS` | mobility/calibration.rs:58 | `0.1` | kept because it is a check or calibration reference, not a model input |
| `QUEUED_MPS` | mobility/calibration.rs:61 | `1.0` | kept because it is a check or calibration reference, not a model input |
| `QUEUE_HEAD_M` | mobility/calibration.rs:64 | `10.0` | kept because it is a check or calibration reference, not a model input |
| `QUEUE_LINK_M` | mobility/calibration.rs:66 | `8.0` | kept because it is a check or calibration reference, not a model input |
| `LAUNCH_TO_MPS` | mobility/calibration.rs:68 | `8.0` | kept because it is a check or calibration reference, not a model input |
| `STOP_FROM_MPS` | mobility/calibration.rs:70 | `5.0` | kept because it is a check or calibration reference, not a model input |
| `POSITIONS` | mobility/calibration.rs:72 | `10` | kept because it is a check or calibration reference, not a model input |
| `LEFT_ZONE_M` | mobility/calibration.rs:76 | `6.0` | kept because it is a check or calibration reference, not a model input |
| `HCM_BASE_SATURATION_FLOW` | mobility/calibration.rs:1285 | `1900.0` | kept because it is a check or calibration reference, not a model input |
| `HCM_CBD_AREA_FACTOR` | mobility/calibration.rs:1287 | `0.90` | kept because it is a check or calibration reference, not a model input |
| `HCM_STARTUP_LOST_TIME_S` | mobility/calibration.rs:1289 | `2.0` | kept because it is a check or calibration reference, not a model input |
| `HCM_QUEUE_SPACING_M` | mobility/calibration.rs:1291 | `7.62` | kept because it is a check or calibration reference, not a model input |
| `MANHATTAN_PEDESTRIAN_COMPLIANCE` | mobility/calibration.rs:1296 | `1.0 - (974.0 / 0.42) / 21_760.0` | kept because it is a check or calibration reference, not a model input |
| `MIDTOWN_AVENUE_VEH_PER_H` | mobility/calibration.rs:1300 | `1400.0` | kept because it is a check or calibration reference, not a model input |
| `MIDTOWN_STREET_VEH_PER_H` | mobility/calibration.rs:1303 | `450.0` | kept because it is a check or calibration reference, not a model input |
| `JAM_DENSITY_VEH_KM` | mobility/fd.rs:83 | `140.0` | kept because it is a check or calibration reference, not a model input |
| `WAVE_SPEED_KMH` | mobility/fd.rs:85 | `-15.0` | kept because it is a check or calibration reference, not a model input |
| `CONVECTIVE_STABILITY_VEH_H` | mobility/fd.rs:95 | `1050.0` | kept because it is a check or calibration reference, not a model input |
| `CAPACITY_BELOW_THEORY_TOLERANCE` | mobility/fd.rs:106 | `0.20` | kept because it is a check or calibration reference, not a model input |
| `ABOUT_TOLERANCE` | mobility/fd.rs:111 | `0.20` | kept because it is a check or calibration reference, not a model input |
| `TR36885_ANTENNA_HEIGHT_M` | mobility/classes.rs:663 | `1.5` | kept: 3GPP TR 36.885 fixed evaluation value |
| `WEIDMANN_V0_MPS` | mobility/network.rs:42 | `1.34` | kept: pedestrian fundamental diagram (Weidmann 1993), used by the meso/network checks |
| `WEIDMANN_GAMMA` | mobility/network.rs:44 | `1.913` | kept: Weidmann 1993 constant |
| `WEIDMANN_JAM_DENSITY` | mobility/network.rs:46 | `5.4` | kept: Weidmann 1993 constant |
| `CROWD_RADIUS_M` | mobility/network.rs:73 | `5.0` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `MOVING_MPS` | mobility/network.rs:85 | `2.0` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `STOPPED_MPS` | mobility/network.rs:87 | `0.1` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `RIGHT_HOOK_M` | mobility/network.rs:89 | `12.0` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `DOOR_ZONE_M` | mobility/network.rs:91 | `1.0` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `LANE_UTIL_MIN` | mobility/network.rs:93 | `20` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `STEADY_MARGIN_M` | mobility/network.rs:97 | `4.0` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `PED_CELL_M` | mobility/network.rs:100 | `10.0` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `PED_INTERVAL_S` | mobility/network.rs:102 | `5.0` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `REFERENCE_VEHICLE_M` | mobility/meso/mod.rs:97 | `7.5` | kept: meso queue cell length (7.5 m, the HCM passenger-car equivalent); `scale.traffic.meso` exposes the tier |
| `MAX_DEPART_DELAY_S` | mobility/meso/mod.rs:101 | `120.0` | kept: how long a trip waits to enter before it is dropped; minor |
| `CLOSURE_SIGHT_LANES` | mobility/engine.rs:102 | `4` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `STOP_LINE_MARGIN_M` | mobility/engine.rs:109 | `0.5` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `DEFAULT_NO_CHANGE_ZONE_M` | mobility/engine.rs:118 | `30.0` | kept: lane-change geometry near the stop line; minor, and a lower value produces illegal last-moment changes |
| `DEFAULT_TURN_LATERAL_ACCEL_MPS2` | mobility/engine.rs:122 | `5.5` | exposed as `traffic.turn_lateral_accel_mps2` (new) |
| `EXIT_QUEUE_MOVING_MPS` | mobility/engine.rs:134 | `2.0` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `TRAIL_LANES` | mobility/engine.rs:138 | `3` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `RELEASE_JERK_MPS3` | mobility/engine.rs:149 | `10.0` | kept because it is a model internal |
| `PLANNED_BRAKE_JERK_MPS3` | mobility/engine.rs:153 | `20.0` | kept because it is a model internal |
| `PLANNED_STOP_MAX_DECEL_MPS2` | mobility/engine.rs:158 | `3.4` | kept: ITE 3.4 m/s² planned-stop deceleration, the same figure the yellow timing assumes |
| `CAP_MAX_DECEL_MPS2` | mobility/engine.rs:164 | `8.5` | kept: a physical braking cap (dry road) |
| `STOP_FLOOR_BIND_MPS2` | mobility/engine.rs:171 | `0.3` | kept because it is a model internal |
| `REACTION_MEDIAN_S` | mobility/engine.rs:193 | `1.08` | exposed as `actors.vehicles.drivers.start_reaction_s` (new) |
| `REACTION_SIGMA` | mobility/engine.rs:191 | `0.5` | kept: a fitted shape (Taoka 1989); the median is exposed |
| `FOLLOW_REACTION_MEDIAN_S` | mobility/engine.rs:208 | `0.5` | exposed as `actors.vehicles.drivers.queue_start_delay_s` (new) |
| `AMBER_GO_MARGIN_S` | mobility/engine.rs:228 | `0.3` | kept: a numerical margin on the amber decision; the dilemma zone itself is sourced (Zegeer & Deen 1978) |
| `CLEARING_WINDOW_S` | mobility/engine.rs:234 | `3.0` | kept: a numerical window for junction clearance |
| `QUEUE_FOLLOW_GAP_M` | mobility/engine.rs:240 | `10.0` | kept: a queue-detection geometry threshold |
| `GO_ACCEL_MPS2` | mobility/engine.rs:245 | `0.2` | kept because it is a model internal |
| `STANDSTILL_MPS` | mobility/engine.rs:248 | `0.1` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `HEAVY_CRAWL_MPS` | mobility/engine.rs:253 | `1.0` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `KERB_PASS_LOOK_M` | mobility/engine.rs:257 | `60.0` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `PULL_OUT_FOLLOWER_DECEL_MPS2` | mobility/engine.rs:263 | `2.0` | kept because it is a model internal |
| `PULL_OUT_SPEED_MPS` | mobility/engine.rs:269 | `2.0` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `DIVERGED_M` | mobility/engine.rs:473 | `2.5` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `PEDESTRIAN_WALK_LANES` | mobility/engine.rs:476 | `12` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `GROUP_SPACING_M` | mobility/engine.rs:483 | `0.7` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `GRAVITY_MPS2` | mobility/engine.rs:551 | `9.80665` | kept: physical constant |
| `CYCLIST_MAX_DESIRED_MPS` | mobility/engine.rs:636 | `25.0 / 3.6` | kept because it is the pedelec assistance cut-off (EU 25 km/h), a legal fact; `actors.vru.ebike_share` is exposed |
| `AMBER_COMMIT_LAPSE_MPS` | mobility/engine.rs:640 | `1.0` | kept: an implementation threshold; the behaviour is an ablation switch for tests |
| `STRATEGIC_M_PER_CHANGE` | mobility/engine.rs:870 | `25.0` | kept: a choice derived from MOBIL transition time; minor effect |
| `STRATEGIC_CHANGE_COST_S` | mobility/engine.rs:875 | `4.0` | kept: a route-choice tie-breaker; minor effect |
| `STRATEGIC_GIVE_UP_S` | mobility/engine.rs:880 | `8.0` | kept because it is a fallback timer, documented on `traffic.strategic_lane_choice` (which is exposed) |
| `CONGESTION_ADAPTATION_STEPS` | mobility/engine.rs:885 | `180.0` | kept because it is SUMO's adaptation memory (180 s); the period is exposed as `traffic.reroute_period_s` |
| `CONGESTION_FLOOR_MPS` | mobility/engine.rs:889 | `0.5` | kept: a numerical floor that keeps a jammed lane routable |
| `STUCK_SPEED_MPS` | mobility/engine.rs:893 | `0.1` | kept because it is SUMO's halting threshold (0.1 m/s), a definition; the timeout is `traffic.gridlock_timeout_s` |
| `MAX_DEPART_DELAY` | mobility/engine.rs:4452 | `Duration::from_secs(120)` | kept: how long a trip waits to enter before it is dropped; minor |
| `REDRAW_TRIES` | mobility/engine.rs:4643 | `4` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `MIDBLOCK_HOLD_MARGIN_M` | mobility/engine.rs:6015 | `0.3` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `MIDBLOCK_LOOKAHEAD_M` | mobility/engine.rs:6021 | `2.0` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `STOP_LINE_OFFSET_M` | mobility/intersection/mod.rs:38 | `2.0` | kept: geometry: where vehicles halt before the line |
| `MAX_CIRCULATING_SINGLE_LANE_VEH_H` | mobility/intersection/mod.rs:85 | `1800.0` | kept: roundabout capacity reference (HCM), used for checks |
| `MAX_EXIT_SINGLE_LANE_VEH_H` | mobility/intersection/mod.rs:89 | `1200.0` | kept: roundabout capacity reference (HCM), used for checks |
| `DESIGN_DEGREE_OF_SATURATION` | mobility/intersection/mod.rs:92 | `0.85` | kept: a design reference used for checks |
| `LEGACY_LIGHT_CYCLE_S` | mobility/intersection/two_coloring.rs:46 | `24.0` | kept because it applies only in the legacy-parity mode |
| `LEGACY_TURN_SPEED_MPS` | mobility/intersection/two_coloring.rs:49 | `6.0` | kept because it applies only in the legacy-parity mode |
| `LEGACY_TURN_MIN_ANGLE_DEG` | mobility/intersection/two_coloring.rs:52 | `40.0` | kept because it applies only in the legacy-parity mode |
| `OPPOSING_START_ACCEL_MPS2` | mobility/intersection/signal_fixed_time.rs:62 | `1.4` | kept: kinematics of the opposing queue in the permitted-left model |
| `OPPOSING_CRUISE_CAP_MPS` | mobility/intersection/signal_fixed_time.rs:67 | `11.176` | kept: kinematics of the opposing queue in the permitted-left model |
| `HCM_PERMITTED_LEFT_CRITICAL_HEADWAY_S` | mobility/intersection/signal_fixed_time.rs:71 | `4.5` | exposed as `traffic.permitted_left_gap_s` (new) |
| `RIGHT_ON_RED_AT_LINE_M` | mobility/intersection/signal_fixed_time.rs:77 | `3.0` | kept: geometry of a full stop at the line; right-on-red itself is exposed (`traffic.right_turn_on_red`) |
| `RIGHT_ON_RED_STANDING_MPS` | mobility/intersection/signal_fixed_time.rs:80 | `0.1` | kept: definition of a full stop |
| `STANDING_SPEED_MPS` | mobility/intersection/gap_acceptance.rs:85 | `crate::carfollowing::idm::LEGACY_V0_FLOOR_MPS` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `MIN_SIN_CROSSING` | mobility/intersection/zones.rs:77 | `0.5` | kept because it is a model internal |
| `LEGACY_CIRCLE_FACTOR` | mobility/gnss/ou_bias_legacy.rs:50 | `2.448` | kept because it applies only in the legacy-parity mode |
| `GPS_SENSOR_HORIZONTAL_M` | mobility/gnss/gauss_markov.rs:96 | `1.6` | kept because it is the white share of the measured Reid 2019 total; the receiver grade is exposed as `actors.vehicles.gnss.rtk` (new) |
| `GPS_SENSOR_VERTICAL_M` | mobility/gnss/gauss_markov.rs:99 | `3.0` | kept: vertical error does not reach a 2D BSM position test |
| `GPS_SENSOR_DECAY_FACTOR` | mobility/gnss/gauss_markov.rs:102 | `0.999` | kept: MathWorks gpsSensor default (bias memory) |
| `GPS_SENSOR_SAMPLE_S` | mobility/gnss/gauss_markov.rs:108 | `1.0` | kept: assumption recorded on the GNSS card |
| `SPS_OUTAGE_P95_S` | mobility/gnss/gauss_markov.rs:111 | `7.0` | kept: GPS SPS performance standard figure |
| `RTK_OUTAGE_P95_S` | mobility/gnss/gauss_markov.rs:114 | `60.0` | kept: measured RTK figure (Reid 2019) |
| `SPS_VELOCITY_P95_MPS` | mobility/gnss/gauss_markov.rs:117 | `0.2` | kept: GPS SPS performance standard figure |
| `EVENT_STEP_S` | mobility/gnss/gauss_markov.rs:131 | `1.0` | kept: the unit the event rates are stated per |
| `STOP_BEFORE_CROSSWALK_M` | mobility/vru/crosswalk.rs:37 | `1.2` | kept because it is crosswalk-yield geometry; yielding is a fixed legal duty |
| `YIELD_DECEL_MPS2` | mobility/vru/crosswalk.rs:41 | `3.4` | kept because it is crosswalk-yield geometry; yielding is a fixed legal duty |
| `PEDESTRIAN_REACTION_MARGIN_S` | mobility/vru/crosswalk.rs:45 | `1.0` | kept because it is crosswalk-yield geometry; yielding is a fixed legal duty |
| `STANDING_MPS` | mobility/vru/crosswalk.rs:49 | `1.0` | kept because it is crosswalk-yield geometry; yielding is a fixed legal duty |
| `HORIZON_M` | mobility/vru/crosswalk.rs:53 | `80.0` | kept because it is crosswalk-yield geometry; yielding is a fixed legal duty |
| `APPROACHING_MPS` | mobility/vru/crosswalk.rs:372 | `0.5` | kept because it is crosswalk-yield geometry; yielding is a fixed legal duty |
| `RAYLEIGH_95` | mobility/gnss/common.rs:13 | `2.4477468306808265` | kept: mathematical constant |
| `SUMO_TAU_S` | mobility/carfollowing/krauss.rs:79 | `1.0` | kept: Krauss model card default; overridable via `actors.vehicles.driving.car_following.params` |
| `CROSSING_GAP_M` | mobility/vru/social_force.rs:84 | `10.0` | kept: social-force crossing geometry; the gap margin is exposed (`actors.vru.crossing_gap_margin_s`) |
| `CROWD_SPEED_CAP` | mobility/vru/social_force.rs:89 | `1.1` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `PED_MAX_YAW_RATE` | mobility/vru/social_force.rs:479 | `4.0` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `PED_FACING_SPEED_MPS` | mobility/vru/social_force.rs:483 | `0.25` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `LATERAL_SPEED_FACTOR` | mobility/vru/social_force.rs:487 | `0.4` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `LATERAL_RELAX_S` | mobility/vru/social_force.rs:490 | `1.0` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `SIDE_SPREAD` | mobility/vru/social_force.rs:494 | `0.8` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `STEER_LOOKAHEAD_M` | mobility/vru/social_force.rs:497 | `1.0` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `KERB_DECEL_MPS2` | mobility/vru/social_force.rs:500 | `3.0` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `CARRY_CLOSE_MPS` | mobility/vru/social_force.rs:516 | `0.7` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `CARRY_CLOSE_TAU_S` | mobility/vru/social_force.rs:519 | `1.0` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `CARRY_MAX_M` | mobility/vru/social_force.rs:527 | `12.0` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `CARRY_CLOSE_MAX_S` | mobility/vru/social_force.rs:531 | `4.0` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `FLOOR_M` | mobility/vru/social_force.rs:959 | `1e-3` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `MAX_WALK_LANES` | mobility/vru/social_force.rs:1845 | `12` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `DYNAMIC_BASE` | mobility/vru/midblock.rs:52 | `1 << 24` | kept because it is pedestrian steering or crowd geometry (model internal); behaviour presets and compliance shares are exposed under `actors.vru` |
| `GIPPS_A_MPS2` | mobility/carfollowing/gipps.rs:52 | `1.7` | kept because it is a Gipps (1981) card default, overridable via `actors.vehicles.driving.car_following.params` |
| `GIPPS_EFFECTIVE_SIZE_M` | mobility/carfollowing/gipps.rs:54 | `6.5` | kept because it is a Gipps (1981) card default, overridable via `actors.vehicles.driving.car_following.params` |
| `GIPPS_V_MPS` | mobility/carfollowing/gipps.rs:56 | `20.0` | kept because it is a Gipps (1981) card default, overridable via `actors.vehicles.driving.car_following.params` |
| `GIPPS_TAU_S` | mobility/carfollowing/gipps.rs:58 | `2.0 / 3.0` | kept because it is a Gipps (1981) card default, overridable via `actors.vehicles.driving.car_following.params` |
| `WINDOW_M` | mobility/lateral/mod.rs:167 | `100.0` | kept because it is a numerical threshold or geometry tolerance with no behavioural meaning a user would vary |
| `V80_MPS` | mobility/carfollowing/w99.rs:70 | `80.0 / 3.6` | kept: W99 card constant; overridable via `actors.vehicles.driving.car_following.params` |
| `DEFAULT_STEP_S` | mobility/carfollowing/common.rs:38 | `0.1` | kept: the car-following integration step; the run step is `time.mobility_step_ms` |
| `LEGACY_A_MIN_MPS2` | mobility/carfollowing/idm.rs:96 | `-6.0` | kept because it applies only in the legacy-parity mode |
| `LEGACY_V0_FLOOR_MPS` | mobility/carfollowing/idm.rs:99 | `0.1` | kept: numerical floor (legacy parity) |
| `LEGACY_GAP_FLOOR_M` | mobility/carfollowing/idm.rs:102 | `0.5` | kept because it applies only in the legacy-parity mode |
| `LEGACY_LOOKAHEAD_M` | mobility/carfollowing/idm.rs:105 | `70.0` | kept because it applies only in the legacy-parity mode |
| `URBAN_HCM_TIME_HEADWAY_S` | mobility/carfollowing/idm.rs:111 | `1.0` | exposed as `actors.vehicles.drivers.time_headway_s` (cars) and `.truck/.bus.time_headway_s` (new) |
| `URBAN_HCM_MAX_ACCEL_MPS2` | mobility/carfollowing/idm.rs:115 | `1.7` | exposed as `actors.vehicles.drivers.max_accel_mps2` (cars) and per class (new) |
| `URBAN_HCM_TRUCK_TIME_HEADWAY_S` | mobility/carfollowing/idm.rs:123 | `URBAN_HCM_TIME_HEADWAY_S * (2.0 / 1.5)` | exposed as `actors.vehicles.drivers.truck.time_headway_s` (new) |
| `CLUTCH_ENGAGE_SPEED_MPS` | mobility/carfollowing/eidm.rs:91 | `0.5` | kept: EIDM card default; overridable via `actors.vehicles.driving.car_following.params` |
| `WEEKEND_VOLUME_RATIO` | mobility/demand/daily.rs:51 | `0.819` | kept: FHWA TMG weekend factor in the daily profile; the profile is chosen in `actors.vehicles.demand.params` |
| `TR36885_PEDESTRIAN_SPEED_MPS` | mobility/demand/tr36885.rs:56 | `3.0 / 3.6` | kept: 3GPP TR 36.885 fixed evaluation value |
| `LEGACY_GRAVITY_SCALE` | mobility/demand/od.rs:44 | `2.0` | kept: legacy OD preset |
| `MIN_ORIGIN_LANE_M` | mobility/demand/od.rs:72 | `13.0` | kept: trip-endpoint geometry |
| `LEGACY_ARRIVAL_RATE_PER_S` | mobility/demand/mod.rs:91 | `2.0` | kept: legacy default; the rate is exposed (`actors.vehicles.demand.rate_veh_per_h`) |
| `LEGACY_MAX_HEADING_DEV_DEG` | mobility/lanechange/mobil.rs:81 | `12.0` | kept because it applies only in the legacy-parity mode |
| `SPS_TIME_TRANSFER_P95_NS` | mobility/clock/tcxo_ocxo.rs:66 | `30.0` | kept: GPS SPS standard figure (clock model) |
| `DAY_S` | demand/purpose.rs:59 | `86_400.0` | kept because it is a unit (seconds per day) |
| `GRID_S` | demand/purpose.rs:177 | `60.0` | kept because it is the time grid of the purpose model, a numerical choice |
| `MIN_ENDPOINT_LANE_M` | demand/od/zones.rs:32 | `20.0` | kept because it is trip-endpoint geometry |
