# Radio, network and message models

Code: `crates/v2xw-radio` (geometry to decibels to frame outcomes), `crates/v2xw-net`
(headers, routing, fragmentation, byte accounting), `crates/v2xw-msg` (what is sent and
what it weighs). Every cited constant carries its clause in the code and in its model
card. Validation of each layer against its reference, with fault injection:
`crates/v2xw-netval` ([../testing.md](../testing.md)).

Radio access technology is chosen by `radio.rat`: `dsrc-80211p` (IEEE 802.11p / ITS-G5),
`lte-v2x-pc5`, `nr-v2x-pc5`, or `hybrid`. Each family has abstract, medium and high
tiers (`radio.tiers`), and specific models can be named in `radio.models`.

## Link budget

`budget.rs` composes three terms per link: a deterministic distance law, obstacle loss,
and stochastic shadowing and fading. Carrier 5.9 GHz unless a 3GPP model sets its own.

### Path loss (`prop.rs`, `winner.rs`)

| Model id | Model | Source |
|---|---|---|
| `propagation/free-space` | Friis | |
| `propagation/two-ray-ground` | two-ray ground-reflection asymptote | ns-3 `TwoRayGroundPropagationLossModel` |
| `propagation/two-ray-interference` | phase-exact ground reflection with the Fresnel coefficient of the road | Sommer, Joerer & Dressler, IEEE VNC 2012, Eq. 4-6; Rappaport §4.6 |
| `propagation/log-distance-shadowing` | dual-slope log-distance with spatially correlated log-normal shadowing; LOS and OLOS rows | Abbas et al. 2015; Kunisch & Pamp and Cheng rows via Karedal 2011 |
| `propagation/v2v-urban-geometric` | NLOS at intersections | Mangel, Klemp & Hartenstein, EURASIP JWCN 2011:182 |
| `propagation/winner-plus-b1` | WINNER+ B1 LOS with effective antenna heights, as adopted for V2V | 3GPP TR 36.885 |
| `propagation/tr37885` | the TR 37.885 V2X channel states (LOS, NLOSv, NLOS) | 3GPP TR 37.885 §6.2, Tables 6.2-1, 6.2.1-1 |

Vehicle antenna patterns: `antenna/vehicle/tr37885-option1` (TR 37.885 V15.3.0 §6.1.4).

Received power and the link's decision quantity:

$$
P_r = P_t + G_t + G_r - PL(d) - L_{\text{obstacle}} - X_\sigma + 10\log_{10} g_{\text{fading}},\qquad
\text{SINR} = \frac{P_r}{N + \sum_j I_j},
$$

with interference $I_j$ from concurrent frames and jammers summed in node-id order.

| Model | Equation (from the card) |
|---|---|
| two-ray ground (asymptote) | crossover $d_c = 4\pi h_t h_r/\lambda$; above it $L = 10\log_{10}\!\big(d^4 L_{\text{sys}}/(h_t^2 h_r^2)\big)$ |
| two-ray interference | $d_{\text{los}} = \sqrt{d^2 + (h_t - h_r)^2}$, $d_{\text{ref}} = \sqrt{d^2 + (h_t + h_r)^2}$, $\varphi = 2\pi(d_{\text{los}} - d_{\text{ref}})/\lambda$, $\Gamma_\perp = \dfrac{\sin\theta - \sqrt{\varepsilon_r - \cos^2\theta}}{\sin\theta + \sqrt{\varepsilon_r - \cos^2\theta}}$ |
| dual-slope log-distance | $PL = PL_0 + 10 n_1 \log_{10}(d/d_0)$ for $d \le d_b$, $PL(d_b) + 10 n_2 \log_{10}(d/d_b)$ above, $d_0 = 10$ m; Abbas 2015 urban LOS: $n_1$ 1.81, $n_2$ 2.85, $PL_0$ 63.9 dB, σ 4.15 dB, $d_b$ 104 m |
| TR 37.885 LOS / NLOSv | urban $38.77 + 16.7\log_{10} d_{3D} + 18.2\log_{10} f_c$; highway $32.4 + 20\log_{10} d_{3D} + 20\log_{10} f_c$; urban $P(\text{LOS}) = \min\{1, 1.05\,e^{-0.0114 d}\}$ |
| buildings (Sommer 2011) | $PL_{\text{LOS}} + 9\,n_{\text{walls}} + 0.4\,d_{\text{in}}$ dB |
| knife-edge (ITU-R P.526) | $J(\nu) = 6.9 + 20\log_{10}\!\big(\sqrt{(\nu - 0.1)^2 + 1} + \nu - 0.1\big)$ dB for $\nu > -0.78$ |
| rain (ITU-R P.838-3) | $A = k(f)\,R^{\alpha(f)}\,d_{\text{km}}$ dB |

### Obstacles (`obstacle.rs`, `obstacle/`, `foliage.rs`, `terrain.rs`)

| Model id | Model | Source |
|---|---|---|
| `obstacle/building/sommer-2011` | per-wall and per-metre loss through building footprints (9 dB per wall, 0.4 dB/m) | Sommer, Eckhoff, German & Dressler 2011 |
| `obstacle/vehicle/tr37885-nlosv` | blockage by vehicles | TR 37.885 NLOSv; Boban vehicle heights |
| `obstacle/reflection/gemv2-single-bounce` | single-bounce building reflection | Boban et al., GEMV² |
| `obstacle/terrain/knife-edge-p526` | multiple knife-edge diffraction over the DEM profile | ITU-R P.526-14 |
| `obstacle/foliage/p833` | excess loss through vegetation | ITU-R P.833-10 §3.1 |

Building material properties follow ITU-R P.2040-3. Rain attenuation follows ITU-R
P.838-3.

### Fading (`fading.rs`)

`fading/nakagami-m` samples one power gain per directed link per frame from a stream
keyed by that link and frame, so a repeated query gives the same sample; distance-
dependent m profiles are cited on the card. `fading/none` disables it.

## 802.11p / ITS-G5

| Model id | What | Source |
|---|---|---|
| `phy/80211p/ofdm-10mhz` | air time from the OFDM symbol structure, noise floor (-104 dBm in 10 MHz plus noise figure), EN 302 663 sensitivity table, CCA thresholds, SINR per overlap window, capture | IEEE 802.11-2020 §17; ETSI EN 302 663; EN 302 571 |
| `phy/80211p/nist-per` | NIST OFDM packet-error model | Pei & Henderson 2010 |
| `mac/80211p/edca-ocb` | EDCA in OCB mode: four access categories, AIFS and contention windows, backoff that freezes on a busy medium; broadcast frames are never retransmitted and the window never doubles | IEEE 802.11; EN 302 663 Annex C |
| `mac/80211p/slotted-abstraction` | slotted medium-tier abstraction | |
| alternating channel access (`net.channel_access: alternating`, `wave_schedule.rs`) | the EDCA MAC gated on the CCH/SCH schedule | IEEE 1609.4-2016 §6.2.5 |

Channel busy ratio is measured by the MAC (`radio/mac/cbr-window`). Regional channel
plans, power limits, spectrum masks and adjacent-channel leakage are in `regulation.rs`.

## C-V2X sidelink

| Model id | What | Source |
|---|---|---|
| `mac/lte-v2x/sps-sensing`, `mac/nr-v2x/sps-sensing` | sensing-based semi-persistent scheduling: 1,000-subframe sensing window, RSRP exclusion, reselection | 3GPP Mode 4 (LTE) and Mode 2 (NR) procedures |
| `phy/lte-v2x/mode4`, `phy/nr-v2x/mode2` | per-sub-channel SINR, in-band emission, SCI decoding, half-duplex | TS 36.101 in-band emission |
| `phy/lte-v2x/bler-lut-r1-160284` | LTE-V2X BLER curves | 3GPP R1-160284 |
| `phy/cv2x/bler-lut-wilab` | SINR at 10 % PER per MCS | WiLabV2Xsim |
| `phy/cv2x/bler-se-gap-fit` | spectral-efficiency fit for MCS no table prints | |
| `phy/nr-v2x/bler-lut-lusvarghi-2024` | NR-V2X link-level lookups | Lusvarghi et al. 2024 |

Numerologies, sub-channels and SCI structure are in `sidelink.rs`.

## Congestion control (`dcc.rs`)

| Model id | What | Source |
|---|---|---|
| `dcc/etsi/reactive-ts102687` | reactive state machine | ETSI TS 102 687 |
| `dcc/etsi/adaptive-ts102687` | adaptive duty-cycle control | ETSI TS 102 687; TS 103 175 |
| EN 302 571 floor | idle-time bound enforced under any algorithm | EN 302 571 §4.2.10.2; TS 103 175 §7.2 Eq. 1 |
| `dcc/sae/j2945-1-rate-power` (+ `/tracking`, `/inference`) | BSM rate and power control with tracking-error and channel-quality inference | Ahmad et al. (CAMP), IEEE TVT 2019, Table I, Eq. 5-9; FMVSS 150 NPRM 82 FR 3854 |
| `dcc/sae/j3161-1-rate` | LTE-V2X rate control presets | SAE J3161/1 |

Adaptive DCC (TS 102 687), every 200 ms:

$$
\text{CBR} \leftarrow \tfrac12\text{CBR} + \tfrac12\cdot\tfrac{\text{CBR}_{L0} + \text{CBR}_{L0,\text{prev}}}{2},\quad
\delta \leftarrow (1-\alpha)\,\delta + \operatorname{clip}\big(\beta(\text{CBR}_{\text{target}} - \text{CBR})\big),\quad
t_{\text{go}} = t_{pg} + \min\!\big(\max(T_{\text{on}}/\delta,\ 25\,\text{ms}),\ 1\,\text{s}\big),
$$

with $\alpha$ = 0.016 and $\delta$ clamped to $[\delta_{\min}, \delta_{\max}]$. SAE J2945/1 rate
control smooths the vehicle density within 100 m, $N_s(t) = \lambda N(t) + (1-\lambda)N_s(t - 100\,\text{ms})$,
and sets $\text{Max\_ITT} = 100$ ms for $N_s \le B$, $100\,N_s/B$ ms above, capped at
`vMaxITT`; its power control moves the radiated power linearly from `vRPMax` to `vRPMin`
between `vMinCU` and `vMaxCU` of the smoothed channel busy percentage (6.3.8.1, Eq. 2).

## Cellular and backhaul

| Model id | What |
|---|---|
| `cellular/uu/fixed-latency` | one-way latency per direction from measured means |
| `cellular/uu/cell-capacity-mm1` | per-cell capacity (cells by inter-site distance), an M/M/1 queue per node, the measured transport-network chain |
| `cellular/uu/handover-outage` | handover interruption per TS 36.133, measured loss percentiles, coverage outage, store-and-forward |
| `cellular/uu/packet-pf` | packet-level LTE/NR: proportional-fair scheduler per cell and direction, HARQ, RLC AM, handover with forwarding, core-network leg |
| `backhaul/{fixed, fibre, dsl, microwave, cellular, none}` | RSU backhaul |
| `transport/tcp-cubic-tls` | TCP slow start, CUBIC, retransmission, TLS handshake and HTTP framing over any packet path |

`hybrid.rs` composes a direct radio and a cellular one with a per-message policy.
Jammer emissions enter the interference sums as transmitters (`jamming.rs`;
behaviour in [threats.md](threats.md)).

## Network layer (`crates/v2xw-net`)

The network layer counts octets exactly but does not synthesise header bytes that no
code reads: a `NetPdu` carries the SDU's real bytes and the header's exact size and
decoded fields.

| Model id | What | Source |
|---|---|---|
| `net/wsmp/1609-3` | WSMP header (5 octets for a BSM) | IEEE 1609.3 |
| `net/gn-btp/en302636` | GeoNetworking + BTP: 52 octets below a secured CAM (LLC/SNAP 8, GN SHB 40, BTP-B 4); `itsGnMaxSduSize` 1,398 | EN 302 636-4-1 V1.4.1 Annex H; EN 302 636-5-1 |
| `net/gn-stack/dissemination` (`stack/`) | GeoAdhoc router: SHB, TSB, GBC, GUC, beacons, location table, duplicate detection, greedy and contention-based forwarding (`etsi`, `dpd`, `gpc` variants), geographic areas | EN 302 636-4-1 §8-10, Annexes D-F; EN 302 931 §5 |
| CA and DEN basic services | CAM generation rules; DENM trigger, update, repetition, keep-alive | EN 302 637-2 V1.4.1 §6.1.3; EN 302 637-3 V1.3.1 §6, §8 |
| WSA and channel switching | service advertisement, 1609.4 schedule | IEEE 1609.3-2020 §8; IEEE 1609.4-2016 §6.2.5 |

### Fragmentation (`frag/`)

Neither WSMP nor GeoNetworking can fragment, so an SDU over the MTU (a post-quantum
certificate, for example) has three fates, chosen by `net.fragmenter`:

| Model id | What |
|---|---|
| `fragmenter/none` | refused |
| `fragmenter/facilities-segmentation` | segmented at the facilities layer into independently usable pieces |
| `fragmenter/generic-sdu` | split into pieces useless alone; loss amplifies as `P_sdu = 1 - prod(1 - p_i)` |
| `fragmenter/cert-cycle-partial-hybrid` | the partially hybrid certificate cycle (NDSS 2024) |

Byte accounting (`accounting.rs`) attributes every octet on the air, the Uu link, the
backhaul and the backend to a layer, and the sum is an invariant; `frame.rs` breaks one
PSDU down by payload, envelope, network header, LLC/SNAP, MAC header and FCS.

## Messages

What is sent, when, and how real each message's bytes are (generated ASN.1,
oracle-validated hand-written codecs, or a size model), with the security envelope's
measured overhead: [Messages](messages.md).

## Modelled versus simplified

| Modelled | Simplified or absent |
|---|---|
| per-link deterministic path loss, obstacle loss, correlated shadowing and per-frame fading, keyed so a repeated query gives the same sample | no ray tracing beyond single-bounce reflection; materials by ITU-R P.2040-3 classes |
| SINR per overlap window with capture, in-band emission and half-duplex on the sidelink | link-level behaviour is from lookup tables (NIST, R1-160284, WiLabV2Xsim, Lusvarghi 2024), not a waveform simulation |
| EDCA in OCB mode, IEEE 1609.4 channel switching, SPS sensing and reselection | the slotted MAC abstraction at the medium tier |
| ETSI and SAE congestion control, with the EN 302 571 floor | |
| cellular latency, capacity, handover and a packet-level PF scheduler | the fixed-latency and M/M/1 tiers are means from measurements, not protocol stacks |
| exact header sizes, GeoNetworking routing and forwarding, fragmentation fates | header bytes that no code reads are counted, not synthesised |

Every card's parameters, equations and limitations: [Model parameters](../configuration/reference/models.md);
the radio models a run composes are registered by that run and pinned in its manifest.
