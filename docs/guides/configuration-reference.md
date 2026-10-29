# Configuration reference

Every setting a researcher or engineer varies to compare V2X protocols, with its scenario
path, default, accepted range and source. Every row is a scenario field: the loader
validates it, and the engine publishes it with a unit, label, explanation and status
(`settings.describe` / `crate::scenario::publish`). The Studio settings window and the
agent read it from there. A value outside its range, or a combination where the setting
would have no effect, is refused when the scenario loads, with the field named in the
error.

Rows marked **new** were added by the researcher-configuration track (2026-10-07). The
others already existed and are listed so the page is complete. The ranges are the
`validate::BOUNDS` rows, and the closed value sets are the `validate::CHOICES` rows.

## How this compares with the reference simulators

The knobs below are the ones Veins (`omnetpp.ini` for `Mac1609_4`, `PhyLayer80211p`,
`Decider80211p`, `SimpleObstacleShadowing`), Artery (`Middleware`, `DccEntity`,
`GeoNetworking`), OpenCV2X (`LteMacVUeMode4`, `LtePhyVUeMode4`), ns-3 WAVE and
NR-V2X (`WaveHelper`, `YansWifiPhy`, `LogDistancePropagationLossModel`,
`NakagamiPropagationLossModel`, `BsmApplication`, `nr-v2x` SPS attributes) and SUMO
(`vType` `accel`, `decel`, `tau`, `minGap`, `lcCooperative`) expose. A knob those tools
expose that this engine does not is listed under [Not configurable yet](#not-configurable-yet)
with its reason.

| What researchers vary | Veins / Artery / OpenCV2X / ns-3 / SUMO | This engine |
|---|---|---|
| Transmit power, antenna gain, cable loss, antenna height | `txPower`, `antennaGain`, `TxGain`/`RxGain`, `antennaOffsetZ` | `radio.devices.{obu,rsu,vru}.*` |
| Data rate / MCS | `bitrate`, `DataMode`, `mcs` | `radio.phy.data_rate_mbps` (802.11p, **new**), `radio.models.sidelink.params.mcs` (C-V2X) |
| Noise figure | `noiseFloor`, `RxNoiseFigure` | `radio.phy.noise_figure_db` (**new**) |
| Receiver sensitivity | `minPowerLevel`, `RxSensitivity` | `radio.phy.sensitivity_dbm` (**new**) or `radio.models.phy.params.sensitivity` |
| CCA threshold | `ccaThreshold`, `CcaEdThreshold` | `radio.phy.cca_threshold_dbm` (**new**) |
| Capture rule | ns-3 `FrameCaptureModel`, Veins SINR decider | `radio.phy.capture_threshold_db` (**new**; unset = SINR trace) |
| Path-loss law and exponent | `LogDistancePropagationLossModel::Exponent`, Veins `alpha` | `radio.models.propagation`, `radio.channel_model.path_loss_exponent` (**new**) |
| Shadowing sigma, decorrelation | Veins `LogNormalShadowing`, Artery `VehicleObstacleShadowing` | `radio.channel_model.shadowing_sigma_db`, `.shadowing_decorrelation_m` (**new**) |
| Fading | `NakagamiFading`, ns-3 `Nakagami m0..m2`, Rayleigh/Rice | `radio.channel_model.fading` + `.nakagami_shape` / `.rice_k_db` (**new**) |
| Building obstruction | `SimpleObstacleShadowing` on/off | `world.buildings.enabled`, `radio.models.obstacle` |
| Access category per message | `Mac1609_4` `userPriority`, Artery `DccProfile` | `radio.mac.periodic_access_category`, `.event_access_category` (**new**) |
| CWmin, AIFSN, queue length | `Mac1609_4` EDCA tables, `queueSize` | `radio.mac.cw_min_*`, `.aifsn_*`, `.queue_length` (**new**) |
| Channel switching | `useServiceChannel`, 1609.4 | `net.channel_access` |
| `net.rsu_ip.air` **new** | `constant` | `constant`, `sch` | WP-19. `sch`: each relaying RSU signs and sends an IEEE 1609.3 WSA (SCMS PSID 0x23, an ASSUMPTION; IPv6 Routing; WAVE Routing Advertisement), and a vehicle binds to a unit only while it holds that unit's verified WSA, at most 1 s old. Relayed misbehaviour reports go on the `radio.channels` service channel when there is one (M11). Downloads ride TCP/TLS on a service-channel session (`transport/80211p-sch`: contact window, EDCA on the unit's CBR, 1609.4 intervals, link-budget PER); a transfer that fails cuts its flow off, and the vehicle starts again at its next contact (`relayed_flows_cut_off`). |
| `net.rsu_ip.wsa_interval_ms` **new** | 100 | 20 to 5000 ms | USDOT RSU Spec v4.1 App. D: `RepeatRate` 50 per 5 s. Read only under `sch`. |
| DCC on/off and algorithm | Artery `DccEntity` (`Reactive`, `Limeric`), J2945/1 | `radio.models.dcc`, `radio.dcc.enabled` (**new**) |
| DCC parameters | LIMERIC `alpha`/`beta`/target, J2945/1 tables | `radio.dcc.*` (**new**) |
| SPS RRI, keep probability, sensing threshold | OpenCV2X `rri`, `probResourceKeep`, `rsrpThreshold`; ns-3 NR `RRI` | `radio.sidelink.*` (**new**) |
| Sidelink MCS, retransmissions, CR limits | OpenCV2X `mcs`, `numRetx`, `crLimit` | `radio.models.sidelink.params.{mcs,max_transmissions,congestion_control}` |
| Beacon rate | ns-3 `BsmApplication` interval, Veins `beaconInterval` | `messages.rate_hz` (**new**), or `messages.generator.params` |
| CAM triggers | Artery `CaService` | `messages.generator` `generator/cam-en302637-2` params |
| Message sets (CAM, DENM, CPM, VAM, SPaT, MAP, PSM) | Artery services | `messages.sets` |
| GeoNetworking, hop limit, CBF | Artery/Vanetza MIB | `net.layer`, `net.geonet.*` |
| Signature scheme, envelope, certificate attach | Artery `SecurityEntity` | `security.signature`, `security.envelope`, `security.signer_id_policy` |
| Verification policy | n/a in Veins | `security.verification_policy` |
| Pseudonym change | n/a in Veins (PREXT plug-in) | `security.pseudonym_change.*` |
| Crypto hardware | n/a | `nodes.default_obu`, `nodes.per_class`, `nodes.compute_tier` |
| Demand, vehicle mix | SUMO routes, `vTypeDistribution` | `actors.vehicles.demand.*`, `actors.vehicles.classes` |
| Car-following model and IDM parameters | SUMO `carFollowModel`, `accel`, `decel`, `tau`, `minGap` | `actors.vehicles.driving.car_following`, `actors.vehicles.drivers.*` (**new**) |
| Lane-change politeness | SUMO `lcCooperative`, MOBIL `p` | `actors.vehicles.drivers.politeness` (**new**) |
| Duration, seed, replications | `sim-time-limit`, `seed-set`, `repeat` | `time.duration_s`, `seed`, `experiment.seeds`, `experiment.replications` |
| Warm-up | OMNeT++ `warmup-period` | `time.warmup_s` (**new**) |
| Metric sampling interval | OMNeT++ statistic recording interval | `time.metric_period_s` |

## Physical layer: 802.11p (`radio.phy`, `radio.devices`)

`radio.phy` is refused on a C-V2X run (`radio.rat: lte-v2x-pc5` or `nr-v2x-pc5`): the
sidelink's MCS and sensitivity are `radio.models.sidelink`'s.

| Path | Default | Range | Source and effect |
|---|---|---|---|
| `radio.rat` | `dsrc-80211p` | `dsrc-80211p`, `lte-v2x-pc5`, `nr-v2x-pc5` | Which access layer runs. |
| `radio.region` | `us-2016` (802.11p), `us` (C-V2X) | `us`, `us-2016`, `eu` | Band plan, deployment channel, EIRP limits (47 CFR 90.377, FCC 24-123, EN 302 571). Sets the carrier frequency. |
| `radio.channel` | region's channel | 170-184 | Carrier frequency through the band plan: `f = 5000 + 5n` MHz. 802.11p channels are 10 MHz. |
| `radio.devices.obu.tx_power_dbm` | 20 | -20 to 33 dBm | Conducted power. DCC may lower it, and the region's EIRP cap applies. |
| `radio.devices.{obu,rsu,vru}.antenna_gain_dbi` | profile | -10 to 20 dBi | Link budget at both ends. |
| `radio.devices.{obu,rsu,vru}.cable_loss_db` | profile | 0 to 20 dB | Link budget at both ends. |
| `radio.devices.{obu,rsu,vru}.antenna_height_m` | body class / mast | 0.1 to 15 m | Two-ray and geometric laws, LOS tracing. |
| `radio.phy.data_rate_mbps` **new** | 6 | 3, 4.5, 6, 9, 12, 18, 24, 27 | Every safety frame's MCS: its air time and its sensitivity row (IEEE 802.11-2020 Table 17-4; SAE J2945/1 `vDataRate` 6 Mbit/s). |
| `radio.phy.noise_figure_db` **new** | 6 (PHY card) | 0 to 15 dB | Noise floor = -104 dBm (10 MHz thermal) + NF. Also sets the candidate range. |
| `radio.phy.sensitivity_dbm` **new** | per-rate table (`measured-obu`, -92 dBm at 6 Mbit/s) | -105 to -60 dBm | One flat floor at every rate, as Veins' `minPowerLevel` (-89 dBm in Veins). |
| `radio.phy.cca_threshold_dbm` **new** | -85 | -100 to -50 dBm | Carrier sense and the CBR busy threshold (EN 302 571 §4.2.10.1; ns-3 -82, Veins -65). |
| `radio.phy.capture_threshold_db` **new** | unset (SINR trace) | 0 to 30 dB | A hard capture rule (Torrent-Moreno 2009 Eq. 1) in place of the error model. |
| `radio.models.per` | `per/nist-ofdm` ideal | presets `ideal`, `sjoberg-atheros` | Implementation loss of the NIST error model. |
| `radio.models.phy` | `measured-obu` | `etsi-static`, `etsi-dynamic`, `cohda-mk5`, `measured-obu` | Per-rate sensitivity table. |
| `radio.tiers.phy` | `medium` | `abstract`, `medium`, `high` | Fidelity. |
| `radio.tiers.propagation`, `radio.tiers.mac` | `medium` | `abstract`, `medium`, `high` | Fidelity. `abstract` is not a cheap large-fleet mode: free space with no buildings lets more links reach, and receptions dominate run cost (all three abstract measured 11 % cheaper than medium, `docs/validation/test-run-cost.md`). For large fleets cap `radio.range.max_m` or use city-scale traffic tiers. |

## Channel model (`radio.channel_model`, `radio.models`)

| Path | Default | Range | Source and effect |
|---|---|---|---|
| `radio.models.propagation` | tier: free space (abstract), dual-slope log-distance (medium), geometric urban V2V (high) | `free-space`, `two-ray-ground`, `log-distance-shadowing`, `tr37885`, `v2v-urban-geometric` | The path-loss law (Abbas 2015, 3GPP TR 37.885, Mangel 2011). |
| `radio.channel_model.path_loss_exponent` **new** | unset (measured dual-slope fits) | 1.5 to 6 | Single-slope `PL = PL_FS(10 m) + 10·n·log10(d/10 m)` at the run's carrier (Rappaport Eq. 4.68). Setting it runs the log-distance law. It is refused beside another explicitly named law. |
| `radio.channel_model.shadowing_sigma_db` **new** | the law's own sigma (4.15 dB LOS, 6.67 dB OLOS urban, Abbas 2015) | 0 to 15 dB | Log-normal shadowing spread, applied to every law with shadowing (log-distance, TR 37.885, geometric, WINNER+ B1). |
| `radio.channel_model.shadowing_decorrelation_m` **new** | environment's (about 10 m city) | 0 to 500 m (exclusive of 0) | Gudmundson 1991 correlation distance. |
| `radio.channel_model.fading` **new** | tier's (Nakagami m = 3 for 802.11p, none for C-V2X curves) | `none`, `rayleigh`, `rice`, `nakagami` | Small-scale fading. Refused beside `radio.models.fading`. |
| `radio.channel_model.fading_coherence` **new** | `independent` | `independent`, `doppler` | `doppler` draws one fade per link per coherence block, `T_c = 0.423/f_D` (Rappaport 2002 Eq. 5.40c) with `f_D = (|v_tx|+|v_rx|)·f_c/c` (Akki & Haber 1986), floored at 0.05 m/s: queued vehicles fade in bursts, fast ones decorrelate per frame. Refused when the run draws no fading. |
| `radio.channel_model.nakagami_shape` **new** | 3 | 0.5 to 20 | Nakagami m (needs `fading: nakagami`). |
| `radio.channel_model.rice_k_db` **new** | 6 dB (Cheng 2007 LOS 3-10 dB) | -10 to 30 dB | Rice K, simulated as Nakagami `m = (K+1)²/(2K+1)` (Stüber Eq. 2.57). Needs `fading: rice`. |
| `radio.channel_model.nakagami_shape_far`, `.nakagami_breakpoint_m` **new** | unset (one m everywhere) | 0.5 to 20; (0, 5000] m | Distance-dependent Nakagami m (Veins-style 1.5 near, 0.75 far). Need `fading: nakagami`, and each other. |
| `radio.channel_model.building_wall_loss_db`, `.building_loss_db_per_m` **new** | 9 dB, 0.4 dB/m (Sommer 2011) | 0 to 40 dB; 0 to 10 dB/m | Sommer β and γ, wherever buildings are charged. Refused with buildings off or under TR 37.885. |
| `radio.channel_model.vehicle_blockage` **new** | on with the geometric law | bool | TR 37.885 NLOSv under any law, or vehicles radio-transparent. |
| `world.sumo.signal_programmes` **new (WP-23)** | `as-file` | `as-file`, `static` | How a SUMO `net.xml`'s `<tlLogic>` programmes are imported. `as-file`: an `actuated`, `delay_based` or `NEMA` programme runs as the live controller its `type` names, with its phases' `minDur`/`maxDur` (and NEMA `name`, `vehext`, `yellow`, `red`) and its `<param>`s; `delay_based` runs as actuated, counted (`delay-based-as-actuated`); every attribute and parameter is counted in the import report's `tl_ledger` (applied, approximated, not modelled). `static`: every programme runs its phase list, as before WP-23. Static programmes import byte-identically either way. SUMO-network worlds only (refused otherwise). |
| `radio.channel_model.foliage` **new** | off | bool | Foliage excess on a path through `world.vegetation`: depth × 0.79·f^0.61 dB/m (GEMV² eq. 4; 2.33 dB/m at 5.9 GHz), capped at the P.526 knife edge over a 15 m canopy (P.833-10 §3.1, used above 1 GHz; [gemv2-p833-constants.md](../design/research/gemv2-p833-constants.md)). Refused without `world.vegetation`. |
| `radio.channel_model.density_sigma` **new** | off | bool | GEMV² eq. 10: the log-distance shadowing σ from the vehicles and building footprints inside each link's ellipse, between Table V's bounds (LOS 3.3-5.2 dB, NLOSv 3.8-5.3, NLOSb 4.1-6.8). Normalising densities are assumptions (11,500 veh/km², 60 % built). |
| `world.vegetation` **new** | false | bool | The OSM import keeps `natural=wood`, `landuse=forest`, `natural=tree_row` (8 m strip) and `natural=tree` (`diameter_crown`, else 8 m) as a vegetation layer. A world without it keeps its content hash. |
| `radio.channel_model.reflection` **new (WP-29)** | off | bool | GEMV² single-bounce building reflection (Boban 2014 §IV-B.3, eqs. 6-8): a building-blocked link also receives the strongest specular ray off a footprint wall whose legs are clear, priced by free-space spreading over the unfolded path (at most Table VI's 300 m) and the TE Fresnel coefficient of the wall material (ITU-R P.2040-3 Table 3; unknown walls are concrete), power-summed with the shadowed path. With `density_sigma`, NLOSb σ_min becomes 0 dB (Table V). Refused without buildings. |
| `world.terrain.drape` **new** | false | bool | Lays lanes, junctions, crossings, buildings, sites and signal heads on `world.terrain.dem` (z += the DEM height; 04-models.md §1.4), so `traffic.vehicle_dynamics.grade` has a slope to read. Off, the DEM is only the radio's ground and the geometry stays flat. Refused without `world.terrain.dem`. A world without it keeps its content hash. |
| `world.buildings.enabled` | true | bool | Buildings exist, and so obstruct. |
| `radio.models.obstacle` | `obstacle/building-shadowing` (Sommer 2011) | presets | Per-wall and per-metre loss. |
| `radio.range.margin_db`, `radio.range.max_m` | 10 dB, unset | 0 to 40 dB; 50 m to 100 km | How far a transmission is followed. This changes cost, not physics. |

## 802.11p medium access (`radio.mac`)

This applies at `radio.tiers.mac: medium` or `high`, and is refused at `abstract`, which
models no access. Broadcast OCB never doubles the window and never retries
(EN 302 663 Annex C.4.2), so `CWmax` and a retry limit would change nothing and are not
offered.

| Path | Default | Range | Source and effect |
|---|---|---|---|
| `radio.mac.periodic_access_category` **new** | `vo` | `vo`, `vi`, `be`, `bk` | Queue of BSM, CAM, PSM, VAM, CPM, SPaT and MAP. SAE J2945/1 uses voice; EN 302 663 Annex C puts CAM at video. |
| `radio.mac.event_access_category` **new** | `vo` | same | Queue of DENM. |
| `radio.mac.cw_min_vo` / `cw_min_vi` **new** | 3 / 7 | 2^k - 1, from 1 to 1023 | Random back-off window (EN 302 663 Table C.4; IEEE 802.11-2020 §9.4.2.28). |
| `radio.mac.aifsn_vo` / `aifsn_vi` **new** | 2 / 3 | 2 to 15 | `AIFS = AIFSN·13 µs + 32 µs`. |
| `radio.mac.queue_length` **new** | 64 | 1 to 1024 frames | Per-category queue. A full queue drops (`mac_drops`). |
| `net.channel_access` | `continuous` | `continuous`, `alternating` | IEEE 1609.4 channel switching. `alternating` gates the EDCA MAC (WP-16): a one-radio unit sends and hears the safety channel only in the CCH interval and the service channel only in the SCH interval; no frame starts in a 4 ms guard or crosses its interval's end (report `mac_gate_deferrals`, `mac_guard_deferrals`, `straddling_frames`). Two-radio units (`radio.devices.*.radios: 2`) do not switch. Interval lengths UNVERIFIED. |

## C-V2X sidelink (`radio.sidelink`, `radio.models.sidelink`)

Refused on an 802.11p run.

| Path | Default | Range | Source and effect |
|---|---|---|---|
| `radio.models.sidelink.params.profile` | region's (SAE J3161/1 US LTE; EN 303 613 EU LTE; EN 303 798 NR) | profiles | Pool, sub-channels, scheduler. |
| `radio.models.sidelink.params.mcs` | 7 | profile's table | MCS (TS 36.213 / TS 38.214 Table 5.1.3.1-2). |
| `radio.models.sidelink.params.max_transmissions` | 1 | 1 to 3 | Blind retransmissions (HARQ without feedback). |
| `radio.models.sidelink.params.congestion_control` | profile's CR-limit table | `off` or table id | TS 36.213 §14.1.1.4C CR limits. |
| `radio.models.sidelink.params.sizing` **new** | `code-rate` | `code-rate`, `ts-36-213` (LTE, not `molina-masegosa-2017`) | How a transport block is sized: the profile's Qm·R estimate, or TS 36.213 Table 7.1.7.2.1-1 at the MCS's I_TBS (Table 8.6.1-1), as a UE and OpenCV2X do (190 B fits one 15-RB sub-channel at MCS 7). |
| `radio.sidelink.reservation_interval_ms` **new** | 100 | LTE 20, 50, 100, ... 1000; NR 1-99 or a multiple of 100 up to 1000 | Resource reservation interval (TS 36.331). |
| `radio.sidelink.resource_keep_probability` **new** | profile's (0.8 J3161/1) | 0, 0.2, 0.4, 0.6, 0.8 | `probResourceKeep`. |
| `radio.sidelink.rsrp_threshold_dbm` **new** | profile's (-128 NR, -110 LTE studies) | -128 to -60 dBm | Sensing exclusion threshold (TS 36.213 §14.1.1.6), raised 3 dB until enough candidates remain. |
| `radio.sidelink.sensing` **new** | on | bool | `false` is the random-selection baseline (Ali 2021 §II.1). |
| `radio.sidelink.candidate_percent` **new** | 20 | 20 (LTE); 20, 35, 50 (NR) | Share of the selection window that must survive exclusion. |
| `radio.sidelink.selection_window_ms` **new** | profile's T2 | 20-100 (LTE), 1-100 (NR) | End of the selection window T2. |
| `radio.sidelink.noise_figure_db` **new** | 9 dB (TR 36.885 Annex A.1.1) | 0 to 15 dB | The UE receiver's noise figure: every sidelink noise floor and the candidate range. |
| `radio.sidelink.cbr_threshold_dbm` **new** | -94 dBm | -112 to -8 dBm, 2 dB steps (TS 36.331 `sl-ThresS-RSSI-CBR`) | S-RSSI above which a sub-channel counts busy in the CBR that the CR limits and metrics read. |
| `radio.sidelink.align_generation` **new** (WP-34) | off | bool | Each UE generates its periodic BSM just before its next reserved subframe (SAE J3161/1, second-hand), following every reselection; an interval it changes stays inside J2945/1's `vMinITT`..`vMaxITT`. Needs a desynchronised `messages.generator` and a mobility step dividing the RRI. Run report: `sidelink.bsm_aligned`, `bsm_unaligned`, `bsm_realigned_on_reselection`. |
| `radio.sidelink.align_margin_ms` **new** (WP-34) | pool's `T1` (1 ms LTE), ASSUMPTION | 0 to 50 ms | How long before the reserved subframe an aligned BSM reaches the MAC, on top of the device's longest signing time. Only with `align_generation`. |

Numerology is fixed by the profile (µ = 1, 30 kHz for NR), and mode 2 (autonomous) is
the only mode. See below.

## Congestion control (`radio.dcc`, `radio.models.dcc`)

| Path | Default | Range | Source and effect |
|---|---|---|---|
| `radio.models.dcc` | region's: J2945/1 (US), ETSI adaptive (EU) | `dcc/sae/j2945-1-rate-power`, `dcc/etsi/adaptive-ts102687`, `dcc/etsi/reactive-ts102687` | Which algorithm. |
| `radio.dcc.enabled` **new** | on | bool | `false` runs no congestion control at all: full rate and power. |
| `radio.dcc.cbr_target` **new** | 0.68 | 0.1 to 0.95 | ETSI adaptive target (TS 102 687 V1.2.1 Table 3). |
| `radio.dcc.adaptive_alpha` **new** | 0.016 | 0.001 to 0.5 | ETSI adaptive smoothing α (ibid.). |
| `radio.dcc.adaptive_beta` **new** | 0.0012 | 0.0001 to 0.05 | ETSI adaptive gain β (ibid.). |
| `radio.dcc.max_itt_ms` **new** | 600 | 100 to 2000 ms | J2945/1 `vMax_ITT`. |
| `radio.dcc.density_coefficient` **new** | 25 | 1 to 200 vehicles | J2945/1 `B`. |
| `radio.dcc.power_control` **new** | on (J2945/1), off (J3161/1) | bool | J2945/1 §6.3.8 power control. |
| `radio.dcc.min_power_dbm` **new** | 10 | -10 to 20 dBm | J2945/1 `vRP_Min`. |
| `radio.dcc.reactive_table` **new** | `a1` | `a1`, `a2` | TS 102 687 V1.2.1 Annex A Table A.1 (T_on up to 1 ms) or A.2 (up to 500 µs). |
| `radio.dcc.reactive_hold_ms` **new** | 0 | 0 to 5000 ms | Reactive state-hold time (no hysteresis in the tables). |

A number for an algorithm the run does not use is refused. For example, `cbr_target`
under the US default J2945/1 is refused, and so is any number when `enabled: false`.

## Messages (`messages`)

| Path | Default | Range | Source and effect |
|---|---|---|---|
| `messages.sets` | `[bsm]` | `bsm`, `cam`, `denm`, `spat`, `map`, `psm`, `vam`, `cpm`, `srm`, `ssm` | Which messages run. CPM and VAM are enabled here. |
| `messages.rate_hz` **new** | unset (J2945/1 10 Hz; ETSI CAM triggers) | 1 to 50 Hz | A fixed BSM and CAM period: the ns-3 `BsmApplication` and Veins beacon. J2945/1's `vMinITT` follows it, and DCC may still stretch it. Refused beside `messages.generator` interval params. |
| `messages.generator` `generator/bsm-j2945-1` params | 100 / 100 / 600 ms | `nominal_itt_ms`, `min_itt_ms`, `max_itt_ms` | J2945/1 generation. |
| `messages.generator` `generator/cam-en302637-2` params | EN 302 637-2 §6.1.3 | `t_gen_cam_min_ms` 100, `t_gen_cam_max_ms` 1000, thresholds 4°, 4 m, 0.5 m/s, `n_gen_cam` 3 | ETSI CAM triggers. |
| `messages.generator` `generator/timing-phase-jitter` | 100 ms phase window, 10 ms jitter (5 ms under J2945/1) | ms | Desynchronisation. |
| `messages.codec_tier` | `uper` | `uper`, `size-model` | Real encoding or a size model. |
| `messages.spat_interval_ms`, `messages.map_interval_ms` **new** | 100 / 1000 ms (CTI 4501) | 100 to 10 000 / 60 000 ms | Roadside SPaT and MAP periods. Refused without that message in `sets`. |
| `messages.cpm_interval_ms` **new** | 100 ms | 100 to 1000 ms | `T_GenCpm` (ETSI TS 103 324 §6.1.2.3). Refused without `cpm` in `sets`. |
| `messages.cpm_dcc_limited` **new** (WP-12) | off | bool | `T_GenCpm` follows the DCC's minimum gap `T_off`, clamped to [100, 1000] ms (ETSI TS 103 324 V2.1.1 §6.1.2.1, Annex D; Table F.1). Refused without `cpm` in `sets`. |
| `messages.cpm_quality_threshold` **new** (WP-12) | unset (not applied) | 0 to 15 (ratio); TS recommends 3 | `ObjectPerceptionQualityThreshold`: a CPM carries only objects whose §7.1.8.6 quality (α 0.5, weights 1, Table F.4) exceeds it, and states that quality. Refused without `cpm` in `sets`. |
| `messages.cpm_max_objects` **new** (WP-12) | unset (255, perception order) | 1 to 255 objects | Objects per CPM, ordered by the §6.1.3.2 utility (Table F.2 thresholds); the lowest-utility objects past it wait for the next event. Refused without `cpm` in `sets`. |
| `messages.denm_traffic_condition` **new** (WP-12) | off | bool | The C2C-CC RS 2007 R1.6.10 sudden-speed-drop (`dangerousEndOfQueue` 27) and local-slow-down (`trafficCondition` 1) DENMs, non-urban roads only. Refused without `denm` in `sets`. |
| `messages.denm_traction_loss` **new** (WP-12) | off | bool | The C2C-CC RS 2002 R1.6.10 §3.3 traction-loss DENM (`adhesion` 6) from the own ABS's friction estimate (< 0.3 or < 0.2 for 5 s). Refused without `denm` in `sets`. |
| `messages.path_history_points` **new** | 15 | 0 to 23 points | The most path-history points a BSM Part II (or a CAM) carries: SAE J2945/1 recalls 15, J2735 `PathHistoryPointList` allows 23, 0 sends none. About 8 to 9 bytes a point. Refused without `bsm` or `cam` in `sets`. |
| `security.signer_id_policy.full_cert_every_ms` | 1000 | ms | Certificate attach cadence. A digest is sent otherwise (`digest_otherwise`). 0 attaches the certificate to every message. |

## Networking (`net`)

| Path | Default | Range | Source and effect |
|---|---|---|---|
| `net.layer` | `wsmp` | `wsmp`, `gn-btp` | Single-hop WSMP (IEEE 1609.3) or GeoNetworking + BTP (EN 302 636). |
| `net.geonet.hop_limit` | 10 | 1 to 255 | EN 302 636-4-1 Annex H. |
| `net.geonet.area_forwarding` | `cbf` | `cbf`, `simple` | Inside the destination area. |
| `net.geonet.non_area_forwarding` | `greedy` | `greedy`, `cbf` | Towards the area. |
| `net.geonet.cbf_variant` | `etsi` | `etsi`, `dpd`, `gpc` | CBF timer rule (1-100 ms over `DIST_MAX` 1000 m). |
| `net.geonet.area_radius_m` | 1000 | 0 to 1784 m | DENM destination area. |
| `net.geonet.repetition_interval_ms` / `_duration_ms` | 1000 / unset (5000 overlay, 15000 on-air C2C-CC stationary profile) | 0 to 60 s / 0 to 600 s | DENM repetition (EN 302 637-3). An explicit duration, 5000 included, is used as given. |
| `net.geonet.keep_alive_forwarding` | false | bool | Keep-alive forwarding. |
| `net.geonet.cbf_min_time_ms`, `.cbf_max_time_ms` **new** | 1 / 100 ms | 0 to 1000 ms, min below max | `itsGnGeoBroadcastCbfMinTime` / `MaxTime` (EN 302 636-4-1 Annex H). Refused unless a forwarding algorithm is `cbf`. |
| `net.geonet.max_comm_range_m` **new** | 1000 m | (0, 5000] m | `DIST_MAX`, where the CBF timer reaches its minimum (Annex H). |
| `net.fragmenter` | none | model | Fragmentation over the MTU. |

## Security (`security`, `nodes`)

| Path | Default | Range | Source and effect |
|---|---|---|---|
| `security.envelope` | `ieee1609.2` | `ieee1609.2`, `etsi103097` | Secured-message format. |
| `security.signature` | `ecdsa-p256` | schemes in `v2xw_sec::scheme` (ECDSA, ML-DSA, Falcon, ...) | See `signature-schemes.md`. |
| `security.crypto_mode` | `modeled` | `modeled`, `real` | Priced or real mathematics. |
| `security.verification_policy` | `verify-all` | `verify-all`, `on-demand`, `prioritized` | Verify all, verify on demand, or verify the safety-relevant first (verify-before-act). |
| `security.pseudonym_change.strategy` | `time` | `time`, `distance`, `j2945-1` (5 min and 2 km), `nyc-pilot` (2 km or 5 min), `c2c-cc`, `mix-zone`, `silent` | Pseudonym strategy, with `period_s` (1 s to 1 day), `distance_m`, `silent_period_s`, `mix_zone_radius_m`. |
| `nodes.default_obu`, `nodes.per_class` | profile | shipped `obu/*` profiles | Crypto hardware profile: signing and verification service times. |
| `nodes.compute_tier` | `medium` | tiers | Queueing of crypto work. |
| `nodes.verify_queue_depth` **new** | 64 | 1 to 4096 | Verification queue. A full queue drops (`verify_overflow`). |
| `nodes.peer_cert_cache` **new** | 128 | 1 to 65536 | Peer certificates each vehicle or roadside unit holds (LRU, 06-node-models.md §2.2). An evicted peer's next digest-signed message needs a P2PCD request and cannot be verified until its certificate arrives again. |
| `nodes.validated_cert_cache` **new** | 4096 | 1 to 1048576 | Validated end-entity certificates each device remembers (LRU, `rxval::EE_CACHE`); a forgotten signer pays first-sight validation again (`ee_evictions`). |
| `security.scms.crl_overdue_tolerance_s` **new** | 1209600 (two weeks, CAMP 1.2.2 Table 67) | 0 to 31536000 s | How long past a CRL's `nextCrl` (a week after issue) a device keeps trusting the certificates it governs; then it refuses them (`crl-overdue`). See `config-inventory-scms.md`. |
| `security.scms.crl_overdue_scope` **new** | `craca` | `craca`, `all` | What an overdue pseudonym CRL refuses: `craca`, the signers whose CRACA is the overdue list's; `all`, every message while any held series-1 list is past `nextCrl` + the tolerance (CAMP 1.2.2 Table 67: the EE is "rejecting all of them"; the sources leave both readings open). They differ only once a device holds a list nobody renews, after a CRACA changeover. |
| `nodes.hsm_sign_lower_us` **new** | unset (every signature at the profile's upper bound, CRATON2 <9 ms) | (0, 1e6) us, below the bound | Each HSM signature drawn uniformly in [this, bound] from the node's own stream. No typical CRATON2/SECTON value is published (searched 2026-10-13). Refused when the default OBU has no upper-bound signing time. |
| `security.on_demand_threshold` **new** | 0.5 | (0, 1] | Relevance score (1/(1+TTC) from the sender's claimed position, speed and heading against the receiver's own) at or above which an `on-demand` unit verifies a known signer. No published operating point (Krishnan & Weimerskirch 2011). Refused under any other policy. |
| `security.prioritized_horizon_m` **new** | 300 | 10 to 2000 m | Range over which nearness raises a `prioritized` unit's verification priority; the queue now serves the highest priority first. Refused under any other policy. |
| `security.signer_id_policy.on_new_neighbour` **new** | false | bool | Also attach the certificate to the next BSM or CAM after a vehicle hears a station it did not know (ETSI TS 103 097 V1.3.1 §7.1.1), on top of the cadence; a forced certificate does not restart the cadence. Refused when every message already carries one, or with no `bsm`/`cam`. |

## Traffic (`actors.vehicles`, `world.signals`, `apps`)

| Path | Default | Range | Source and effect |
|---|---|---|---|
| `actors.vehicles.demand.rate_veh_per_h` | scenario's | 0 to 10^8 veh/h | Demand level. |
| `actors.vehicles.classes` | all passenger cars | shares in 0-1 | Vehicle mix. |
| `actors.vehicles.equipped_fraction` | 1 | 0 to 1 | Penetration rate. |
| `actors.vehicles.driving.car_following` | IDM, urban HCM calibration | `idm`, `eidm`, `krauss`, `gipps`, `w99`, `acc`, `cacc` + params | Car-following model. |
| `actors.vehicles.drivers.max_accel_mps2` **new** | model's (1.4) | 0.3 to 5 m/s² | IDM `a`, passenger cars. |
| `actors.vehicles.drivers.comfort_decel_mps2` **new** | model's (2.0) | 0.5 to 6 m/s² | IDM `b`. |
| `actors.vehicles.drivers.time_headway_s` **new** | model's (about 1.5) | 0.3 to 4 s | IDM `T`. |
| `actors.vehicles.drivers.min_gap_m` **new** | model's (2.0) | 0.5 to 10 m | IDM `s0`. |
| `actors.vehicles.drivers.politeness` **new** | 0.5 | 0 to 1 | MOBIL `p` (Kesting 2007), all classes. |
| `actors.vehicles.drivers.lane_change_threshold_mps2` **new** | 0.1 | 0 to 2 m/s² | MOBIL `Δa_th`. |
| `actors.vehicles.drivers.distraction` **new (WP-39)** | false | bool | Drivers glance off the road: on-path dwells inverse Gaussian (3.56 s, 1.50 s), off-path glances lognormal (−0.21, 0.50; mean 0.92 s, 3.5 % over 2 s), Morando, Victor & Dozza 2019 (IEEE T-ITS 20(8), Fig. 6); 804 glances an hour, 20.5 % eyes off. The vehicle ahead is seen as last seen while the eyes are off; an FCW or EEBL warning from the vehicle's own unit ends the glance. Native engine only. [driver-distraction.md](../design/research/driver-distraction.md). |
| `world.signals.coordinate`, `.progression_speed_mps` | off | 1 to 40 m/s | Green-wave signal timing. |
| `world.signals.join_m` **new** | off | 0 to 40 m | Joins signalised junctions this close into one plan (netconvert `--tls.join`, 20 m default there). |
| `traffic.signals.short_link_clearance_m` **new** | off | 10 to 200 m | Downstream green for a link shorter than this starts 2 s before the upstream green that fills it (FHWA Signal Timing Manual 2e ch. 6, closely spaced signals). |
| `traffic.signals.detector_placement` **new** | stop-line | stop-line, sumo | `sumo`: one loop per approach lane at speed limit x 2 s upstream (SUMO actuated `detector-gap`); actuated control only. |
| `traffic.signals.control: nema`, `traffic.signals.nema.*` **new, partial (WP-17)** | absent | rings `[1,2,3,4]`/`[5,6,7,8]`; barriers `[4,8]`, `[2,6]`; `lefts` lead-lead/lag-lag/lead-lag; `left_turn_mode` protected-permissive/protected/permissive; `permissive_display` fya/green-ball; left min/max/passage 4/25/2.5 s (2-60, 5-180, 0.2-10 s); `yellow_s` 3-6 s and `red_clear_s` 0-6 s (absent: the plan's own); `walk_s` 4-30 s, `ped_speed_mps` 0.8-1.5 (absent: `world.signals.*`, else 7 s and 1.067 m/s); `min_recall` `[]`, `soft_recall` `[2,6]`, `max_recall` `[]`, `ped_recall` `[]`; `coordination` false with `coordinate_phases` `[2,6]`, `force_off` floating/fixed, `controller_type` ts2/type170 | NEMA dual-ring eight-phase controller: FHWA Signal Timing Manual 2nd ed. (NCHRP 812) Exhibits 5-3, 6-4, 6-9, 6-22; MUTCD 2009 §4D.20, §4D.26, §4E.06; SUMO NEMA parameter names. Runs from WP-17 P17.5 (off by default): the rings rewrite each plan as composite intervals, unmapped junctions keep the single ring. Lefts follow `left_turn_mode` and `permissive_display` (P17.6: FYA window = opposing through, circular green = own approach; a protected-permissive left sharing the through's lane runs permissive). Crosswalks walk on a push-button call from a pedestrian standing at the kerb end, with pedestrian recycle while a green rests, or every cycle under `ped_recall` (P17.7). Coordination (P17.8) keeps the plan's cycle and offset (a green wave's when set) with fixed or floating force-offs; under nema a green wave needs `coordination: true`, and `short_link_clearance_m` is refused; priority requests are served through the rings (P17.9: green extension up to max + 10 s, early green after 7 s), and SPaT `minEndTime` is the rings' earliest possible change. |
| `world.source.params.ew_green_share` **new** | 0.5 | 0.1 to 0.9 | East-west share of a generated grid's green (Webster 1958); signalised grids only. |
| `traffic.turn_curve_window_m` **new** | 0 (whole connector) | 0 to 30 m | Judges a junction turn's radius over its tightest arc of this length instead of its average (SUMO netconvert `--junctions.limit-turn-speed` averages). At 6 m the 85th-percentile peak lateral acceleration equals `turn_lateral_accel_mps2` (5.46 at 5.5, 3.52 at 3.5, AASHTO 0.38 g = 3.73); at 0 it is 7.7 at 5.5. Refused when `turn_lateral_accel_mps2` is 0. |
| `traffic.vehicle_dynamics.grade` **new** | false | bool | Subtracts g·sin θ of the draped lane (10 m chord, clamped at 0.3) from the driver's acceleration, as SUMO KraussPS does; an upper bound only, so braking and downhill speed are unchanged. Flat maps are byte-identical. Native engine only. |
| `traffic.vehicle_dynamics.power_limit` **new** | false | bool | Caps acceleration at η·P/(m·v) less rolling and air resistance, for passenger cars (PHEMlight PC_D_EU4: 93 kW, 1,500 kg) and trailers (Green Book design truck 120 kg/kW at 36.3 t, C_dA 5.95 m², c_rr 0.0077), the rigid truck (NCHRP 505 Table 30, 82.9 lb/hp), the bus (Altoona 1211-P XD40, η 0.54), the coach (Altoona 1209 X3-45, η 0.84) and the van (EPA MY2020 Transit road load), with a rolling speed term that is zero on the car and trailer rows; emergency vehicles and two-wheelers have no sourced row. With `grade` the trailer crawls at 12.9 m/s (46 km/h) on 5 %. Binds on flat roads at speed. [grade-dynamics.md](../design/research/grade-dynamics.md). |
| `traffic.emergency_yield.enabled` **new (WP-20)** | false | bool | A driver with an emergency vehicle running its beacons within the reaction distance behind, on the same carriageway, yields by the jurisdiction's rule (`v2xw_mobility::rules::EmergencyYield`): US presets move to the right-hand lane and stop clear of the junction until it has passed (NY VTL §1144(a); UVC §11-405 is the model); one who cannot move right and is in its lane drives on. Native engine only. |
| `traffic.emergency_yield.reaction_distance_m` **new (WP-20)** | 25 m | 1 to 500 m | Gap from the emergency vehicle's front bumper to the driver's rear at which they react (SUMO bluelight device `reactiondist` default; calibrate against siren audibility). Needs `enabled`. |
| `traffic.emergency_yield.red_pass_speed_mps` **new (WP-20)** | 0 m/s | 0 to 10 m/s | An emergency vehicle at a red stops (0) or slows to this at the line, then passes once nothing is inside on a conflicting path, the exit has room and no conflicting vehicle will reach its line within 4 s (UVC §11-106(b)2: "only after slowing down as may be necessary"; 0 is fire-service practice, NFPA 1451 §7.1.3 as quoted, secondary). Tagged as an intended red entry. Needs `enabled`. |
| `apps.glosa_compliance` | scenario's | 0 to 1 | Share of drivers who follow advice. |
| `actors.vru.red_crossing_share` | scenario's | 0 to 1 | Pedestrian non-compliance. |
| `actors.vru.walking_speed_mps` **new** | 1.51 (`observed`), 1.34 (`document`) | 0.5 to 2.5 m/s | Mean walking speed under 65 (Knoblauch 1996); older walkers scale with it. |

The traffic audit (2026-10-11) added further traffic fields: free-speed factor and
spread, start-up reaction, queue start-up delay, per-class drivers (truck, bus,
motorcycle, bicycle), junction gaps and turning speed, kerbside stops, OSM signal cycle,
all-red and pedestrian timing, and GNSS receiver settings (`actors.vehicles.gnss`).
[config-inventory-traffic.md](config-inventory-traffic.md) lists them with their
defaults, ranges and sources, together with every parameter left hard-coded and the
reason.

The IDM `delta` exponent and the other model internals are parameters of the chosen
model: `actors.vehicles.driving.car_following.params`, published from the model's card.
`actors.vehicles.drivers` is refused with `mobility.provider: sumo`, since SUMO's `vType`
holds those drivers.

## Run (`time`, `seed`, `experiment`)

| Path | Default | Range | Source and effect |
|---|---|---|---|
| `time.duration_s` | 60 | > 0 s | Run length. |
| `seed` | scenario's | u64 | Keyed RNG master seed. Same seed and settings give the same run. |
| `experiment.seeds`, `experiment.replications` | none | | Replications for confidence intervals. |
| `time.warmup_s` **new** | none | 0 to less than `duration_s` | Metric samples whose window ends inside it are not recorded (Law, *Simulation Modeling and Analysis*, 5th ed., §9.5.1). The run report's whole-run counters still count it. |
| `time.metric_period_s` | 1 | 0.1 to 3600 s | Metric sampling interval. |
| `time.mobility_step_ms` | 100 | 10 to 100 ms | Mobility step (ADR 0004). |
| `events[].when` **new** (WP-21) | none | `ttc_below {ttc_s}` (0 to 60 s), `in_polygon {points_m}` (≥ 3 corners), `speed_diff_above {speed_diff_mps}` (0 to 100 m/s, leader within 100 m), `probability {p}` (0 to 1), or one `all`/`any` `{of: [...]}` of them | Condition-triggered item: armed at `t`, fires at the first mobility step the clause holds on ground truth (the scenario director's hand, not a detector). TTC is WP-11's gap over closing speed on the time grid. A `safety.*` `target` may be `when.leader`, `when.follower` (pair clauses) or `when.actor` (area). Probability draws are keyed by seed, item and step. Not on `attack.wave`. Design reference Artery's storyboard (behaviour only). |
| `events[].max_fires` **new** | 1 | 1 to 10⁶ | Firings of a `when` item. |
| `events[].cooldown_s` **new** | 0 | ≥ 0 s | Least time between two firings of a `when` item. |

## Not configurable yet

| Knob | Why it is not offered |
|---|---|
| 802.11p 20 MHz channels | The PHY, the error model and the sensitivity tables are 10 MHz, and 802.11p is refused on a wider channel. A 20 MHz PHY needs its own timing and PER curves. |
| `CWmax`, retry limit (802.11p) | Broadcast OCB has no ACK. CW never doubles and nothing is retried (EN 302 663 Annex C.4.2), so these would change nothing. |
| CWmin/AIFSN for `be`/`bk` | Safety traffic uses `vo`/`vi`. If one of these categories is moved to `be`/`bk`, it runs the standard's values. |
| NR numerology other than the profile's | Sub-channel and slot structure come from the profile's transcribed link-level curves (Lusvarghi 2024 at µ = 1). |
| Sidelink mode 1 (network-scheduled) | No gNB/eNB scheduler is modelled. |
| Sub-channel size and count | Fixed by the profile (J3161/1, EN 303 613, EN 303 798), because the BLER curves were computed for those pools. |
| LIMERIC as such | The ETSI adaptive approach is TS 102 687's LIMERIC-derived algorithm, and its α, β and target are above. |
| Signal timing plan per junction | Fixed-time plans come from the map. Coordination, cycle length, all-red and pedestrian timing are configurable for every junction at once (`world.signals`), not per junction. |

## Tests

`crates/v2xw-engine/tests/researcher_config.rs` runs a baseline and the setting side by
side on the same seed for each area:

- **PHY:** 27 Mbit/s and a 15 dB noise figure lower delivery.
- **MAC:** CWmin 1023 multiplies the access delay.
- **Channel:** exponent 3.8, and Rayleigh fading, decode less than free space with no fading.
- **DCC:** ETSI α = 0.5 gates frames, and DCC off records no DCC state.
- **Sidelink:** random selection changes collisions.
- **Traffic:** timid drivers are slower.
- **Messages:** 5 Hz halves the frames.
- **Run:** the warm-up drops early samples.
- **Loader:** it refuses the meaningless settings.

`crates/v2xw-engine/tests/network_audit_config.rs` does the same for the network audit's
fields: sidelink noise figure and CBR threshold, the reactive DCC table, CBF timers, SPaT, MAP
and CPM intervals, the verification queue depth, and the loader's refusals. Every number found in
the network crates, exposed or kept and why, is listed in `config-inventory-network.md`.

`v2xw sensitivity --catalogue` sweeps every published numeric and choice setting at once.
It runs each one at both ends of its recommended range, or at each of its choices, against a
same-seed base on a small family of scenarios (`scenarios/sensitivity/catalogue.yaml`). It
then lists the settings that move no metric, the effects that contradict
`crates/v2xw-engine/tests/expectations/*.yaml`, and the values every family refuses. The
result is `docs/validation/sensitivity-catalogue.md`, and
`crates/v2xw-engine/tests/sensitivity_catalogue.rs` is its fast CI subset.
