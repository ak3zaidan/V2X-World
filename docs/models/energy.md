# Energy and emissions

`v2xw-energy` scores what the traffic burns and emits: tailpipe CO₂, CO, HC, NOx, PM and
fuel of combustion vehicles, and the traction-battery energy of battery-electric ones. It
reads the recorded ground-truth trajectories (`gt.kinematics`) and nothing else, so the
same numbers come from a live run, from a replayed recording, or from a recording made
before the `energy` section existed, and a recording can be re-scored under another model
or fleet without re-simulating.

Code: `crates/v2xw-energy/src/` (`hbefa3`, `phemlight`, `moves`, `ev`, `powertrain`,
`provider`, `bands`), `crates/v2xw-cli/src/energy.rs` (the `v2xw energy` command).
Settings: [`energy`](../configuration/reference/energy.md). Card parameters:
[model parameters, emission and energy families](../configuration/reference/models.md#emission).

!!! note "Post-hoc, no feedback"
    Energy does not feed back into driving: an EV that runs out of charge keeps moving
    and is counted in `ev_depleted`. There is no charging infrastructure.

## The `energy` section

A scenario without the section computes no energy metrics (and keeps its content hash).
`energy: {}` runs HBEFA 3 over a Euro 4 petrol fleet (diesel for heavy trucks, buses and
coaches) with no electric vehicles.

```yaml
energy:
  emission_model: energy/emission/hbefa3    # or hbefa3 | phemlight | moves-opmode
  emission_model_by_segment:                # optional: another model per segment
    passenger-car: phemlight
  ev_model: energy/ev/kurczveil             # or kurczveil | vt-cpem
  fleet:
    passenger-car: { ev_share: 0.2, diesel_share: 0.3, euro: 6 }
    urban-bus:     { ev_share: 0.5 }
  ev:
    initial_soc: 0.8
    params: { mass_kg: 1600 }               # the EV model's card parameter names
  stop_speed_mps: 0.1
```

| Field | Default | Range | Meaning |
|---|---|---|---|
| `emission_model` | `energy/emission/hbefa3` | id or last segment | combustion model |
| `emission_model_by_segment.<segment>` | none | id | per-segment override |
| `ev_model` | `energy/ev/kurczveil` | id or last segment | battery-electric model |
| `fleet.<segment>.ev_share` | 0 | 0 to 1 | share of battery-electric vehicles |
| `fleet.<segment>.diesel_share` | 0 | 0 to 1 | diesel share among combustion vehicles |
| `fleet.<segment>.euro` | 4 | 0 to 6 | Euro emission standard |
| (segment not named) | all petrol, Euro 4, no EVs; diesel for `heavy-truck`, `urban-bus`, `coach` | | HBEFA 3 has no petrol bus or coach class |
| `ev.initial_soc` | 0.8 (`todo-calibrate`) | 0 to 1 | starting state of charge |
| `ev.params` | card defaults | the EV card's names | EV parameter overrides |
| `stop_speed_mps` | 0.1 | (0, 5] m/s | below this a vehicle is halting (SUMO's threshold) |

Segments: `passenger-car`, `light-commercial`, `heavy-truck`, `urban-bus`, `coach`,
`motorcycle`. Mobility classes map onto them (`passenger`, `emergency`, `taxi`… are
passenger cars; `delivery` is light commercial; `truck`, `trailer` heavy trucks; `bus`;
`coach`; `motorcycle`, `moped`). Pedestrians, bicycles and e-scooters are skipped; an
unknown class is counted in `energy_unmodelled`, never guessed.

### The fleet draw

Each vehicle draws its powertrain once, from the stream
`(plugin("energy/fleet/draw"), Actor(id))`: one uniform decides electric against
combustion, a second diesel against petrol. Both are always drawn, so changing one share
never shifts the other. The draw is keyed by the master seed, so a re-score reproduces the
run's own fleet.

## The integration interval

Between two consecutive `gt.kinematics` records of one actor, at most 5 s apart, every
model sees the same kinematically consistent interval:

$$
\bar v = \frac{v_0 + v_1}{2},\qquad a = \frac{v_1 - v_0}{\Delta t},\qquad s = \bar v\,\Delta t,\qquad \text{grade} = \frac{\Delta z}{s}\ (s \ge 1\,\text{m},\ |\text{grade}| \le 0.3).
$$

With these, $m\,a\,\bar v\,\Delta t = \tfrac12 m (v_1^2 - v_0^2)$ exactly: a power-based model
charges the kinetic energy the vehicle actually gained. The interval is charged to the
lane the vehicle was on at its start. A stop is counted when $v < $ `stop_speed_mps` after
$v > 1$ m/s (hysteresis).

## Emission models

### `energy/emission/hbefa3`: HBEFA 3.1 polynomial fit

45 classes (passenger cars and light commercial vehicles, petrol and diesel, Euro 0 to 6;
heavy trucks, urban buses, coaches), six quantities each. Tiers abstract, medium.

$$
a' = a + g \sin(\arctan(\text{grade})),\qquad
E = \frac{\max\!\big(0,\; c_0 + c_1 a' v + c_2 a'^2 v + c_3 v + c_4 v^2 + c_5 v^3\big)}{3.6}\ \ [\text{mg/s}],
$$

with $v$ in m/s, $g = 9.80665$ m/s² and one coefficient row per class and quantity (the
fuel row is fuel mass). Coefficients: Krajzewicz et al. 2015, transcribed from Eclipse
SUMO `HelpersHBEFA3.cpp` (numeric data only).

!!! warning "High idle"
    The intercept is the standstill rate and it is high: PC_G_EU4 idles at 2.6 g/s CO₂
    and 0.84 g/s fuel, about four times a warm petrol car's measured idle. Traffic that
    queues a lot reads high, and an application that removes idling reads a larger
    saving under HBEFA 3 than under PHEMlight. SUMO's `HBEFA3` behaves the same.

### `energy/emission/phemlight`: power-based (TU Graz)

The two openly published vehicles, a Euro 4 petrol and a Euro 4 diesel passenger car.
Tiers medium, high.

$$
P = \frac{(m{+}L) g (f_0 + f_1 v + f_4 v^4) v + \tfrac{\rho}{2} c_d A v^3 + (m\,k_{\text{rot}}(v) + m_{\text{rot}} + L) a v + (m{+}L) g\,\tfrac{\text{grade}\%}{100} v}{1000\,\eta} + P_{\text{aux}}\ \ [\text{kW}]
$$

$$
FC = FC_{\text{curve}}(P/P_{\text{rated}})\,P_{\text{rated}},\qquad
E_X = E_{X,\text{curve}}(P/P_{\text{drive}}),\ P_{\text{drive}} = P(19.444\,\text{m/s},\ 0.45\,\text{m/s}^2,\ 0),
$$

$$
\text{CO}_2 = \frac{FC\,f_C - 0.429\,\text{CO} - 0.866\,\text{HC}}{0.273},\quad f_C = 0.865\ \text{(petrol)},\ 0.863\ \text{(diesel)}.
$$

Fuel is cut off (every rate 0) when the car decelerates harder than it would coast in
gear. Idle rows apply at $|v| \le 0.5$ m/s. A Euro 0 to 3, 5 or 6 car is computed with the
Euro 4 vehicle of its fuel. Parameters: gravity 9.81, air density 1.182 kg/m³,
drivetrain efficiency 0.9 (SUMO's PHEMlight data, EPL-2.0; Hausberger et al. 2009).

### `energy/emission/moves-opmode`: US EPA MOVES operating modes

Each interval is binned into one of 23 running operating modes by speed and scaled
tractive power, and charged that mode's rate. Light-duty only. Tiers abstract, medium.

$$
\text{STP} = \frac{A v + B v^2 + C v^3 + m v (a + g \sin\theta)}{f}\ \ [\text{kW/tonne}],\qquad E = \frac{\text{rate}[\text{mode}]}{3600}\ \ [\text{g/s}],
$$

mode 0 braking ($a \le -2$ mph/s, or $a < -1$ mph/s for 3 s), mode 1 idle ($v < 1$ mph),
11 to 16, 21 to 30, 33 to 40 by speed band and STP bin. Fuel by carbon balance from CO₂.

| Source type | A (kW s/m) | B (kW s²/m²) | C (kW s³/m³) | m, f (t) |
|---|---|---|---|---|
| passenger car | 0.156461 | 0.002002 | 0.000493 | 1.4788 |
| passenger truck | 0.22112 | 0.002838 | 0.000698 | 1.86686 |

Rates: MOVESTAR's light-duty vehicle and truck tables (Wang, Wu & Scora 2020,
arXiv:2008.04986), derived from MOVES. `v2xw energy --opmode-csv` writes the
operating-mode distribution per lane (MOVES `opModeDistribution`).

## Electric-vehicle models

### `energy/ev/kurczveil`: SUMO's battery device (default)

$$
\Delta E = \tfrac12 (m + m_{\text{rot}})(v_1^2 - v_0^2) + m g \sin\theta\, s + \tfrac12 \rho A c_d \bar v^2 s + c_r m g s + c_{\text{rad}} \frac{m \bar v^2}{r}\Delta t,
$$

$$
E_{\text{bat}} = \begin{cases} \Delta E / \eta_{\text{prop}} & \Delta E > 0 \\ \Delta E\, \eta_{\text{recup}}\, e^{-\alpha/|a|} & \Delta E \le 0 \end{cases},\qquad P = E_{\text{bat}}/\Delta t + P_{\text{const}},
$$

with $r = s/|\Delta\psi|$ clamped to $[10^{-4}, 10^4]$ m.

| Parameter | Default | Unit |
|---|---|---|
| `mass_kg` | 1830 | kg |
| `front_area_m2` | 2.6 | m² |
| `drag_coefficient` | 0.35 | |
| `rotating_mass_kg` | 40 | kg |
| `roll_coefficient` | 0.01 | |
| `radial_coefficient` | 0.1 | |
| `constant_power_w` | 100 | W |
| `propulsion_efficiency` | 0.98 | |
| `recuperation_efficiency` | 0.96 | |
| `recuperation_decel_alpha` | 0 (SUMO); > 0 applies Fiori 2016's regeneration | m/s² |
| `air_density` | 1.2041 | kg/m³ |
| `capacity_wh` | 35000 | Wh |

Source: Kurczveil, López & Schnieder 2014 (SUMO 2013, LNCS 8594); defaults from SUMO
`HelpersEnergy`.

### `energy/ev/vt-cpem`: VT-CPEM (Fiori, Ahn & Rakha 2016)

$$
P_w = \Big(m a + m g \cos\theta\,\frac{C_r}{1000}(c_1 v_{\text{km/h}} + c_2) + \tfrac12 \rho A_f C_D v^2 + m g \sin\theta\Big) v,
$$

$$
P_m = \begin{cases} P_w / (\eta_D \eta_{EM}) & P_w \ge 0 \\ P_w\, \eta_D \eta_{EM} \eta_{rb},\ \ \eta_{rb} = e^{-\alpha/|a|}\ (a<0) & P_w < 0 \end{cases},\qquad
P_b = \frac{P_m}{\eta_{\text{bat}}}\ \text{or}\ P_m \eta_{\text{bat}},\ + P_{\text{aux}}.
$$

Parameterised for a 2011 to 2013 Nissan Leaf: 1521 kg, $C_D$ 0.28, $A_f$ 2.3316 m²,
$\eta_D$ 0.92, $\eta_{EM}$ 0.91, $\eta_{\text{bat}}$ 0.9 (`todo-calibrate`), $P_{\text{aux}}$ 700 W,
$\alpha$ 0.0411, 24 kWh. Rolling resistance from Rakha et al. 2001 ($C_r$ 1.75, $c_1$ 0.0328,
$c_2$ 4.575).

!!! warning "Recalled values"
    The card marks the Fiori 2016 parameter table as recalled, not re-read; check it
    before citing results.

## Metrics

Provider `metrics/energy` (visibility GT):

| Metric | Unit | Window | Dimensions |
|---|---|---|---|
| `co2_mass`, `fuel_mass`, `nox_mass`, `pm_mass`, `co_mass`, `hc_mass` | g | per flush | total, by `class` |
| `ev_battery_energy` (net), `ev_regen_energy` | Wh | per flush | total, by `class` |
| `vehicle_distance` | km | per flush | total, by `class` |
| `vehicle_stops` | count | per flush | total, by `class` |
| `link_co2_mass`, `link_nox_mass`, `link_pm_mass`, `link_fuel_mass`, `link_ev_energy`, `link_distance` | g, Wh, km | per flush | `region` = lane id |
| `co2_per_km`, `nox_per_km` | g/km | run so far | total, by `class` |
| `fuel_per_100km` | L/100 km | run so far | total, by `class` |
| `ev_energy_per_100km` | kWh/100 km | run so far | total, by `class` |
| `stops_per_km` | 1/km | run so far | total, by `class` |
| `vehicle_co2_per_km` | g/km | run so far | distribution over combustion vehicles with ≥ 0.5 km |
| `ev_soc_min` | ratio | run so far | lowest state of charge any EV reached |
| `ev_depleted` | count | run so far | EVs that needed energy the battery did not have |
| `energy_unmodelled` | count | run so far | vehicles the emission model has no class for |

Fuel volume uses 742 g/L (petrol) and 836 g/L (diesel), the point values of EN 228 and
EN 590 that SUMO uses.

## Health bands

`v2xw_energy::bands` grades a run's latest samples: `ok` inside the band, `warn` outside
it but inside the plausible range, `fail` outside that (almost certainly a defect), and
`not-measured` when there is no usable sample. A band is a sanity range, not a target.

| Metric | Class | OK | Plausible | Basis |
|---|---|---|---|---|
| `co2_per_km` | passenger | 90 to 350 g/km | 40 to 700 | EEA 2010 new-car fleet average 140.3 g/km (NEDC); urban stop-and-go well above it |
| `fuel_per_100km` | passenger | 4 to 15 | 2 to 30 | the CO₂ band through EPA's 8,887 g CO₂ per gallon |
| `ev_energy_per_100km` | passenger | 9 to 25 kWh | 3 to 45 | 2013 Nissan Leaf label, 18 kWh/100 km at the wall |
| `nox_per_km` | passenger | 0 to 1 g/km | 0 to 3 | Directive 98/69/EC Euro 4 limits |
| `ev_soc_min` | all | 0.05 to 1 | 0 to 1 | below 5 % an EV would be seeking a charger |
| `energy_unmodelled` | all | 0 | ≥ 0 | every vehicle had a class in the selected model |

## The `v2xw energy` command

```bash
v2xw energy runs/<name>/recording.mcap
v2xw energy runs/<name>/recording.mcap --emission-model phemlight --ev-model vt-cpem
v2xw energy runs/<name>/recording.mcap --spec fleet-2030.yaml \
    --vehicles-csv vehicles.csv --opmode-csv opmode.csv --json
```

| Argument | Meaning |
|---|---|
| `RECORDING` | a full-profile recording (it reads `gt.kinematics`) |
| `--spec FILE` | a JSON or YAML `energy:` section body; replaces the scenario's |
| `--emission-model ID` | `hbefa3`, `phemlight`, `moves-opmode` or a full id |
| `--ev-model ID` | `kurczveil`, `vt-cpem` or a full id |
| `--vehicles-csv FILE` | one row per vehicle |
| `--opmode-csv FILE` | MOVES operating-mode distribution per lane |
| `--json` | print the outcome as JSON |

The starting section is the one the recording's attached scenario carried; the
simulation is not repeated, and the same recording scored twice gives the same numbers.
A NODE-only recording has no `gt.kinematics` and cannot be scored.

## Modelled versus simplified

| Modelled | Simplified or absent |
|---|---|
| hot running exhaust emissions by three published methods | cold start, evaporative emissions, non-exhaust PM (brakes, tyres, road), ambient temperature and air conditioning, deterioration |
| HBEFA 3 for 45 classes | no two-wheeler class; polynomial extrapolates outside its driving situations; no SUMO coasting rule |
| PHEMlight with fuel cut-off | two vehicles only (Euro 4 petrol and diesel passenger car); PN, NO and NOx corrections ignored |
| MOVES operating modes with MOVESTAR rates | light-duty only; one MOVES run's fleet and calendar year; start and hotelling processes ignored |
| EV energy balance with regeneration (two models) | constant efficiencies (no motor map), no battery resistance, temperature or ageing; no charging |
| grade from the recorded height change | grade clamped to ±0.3; upstream (well-to-tank) emissions ignored |
