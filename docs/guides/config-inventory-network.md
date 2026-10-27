# Network parameter inventory

The 2026-10-11 audit of every hard-coded number and every unreachable model-struct field
in the network stack: `v2xw-radio`, `v2xw-net`, `v2xw-msg`, `v2xw-sec` and `v2xw-node`,
plus the backend links. Every row says where the number is and either the scenario path
that sets it or why it is kept fixed.

The rule used to classify:

- **IMPORTANT** means it changes a metric a researcher would study (PDR, CBR, latency,
  channel load, verification load). These are exposed under the settings contract: a
  validated scenario field, with its unit, guide text, label and Basic/Advanced level, a
  default equal to today's value, read by the model, and refused where it would do nothing.
- **MINOR** means a numerical tolerance, an implementation detail, a protocol encoding,
  an identifier, or a validation anchor. These are kept.

Protocol constants that a standard fixes (ASN.1 bounds, header sizes, PSIDs, PHY timing)
are kept on purpose. A simulator that let them vary would no longer simulate the protocol.

## Exposed by this audit

Each group has a same-seed test in `crates/v2xw-engine/tests/network_audit_config.rs`.
The test runs a baseline and the setting side by side and requires the output to change.

| Parameter | Where | Exposed as | Default (source) | Range | Test |
|---|---|---|---|---|---|
| Sidelink UE noise figure | `v2xw-radio/src/cv2x.rs` `UE_NOISE_FIGURE_DB` | `radio.sidelink.noise_figure_db` | 9 dB (3GPP TR 36.885 Annex A.1.1) | 0 to 15 dB | 15 dB decodes less |
| Sidelink CBR S-RSSI threshold | `v2xw-radio/src/sidelink.rs` `CBR_SRSSI_THRESHOLD_DBM` → `SpsParams::cbr_threshold_dbm` | `radio.sidelink.cbr_threshold_dbm` | -94 dBm | -112 to -8 dBm in 2 dB steps (TS 36.331 `sl-ThresS-RSSI-CBR`) | -112 dBm raises mean CBR |
| ETSI reactive DCC table | `v2xw-radio/src/dcc.rs` `ReactiveDcc::with_table` | `radio.dcc.reactive_table` | `a1` (TS 102 687 V1.2.1 Table A.1) | `a1`, `a2` | A.2 moves the transmit instants |
| ETSI reactive DCC state hold | `dcc.rs` `ReactiveDcc::with_hold` | `radio.dcc.reactive_hold_ms` | 0 (the tables give no hysteresis) | 0 to 5000 ms | covered by the loader test (reactive only) |
| GN CBF minimum time | `v2xw-net/src/stack/mib.rs` `CBF_MIN_TIME_MS` | `net.geonet.cbf_min_time_ms` | 1 ms (EN 302 636-4-1 Annex H) | 0 to 1000 ms, below the max | |
| GN CBF maximum time | `mib.rs` `CBF_MAX_TIME_MS` | `net.geonet.cbf_max_time_ms` | 100 ms (Annex H) | (0, 1000] ms | 400 ms raises the p95 forwarding latency |
| GN `DIST_MAX` | `mib.rs` `DEFAULT_MAX_COMM_RANGE_M` | `net.geonet.max_comm_range_m` | 1000 m (Annex H) | (0, 5000] m | |
| SPaT interval | `v2xw-node/src/generate.rs` `SPAT_INTERVAL` | `messages.spat_interval_ms` | 100 ms (CTI 4501, 04-models.md §8.1) | 100 to 10 000 ms | 200 ms sends about half the SPaTs |
| MAP interval | `generate.rs` `MAP_INTERVAL` | `messages.map_interval_ms` | 1000 ms (CTI 4501) | 100 to 60 000 ms | 500 ms sends more than 1.5× the MAPs |
| CPM generation interval `T_GenCpm` | `v2xw-node/src/cpm.rs` `T_GEN_CPM` | `messages.cpm_interval_ms` | 100 ms (ETSI TS 103 324 §6.1.2.3) | 100 to 1000 ms (`T_GenCpmMax`) | 500 ms sends fewer CPMs |
| Verification queue depth | `v2xw-node/src/runtime.rs` `NodeConfig::queue_capacity[1]` | `nodes.verify_queue_depth` | 64 (engine default, no profile publishes one) | 1 to 4096 | depth 1 drops (`verify_overflow`) under load |

The loader refuses each field where the run would not read it:

- the sidelink fields on an 802.11p run;
- a CBR threshold off the 2 dB grid;
- the reactive fields unless the reactive algorithm runs;
- the CBF fields when neither forwarding algorithm is `cbf`, and a minimum at or above the maximum;
- the SPaT, MAP and CPM intervals when `messages.sets` lacks that message.

## Already configurable before this audit

These are listed so that the inventory is complete. The paths and sources are in
`configuration-reference.md`.

| Parameter | Where | Exposed as |
|---|---|---|
| 802.11p data rate, noise figure, sensitivity, CCA/CBR threshold, capture threshold | `phy.rs` `NOISE_FIGURE_HARDWARE_DB`, `CBR_BUSY_THRESHOLD_DBM`, `CAPTURE_THRESHOLD_DB` | `radio.phy.*` |
| Per-rate sensitivity tables (`MEASURED_OBU_MARGIN_DB`, ETSI, Cohda) | `phy.rs` | `radio.models.phy` |
| CCA presets ETSI/ns-3/Veins | `phy.rs` `CcaConfig` | `radio.phy.cca_threshold_dbm` |
| PER implementation loss, PER tables | `per.rs` | `radio.models.per` |
| Transmit power, antenna gain, cable loss, height, pattern (OBU, RSU, VRU), sidelink power | `radio.devices` (sidelink uses the device power, `CV2X_TX_POWER_DBM` only labels the card) | `radio.devices.*` |
| Region, channel, EIRP caps, spectrum masks | `regulation.rs` | `radio.region`, `radio.channel`, `radio.adjacent_channel` |
| Own channel map, second radio, own-channel leakage (WP-13) | `run/channels.rs`, `regulation.rs` `own_channel_leakage`, `phy.rs` `leaked_dbm` | `radio.channels: [{channel (channel number), messages: [spat, map, wsa, ...]}]` (default empty: one channel); `radio.devices.{obu,vru}.radios` (default 1), `radio.devices.rsu.radios` (default 2; USDOT RSU Spec v4.1); `radio.channel_leakage` (default on: ACIR from the EN 302 571 mask and Table 8 ACS, TR 36.942 §5.1.1.3) |
| Path-loss law, presets, exponent, shadowing sigma and decorrelation, fading law and shapes | `prop.rs`, `fading.rs`, `winner.rs` | `radio.models.propagation`, `radio.channel_model.*` |
| Sommer building loss β, γ; vehicle blockage | `prop.rs` `SommerCoefficients`, `obstacle.rs` | `radio.channel_model.building_*`, `.vehicle_blockage`, `radio.models.obstacle` |
| Rain attenuation (ITU-R P.838) | `prop.rs` | `weather.*` |
| Candidate range margin | `abstract_tier.rs`, wiring | `radio.range.*` |
| EDCA AC mapping, CWmin/AIFSN vo/vi, MAC queue (`mac.rs` `DEFAULT_QUEUE_DEPTH`) | `mac.rs`, `types.rs` | `radio.mac.*` |
| DCC on/off, ETSI adaptive target/α/β (`AdaptiveParams::TS102687`), J2945/1 `vMax_ITT`, `B`, power control, `vRP_Min` | `dcc.rs` | `radio.dcc.*`, `radio.models.dcc` |
| Sidelink profile, MCS, blind retransmissions, CR-limit table, J3161 rate control, conformance sensitivity | `sidelink.rs`, `sps.rs`, `cv2x.rs` `SIDELINK_SENSITIVITY_DBM` | `radio.models.sidelink.params.*` |
| SPS RRI, keep probability, RSRP threshold, sensing, candidate %, T2 | `sps.rs` `SpsParams` | `radio.sidelink.*` |
| Cellular Uu: background and neighbour load (`DEFAULT_BACKGROUND_LOAD`), up/downlink rate, ISD, sites, loss percentile, MBMS | `cellular.rs`, `cellular_packet.rs` | `net.uu.params.*` |
| Backhaul fibre/DSL/microwave | `ip_transport.rs` | `net.backhaul`, `net.backend_net` |
| Jammer power, period, duty, reaction delay | `jamming.rs` | `threats.jammers[].params` |
| Net layer, GN forwarding, CBF variant, hop limit (`DEFAULT_HOP_LIMIT`), area, repetition, keep-alive | `stack/*`, `gn.rs` | `net.layer`, `net.geonet.*` |
| 1609.4 switching | `stack/wave.rs` | `net.channel_access` |
| Fragmenters: header bytes, reassembly timeout, open SDUs, CPM lifetime, cert-cycle period, P2PCD back-off | `frag/generic.rs`, `frag/facilities.rs`, `frag/cert_cycle.rs` | `net.fragmenter.params.*` |
| BSM ITT, CAM triggers (`ca.rs` thresholds), phase and jitter, fixed rate | `generator.rs`, `stack/ca.rs` | `messages.generator`, `messages.rate_hz` |
| Codec or size model | `codec.rs`, `size_model.rs`, `etsi_size.rs` | `messages.codec_tier` |
| Envelope, signature, KEM, crypto mode, verification policy, certificate attach cadence | `envelope.rs` `SignerIdPolicy` presets, `scheme/*` | `security.*` |
| Pseudonym rotation (`stores.rs` `RotationPolicy` presets) | `stores.rs` | `security.pseudonym_change.*` |
| Crypto hardware timing (`profile.rs`, `profiles/`) | `v2xw-node` | `nodes.default_obu`, `nodes.per_class` |

## Kept fixed

### v2xw-radio

| Parameter | Where | Kept because |
|---|---|---|
| `THERMAL_NOISE_10MHZ_DBM`, `THERMAL_NOISE_DBM_PER_HZ`, `SPEED_OF_LIGHT_M_S`, `FRIIS_CONST_DB` | `phy.rs`, `cv2x.rs`, `numeric.rs`, `prop.rs` | Physical constants. |
| `Q_DB`, `Q_RATIO`, `DEG_PER_RAD`, `ENVELOPE_PAD_M` | `numeric.rs`, `antenna.rs`, `obstacle.rs` | Numerical quanta (determinism contract). |
| `NOISE_FIGURE_3GPP_DB` 9 dB | `phy.rs` | Labels the 3GPP comparison preset. The run's NF is `radio.phy.noise_figure_db`. |
| `TX_INTERVAL_RETENTION_NS` | `phy.rs` | Bookkeeping horizon for the airtime ledger. |
| `SLOT_TIME`, `SIFS`, `PREAMBLE`, `SIGNAL`, `SYMBOL`, `N_SERVICE`, `N_TAIL`, `SIGNAL_BITS` | `types.rs` | IEEE 802.11-2020 §17 OFDM 10 MHz timing. Fixed by the PHY. |
| `A_CW_MIN`, `A_CW_MAX`, BE/BK CWmin/AIFSN | `types.rs` | Standard defaults. No safety message uses BE/BK, and broadcast never doubles CW (EN 302 663 Annex C.4.2). |
| `MAX_MSDU_BYTES`, `MAC_HEADER_*`, `FCS_BYTES`, `LLC_SNAP_BYTES` | `types.rs`, `v2xw-net/frame.rs` | Frame format. |
| `CCH`, `SCH1`, `SCH2` | `types.rs` | Channel identifiers. The channel is `radio.channel`. |
| `T_CBR` 100 ms, `T_DCC` 200 ms | `types.rs` | TS 102 687 measurement and evaluation intervals. Fixed by the standard that the DCC models implement. |
| `En302571Floor::CONFORMANT` | `dcc.rs` | Regulatory floor (EN 302 571). A non-conformant floor is not a study setting. |
| `AdaptiveDcc` `WINDOW` 1 s | `dcc.rs` | TS 102 687 averaging window. |
| `J2945Params::{J2945_1, J3161_1, ROSTAMI}`: PER interval, vMax power, smoothing | `dcc.rs` | SAE J2945/1 Table constants. The four that studies vary (`max_itt_ms`, `density_coefficient`, `power_control`, `min_power_dbm`) are exposed. |
| Reactive CBR state bands (30/40/50/60 %) and rates | `dcc.rs` `ReactiveTable::limits` | These are the tables themselves. The choice between them is `radio.dcc.reactive_table`. |
| `SIDELINK_MAX_INPUT_DBM` -22 dBm | `cv2x.rs` | Receiver overload at under 1 m. It never binds in vehicle spacing. |
| `NR_MCS_TABLE*`, `LTE_*` MCS specs, `REPRESENTATIVE` | `sidelink.rs`, `bler_nr.rs` | Standard tables. The choice is `radio.models.sidelink.params.mcs`. |
| Numerology, sub-channel sizes and counts, PSCCH PRBs | `sidelink.rs` | Fixed by the profile, because the BLER curves were computed for those pools (see the reference's "Not configurable yet"). |
| `SciSize`, `TB_CRC_BITS` | `sidelink.rs` | TS 36.212 / 38.212 formats. |
| `CR_LIMIT_R1_1611594`, `CrLimitTable::{ETSI_TS_103_574, SAE_J3161}` | `sidelink.rs` | Published tables, chosen by `congestion_control`. |
| `Pppp::BSM` 5 | `sidelink.rs` | J3161/1 priority of the BSM. |
| `PREEMPTION_MARGIN_DB` 6 dB | `sps.rs` | NR pre-emption check margin. Studies vary the RSRP threshold it is added to, which is exposed. |
| `IbeMask::adjacent_db` | `sidelink.rs` (card `todo: calibrate`) | Profile in-band emission mask (TS 36.101 §6.5.2A.3). It is a calibration item, not a study knob. |
| BLER and PER look-up tables (`WILAB_*`, `NR_TB_LUTS`, `TABLE_10PCT`, `CONTROL_ADVANTAGE_DB`) | `bler.rs`, `bler_nr.rs`, `per.rs` | Transcribed link-level curves. The choice is a model preset. |
| `D0_M`, `D_MIN_M` | `prop.rs` | Reference distance and singularity guard of the log-distance law. |
| `LogDistancePreset::ALL`, `NakagamiPreset::ALL` | `prop.rs`, `fading.rs` | Published fits, offered as presets. The exponent, sigma and m override them. |
| `MANGEL_*` coefficients | `prop.rs` | Mangel 2011 fitted constants of the geometric law. `path_loss_exponent` switches to the single-slope law. |
| `DECORRELATION_INFRASTRUCTURE_M` 50 m | `prop.rs` | The RSU-link decorrelation. `shadowing_decorrelation_m` overrides decorrelation. |
| `TR37885::RESTATE_INTERVAL` 100 ms | `prop.rs` | LOS-state update cadence (TR 37.885 §6.2.3). |
| `P838_*` | `prop.rs` | ITU-R P.838 coefficients. Weather intensity is `weather.*`. |
| WINNER+ B1 constants (`SIGMA_*`, `HALF_STREET_WIDTH_M`, heights) | `winner.rs` | Published model constants. Sigma is overridable. |
| TR 37.885 antenna pattern (`THETA_3DB_DEG`, `SLA_V_DB`, ...) | `antenna.rs` | Standard pattern. The choice is `radio.devices.obu.antenna_pattern`. |
| Two-ray interference ground permittivity (`GROUND_EPSILON_R` 1.02) | `prop.rs` | Sommer 2012 Figure 4. Overridable: `radio.models.propagation: {id: propagation/two-ray-interference, params: {epsilon_r}}` (1 to 100). |
| Measured antenna table (`antenna/sampled`) | `antenna.rs` | `radio.devices.obu.sampled_pattern: {csv, rotation_deg (0, deg), rotation_jitter (none/uniform/normal), rotation_jitter_deg (0, deg)}`; gains dB relative to `antenna_gain_dbi` (Eckhoff 2016). Unset by default. |
| `DETOUR_RATIO`, `MAX_CANDIDATES`, `MAX_WALL_M`, `GAP_M`, `SNAP_M` | `obstacle.rs` | Building-geometry search limits. |
| `DEFAULT_PROFILE_SAMPLES`, `DEFAULT_MAX_EDGES` | `terrain.rs` | Terrain profile sampling resolution. |
| Band-plan rules and spectrum masks | `regulation.rs` | Regulatory tables, chosen by `radio.region` and `radio.channel`. |
| `DISTANCE_BIN_M`, `RANGE_MAX_M`, `CBR_BIN`, `HEARD_BIN`, `DEFAULT_SEEDS`, `*_TOLERANCE`, `LegacyParams` | `abstract_tier.rs`, `focus.rs` | Calibration grid and tolerances of the abstract tier. |
| `MOLINA_*`, sweep tables | `sweep.rs`, `sweep/*` | Validation anchors. |
| `PUNAL_*`, `BLIND_AREA_TOLERANCE_M` | `jamming.rs` | Validation anchors of the jammer model (Punal et al.). The jammer's own numbers are its params. |
| `ADJACENT_CHANNEL_SOURCE_BASE` | `jamming.rs` | Identifier range. |
| `SHANNON_ALPHA`, `SHANNON_SNR_EFF`, `PF_WINDOW_TTI`, `HIST_BINS`, `ROLLOUT_LIMIT` | `cellular_packet.rs` | Internals of the packet-level Uu scheduler (attenuated Shannon, proportional fair). Load and rates are exposed. |
| `T_IU_MS`, `KNOWN_TARGET_MS`, `T_SEARCH_MS`, `RRC_PROCEDURE_MS`, `EUTRA_TO_UTRA_*` | `cellular.rs` | TS 36.133 handover interruption figures. |
| `MOSAIC_UPLINK_BPS`, `MOSAIC_DOWNLINK_BPS`, `ARRIVAL_WINDOW`, `LOSS_PERCENTILES` | `cellular.rs` | Preset anchors. `uplink_mbps`, `downlink_mbps` and `loss_percentile` override them. |
| `FIBRE_US_PER_KM`, `AIR_US_PER_KM`, `DSL_INTERLEAVE_MS`, `MICROWAVE_OUTAGE` | `ip_transport.rs` | Physical and technology constants of the backhaul presets. |
| `TRAFFIC_MODEL_1_BUDGET`, `DIRECT_ONE_HOP_MS`, `CELLULAR_ONE_HOP_MS`, `OFFLOAD_LOAD_THRESHOLD` | `hybrid.rs` | The hybrid policy is not built by the engine yet (no `radio/hybrid/policy` in the run), so a setting would do nothing. |

### v2xw-net

| Parameter | Where | Kept because |
|---|---|---|
| GN header sizes, BTP header, EtherTypes, LLC/SNAP, `ITS_GN_MAX_*`, `MTU_AL_BYTES`, `BELOW_SECURED_CAM_BYTES` | `netlayer.rs`, `gn.rs` | EN 302 636-4-1 / -5-1 formats. |
| WSMP header sizes, `WSMP_MAX_LENGTH_FIELD_PAYLOAD`, P-encoding limits | `wsmp.rs`, `netlayer.rs` | IEEE 1609.3 formats. |
| `WSM_MAX_PAYLOAD_MIB_DEFAULT` 1400 B | `wsmp.rs` | IEEE 1609.3 MIB default. An oversize SDU is the fragmenter's job (`net.fragmenter`). |
| PSIDs, BTP ports | `netlayer.rs`, `msg/registry.rs` | Identifiers. |
| `LIFETIME_LOC_TE_MS` 20 s, `BEACON_RETRANSMIT_MS` 3 s, `BEACON_MAX_JITTER_MS` | `stack/mib.rs` | Annex H defaults. A station that sends CAMs never beacons (§10.3.6.2), and the location table outlives the warning window. |
| `DEFAULT_PACKET_LIFETIME_MS`, `MAX_PACKET_LIFETIME_MS` | `stack/mib.rs` | Annex H. DENM lifetime is set by the repetition settings and the warning window. |
| `DPL_LENGTH` 8, `MAX_GEO_AREA_SIZE_KM2` | `stack/mib.rs` | Duplicate-list size and an area-validation limit. |
| `DEFAULT_VALIDITY_MS` (stack DEN) | `stack/den.rs` | Used by the protocol-evaluation harness, not the run. |
| `SYNC_INTERVAL_MS` 100, `CCH_INTERVAL_MS` 50, `GUARD_INTERVAL_MS` 4, `CCH_CHANNEL` | `stack/wave.rs` | IEEE 1609.4 fixed intervals. Switching itself is `net.channel_access`. |
| `REASSEMBLY_MAX_FRAGMENTS`, `MAX_FRAGMENTS`, `MAX_OPEN_CERTIFICATES` | `frag.rs`, `frag/*` | Format and memory guards. |
| Cert-cycle size anchors (`ECDSA_CERTIFICATE_BYTES`, `CYCLE_*`, `*_PAYLOAD_CAP_BYTES`) | `frag/cert_cycle.rs` | Published frame-size anchors of the model. |
| `ETSI_SEGMENTATION_THRESHOLD_BYTES`, `CPM_REPEATED_CONTAINERS_BYTES` | `frag/facilities.rs` | TS 103 324 segmentation rule. |
| `AMADOR_*`, `LEARNING_RESPONSE_ANCHORS`, `FRAME_SIZE_ANCHORS` | `stack/eval.rs`, `frag/cert_cycle.rs` | Validation anchors. |
| `Csma::AC_VO` | `stack/dissem.rs` | MAC preset of the evaluation harness. The run's EDCA is `radio.mac`. |
| `ASSUMPTIONS`, `MODELS` | `amplification.rs`, `lib.rs` | Card text and registry. |

There is no unicast retry in the run. Every safety frame is OCB broadcast, so a retry
limit would do nothing (EN 302 663 Annex C.4.2).

### v2xw-msg

| Parameter | Where | Kept because |
|---|---|---|
| About 340 ASN.1 bounds and encodings | `j2735/bsm.rs`, `map.rs`, `spat.rs`, `psm.rs`, `infra.rs` | SAE J2735 definitions. |
| Protocol versions and message ids (CAM, DENM, VAM, CPM, SREM, SSEM) | `cam.rs`, `denm.rs`, `vam.rs`, `cpm.rs`, `node/events.rs` | Identifiers. |
| PSIDs and ITS-AIDs | `registry.rs` | Identifiers. |
| `MAX_PATH_POINTS` 40 | `cam.rs` | EN 302 637-2 limit. |
| `ExteriorLightMask` bits | `cam.rs` | Format. |
| CAM/DENM size-check constants (`ENVELOPE_*`, `AT_*`, `LOWER_LAYERS`) | `cam.rs`, `denm.rs` | Test anchors. |
| DENM `DEFAULT_VALIDITY` 600 s, `MAX_VALIDITY_S`, `FORWARDING_JITTER` 150 ms | `denm.rs` | EN 302 637-3 defaults. Repetition and area are `net.geonet.*`. |
| Size-model coefficients (`CPM_*_B`, `ENVELOPE_*_B`, `WSMP_HEADER_B`, `MESSAGE_FRAME_B`) | `size_model.rs`, `etsi_size.rs` | Validated size model (`messages.codec_tier: size-model`). |
| `Q_*` unit quanta, `FINEST_QUANTUM` | `units.rs`, `lib.rs` | J2735 LSBs and determinism quanta. |
| `PathHistoryParams::J2945_1`, `PathPredictionParams::J2945_1`, `LIGHTS_CHANGED_HOLD_NS`, `G_MPS2` | `j2945.rs` | SAE J2945/1 §6.3 Part II construction rules. They move a BSM's size by a few octets. |
| `NS3_TX_MAX_DELAY`, `GenerationTiming::SYNCHRONISED` | `generator.rs` | Comparison presets. Jitter is `generator/timing-phase-jitter`. |
| `PLACEHOLDER_FILL` | `codec.rs` | Padding byte. |

### v2xw-sec

| Parameter | Where | Kept because |
|---|---|---|
| Primitive sizes and ids (51 constants) | `primitive.rs` | Algorithm definitions. The choice is `security.signature` and `security.kem`. |
| Scheme definitions (ECDSA, ML-DSA, Falcon, SLH-DSA, hybrids, KEMs) | `scheme/*` | Chosen by `security.signature` and `security.kem`. |
| `BASELINE_*_OCTETS`, `ECIES_EPHEMERAL_OCTETS`, `WRAPPED_KEY_AND_TAG_OCTETS` | `scheme/mod.rs` | Size baselines. |
| `PROTOCOL_VERSION`, `CERT_VERSION`, `COMPRESSED_KEY_BYTES`, `PUBLIC_MATERIAL_BYTES` | `envelope.rs`, `cert.rs`, `crypto.rs` | IEEE 1609.2 formats. |
| `SignerIdPolicy` presets (digest only, always, SAE 450 ms, ETSI 1 s) | `envelope.rs` | Exposed as `security.signer_id_policy`. |
| `EXPANDED_PERIODS` | `envelope.rs` | Implementation detail. |
| Linkage sizes, `DEFAULT_JMAX` 20, `DEFAULT_MAX_FORWARD_PERIODS` | `linkage.rs` | SCMS linkage-value format (IEEE 1609.2.1). It is backend-side and does not change on-air metrics. |
| AES and butterfly sizes, curve order | `aes128.rs`, `butterfly.rs`, `ec.rs` | Cryptographic definitions. |

### v2xw-node

| Parameter | Where | Kept because |
|---|---|---|
| Rx, app, Tx and CRL queue depths (64, 64, 32, 16) | `runtime.rs` `NodeConfig::queue_capacity` | They do not saturate before the verification queue (exposed) in any shipped scenario. Candidates if a study shows otherwise. |
| **Peer certificate cache size 128** | `runtime.rs` `peer_cache_capacity`, `stores.rs` `PeerCertCache` | **Finding.** Eviction removes the digest from the LRU list but not the certificate from the verification store (`note_insert` never touches `inner`). The size changes only storage and P2PCD counters, not what is verifiable. Exposing it would promise an effect the model lacks, so fix the eviction first. |
| **`prioritized` horizon 300 m** | `wiring.rs` `Prioritized::new(300.0)` | **Finding.** The policy computes a priority, but the runtime's verification queue is FIFO and ignores it (`runtime.rs` `VerifyDecision::Verify { .. }`). The setting changed no output even with a saturated queue. |
| **`on-demand` relevance threshold 0.5** | `wiring.rs` `OnDemand::new(0.5)` | **Finding.** Nothing calls `ObuRuntime::set_relevance`, so every message has no relevance. On-demand therefore verifies only unknown signers, whatever the threshold (thresholds 0 and 1 gave identical runs). |
| `neighbor_capacity` 256, `telemetry_period` 1 s | `runtime.rs` | Table bound, and the telemetry cadence (aligned with `time.metric_period_s`). |
| `DEFAULT_T_NEIGHBOR` 3 s, `PLAUSIBLE_PERIOD_SKEW`, `REPORT_MAX_AGE` | `stores.rs` | Neighbour expiry and misbehaviour-report housekeeping. |
| `SRM_RANGE_M` 300, `SRM_INTERVAL` 1 s, `SSM_MIN_INTERVAL` 100 ms, `MAP_MEMORY` 5 s | `events.rs` | Signal-priority request rules (J2735 / CTI 4501). These are application layer, and a candidate for an apps audit. |
| `ESS_ON_MPS2` 6, `ESS_OFF_MPS2` 2.5, `DENM_REPETITION` 100 ms, `DENM_VALIDITY` 2 s, stationary-vehicle timers | `events.rs` | C2C-CC triggering conditions for EEBL and stationary-vehicle DENMs. These are application triggers, and a candidate for an apps audit. |
| `T_GEN_CPM_MAX` 1 s, inclusion thresholds (4 m, 0.5 m/s, 4°), `OBJECT_REFRESH`, `VRU_REFRESH`, `SENSOR_INFORMATION_EVERY`, `MAX_OBJECTS` | `cpm.rs` | TS 103 324 fixed inclusion rules. `T_GenCpm` is exposed. |
| `T_ASSEMBLE_VAM` 50 ms, `VAM_FRAME_BYTES`, `DEFAULT_PA_EFFICIENCY` 0.35 | `vru.rs` | VAM assembly latency (TS 103 300-3) and a device-energy constant. |
| `BRAKE_SWITCH_DECEL_MPS2`, `TURN_SIGNAL_LEAD_M`, `HEADLIGHT_VISIBILITY_M` | `vehicle.rs` | Vehicle-bus emulation for BSM Part II flags. |
| `TTC_CONFLICT_THRESHOLD_S` 1.5, `EEBL_DECEL_THRESHOLD_MPS2` 3.92 | `safety.rs` | Safety-application thresholds. These are application layer. |
| `CLEAR_HOLD`, `UPDATE_EVERY`, `FUSION_GATE_M`, `LANE_MATCH_SLACK_M` | `apps.rs` | Application internals. |
| `DEFAULT_DEGRADED_MULTIPLIER` 10, `FORWARD_MAX_AGE` | `rsu.rs` | RSU degraded-mode and forwarding housekeeping. |
| `SOFTWARE_ECDSA_P256_A53_US` | `profile.rs` | Profile figure. The choice is `nodes.default_obu`. |
| `PSID_SAFETY`, `SIGN_PRIMITIVE`, `SUB_CAUSE_*`, `*_MESSAGE_ID` | `secure.rs`, `events.rs` | Identifiers. |
| `SURROGATE_Q`, `MAX_SAMPLES`, telemetry `MAX`, `UNLIMITED_COST` | `safety.rs`, `queue.rs`, `telemetry.rs`, `server.rs` | Quanta and buffer bounds. |
