# Radio fidelity: the channel, the receiver and the MAC against Veins, OpenCV2X and the literature

Written 2026-10-07 by the radio-fidelity track. Every number below comes from a run on this
branch: the delivery tables from `cargo test -p v2xw-interop --lib winner -- --ignored
--nocapture` (debug build, highway sweep harness `v2xw_radio::sweep`, about 5 minutes),
the end-to-end checks from `cargo test -p v2xw-engine --test radio_tuning`.

## 1. The 30-point 802.11p gap against Veins was the channel's breakpoint

The cross-validation report had this engine's 802.11p **30 PDR points** above Veins at
300-400 m. All three reference studies ran on WINNER+ B1, which the workspace had only
as a shaped stand-in. `propagation/winner-plus-b1` now ships (`crates/v2xw-radio/src/winner.rs`):

```
d'BP = 4·h'_t·h'_r·f/c
PL = 22.7·log10(d) + 27.0 + 20·log10(f_GHz)                                  d ≤ d'BP
PL = 40·log10(d) + 7.56 − 17.3·log10(h'_t) − 17.3·log10(h'_r) + 2.7·log10(f_GHz)   d > d'BP
σ_SF 3 dB LOS, 4 dB blocked; d floored at 3 m
```

(WINNER+ D5.3 Table 4-1 B1 LOS, adopted for V2V by 3GPP TR 36.885 Table A.1.4-1.)

The stand-in's error was one constant: it computed the breakpoint from the antennas'
**actual** 1.5 m heights (177 m at 5.9 GHz). WINNER and TR 36.885 use **effective**
heights `h' = h − 1 m`, which puts the breakpoint at 19.7 m. The 40 dB/decade slope then
starts nine times nearer, and the 802.11p range falls where Veins puts it.

There is a second finding: **the published simulators do not agree on the convention**.
Veins (Sepulcre et al. 2021) matches the printed effective heights. OpenCV2X (McCarthy et
al. 2021) and Molina-Masegosa 2017 match only the actual heights: with effective heights
their LTE-V2X curves would collapse by 300 m, but they report 0.79-0.89 there. The model
offers both conventions (`radio.models.propagation: {id: propagation/winner-plus-b1,
params: {heights: effective | actual}}`). Effective is the default because it is what the
standard prints.

Mean absolute deviation, in PDR points (lower is better; the digitised references carry
±1-2 points of their own):

| study (reference) | stand-in (before) | TR 37.885 highway LOS | **WINNER+ B1, effective h'** | **WINNER+ B1, actual h** |
|---|---:|---:|---:|---:|
| Veins 802.11p, 60 veh/km (Sepulcre 2021 Fig. 4a) | 30.0 | 29.6 | **5.7** | 30.0 |
| OpenCV2X LTE Mode 4, 60 veh/km (McCarthy 2021 Fig. 3) | 5.3 | 5.9 | 34.5 | **4.1** |
| OpenCV2X LTE Mode 4, 120 veh/km (McCarthy 2021 Fig. 3) | 9.8 | 13.6 | 31.3 | **9.3** |
| Molina-Masegosa 2017 LTE Mode 4, 120 veh/km (Fig. 3) | 6.3 | 11.2 | 26.1 | **6.0** |

The stand-in and TR 37.885 columns are from `docs/validation/cross-validation.md`.

### Veins 802.11p, 60 veh/km, 10 Hz, 190 B, 23 dBm, 6 Mbit/s: WINNER+ B1, effective heights

| distance (m) | Veins (published) | this engine | difference |
|---:|---:|---:|---:|
| 0 | 0.986 | 1.000 | +0.014 |
| 50 | 0.979 | 1.000 | +0.021 |
| 100 | 0.968 | 0.980 | +0.012 |
| 150 | 0.930 | 0.922 | −0.008 |
| 200 | 0.859 | 0.825 | −0.034 |
| 250 | 0.630 | 0.431 | −0.199 |
| 300 | 0.297 | 0.155 | −0.142 |
| 350 | 0.091 | 0.028 | −0.063 |
| 400 | 0.019 | 0.001 | −0.018 |

Before this change the engine read 0.960 / 0.901 / 0.845 / 0.785 at 250-400 m. The
remaining error is a steeper fall at 250-300 m. The likely causes are Veins' FER table
against this engine's NIST bound, and the harness's 3 lanes per direction against Veins' 2.

### OpenCV2X LTE-V2X Mode 4, 20 dBm, MCS 7, 190 B, RSRP −126 dBm: WINNER+ B1, actual heights

| distance (m) | 60 veh/km published | this engine | 120 veh/km published | this engine |
|---:|---:|---:|---:|---:|
| 0 | 0.990 | 0.995 | 0.990 | 0.987 |
| 50 | 0.980 | 0.972 | 0.960 | 0.961 |
| 100 | 0.965 | 0.962 | 0.930 | 0.914 |
| 150 | 0.950 | 0.953 | 0.920 | 0.844 |
| 200 | 0.945 | 0.937 | 0.895 | 0.743 |
| 250 | 0.920 | 0.904 | 0.835 | 0.651 |
| 300 | 0.890 | 0.843 | 0.770 | 0.570 |
| 350 | 0.820 | 0.773 | 0.680 | 0.506 |
| 400 | 0.670 | 0.736 | 0.530 | 0.448 |
| 450 | 0.460 | 0.666 | 0.350 | 0.396 |

The 120 veh/km gap at 150-350 m comes from congestion rather than the channel. The paper
uses 2 sub-channels of 12 RB in 10 MHz; the harness keeps its 50-PRB pool, so collisions
differ. `radio.sidelink` (§3) now exposes the knobs needed to close it in a full
engine run.

### Molina-Masegosa 2017 LTE-V2X Mode 4, 120 veh/km, 10 pps: WINNER+ B1, actual heights

| distance (m) | published | this engine | difference |
|---:|---:|---:|---:|
| 0 | 0.970 | 0.987 | +0.017 |
| 100 | 0.950 | 0.929 | −0.021 |
| 200 | 0.900 | 0.822 | −0.078 |
| 300 | 0.790 | 0.693 | −0.097 |
| 400 | 0.660 | 0.571 | −0.089 |

## 2. What the stack models, against Veins and OpenCV2X

| capability | Veins 5.x | OpenCV2X / SimuLTE | this engine |
|---|---|---|---|
| Building obstruction | SimpleObstacleShadowing: β per wall + γ per metre | none in the sidelink model | Sommer 2011 β/γ (six fitted rows or custom), R-tree index over footprints, cached per world; street-canyon ceiling against TR 37.885 NLOS; corner tracing for the Mangel 2011 law |
| Vehicle blockage | none (VehicleObstacleShadowing is a separate add-on) | none | TR 37.885 NLOSv per antenna-height case and the Boban knife-edge, from the actual bodies on the path; under any law (`vehicle_blockage: true`) |
| Path loss | free space, two-ray, log-normal | WINNER+ B1 | free space, two-ray, dual-slope log-distance (Abbas), TR 37.885 LOS/NLOS/NLOSv state machine, city-street geometric law, WINNER+ B1 (both height conventions) |
| Shadowing | i.i.d. per frame | log-normal | AR(1) spatially correlated per link (Gudmundson; TR 36.885 decorrelation) |
| Fast fading | Nakagami, distance-dependent m | none | Nakagami-m per link per frame, keyed draws; fixed, Yin, or any near/far m profile |
| SINR | cumulative interference | cumulative interference | every concurrent transmitter, including energy-only frames below the attempt range, in time windows across the frame; ordered sums |
| Capture | SINR trace against the error model | — | SINR trace (default) or hard threshold; preamble capture at high tier |
| Hidden terminals | emerge, not labelled | — | emerge, and each loss is labelled `hidden-terminal` when the later transmitter could not hear the earlier frame above CCA (§4) |
| 802.11p EDCA | per AC | — | per AC, EN 302 663 parameter set, backoff freeze (netval ACC-01-04) |
| C-V2X SPS | — | Mode 4: sensing, RSRP exclusion, 20 % rule, reselection counter, keep probability | Mode 4 and Mode 2 on one engine: 1,000-subframe sensing ring, 3 dB step-up to 20/35/50 %, S-RSSI ranking, C_resel by RRI, probResourceKeep, size-triggered reselection, blind HARQ (1-3 transmissions), Rel-16 re-evaluation and pre-emption, CR limits (netval ACC-09-13) |
| Half duplex (C-V2X) | — | yes | yes, plus the half-duplex `q·RRI` exclusion in sensing |

## 3. New settings (published to the settings window and the agent)

These merged with the netconfig track's researcher settings into one surface: the channel
numbers are `radio.channel_model`, the 802.11p receiver numbers `radio.phy`, and the
resource-selection numbers `radio.sidelink` (see `docs/guides/configuration-reference.md`).
Every field is optional. Unset keeps the cited default, and an empty section leaves the
scenario hash unchanged. Each field carries a unit, a bound, a plain title, a one-sentence
explanation and a recommended range (`x-explain`, `x-recommended`). The loader refuses an
override the composed model would ignore, so every accepted value changes the run.

| field | range | default (cited) | acts on |
|---|---|---|---|
| `radio.channel_model.path_loss_exponent` | 1.5-6 | preset slopes (Abbas 2015) | log-distance law (one slope) |
| `radio.channel_model.shadowing_sigma_db` | 0-15 dB | 3/4 dB (TR 36.885/37.885), 3.95 (Abbas) | every law with shadowing |
| `radio.channel_model.shadowing_decorrelation_m` | (0, 500] m | 10 urban / 25 freeway (TR 36.885 A.1.4) | same |
| `radio.channel_model.fading: nakagami` with `nakagami_shape`, `nakagami_shape_far`, `nakagami_breakpoint_m` | 0.5-20, (0, 5000] m | m = 3 (Torrent-Moreno 2009) | small-scale fading |
| `radio.channel_model.building_wall_loss_db`, `building_loss_db_per_m` | 0-40 dB, 0-10 dB/m | 9 dB, 0.4 dB/m (Sommer 2011) | Sommer term, geometric law's through-building path |
| `radio.channel_model.vehicle_blockage` | on/off | on with the geometric law | TR 37.885 NLOSv, any law |
| `radio.phy.noise_figure_db` | 0-20 dB | 6 dB (EN 302 663) | 802.11p |
| `radio.phy.cca_threshold_dbm` | −100 to −50 dBm | −85 dBm (ETSI) | 802.11p CCA and CBR |
| `radio.phy.capture_threshold_db` | 0-30 dB | unset: SINR trace | 802.11p |
| `radio.sidelink.sensing` | on/off | on | SPS |
| `radio.sidelink.rsrp_threshold_dbm` | −128 to 0 dBm | profile's (−110 / −128) | SPS exclusion |
| `radio.sidelink.candidate_percent` | 20 (LTE); 20/35/50 (NR) | 20 | SPS step 4 |
| `radio.sidelink.resource_keep_probability` | 0, 0.2 … 0.8 | profile's (0 / 0.8) | reselection |
| `radio.sidelink.selection_window_ms` | 20-100 (LTE), 1-100 (NR) | profile's T2 | selection window |

End-to-end evidence (`crates/v2xw-engine/tests/radio_tuning.rs`, phase-1 grid):

| check | measured |
|---|---|
| WINNER+ B1 against TR 37.885 at 150-300 m, open air | mean RSSI −95.7 against −67.2 dBm |
| `shadowing_sigma_db: 0` against default, WINNER+ B1 | spread about the law 0.45 dB against 2.45 dB |
| refusals | exponent under TR 37.885; sidelink knobs on 802.11p; keep 0.3; Nakagami shape without `fading: nakagami` |
| hidden terminals, 250 vehicles, 4 s, buildings on | 156 hidden-terminal losses, 0 plain collisions |

## 4. Hidden and exposed terminals (802.11p)

A collision is now classified when interference is declared. Of two overlapping frames,
the later one was sent by a node whose carrier sense should have held it. If the earlier
frame reached that node below the CCA threshold, or not at all, the loss is
`hidden-terminal`, which CSMA cannot prevent. Otherwise it is `collision`: equal backoff
expiry, or the sensing gap of a node that started within a slot. Frames that start in the
same nanosecond stay `collision`. In the grid run above, every interference loss among
buildings was a hidden terminal. That is the expected signature: CSMA works on the street
and fails round the corner. `radio.phy.cca_threshold_dbm` moves the boundary between the two.

The exposed-node effect is not labelled. A node that defers to a transmission its own
frame would not have harmed shows up as access delay and lower
channel use, not as a loss. Telling it apart needs a counterfactual reception per deferral,
which this track did not build.

## 5. Runtime: state cost per 1,000 vehicles

| state | per unit | per 1,000 vehicles | notes |
|---|---|---|---|
| shadowing, per directed link | ShadowProcess 64 B + key and B-tree overhead ≈ 95 B | ≈ 14 MB at 150 neighbours each | links are forgotten when a node retires (`forget_links`) |
| TR 37.885 link state | ≈ 110 B per link | ≈ 17 MB | state re-drawn every 100 ms |
| SPS sensing ring, LTE J3161 pool (10 sub-channels × 1,000 subframes × 32 B) | ≈ 328 KB per UE | ≈ 330 MB | the largest radio state; NR EN 303 798 (4 × 200 slots) is ≈ 27 KB per UE, ≈ 27 MB |
| `radio.channel_model` / `radio.phy` / `radio.sidelink` overrides | constants in the model | 0 | no per-link state added |
| hidden-terminal attribution | two map lookups per overlapping pair | 0 | no state |
| Building R-tree | per world, shared | 0 per vehicle | 7,390 Manhattan footprints |

The LTE sensing ring is the item to shrink next. Storing RSRP and S-RSSI as `f32`, and the
slot as an offset, would halve it. That changes outputs at the last bit, so it needs its
own determinism review.

## 6. Not done

- IPG (inter-packet gap) against OpenCV2X's IPG figures: not measured.
- B1 NLOS (the Manhattan two-leg formula): a building-blocked link keeps the LOS law plus
  the Sommer term, as Veins composes it.
- The OpenCV2X sub-channelisation (2 × 12 RB in 10 MHz) is not reproduced in the harness,
  which is the remaining 120 veh/km gap.
- Labelling exposed-node deferrals (§4).
