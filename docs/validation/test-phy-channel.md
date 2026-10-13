# Physical layer and channel: does each setting move the metrics as theory says?

Track: phy-channel, 2026-10-13, release binary of `e94e43c`. Runs: base-only runs of the
open 1.2 km grid (`scenarios/sensitivity/dsrc-grid.yaml`, 40 vehicles, 802.11p, same seed),
read as PRR per 20 m bin. The quantitative rows are the spec
`scenarios/sensitivity/phy-channel.yaml`. The directional rows for settings that had no
row are in `crates/v2xw-engine/tests/expectations/phy-channel.yaml`.

## Method

The base is made analytically clean:
- a single-slope log-distance law, n = 2.5;
- no shadowing and no fading;
- BSMs at 2 Hz for 4 s, so collisions stay under 3 %.

Without fading, PRR falls to 0 at the sensitivity distance d0. The budget is a 20 dBm
radiated power (the J2945/1 cap) plus 3 dBi of receive gain, against −92 dBm. That gives
115 dB = 67.9 dB (free space to 10 m at 5.89 GHz) + 25 log10(d0/10 m), so d0 = 766 m. A
budget change of D dB moves d0 by 10^(D/25) (Rappaport 2e, Eq. 4.68). For fading, the
range90 shift is the change in the fading law's 10th percentile: Gamma(3) is −4.35 dB,
Rayleigh −9.77 dB, Gamma(0.75) −12.5 dB, and Gamma(3) × lognormal(6 dB) is −9.2 dB (Monte
Carlo). The measured edge is the start of the first 20 m bin below PRR 0.5.

## Results: link budget at the sensitivity edge (no fading, 2 Hz)

| Setting | Change | Metric | Expected | Measured | Pass |
|---|---|---|---|---|---|
| (base) | — | edge (PRR 0.5) | 766 m | 760 m | yes |
| `radio.devices.obu.tx_power_dbm` | 23 → 11 dBm (EIRP 20 → 14) | edge | 440 m (×0.575) | 440 m | yes |
| `radio.devices.obu.tx_power_dbm` | 23 → 17 dBm | everything | no change (J2945/1 holds EIRP at 20 dBm) | bit-identical | yes |
| `radio.devices.obu.tx_power_dbm` | 23 → 14 (m = 3) | edge | ×0.76 | 740 → 540 m (×0.73) | yes |
| `radio.phy.sensitivity_dbm` | −92 → −86 | edge | ×0.575 | 740 → 420 m (×0.57, m = 3) | yes |
| `radio.phy.noise_figure_db` | 6 → 12 dB | edge | ×0.575 (SNR binds at about −86 dBm) | 760 → 440 m (×0.58) | yes |
| `radio.phy.data_rate_mbps` | 6 → 12 | edge | ×0.60 (5 to 6 dB) | 760 → 420 m (×0.55) | yes |
| `radio.channel_model.path_loss_exponent` | 2.5 → 3.0 | edge | 372 m | 360 m | yes |
| `radio.devices.obu.antenna_gain_dbi` | 3 → 6 dBi (m = 3) | edge | ×1.32 (receive side only) | 740 → 900 m (×1.22; bins past 900 m are thin) | yes, at the low edge |
| `radio.devices.obu.cable_loss_db` | 0 → 1 dB (m = 3) | edge | ×0.91 (receive side only) | 740 → 640 m (×0.86) | yes |
| `radio.dcc.enabled` | off (no fading) | edge | no change (the region cap is also 20 dBm) | 760 m | yes |
| `radio.region` | us-2016 → eu (no fading) | range90 | up (33 dBm EIRP, EN 302 571) | 720 → 1000 m (grid limit) | yes |

## Results: fading and shadowing (range90, 2 Hz)

| Setting | Change | Expected | Measured | Pass |
|---|---|---|---|---|
| `radio.channel_model.fading` | Nakagami m = 3 → none | ×1.49 | 480 → 720 m (×1.50) | yes |
| `radio.channel_model.fading` | m = 3 → Rayleigh | ×0.61 (312 m) | 320 m (×0.67) | yes |
| `radio.channel_model.shadowing_sigma_db` | 0 → 6 dB (m = 3) | ×0.64 | 480 → 340 m (×0.71) | yes |
| `radio.channel_model.shadowing_sigma_db` | 0 → 6 dB | PRR at 800 to 1000 m up | 0.15 → 0.29 (10 Hz base) | yes |
| `radio.channel_model.nakagami_shape_far` | 0.75 beyond 80 m | ×0.47 | 360 → 180 m (×0.50, 10 Hz base) | yes |
| `radio.channel_model.shadowing_decorrelation_m` | 10 → 200 m | mean PRR the same | 0.582 → 0.580 | yes |
| `radio.channel_model.shadowing_decorrelation_m` | 10 → 200 m | IPG p95 up | 200 → 200 ms | no: not resolvable (see below) |

## Results: law, height, buildings, weather (default medium tier, 2 Hz)

| Setting | Change | Expected | Measured | Pass |
|---|---|---|---|---|
| `radio.models.propagation` | WINNER+ B1, effective vs actual heights | effective (d'BP ≈ 20 m) far shorter | edge 220 m vs 560 m | yes |
| B1 actual heights, `antenna_height_m` | 1.5 → 3 m | up (d_BP ∝ h_t·h_r) | range90 400 → 760 m | yes |
| B1 effective heights, `antenna_height_m` | 1.5 → 3 m | up (h' 0.5 → 2 m) | edge 220 → 740 m | yes |
| two-ray ground, `antenna_height_m` | 1.5 → 3 m | up (d_c ∝ h_t·h_r) | range90 820 → 1000 m | yes |
| default dual-slope, `antenna_height_m` | 1.5 → 3 m | no change (the fit has no height term) | bit-identical | yes |
| `world.buildings` | block buildings on | PRR at 100 to 300 m down (NLOS round corners) | PRR at 200 m 0.99 → 0.22, edge 160 m | yes |
| `weather` rain 0.8, fog 0.8 | — | no visible change (P.838-3: 0.12 dB/km) | range90 unchanged (10 Hz base) | yes |

## What this found

- **No wiring defect in this area.** Every link-budget term moves the sensitivity edge by
  the factor the law gives, within one 20 m bin, once interference is out of the way.
  Fading and shadowing move range90 by the percentile shift of their distribution.
- **At 10 Hz the edge is interference-limited, and that is correct.** On the catalogue's
  DSRC family (10 Hz, 2 s), PRR from 300 m out is set by hidden-terminal collisions. With no
  fading, PRR is 0.99 at 200 m and 0.73 at 700 m, then drops to 0 at the 766 m sensitivity
  edge. Lowering every power by the same amount leaves the SINR unchanged. As a result,
  −6 dB of sensitivity moves range90 by only ×0.72 there, not ×0.575, and turning fading off
  *shortens* range90 (360 → 320 m). This is because Nakagami draws let some frames survive
  a mean SIR below the capture threshold. A study of link-budget settings should use a
  light load, or read `range_m` (PRR 0.5), not range90.
- **Two settings act less than their name suggests.** Both follow from the standards and
  need a sentence in the guide:
  - `obu.tx_power_dbm` above 17 dBm has no effect on US DSRC. J2945/1 sets the radiated
    power, at 20 dBm at most, so 23 dBm into the 3 dBi default antenna is cut back to
    17 dBm.
  - `obu.antenna_gain_dbi` and `cable_loss_db` act on the receive side only, for the same
    reason. The transmit gain is given back as conducted power.
- **`antenna_height_m` does nothing under the default law in open air.** The Abbas dual-slope
  fit has no height term. The height still matters for two-ray ground, WINNER+ B1 and
  TR 37.885, and for building and vehicle line-of-sight geometry, so the loader is right to
  accept it.
- **Shadowing decorrelation is not visible in IPG p95 on a 4 s run.** At 14 m/s a vehicle
  moves about 56 m. A 200 m correlation length freezes each link's shadow, so a shadowed
  link loses every frame and contributes no gaps. Testing it needs a run of a minute or
  more, or a per-link burst-length metric.
- **Missing metrics:** there is no RSSI or SINR distribution in the sensitivity
  fingerprint. The checks above use PRR per distance bin and the loss-cause shares.
