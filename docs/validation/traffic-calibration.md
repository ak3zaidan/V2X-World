# Traffic calibration: fundamental diagrams, waves, headways and human drivers

Written by `cargo run -p v2xw-trafficval --bin traffic_calibration` (full mode: 300 s warm-up, 300 s measured per density). Every run is the engine's own `NativeMobility` on a single-lane ring (`v2xw_trafficval::fdlab`), loaded evenly with one car 20 % slow. Overall: **pass**.

Driver populations: *identical* is the textbook model (every car the preset's driver, reacting at once); *human* is `actors.vehicles.drivers.realism: human` (`v2xw_mobility::drivers::HumanDriverParams::human`): every driver their own T, a, b, s0 (mean-one lognormal, cv 0.20–0.25), a 0.8 s reaction time (Green 2000) and the Human Driver Model's estimation errors (V_s = 5 %, σ_r = 0.01 s⁻¹, τ̃ = 20 s; Treiber, Kesting & Helbing 2006). *SUMO default* is SUMO's default car: Krauß with σ = 0.5, τ = 1 s, a = 2.6 m/s², b = 4.5 m/s², 5 m long, minGap 2.5 m (the SUMO vType defaults).

## Held figures

| figure | measured | band | reference | verdict |
|---|---|---|---|---|
| city capacity below the IDM's equilibrium maximum flow, identical drivers, fraction | 0.000 | 0.000 – 0.050 | max over s of v_e(s)/(s + l) = 1899 veh/h (Treiber & Kesting 2013 §11.3.4, the IDM's steady state); the city drivers are string stable here, so the ring sits on it, less the density grid's step | pass |
| corridor capacity below the IDM's equilibrium maximum flow, identical drivers, fraction | 0.003 | 0.000 – 0.200 | max v_e(s)/(s + l) = 1742 veh/h; Treiber 2000's drivers are string unstable near capacity, so a perturbed ring sits below it | pass |
| corridor capacity against Kesting 2010's Q_max, identical drivers, ratio | 0.874 | — | Kesting 2010 Eq. 4.1 approximation: Q_max = 1989 veh/h | reported |
| city capacity against Kesting 2010's Q_max, identical drivers, ratio | 0.793 | — | Kesting 2010 Eq. 4.1 approximation: Q_max = 2394 veh/h; it ignores the (v/v0)^δ term, which matters at 50 km/h | reported |
| corridor capacity, human drivers, against identical ones, ratio | 0.979 | 0.800 – 1.050 | the population keeps the mean parameters, so it keeps capacity within its own waves' loss | pass |
| corridor capacity, Treiber 2000 drivers, against the HCM's 2,400 pc/h/ln, ratio | 0.724 | — | HCM 6th ed. Exhibit 12-6 (secondary): Treiber 2000's T = 1.6 s is a cautious German-autobahn calibration, below the HCM's US base capacity by construction | reported |
| corridor capacity, SUMO default, against the HCM's 2,400 pc/h/ln, ratio | 1.170 | — | Krauß with τ = 1 s: q_max ≈ 1/(τ + l_eff/v0) | reported |
| triangular fit beats Greenshields on the corridor (R² congested branch − R² Greenshields) | 0.073 | 0.000 – 1.000 | Greenshields' linear speed-density law is known to fit freeway data poorly near capacity; the triangular (Newell 1993; Daganzo 1994) is the standard (Treiber & Kesting 2013 §4.4) | pass |
| stop-and-go wave speed, human drivers (median over jammed densities), km/h | -12.146 | -25.000 – -10.000 | upstream at 15–20 km/h (Kerner & Rehborn 1996; Treiber & Kesting 2013 §18.1: −15 ± 5 km/h) | pass |
| triangular congested wave speed, corridor human drivers, km/h | -13.566 | -25.000 – -10.000 | same: the slope of the congested branch is the kinematic wave speed | pass |
| densities (of 11) where human city drivers break into stop-and-go and identical ones do not | 5.000 | 1.000 – 11.000 | Treiber, Kesting & Helbing 2006 (HDM): a reaction time of the order of the time gap destabilises traffic the ODE model carries smoothly | pass |
| jammed densities, city identical / human | 106.000 | — | encoded as identical × 100 + human | reported |
| smallest net gap in any run, m (no collision) | 1.037 | 0.000 – inf | a car-following model must be collision-free (Treiber & Kesting 2013 §11.3) | pass |
| headway coefficient of variation near capacity, human corridor drivers | 0.218 | 0.150 – 1.500 | observed freeway headways are widely spread (lognormal-like, cv well above zero; NGSIM I-80 / US-101 per-lane headways, e.g. Treiber & Kesting 2013 Fig. 3.7, secondary). Identical drivers give a cv near zero. | pass |
| headway coefficient of variation near capacity, identical corridor drivers | 0.006 | — | the ideal model: every car the same gap | reported |

## Fundamental-diagram fits

Greenshields: free speed km/h / jam density veh/km / capacity veh/h / R² of speed on density. Triangular: free speed km/h / congested wave km/h / jam density veh/km / critical density veh/km / capacity veh/h / R² of the congested branch.

| configuration | highest measured flow, veh/h | Greenshields | triangular |
|---|---|---|---|
| city, IDM city drivers, identical | 1899 | 57.8 / 131 / 1893 / 0.981 | 48.1 / -16.6 / 161 / 41.1 / 1980 / 0.894 |
| city, IDM city drivers, human | 1834 | 56.7 / 120 / 1703 / 0.935 | 48.2 / -17.9 / 138 / 37.3 / 1800 / 0.982 |
| city, SUMO default (Krauss) | 2240 | 63.4 / 127 / 2013 / 0.919 | 49.8 / -26.4 / 133 / 46.1 / 2297 / 1.000 |
| corridor, IDM Treiber 2000, identical | 1737 | 104.3 / 94 / 2448 / 0.851 | 113.8 / -12.2 / 158 / 15.3 / 1739 / 0.924 |
| corridor, IDM Treiber 2000, human | 1700 | 102.2 / 91 / 2334 / 0.824 | 113.3 / -13.6 / 137 / 14.6 / 1659 / 0.970 |
| corridor, SUMO default (Krauss) | 2808 | 131.0 / 99 / 3257 / 0.918 | 118.2 / -26.3 / 133 / 24.3 / 2866 / 1.000 |

Against SUMO: the same ring, the same densities, SUMO's default driver. On the corridor the engine's IDM carries 1737 veh/h at its best density against Krauß's 2808; its congested wave runs at -12.2 km/h (human drivers -13.6) against Krauß's -26.3. Krauß's dawdling (σ) is what makes SUMO's default traffic jam at all; the engine gets its waves from reaction time and misjudgement instead, which is what the HDM showed reproduces the observed wave speed and capacity drop without random braking.

## Headways near capacity

Time headways at a fixed detector, at the density of the most flow.

| configuration | n | mean, s | cv | share < 1 s | p15, s | median, s | p85, s |
|---|---|---|---|---|---|---|---|
| city, identical | 157 | 1.90 | 0.000 | 0.000 | 1.90 | 1.90 | 1.90 |
| city, human | 152 | 1.97 | 0.153 | 0.000 | 1.68 | 1.92 | 2.25 |
| city, SUMO default | 186 | 1.61 | 0.060 | 0.000 | 1.57 | 1.58 | 1.63 |
| corridor, identical | 143 | 2.08 | 0.006 | 0.000 | 2.07 | 2.08 | 2.09 |
| corridor, human | 140 | 2.13 | 0.218 | 0.000 | 1.67 | 2.09 | 2.62 |
| corridor, SUMO default | 232 | 1.28 | 0.028 | 0.000 | 1.25 | 1.27 | 1.32 |

NGSIM: the I-80 and US-101 trajectory sets are not in this repository, so the headway rows are held only to the spread a heterogeneous population must show; importing NGSIM's per-lane detector headways and holding the distribution (Kolmogorov-Smirnov) is the next step.

## Per-density runs

**city, IDM city drivers, identical**

| density, veh/km | speed, km/h | flow, veh/h | speed sd, m/s | slowest, m/s | wave, km/h | smallest gap, m |
|---|---|---|---|---|---|---|
| 10.0 | 49.7 | 497 | 0.002 | 13.80 | NaN | 93.55 |
| 20.0 | 48.5 | 970 | 0.002 | 13.47 | NaN | 44.79 |
| 30.0 | 46.2 | 1386 | 0.002 | 12.83 | NaN | 28.28 |
| 40.0 | 42.5 | 1702 | 0.001 | 11.82 | NaN | 19.98 |
| 45.0 | 40.2 | 1808 | 0.001 | 11.16 | NaN | 17.21 |
| 50.0 | 37.5 | 1875 | 0.001 | 10.41 | NaN | 14.99 |
| 55.0 | 34.5 | 1899 | 0.002 | 9.59 | NaN | 13.17 |
| 70.0 | 25.1 | 1760 | 0.005 | 6.98 | NaN | 9.28 |
| 90.0 | 14.7 | 1325 | 0.012 | 4.06 | NaN | 6.08 |
| 110.0 | 7.5 | 828 | 0.008 | 2.07 | NaN | 4.07 |
| 130.0 | 2.5 | 324 | 0.002 | 0.69 | NaN | 2.69 |

**city, IDM city drivers, human**

| density, veh/km | speed, km/h | flow, veh/h | speed sd, m/s | slowest, m/s | wave, km/h | smallest gap, m |
|---|---|---|---|---|---|---|
| 10.0 | 49.7 | 497 | 0.049 | 13.56 | NaN | 53.02 |
| 20.0 | 48.5 | 971 | 0.084 | 13.12 | NaN | 24.83 |
| 30.0 | 46.4 | 1392 | 0.130 | 12.35 | NaN | 13.99 |
| 40.0 | 42.9 | 1717 | 0.171 | 11.25 | NaN | 10.23 |
| 45.0 | 40.8 | 1834 | 0.199 | 10.25 | NaN | 9.13 |
| 50.0 | 30.9 | 1545 | 4.191 | 0.00 | -20.2 | 1.62 |
| 55.0 | 26.3 | 1446 | 4.695 | 0.00 | -19.3 | 1.51 |
| 70.0 | 16.5 | 1155 | 4.227 | 0.00 | -17.9 | 1.32 |
| 90.0 | 8.9 | 801 | 2.277 | 0.00 | -17.0 | 1.37 |
| 110.0 | 4.4 | 481 | 0.937 | 0.00 | -16.5 | 1.35 |
| 130.0 | 1.6 | 210 | 0.476 | 0.00 | NaN | 1.04 |

**city, SUMO default (Krauss)**

| density, veh/km | speed, km/h | flow, veh/h | speed sd, m/s | slowest, m/s | wave, km/h | smallest gap, m |
|---|---|---|---|---|---|---|
| 10.0 | 49.8 | 498 | 0.036 | 13.74 | NaN | 93.27 |
| 20.0 | 49.8 | 996 | 0.037 | 13.74 | NaN | 43.49 |
| 30.0 | 49.8 | 1494 | 0.037 | 13.74 | NaN | 26.35 |
| 40.0 | 49.8 | 1991 | 0.037 | 13.73 | NaN | 16.65 |
| 45.0 | 49.8 | 2240 | 0.037 | 13.67 | NaN | 16.58 |
| 50.0 | 44.0 | 2199 | 0.091 | 10.13 | NaN | 14.49 |
| 55.0 | 37.6 | 2066 | 0.067 | 9.63 | NaN | 13.01 |
| 70.0 | 23.8 | 1664 | 0.052 | 6.31 | NaN | 9.20 |
| 90.0 | 12.5 | 1128 | 0.044 | 3.28 | NaN | 6.03 |
| 110.0 | 5.5 | 609 | 0.024 | 1.47 | NaN | 4.06 |
| 130.0 | 0.7 | 88 | 0.003 | 0.18 | NaN | 2.69 |

**corridor, IDM Treiber 2000, identical**

| density, veh/km | speed, km/h | flow, veh/h | speed sd, m/s | slowest, m/s | wave, km/h | smallest gap, m |
|---|---|---|---|---|---|---|
| 5.0 | 117.5 | 587 | 0.038 | 32.58 | NaN | 186.77 |
| 10.0 | 110.1 | 1101 | 0.019 | 30.54 | NaN | 94.00 |
| 15.0 | 98.3 | 1474 | 0.016 | 27.25 | NaN | 61.38 |
| 20.0 | 83.8 | 1676 | 0.144 | 23.03 | NaN | 44.22 |
| 25.0 | 69.5 | 1737 | 0.737 | 17.39 | NaN | 30.53 |
| 30.0 | 55.8 | 1674 | 2.637 | 7.50 | NaN | 13.28 |
| 40.0 | 37.4 | 1495 | 3.835 | 0.72 | -12.5 | 3.04 |
| 55.0 | 23.5 | 1291 | 2.771 | 0.03 | -13.6 | 2.02 |
| 70.0 | 15.6 | 1095 | 1.641 | 0.07 | -13.5 | 2.08 |
| 90.0 | 9.1 | 818 | 0.660 | 0.12 | -13.6 | 2.15 |
| 110.0 | 4.7 | 517 | 0.093 | 0.68 | NaN | 3.04 |

**corridor, IDM Treiber 2000, human**

| density, veh/km | speed, km/h | flow, veh/h | speed sd, m/s | slowest, m/s | wave, km/h | smallest gap, m |
|---|---|---|---|---|---|---|
| 5.0 | 116.9 | 584 | 0.425 | 30.79 | NaN | 103.99 |
| 10.0 | 109.7 | 1097 | 0.629 | 28.23 | NaN | 46.26 |
| 15.0 | 98.6 | 1479 | 0.814 | 25.11 | NaN | 23.10 |
| 20.0 | 85.0 | 1700 | 0.988 | 20.89 | NaN | 18.78 |
| 25.0 | 61.8 | 1545 | 6.649 | 3.75 | NaN | 5.16 |
| 30.0 | 49.1 | 1474 | 7.391 | 0.00 | -11.6 | 1.37 |
| 40.0 | 34.1 | 1364 | 4.469 | 0.00 | -12.0 | 2.09 |
| 55.0 | 20.6 | 1131 | 4.729 | 0.00 | -12.1 | 1.33 |
| 70.0 | 12.3 | 860 | 3.413 | 0.00 | -12.0 | 1.29 |
| 90.0 | 6.9 | 623 | 1.577 | 0.00 | -11.7 | 1.33 |
| 110.0 | 3.4 | 370 | 0.630 | 0.00 | NaN | 1.33 |

**corridor, SUMO default (Krauss)**

| density, veh/km | speed, km/h | flow, veh/h | speed sd, m/s | slowest, m/s | wave, km/h | smallest gap, m |
|---|---|---|---|---|---|---|
| 5.0 | 119.6 | 598 | 0.036 | 33.14 | NaN | 183.63 |
| 10.0 | 119.6 | 1196 | 0.037 | 33.14 | NaN | 83.85 |
| 15.0 | 119.6 | 1794 | 0.037 | 33.14 | NaN | 52.24 |
| 20.0 | 119.6 | 2393 | 0.037 | 33.14 | NaN | 39.17 |
| 25.0 | 112.3 | 2808 | 0.788 | 26.75 | NaN | 33.57 |
| 30.0 | 90.0 | 2701 | 0.538 | 20.48 | NaN | 26.89 |
| 40.0 | 61.5 | 2462 | 0.178 | 14.23 | NaN | 19.25 |
| 55.0 | 37.6 | 2065 | 0.069 | 8.79 | NaN | 12.84 |
| 70.0 | 23.8 | 1664 | 0.052 | 6.28 | NaN | 9.20 |
| 90.0 | 12.5 | 1128 | 0.044 | 3.28 | NaN | 6.04 |
| 110.0 | 5.5 | 609 | 0.024 | 1.47 | NaN | 4.06 |

## The model-level checks behind the open findings

The IDM's gap at rest (1.927 m against the old band [1.95, 2.5]) and string stability against theory at six densities were open findings; both are re-run here. The rest gap is held to an independent integration and to s0 within the continuous model's own shortfall; Sugiyama's ring now runs with the human population and its fault is the ideal driver.

#### `model/idm-stops-at-s0` — A city driver closing on a standing car stops at the jam distance s0

| figure | measured | band | verdict |
|---|---|---|---|
| gap at rest, |engine − independent IDM integration at 0.1 s|, m | 0.0000 | 0.000 – 0.000 | pass |
| gap at rest, m | 1.9270 | 1.900 – 2.500 | pass |
| smallest gap on the approach, m | 1.9270 | 1.900 – inf | pass |
| speed after 120 s, m/s | 0.0000 | 0.000 – 0.050 | pass, not proved |
| peak deceleration, m/s² | 1.9650 | — | reported |

independent integration: 1.9270 m at 0.1 s, 1.9156 m at 1 ms

- fault *the model evaluated with s0 = 0.5 m instead of the driver's 2 m*: red on gap at rest, |engine − independent IDM integration at 0.1 s|, m; gap at rest, m; smallest gap on the approach, m

#### `model/string-stability` — Platoons amplify or damp a disturbance exactly where linear string-stability theory says

| figure | measured | band | verdict |
|---|---|---|---|
| densities where simulation and theory disagree on growth (|G| > 1) | 0.0000 | 0.000 – 0.000 | pass |
| largest |simulated − linear-theory| per-car gain at the probe frequency | 0.0012 | 0.000 – 0.003 | pass |
| city drivers (UrbanHcm), 25 mph: lowest unstable density, veh/km | 90.0000 | — | reported |
| Treiber 2000, 120 km/h: lowest unstable density, veh/km | 30.0000 | — | reported |
| Treiber 2000: densities (of 12) where the platoon is unstable | 10.0000 | 1.000 – 11.000 | pass, not proved |

city drivers (UrbanHcm), 25 mph: 10: criterion +0.191, ω 0.437 rad/s, |G| theory 0.0198, simulated 0.0198 (car-to-car 0.0198 0.0199 0.0258 0.0740); 20: criterion +0.207, ω 0.455 rad/s, |G| theory 0.0833, simulated 0.0834 (car-to-car 0.0834 0.0836 0.0903 0.0875); 30: criterion +0.225, ω 0.475 rad/s, |G| theory 0.1918, simulated 0.1923 (car-to-car 0.1922 0.1923 0.1921 0.1854); 40: criterion +0.232, ω 0.481 rad/s, |G| theory 0.3427, simulated 0.3435 (car-to-car 0.3435 0.3435 0.3435 0.3436); 50: criterion +0.210, ω 0.458 rad/s, |G| theory 0.5320, simulated 0.5331 (car-to-car 0.5331 0.5332 0.5331 0.5331); 60: criterion +0.155, ω 0.394 rad/s, |G| theory 0.7479, simulated 0.7490 (car-to-car 0.7490 0.7491 0.7491 0.7490); 70: criterion +0.084, ω 0.289 rad/s, |G| theory 0.9298, simulated 0.9303 (car-to-car 0.9302 0.9302 0.9302 0.9303); 80: criterion +0.020, ω 0.142 rad/s, |G| theory 0.9968, simulated 0.9969 (car-to-car 0.9969 0.9969 0.9969 0.9969); 90: criterion -0.026, ω 0.161 rad/s, |G| theory 1.0011, simulated 1.0011 (car-to-car 1.0011 1.0011 1.0011 1.0011); 100: criterion -0.060, ω 0.244 rad/s, |G| theory 1.0037, simulated 1.0038 (car-to-car 1.0038 1.0038 1.0038 1.0038); 110: criterion -0.089, ω 0.299 rad/s, |G| theory 1.0057, simulated 1.0058 (car-to-car 1.0058 1.0059 1.0059 1.0059); 120: criterion -0.122, ω 0.350 rad/s, |G| theory 1.0072, simulated 1.0074 (car-to-car 1.0072 1.0073 1.0074 1.0074)

Treiber 2000, 120 km/h: 10: criterion +0.008 (marginal, not compared); 20: criterion +0.001 (marginal, not compared); 30: criterion -0.012, ω 0.110 rad/s, |G| theory 1.0190, simulated 1.0192 (car-to-car 1.0192 1.0192 1.0192 1.0192); 40: criterion -0.021, ω 0.145 rad/s, |G| theory 1.0290, simulated 1.0293 (car-to-car 1.0292 1.0293 1.0293 1.0294); 50: criterion -0.029, ω 0.170 rad/s, |G| theory 1.0333, simulated 1.0337 (car-to-car 1.0338 1.0337 1.0337 1.0336); 60: criterion -0.036, ω 0.190 rad/s, |G| theory 1.0348, simulated 1.0352 (car-to-car 1.0352 1.0352 1.0353 1.0353); 70: criterion -0.044, ω 0.210 rad/s, |G| theory 1.0346, simulated 1.0350 (car-to-car 1.0349 1.0349 1.0350 1.0351); 80: criterion -0.052, ω 0.227 rad/s, |G| theory 1.0329, simulated 1.0333 (car-to-car 1.0333 1.0332 1.0333 1.0333); 90: criterion -0.060, ω 0.244 rad/s, |G| theory 1.0301, simulated 1.0304 (car-to-car 1.0305 1.0304 1.0304 1.0304); 100: criterion -0.067, ω 0.259 rad/s, |G| theory 1.0261, simulated 1.0265 (car-to-car 1.0265 1.0264 1.0265 1.0265); 110: criterion -0.073, ω 0.271 rad/s, |G| theory 1.0212, simulated 1.0215 (car-to-car 1.0215 1.0215 1.0216 1.0216); 120: criterion -0.077, ω 0.277 rad/s, |G| theory 1.0155, simulated 1.0157 (car-to-car 1.0158 1.0157 1.0156 1.0157)

- fault *the simulated drivers react 0.8 s late (a delay the ODE criterion does not have)*: red on densities where simulation and theory disagree on growth (|G| > 1); largest |simulated − linear-theory| per-car gain at the probe frequency

#### `model/sugiyama-ring` — 22 city drivers on a 230 m circuit form a stop-and-go jam with no bottleneck (Sugiyama et al. 2008)

| figure | measured | band | verdict |
|---|---|---|---|
| slowest car in the jam, m/s | 0.0000 | 0.000 – 2.000 | pass |
| jam propagation speed, km/h | -14.9352 | -25.000 – -10.000 | pass |
| mean speed, km/h | 7.0284 | — | reported |
| speed standard deviation, m/s | 1.7573 | — | reported |

- fault *the ideal driver: no reaction time, no perception errors, every driver alike*: red on slowest car in the jam, m/s; jam propagation speed, km/h

