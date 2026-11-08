# Messages

What a node sends, when it sends it, and how many bytes it weighs. The message layer is
built so that every reported size says how real it is: generated from a published ASN.1
module, hand-written and checked against an independent decoder, or a length from a
size model.

Code: `crates/v2xw-msg` (codecs, generators, size models, evidence table),
`crates/v2xw-node/src/{generate,events,denm_triggers,cpm,bsm_timing,vru,rsu}.rs` (which
node sends what), `crates/v2xw-sec` (the security envelope). Settings:
[`messages`](../configuration/reference/messages.md),
[`actors.vehicles.bsm_timing`](../configuration/reference/actors.md). The radio and network
layers that carry these bytes are on [Radio and network](network.md).

## Message sets

`messages.sets` (default `[bsm]`) chooses which services the nodes run. The loader
refuses a set no node could send.

| Set | Standard | Sent by | Requires |
|---|---|---|---|
| `bsm` | SAE J2735 BSM, SAE J2945/1 rules | equipped vehicles | |
| `cam` | ETSI EN 302 637-2, TS 103 900 | equipped vehicles | |
| `denm` | ETSI EN 302 637-3, TS 103 831 | equipped vehicles, on events | `net.layer: gn-btp` |
| `spat`, `map` | SAE J2735 SPaT and MAP (SPATEM/MAPEM on the ETSI stack) | roadside units with the `spat` / `map` role | an RSU with that role |
| `srm` | J2735 signal request | emergency-class vehicles | `map` in the sets and an `emergency` class share |
| `ssm` | J2735 signal status | RSUs with the `spat` role | `srm` and a `spat` RSU |
| `cpm` | ETSI TS 103 324 collective perception | vehicles; RSUs with the `cpm` role | `net.layer: gn-btp` |
| `psm`, `vam` | SAE J2735 PSM (J2945/9), ETSI TS 103 300-3 VAM | VRU devices | refused as sets: set `actors.vru.device_fraction` > 0 |

A VRU device sends PSMs when the sets include `bsm` (or `psm`), VAMs when they include
`cam` (or `vam`), both when both stacks are present, and PSMs when neither is
(`v2xw_engine::hosted::vru_services`).

## How real the bytes are

Every encoded message carries a `size_source` (`uper`, `coer` or `size-model`), and
`v2xw_msg::evidence::EVIDENCE` states, per message type, what has checked it. A size is
byte-exact only when the evidence level says so.

| Level | Meaning | Byte-exact |
|---|---|---|
| `oracle-validated` | real wire bytes; an independent ASN.1 implementation agreed byte for byte on recorded vectors | yes |
| `generated-from-module` | real wire bytes from `rasn` bindings generated at build time from the committed ASN.1 module; no third-party vectors checked | yes |
| `real-uper-unvalidated` | real bytes of a documented subset, checked only by this crate's own tests | no |
| `size-modelled` | a fill pattern of exactly the modelled length | no |
| `not-encoded` | no codec claims the type | |

| Message | On the air | Evidence | Encoder |
|---|---|---|---|
| BSM | UPER | oracle-validated (pycrate compiled from the J2735 modules) | hand-written, `v2xw_msg::j2735::bsm` |
| SPaT, MAP | UPER | oracle-validated (pycrate, public J2735 2016 and 2020 modules) | hand-written, `j2735::spat`, `j2735::map` |
| PSM | UPER | the VRU device's bytes are hand-written UPER checked against pycrate; the generic codec row is a size model | `j2735::psm` |
| CAM, DENM | UPER | generated from module (TS 103 900, TS 103 831 over TS 102 894-2 Release 2) | `rasn`, `v2xw_msg::cam`, `denm` |
| VAM | UPER | generated from TS 103 300-3 V2.2.1; decoded and re-encoded identically by asn1tools | `v2xw_msg::vam` |
| CPM | UPER | generated from TS 103 324 V2.1.1; decoded and re-encoded identically by asn1tools | `v2xw_msg::cpm` |
| WSA | UPER | oracle-validated against asn1tools on the IEEE 1609.3-2016 modules | hand-written, `v2xw_msg::wsa` |
| SRM, SSM | size model | derived from the ASN.1 structure, `todo-calibrate` | `size_model` |
| CRL, P2PCD | COER | encoded by `v2xw-sec` over the generated IEEE 1609.2 bindings | [Security](security.md) |
| ETSI misbehaviour report | not encoded through a codec | | [Threats](threats.md) |

!!! note "Why SAE J2735 is hand-written"
    The SAE J2735 ASN.1 is licensed and not redistributable, so it is not in the
    repository (it is git-ignored under `third_party/asn1/j2735/`). `rasn-compiler`
    cannot compile it either: the `RegionalExtension` information-object-class idiom
    produces compile errors. The BSM, SPaT, MAP and PSM codecs are therefore written by
    hand against the bit-level UPER engine in `j2735::uper`, and checked against an
    independent decoder when a local copy of the modules is present. The ETSI modules
    are BSD-3-Clause and are committed under `third_party/asn1/etsi/` with their
    provenance.

`messages.codec_tier: size-model` replaces encoding with the size model for very large
runs; the default is `uper`.

## Encoders

### SAE J2735 BSM

`BasicSafetyMessage` = `coreData` (14 mandatory fields, 290 bits, every message) plus an
optional `partII` with `VehicleSafetyExtensions` (container 0). A Part I-only BSM is
37 octets, 40 inside a `MessageFrame`. Every field holds the wire integer (latitude in
0.1 µdeg, speed in 0.02 m/s, acceleration in 0.01 m/s²) and uses the type's
`unavailable` sentinel for a missing quantity.

| Element | Status |
|---|---|
| `BSMcoreData`, every field | encoded and decoded |
| `VehicleSafetyExtensions`: `events`, `pathHistory.crumbData`, `pathPrediction`, `lights`, `currGNSSstatus` | encoded and decoded |
| `PathHistory.initialPosition` | refused |
| Part II containers 1 (special vehicle) and 2 (supplemental) | carried verbatim |
| `regional`, any extension addition | refused |

### ETSI CAM, DENM, VAM, CPM

| Message | Containers filled |
|---|---|
| CAM | basic and high-frequency every time; low-frequency on the first CAM and then every ≥ 500 ms (EN 302 637-2 §6.1.3). A typical unsecured CAM is about 60 octets of UPER |
| DENM | management, situation and location for a new or updated event; neither situation nor location for a termination (the `WITH COMPONENTS` constraint `rasn` does not carry is enforced in `denm.rs`) |
| VAM | header (`messageId vam(16)`), basic container (`stationType` pedestrian 1 or cyclist 2), VRU high-frequency, VRU low-frequency on its cadence; no cluster or motion-prediction container |
| CPM | management (`referenceTime`, `referencePosition`), originating vehicle (or RSU), sensor information, perceived objects in an ENU frame centred on the reference position (TS 103 324 §7.1.8.1) |

Every input is the sender's belief (its GNSS estimate), never ground truth.

### WSA

`v2xw_msg::wsa` encodes the IEEE 1609.3-2016 WAVE Service Advertisement (protocol version
3) as UPER. A roadside unit with the WSA role advertises the SCMS service: WAVE security
management (PSID `0x23`, the IEEE 1609.2.1 `SecurityMgmtPsid`; that a US RSU uses exactly
this PSID is an assumption the code states) and IPv6 Routing, with a WAVE Routing
Advertisement: 84 octets. The default period is 10 Hz (USDOT RSU Specification v4.1
App. D).

## Generation rules

### BSM: SAE J2945/1

`generator/bsm-j2945-1`. Part I in every message, 10 Hz nominal.

| Parameter | Default | Source |
|---|---|---|
| `nominal_itt_ms` | 100 ms | SAE J2945/1, 10 Hz |
| `min_itt_ms` (`vMinITT`) | 100 ms | `todo-calibrate` |
| `max_itt_ms` (`vMaxITT`) | 600 ms | `todo-calibrate` |
| `vTxRand` | 5 ms | J2945/1 (2020) 6.3.8.8, Table 21 |
| `vRescheduleTh` | 25 ms | J2945/1 6.3.8.5 |

On a vehicle the engine runs J2945/1's own schedule (6.3.8.5 to 6.3.8.8) on the unit's
clock:

$$
t_{\text{next}} = t_{\text{last tx}} + \text{Max\_ITT} + \text{RandOffset},\qquad \text{RandOffset} \sim U(-v_{\text{TxRand}}, +v_{\text{TxRand}}),
$$

with `RandOffset` drawn from the key `(plugin(generator/bsm-j2945-1), LinkFrame((n, n), LastTxTime))`.
Every `vTxRateCntrlInt` (100 ms) and at the onset of a critical event, the transmission
decision may move the scheduled BSM. `Max_ITT` comes from the congestion control
(`dcc/sae/j2945-1-rate-power`, [network](network.md#congestion-control-dccrs)) and is held
inside `[min_itt, max_itt]` and never below the nominal interval.

### CAM: EN 302 637-2

`generator/cam-en302637-2`, the §6.1.3 state machine, checked every `T_CheckCamGen`:

| Parameter | Default | Clause |
|---|---|---|
| `t_gen_cam_min_ms` | 100 ms | §6.1.3 |
| `t_gen_cam_max_ms` | 1000 ms | §6.1.3 |
| `t_check_cam_gen_ms` | 100 ms | §6.1.3 |
| `n_gen_cam` | 3 | §6.1.3 |
| `heading_threshold_deg` | 4° | §6.1.3 |
| `position_threshold_m` | 4 m | §6.1.3 |
| `speed_threshold_mps` | 0.5 m/s | §6.1.3 |
| `low_frequency_interval_ms` | 500 ms | §6.1.4 |

Trigger 1 (dynamics) fires when
$|\Delta\psi| > 4^\circ \lor |\Delta \mathbf{p}| > 4\,\text{m} \lor |\Delta v| > 0.5\,\text{m/s}$
since the last CAM, and sets
$T_{\text{GenCam}} := \operatorname{clamp}(\text{elapsed}, T_{\text{GenCamMin}}, T_{\text{GenCamMax}})$;
trigger 2 is the periodic deadline, and after `N_GenCam` consecutive trigger-2 CAMs
$T_{\text{GenCam}} := T_{\text{GenCamMax}}$. `T_GenCam_Dcc` arrives as the DCC's `T_off`
and acts as a floor (C2C-CC RS 2037 RS_BSP_293). The thresholds are evaluated on the GNSS
belief, so a noisy fix raises the CAM rate as it does on a road.

`messages.rate_hz` replaces both rules with a fixed awareness rate (the ns-3
`BsmApplication` and Veins `beaconInterval` convention); congestion control may still
stretch it.

### Generation timing

Each node's generation clock is offset from the engine's step grid by
$\varphi \sim U[0, T)$, drawn once per node from `(plugin(generator/timing-phase-jitter), Node(id))`,
where $T$ is the generation period. Without this, every node would sign and contend in
the same few hundred microseconds of each 100 ms. A per-message hand-off jitter
$j \sim U[0, j_{\max}]$ follows signing; its default is ns-3 `BsmApplication`'s
`TxMaxDelay` (10 ms), a simulator convention rather than a standard's number. A J2945/1
unit on 802.11p uses `RandOffset` instead and has no hand-off jitter; on a PC5 sidelink
there is none either, because SPS schedules rather than contends.

```yaml
messages:
  generator:
    id: generator/timing-phase-jitter
    params: { phase_window_ms: 0, max_jitter_ms: 0 }   # every node on one grid
```

### BSM Part II: path history and prediction (J2945/1)

`v2xw_msg::j2945`, from the node's own GNSS fixes:

| Rule | Value | Source |
|---|---|---|
| path history covers at least | 200 m (`vMinPHistDistance`) | J2945/1 §6.3.6.16, Table 21 |
| most points per message | 15 (`vMaxPHistPoints`; J2735 allows 23) | Table 21; `messages.path_history_points` |
| chord perpendicular error below | 1 m (`vPathPerpendicularDist`) | Table 21 |
| longest chord | 210 m (`vMaxChordLength`) | Table 21 |
| start-up history | one point at the newest fix | §6.3.1, §6.3.7 |
| path prediction | $R = v/\dot\psi$, straight beyond 2,500 m | radius cap recalled, a parameter |
| hard-braking flag | > 0.4 g; > 0.2 g for buses and Class 7 to 13 trucks | J2735 event flags |
| lights-changed flag | held 2 s after a change | J2735 |
| on a pseudonym change | `keep` (default for BSM nodes) or `clear` | `messages.bsm.path_history_on_change` |

### BSM timestamp model

`actors.vehicles.bsm_timing` (off by default) gives each OBU a timing class drawn once
per device: whether its `secMark` sits on the GNSS fix epoch ("locked") or not ("free"),
epoch slips between the reported position and the timestamp, frozen repeats, the jitter of
the 1609.2 `generationTime` against `secMark`, and (schema v2 and v3) a free-running BSM
clock, output noise and a position-error random walk. Classes and shares come from a
calibration file (`data/calibration/bsm-timing-cvpilot.yaml` when present) fitted to
public CV-pilot BSMs; the built-in values are provisional. Only the transmitted BSM is
affected; ground truth and the GNSS model are not.

### Event and infrastructure messages

| Message | Trigger | Source |
|---|---|---|
| DENM `emergencyElectronicBrakeLights` | the emergency stop signal: deceleration ≥ 6 m/s², off below 2.5 m/s²; update every 100 ms while on; validity 2 s | C2C-CC RS 2003 RS_tcDaSi_167; UN R48 / R13-H |
| DENM `stationaryVehicle` / `vehicleBreakdown` | hazard lights on, standing, Triggering Timer (10 s) elapsed; repeated every 1 s for 15 s; validity 30 s | C2C-CC RS 2006 §3.2 |
| DENM sudden speed drop, local slow down | off unless `messages.denm_traffic_condition` | C2C-CC RS 2007 §3.1, §3.2 |
| DENM traction loss (`adhesion`) | friction estimate below 0.3 or 0.2 for 5 s; off unless `messages.denm_traction_loss` | C2C-CC RS 2002 §3.3 |
| SPaT / MAP | every 100 ms / 1 s (`messages.spat_interval_ms`, `map_interval_ms`) | CTI 4501 |
| SRM | a junction whose MAP was heard within 300 m ahead; repeated every 1 s | J2735; range and interval are this build's choice |
| SSM | a received request; at most one per 100 ms | J2735 |
| PSM | 1 Hz, floored at 100 ms | `todo-calibrate` |
| VAM | the CAM machine with `T_GenVamMin` 100 ms, `T_GenVamMax` 5 s, 4 m / 0.5 m/s / 4°, low-frequency every ≥ 2 s | TS 103 300-3 §6.2 |

The DENM lifecycle (EN 302 637-3 V1.3.1) is implemented in `v2xw_msg::denm`: unused
`actionId` per new event (§8.2.1.5), updates keep it and advance `referenceTime`
(§6.1.2), repetition only when the application supplies both interval and duration
(§6.1.4.2), default validity 600 s, termination by cancellation or negation (§8.3.2.5),
and keep-alive forwarding at $2\,T_{\text{tx}} + U(0, 150\,\text{ms})$ (Annex B).
A VRU device's VAM omits the trajectory-interception trigger and redundancy mitigation,
so it transmits at least as often as a conformant device.

### CPM: TS 103 324 V2.1.1

`v2xw_node::cpm`, on the objects the [perception model](applications.md#perception) reports.

| Rule | Value | Clause |
|---|---|---|
| generation check `T_GenCpm` | 100 ms (`messages.cpm_interval_ms`) | §6.1.2.1, Table F.1 |
| a CPM at least every `T_GenCpmMax` | 1000 ms | Table F.1 |
| Type-B object (vehicle) included | first detection, or moved > 4 m, speed change > 0.5 m/s, orientation change ≥ 4°, or last included ≥ 1 s ago | §6.1.2.3 rule 2 |
| Type-A object (pedestrian, cyclist, animal) | first detection; when any has gone 500 ms unincluded, all are included | §6.1.2.3 rule 1 |
| sensor information container | first CPM, then every ≥ 1000 ms | §6.1.2.2 |
| objects per CPM | ≤ 255 (`messages.cpm_max_objects`) | ASN.1 |
| DCC-limited `T_GenCpm` | `T_off` clamped to [100, 1000] ms (`messages.cpm_dcc_limited`) | §6.1.2.1, Annex D |
| quality threshold | objects whose `objectPerceptionQuality` exceeds it (`messages.cpm_quality_threshold`, Table F.1 recommends 3) | §6.1.2.3, §7.1.8.6 |

Object perception quality (§7.1.8.6, Table F.4 weights all 1, EMA factor 0.5):

$$
q = \left\lfloor \frac{w_d r_d + w_c r_c + w_{oa} r_{oa}}{w_d + w_c + w_{oa}} \right\rfloor,\quad r = \lfloor \mathrm{EMA}\cdot 15 \rfloor,\quad r_{oa} = \min(\lfloor oa / 100\,\text{ms}\rfloor, 15).
$$

Under a budget, objects are ordered by the §6.1.3.2 utility
$p_{\text{conf}} + p_{\text{pos}} + p_{\text{speed}} + p_{\text{orient}} + p_{\text{time}}$ and the lowest are
cut. Perception regions, look-ahead inclusion, segmentation by `MTU_CPM` and
multi-channel operation are not modelled.

## Sizes on the air

### The security envelope

The IEEE 1609.2 `SignedData` envelope and its ETSI TS 103 097 profile are encoded for real
(COER) by `v2xw_sec::envelope`, so the overhead is measured, not tabled. With ECDSA P-256
under PSID `0x20`:

| Component | Octets |
|---|---:|
| `Ieee1609Dot2Data` (`protocolVersion`, content choice) | 2 |
| `hashId` | 1 |
| `SignedDataPayload` preamble and inner `Ieee1609Dot2Data` | 1 + 3 to 4 |
| `HeaderInfo` preamble, `psid`, `generationTime` | 1 + 2 + 8 |
| signer = `digest` (`HashedId8`) | 9 |
| signer = `certificate` | 3 + certificate |
| ECDSA P-256 signature (`x-only` `rSig`, `sSig`) | 66 |
| `generationLocation` (DENM, TS 103 097 §7.1.2) | +10 |

Measured (`crates/v2xw-sec/tests/overhead.rs`): **93 octets with a digest signer**, and
**87 octets plus the certificate** with a certificate signer (an implicit certificate is
about 80 octets, an explicit one about 147). A PSID ≥ 256 adds one octet. How often a
full certificate is attached is `security.signer_id_policy`
([Security](security.md)).

### The size model

For the messages no codec encodes (and for `codec_tier: size-model`), `v2xw_msg::size_model`
and `etsi_size` give a base plus a per-element increment per content profile, each row
checked against its published anchors after converting them to payload scope (removing the
93-octet digest envelope, the 87 + 80 certificate envelope and the 5-octet WSMP header).

| Message | Profile | Base | Increment | Basis |
|---|---|---:|---|---|
| PSM | typical | 27 B | 8 B per path-history point | ASN.1 structure; the increment from C2C-CC TR 2052 §3.1 |
| SRM | typical | 20 B | 10 B per request | ASN.1 structure, `todo-calibrate` |
| SSM | typical | 16 B | 12 B per status | ASN.1 structure, `todo-calibrate` |
| SPaT, MAP | (retired) | 17 B, 40 B | per movement / lane | superseded by the real encoders, kept as a cross-check |
| CPM | typical | | per object | ETSI TR 103 562 Table 3 container sizes, bounded by C2C-CC TR 2050 Fig. 13 |
| VAM | typical | | | fitted to four published planning sizes (TR 2050 Fig. 12 and others) |

Below the envelope, the network layer adds its own headers (WSMP 5 octets for a BSM;
GeoNetworking and BTP 52 octets including LLC/SNAP below a secured CAM); see
[Radio and network](network.md#network-layer-cratesv2xw-net).

## Outputs

| Channel | What |
|---|---|
| `node.tx` | every transmission: message type, bytes on air, payload bytes, signer identifier type, pseudonym digest ([VWP §3.6.4](../protocol/vwp-v1.md)) |
| `node.rx`, `node.verify` | receptions and their verification |
| `metric.sample` | message-rate, size and channel metrics ([Metrics](../recording-and-metrics.md)) |

## Modelled versus simplified

| Modelled | Simplified or absent |
|---|---|
| real UPER bytes for BSM, SPaT, MAP, PSM, WSA (hand-written, oracle-checked) and CAM, DENM, VAM, CPM (generated) | SRM and SSM are sized, not encoded |
| real COER security envelope, measured overhead | encrypted SPDUs and countersignatures are not produced |
| J2945/1 BSM schedule with `RandOffset` and congestion-driven `Max_ITT` | `vMinITT` and `vMaxITT` defaults are uncited (`todo-calibrate`) |
| EN 302 637-2 CAM triggers on the GNSS belief and the believed clock | the CAM's ≤ 50 ms generation budget is not a delay; special-vehicle and RSU CAM containers are not filled |
| J2945/1 path history and prediction | `PathHistory.initialPosition` and BSM regional extensions are refused |
| C2C-CC DENM triggering conditions | VAM clustering, redundancy mitigation and trajectory interception are absent |
| TS 103 324 CPM inclusion, quality and utility rules | CPM segmentation, perception regions and multi-channel operation are absent |
