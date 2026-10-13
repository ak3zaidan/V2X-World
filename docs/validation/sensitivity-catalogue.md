# Catalogue-wide sensitivity sweep

This sweep moves every numeric or choice setting the engine publishes, one at a time. The
settings come from `scenario::publish::fields()`, the same index the Studio settings window
and the agent read. Each setting goes to the low and high end of its recommended range, or
to each of its choices, on a small scenario family run with the same seed. The sweep then
records which metrics changed and by how much.

- **Command:** `v2xw sensitivity --catalogue [--only PREFIXES] [--jobs N] [--cache FILE] [--markdown FILE] [--strict]`. Use `--plan` to list the perturbations without running them.
- **Code:** `crates/v2xw-engine/src/sensitivity/sweep.rs`.
- **Families, routing, prerequisites and value overrides:** `scenarios/sensitivity/catalogue.yaml`.
- **Expectation table:** `crates/v2xw-engine/tests/expectations/*.yaml`. Each row gives a setting, a change, a metric, a direction and the theory it comes from. The next wave extends this table.
- **Fast CI subset:** `crates/v2xw-engine/tests/sensitivity_catalogue.rs`, about 4 s. It checks that every expectation names a setting and end the sweep plans, and that the plan covers at least 150 settings. It also checks that four link-budget, rate and frame settings on the DSRC family move the way the table says and are not dead. An injected wrong-direction row turns this check red.

**What a run measures.** Each run produces a fingerprint of about 250 to 300 metrics:

- the radio harness's metrics: PDR, PRR, the range edges, CBR, the latency stages, age of information and the inter-packet gap;
- every numeric leaf of the `RunReport` (`report.*`);
- the sample-weighted mean of every `metric.sample` (`metric.*`, with `metrics: [all]`);
- the ground-truth kinematics of vehicles and of VRUs (`traffic.*`, `vru.*`).

Both runs use the same seed. A setting the run never reads therefore gives a bit-identical run, and "dead" needs no tolerance.

**Cost.** A full sweep from an empty cache takes about 47 minutes on two threads. The saturated family dominates: 250 vehicles for 90 s is about 5 minutes a run. A rerun with `--cache` re-measures only the family bases and the entries whose setting, value or family changed, which takes about 8 minutes.

## Findings and what to do with them

The tables below come from the regenerated sweep:

- 217 settings and 442 perturbations.
- 69 dead settings, down from 89.
- 4 perturbations refused by every family, down from 13.
- 2 of 151 expectations contradicted. Both are explained in `docs/validation/sensitivity-expectations.md`: weather intensity on arterials, and truck headway, which is below this family's resolution.

**What changed since the first sweep:**

- The expectation wave added contexts to the families file:
  - 20 % trucks, buses or motorcycles for the per-class driver settings and for NLOSv blockage;
  - the NYC preset for kerbside;
  - a fragmenter and a pricing OBU for the post-quantum signatures;
  - real ends for the GNSS outlier rate;
  - an arrival-rate reading of `rate_veh_per_h_per_lane_km`.
- A boolean with no published default is now swept both ways.
- This resolves findings 3 and 6 below (the PQ signatures, and part of the per-class gap). Item 1 is unchanged.

The sweep reused the round-12 cache wherever a setting's family and context were unchanged. Those entries predate the `metric.headway_time` definition change (following pairs only), so the headway columns of reused rows show the old metric. The other metrics are unaffected.

The findings of the first sweep follow, kept for the record:

1. **On arterials, `weather.intensity` acts on nothing (contradiction (b)).** `v2xw-mobility/src/weather.rs` `fhwa_driving_effects` reads intensity only as a step at 0.5, and only on freeway rows. The arterial rain and snow rows ignore it and take the midpoint of the FHWA band, so on the Midtown grid rain at 0.8 equals rain at 0.3. Proposed fix: interpolate within the band by intensity on arterials too. This changes `midtown-rain.yaml`'s digest, so the owner should decide; it is not changed here.
2. **`traffic.right_turn_on_red` is dead on the procedural grid but live on Portland.** On Portland, `false` moved 86 metrics. On the grid, `true` and `false` give bit-identical runs, so the procedural world has no right turn that is eligible on red. Either the grid generator's signal plans or its jurisdiction should allow RTOR, or the loader should refuse the key on a grid.
3. **The loader is inconsistent about per-class settings.** `actors.vehicles.drivers.bicycle.*` is refused when `actors.vru.cyclists` is 0. But `truck.*`, `bus.*` and `motorcycle.*` load and do nothing on a fleet with no such class, and so do `radio.devices.rsu.*` and `radio.devices.vru.*` with no RSU or VRU. Under the settings contract, the loader should refuse these combinations the way it refuses the bicycle ones.
4. **`traffic.signals.min_green_s` and `max_green_s` (list (c)).** The published recommended ranges (2 to 60 s and 5 to 180 s) cross each other's defaults. A minimum green of 60 s is refused against the default maximum of 45 s, and a maximum of 5 s is refused against the default minimum of 7 s. Either the guide text should state the coupling or the bounds should be narrowed.
5. **Settings that are quiet because of the family, not dead.** These need a heavier or longer family before they can be judged:
   - **Low load:** `radio.mac.queue_length`, `nodes.verify_queue_depth`, `radio.dcc.*` (beyond `adaptive_alpha` and `reactive_table`, which moved) and `radio.mac.*_vi`. The DSRC family runs at a CBR of 0.08, so there is no queueing or congestion, and BSMs use the VO category.
   - **The GN family's single DENM:** `net.geonet.repetition_*`, `keep_alive_forwarding` and `range_m`. The single warning shows no effect within 24 s.
   - **No mixed classes:** the per-class driver parameters (item 3).
   - **The `meso/queue` context on the DSRC grid:** `scale.*`. It runs; the likely cause is that in 2 s no vehicle leaves its segment.
6. **Dead here, cause not yet confirmed.** These are probably by design and need a look before they are filed:
   - `messages.codec_tier: size-model` against `uper`.
   - `security.signature: ecdsa-brainpoolp256r1` against P-256, which has the same signature octets.
   - `security.kem` over a 30 s run.
   - `weather.initial: wind`.
   - `weather.surface` other than `ice`. `v2xw-mobility/src/weather.rs` uses the surface only to cap braking at μg, and the cap binds on ice alone.
7. **Refusals that are correct by design (list (c)):**
   - `radio.rat: hybrid`, which needs an arbitration policy.
   - `radio.tiers.propagation: abstract` with a medium PHY.
   - The post-quantum `security.signature` values on an OBU that publishes no figure for them.
   - `nakagami_shape_far` and `nakagami_breakpoint_m`, which must be set together. The families file now supplies both. That fix is committed, but this table predates it.

Dead settings are wired fields whose fingerprint did not change on the family the setting was routed to. Any later wave should read a dead setting as "either a wiring bug or a family that does not exercise it". Before filing it as a bug, extend `catalogue.yaml` (with a context, a route or a family).

Generated by `v2xw sensitivity --catalogue` (families `scenarios/sensitivity/catalogue.yaml`, expectations `crates/v2xw-engine/tests/expectations`).

**217 settings swept, 442 perturbations (438 ran, 4 refused everywhere), 144 settings skipped; 1144 s wall.**

Families (base fingerprints):

- `cv2x`: 291 metrics; pdr 0.953, cbr 0.236, e2e 57.942 ms, speed 0.416 m/s
- `dsrc`: 260 metrics; pdr 0.894, cbr 0.081, e2e 12.321 ms, speed 0.416 m/s
- `dsrc + {"radio.channel_model.fading":"nakagami","radio.channel_model.nakagami_breakpoint_m":90.0,"radio.channel_model.nakagami_shape_far":0.75}`: 260 metrics; pdr 0.767, cbr 0.066, e2e 12.317 ms, speed 0.416 m/s
- `dsrc + {"radio.channel_model.fading":"nakagami","radio.channel_model.nakagami_breakpoint_m":90.0,"radio.channel_model.nakagami_shape_far":0.9}`: 260 metrics; pdr 0.790, cbr 0.069, e2e 12.319 ms, speed 0.416 m/s
- `dsrc + {"radio.channel_model.fading":"nakagami"}`: 260 metrics; pdr 0.894, cbr 0.081, e2e 12.321 ms, speed 0.416 m/s
- `dsrc + {"radio.channel_model.fading":"rice"}`: 260 metrics; pdr 0.890, cbr 0.081, e2e 12.323 ms, speed 0.416 m/s
- `dsrc + {"radio.models.dcc":{"id":"dcc/etsi/adaptive-ts102687","params":{}}}`: 260 metrics; pdr 0.903, cbr 0.083, e2e 14.983 ms, speed 0.416 m/s
- `dsrc + {"radio.models.dcc":{"id":"dcc/etsi/reactive-ts102687","params":{}}}`: 260 metrics; pdr 0.896, cbr 0.084, e2e 18.423 ms, speed 0.416 m/s
- `dsrc + {"radio.tiers.mac":"high"}`: 260 metrics; pdr 0.894, cbr 0.081, e2e 12.321 ms, speed 0.416 m/s
- `dsrc + {"scale.traffic.model":"mobility/meso/queue"}`: 260 metrics; pdr 0.896, cbr 0.082, e2e 12.315 ms, speed 14.159 m/s
- `dsrc + {"time.mobility_step_ms":50}`: 260 metrics; pdr 0.896, cbr 0.081, e2e 12.321 ms, speed 0.417 m/s
- `gn`: 329 metrics; pdr 0.919, cbr 0.074, e2e 12.355 ms, speed 8.429 m/s
- `gn + {"messages.sets":["bsm","cpm"]}`: 341 metrics; pdr 0.885, cbr 0.131, e2e 14.279 ms, speed 8.429 m/s
- `nr`: 292 metrics; pdr 0.922, cbr 0.079, e2e 29.094 ms, speed 0.416 m/s
- `osm`: 268 metrics; pdr 0.077, cbr 0.002, e2e 12.240 ms, speed 3.771 m/s
- `pedestrians`: 271 metrics; pdr 0.095, cbr 0.002, e2e 10.364 ms, speed 5.596 m/s
- `pedestrians + {"actors.vru.cyclists":60}`: 275 metrics; pdr 0.096, cbr 0.002, e2e 9.684 ms, speed 5.510 m/s
- `saturated`: 311 metrics; pdr 0.180, cbr 0.095, e2e 12.349 ms, speed 4.825 m/s
- `security`: 275 metrics; pdr 0.193, cbr 0.002, e2e 12.289 ms, speed 4.898 m/s
- `security + {"net.fragmenter":{"id":"fragmenter/generic-sdu"},"nodes.default_obu":"obu/generic-automotive-soc-no-hsm"}`: 275 metrics; pdr 0.192, cbr 0.002, e2e 3.766 ms, speed 4.898 m/s
- `security + {"security.verification_policy":"on-demand"}`: 274 metrics; pdr 0.193, cbr 0.002, e2e 11.954 ms, speed 4.898 m/s
- `security + {"security.verification_policy":"prioritized"}`: 276 metrics; pdr 0.193, cbr 0.002, e2e 12.286 ms, speed 4.898 m/s
- `traffic`: 302 metrics; pdr 0.217, cbr 0.014, e2e 12.251 ms, speed 5.871 m/s
- `traffic + {"actors.rsu_placement.count":4}`: 305 metrics; pdr 0.219, cbr 0.014, e2e 12.260 ms, speed 5.871 m/s
- `traffic + {"actors.vehicles.classes":{"bus":{"fraction":0.2},"passenger":{"fraction":0.8}}}`: 302 metrics; pdr 0.217, cbr 0.014, e2e 12.237 ms, speed 5.861 m/s
- `traffic + {"actors.vehicles.classes":{"motorcycle":{"fraction":0.2},"passenger":{"fraction":0.8}}}`: 302 metrics; pdr 0.216, cbr 0.014, e2e 12.237 ms, speed 5.911 m/s
- `traffic + {"actors.vehicles.classes":{"passenger":{"fraction":0.8},"truck":{"fraction":0.2}}}`: 302 metrics; pdr 0.217, cbr 0.014, e2e 12.237 ms, speed 5.861 m/s
- `traffic + {"traffic.signals.control":"actuated"}`: 309 metrics; pdr 0.241, cbr 0.017, e2e 12.234 ms, speed 7.468 m/s
- `traffic + {"weather.initial":"rain"}`: 293 metrics; pdr 0.212, cbr 0.014, e2e 12.239 ms, speed 5.401 m/s
- `traffic + {"world.highway_preset":"urban-us-nyc"}`: 302 metrics; pdr 0.218, cbr 0.015, e2e 12.236 ms, speed 5.473 m/s

## (a) Dead settings — no metric moved (69)

Same seed, so an unread setting gives a bit-identical run. A setting listed here is either not read on this family, or read by something no reported metric observes.

| Setting | Status | Family | Values tried |
|---|---|---|---|
| `world.buildings.keep_holes` | wired | traffic | false |
| `world.buildings.metres_per_level` | wired | traffic | 1.5, 10.0 |
| `world.signals.all_red_s` | wired | osm | 0.0, 2.0 |
| `world.signals.pedestrian_walk_s` | wired | osm | 4.0, 10.0 |
| `world.signals.pedestrian_speed_mps` | wired | osm | 0.91, 1.067 |
| `actors.vehicles.drivers.truck_lane_restriction` | wired | traffic | true, false |
| `actors.vehicles.drivers.motorcycle_filtering` | wired | traffic | true, false |
| `actors.vehicles.gnss.outlier_rate_hz` | wired | security | 0.0, 0.1 |
| `actors.vehicles.gnss.outlier_magnitude_m` | wired | security | 5.0, 30.0 |
| `actors.vru.crossing_gap_margin_s` | wired | pedestrians | 0.5, 6.0 |
| `radio.tiers.propagation` | wired | dsrc | abstract, medium |
| `radio.devices.obu.antenna_height_m` | wired | dsrc | 0.1, 15.0 |
| `radio.devices.obu.antenna_pattern` | wired | dsrc | tr37885, rooftop, isotropic |
| `radio.devices.rsu.tx_power_dbm` | wired | dsrc | 11.5, 33.0 |
| `radio.devices.rsu.antenna_gain_dbi` | wired | dsrc | 1.5, 6.0 |
| `radio.devices.rsu.cable_loss_db` | wired | dsrc | 1.0 |
| `radio.devices.rsu.antenna_height_m` | wired | dsrc | 0.1, 15.0 |
| `radio.devices.vru.tx_power_dbm` | wired | dsrc | 11.5, 33.0 |
| `radio.devices.vru.antenna_gain_dbi` | wired | dsrc | -1.0, 1.0 |
| `radio.devices.vru.cable_loss_db` | wired | dsrc | 1.0 |
| `radio.devices.vru.antenna_height_m` | wired | dsrc | 0.75, 3.0 |
| `radio.range.margin_db` | wired | dsrc | 5.0, 20.0 |
| `radio.mac.event_access_category` | wired | dsrc | vo, vi, be, bk |
| `radio.mac.cw_min_vi` | wired | dsrc | 1, 1023 |
| `radio.mac.aifsn_vi` | wired | dsrc | 2, 15 |
| `radio.mac.queue_length` | wired | dsrc | 16, 64 |
| `radio.dcc.cbr_target` | wired | dsrc | 0.6, 0.7 |
| `radio.dcc.adaptive_beta` | wired | dsrc | 0.0001, 0.05 |
| `radio.dcc.max_itt_ms` | wired | dsrc | 100.0, 2000.0 |
| `radio.dcc.min_power_dbm` | wired | dsrc | -10.0, 20.0 |
| `radio.dcc.reactive_hold_ms` | wired | dsrc | 0.0, 1000.0 |
| `net.layer` | wired | gn | gn-btp |
| `net.backhaul.id` | wired | gn | backhaul/fixed, backhaul/fibre, backhaul/dsl, backhaul/microwave, backhaul/cellular, backhaul/none |
| `net.backend_net.id` | wired | gn | backend-net/fixed |
| `net.geonet.non_area_forwarding` | wired | gn | greedy, cbf |
| `net.geonet.area_radius_m` | wired | gn | 1784.0 |
| `net.geonet.range_m` | wired | gn | 3000.0 |
| `net.geonet.repetition_interval_ms` | wired | gn | 0, 60000 |
| `net.geonet.repetition_duration_ms` | wired | gn | 0, 600000 |
| `net.geonet.keep_alive_forwarding` | wired | gn | true, false |
| `net.geonet.window_s` | wired | gn | 600.0 |
| `messages.codec_tier` | wired | dsrc | size-model |
| `security.envelope` | wired | security | etsi103097 |
| `security.kem` | wired | security | ml-kem-512, ml-kem-768, ml-kem-1024, hybrid-ecies-p256-ml-kem-768 |
| `security.crypto_mode` | wired | security | real |
| `security.pseudonym_change.distance_m` | wired | security | 1.0, 100000.0 |
| `security.prioritized_horizon_m` | wired | security | 100.0, 1000.0 |
| `nodes.backend_tier` | partial | dsrc | abstract, high |
| `nodes.verify_queue_depth` | wired | dsrc | 6, 256 |
| `threats.compromised_rsu_attack.id` | partial | dsrc | threat/attacker/compromised-rsu |
| `threats.eavesdropper.id` | wired | security | threat/observer/passive-privacy |
| `detection.ma.id` | wired | security | threat/ma/corroborated, threat/ma/legacy-window |
| `scale.traffic.meso.tau_ff_s` | wired | dsrc | 60.0 |
| `scale.traffic.meso.tau_fj_s` | wired | dsrc | 60.0 |
| `scale.traffic.meso.tau_jf_s` | wired | dsrc | 60.0 |
| `scale.traffic.meso.tau_jj_s` | wired | dsrc | 60.0 |
| `scale.traffic.meso.jam_threshold` | wired | dsrc | 0.0, 1.0 |
| `scale.traffic.meso.junction_control` | wired | dsrc | true, false |
| `scale.radio.follows` | wired | dsrc | true, false |
| `scale.radio.margin_m` | wired | dsrc | 0.0 |
| `scale.radio.receive_in_margin` | wired | dsrc | true, false |
| `traffic.signals.max_green_s` | wired | traffic | 5.0, 180.0 |
| `traffic.signals.green_wave_heading_deg` | wired | traffic | 0.0, 360.0 |
| `traffic.critical_gap_factor` | wired | traffic | 0.8, 1.2 |
| `traffic.kerbside.bus_stop_spacing_m` | wired | traffic | 200.0, 400.0 |
| `traffic.kerbside.bus_dwell_mean_s` | wired | traffic | 15.0, 60.0 |
| `traffic.kerbside.taxi_dwell_median_s` | wired | traffic | 20.0, 60.0 |
| `traffic.kerbside.delivery_stop_share` | wired | traffic | 0.3, 0.6 |
| `traffic.kerbside.delivery_dwell_median_s` | wired | traffic | 30.0, 3600.0 |

### Values that moved nothing, of settings live at another value (76)

Usually the value equals the model's implicit default (a field with no published default), or the family never exercises it; a choice listed here may also be a value the model treats like the default.

| Setting | Value | Family |
|---|---|---|
| `world.signals.coordinate` | true | osm |
| `world.signals.join_m` | 0.0 | osm |
| `world.service_roads` | true | osm |
| `actors.vehicles.drivers.realism` | calibrated | traffic |
| `actors.vehicles.drivers.parameter_spread` | 0.0 | traffic |
| `actors.vehicles.drivers.perception_error` | 0.0 | traffic |
| `actors.vehicles.drivers.time_headway_s` | 1.0 | traffic |
| `actors.vehicles.drivers.politeness` | 0.2 | traffic |
| `actors.vehicles.drivers.lane_change_threshold_mps2` | 0.1 | traffic |
| `actors.vehicles.drivers.truck.min_gap_m` | 4.0 | traffic |
| `actors.vehicles.drivers.bus.min_gap_m` | 4.0 | traffic |
| `actors.vehicles.drivers.bicycle.comfort_decel_mps2` | 3.0 | pedestrians |
| `actors.vehicles.drivers.bicycle.time_headway_s` | 1.0 | pedestrians |
| `actors.vehicles.drivers.bicycle.min_gap_m` | 0.5 | pedestrians |
| `actors.vehicles.gnss.rtk` | false | security |
| `actors.vehicles.gnss.burst_rate_hz` | 0.0 | security |
| `actors.vehicles.gnss.outage_probability` | 0.0 | security |
| `actors.rsu_placement.profile` | rsu/commsignia-its-rs4 | traffic |
| `weather.initial` | wind | traffic |
| `weather.surface` | dry | traffic |
| `weather.surface` | wet | traffic |
| `weather.surface` | flooded | traffic |
| `radio.tiers.phy` | abstract | dsrc |
| `radio.tiers.mac` | high | dsrc |
| `radio.range.max_m` | 2000.0 | dsrc |
| `radio.region` | us | cv2x |
| `radio.region` | us-2016 | dsrc |
| `radio.phy.sensitivity_dbm` | -92.0 | dsrc |
| `radio.phy.cca_threshold_dbm` | -85.0 | dsrc |
| `radio.mac.periodic_access_category` | vo | dsrc |
| `radio.mac.cw_min_vo` | 3 | dsrc |
| `radio.mac.aifsn_vo` | 2 | dsrc |
| `radio.channel_model.shadowing_decorrelation_m` | 10.0 | dsrc |
| `radio.channel_model.fading` | nakagami | dsrc |
| `radio.channel_model.building_wall_loss_db` | 9.0 | traffic |
| `radio.channel_model.vehicle_blockage` | false | traffic |
| `radio.dcc.enabled` | true | dsrc |
| `radio.dcc.adaptive_alpha` | 0.001 | dsrc |
| `radio.dcc.density_coefficient` | 200.0 | dsrc |
| `radio.dcc.power_control` | true | dsrc |
| `radio.dcc.reactive_table` | a1 | dsrc |
| `radio.sidelink.reservation_interval_ms` | 100 | cv2x |
| `radio.sidelink.resource_keep_probability` | 0.8 | cv2x |
| `radio.sidelink.rsrp_threshold_dbm` | -110.0 | cv2x |
| `radio.sidelink.sensing` | true | cv2x |
| `radio.sidelink.candidate_percent` | 20 | cv2x |
| `radio.sidelink.selection_window_ms` | 100 | cv2x |
| `radio.sidelink.noise_figure_db` | 9.0 | cv2x |
| `net.geonet.forwarding` | on-air | gn |
| `net.geonet.area_forwarding` | cbf | gn |
| `net.geonet.cbf_variant` | gpc | gn |
| `net.geonet.hop_limit` | 255 | gn |
| `net.geonet.cbf_min_time_ms` | 1 | gn |
| `net.geonet.cbf_max_time_ms` | 100 | gn |
| `net.geonet.max_comm_range_m` | 1000.0 | gn |
| `net.channel_access` | continuous | gn |
| `net.access_categories` | single | gn |
| `net.dcc_cbr` | local | gn |
| `messages.cpm_interval_ms` | 100 | gn |
| `messages.path_history_points` | 23 | dsrc |
| `security.signature` | ecdsa-brainpoolp256r1 | security |
| `security.signer_id_policy.on_new_neighbour` | false | security |
| `security.pseudonym_change.strategy` | distance | security |
| `security.pseudonym_change.strategy` | c2c-cc | security |
| `security.pseudonym_change.strategy` | silent | security |
| `security.pseudonym_change.period_s` | 945.0 | security |
| `nodes.compute_tier` | high | dsrc |
| `nodes.peer_cert_cache` | 1024 | dsrc |
| `traffic.right_turn_on_red` | true | osm |
| `traffic.lane_changes` | true | traffic |
| `traffic.strategic_lane_choice` | true | traffic |
| `traffic.gridlock_timeout_s` | 0.0 | saturated |
| `traffic.signals.control` | fixed-time | traffic |
| `traffic.permitted_left_gap_s` | 4.0 | traffic |
| `traffic.kerbside.enabled` | true | traffic |
| `traffic.kerbside.taxi_share` | 0.0 | traffic |

## (b) Effects that contradict the expectation table (2 of 151)

| Setting | Change | Metric | Expected | Base | Changed | Note | Theory |
|---|---|---|---|---|---|---|---|
| `weather.intensity` | high | `traffic.speed_mean_mps` | Down | 5.4009 | 5.4009 | expected Down, changed by +0.0 % | rain and snow lower free-flow speed (HCM 7th ed. Exhibit 11-20; Rakha et al., FHWA-HOP-07-073, 2007) |
| `actors.vehicles.drivers.truck.time_headway_s` | high | `metric.headway_time` | Up | 3.7541 | 3.7269 | expected Up, changed by -0.7 % | IDM desired gap s0 + vT applies per class: trucks keeping longer gaps raise the fleet's mean headway (Treiber 2000; Yang et al. TRR 2014 truck headways) |

## (c) Perturbations refused by every family (4)

A value inside the published recommended range (or bound) that no family's loader accepts: either the family lacks a prerequisite the setting needs (extend the families file), or the loader refuses a combination it should accept.

| Setting | Value | Refusal (first family) |
|---|---|---|
| `radio.rat` | hybrid | dsrc: radio.rat: is 'hybrid', which needs a per-message arbitration policy between the 802.11p and the sidelink stack that no scenario key states; choose dsrc-80211p, lte-v2x-pc5 or nr-v2x-pc5 |
| `radio.tiers.propagation` | abstract | dsrc: radio.tiers.propagation: 'abstract' conflicts with phy 'medium': an abstract propagation model returns a reception probability rather than a received power, and a 'medium' PHY needs a power to accumulate SINR from |
| `traffic.signals.min_green_s` | 60.0 | traffic: traffic.signals.max_green_s: is 45 s, below the minimum green of 60 s |
| `traffic.signals.max_green_s` | 5.0 | traffic: traffic.signals.max_green_s: is 5 s, below the minimum green of 7 s |

## Expectations that held (149)

| Setting | Change | Metric | Expected | Base | Changed |
|---|---|---|---|---|---|
| `radio.devices.obu.tx_power_dbm` | low | `range90_m` | Down | 640.0000 | 200.0000 |
| `radio.devices.obu.tx_power_dbm` | low | `prr` | Down | 0.9078 | 0.2940 |
| `radio.devices.obu.tx_power_dbm` | low | `cbr_mean` | Down | 0.0815 | 0.0132 |
| `radio.phy.noise_figure_db` | high | `range90_m` | Down | 640.0000 | 600.0000 |
| `radio.phy.noise_figure_db` | high | `prr` | Down | 0.9078 | 0.8459 |
| `radio.phy.data_rate_mbps` | high | `airtime_us` | Down | 322.5157 | 134.4403 |
| `radio.phy.data_rate_mbps` | high | `range90_m` | Down | 640.0000 | 360.0000 |
| `radio.phy.data_rate_mbps` | low | `airtime_us` | Up | 322.5157 | 597.6352 |
| `radio.phy.data_rate_mbps` | high | `cbr_mean` | Down | 0.0815 | 0.0345 |
| `radio.channel_model.path_loss_exponent` | high | `range90_m` | Down | 640.0000 | 140.0000 |
| `radio.channel_model.path_loss_exponent` | low | `range90_m` | Up | 640.0000 | 1000.0000 |
| `radio.phy.sensitivity_dbm` | high | `range90_m` | Down | 640.0000 | 400.0000 |
| `messages.rate_hz` | low | `tx_rate_hz` | Down | 9.9375 | 1.4125 |
| `messages.rate_hz` | low | `cbr_mean` | Down | 0.0814 | 0.0196 |
| `messages.rate_hz` | low | `aoi_ms` | Up | 70.3455 | 318.5285 |
| `messages.rate_hz` | low | `ipg_ms_mean` | Up | 106.6422 | 606.2098 |
| `messages.path_history_points` | low | `psdu_bytes` | Down | 205.9623 | 195.9623 |
| `messages.path_history_points` | low | `airtime_us` | Down | 322.5157 | 306.5157 |
| `radio.mac.aifsn_vo` | high | `aifs_us` | Up | 8.6818 | 33.4075 |
| `radio.mac.cw_min_vo` | high | `backoff_us` | Up | 2.3711 | 12.4113 |
| `security.signer_id_policy.full_cert_every_ms` | low | `psdu_bytes` | Up | 208.3138 | 222.1066 |
| `radio.sidelink.noise_figure_db` | 15.0 | `prr_600_1000` | Down | 0.9230 | 0.9195 |
| `net.geonet.hop_limit` | low | `gn_forwards_sent` | Down | 9.0000 | 0.0000 |
| `actors.vehicles.demand.rate_veh_per_h` | high | `traffic.actors` | Up | 44.0000 | 49.0000 |
| `actors.vehicles.demand.rate_veh_per_h` | high | `frames_tx` | Up | 20847.0000 | 23737.0000 |
| `actors.vehicles.demand.rate_veh_per_h` | low | `traffic.actors` | Down | 44.0000 | 16.0000 |
| `actors.vehicles.equipped_fraction` | low | `frames_tx` | Down | 20847.0000 | 0.0000 |
| `actors.vehicles.drivers.speed_factor` | low | `traffic.speed_mean_mps` | Down | 5.8709 | 5.5829 |
| `actors.vehicles.drivers.speed_factor` | high | `traffic.speed_mean_mps` | Up | 5.8709 | 5.9910 |
| `actors.vehicles.drivers.max_accel_mps2` | low | `traffic.speed_mean_mps` | Down | 5.8709 | 5.5853 |
| `actors.vehicles.drivers.time_headway_s` | high | `traffic.speed_mean_mps` | Down | 5.8709 | 5.8520 |
| `traffic.right_turn_on_red` | false | `traffic.stopped_share` | Up | 0.2962 | 0.3184 |
| `actors.vru.walking_speed_mps` | high | `vru.speed_mean_mps` | Up | 1.2005 | 1.2589 |
| `actors.vru.walking_speed_mps` | low | `vru.speed_mean_mps` | Down | 1.2005 | 1.0024 |
| `actors.vru.pedestrians` | high | `vru.actors` | Up | 165.0000 | 315.0000 |
| `radio.devices.obu.tx_power_dbm` | low | `aoi_ms` | Up | 69.9074 | 98.3304 |
| `radio.devices.obu.antenna_gain_dbi` | high | `prr` | Up | 0.9078 | 0.9635 |
| `radio.devices.obu.antenna_gain_dbi` | high | `cbr_mean` | Up | 0.0815 | 0.0963 |
| `radio.devices.obu.antenna_gain_dbi` | low | `prr` | Down | 0.9078 | 0.8657 |
| `radio.devices.obu.antenna_gain_dbi` | low | `cbr_mean` | Down | 0.0815 | 0.0733 |
| `radio.devices.obu.cable_loss_db` | high | `prr` | Down | 0.9078 | 0.8785 |
| `radio.devices.obu.cable_loss_db` | high | `cbr_mean` | Down | 0.0815 | 0.0762 |
| `radio.phy.noise_figure_db` | low | `prr` | Up | 0.9078 | 0.9094 |
| `radio.phy.sensitivity_dbm` | high | `prr` | Down | 0.9078 | 0.6974 |
| `radio.channel_model.path_loss_exponent` | high | `prr` | Down | 0.9078 | 0.1065 |
| `radio.channel_model.path_loss_exponent` | high | `cbr_mean` | Down | 0.0815 | 0.0053 |
| `radio.channel_model.path_loss_exponent` | low | `prr` | Up | 0.9078 | 0.9854 |
| `radio.channel_model.path_loss_exponent` | low | `cbr_mean` | Up | 0.0815 | 0.1059 |
| `radio.channel_model.fading` | rayleigh | `prr` | Down | 0.9078 | 0.8256 |
| `radio.channel_model.fading` | none | `prr` | Up | 0.9078 | 0.9393 |
| `radio.channel_model.nakagami_shape` | low | `prr` | Down | 0.9078 | 0.8256 |
| `radio.channel_model.nakagami_shape` | high | `prr` | Up | 0.9078 | 0.9204 |
| `radio.channel_model.rice_k_db` | high | `prr` | Up | 0.9039 | 0.9229 |
| `radio.channel_model.rice_k_db` | low | `prr` | Down | 0.9039 | 0.8785 |
| `radio.channel_model.shadowing_sigma_db` | high | `prr` | Changes | 0.9078 | 0.8496 |
| `radio.phy.capture_threshold_db` | high | `prr` | Down | 0.9078 | 0.8030 |
| `radio.phy.capture_threshold_db` | high | `collision_rate` | Up | 0.0381 | 0.0493 |
| `radio.phy.cca_threshold_dbm` | high | `collision_rate` | Up | 0.0381 | 0.1363 |
| `radio.phy.cca_threshold_dbm` | high | `cbr_mean` | Down | 0.0815 | 0.0776 |
| `radio.range.max_m` | low | `cbr_mean` | Down | 0.0815 | 0.0091 |
| `radio.phy.data_rate_mbps` | low | `cbr_mean` | Up | 0.0815 | 0.1497 |
| `radio.phy.data_rate_mbps` | high | `airtime_us` | Down | 322.5157 | 134.4403 |
| `messages.rate_hz` | high | `cbr_mean` | Up | 0.0814 | 0.1679 |
| `messages.rate_hz` | high | `frames_tx` | Up | 795.0000 | 1590.0000 |
| `messages.rate_hz` | high | `aoi_ms` | Down | 70.3455 | 43.3461 |
| `messages.rate_hz` | high | `collision_rate` | Up | 0.0383 | 0.0835 |
| `messages.rate_hz` | high | `prr` | Down | 0.9097 | 0.8636 |
| `messages.path_history_points` | low | `cbr_mean` | Down | 0.0815 | 0.0776 |
| `radio.mac.periodic_access_category` | bk | `aifs_us` | Up | 8.6818 | 21.9283 |
| `radio.mac.periodic_access_category` | bk | `access_ms` | Up | 0.0309 | 0.0577 |
| `net.channel_access` | alternating | `e2e_ms_mean` | Up | 12.3214 | 30.9920 |
| `net.channel_access` | alternating | `collision_rate` | Up | 0.0381 | 0.3846 |
| `net.channel_access` | alternating | `prr` | Down | 0.9078 | 0.5002 |
| `radio.dcc.adaptive_alpha` | high | `cbr_mean` | Down | 0.0829 | 0.0062 |
| `radio.dcc.adaptive_alpha` | high | `aoi_ms` | Up | 74.4392 | 146.6415 |
| `radio.dcc.adaptive_alpha` | high | `frames_tx` | Down | 795.0000 | 441.0000 |
| `radio.sidelink.cbr_threshold_dbm` | high | `cbr_mean` | Down | 0.2359 | 0.1190 |
| `radio.sidelink.cbr_threshold_dbm` | low | `cbr_mean` | Up | 0.2359 | 0.4768 |
| `radio.sidelink.candidate_percent` | high | `collision_rate` | Down | 0.0696 | 0.0254 |
| `radio.sidelink.selection_window_ms` | low | `e2e_ms_mean` | Down | 29.0945 | 28.6228 |
| `radio.sidelink.sensing` | false | `collision_rate` | Up | 0.0426 | 0.0498 |
| `radio.channel_model.vehicle_blockage` | true | `prr` | Down | 0.2113 | 0.2112 |
| `radio.sidelink.resource_keep_probability` | low | `sl_selections` | Up | 62.0000 | 118.0000 |
| `security.signer_id_policy.digest_otherwise` | false | `psdu_bytes` | Up | 208.3138 | 332.3076 |
| `security.signer_id_policy.digest_otherwise` | false | `airtime_us` | Up | 325.3375 | 490.6625 |
| `security.signer_id_policy.digest_otherwise` | false | `cbr_mean` | Up | 0.0023 | 0.0038 |
| `security.signer_id_policy.full_cert_every_ms` | high | `psdu_bytes` | Down | 208.3138 | 201.4173 |
| `security.signer_id_policy.full_cert_every_ms` | low | `airtime_us` | Up | 325.3375 | 343.7280 |
| `security.signer_id_policy.on_new_neighbour` | true | `psdu_bytes` | Up | 208.3138 | 209.8068 |
| `security.signature` | falcon-512 | `psdu_bytes` | Up | 208.3138 | 873.4294 |
| `security.signature` | falcon-512 | `airtime_us` | Up | 325.3375 | 1212.5500 |
| `security.signature` | ml-dsa-65 | `psdu_bytes` | Up | 208.3138 | 1201.5078 |
| `security.signature` | ml-dsa-65 | `frames_tx` | Up | 1941.0000 | 6611.0000 |
| `security.pseudonym_change.period_s` | low | `aoi_ms` | Up | 308.8016 | 456.0920 |
| `nodes.peer_cert_cache` | low | `aoi_ms` | Up | 69.9074 | 215.3479 |
| `security.verification_policy` | on-demand | `e2e_ms_mean` | Down | 12.2893 | 11.9536 |
| `actors.vehicles.gnss.burst_rate_hz` | high | `report.phase2.verdicts_fired` | Up | 0.0000 | 1.0000 |
| `actors.vehicles.gnss.outage_probability` | high | `report.phase2.verdicts_fired` | Up | 0.0000 | 58.0000 |
| `actors.vehicles.gnss.outlier_rate_hz` | high | `report.phase2.verdicts_fired` | Same | 0.0000 | 0.0000 |
| `net.geonet.area_forwarding` | simple | `gn_forwards_sent` | Up | 9.0000 | 117.0000 |
| `net.geonet.cbf_max_time_ms` | low | `report.geonet.0.latency_p95_us` | Down | 21980.0000 | 16028.0000 |
| `net.geonet.max_comm_range_m` | low | `gn_forwards_sent` | Up | 9.0000 | 96.0000 |
| `messages.cpm_interval_ms` | high | `cbr_mean` | Down | 0.1313 | 0.1037 |
| `messages.cpm_interval_ms` | high | `frames_tx` | Down | 12248.0000 | 10554.0000 |
| `actors.vehicles.demand.rate_veh_per_h` | high | `cbr_mean` | Up | 0.0143 | 0.0186 |
| `actors.vehicles.demand.rate_veh_per_h` | high | `traffic.speed_mean_mps` | Down | 5.8709 | 5.6194 |
| `actors.vehicles.demand.rate_veh_per_h` | low | `cbr_mean` | Down | 0.0143 | 0.0056 |
| `actors.vehicles.demand.rate_veh_per_h` | low | `frames_tx` | Down | 20847.0000 | 8655.0000 |
| `actors.vehicles.demand.fleet_size` | high | `traffic.actors` | Up | 44.0000 | 89.0000 |
| `actors.vehicles.demand.fleet_size` | high | `cbr_mean` | Up | 0.0143 | 0.0402 |
| `actors.vehicles.demand.rate_veh_per_h_per_lane_km` | high | `traffic.actors` | Up | 44.0000 | 59.0000 |
| `actors.vehicles.demand.rate_veh_per_h_per_lane_km` | low | `traffic.actors` | Down | 44.0000 | 19.0000 |
| `actors.vehicles.equipped_fraction` | 0.5 | `cbr_mean` | Down | 0.0143 | 0.0099 |
| `actors.vehicles.drivers.time_headway_s` | high | `metric.headway_time` | Up | 3.7447 | 4.2928 |
| `actors.vehicles.drivers.min_gap_m` | high | `metric.headway_distance` | Up | 59.8185 | 60.3690 |
| `actors.vehicles.drivers.reaction_time_s` | high | `traffic.stopped_share` | Changes | 0.1657 | 0.1668 |
| `actors.vehicles.drivers.speed_factor` | low | `metric.flow` | Down | 21.5366 | 20.3601 |
| `actors.vehicles.drivers.start_reaction_s` | high | `traffic.stopped_share` | Up | 0.1657 | 0.1673 |
| `actors.vehicles.drivers.start_reaction_s` | low | `traffic.stopped_share` | Down | 0.1657 | 0.1594 |
| `actors.vehicles.drivers.max_accel_mps2` | low | `metric.flow` | Down | 21.5366 | 20.6605 |
| `actors.vehicles.drivers.truck.max_accel_mps2` | low | `traffic.speed_mean_mps` | Down | 5.8613 | 5.8561 |
| `actors.vehicles.drivers.bus.max_accel_mps2` | low | `traffic.speed_mean_mps` | Down | 5.8608 | 5.8558 |
| `actors.vehicles.drivers.motorcycle.max_accel_mps2` | low | `metric.acceleration` | Changes | 0.0648 | 0.0649 |
| `traffic.signals.control` | actuated | `traffic.stopped_share` | Down | 0.1657 | 0.0435 |
| `traffic.signals.control` | actuated | `traffic.speed_mean_mps` | Up | 5.8709 | 7.4676 |
| `traffic.signals.passage_s` | high | `traffic.stopped_share` | Up | 0.0435 | 0.0660 |
| `traffic.signals.min_green_s` | low | `traffic.stopped_share` | Down | 0.0435 | 0.0378 |
| `world.signals.cycle_s` | high | `traffic.speed_mean_mps` | Down | 3.7712 | 3.4320 |
| `world.signals.cycle_s` | low | `traffic.speed_mean_mps` | Up | 3.7712 | 4.4271 |
| `world.signals.progression_speed_mps` | high | `traffic.speed_mean_mps` | Down | 3.7712 | 3.7269 |
| `world.signals.progression_speed_mps` | low | `traffic.speed_mean_mps` | Down | 3.7712 | 3.5464 |
| `traffic.right_turn_on_red` | false | `traffic.speed_mean_mps` | Down | 3.7712 | 3.6248 |
| `traffic.kerbside.taxi_share` | high | `traffic.speed_mean_mps` | Down | 5.8709 | 5.4727 |
| `traffic.kerbside.taxi_share` | high | `traffic.stopped_share` | Up | 0.1657 | 0.1959 |
| `traffic.turn_lateral_accel_mps2` | low | `traffic.speed_mean_mps` | Down | 5.8709 | 5.8162 |
| `traffic.kerbside.enabled` | false | `traffic.stopped_share` | Down | 0.1959 | 0.1657 |
| `traffic.lane_changes` | false | `traffic.speed_mean_mps` | Changes | 5.8709 | 5.5152 |
| `weather.initial` | rain | `traffic.speed_mean_mps` | Down | 5.8709 | 5.4009 |
| `weather.initial` | snow | `traffic.speed_mean_mps` | Down | 5.8709 | 4.5078 |
| `weather.initial` | snow | `metric.headway_time` | Up | 3.7447 | 4.1197 |
| `weather.surface` | ice | `metric.ttc_conflicts` | Up | 0.0000 | 28.0000 |
| `world.buildings.enabled` | false | `prr` | Up | 0.2128 | 0.9817 |
| `world.buildings.enabled` | false | `cbr_mean` | Up | 0.0143 | 0.0817 |
| `radio.channel_model.building_loss_db_per_m` | low | `prr` | Up | 0.2128 | 0.2254 |
| `radio.channel_model.building_wall_loss_db` | low | `prr` | Up | 0.2128 | 0.2167 |
| `actors.vru.pedestrians` | high | `frames_tx` | Up | 5410.0000 | 7532.0000 |
| `actors.vru.device_fraction` | high | `frames_tx` | Up | 5410.0000 | 6971.0000 |
| `actors.vru.cyclists` | high | `vru.actors` | Up | 165.0000 | 300.0000 |
| `actors.vru.red_crossing_share` | high | `traffic.stopped_share` | Up | 0.1660 | 0.1801 |

## Matrix — setting × metric (relative change against the family's base)

`↑12%` / `↓12%`: the metric rose or fell by that share; empty: unchanged (bit-identical). *Moved* counts every fingerprint metric that changed; *largest* names the three largest relative changes outside the headline columns.

| Setting | Value | Family | Moved | PDR | PRR | range90 | coll | CBR | e2e | AoI | frames | bytes | GN reach | speed | stopped | VRU speed | Largest other |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `time.mobility_step_ms` | 50 | dsrc | 59 | ↑0% | ↑0% | ↑9% | ↑1% | ↓0% | ↓0% | ↑1% |  |  |  | ↑0% | ↑2% |  | report.events_by_class.MobilityStep ↑95%; report.mobility_steps ↑95%; metric.ttc_min ↑53% |
| `time.metric_period_s` | 0.1 | dsrc | 46 |  |  |  |  | ↑5% |  |  |  |  |  |  |  |  | metric.drac gone; metric.headway_distance gone; metric.headway_time gone |
| `time.metric_period_s` | 3600.0 | dsrc | 67 |  |  |  |  | gone |  |  |  |  |  |  |  |  | cbr_max gone; metric.acceleration gone; metric.air_bytes_per_payload_byte gone |
| `time.warmup_s` | 10.0 | traffic | 44 |  |  |  |  |  |  |  |  |  |  |  |  |  | metric.acceleration ↓45%; metric.bytes_air ↑10%; metric.bytes_total ↑10% |
| `time.warmup_s` | 60.0 | traffic | 51 |  |  |  |  |  |  |  |  |  |  |  |  |  | metric.acceleration ↓102%; metric.mac_queue_depth ↑96%; metric.mac_access_delay ↑80% |
| `world.buildings.enabled` | false | traffic | 78 | ↑353% | ↑361% | ↑850% | ↑68% | ↑470% | ↑0% | ↓97% |  |  |  |  |  |  | report.rx_losses.collision ↑6250%; loss_collision ↑6135%; prr_600_1000 ↑1752% |
| `world.buildings.keep_holes` | false | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `world.buildings.metres_per_level` | 1.5 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `world.buildings.metres_per_level` | 10.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `world.highway_preset` | sumo-german | osm | 88 | ↓6% | ↓4% |  | ↑3% | ↓8% | ↑0% | ↑14% |  | ↓1% |  | ↓4% | ↑7% |  | ipg_ms_p95 ↑46%; report.apps.lta.lead_ms_sum ↓43%; stage_access_ms ↓28% |
| `world.highway_preset` | urban-us-co | osm | 79 | ↓5% | ↓5% |  | ↑14% | ↓6% | ↓0% | ↑3% |  | ↑0% |  | ↓4% | ↑1% |  | stage_verify_wait_ms ↑126%; ipg_ms_p95 ↑46%; prr_400_600 ↓16% |
| `world.highway_preset` | urban-us-il | osm | 76 | ↑0% | ↑0% |  | ↓0% | ↑0% | ↑0% | ↓2% |  | ↑0% |  | ↓0% | ↑1% |  | metric.p2pcd_requests ↑4%; prr_600_1000 ↑2%; metric.pdr_by_cause ↓2% |
| `world.highway_preset` | urban-us-mn | osm | 76 | ↑0% | ↑0% |  | ↓0% | ↑0% | ↑0% | ↓2% |  | ↑0% |  | ↓0% | ↑1% |  | metric.p2pcd_requests ↑4%; prr_600_1000 ↑2%; metric.pdr_by_cause ↓2% |
| `world.highway_preset` | urban-us-nm | osm | 79 | ↓5% | ↓5% |  | ↑14% | ↓6% | ↓0% | ↑3% |  | ↑0% |  | ↓4% | ↑1% |  | stage_verify_wait_ms ↑126%; ipg_ms_p95 ↑46%; prr_400_600 ↓16% |
| `world.highway_preset` | urban-us-tx | osm | 76 | ↑0% | ↑0% |  | ↓0% | ↑0% | ↑0% | ↓2% |  | ↑0% |  | ↓0% | ↑1% |  | metric.p2pcd_requests ↑4%; prr_600_1000 ↑2%; metric.pdr_by_cause ↓2% |
| `world.signals.coordinate` | true | osm | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `world.signals.coordinate` | false | osm | 93 | ↑5% | ↑4% |  | ↑6% | ↑4% | ↑0% | ↓15% |  | ↑0% |  | ↓4% | ↓1% |  | stage_verify_wait_ms ↑107%; report.apps.lta.lead_ms_sum ↑101%; metric.app_lead_time_s ↑61% |
| `world.signals.progression_speed_mps` | 1.0 | osm | 91 | ↑6% | ↑3% |  | ↓3% | ↑3% | ↑0% | ↓10% |  | ↑0% |  | ↓6% | ↑3% |  | report.apps.lta.missed ↑1.000; metric.app_miss_ratio ↑0.200; metric.app_lead_time_s ↑65% |
| `world.signals.progression_speed_mps` | 40.0 | osm | 82 | ↑12% | ↑9% |  | ↓10% | ↑5% | ↑0% | ↓20% |  | ↑1% |  | ↓1% | ↓3% |  | report.apps.lta.lead_ms_sum ↑65%; metric.app_lead_time_s ↑65%; prr_400_600 ↑40% |
| `world.signals.cycle_s` | 60.0 | osm | 97 | ↑12% | ↑10% |  | ↑38% | ↑7% | ↑0% | ↓12% |  | ↑1% |  | ↑17% | ↓33% |  | report.apps.lta.missed ↑1.000; metric.app_miss_ratio ↑0.250; stage_verify_wait_ms ↑52% |
| `world.signals.cycle_s` | 120.0 | osm | 82 | ↓0% | ↓2% |  | ↑6% | ↓6% | ↓0% | ↑13% |  | ↓0% |  | ↓9% | ↑4% |  | report.apps.lta.lead_ms_sum ↑65%; metric.app_lead_time_s ↑65%; metric.acceleration ↓35% |
| `world.signals.all_red_s` | 0.0 | osm | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `world.signals.all_red_s` | 2.0 | osm | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `world.signals.pedestrian_walk_s` | 4.0 | osm | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `world.signals.pedestrian_walk_s` | 10.0 | osm | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `world.signals.pedestrian_speed_mps` | 0.91 | osm | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `world.signals.pedestrian_speed_mps` | 1.067 | osm | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `world.signals.join_m` | 0.0 | osm | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `world.signals.join_m` | 30.0 | osm | 1 |  |  |  |  |  |  |  |  |  |  |  |  |  | metric.flow ↓0% |
| `world.service_roads` | true | osm | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `world.service_roads` | false | osm | 122 | ↑15% | ↑15% |  | ↓35% | ↑10% | ↑0% | ↑1% |  | ↓1% |  | ↓1% | ↑25% |  | report.apps.bsw.false_warnings new; report.apps.bsw.issued new; report.apps.bsw.lead_ms_sum new |
| `actors.vehicles.demand.kind` | mobility/demand/none | traffic | 163 | ↓100% | gone | gone | ↓100% | gone | gone | gone | ↓100% | gone |  | gone | gone |  | access_ms gone; aifs_us gone; airtime_us gone |
| `actors.vehicles.demand.rate_veh_per_h` | 450.0 | traffic | 141 | ↑4% | ↑5% |  | ↓60% | ↓61% | ↓0% | ↑10% | ↓58% | ↑1% |  | ↑17% | ↓21% |  | loss_collision gone; metric.app_lead_time_s gone; metric.drac gone |
| `actors.vehicles.demand.rate_veh_per_h` | 1800.0 | traffic | 138 | ↑1% | ↑1% |  | ↓5% | ↑30% | ↑0% | ↓9% | ↑14% | ↑0% |  | ↓4% | ↑11% |  | loss_collision gone; metric.pet new; report.apps.lta.false_warnings gone |
| `actors.vehicles.demand.rate_veh_per_h_per_lane_km` | 25.0 | traffic | 136 | ↓1% | ↓1% |  | ↓57% | ↓50% | ↑0% | ↑3% | ↓52% | ↑1% |  | ↑15% | ↓29% |  | loss_collision gone; metric.app_lead_time_s gone; metric.headway_time gone |
| `actors.vehicles.demand.rate_veh_per_h_per_lane_km` | 100.0 | traffic | 134 | ↑0% | ↑1% |  | ↑25% | ↑51% | ↓0% | ↓8% | ↑38% | ↑0% |  | ↓5% | ↑8% |  | metric.pet new; report.despawn_causes.TripComplete new; report.apps.fcw.missed ↑2.000 |
| `actors.vehicles.demand.fleet_size` | 10 | traffic | 142 | ↑14% | ↑15% |  | ↓88% | ↓63% | ↑0% | ↑146% | ↓57% | ↑3% |  | ↑16% | ↓13% |  | loss_collision gone; metric.app_lead_time_s gone; metric.app_miss_ratio gone |
| `actors.vehicles.demand.fleet_size` | 80 | traffic | 142 | ↓6% | ↓6% |  | ↑161% | ↑180% | ↑0% | ↑99% | ↑244% | ↑2% |  | ↑4% | ↑11% |  | metric.pet new; report.despawn_causes.TripComplete new; report.apps.ima.lead_ms_sum ↑48084.000 |
| `actors.vehicles.equipped_fraction` | 0.05 | traffic | 148 | ↓100% | gone | gone | ↓100% | gone | gone | gone | ↓100% | gone |  |  |  |  | access_ms gone; aifs_us gone; airtime_us gone |
| `actors.vehicles.equipped_fraction` | 0.5 | traffic | 118 | ↑10% | ↑11% |  | ↓39% | ↓31% | ↓0% | ↑23% | ↓35% | ↓0% |  |  |  |  | loss_collision gone; report.rx_losses.collision gone; report.apps.ima.lead_ms_sum ↑3326.000 |
| `actors.vehicles.drivers.realism` | calibrated | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.realism` | identical | traffic | 130 | ↑0% | ↑0% |  | ↓1% | ↑1% | ↓0% | ↓14% | ↑0% | ↑0% |  | ↑2% | ↓14% |  | loss_collision gone; report.rx_losses.collision gone; report.apps.fcw.missed ↑2.000 |
| `actors.vehicles.drivers.realism` | heterogeneous | traffic | 124 | ↑0% | ↑0% |  | ↓1% | ↑1% | ↓0% | ↑1% | ↑0% | ↓0% |  | ↓0% | ↓3% |  | loss_collision gone; report.rx_losses.collision gone; report.apps.ima.lead_ms_sum ↑3521.000 |
| `actors.vehicles.drivers.realism` | human | traffic | 124 | ↑1% | ↑1% |  | ↓2% | ↑1% | ↓0% | ↑4% | ↓0% | ↓0% |  | ↓1% | ↑1% |  | loss_collision gone; report.rx_losses.collision gone; report.apps.lta.false_warnings ↑1.000 |
| `actors.vehicles.drivers.parameter_spread` | 0.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.parameter_spread` | 2.0 | traffic | 115 | ↑1% | ↑1% |  | ↓1% | ↑1% | ↓0% | ↑2% |  | ↓0% |  | ↓1% | ↓6% |  | loss_collision gone; report.apps.eebl.false_warnings new; report.apps.eebl.issued new |
| `actors.vehicles.drivers.reaction_time_s` | 0.6 | traffic | 129 | ↑0% | ↑0% |  | ↓2% | ↑1% | ↓0% | ↑4% | ↓0% | ↓0% |  | ↓0% | ↑4% |  | metric.pet new; report.apps.eebl.false_warnings new; report.apps.eebl.issued new |
| `actors.vehicles.drivers.reaction_time_s` | 1.2 | traffic | 130 | ↑1% | ↑0% |  | ↑2% | ↑2% | ↓0% | ↑8% | ↓0% | ↓0% |  | ↓0% | ↑1% |  | loss_collision gone; metric.pet new; report.apps.eebl.false_warnings new |
| `actors.vehicles.drivers.perception_error` | 0.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.perception_error` | 3.0 | traffic | 117 | ↑0% | ↑0% |  | ↓0% | ↓0% | ↓0% | ↑0% | ↓0% | ↓0% |  | ↑0% | ↓1% |  | report.apps.fcw.missed ↑1.000; metric.app_miss_ratio ↑0.143; report.apps.fcw.lead_n ↓50% |
| `actors.vehicles.drivers.truck_lane_restriction` | true | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.truck_lane_restriction` | false | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.motorcycle_filtering` | true | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.motorcycle_filtering` | false | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.max_accel_mps2` | 1.0 | traffic | 140 | ↓2% | ↓2% |  | ↑4% | ↓3% | ↑0% | ↓1% | ↓0% | ↓0% |  | ↓5% | ↓10% |  | loss_collision gone; metric.app_lead_time_s gone; report.apps.eebl.false_warnings new |
| `actors.vehicles.drivers.max_accel_mps2` | 2.0 | traffic | 123 | ↑0% | ↓0% |  | ↓1% | ↑1% | ↓0% | ↑0% | ↓0% | ↑0% |  | ↑1% | ↑1% |  | report.apps.ima.true_warnings ↑100%; report.apps.fcw.lead_ms_sum ↓56%; report.apps.fcw.lead_n ↓50% |
| `actors.vehicles.drivers.comfort_decel_mps2` | 1.5 | traffic | 117 | ↑0% | ↑0% |  | ↓0% | ↓0% | ↑0% | ↓1% | ↑0% | ↓0% |  | ↓0% | ↓1% |  | report.apps.ima.lead_ms_sum ↑3406.000; report.apps.ima.lead_n ↑1.000; report.apps.fcw.lead_n ↓50% |
| `actors.vehicles.drivers.comfort_decel_mps2` | 3.0 | traffic | 128 | ↑0% | ↑0% |  | ↓0% | ↑0% | ↓0% | ↑0% | ↓0% | ↓0% |  | ↑0% | ↑0% |  | report.apps.eebl.false_warnings new; report.apps.eebl.issued new; report.apps.eebl.lead_ms_sum new |
| `actors.vehicles.drivers.time_headway_s` | 1.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.time_headway_s` | 2.0 | traffic | 120 | ↑0% | ↑0% |  | ↓0% | ↓0% | ↓0% | ↑0% | ↓0% | ↓0% |  | ↓0% | ↓6% |  | metric.ttc_min ↑121%; report.apps.fcw.lead_ms_sum ↓100%; report.apps.fcw.lead_n ↓100% |
| `actors.vehicles.drivers.min_gap_m` | 1.5 | traffic | 117 | ↓0% | ↓0% |  | ↑0% | ↑0% | ↑0% | ↑1% | ↑0% | ↑0% |  | ↑0% | ↑0% |  | metric.pet new; report.apps.fcw.missed ↑1.000; metric.app_miss_ratio ↑0.143 |
| `actors.vehicles.drivers.min_gap_m` | 3.0 | traffic | 115 | ↑0% | ↑0% |  | ↑0% | ↑1% | ↓0% | ↓0% | ↓0% | ↑0% |  | ↓0% | ↓1% |  | report.apps.fcw.lead_n ↓50%; report.apps.fcw.lead_ms_sum ↓48%; metric.ttc_min ↑32% |
| `actors.vehicles.drivers.politeness` | 0.2 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.politeness` | 0.5 | traffic | 112 | ↑0% | ↑0% |  | ↓0% | ↑1% | ↑0% | ↑0% | ↓0% | ↑0% |  | ↑0% | ↓1% |  | report.apps.bsw.issued ↓12%; report.apps.fcw.false_warnings ↓11%; metric.app_warnings ↓9% |
| `actors.vehicles.drivers.lane_change_threshold_mps2` | 0.1 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.lane_change_threshold_mps2` | 0.3 | traffic | 98 | ↓0% | ↓0% |  | ↓4% | ↓0% | ↓0% | ↑3% |  | ↓0% |  | ↓0% |  |  | report.apps.fcw.true_warnings ↑75%; report.apps.fcw.lead_n ↑50%; report.apps.fcw.lead_ms_sum ↑48% |
| `actors.vehicles.drivers.speed_factor` | 0.9 | traffic | 131 | ↓3% | ↓3% |  | ↑2% | ↓3% | ↓0% | ↑1% | ↓0% | ↓0% |  | ↓5% | ↑2% |  | loss_collision gone; report.apps.ima.false_warnings gone; report.apps.ima.issued gone |
| `actors.vehicles.drivers.speed_factor` | 1.1 | traffic | 120 | ↑1% | ↑1% |  | ↑1% | ↑2% | ↓0% | ↓1% | ↑0% | ↑0% |  | ↑2% | ↑6% |  | metric.pet new; report.apps.ima.true_warnings ↓100%; stage_access_ms ↓66% |
| `actors.vehicles.drivers.speed_spread` | 0.0 | traffic | 132 | ↑0% | ↑0% |  | ↑4% | ↑0% | ↓0% | ↓6% | ↑0% | ↑0% |  | ↑1% | ↓9% |  | loss_collision gone; report.apps.lta.false_warnings gone; report.apps.lta.issued gone |
| `actors.vehicles.drivers.speed_spread` | 2.0 | traffic | 127 | ↓2% | ↓3% |  | ↓2% | ↓2% | ↓0% | ↑2% | ↓0% | ↓0% |  | ↓3% | ↑9% |  | loss_collision gone; metric.pet new; report.rx_losses.collision gone |
| `actors.vehicles.drivers.start_reaction_s` | 0.8 | traffic | 119 | ↓0% | ↓0% |  | ↓1% | ↑0% | ↓0% | ↓4% | ↑0% | ↓0% |  | ↑1% | ↓4% |  | report.apps.fcw.missed ↑1.000; metric.app_miss_ratio ↑0.111; report.apps.ima.true_warnings ↑100% |
| `actors.vehicles.drivers.start_reaction_s` | 1.5 | traffic | 121 | ↑0% | ↑0% |  | ↑1% | ↑0% | ↑0% | ↓2% | ↓0% | ↓0% |  | ↓0% | ↑1% |  | metric.pet new; report.apps.ima.false_warnings ↑100%; report.apps.ima.true_warnings ↓100% |
| `actors.vehicles.drivers.queue_start_delay_s` | 0.3 | traffic | 84 | ↑0% | ↑0% |  | ↓0% | ↓0% | ↑0% | ↑0% |  | ↓0% |  | ↑0% | ↓0% |  | report.apps.fcw.false_warnings ↑6%; metric.app_precision ↓4%; report.apps.fcw.issued ↑3% |
| `actors.vehicles.drivers.queue_start_delay_s` | 1.0 | traffic | 108 | ↑0% | ↓0% |  | ↓0% | ↓0% | ↓0% | ↑0% | ↓0% | ↓0% |  | ↑0% | ↑0% |  | metric.verify_wait ↑9%; report.apps.fcw.issued ↑6%; metric.headway_time ↑2% |
| `actors.vehicles.drivers.truck.max_accel_mps2` | 0.5 | traffic | 90 | ↑0% | ↑0% |  | ↓0% | ↓0% | ↑0% | ↑0% |  | ↓0% |  | ↓0% | ↓2% |  | metric.ttc_min ↓46%; metric.verify_wait ↑8%; metric.acceleration ↓2% |
| `actors.vehicles.drivers.truck.max_accel_mps2` | 1.3 | traffic | 90 | ↓0% | ↓0% |  | ↓3% | ↓0% | ↑0% | ↑1% |  | ↑0% |  | ↑0% | ↑7% |  | metric.ttc_min ↓10%; report.rx_losses.hidden-terminal ↓3%; loss_hidden-terminal ↓3% |
| `actors.vehicles.drivers.truck.comfort_decel_mps2` | 1.5 | traffic | 69 | ↓0% | ↓0% |  | ↓0% |  | ↑0% | ↑0% |  | ↓0% |  | ↓0% | ↓0% |  | metric.ttc_min ↓50%; metric.drac ↑1%; loss_fading ↓0% |
| `actors.vehicles.drivers.truck.comfort_decel_mps2` | 4.0 | traffic | 62 | ↑0% | ↑0% |  | ↑0% | ↑0% | ↑0% | ↑0% |  |  |  | ↑0% | ↑0% |  | metric.ttc_min ↓30%; report.rx_losses.fading ↓0%; loss_fading ↓0% |
| `actors.vehicles.drivers.truck.time_headway_s` | 1.5 | traffic | 71 | ↓0% | ↓0% |  | ↓0% |  | ↓0% | ↑0% |  | ↓0% |  | ↓0% | ↓0% |  | metric.ttc_min ↓50%; metric.drac ↑1%; metric.p2pcd_requests ↑1% |
| `actors.vehicles.drivers.truck.time_headway_s` | 2.5 | traffic | 78 | ↓0% |  |  | ↑0% | ↓0% | ↓0% | ↓0% |  | ↓0% |  | ↓0% | ↓1% |  | metric.ttc_min ↓62%; report.apps.ima.issued ↑25%; loss_fading ↓1% |
| `actors.vehicles.drivers.truck.min_gap_m` | 2.5 | traffic | 83 | ↓0% | ↓0% |  | ↑0% | ↓0% | ↑0% | ↓0% |  | ↓0% |  | ↑0% | ↑0% |  | metric.ttc_min ↓68%; report.apps.bsw.issued ↑4%; metric.app_warnings ↑3% |
| `actors.vehicles.drivers.truck.min_gap_m` | 4.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.bus.max_accel_mps2` | 0.5 | traffic | 89 | ↑0% | ↑0% |  | ↓1% | ↓0% | ↑0% | ↓0% |  | ↓0% |  | ↓0% | ↓2% |  | metric.ttc_min ↓65%; metric.acceleration ↓2%; metric.drac ↑2% |
| `actors.vehicles.drivers.bus.max_accel_mps2` | 1.2 | traffic | 88 | ↓0% | ↓0% |  | ↓3% | ↓0% | ↑0% | ↑1% |  | ↑0% |  | ↑0% | ↑7% |  | metric.ttc_min ↓52%; metric.verify_wait ↓8%; metric.collision_rate ↓4% |
| `actors.vehicles.drivers.bus.comfort_decel_mps2` | 1.5 | traffic | 71 | ↑0% | ↑0% |  | ↓0% | ↓0% | ↓0% | ↓0% |  | ↓0% |  | ↓0% | ↓0% |  | metric.ttc_min ↓70%; metric.drac ↑1%; loss_fading ↑0% |
| `actors.vehicles.drivers.bus.comfort_decel_mps2` | 4.0 | traffic | 72 | ↑0% | ↑0% |  | ↓0% |  | ↑0% | ↑0% |  | ↑0% |  | ↑0% | ↑0% |  | metric.ttc_min ↓46%; metric.drac ↑0%; report.faint_arrivals ↑0% |
| `actors.vehicles.drivers.bus.time_headway_s` | 1.5 | traffic | 66 | ↓0% | ↓0% |  | ↑0% | ↓0% | ↓0% | ↑0% |  | ↓0% |  | ↓0% | ↓0% |  | metric.ttc_min ↓70%; metric.drac ↑1%; metric.collision_rate ↑0% |
| `actors.vehicles.drivers.bus.time_headway_s` | 2.5 | traffic | 82 | ↑0% | ↑0% |  | ↓0% | ↓0% | ↑0% | ↓0% |  | ↓0% |  | ↓0% | ↓1% |  | metric.ttc_min ↓70%; report.apps.bsw.issued ↓2%; metric.app_warnings ↓1% |
| `actors.vehicles.drivers.bus.min_gap_m` | 2.5 | traffic | 80 | ↓0% | ↓0% |  | ↑0% | ↓0% | ↑0% | ↑0% |  | ↓0% |  | ↑0% | ↑0% |  | metric.ttc_min ↓70%; report.apps.bsw.issued ↑2%; metric.app_warnings ↑1% |
| `actors.vehicles.drivers.bus.min_gap_m` | 4.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.motorcycle.max_accel_mps2` | 2.0 | traffic | 90 | ↑0% | ↑0% |  | ↑1% | ↓0% | ↓0% | ↓0% |  | ↓0% |  | ↓0% | ↓1% |  | report.apps.fcw.false_warnings ↑6%; metric.ttc_min ↓5%; metric.app_precision ↓4% |
| `actors.vehicles.drivers.motorcycle.max_accel_mps2` | 5.0 | traffic | 101 | ↓2% | ↓2% |  | ↑1% | ↓3% | ↑0% | ↑3% |  | ↑0% |  | ↑4% | ↓10% |  | stage_access_ms ↑103%; access_ms ↑89%; report.mac_access_delay_ns ↑89% |
| `actors.vehicles.drivers.motorcycle.comfort_decel_mps2` | 2.5 | traffic | 75 | ↑0% | ↑0% |  | ↓0% | ↓0% | ↑0% | ↓0% |  | ↓0% |  | ↓0% | ↓0% |  | stage_verify_wait_ms ↑0%; metric.ttc_min ↓0%; prr_600_1000 ↑0% |
| `actors.vehicles.drivers.motorcycle.comfort_decel_mps2` | 4.0 | traffic | 65 | ↓0% |  |  | ↓0% |  | ↓0% | ↓0% |  | ↓0% |  | ↑0% | ↑0% |  | metric.ttc_min ↑0%; report.rx_losses.fading ↑0%; loss_fading ↑0% |
| `actors.vehicles.drivers.motorcycle.time_headway_s` | 0.8 | traffic | 76 | ↓0% | ↓0% |  | ↑0% |  | ↑0% | ↑0% |  | ↓0% |  | ↑0% | ↑0% |  | metric.ttc_min ↓22%; metric.drac ↑2%; metric.headway_time ↓0% |
| `actors.vehicles.drivers.motorcycle.time_headway_s` | 1.5 | traffic | 75 | ↑0% | ↑0% |  | ↑0% | ↓0% | ↑0% | ↓0% |  | ↓0% |  | ↓0% | ↓0% |  | metric.ttc_min ↑10%; metric.drac ↓1%; metric.headway_time ↑0% |
| `actors.vehicles.drivers.motorcycle.min_gap_m` | 1.0 | traffic | 74 | ↑0% | ↑0% |  | ↑0% |  | ↓0% | ↓0% |  | ↓0% |  | ↑0% |  |  | report.apps.bsw.issued ↓1%; metric.ttc_min ↓1%; metric.app_warnings ↓0% |
| `actors.vehicles.drivers.motorcycle.min_gap_m` | 2.0 | traffic | 69 | ↑0% |  |  | ↑0% | ↓0% | ↓0% | ↑0% |  | ↓0% |  | ↓0% | ↑0% |  | metric.ttc_min ↓8%; report.apps.fcw.false_warnings ↑6%; metric.app_precision ↓4% |
| `actors.vehicles.drivers.bicycle.max_accel_mps2` | 0.8 | pedestrians | 78 | ↑0% | ↑0% |  | ↓3% | ↓0% | ↓0% | ↓1% |  |  |  | ↓0% | ↑0% | ↓2% | metric.app_warnings ↓75%; report.apps.pcw.issued ↓75%; report.apps.pcw.false_warnings ↓50% |
| `actors.vehicles.drivers.bicycle.max_accel_mps2` | 1.5 | pedestrians | 67 | ↓0% | ↓0% | ↓33% | ↑0% | ↓0% | ↑0% | ↑2% |  |  |  | ↑0% |  | ↑1% | metric.ttc_min ↓14%; metric.headway_distance ↑6%; metric.headway_time ↑3% |
| `actors.vehicles.drivers.bicycle.comfort_decel_mps2` | 1.5 | pedestrians | 61 | ↓0% | ↓0% |  | ↓0% | ↓0% | ↑0% | ↑0% |  |  |  | ↓0% |  | ↓0% | metric.ttc_min ↓8%; metric.headway_time ↓6%; metric.drac ↓2% |
| `actors.vehicles.drivers.bicycle.comfort_decel_mps2` | 3.0 | pedestrians | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.bicycle.time_headway_s` | 1.0 | pedestrians | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.bicycle.time_headway_s` | 2.0 | pedestrians | 64 | ↑0% | ↑0% |  | ↑0% | ↑0% | ↓0% | ↓0% |  |  |  | ↓0% |  | ↑0% | metric.ttc_min ↑85%; metric.headway_distance ↑49%; metric.drac ↓20% |
| `actors.vehicles.drivers.bicycle.min_gap_m` | 0.5 | pedestrians | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.drivers.bicycle.min_gap_m` | 1.5 | pedestrians | 64 | ↓0% | ↓0% |  | ↓0% |  | ↑0% | ↑0% |  |  |  | ↑0% |  | ↓0% | metric.headway_time ↓16%; metric.ttc_min ↑4%; metric.headway_distance ↑1% |
| `actors.vehicles.gnss.rtk` | true | security | 27 |  |  |  |  |  |  |  |  |  |  |  |  |  | report.apps.ima.false_warnings new; report.apps.ima.issued new; report.apps.ima.lead_ms_sum new |
| `actors.vehicles.gnss.rtk` | false | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.gnss.outlier_rate_hz` | 0.0 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.gnss.outlier_rate_hz` | 0.1 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.gnss.outlier_magnitude_m` | 5.0 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.gnss.outlier_magnitude_m` | 30.0 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.gnss.burst_rate_hz` | 0.0 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.gnss.burst_rate_hz` | 0.5 | security | 61 |  |  |  |  | ↑1% | ↑0% | ↑0% |  | ↑1% |  |  |  |  | report.apps.ima.false_warnings new; report.apps.ima.issued new; report.apps.ima.lead_ms_sum new |
| `actors.vehicles.gnss.outage_probability` | 0.0 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vehicles.gnss.outage_probability` | 0.05 | security | 75 |  |  |  |  | ↓4% | ↓0% | ↓0% |  | ↓4% |  |  |  |  | report.events_by_class.NetDeliver new; report.phase2.access.exchange_latency.report-relay.count new; report.phase2.access.exchange_latency.report-relay.max_ns new |
| `actors.vru.pedestrians` | 50 | pedestrians | 116 | ↑7% | ↑8% | ↑150% | ↓30% | ↑1% | ↑7% | ↓13% | ↓21% | ↓6% |  | ↓1% | ↓6% | ↑20% | tx_phase_peak ↑79%; report.rx_losses.hidden-terminal ↓71%; report.rx_losses.fading ↓70% |
| `actors.vru.pedestrians` | 300 | pedestrians | 126 | ↓13% | ↓14% |  | ↑103% | ↑7% | ↓9% | ↑16% | ↑39% | ↑7% |  | ↓0% | ↓9% | ↓2% | metric.ped_wait new; report.apps.lta.false_warnings new; report.apps.lta.issued new |
| `actors.vru.cyclists` | 150 | pedestrians | 123 | ↑5% | ↑4% | ↑50% | ↑174% | ↑3% | ↓18% | ↓10% | ↑35% | ↑6% |  | ↓7% | ↓4% | ↑66% | metric.drac new; metric.headway_distance new; metric.headway_time new |
| `actors.vru.device_fraction` | 0.5 | pedestrians | 106 | ↓9% | ↓9% |  | ↑90% | ↓1% | ↓8% | ↑17% | ↑29% | ↑5% |  |  |  |  | metric.verify_wait ↑491%; stage_verify_wait_ms ↑367%; report.rx_losses.hidden-terminal ↑307% |
| `actors.vru.behaviour` | document | pedestrians | 92 | ↑3% | ↑2% |  | ↓9% | ↑4% | ↑2% | ↑8% |  | ↓0% |  | ↓1% | ↓7% | ↑4% | metric.app_warnings ↑150%; report.apps.pcw.issued ↑150%; metric.ped_midblock_share ↓100% |
| `actors.vru.midblock_rate_per_100m` | 0.0 | pedestrians | 61 | ↓0% | ↓0% |  | ↓1% |  | ↓0% | ↑0% |  |  |  | ↑0% |  | ↑0% | metric.ped_midblock_share ↓100%; report.rx_losses.fading ↓3%; loss_fading ↓3% |
| `actors.vru.midblock_rate_per_100m` | 100.0 | pedestrians | 114 | ↓3% | ↓3% |  | ↓15% | ↓2% | ↑0% | ↓1% | ↓1% | ↑0% |  | ↓4% | ↑7% | ↓0% | metric.ped_wait new; metric.ped_midblock_share ↑1844%; metric.app_warnings ↑100% |
| `actors.vru.red_crossing_share` | 0.0 | pedestrians | 53 | ↓0% | ↑0% |  | ↓0% |  | ↓0% | ↑0% |  |  |  |  |  | ↓0% | metric.aoi ↑3%; vru.stopped_share ↑2%; report.rx_losses.fading ↑1% |
| `actors.vru.red_crossing_share` | 1.0 | pedestrians | 72 | ↓1% | ↓1% |  | ↓13% | ↓1% | ↑1% | ↑1% |  |  |  | ↓4% | ↑8% | ↑10% | metric.ped_wait new; vru.stopped_share ↓51%; metric.app_warnings ↑50% |
| `actors.vru.ebike_share` | 0.0 | pedestrians | 6 |  |  |  |  |  |  |  |  |  |  |  |  | ↓1% | metric.flow ↓2%; metric.mean_speed ↓2%; metric.speed ↓2% |
| `actors.vru.ebike_share` | 1.0 | pedestrians | 61 | ↑0% | ↑0% |  | ↑0% | ↓0% | ↑0% | ↓0% |  |  |  | ↓1% |  | ↑1% | metric.mean_speed ↑2%; metric.speed ↑2%; metric.flow ↑2% |
| `actors.vru.crossing_gap_margin_s` | 0.5 | pedestrians | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vru.crossing_gap_margin_s` | 6.0 | pedestrians | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.vru.walking_speed_mps` | 1.2 | pedestrians | 77 | ↑1% | ↑1% | ↑50% | ↓12% | ↑1% | ↑0% | ↑7% |  |  |  | ↓0% |  | ↓17% | metric.ped_conflicts ↑1.000; metric.app_warnings ↑50%; report.apps.pcw.false_warnings ↑50% |
| `actors.vru.walking_speed_mps` | 1.6 | pedestrians | 70 | ↓0% | ↓0% |  | ↓7% | ↓0% | ↑0% | ↑11% |  |  |  | ↑0% |  | ↑5% | metric.ped_midblock_share ↑30%; stage_access_ms ↑7%; report.rx_losses.hidden-terminal ↓7% |
| `actors.rsu_placement.count` | 0 | traffic | 120 | ↓1% | ↓1% |  | ↑10% | ↓0% | ↓0% | ↑9% |  | ↓0% |  |  |  |  | loss_collision new; metric.backend_link_up gone; metric.cert_pool_valid gone |
| `actors.rsu_placement.count` | 20 | traffic | 127 | ↑2% | ↑1% |  | ↑20% | ↑2% | ↑0% | ↑5% | ↓0% | ↓0% |  |  |  |  | report.apps.ima.lead_ms_sum ↑3326.000; report.apps.fcw.missed ↑1.000; report.apps.ima.lead_n ↑1.000 |
| `actors.rsu_placement.profile` | rsu/cohda-mk5-rsu | traffic | 27 |  |  |  |  |  | ↓0% | ↑3% |  |  |  |  |  |  | metric.verify_wait ↓85%; stage_verify_wait_ms ↓55%; delivery_ratio ↓13% |
| `actors.rsu_placement.profile` | rsu/commsignia-its-rs4 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `actors.rsu_placement.min_spacing_m` | 0.0 | traffic | 57 | ↑3% | ↑3% |  | ↑3% | ↑5% | ↑0% | ↓1% |  |  |  |  |  |  | metric.verify_wait ↓16%; stage_verify_wait_ms ↓8%; cbr_max ↑7% |
| `actors.rsu_placement.min_spacing_m` | 5000.0 | traffic | 115 | ↓0% | ↓0% |  | ↑7% | ↑3% | ↓0% | ↑11% |  | ↓0% |  |  |  |  | loss_collision new; report.rx_losses.collision new; report.apps.ima.lead_ms_sum ↑3326.000 |
| `weather.initial` | rain | traffic | 130 | ↓2% | ↓2% |  | ↑2% | ↓3% | ↓0% | ↑4% | ↓0% | ↓0% |  | ↓8% | ↓10% |  | loss_collision gone; report.apps.lta.false_warnings gone; report.apps.lta.issued gone |
| `weather.initial` | snow | traffic | 128 | ↓3% | ↓3% |  | ↑2% | ↓3% | ↓0% | ↑5% | ↓0% | ↓0% |  | ↓23% | ↓4% |  | loss_collision gone; report.apps.lta.false_warnings gone; report.apps.lta.issued gone |
| `weather.initial` | sleet | traffic | 130 | ↓3% | ↓3% |  | ↑1% | ↓3% | ↓0% | ↑5% | ↓0% | ↓0% |  | ↓23% | ↓4% |  | loss_collision gone; metric.app_lead_time_s gone; report.apps.lta.false_warnings gone |
| `weather.initial` | fog | traffic | 128 | ↓2% | ↓2% |  | ↑2% | ↓2% | ↓0% | ↑4% | ↓0% | ↑0% |  | ↓8% | ↓10% |  | loss_collision gone; report.apps.lta.false_warnings gone; report.apps.lta.issued gone |
| `weather.initial` | wind | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `weather.intensity` | 0.3 | traffic | 53 | ↓0% | ↓0% |  | ↓0% | ↓0% | ↑0% | ↑0% |  |  |  |  |  |  | report.faint_arrivals ↑1%; report.rx_losses.fading ↓1%; loss_fading ↓1% |
| `weather.intensity` | 0.8 | traffic | 56 | ↓0% | ↓0% |  | ↓0% | ↓0% | ↑0% | ↑1% |  |  |  |  |  |  | report.faint_arrivals ↑3%; report.rx_losses.fading ↓2%; loss_fading ↓2% |
| `weather.surface` | dry | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `weather.surface` | wet | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `weather.surface` | flooded | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `weather.surface` | snow | traffic | 96 | ↓0% | ↓0% |  | ↓0% | ↑0% | ↑0% | ↑0% |  | ↑0% |  | ↑0% | ↑0% |  | report.apps.fcw.missed ↑1.000; metric.app_miss_ratio ↑0.167; report.apps.ima.true_warnings ↓100% |
| `weather.surface` | ice | traffic | 130 | ↑0% | ↑0% |  | ↑2% | ↑2% | ↓0% | ↑1% | ↓0% | ↑0% |  | ↑1% | ↑4% |  | loss_collision gone; metric.pet new; report.apps.eebl.false_warnings new |
| `radio.rat` | lte-v2x-pc5 | dsrc | 133 | ↑7% | ↑5% | ↑25% | ↑12% | ↑190% | ↑370% | ↑56% | ↑94% | ↓18% |  |  |  |  | loss_below-sensitivity gone; loss_collision new; loss_hidden-terminal gone |
| `radio.rat` | nr-v2x-pc5 | dsrc | 134 | ↑3% | ↑2% | ↑3% | ↑82% | ↓3% | ↑136% | ↑18% | ↓1% | ↓18% |  |  |  |  | loss_below-sensitivity gone; loss_collision new; loss_hidden-terminal gone |
| `radio.rat` | hybrid | refused | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.tiers.propagation` | abstract | refused | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.tiers.propagation` | medium | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.tiers.phy` | abstract | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.tiers.phy` | high | dsrc | 42 | ↓0% | ↓0% |  | ↓43% |  | ↑0% | ↑1% |  |  |  |  |  |  | loss_preamble-missed new; report.rx_losses.preamble-missed new; metric.collision_rate ↓43% |
| `radio.tiers.mac` | abstract | dsrc | 74 | ↓9% | ↓9% | ↓41% | ↑215% | gone | ↑21% | ↑22% |  |  |  |  |  |  | cbr_max gone; loss_collision new; metric.cbr gone |
| `radio.tiers.mac` | high | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.obu.tx_power_dbm` | 1.0 | dsrc | 60 | ↓69% | ↓68% | ↓69% | ↓30% | ↓84% | ↓0% | ↑41% |  |  |  |  |  |  | metric.per ↑2088%; loss_below-sensitivity ↑965%; report.rx_losses.below-sensitivity ↑965% |
| `radio.devices.obu.antenna_gain_dbi` | 1.5 | dsrc | 56 | ↓5% | ↓5% |  | ↑29% | ↓10% | ↓0% | ↑2% |  |  |  |  |  |  | loss_fading ↑67%; report.rx_losses.fading ↑67%; loss_below-sensitivity ↑50% |
| `radio.devices.obu.antenna_gain_dbi` | 6.0 | dsrc | 60 | ↑7% | ↑6% | ↑38% | ↓54% | ↑18% | ↓0% | ↓6% |  |  |  |  |  |  | report.rx_losses.below-sensitivity ↓63%; loss_below-sensitivity ↓63%; metric.per ↓58% |
| `radio.devices.obu.cable_loss_db` | 1.0 | dsrc | 55 | ↓3% | ↓3% |  | ↑24% | ↓6% | ↓0% | ↑2% |  |  |  |  |  |  | loss_fading ↑46%; report.rx_losses.fading ↑46%; loss_below-sensitivity ↑32% |
| `radio.devices.obu.antenna_height_m` | 0.1 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.obu.antenna_height_m` | 15.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.obu.antenna_pattern` | tr37885 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.obu.antenna_pattern` | rooftop | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.obu.antenna_pattern` | isotropic | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.rsu.tx_power_dbm` | 11.5 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.rsu.tx_power_dbm` | 33.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.rsu.antenna_gain_dbi` | 1.5 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.rsu.antenna_gain_dbi` | 6.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.rsu.cable_loss_db` | 1.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.rsu.antenna_height_m` | 0.1 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.rsu.antenna_height_m` | 15.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.vru.tx_power_dbm` | 11.5 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.vru.tx_power_dbm` | 33.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.vru.antenna_gain_dbi` | -1.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.vru.antenna_gain_dbi` | 1.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.vru.cable_loss_db` | 1.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.vru.antenna_height_m` | 0.75 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.devices.vru.antenna_height_m` | 3.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.range.margin_db` | 5.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.range.margin_db` | 20.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.range.max_m` | 200.0 | dsrc | 67 | ↑10% | ↓91% | ↓69% | ↓65% | ↓89% | ↓1% | ↓10% |  |  |  |  |  |  | loss_below-sensitivity gone; loss_collision new; loss_fading gone |
| `radio.range.max_m` | 2000.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.region` | us | cv2x | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.region` | us-2016 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.region` | eu | dsrc | 68 | ↑10% | ↑8% | ↑56% | ↓68% | ↑34% | ↑20% | ↓5% |  |  |  |  |  |  | stage_handoff_ms ↑98%; metric.unverified_ratio ↓96%; metric.p2pcd_requests ↓95% |
| `radio.phy.data_rate_mbps` | 3.0 | dsrc | 64 | ↓0% | ↓1% | ↓16% | ↑111% | ↑84% | ↑3% | ↑3% |  |  |  |  |  |  | stage_access_ms ↑162%; report.mac_access_delay_ns ↑159%; access_ms ↑159% |
| `radio.phy.data_rate_mbps` | 18.0 | dsrc | 65 | ↓37% | ↓35% | ↓44% | ↓71% | ↓58% | ↓2% | ↑11% |  |  |  |  |  |  | report.rx_losses.fading ↑1106%; loss_fading ↑1106%; loss_below-sensitivity ↑528% |
| `radio.phy.noise_figure_db` | 3.0 | dsrc | 38 | ↑0% | ↑0% |  | ↓1% |  | ↑0% | ↓0% |  |  |  |  |  |  | loss_fading gone; report.rx_losses.fading gone; metric.pdr_by_cause ↑33% |
| `radio.phy.noise_figure_db` | 9.0 | dsrc | 40 | ↓8% | ↓7% | ↓6% | ↑2% |  | ↓0% | ↑1% |  |  |  |  |  |  | loss_fading ↑4063%; report.rx_losses.fading ↑4063%; metric.loss_rate ↑56% |
| `radio.phy.sensitivity_dbm` | -92.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.phy.sensitivity_dbm` | -85.0 | dsrc | 47 | ↓25% | ↓23% | ↓38% | ↓41% |  | ↓0% | ↑9% |  |  |  |  |  |  | loss_fading gone; report.rx_losses.fading gone; loss_below-sensitivity ↑375% |
| `radio.phy.cca_threshold_dbm` | -85.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.phy.cca_threshold_dbm` | -65.0 | dsrc | 59 | ↓12% | ↓12% | ↓50% | ↑258% | ↓5% | ↓0% | ↑16% |  |  |  |  |  |  | metric.per ↑801%; metric.collision_rate ↑258%; loss_hidden-terminal ↑258% |
| `radio.phy.capture_threshold_db` | 4.0 | dsrc | 42 | ↑0% | ↑0% | ↑9% | ↑1% |  | ↑0% | ↓0% |  |  |  |  |  |  | loss_fading gone; report.rx_losses.fading gone; metric.pdr_by_cause ↑33% |
| `radio.phy.capture_threshold_db` | 10.0 | dsrc | 46 | ↓13% | ↓12% | ↓22% | ↑29% |  | ↑0% | ↑7% |  |  |  |  |  |  | loss_fading gone; report.rx_losses.fading gone; loss_below-sensitivity ↑162% |
| `radio.mac.periodic_access_category` | vo | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.mac.periodic_access_category` | vi | dsrc | 57 | ↑0% | ↑0% | ↑9% | ↓7% | ↓0% | ↑0% | ↓1% |  |  |  |  |  |  | backoff_us ↑140%; stage_verify_wait_ms ↓48%; metric.verify_wait ↓43% |
| `radio.mac.periodic_access_category` | be | dsrc | 55 | ↓0% | ↓0% | ↑6% | ↓1% | ↓0% | ↑0% | ↓1% |  |  |  |  |  |  | backoff_us ↑410%; aifs_us ↑88%; metric.verify_wait ↓72% |
| `radio.mac.periodic_access_category` | bk | dsrc | 58 | ↓0% | ↓0% |  | ↑6% | ↑0% | ↑0% | ↓0% |  |  |  |  |  |  | backoff_us ↑413%; aifs_us ↑153%; stage_access_ms ↑89% |
| `radio.mac.event_access_category` | vo | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.mac.event_access_category` | vi | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.mac.event_access_category` | be | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.mac.event_access_category` | bk | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.mac.cw_min_vo` | 3 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.mac.cw_min_vo` | 15 | dsrc | 54 | ↓0% | ↓0% |  | ↓0% | ↑1% | ↑0% | ↑0% |  |  |  |  |  |  | backoff_us ↑423%; stage_verify_wait_ms ↓53%; metric.verify_wait ↓43% |
| `radio.mac.aifsn_vo` | 2 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.mac.aifsn_vo` | 15 | dsrc | 57 | ↓0% | ↓0% | ↑6% | ↑0% | ↑0% | ↑0% | ↓1% |  |  |  |  |  |  | aifs_us ↑285%; stage_access_ms ↑99%; metric.mac_access_delay ↑98% |
| `radio.mac.cw_min_vi` | 1 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.mac.cw_min_vi` | 1023 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.mac.aifsn_vi` | 2 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.mac.aifsn_vi` | 15 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.mac.queue_length` | 16 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.mac.queue_length` | 64 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.channel_model.path_loss_exponent` | 1.8 | dsrc | 59 | ↑10% | ↑9% | ↑56% | ↓77% | ↑30% | ↓0% | ↓9% |  |  |  |  |  |  | loss_below-sensitivity ↓88%; report.rx_losses.below-sensitivity ↓88%; metric.unverified_ratio ↓87% |
| `radio.channel_model.path_loss_exponent` | 3.5 | dsrc | 64 | ↓88% | ↓88% | ↓78% | ↓92% | ↓93% | ↓0% | ↑15% |  |  |  |  |  |  | report.faint_arrivals ↑1002.000; metric.per ↑8316%; loss_below-sensitivity ↑1275% |
| `radio.channel_model.shadowing_sigma_db` | 3.0 | dsrc | 55 | ↑2% | ↑2% | ↑16% | ↑2% | ↑2% | ↓0% | ↓1% |  |  |  |  |  |  | loss_below-sensitivity ↓32%; report.rx_losses.below-sensitivity ↓32%; metric.per ↓28% |
| `radio.channel_model.shadowing_sigma_db` | 7.0 | dsrc | 59 | ↓7% | ↓6% | ↓19% | ↑6% | ↓6% | ↓0% | ↑0% |  |  |  |  |  |  | metric.per ↑128%; loss_below-sensitivity ↑87%; report.rx_losses.below-sensitivity ↑87% |
| `radio.channel_model.shadowing_decorrelation_m` | 10.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.channel_model.shadowing_decorrelation_m` | 50.0 | dsrc | 53 | ↓0% | ↓0% | ↑9% | ↑4% | ↓0% | ↑0% | ↑0% |  |  |  |  |  |  | metric.collision_rate ↑4%; loss_hidden-terminal ↑4%; report.rx_losses.hidden-terminal ↑4% |
| `radio.channel_model.fading` | none | dsrc | 60 | ↑4% | ↑3% | ↑25% | ↓20% | ↑7% | ↓0% | ↓6% |  |  |  |  |  |  | ipg_ms_p95 ↓47%; metric.per ↓45%; metric.unverified_ratio ↓41% |
| `radio.channel_model.fading` | rayleigh | dsrc | 63 | ↓9% | ↓9% | ↓22% | ↑13% | ↓11% | ↓0% | ↑16% |  |  |  |  |  |  | report.faint_arrivals ↑5.000; metric.per ↑259%; loss_below-sensitivity ↑119% |
| `radio.channel_model.fading` | rice | dsrc | 53 | ↓0% | ↓0% |  | ↑3% | ↓1% | ↑0% | ↑1% |  |  |  |  |  |  | loss_below-sensitivity ↑4%; report.rx_losses.below-sensitivity ↑4%; metric.per ↑4% |
| `radio.channel_model.fading` | nakagami | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.channel_model.nakagami_shape` | 1.0 | dsrc | 63 | ↓9% | ↓9% | ↓22% | ↑13% | ↓11% | ↓0% | ↑16% |  |  |  |  |  |  | report.faint_arrivals ↑5.000; metric.per ↑259%; loss_below-sensitivity ↑119% |
| `radio.channel_model.nakagami_shape` | 5.0 | dsrc | 58 | ↑2% | ↑1% | ↑16% | ↓4% | ↑2% | ↓0% | ↓3% |  |  |  |  |  |  | ipg_ms_p95 ↓47%; metric.per ↓38%; report.rx_losses.below-sensitivity ↓19% |
| `radio.channel_model.rice_k_db` | 3.0 | dsrc | 59 | ↓3% | ↓3% |  | ↑9% | ↓3% | ↓0% | ↑4% |  |  |  |  |  |  | metric.per ↑56%; report.rx_losses.fading ↑45%; loss_fading ↑45% |
| `radio.channel_model.rice_k_db` | 10.0 | dsrc | 59 | ↑2% | ↑2% | ↑16% | ↓7% | ↑3% | ↓0% | ↓4% |  |  |  |  |  |  | ipg_ms_p95 ↓47%; metric.per ↓40%; report.rx_losses.below-sensitivity ↓25% |
| `radio.channel_model.nakagami_shape_far` | 0.75 | dsrc | 61 | ↓3% | ↓3% | ↓11% | ↓2% | ↓5% | ↓0% | ↑3% |  |  |  |  |  |  | report.faint_arrivals ↑200%; metric.per ↑44%; metric.unverified_ratio ↑27% |
| `radio.channel_model.nakagami_shape_far` | 1.0 | dsrc | 61 | ↑3% | ↑3% | ↑32% | ↓12% | ↑5% | ↑0% | ↓4% |  |  |  |  |  |  | report.faint_arrivals ↓69%; metric.per ↓22%; report.rx_losses.fading ↑16% |
| `radio.channel_model.nakagami_breakpoint_m` | 80.0 | dsrc | 33 | ↓0% | ↓0% |  | ↑0% |  | ↑0% | ↑0% |  |  |  |  |  |  | metric.per ↑1%; stage_verify_wait_ms ↓0%; metric.aoi_peak ↑0% |
| `radio.channel_model.nakagami_breakpoint_m` | 100.0 | dsrc | 34 | ↑0% | ↑0% |  | ↓0% |  | ↑0% | ↓0% |  |  |  |  |  |  | metric.per ↓0%; stage_verify_wait_ms ↑0%; metric.aoi_peak ↓0% |
| `radio.channel_model.building_wall_loss_db` | 2.4 | traffic | 65 | ↑2% | ↑2% |  | ↓4% | ↑5% | ↑0% | ↓1% |  |  |  |  |  |  | report.rx_losses.collision ↑50%; loss_collision ↑50%; cbr_max ↑13% |
| `radio.channel_model.building_wall_loss_db` | 9.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.channel_model.building_loss_db_per_m` | 0.1 | traffic | 67 | ↑6% | ↑6% | ↑25% | ↑1% | ↑10% | ↑0% | ↓2% |  |  |  |  |  |  | report.rx_losses.collision ↑50%; loss_collision ↑50%; ipg_ms_p95 ↓32% |
| `radio.channel_model.building_loss_db_per_m` | 0.63 | traffic | 57 | ↓0% | ↓0% |  | ↓0% | ↓0% | ↑0% | ↑0% |  |  |  |  |  |  | metric.p2pcd_requests ↓1%; backoff_us ↓0%; metric.collision_rate ↓0% |
| `radio.channel_model.vehicle_blockage` | true | traffic | 51 | ↓0% | ↓0% |  | ↑0% | ↓0% | ↓0% | ↑0% |  |  |  |  |  |  | prr_400_600 ↓0%; stage_verify_wait_ms ↓0%; prr_600_1000 ↓0% |
| `radio.channel_model.vehicle_blockage` | false | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.enabled` | true | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.enabled` | false | dsrc | 67 | ↑1% | ↑1% | ↑16% | ↓15% | ↑2% | ↑22% | ↑6% |  |  |  |  |  |  | access_ms ↑382%; report.mac_access_delay_ns ↑382%; metric.mac_access_delay ↑380% |
| `radio.dcc.cbr_target` | 0.6 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.cbr_target` | 0.7 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.adaptive_alpha` | 0.001 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.adaptive_alpha` | 0.5 | dsrc | 92 | ↑0% | ↑0% |  | ↑2% | ↓93% | ↑4% | ↑97% | ↓45% | ↑5% |  |  |  |  | stage_access_ms ↑304%; metric.mac_access_delay ↑289%; access_ms ↑288% |
| `radio.dcc.adaptive_beta` | 0.0001 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.adaptive_beta` | 0.05 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.max_itt_ms` | 100.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.max_itt_ms` | 2000.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.density_coefficient` | 1.0 | dsrc | 91 | ↑0% | ↑0% | ↑9% | ↓4% | ↓8% | ↑0% | ↑4% | ↓3% | ↑0% |  |  |  |  | metric.cbr ↓8%; report.rx_losses.hidden-terminal ↓7%; report.rx_losses.half-duplex ↓7% |
| `radio.dcc.density_coefficient` | 200.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.power_control` | true | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.power_control` | false | dsrc | 67 | ↑1% | ↑1% | ↑16% | ↓15% | ↑2% | ↑22% | ↑6% |  |  |  |  |  |  | access_ms ↑382%; report.mac_access_delay_ns ↑382%; metric.mac_access_delay ↑380% |
| `radio.dcc.min_power_dbm` | -10.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.min_power_dbm` | 20.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.reactive_table` | a1 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.reactive_table` | a2 | dsrc | 93 | ↑1% | ↑1% | ↑9% | ↓20% | ↓2% | ↓11% | ↓5% | ↑1% | ↓0% |  |  |  |  | access_ms ↓57%; metric.mac_access_delay ↓57%; report.mac_access_delay_ns ↓57% |
| `radio.dcc.reactive_hold_ms` | 0.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.dcc.reactive_hold_ms` | 1000.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.sidelink.reservation_interval_ms` | 10 | nr | 112 | ↑7% | ↑7% | ↑52% | ↓80% | ↑14% | ↓31% | ↓10% | ↓7% | ↓4% |  |  |  |  | report.sidelink.selections.latency-budget gone; mac_drops ↑60.000; metric.mac_drops ↑60.000 |
| `radio.sidelink.reservation_interval_ms` | 100 | cv2x | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.sidelink.resource_keep_probability` | 0.0 | cv2x | 93 | ↑2% | ↑1% | ↑25% | ↓34% | ↓11% | ↑8% | ↑7% |  | ↓0% |  |  |  |  | metric.per ↑741%; loss_in-band-emission ↑300%; report.rx_losses.in-band-emission ↑300% |
| `radio.sidelink.resource_keep_probability` | 0.8 | cv2x | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.sidelink.rsrp_threshold_dbm` | -128.0 | cv2x | 96 | ↓0% | ↓0% |  | ↑0% | ↑1% | ↑1% | ↑0% | ↓0% | ↑0% |  |  |  |  | report.sidelink.combined_decodes ↑7%; metric.verify_wait ↓7%; stage_verify_wait_ms ↓6% |
| `radio.sidelink.rsrp_threshold_dbm` | -110.0 | cv2x | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.sidelink.sensing` | true | cv2x | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.sidelink.sensing` | false | cv2x | 111 | ↓1% | ↓1% | ↑8% | ↑17% | ↓5% | ↑12% | ↑12% | ↓0% | ↑0% |  |  |  |  | loss_in-band-emission ↑802%; report.rx_losses.in-band-emission ↑800%; metric.per ↑754% |
| `radio.sidelink.candidate_percent` | 20 | cv2x | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.sidelink.candidate_percent` | 50 | nr | 83 | ↑5% | ↑6% | ↑52% | ↓64% | ↑8% | ↓6% | ↓6% |  |  |  |  |  |  | loss_resource-collision ↓96%; report.rx_losses.resource-collision ↓96%; loss_in-band-emission ↓96% |
| `radio.sidelink.selection_window_ms` | 10 | nr | 104 | ↑4% | ↑4% | ↑52% | ↓41% | ↑2% | ↓2% | ↓4% | ↑0% | ↓0% |  |  |  |  | report.sidelink.selections.preemption ↑400%; e2e_ms_p95 ↑225%; report.sidelink.selections.latency-budget ↑200% |
| `radio.sidelink.selection_window_ms` | 100 | cv2x | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.sidelink.noise_figure_db` | 3.0 | cv2x | 41 | ↑0% | ↑0% |  | ↓1% |  | ↓0% | ↓0% |  |  |  |  |  |  | loss_collision ↓3%; report.rx_losses.collision ↓3%; metric.p2pcd_requests ↑2% |
| `radio.sidelink.noise_figure_db` | 9.0 | cv2x | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `radio.sidelink.noise_figure_db` | 15.0 | cv2x | 50 | ↓0% | ↓0% |  | ↑3% |  | ↑0% | ↑0% |  |  |  |  |  |  | loss_fading ↑1075%; report.rx_losses.fading ↑1075%; loss_in-band-emission ↑200% |
| `radio.sidelink.cbr_threshold_dbm` | -112.0 | cv2x | 6 |  |  |  |  | ↑102% |  |  |  |  |  |  |  |  | cbr_max ↑114%; report.sidelink.max_cbr_at_grant_pm ↑107%; metric.cbr ↑102% |
| `radio.sidelink.cbr_threshold_dbm` | -80.0 | cv2x | 6 |  |  |  |  | ↓50% |  |  |  |  |  |  |  |  | metric.cbr ↓50%; sl_cbr ↓49%; report.sidelink.mean_cbr_at_grant_pm ↓49% |
| `net.layer` | gn-btp | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.backhaul.id` | backhaul/fixed | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.backhaul.id` | backhaul/fibre | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.backhaul.id` | backhaul/dsl | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.backhaul.id` | backhaul/microwave | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.backhaul.id` | backhaul/cellular | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.backhaul.id` | backhaul/none | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.backend_net.id` | backend-net/fixed | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.forwarding` | on-air | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.forwarding` | overlay | gn | 111 | ↓0% | ↓0% |  | ↑1% | ↑0% | ↓1% | ↓0% | ↓0% | ↓0% |  |  |  |  | gn_forward_airtime_us gone; gn_forwards_sent gone; report.gn_air.cbf_armed gone |
| `net.geonet.area_forwarding` | cbf | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.area_forwarding` | simple | gn | 110 | ↓1% | ↓1% | ↓6% | ↑32% | ↓0% | ↑1% | ↑0% | ↑2% | ↑1% |  |  |  |  | report.gn_air.drops.CbfCancelled gone; report.mac_access_delay_ns ↑1217%; gn_forwarded ↑1200% |
| `net.geonet.non_area_forwarding` | greedy | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.non_area_forwarding` | cbf | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.cbf_variant` | etsi | gn | 108 | ↓0% | ↓0% |  | ↑6% | ↓0% | ↑2% | ↑0% | ↑1% | ↑0% |  |  |  |  | report.gn_air.drops.Duplicate gone; report.gn_air.drops.HopLimit new; report.gn_air.drops.CbfCancelled ↑459% |
| `net.geonet.cbf_variant` | dpd | gn | 100 | ↓0% | ↓0% |  | ↑0% | ↓0% | ↓0% | ↓0% | ↓0% | ↓0% |  |  |  |  | report.gn_air.drops.Duplicate ↓37%; gn_forward_airtime_us ↓22%; report.gn_air.forward_airtime_us ↓22% |
| `net.geonet.cbf_variant` | gpc | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.hop_limit` | 1 | gn | 110 | ↓0% | ↓0% |  | ↑1% | ↑0% | ↓1% | ↓0% | ↓0% | ↓0% |  |  |  |  | report.gn_air.drops.CbfCancelled gone; report.gn_air.drops.Duplicate gone; report.gn_air.drops.HopLimit new |
| `net.geonet.hop_limit` | 255 | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.area_radius_m` | 1784.0 | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.range_m` | 3000.0 | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.repetition_interval_ms` | 0 | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.repetition_interval_ms` | 60000 | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.repetition_duration_ms` | 0 | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.repetition_duration_ms` | 600000 | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.keep_alive_forwarding` | true | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.keep_alive_forwarding` | false | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.window_s` | 600.0 | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.cbf_min_time_ms` | 1 | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.cbf_min_time_ms` | 4 | gn | 57 | ↓0% | ↓0% |  | ↑2% | ↓0% | ↓0% | ↑0% |  |  |  |  |  |  | report.gn_air.drops.Duplicate ↓22%; report.geonet.0.latency_p95_us ↑13%; metric.per ↑2% |
| `net.geonet.cbf_max_time_ms` | 20 | gn | 102 | ↑0% | ↑0% |  | ↑0% | ↑0% | ↓0% | ↓0% | ↓0% | ↓0% |  |  |  |  | report.geonet.0.latency_p95_us ↓27%; report.gn_air.drops.Duplicate ↓17%; gn_forward_airtime_us ↓12% |
| `net.geonet.cbf_max_time_ms` | 100 | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.geonet.max_comm_range_m` | 300.0 | gn | 109 | ↓1% | ↓1% | ↓3% | ↑24% | ↓0% | ↑1% | ↓0% | ↑1% | ↑1% |  |  |  |  | report.rx_losses.collision ↑1113%; loss_collision ↑1098%; gn_forwarded ↑967% |
| `net.geonet.max_comm_range_m` | 1000.0 | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.channel_access` | continuous | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.channel_access` | alternating | dsrc | 101 | ↓46% | ↓45% | ↓78% | ↑909% | ↑81% | ↑152% | ↑95% | ↓3% | ↑0% |  |  |  |  | loss_collision new; report.rx_losses.collision new; metric.mac_queue_depth ↑0.113 |
| `net.access_categories` | single | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.access_categories` | etsi-profiles | gn | 60 | ↓0% | ↓0% |  | ↑2% | ↓0% | ↑0% | ↑0% |  |  |  |  |  |  | loss_collision gone; report.rx_losses.collision gone; backoff_us ↑414% |
| `net.dcc_cbr` | local | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `net.dcc_cbr` | global | gn | 3 |  |  |  |  |  |  |  |  |  |  |  |  |  | report.gcbr.max_rise_permille new; report.gcbr.raised new; report.gcbr.updates new |
| `messages.codec_tier` | size-model | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `messages.rate_hz` | 1.0 | dsrc | 96 | ↑4% | ↑4% | ↑9% | ↓100% | ↓76% | ↓0% | ↑353% | ↓86% | ↑34% |  |  |  |  | loss_half-duplex gone; loss_hidden-terminal gone; report.rx_losses.half-duplex gone |
| `messages.rate_hz` | 20.0 | dsrc | 98 | ↓5% | ↓5% | ↓26% | ↑118% | ↑106% | ↑0% | ↓38% | ↑100% | ↓3% |  |  |  |  | metric.mac_queue_depth ↑0.013; queue_depth_mean ↑0.013; report.mac_access_delay_ns ↑613% |
| `messages.cpm_interval_ms` | 100 | gn | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `messages.cpm_interval_ms` | 1000 | gn | 119 | ↑0% | ↑0% | ↑3% | ↓2% | ↓21% | ↓8% | ↑2% | ↓14% | ↑1% |  |  |  |  | metric.mac_queue_depth ↑0.001; queue_depth_mean ↑0.001; loss_collision ↑120% |
| `messages.path_history_points` | 0 | dsrc | 74 | ↑1% | ↑1% | ↑16% | ↓20% | ↓5% | ↓0% | ↓1% |  | ↓5% |  |  |  |  | stage_verify_wait_ms ↑60%; metric.verify_wait ↑57%; ipg_ms_p95 ↓47% |
| `messages.path_history_points` | 23 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.envelope` | etsi103097 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.signature` | ecdsa-brainpoolp256r1 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.signature` | hybrid-falcon512-ecdsa-p256 | security | 132 | ↓2% | ↓2% | ↓25% | ↑36% | ↑286% | ↑78% | ↓9% | ↑10% | ↑352% |  |  |  |  | metric.frag_content_loss new; metric.frag_content_loss_predicted new; metric.frag_fragment_loss new |
| `security.signature` | hybrid-mldsa65-ecdsa-p256 | security | 121 | ↓4% | ↓3% |  | ↑680% | ↑1579% | ↑319% | ↓9% | ↑241% | ↑487% |  |  |  |  | loss_collision new; metric.frag_content_loss new; metric.frag_content_loss_predicted new |
| `security.signature` | ml-dsa-65 | security | 121 | ↓3% | ↓3% |  | ↑661% | ↑1541% | ↑293% | ↓21% | ↑241% | ↑477% |  |  |  |  | loss_collision new; metric.frag_content_loss new; metric.frag_content_loss_predicted new |
| `security.signature` | falcon-512 | security | 117 | ↓1% | ↓2% | ↓25% | ↑6% | ↑266% | ↑52% | ↓9% | ↑10% | ↑319% |  |  |  |  | metric.frag_content_loss new; metric.frag_content_loss_predicted new; metric.frag_fragment_loss new |
| `security.signature` | slh-dsa-sha2-128s | security | 151 | ↓12% | ↓12% | ↑50% | ↑56% | ↓100% | ↑358423% | ↑3978% | ↓89% | ↑558% |  |  |  |  | metric.aoi_peak gone; metric.app_lead_time_s gone; metric.app_precision gone |
| `security.kem` | ml-kem-512 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.kem` | ml-kem-768 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.kem` | ml-kem-1024 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.kem` | hybrid-ecies-p256-ml-kem-768 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.threshold.preprocessed` | true | security | 13 |  |  |  |  |  |  |  |  |  |  |  |  |  | report.phase2.committees.pca.bytes new; report.phase2.committees.pca.completed new; report.phase2.committees.pca.exclusions new |
| `security.threshold.preprocessed` | false | security | 13 |  |  |  |  |  |  |  |  |  |  |  |  |  | report.phase2.committees.pca.bytes new; report.phase2.committees.pca.completed new; report.phase2.committees.pca.exclusions new |
| `security.threshold.ask_all` | true | security | 13 |  |  |  |  |  |  |  |  |  |  |  |  |  | report.phase2.committees.pca.bytes new; report.phase2.committees.pca.completed new; report.phase2.committees.pca.exclusions new |
| `security.threshold.ask_all` | false | security | 13 |  |  |  |  |  |  |  |  |  |  |  |  |  | report.phase2.committees.pca.bytes new; report.phase2.committees.pca.completed new; report.phase2.committees.pca.exclusions new |
| `security.crypto_mode` | real | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.verification_policy` | on-demand | security | 30 |  |  |  |  |  | ↓3% | ↑46% |  |  |  |  |  |  | metric.verify_cost gone; metric.verify_wait gone; report.phase2.verification_states.Unverified new |
| `security.verification_policy` | prioritized | security | 21 |  |  |  |  |  | ↓0% | ↑46% |  |  |  |  |  |  | report.phase2.verification_states.Unverified new; metric.aoi ↑49%; ipg_ms_p95 ↑48% |
| `security.signer_id_policy.full_cert_every_ms` | 500 | security | 55 |  |  |  |  | ↑10% | ↑0% | ↑29% |  | ↑7% |  |  |  |  | metric.full_cert_share ↑99%; metric.cert_bytes_share ↑76%; metric.unverified_ratio ↓58% |
| `security.signer_id_policy.full_cert_every_ms` | 2000 | security | 46 |  |  |  |  |  | ↓0% | ↑2% |  | ↓3% |  |  |  |  | metric.unverified_ratio ↑58%; metric.p2pcd_requests ↑54%; metric.full_cert_share ↓49% |
| `security.signer_id_policy.digest_otherwise` | false | security | 75 | ↓0% | ↓0% |  | ↑300% | ↑61% | ↑1% | ↑46% |  | ↑60% |  |  |  |  | metric.full_cert_share ↑889%; metric.cert_bytes_share ↑357%; loss_hidden-terminal ↑300% |
| `security.signer_id_policy.on_new_neighbour` | true | security | 49 |  |  |  |  | ↑0% | ↓0% | ↓1% |  | ↑1% |  |  |  |  | metric.p2pcd_requests ↓35%; metric.unverified_ratio ↓26%; cbr_max ↑19% |
| `security.signer_id_policy.on_new_neighbour` | false | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.pseudonym_change.strategy` | distance | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.pseudonym_change.strategy` | c2c-cc | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.pseudonym_change.strategy` | mix-zone | security | 7 |  |  |  |  |  |  |  |  |  |  |  |  |  | report.phase2.privacy_anonymity_set_sum ↑1.000; report.phase2.privacy_links_claimed ↑1.000; report.phase2.privacy_frames_read ↓50% |
| `security.pseudonym_change.strategy` | silent | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.pseudonym_change.period_s` | 1.0 | security | 74 |  |  |  |  | ↓1% | ↓0% | ↑48% |  | ↓1% |  |  |  |  | metric.det_accuracy new; metric.det_fpr new; metric.linkability new |
| `security.pseudonym_change.period_s` | 945.0 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.pseudonym_change.distance_m` | 1.0 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.pseudonym_change.distance_m` | 100000.0 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.on_demand_threshold` | 0.05 | security | 21 |  |  |  |  |  | ↑1% | ↑0% |  |  |  |  |  |  | metric.verify_cost new; metric.verify_wait new; metric.verify_rate ↑285% |
| `security.on_demand_threshold` | 1.0 | security | 15 |  |  |  |  |  | ↓0% | ↓0% |  |  |  |  |  |  | metric.verify_queue_depth ↓19%; metric.verify_rate ↓4%; report.phase2.verification_states.Verified ↓2% |
| `security.prioritized_horizon_m` | 100.0 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `security.prioritized_horizon_m` | 1000.0 | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `nodes.default_obu` | obu/cohda-mk6c-qualcomm-9150 | dsrc | 95 | ↓1% | ↓1% |  | ↑24% | ↓1% | ↓3% | ↑1% | ↑0% | ↓0% |  |  |  |  | metric.verify_wait ↑5408%; stage_verify_wait_ms ↑5278%; metric.verify_queue_depth ↑3303% |
| `nodes.default_obu` | obu/generic-automotive-soc-no-hsm | dsrc | 97 | ↓0% | ↓0% | ↑6% | ↑7% | ↑1% | ↓68% | ↓10% | ↑1% | ↓0% |  |  |  |  | stage_queue_ms ↑0.074; metric.verify_wait ↑1321%; stage_verify_wait_ms ↑1294% |
| `nodes.default_obu` | obu/unex-obu-301-craton2-software | dsrc | 99 | ↓1% | ↓1% | ↑6% | ↑21% | ↑0% | ↓56% | ↓9% | ↑1% | ↓0% |  |  |  |  | loss_collision new; report.rx_losses.collision new; stage_queue_ms ↑0.280 |
| `nodes.compute_tier` | abstract | dsrc | 94 | ↓1% | ↓1% | ↑6% | ↑15% | ↑1% | ↓76% | ↓11% | ↑1% | ↓0% |  |  |  |  | metric.verify_wait ↓100%; stage_verify_wait_ms ↓100%; stage_sign_ms ↓100% |
| `nodes.compute_tier` | high | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `nodes.backend_tier` | abstract | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `nodes.backend_tier` | high | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `nodes.verify_queue_depth` | 6 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `nodes.verify_queue_depth` | 256 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `nodes.peer_cert_cache` | 16 | dsrc | 23 |  |  |  |  |  | ↑1% | ↑208% |  |  |  |  |  |  | metric.unverified_ratio ↑670%; metric.p2pcd_requests ↑376%; metric.loss_rate ↑243% |
| `nodes.peer_cert_cache` | 1024 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `threats.compromised_rsu_attack.id` | threat/attacker/compromised-rsu | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `threats.eavesdropper.id` | threat/observer/passive-privacy | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `detection.ma.id` | threat/ma/corroborated | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `detection.ma.id` | threat/ma/legacy-window | security | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `energy.ev.initial_soc` | 0.0 | dsrc | 22 |  |  |  |  |  |  |  |  |  |  |  |  |  | metric.co2_mass new; metric.co2_per_km new; metric.co_mass new |
| `energy.ev.initial_soc` | 1.0 | dsrc | 22 |  |  |  |  |  |  |  |  |  |  |  |  |  | metric.co2_mass new; metric.co2_per_km new; metric.co_mass new |
| `energy.stop_speed_mps` | 5.0 | dsrc | 22 |  |  |  |  |  |  |  |  |  |  |  |  |  | metric.co2_mass new; metric.co2_per_km new; metric.co_mass new |
| `scale.traffic.meso.segment_length_m` | 1.0 | dsrc | 58 | ↑0% | ↑0% | ↑3% | ↓4% | ↑0% | ↓0% | ↓0% |  |  |  | ↑2% | ↓100% |  | metric.drac ↓92%; metric.ttc_min ↑14%; metric.per ↓4% |
| `scale.traffic.meso.segment_length_m` | 10000.0 | dsrc | 74 | ↑0% | ↑0% | ↑3% | ↓4% | ↓0% | ↓0% | ↓0% |  | ↓0% |  | ↑0% | ↓17% |  | metric.drac ↓39%; metric.per ↓4%; metric.collision_rate ↓4% |
| `scale.traffic.meso.tau_ff_s` | 60.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `scale.traffic.meso.tau_fj_s` | 60.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `scale.traffic.meso.tau_jf_s` | 60.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `scale.traffic.meso.tau_jj_s` | 60.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `scale.traffic.meso.jam_threshold` | 0.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `scale.traffic.meso.jam_threshold` | 1.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `scale.traffic.meso.junction_control` | true | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `scale.traffic.meso.junction_control` | false | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `scale.radio.follows` | true | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `scale.radio.follows` | false | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `scale.radio.margin_m` | 0.0 | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `scale.radio.receive_in_margin` | true | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `scale.radio.receive_in_margin` | false | dsrc | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.right_turn_on_red` | true | osm | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.right_turn_on_red` | false | osm | 86 | ↓6% | ↓5% |  | ↑3% | ↓9% | ↑0% | ↑17% |  | ↓0% |  | ↓4% | ↑7% |  | ipg_ms_p95 ↑46%; report.apps.lta.lead_ms_sum ↓43%; stage_access_ms ↓28% |
| `traffic.dynamic_rerouting` | true | saturated | 5 |  |  |  |  |  |  |  |  |  |  |  |  |  | metric.signal_gap_outs new; metric.signal_max_outs new; metric.traffic_jammed new |
| `traffic.dynamic_rerouting` | false | saturated | 5 |  |  |  |  |  |  |  |  |  |  |  |  |  | metric.signal_gap_outs new; metric.signal_max_outs new; metric.traffic_jammed new |
| `traffic.reroute_period_s` | 0.0 | saturated | 5 |  |  |  |  |  |  |  |  |  |  |  |  |  | metric.signal_gap_outs new; metric.signal_max_outs new; metric.traffic_jammed new |
| `traffic.reroute_period_s` | 300.0 | saturated | 147 | ↑2% | ↑2% |  | ↓2% | ↑0% | ↓0% | ↓3% | ↓1% | ↑0% |  | ↓0% | ↓2% |  | metric.signal_gap_outs new; metric.signal_max_outs new; metric.traffic_jammed new |
| `traffic.lane_changes` | true | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.lane_changes` | false | traffic | 140 | ↓2% | ↓2% |  | ↓9% | ↑3% | ↓0% | ↓13% | ↓4% | ↑1% |  | ↓6% | ↑17% |  | loss_collision gone; metric.pet new; report.apps.lta.false_warnings gone |
| `traffic.strategic_lane_choice` | true | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.strategic_lane_choice` | false | traffic | 136 | ↓2% | ↓2% |  | ↓9% | ↑3% | ↓0% | ↓13% | ↓4% | ↑1% |  | ↓5% | ↑16% |  | loss_collision gone; metric.pet new; report.despawn_causes.TripComplete new |
| `traffic.keep_junctions_clear` | true | saturated | 5 |  |  |  |  |  |  |  |  |  |  |  |  |  | metric.signal_gap_outs new; metric.signal_max_outs new; metric.traffic_jammed new |
| `traffic.keep_junctions_clear` | false | saturated | 148 | ↑0% | ↑0% |  | ↑2% | ↑2% | ↓0% | ↑1% | ↑1% | ↑0% |  | ↑5% | ↓10% |  | metric.signal_gap_outs new; metric.signal_max_outs new; metric.traffic_jammed new |
| `traffic.gridlock_timeout_s` | 0.0 | saturated | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.gridlock_timeout_s` | 300.0 | saturated | 5 |  |  |  |  |  |  |  |  |  |  |  |  |  | metric.signal_gap_outs new; metric.signal_max_outs new; metric.traffic_jammed new |
| `traffic.signals.control` | fixed-time | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.signals.control` | actuated | traffic | 139 | ↑11% | ↑12% |  | ↑15% | ↑16% | ↓0% | ↑41% | ↓0% | ↑2% |  | ↑27% | ↓74% |  | metric.pet new; metric.signal_gap_outs new; metric.signal_max_outs new |
| `traffic.signals.min_green_s` | 2.0 | traffic | 111 | ↓4% | ↓4% |  | ↑9% | ↓6% | ↑0% | ↓4% |  | ↑0% |  | ↑0% | ↓13% |  | report.rx_losses.collision ↑600%; loss_collision ↑600%; report.apps.fcw.lead_ms_sum ↓100% |
| `traffic.signals.min_green_s` | 60.0 | refused | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.signals.max_green_s` | 5.0 | refused | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.signals.max_green_s` | 180.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.signals.passage_s` | 0.2 | traffic | 95 | ↓1% | ↓1% |  | ↑1% | ↓1% | ↑0% | ↑0% |  | ↑0% |  | ↑0% | ↓7% |  | loss_collision ↑700%; report.rx_losses.collision ↑700%; report.apps.lcw.issued ↓6% |
| `traffic.signals.passage_s` | 10.0 | traffic | 116 | ↓2% | ↓2% |  | ↓1% | ↓4% | ↑0% | ↑2% |  | ↓0% |  | ↓3% | ↑52% |  | metric.signal_max_outs ↑5.000; report.apps.fcw.true_warnings ↑300%; loss_collision ↑100% |
| `traffic.signals.detector_m` | 1.0 | traffic | 143 | ↓14% | ↓15% |  | ↓61% | ↓11% | ↑0% | ↓70% | ↓2% | ↓2% |  | ↓57% | ↑1039% |  | loss_collision gone; metric.pet gone; report.apps.ima.false_warnings gone |
| `traffic.signals.detector_m` | 150.0 | traffic | 138 | ↓8% | ↓8% |  | ↑8% | ↓14% | ↑0% | ↑3% | ↓1% | ↑1% |  | ↑11% | ↑19% |  | metric.signal_max_outs ↑105.000; report.apps.fcw.missed ↑2.000; report.apps.ima.missed ↑2.000 |
| `traffic.signals.green_wave_speed_mps` | 1.0 | traffic | 138 | ↓1% | ↓2% |  | ↓18% | ↑2% | ↓0% | ↑12% | ↓0% | ↑0% |  | ↓10% | ↑30% |  | loss_collision gone; metric.pet new; metric.signal_gap_outs new |
| `traffic.signals.green_wave_speed_mps` | 40.0 | traffic | 129 | ↑2% | ↑3% |  | ↓7% | ↑4% | ↓0% | ↑37% | ↓2% | ↑1% |  | ↑5% | ↓17% |  | loss_collision gone; metric.pet new; report.despawn_causes.TripComplete new |
| `traffic.signals.green_wave_heading_deg` | 0.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.signals.green_wave_heading_deg` | 360.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.critical_gap_factor` | 0.8 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.critical_gap_factor` | 1.2 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.permitted_left_gap_s` | 4.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.permitted_left_gap_s` | 5.5 | traffic | 49 | ↓0% |  |  | ↑0% |  | ↓0% | ↑0% |  |  |  | ↓0% |  |  | metric.acceleration ↓4%; loss_fading ↓0%; report.rx_losses.fading ↓0% |
| `traffic.turn_lateral_accel_mps2` | 3.0 | traffic | 122 | ↑0% | ↑0% |  | ↑2% | ↑1% | ↓0% | ↑2% | ↓0% | ↑0% |  | ↓1% | ↓7% |  | loss_collision gone; report.rx_losses.collision gone; report.apps.ima.false_warnings ↑100% |
| `traffic.turn_lateral_accel_mps2` | 6.0 | traffic | 121 | ↓0% | ↓0% |  | ↓0% | ↑1% | ↓0% | ↑1% | ↓0% | ↓0% |  | ↑0% | ↑0% |  | report.apps.ima.false_warnings ↑100%; report.apps.ima.true_warnings ↓100%; stage_access_ms ↓34% |
| `traffic.kerbside.enabled` | true | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.kerbside.enabled` | false | traffic | 100 | ↓0% | ↓0% |  | ↑7% | ↓2% | ↑0% | ↓4% |  | ↓0% |  | ↑7% | ↓15% |  | stage_access_ms ↑184%; report.mac_access_delay_ns ↑181%; access_ms ↑181% |
| `traffic.kerbside.bus_stop_spacing_m` | 200.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.kerbside.bus_stop_spacing_m` | 400.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.kerbside.bus_dwell_mean_s` | 15.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.kerbside.bus_dwell_mean_s` | 60.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.kerbside.taxi_share` | 0.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.kerbside.taxi_share` | 0.3 | traffic | 100 | ↑0% | ↑0% |  | ↓7% | ↑2% | ↓0% | ↑4% |  | ↑0% |  | ↓7% | ↑18% |  | stage_access_ms ↓65%; access_ms ↓64%; report.mac_access_delay_ns ↓64% |
| `traffic.kerbside.taxi_dwell_median_s` | 20.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.kerbside.taxi_dwell_median_s` | 60.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.kerbside.delivery_stop_share` | 0.3 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.kerbside.delivery_stop_share` | 0.6 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.kerbside.delivery_dwell_median_s` | 30.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `traffic.kerbside.delivery_dwell_median_s` | 3600.0 | traffic | 0 |  |  |  |  |  |  |  |  |  |  |  |  |  |  |
| `world.source.params.ew_green_share` | 0.3 | traffic | 111 | ↑1% | ↑1% |  | ↓4% | ↑1% | ↓0% | ↑26% |  | ↓1% |  | ↓5% | ↑16% |  | loss_collision gone; metric.pet new; report.rx_losses.collision gone |
| `world.source.params.ew_green_share` | 0.7 | traffic | 134 | ↓8% | ↓8% |  | ↓19% | ↓10% | ↓0% | ↑6% | ↓1% | ↑1% |  | ↑1% | ↑12% |  | loss_collision gone; report.apps.ima.false_warnings gone; report.apps.ima.issued gone |

## Settings not swept (144)

- **an element of a list or map: no single field to move** (63): `meta.authors[]`, `meta.tags[]`, `time.time_dilation[].from_s`, `time.time_dilation[].to_s`, `actors.vehicles.classes.*.fraction`, `actors.vehicles.classes.*.obu`, `actors.rsus[].site`, `actors.rsus[].position_m[]`, `actors.rsus[].roles[]`, `actors.rsus[].profile`, `actors.rsus[].backhaul`, `actors.rsus[].backhaul_params`, `actors.rsu_placement.roles[]`, `actors.backend.entities.*.profile`, `actors.backend.entities.*.service_model`, `actors.backend.entities.*.net`, `actors.backend.links[].from`, `actors.backend.links[].to`, `actors.backend.links[].latency_ms`, `actors.backend.links[].capacity_mbps`, `actors.backend.failures[].node`, `actors.backend.failures[].mode`, `actors.backend.failures[].from_s`, `actors.backend.failures[].to_s`, `actors.backend.failures[].extra_ms`, `radio.models.*.id`, `radio.models.*.params`, `radio.adjacent_channel[].rat`, `radio.adjacent_channel[].channel`, `radio.adjacent_channel[].acir_db`, `radio.adjacent_channel[].params`, `messages.sets[]`, `security.threshold.roles[]`, `security.pseudonym_change.silent_period_s[]`, `nodes.per_class.*`, `threats.attackers[].id`, `threats.attackers[].fraction`, `threats.attackers[].count`, `threats.attackers[].actor_ids[]`, `threats.attackers[].params`, and 23 more
- **descriptive** (4): `meta.name`, `meta.description`, `meta.created`, `meta.base`
- **free text** (12): `time.t0`, `city`, `world.source.path`, `world.imported_at`, `world.terrain.dem`, `world.cache`, `actors.rsu_placement.backhaul`, `security.threshold.protocol`, `security.threshold.umbrella_inner`, `detection.responder.id`, `mobility.provider`, `mobility.sumo_home`
- **needs a focus region (a polygon), which a scalar sweep cannot supply** (7): `radio.tiers.focus.region.node`, `radio.tiers.focus.region.radius_m`, `radio.tiers.focus.region.bbox.min_lat_deg`, `radio.tiers.focus.region.bbox.min_lon_deg`, `radio.tiers.focus.region.bbox.max_lat_deg`, `radio.tiers.focus.region.bbox.max_lon_deg`, `radio.tiers.focus.tier`
- **needs a roadside unit with the map role at a signalised junction (tests/network_audit_config.rs covers it)** (1): `messages.map_interval_ms`
- **needs a roadside unit with the spat role at a signalised junction (tests/network_audit_config.rs covers it)** (1): `messages.spat_interval_ms`
- **no perception model in this build: the loader refuses every value** (1): `detection.perception_tier`
- **no value other than the default inside the bound** (28): `world.source.bbox.min_lat_deg`, `world.source.bbox.min_lon_deg`, `world.source.bbox.max_lat_deg`, `world.source.bbox.max_lon_deg`, `weather.visibility_m`, `radio.channel`, `security.threshold.members`, `security.threshold.quorum`, `security.threshold.linkage_members`, `security.threshold.linkage_quorum`, `security.threshold.umbrella_leaves`, `security.threshold.round_timeout_ms`, `security.threshold.retry_after_s`, `security.threshold.member_latency_ms`, `security.pseudonym_change.mix_zone_radius_m`, `security.pseudonym_change.mix_zone_min_age_s`, `apps.glosa_compliance`, `apps.fcw_ttc_s`, `apps.ima_tti_s`, `apps.lta_gap_s`, `apps.pcw_ttc_s`, `apps.rlvw_decel_mps2`, `experiment.replications`, `scale.traffic.region.min_lat_deg`, `scale.traffic.region.min_lon_deg`, `scale.traffic.region.max_lat_deg`, `scale.traffic.region.max_lon_deg`, `traffic.turn_curve_window_m`
- **open choice (free text): no closed set to sweep** (8): `world.source.generator`, `actors.backend.protocol`, `net.fragmenter.id`, `net.uu.id`, `messages.generator.id`, `security.protocol.id`, `energy.emission_model`, `energy.ev_model`
- **structured value (JSON): no scalar to move** (14): `world.source.params`, `actors.vehicles.demand.params`, `actors.vehicles.driving`, `actors.rsu_placement.backhaul_params`, `net.fragmenter.params`, `net.backhaul.params`, `net.uu.params`, `net.backend_net.params`, `messages.generator.params`, `security.protocol.params`, `threats.compromised_rsu_attack.params`, `threats.eavesdropper.params`, `detection.ma.params`, `detection.responder.params`
- **the clock grain: 1 us is the only value the radio tiers accept** (1): `time.des_resolution`
- **the document version** (1): `schema`
- **the master seed: moves everything by construction** (1): `seed`
- **the run length: moves every count by construction** (1): `time.duration_s`
- **the tiered model needs a microscopic region box; tests/scale.rs covers it** (1): `scale.traffic.model`
