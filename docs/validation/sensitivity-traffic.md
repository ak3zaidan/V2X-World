# Traffic sensitivity: the parameters move the metrics the way traffic theory says

Each row is one fast same-seed test in `crates/v2xw-engine/tests/sensitivity_traffic.rs`.
A test changes one parameter from `docs/guides/config-inventory-traffic.md`, keeps
everything else and the seed fixed, and asserts the direction traffic theory (or radio
theory, for the network rows) predicts. A parameter that never reached the model, or a
metric that does not respond to it, fails its row. Run them with:

```
cargo test -p v2xw-engine --test sensitivity_traffic -- --nocapture --test-threads=1
```

The whole file takes about 2.5 minutes (opt-level 2, one thread). The demand-into-network
row takes most of that time, because it puts 150 radios on the channel.

## Set-up

- **World:** `scenarios/phase1-grid.yaml` (seed `0xC0FFEE_5EED`) cut to 4 x 6
  Midtown-shaped blocks (274 x 80 m), two lanes each way, 11.2 m/s limit, a 60 s fixed
  cycle, buildings off. Demand is a Poisson arrival rate, unless a row says "fleet": then
  `demand.fleet_size` holds N vehicles on the road, which keeps the grid on the congested
  branch of the fundamental diagram.
- **Traffic quantities** come from `gt.kinematics`, motor vehicles only:
  - density: mean vehicles in the network per instant;
  - speed: time-mean speed;
  - vehicle-km: the trapezoid-rule integral of speed;
  - travel-time rate: vehicle-seconds per vehicle-km (HCM 6th ed. ch. 16);
  - stops per vehicle-km: a fall from at least 2 m/s to at most 0.1 m/s;
  - stopped delay: time at or below 0.1 m/s, per vehicle;
  - queue: vehicles standing, per instant;
  - start-up: time from the last standing sample to 6 m/s;
  - lane changes: a lane switch that keeps the longitudinal position.
- **Network quantities:**
  - PDR: run report `receptions_ok / reception_attempts`;
  - CBR: the `channel_load` metric (J2945/1 CBR);
  - latency: the `e2e_latency` metric and the run report's mean MAC access delay;
  - neighbours: the node telemetry's `nbr_total`;
  - VRU messages: PSM/VAM `node.tx` records;
  - false verdicts: `phase2.verdicts_fired`. No attacker is present, so every verdict
    is a false positive;
  - position error: each BSM's decoded latitude and longitude, projected back to world
    metres, against the sender's latest true position.

## Results

| Parameter (scenario path) | Change | Prediction (source) | Measured | Verdict |
|---|---|---|---|---|
| Demand `actors.vehicles.demand.rate_veh_per_h` | 400 -> 2400 veh/h, 90 s | density up, speed down, travel time and stops up (Greenshields 1935; HCM 6th ed. ch. 16/19) | density 8.9 -> 33.8; speed 6.85 -> 5.91 m/s; travel-time rate 146 -> 169 s/km; stops 1.28 -> 2.56 /veh-km | holds |
| Demand into the network `demand.fleet_size`, all equipped | 30 -> 150 vehicles, 12 s | CBR up, PDR down, latency up (Bilstrup et al. 2008; SAE J2945/1 §6.3) | CBR 0.088 -> 0.410; PDR 0.983 -> 0.923; MAC access 0.022 -> 0.165 ms; e2e latency 12.249 -> 12.413 ms | holds |
| IDM time headway `actors.vehicles.drivers.time_headway_s` | 1.0 -> 2.5 s, fleet 250, 90 s | flow down: saturation flow ~ 1/(T + (s0 + l)/v) (Treiber, Hennecke & Helbing 2000) | vehicle-km 106.2 -> 92.4 (-13 %); speed 4.82 -> 4.22 m/s | holds |
| Max acceleration `actors.vehicles.drivers.max_accel_mps2` | 2.5 -> 0.8 m/s², 1500 veh/h | start-up lost time up (HCM 6th ed. ch. 19) | 0-6 m/s 2.74 -> 8.16 s | holds |
| MOBIL politeness `actors.vehicles.drivers.politeness` | 0 -> 1, 3-lane 1.5 km unsignalised links (highway-congestion geometry), speed spread 3 | "lane changes down" | lane changes 49 -> **59**; speed 15.2 -> 15.5 m/s | parameter reaches the model; direction **not** as stated (see below) |
| Vehicle mix `actors.vehicles.classes` | 0 -> 50 % trucks, fleet 250 | speed and capacity down (HCM 6th ed. ch. 19 f_HV) | speed 4.82 -> 3.72 m/s; vehicle-km 106.2 -> 80.5 (-24 %) | holds |
| Signal cycle (procedural `cycle_s`; `world.signals.cycle_s` on OSM) | 40 -> 120 s, 1200 veh/h, 120 s | delay and queue up: d1 = 0.5 C (1 - g/C)² / (1 - min(1, X) g/C) (HCM 6th ed. eq. 19-19; Webster 1958) | stopped delay 6.6 -> 22.3 s/veh; queue 2.6 -> 8.6 veh | holds; the ratio 3.4 is close to the cycle ratio 3, as d1 ∝ C says |
| Actuated control `traffic.signals.control` | fixed -> actuated, 300 veh/h | delay down at low demand (FHWA STM 2008 ch. 5) | stopped delay 11.1 -> 2.1 s/veh; stops 1.34 -> 0.12 /veh-km | holds |
| Speed limit (procedural `speed_limit_mps`) | 13.4 -> 8.9 m/s, 600 veh/h | speed down, travel time up, flow down at equal density (Greenshields 1935) | speed 7.58 -> 5.94 m/s; travel-time rate 132 -> 168 s/km; vehicle-km 7.90 -> 6.21 | holds. The steady-state density rise is not reached in 90 s, which is shorter than one trip, so it is not asserted |
| Equipped fraction `actors.vehicles.equipped_fraction` | 1.0 -> 0.3, 1500 veh/h | awareness and neighbours down (ETSI TR 103 562) | neighbours 15.6 -> 4.4; receptions per node 4163 -> 1224 | holds |
| Pedestrians `actors.vru.pedestrians`, device fraction 0.5 | 40 -> 300 | VRU messages up, crossing delay up (HCM 6th ed. ch. 19 f_Lpb/f_Rpb) | PSM/VAM 1380 -> 9659; vehicle stopped delay 3.75 -> 7.06 s | holds |
| Weather `weather.initial` / `intensity` | clear -> rain 1.0 | speed down (FHWA-HOP-12-004); radio loss up only if modelled | speed 7.07 -> 6.06 m/s; PDR 0.980 -> 0.979 | holds. The radio change is recorded, not asserted: ITU-R P.838 rain attenuation at 5.9 GHz is under 1 dB/km |
| GNSS `actors.vehicles.gnss` | RTK -> standard -> standard + outliers (0.3 /s, 60 m) | position error up, misbehaviour false positives up (van der Heijden et al. 2018, VeReMi) | position error 2.61 -> 3.78 -> 5.56 m; false verdicts **14** -> 4 -> 63 | position error holds; false positives hold for unstated error (standard -> outliers). RTK -> standard is lower by design: tolerances scale with the stated accuracy (see below) |

The position error includes a constant offset that is common to every row: the
`gt.kinematics` reference point and the BSM's position are not the same point on the
car, and up to one 100 ms step of motion separates the BSM from its truth sample. The
differences between rows are the GNSS model's.

## Findings

1. **Politeness does not lower the lane-change count, and the test does not assert that
   it does.** The model's MOBIL incentive is the published one (`lanechange/mobil.rs`):
   a_c' - a_c + p (Δa_n + Δa_o) (Kesting, Treiber & Helbing 2007, eq. 3). Raising p
   charges a change with the new follower's loss Δa_n. It also credits the change with the
   old follower's gain Δa_o, which comes from a slow driver moving out of the way. With
   spread desired speeds on long links the credit wins, so the count rises.
   - On the urban grid with strategic lane choice the effect is within noise: 18 -> 19
     with the default fleet, 38 -> 36 with 30 % trucks and speed spread 3. Most changes
     there are owed to the route.
   - The test asserts that p changes the decisions (the wiring) and records the
     direction.
   - Unresolved: whether the owner wants a test of the case the prediction describes
     (cut-ins at a merge).
2. **RTK fixes give more false verdicts than the standard receiver: 14 against 4** (9
   position-speed, 6 heading, 2 position-jump). That is 0.2 % against 0.06 % of the
   about 7,000 messages checked. This is the detector's design, not a wiring fault:
   - The suite scales its tolerances with each message's *stated* accuracy (`phase2.rs`
     `use_stated_accuracy`; `v2xw-threat` `detect.rs`). Position radii are floored at
     `tolerance_floor_factor · consistency_threshold_m`. The heading check runs only when
     the displacement exceeds `heading_conf_factor ×` the stated accuracy.
   - A standard receiver states metres, so its messages are checked less often and with
     wider gates. An RTK receiver states centimetres, so the heading and position-speed
     checks run on almost every step, and turns and 100 ms sampling produce the few
     residual false verdicts.
   - The prediction "error up, false positives up" holds for error the sender does **not**
     state: outliers take the count from 4 to 63, and the test asserts that step.
   - Unresolved: whether the heading check's displacement floor (`heading_min_disp_m`)
     should be raised for high-accuracy senders. That is a detector-suite decision, and
     changing it changes existing digests, so it is not made here.
3. **More pedestrians lower the network PDR** from 0.875 to 0.524, at a CBR of only 0.04
   to 0.05. The MAC access delay rises from 0.02 to 1.99 ms.
   - Recorded, not asserted.
   - The likely cause is many low-power, body-shadowed handset links in the attempt
     count, not channel congestion.
   - Worth a network-track check.
4. **All other rows hold** with the parameter wired through to the model at run time. No
   wiring fix was needed.

## Not covered

- **Green split:** not exposed for procedural grids; the OSM split follows lane counts,
  `SignalDefaults`. Only the cycle row is here.
- **Snow, sleet and visibility:** follow the same FHWA path as rain.
- **The shared sensitivity harness:** was not on this base. The test file carries a
  minimal same-seed helper and a filtering recorder, `Probe`, instead.
