# The OMNeT++ V2X ecosystem against this engine: Veins, INET, Artery, Vanetza, OpenCV2X

Written 2026-10-22. Read-only study: no engine code changed and nothing built. It covers
what the OMNeT++ frameworks model, how each item compares with this engine, and what is
worth bringing in. It extends `docs/validation/radio-fidelity.md` §2 and §7, which already
compare the 802.11p error model, CAM generation, the DCC gatekeeper and Mode 4 SPS line by
line. Those items are only summarised here.

## 1. What was read

Shallow clones (`git clone --depth 1`, sparse where large), with only the parts that matter
read:

| Framework | Commit read | Parts read |
|---|---|---|
| Veins (`sommer/veins`) | `7fff7bf` (2026-06-16) | `modules/analogueModel/*`, `modules/phy/{Decider80211p,NistErrorRate,PhyLayer80211p,SampledAntenna1D}`, `modules/mac/ieee80211p/Mac1609_4`, `modules/obstacle/VehicleObstacleControl`, `utility/Consts80211p.h`, `examples/veins`, `subprojects/veins_catch` |
| INET (`inet-framework/inet`) | `02f24a3` (2026-10-22) | `physicallayer/wireless/{common,ieee80211}`: analog models, path loss, obstacle loss, antennas, error models, medium caches, bit-level OFDM |
| Artery (`riebl/artery`) | `e0ddd8c` (2026-07-30) | `application/*` (CA, DEN and its use cases, middleware, multi-channel policy, VehicleDataProvider), `envmod/*` (environment model, sensors, CpService), `storyboard/*`, `inet/gemv2/*`, `scenarios/artery` |
| Vanetza (`riebl/vanetza`) | `bb42fff` (2026-10-20) | `geonet/*` (router, CBF, location table, DCC information sharing), `dcc/*`, `security/*` and `security/v3/*` |
| OpenCV2X / SimuLTE Mode 4 (`brianmc95/simulte`) | `4b5d33d` (2021-09-06) | `stack/phy/layer/LtePhyVUeMode4`, `stack/mac/layer/LteMacVUeMode4`, `simulations/Mode4/config_channel.xml`. The `brianmc95/OpenCV2X` wrapper is Artery plus submodules, and its Mode 4 code is this SimuLTE fork. |

## 2. Licences: nothing may be copied

| Framework | Licence | What it means for this Apache-2.0 engine |
|---|---|---|
| Veins | GPL-2.0-or-later (some files also CC-BY-SA-4.0) | Do not copy code. Re-implement from the papers, and cite them. |
| Artery, incl. its GEMV² port | GPL-2.0 | Do not copy code. GEMV² is re-implemented from Boban et al. 2014. |
| OpenCV2X wrapper | GPL-2.0 | Do not copy code. |
| INET | LGPL-3.0 | Copying source makes that part LGPL. Treat it as no-copy and re-implement from the standard or paper. |
| Vanetza | LGPL-3.0 (+ GPL-3.0 text) | Same as INET. |
| SimuLTE (Mode 4 fork) | LGPL-3.0 | Same as INET. |

So none of these frameworks can be ported as code. Each item below is a
**re-implementation from the primary source** (a standard, a paper or a measurement set),
and the framework is used only as a numeric oracle (its published outputs, or a run of it
kept outside this repository).

**Licence hygiene in the current tree.** This is a finding, not legal advice:

- `crates/v2xw-msg/tests/artery_cam.rs` says it transcribes Artery's
  `CaService::checkTriggeringConditions`/`sendCam`/`genCamDcc` "field for field" into an
  Apache-2.0 test. Artery is GPL-2.0. Rewrite the oracle from EN 302 637-2 V1.3.2 §6.1.3
  (the rules it encodes are the standard's), keep the expected CAM counts as data, and
  reword the header so that it cites the standard, not the code.
- `crates/v2xw-radio/src/dcc.rs` (`regate`) says it is "Transcribed from Vanetza's
  `LimericBudget::update`" (LGPL-3.0). The equation is TS 102 687 V1.2.1 Annex B, B.2.
  Re-derive it from the standard and say so. Vanetza's 75 ms result can stay as a
  cross-check value.
- `crates/v2xw-radio/src/per.rs` takes its NIST distance-spectrum coefficients from
  `nist_per.py` / ns-3 (GPL-2.0). The coefficients come from Miller's NIST report
  (a US-government work). Cite Tables 3.1.1-3.1.3 of that report as the source.

## 3. Component table

Classes: **ENGINE BETTER**, **EQUIVALENT**, **FRAMEWORK BETTER** (we should improve),
**MISSING** (we should add), **NOT USEFUL**.

### 3.1 Veins: PHY, channel and MAC

| Framework component (defaults, source) | Engine counterpart | Class |
|---|---|---|
| `Decider80211p` + `NistErrorRate`: minimum SINR over the frame after the 32 µs preamble, SIGNAL and DATA trials | `phy.rs` + `per.rs`: the same NIST curves, with SINR apportioned per interference window (ns-3 chunks); a preamble-lock loss at the high tier; hidden-terminal labels | ENGINE BETTER (radio-fidelity §7.1: never more pessimistic, exact under constant interference) |
| `SimplePathlossModel` (α = 2), `BreakpointPathlossModel` | `prop.rs`: free space, two-ray ground, dual-slope (Abbas), TR 37.885, WINNER+ B1, geometric urban | ENGINE BETTER |
| `TwoRayInterferenceModel`: phase-exact ground reflection with Fresnel coefficient, `DielectricConstant` ε_r (1.02 in `veins_catch`; Sommer et al., IEEE VNC 2012) | Not shipped: `prop.rs:42` says "two-ray-interference: ground permittivity ε_r UNVERIFIED (not in cache)" | **MISSING** (P5) |
| `NakagamiFading`: m = 1.5 below 80 m and 0.75 beyond, i.i.d. per frame; `constM` option | `fading.rs`: Nakagami-m per link per frame, with keyed draws; fixed, Yin or near/far profiles; Doppler coherence option | ENGINE BETTER |
| `SimpleObstacleShadowing`: β 9 dB per wall, γ 0.4 dB/m (Sommer 2011) | `obstacle.rs`: the same law, six fitted rows, an R-tree, a street-canyon ceiling, Mangel corners | ENGINE BETTER |
| `VehicleObstacleShadowing`: multiple knife-edge over vehicle heights (`getVehicleAttenuationDZ`) | `obstacle.rs`: TR 37.885 NLOSv per antenna-height case, plus a Boban knife-edge with Deygout or ITU-R modified Epstein-Peterson, from the actual bodies | ENGINE BETTER |
| `SampledAntenna1D`: a measured azimuth pattern from samples, with random offset and rotation | `antenna.rs`: TR 37.885 Option 1 panels only; `antenna.rs:195` says "No measured installed pattern" | **FRAMEWORK BETTER** (P6) |
| `Mac1609_4`: EDCA per AC per channel, CCH/SCH queues, `useServiceChannel`, 50 ms intervals with a 4 ms guard (`Consts80211p.h`), `timeLeftInSlot()` refuses a frame that would cross the guard, `changeServiceChannel` | `mac.rs` EDCA (single channel); `net/stack/wave.rs` alternating access as an **application-layer deferral** only (`run.rs:4954`): no SCH traffic, no SCH queue, no guard-crossing check | **FRAMEWORK BETTER** (P1) |
| `Mac1609_4` unicast with ACK and retries (`useAcks`, retry limits; refused with channel switching) | `mac.rs` broadcast OCB (no ACK, by EN 302 663); a unicast retry limit in `dissem.rs` for GN hops | EQUIVALENT (safety traffic is broadcast) |
| WSA (`DemoBaseApplLayer::sendWSA`/`onWSA`, then switch to the advertised SCH) | `wave.rs`: the WSA schedule and the user's decision; the WSA octets are not encoded and nothing is carried on the SCH | **FRAMEWORK BETTER** (P1, P2) |
| TraCI coupling to SUMO (`TraCIScenarioManager`, launchd) | own mobility engine, SUMO net import, `v2xw-interop/src/traci.rs` | ENGINE BETTER (one deterministic process; TraCI kept for interop) |
| Plexe platooning (Segata 2014, separate project) | `v2xw-mobility/src/carfollowing/cacc.rs`, `coop.rs` | EQUIVALENT |

### 3.2 INET: radio medium and 802.11 PHY

| Framework component | Engine counterpart | Class |
|---|---|---|
| Abstraction levels: unit disk, scalar, dimensional (time-frequency power) analog models; packet level and bit/symbol level (convolutional code, interleaver, OFDM (de)modulator) | `radio.tiers.{phy,propagation,mac}` = abstract / medium / high, plus `abstract_tier.rs` | EQUIVALENT for V2X scale. The bit level is NOT USEFUL at 10⁴ nodes, but see P12. |
| Dimensional analog model: exact partial-band overlap of signals | ACLR/ACS masks and ACIR in `regulation.rs`, used by the jamming and coexistence path (`run/jamming.rs`) | EQUIVALENT on the 10 MHz grid. It matters only once two of our own channels are live (P1). |
| Path loss: FreeSpace, TwoRayGround, **TwoRayInterference**, Breakpoint, LogNormalShadowing, Nakagami, Rayleigh, **Rician**, SUI | `prop.rs`, `fading.rs` (Rice approximated by a Nakagami m from K: `fading.rs:183`) | EQUIVALENT, except two-ray interference (P5). A true Rician draw is low value without a sourced K. |
| `DielectricObstacleLoss`: Fresnel at each face plus a loss-tangent attenuation, from 3D material objects | the Sommer empirical β/γ, with material classes (`MaterialClass`) | NOT USEFUL now: needs per-building permittivity and conductivity that maps do not carry, and the measured V2X fits are better validated. |
| Antennas: isotropic, constant gain, dipole, cosine, parabolic, axially symmetric, **interpolating** (sampled) | `antenna.rs` TR 37.885 | **FRAMEWORK BETTER** (P6, the same item as Veins `SampledAntenna1D`) |
| Error models: NIST, YANS, **BER table** (link-level curves from a file), bit-level OFDM | `per.rs` NIST plus the implementation-loss presets; table-driven BLER for C-V2X only (`bler.rs`, `bler_nr.rs`) | **FRAMEWORK BETTER**: no table-driven 802.11p PER (P12) |
| Medium caches: grid / quadtree / neighbour-list neighbour caches, `MediumLimitCache`, range filters | spatial index, `radio.range.margin_db` / `max_m` | EQUIVALENT |
| Background noise (isotropic scalar, dimensional) | thermal noise + NF in `phy.rs`; jamming sources | EQUIVALENT |
| Energy consumers (state-based) | `v2xw-energy` | EQUIVALENT |
| 802.11 modes beyond 10 MHz OFDM (20 MHz, HT/VHT) | 802.11p 10 MHz only; 20 MHz refused (configuration reference, "Not configurable yet") | NOT USEFUL for US deployment; 802.11bd is a later question |

### 3.3 Artery: facilities, middleware, environment model, storyboard, GEMV²

| Framework component (defaults) | Engine counterpart | Class |
|---|---|---|
| `CaService`: EN 302 637-2 triggers (0.1/1 s, 4°, 4 m, 0.5 m/s), DCC-clamped `T_GenCam` | `generator/cam-en302637-2`, matched check for check (radio-fidelity §7.2) | EQUIVALENT (the engine fixed its 2 s DCC case against Artery) |
| `DenService` use cases: EmergencyBrakeLight, **TrafficJam** (end of queue / jam ahead), **TractionLoss**, ImpactReduction | `v2xw-node/src/events.rs`: EEBL (C2C-CC RS 2003, 6 / 2.5 m/s²) and stationary vehicle (RS 2006); DENM repetition and keep-alive | **MISSING**: traffic jam and adverse-weather/adhesion triggers (P9) |
| `CpService` (TS 103 324): DCC-restricted `T_GenCpm` (`genCpmDcc`), object utility / priority function (min/max thresholds for position, speed, heading, age), `objectPerceptionQualityThreshold` 3, sensor-info period 1 s | `v2xw-node/src/cpm.rs`: Type-A/B inclusion rules, `T_GenCpm`, 255-object cap; `cpm.rs:22` says the quality threshold is not modelled; no DCC limit; no utility ordering | **FRAMEWORK BETTER** (P3b) |
| Environment model: GlobalEnvironmentModel (truth), LocalEnvironmentModel (per-vehicle tracks), CamSensor (objects learned from CAMs) | `v2xw-engine/src/perception.rs` (truth to perceived objects, M-of-N tracking, 1 s drop); the LDM in `v2xw-threat/obs.rs` | ENGINE BETTER on errors and tracking |
| Sensors: front/rear long- and short-range radar, FOV sensors with `numSegments`, **visible-corner occlusion** against building polygons and vehicle outlines, **RSU sensors** (FOV, radar, see-through), per-vehicle-type sensor XML | `perception.rs`: one fixed front suite (LRR 250 m ±9°, SRR 70 m ±45°, camera 80 m ±26°), detection probability and position error, occlusion by **one line** to the object with occluders as **discs** (`perception.rs:387`); no rear sensors, no RSU sensors, no sensor configuration | **FRAMEWORK BETTER** (P3a) |
| Middleware + `MultiChannelPolicy` / `XmlMultiChannelPolicy`: services mapped to channels, two radios (`wlan[0]` ch 180, `wlan[1]` ch 176 in `inet_mco`) | single radio, single channel per run | **MISSING** (P1) |
| `VehicleDataProvider`: curvature = yaw rate / speed with a confidence table; 100 ms update | `v2xw-node/src/vehicle.rs` yaw rate; `v2xw-msg/src/j2945.rs` path prediction R = v/ψ̇ | EQUIVALENT |
| Storyboard: Python-scripted stories with **conditions** (time, TTC, polygon, speed difference, car set, likelihood, limit, AND/OR) and **effects** (stop, speed, emergency stop, signal) | scenario `events` timeline (`schema.rs:3204`): **time-triggered** demand, weather, attack, outage, param change, closure, scripted stop | **FRAMEWORK BETTER** (P7) |
| GEMV² (Boban 2014): link classes LOS / NLOSv / NLOSb / **NLOSf (foliage)**; NLOSb by single-bounce reflection off buildings and vehicles (ε_r 4.5 buildings, 6 vehicles) and diffraction; **small-scale σ scaled by vehicle and obstacle density** in the link's ellipse (LOS 3.3-5.2 dB, NLOSv 3.8-5.3, NLOSb 4.1-6.8) | TR 37.885 LOS/NLOS/NLOSv state machine; AR(1) shadowing with a **fixed σ per law**; no foliage | **FRAMEWORK BETTER** (P4) |
| Testbed: real-time scheduler and a sim0mq bridge to a real OBU (hardware in the loop) | none (no wall-clock pacing or device bridge) | **MISSING** (P10) |
| OTS and transfusion couplings | `v2xw-py`, server API | NOT USEFUL |
| VRU service | none upstream | ENGINE BETTER (`vam.rs`, `vru.rs`, social-force pedestrians) |

### 3.4 Vanetza: GeoNetworking, DCC, security

| Framework component | Engine counterpart | Class |
|---|---|---|
| GN router: SHB, TSB, GBC, GUC, beacons, greedy, CBF with duplicate handling, location table, packet buffers | `v2xw-net/src/stack/router.rs` (SHB, TSB, GBC, GUC, greedy, CBF in three variants), `loct.rs`, `mib.rs` | EQUIVALENT |
| **Location Service** (LS request/reply for GUC to an unknown position), GeoAnycast | `router.rs:102` says "no location service here" | **MISSING**, low value (P11) |
| **DCC information sharing** (TS 102 636-4-2): `CbrAggregator` builds `CBR_G = max(CBR_L_prev, one-hop, two-hop)`, one- and two-hop values from neighbours' `dcc_mco` fields, a max-or-second-max rule against the target | `dcc.rs:733` says "No DCC_NET one-hop CBR sharing (TS 102 636-4-2)" | **MISSING** (P8) |
| DCC: reactive (gradual and fully meshed state machines), LIMERIC (Annex B gatekeeper, B.1 and B.2), bursty TRC, flow control per DCC profile | `dcc.rs`: ETSI adaptive (with B.2 `regate`), reactive A.1/A.2, cross TS 103 175, SAE J2945/1 rate and power | EQUIVALENT (the engine adds the US algorithm and the EN 302 571 floor) |
| Security: TS 103 097 v1.2/v1.3, sign policy (the certificate once per second, or when a peer requests its digest; P2PCD), verify reports (false signature, revoked, inconsistent chain, invalid timestamp, duplicate, invalid mobility data), certificate cache, region checker. Artery runs with no security entity by default (`Vanetza.ned`), and with the `Null` crypto backend when one is enabled (`SecurityEntity.ned`) | `v2xw-sec`: IEEE 1609.2 and ETSI envelopes, full SCMS chain, CRL/CTL, linkage, butterfly keys, P2PCD, region and SSP chain checks, replay/generation-time windows (`v2xw-node/src/{rxval,clock}.rs`), modelled or real crypto with hardware timing, PQ schemes | ENGINE BETTER |
| Facilities: path history, CAM functions | `j2945.rs` path history and prediction; CAM generator | EQUIVALENT |

### 3.5 OpenCV2X / SimuLTE Mode 4

| Framework component | Engine counterpart | Class |
|---|---|---|
| SPS: reselection counter, `probResourceKeep`, 1,000-subframe sensing, RSRP exclusion steps, best 20 % by S-RSSI, half-duplex exclusion | `sps.rs`, `run/sidelink.rs` (and Rel-16 re-evaluation, pre-emption, CR limits, NR Mode 2) | ENGINE BETTER (radio-fidelity §7.4: no mechanism differs) |
| PSCCH (SCI) decoding before PSSCH; resources scheduled around from decoded SCIs | `run/sidelink.rs`: reservations only from SCIs it decoded, with interference | EQUIVALENT |
| Channel: WINNER+ B1, shadowing, Nakagami, `ue-noise-figure` 9, `cable-loss` 2 (`config_channel.xml`) | all available; NF 9 dB default (TR 36.885) | EQUIVALENT |
| 190 B in one 15-RB sub-channel at MCS 7 (TBS table) | `radio.models.sidelink.params.sizing: ts-36-213` | EQUIVALENT now (it was the open item in radio-fidelity §6) |
| In-band emission | modelled (`LossCause::InBandEmission`) | ENGINE BETTER (not in OpenCV2X) |
| Mode 3 / network scheduling (SimuLTE eNB) | not modelled (configuration reference) | NOT USEFUL for US PC5 deployments |

## 4. Prioritised list: what to re-implement, and how

Effort: S ≤ 2 days, M ≈ 1 week, L ≥ 2 weeks. Each item is re-implemented from the cited
primary source, with no framework code copied.

| # | What exactly | Why it raises realism | Effort | How to bring it in |
|---|---|---|---|---|
| **P1** | **Multi-channel operation.** (a) A per-message channel map (Artery `MultiChannelPolicy`): BSM on the safety channel, and SPaT/MAP/TIM/WSA/CPM on their own channels, from the region's band plan. (b) An optional second radio per device. (c) 1609.4 alternating access inside the MAC, not as app-layer jitter: per-channel EDCA queues, no transmission in the guard, and Veins' `timeLeftInSlot` rule (a frame that would cross the interval end waits). (d) Leakage between our own adjacent channels through the existing `regulation.rs` ACLR/ACS into the receiver's noise. | Real US RSUs run several channels, and an RSU's SPaT/MAP and the OBU's BSM do not share one queue. Today every message type contends on one channel, which overstates CBR for safety traffic and hides cross-channel leakage. Alternating mode today never splits a frame at the guard. | L | The PHY is already keyed by `frame.channel` (`phy.rs:680`) and the MAC state by `(node, ch)`. Add `radio.channels` (a list with message-set mapping) and `radio.devices.*.radios`; move the `wave.rs` schedule into `mac.rs` as a gate on `initiate_transmit`. Cite IEEE 1609.4-2016 §6.2.5 and SAE J2945/1 for the US plan, and EN 302 663 / TS 103 141 for the EU plan. |
| **P2** | **RSU-mediated IP over the air**: WSA (with a WAVE Routing Advertisement) encoded per IEEE 1609.3-2020 §8, and an 802.11p/PC5 `PacketPath` (the SCH hop from OBU to RSU) under the existing TCP/TLS model (`ip_transport.rs`), for pseudonym-certificate downloads, CRL fetches and misbehaviour-report uploads through RSUs | The SCMS realism goal: in the US deployment many OBUs reach the SCMS through RSUs. Download success then depends on contact time, SCH contention and TCP over a lossy air hop, not only on the backhaul. Veins has the WSA/SCH mechanism, but no SCMS. | M (after P1) | Implement `PacketPath` for the SCH radio of P1. Encode the WSA from the 1609.3 ASN.1 with the existing codec forge. Cite IEEE 1609.3-2020 and 1609.2.1 §6.3. |
| **P3a** | **Configurable sensors, RSU sensors and outline occlusion.** A sensor suite per vehicle class (front and **rear** radars, cameras; mounting point), **RSU-mounted** sensors (camera or radar at junctions, which produce infrastructure CPMs), and occlusion tested against the target's **outline corners** (visible when any corner is visible), with occluders as oriented rectangles instead of discs | Artery detects partly hidden objects and is blind behind a truck's real outline. The engine's single centre line with disc occluders gets both wrong at junctions. Infrastructure CPM is the deployed CPM use case at intersections, and rear sensors matter for lane change. | M | Extend `perception.rs` `SensorSpec` with a mount and azimuth, and add `perception.sensors` to the scenario schema. Write the occlusion test as polygon/segment intersection (own geometry, after the visibility rule in the Günther et al. CPM papers; cite TS 103 324 Annex for the sensor-information container). |
| **P3b** | **CPM generation details**: a DCC-limited `T_GenCpm` (`[T_GenCpmMin, T_GenCpmMax]` clamp of the DCC interval), object ordering by a utility over position, speed and heading change and age when the message must be cut, and `ObjectPerceptionQualityThreshold` | Under load, real CPMs shrink and slow down. Today the CPM ignores DCC, which overstates its channel load in dense runs. | S | `v2xw-node/src/cpm.rs`: reuse the CAM's DCC clamp. Take the utility and quality rules from TS 103 324 V2.1.1 §6.1.2 and Annex F (Table F.1). |
| **P4** | **GEMV²-style channel enrichment**: (a) a small-scale σ that grows with vehicle density and obstacle density inside the link's ellipse, between the per-class min/max that Boban et al. measured; (b) **foliage loss** per metre through tree polygons (NLOSf), using OSM `natural=wood`, `landuse=forest` and `tree_row`; (c) optionally, single-bounce building reflection for NLOSb when its power beats the Sommer through-wall value | A fixed σ is too narrow on a dense arterial and too wide on an empty road. Suburban and rural tree lines take several dB that the engine never charges today. GEMV² was validated against V2V measurements in Porto and Pittsburgh. | M | Re-implement from Boban, Barros and Tonguz, IEEE TVT 63(9), 2014 (doi 10.1109/TVT.2014.2317803). Foliage from ITU-R P.833 (vegetation) rather than GEMV²'s fixed dB/m. Add it as a modifier card in `obstacle.rs` beside `vehicle_blockage`. |
| **P5** | **Two-ray interference law**: `L = (4π d/λ · 1/|1 + Γ e^{jφ}|)²` with Γ the Fresnel reflection coefficient of ground with ε_r | It is the Veins default for highway studies: it reproduces the measured deep fades at characteristic distances that the d⁻⁴ two-ray model smooths away. The engine lists it as missing only for want of ε_r. | S | The formula is textbook (Rappaport, *Wireless Communications*, §4.6). ε_r = 1.02 is from Sommer, Joerer, Dressler, "On the Applicability of Two-Ray Path Loss Models for Vehicular Network Simulation", IEEE VNC 2012; get the paper into the cache to clear UNVERIFIED. Use the Veins catch test's inputs as a numeric cross-check only. |
| **P6** | **Measured antenna patterns**: load a sampled azimuth (optionally elevation) gain table per device, with interpolation and optional random rotation | Installed roof antennas have ripple and nulls of several dB that change PDR per direction. Veins and INET both support this. | S | New card `antenna/sampled` in `antenna.rs` that reads a CSV of (angle, gain dB). Cite Kornek et al. 2010 (measured vehicle patterns) and Eckhoff, Brummer, Sommer, IEEE VNC 2016 (impact of antenna patterns on VANET simulation). |
| **P7** | **Condition-triggered scenario events**: timeline items that fire on a state condition (TTC below x, a vehicle inside a polygon, a speed difference, a probability, AND/OR), not only at time t | It reproduces safety-critical situations (a hard brake when a follower is at TTC 2 s) independent of the seed, which is how real test catalogues (Euro NCAP-style) are written. | M | Add a `when:` clause to `TimelineItem` with a closed condition enum evaluated each step, and reuse the existing effects (scripted stop, speed). Artery's storyboard is the design reference only. |
| **P8** | **DCC_NET CBR sharing**: carry local and one-hop CBR in the GN SHB/beacon DCC field, and compute `CBR_G` from neighbours (the max or second-max rule against the target) for the ETSI adaptive algorithm | Without it, every ETSI station reacts only to its own CBR. Real stations follow the worst load within two hops, which lowers rates earlier at the edge of congestion. | S-M | `v2xw-net/src/stack/{loct,router}.rs` plus the `dcc.rs` input. Cite ETSI TS 102 636-4-2 (GN media-dependent functionality for ITS-G5) and TS 102 687 V1.2.1. Vanetza's `CbrAggregator` is a behaviour reference only (LGPL). EU stack only. |
| **P9** | **More DENM triggers**: traffic jam ahead / dangerous end of queue, and adverse weather / traction loss (from the weather model plus wheel-slip events) | They are the two most common deployed C-ITS day-one DENMs after EEBL and stationary vehicle. | S each | `v2xw-node/src/events.rs`. Take thresholds from the C2C-CC triggering-condition documents (Traffic condition; Adverse weather), in the same way as RS 2003/2006. |
| **P10** | **Hardware-in-the-loop bridge**: real-time pacing, plus a UDP/ZeroMQ bridge that injects simulated BSMs into a real OBU and reads its transmissions back | It validates the engine's message and security timing against a real device (Cohda MK5/MK6, the profiles the engine already prices). | L | `v2xw-server`: a wall-clock-paced run mode and a bridge crate. Artery `testbed` is the design reference only. |
| **P11** | GN **Location Service** and GeoAnycast | Completeness for GUC to an unknown position. Safety applications rarely use it. | S-M | `router.rs`, EN 302 636-4-1 §10.2.4. |
| **P12** | **Doppler- and fading-aware 802.11p PER tables** (beyond both frameworks: Veins and INET packet level are AWGN NIST) | 802.11p estimates the channel from the preamble only, so long frames at high relative speed fail well above the AWGN SNR. `phy.rs:1448` lists this as missing. | M | Table-driven PER, like the C-V2X BLER path, from published link-level curves per channel profile (e.g. Anwar, Franchi, Fettweis, VTC2019-Fall, 802.11p/bd/LTE-V2X/NR-V2X PHY comparison). Source still to be cached and checked. INET's `Ieee80211BerTableErrorModel` shows the interface shape only. |

**Not worth bringing in:** INET bit-level OFDM (too slow at our scale; P12 covers its
benefit), INET dielectric obstacle loss (no material data), true Rician draws (no sourced
K), SimuLTE Mode 3, 802.11 HT/VHT modes, and Artery's OTS and transfusion couplings.

## 5. What the engine does that these frameworks cannot

- **A full SCMS** (enrolment, butterfly keys, linkage values, CRL/CTL distribution,
  misbehaviour reporting and revocation) and post-quantum and hybrid signature schemes.
  Artery runs without a security entity by default (with the `Null` crypto backend when one is enabled) and has no SCMS.
- **Crypto hardware cost**: per-OBU profiles price sign and verify time, with verify-all,
  on-demand and prioritised policies. Veins and Artery treat security as free.
- **802.11p, LTE-V2X Mode 4 and NR-V2X Mode 2 in one engine, on one channel model**,
  including Rel-16 re-evaluation and pre-emption, in-band emission and CR limits. The
  OMNeT++ world needs Veins + SimuLTE + OpenCV2X + Artery glued together, and has no NR
  Mode 2.
- **A finer PHY**: SINR per window instead of a minimum, preamble capture, and losses
  labelled by cause (`hidden-terminal`, `preamble-missed`, `in-band-emission`).
- **Spatially correlated shadowing** (AR(1), Gudmundson) instead of i.i.d. per frame; TR
  37.885 NLOSv from the actual bodies on the path; Sommer building loss with a
  street-canyon ceiling and corner tracing.
- **Real encodings**: J2735 UPER (BSM, SPaT, MAP, PSM, SRM/SSM) and ETSI CAM, DENM, CPM
  and VAM from the ASN.1, not message stubs.
- **A US deployment profile** (J2945/1 rate and power DCC, J3161/1, the 2020 band plan)
  next to the EU one. Artery is EU-only.
- **Perception with error and tracking** (detection probability, Gaussian position error,
  M-of-N confirmation), where Artery is geometric with perfect detection.
- **Misbehaviour detection and threat models** (`v2xw-threat`), the TCP/TLS backhaul
  model, energy, VRU social-force mobility, its own traffic engine with SUMO import, a
  deterministic single-process run, and a live UI.
