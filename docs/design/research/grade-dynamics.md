# Grade-aware, power-limited acceleration: the class road-load table (WP-22)

Status: 2026-10-28, P22.1 table and P22.2 clamp built (`v2xw-mobility`). Plan:
`docs/design/frameworks/wp16-24-build-plans.md` §WP-22.

## The bound

After every other limit of the decision pass (`MobilityEngine::step`, pass 6), a motor
vehicle's acceleration is held to

`a ≤ min(a_drv, η·P/(m·max(v, 1)) − c_rr·g − ½·ρ·C_dA·v²/m) − g·sin θ`

- `a_drv` is the driver's comfortable acceleration (the IDM `a`).
- `sin θ = (z(s+5) − z(s−5)) / 10` along the lane's 3-D arc length, cut to the lane and
  clamped at 0.3 (the energy provider's `GRADE_CLAMP`). A level chord gives exactly 0.
- It is a `min`: braking and a downhill hold are untouched, so a descent never overspeeds.
- `traffic.vehicle_dynamics.grade` switches the `g·sin θ` term (SUMO KraussPS); with it
  alone the bound is `a_drv − g·sin θ`, and a level chord applies nothing, so flat maps are
  byte-identical. `traffic.vehicle_dynamics.power_limit` switches the power term.
- The mass is the row's own (`ClassDynamics::mass_kg`): the SUMO class mass for the car,
  the loaded design mass for the trailer (SUMO's 13 t is an unladen figure).
- Constants: g 9.81 m/s², ρ 1.182 kg/m³ (PHEMlight `GRAVITY_CONST`, `AIR_DENSITY_CONST`).
- A physical law: it reads ground-truth lane geometry (rule 6 of the plan).

## The table (`VehicleClass::dynamics`)

| Class | P | C_dA | c_rr | η | Source |
|---|---|---|---|---|---|
| passenger (1,500 kg) | 93 kW | 0.3113 × 2.16 = 0.672 m² | 0.009 | 0.9 | PHEMlight `PC_D_EU4` (SUMO `PC_D_EU4.PHEMLight.veh`), already in `v2xw-energy/src/phemlight.rs`; the test `classes::tests::passenger_road_load_is_phemlight_pc_d_eu4` reads it from there [repo]. |
| trailer (36,287 kg loaded) | 302.4 kW = m / 120 (405 hp) | 5.95 m² | 0.0077 | 0.9 | see below |
| every other class, the rigid truck included | none | | | | No sourced row: the grade term only. |

- **120 kg/kW.** The 2001 Green Book design truck for the critical length of grade, read
  through NCHRP Report 505 (2003, doi 10.17226/23379), App. D, pp. 153-159, which adds that
  the data "could also support a choice in the 90- to 108-kg/kW range". Field studies give
  102-108 kg/kW (CA, CO) and 126 kg/kW (PA) (TRR 1881, doi 10.3141/1881-05, summary only).
  That is the sensitivity range.
- **36,287 kg.** 80,000 lb, the federal gross vehicle weight limit on the Interstate
  (23 U.S.C. 127(a)). The ratio, not the mass, sets the crawl speed; the mass sets how
  much the air term matters.
- **No rigid-truck row.** The design ratio is a tractor-semitrailer's. On SUMO's 4.5 t
  rigid truck it gives 37.5 kW, which caps that truck near 70 km/h on the flat; no real
  truck of that mass is so weak. No single-unit truck weight-to-power figure has been
  read (NCHRP 505 has field data; not read). Left for a sourced row.
- **C_dA 5.95 m².** 40 CFR 1037.520(b)(3)(iii), Table 5: the GEM input of a Phase 2
  high-roof sleeper tractor, bin III, the middle of the seven bins (read 2026-10-28 on
  law.cornell.edu).
- **c_rr 0.0077.** The EPA/NHTSA GHG Phase 1 average tractor-trailer, weighted over
  steer, drive and trailer tyres: NRC, *Review of the 21st Century Truck Partnership,
  Second Report* (2012), ch. 18 (read through a search excerpt, 2026-10-28). Phase 2 tyres
  target about 0.006 (trade press), which would raise the crawl speed slightly.
- **η 0.9.** PHEMlight `DRIVE_TRAIN_EFFICIENCY_All`.
- **Left out.** Rotating-mass inertia (PHEMlight `k_rot`) and PHEMlight's `f1·v` and
  `f4·v⁴` rolling terms. At 30 m/s `f1·v` is 17 % of `f0`'s rolling force, and rolling
  is under a quarter of a car's road load there.

## What it gives

- The trailer's steady crawl speed on 5 % (tan θ 0.05) from the balance
  `η·P = m·g·(sin θ + c_rr)·v + ½·ρ·C_dA·v³` is **12.9 m/s (46 km/h)**. Critical length
  is defined as the grade length that costs the design truck 15 km/h (10 mph): from
  85 km/h this model reaches it on 5 % after about 330 m (not compared with the figure).
- **The crawl band is an ASSUMPTION.** The Green Book's speed-distance curve for 120 kg/kW
  is paywalled and has not been read. The test band is the balance above, not the figure.
- The passenger car keeps a 25 m/s limit on 5 %. Its balance speed there is 47 m/s.
- Flat roads, power on: the trailer tops out near 36.6 m/s and the car near 57 m/s. Both
  are above their desired speeds, so on the flat only acceleration near the top speed
  changes.

## Where it is checked

- `v2xw-mobility` unit tests: the table, the chord, the crawl, the downhill hold, the car.
- `calibration/truck-grade-crawl` (`v2xw-trafficval`, fast list of `model_checks`).
- `crates/v2xw-engine/tests/vehicle_dynamics.rs`: grade-only flat identity, the power
  limit on a freeway, and the two `freight`-family rows of `expectations/traffic.yaml`.
- **Not yet reachable from a scenario on a hill.** `world.terrain.dem` is attached
  without draping (`wiring::attach_terrain`), so a shipped scenario's lanes are flat and
  `grade` acts only on worlds whose lanes carry z (SUMO `net.xml` with z, a test world).
  A `world.terrain.drape` key and a DEM in the repo are the next step (P22.4, proposed).

## Sources for the rigid truck, bus, coach and van (WP-25 P25.1, read 2026-10-28)

Plan: `docs/design/frameworks/wp25-plus-build-plans.md` §WP-25. Every figure below was read
on the page named; nothing here is from memory or from a search excerpt unless it says so.

**NCHRP Report 505** (Harwood, Torbic, Richard, Glauz, Elefteriadou 2003, *Review of Truck
Characteristics as Factors in Roadway Design*, doi 10.17226/23379), read as page images at
nap.nationalacademies.org/read/23379 (the TRB PDF link redirects there).

| Figure | Value | Page |
|---|---|---|
| Table 30, average weights and power of trucks (from Gillespie 1985, its ref. 46) | Straight trucks: Interstate-East (n 14) 15,233 lb, 219 hp, 70 lb/hp; Interstate-West (6) 35,050 lb, 267 hp, 131; Primary-East (6) 16,575 lb, 273 hp, 75. Tractor-trailers: 54,452/328/166 (157), 64,775/370/175 (233), 57,487/330/174 (134). 65-ft doubles 64,920/331/196 (19) | p. 55 |
| Eqs. 20-22, rolling resistance F_r/W, V in mph | radial 0.001(4.1 + 0.041 V); mixed 0.001(5.3 + 0.044 V); bias-ply 0.001(6.6 + 0.046 V) | p. 54 |
| Eq. 23, aerodynamic drag (its ref. 43) | F_a = 1.1·D·C_D·A·V²; C_D 0.6 with aerodynamic aids, 0.7 without; A 102 ft² for van bodies, 75 ft² cab only | p. 55 |
| Table 29, average acceleration to 40 mph, ft/s² (tractor-trailers, its ref. 39) | from 0/10/20/30 mph: 100 lb/hp 1.87/1.70/1.47/1.29; 200: 1.22/1.08/0.96/0.79; 300: 0.91/0.81/0.72/0.58; 400: 0.71/0.61/0.50/0.36 | p. 54 |
| App. D, the design ratio | 120 kg/kW [200 lb/hp] (2001 Green Book); the data "could also support" 90-108 kg/kW | p. D-1 (153) |
| App. D, field ratios | 85th-percentile freeway trucks 102-126 kg/kW; two-lane 108-168 kg/kW; no split by truck type | p. D-4 (156) |
| App. E, the TSPM (TWOPAS truck equations, its refs. 85-86) | eqs. 1-4 and a worked example (100 lb/hp, W/A 221 lb/ft², 1,000 ft, a 5.6-6.2 % climb from 65 mph; minimum 41.9 mph) | pp. E-1 to E-6 (160-165) |

- **No single-unit row in App. D.** The crawl-speed survey reports all trucks together, so
  Table 30's straight trucks are the only single-unit weight-to-power figures in the
  report. They are 1984 trucks (Gillespie 1985), and only 26 of them.
- **Reading App. E.** The printed eq. 1 has `−0.004·V′` and eq. 2 has `15368·C_pe`. The
  report's own tabulated example (Fig. E-2, rows at 92.2, 82.2 and 62.1 ft/s) is reproduced
  to 0.01 ft/s² only with `0.0004·V′` and `15868`. Its minimum speed, 41.9 mph on the
  5.7 % segment, is reproduced too. So those are the constants used here, and the printed
  ones are read as typesetting errors. 15,868 = 0.896 × 550 × 32.2: a drivetrain efficiency
  of 0.9 on net horsepower, the same η the rows here use.

**FTA bus tests (Altoona, Penn State LTI Bus Research and Testing Center)**, downloaded
from altoonabustest.psu.edu (the report button is a form postback; bus 1813-P fails on the
server). The performance test runs at seated load weight (SLW) on the test track, three
runs each way at full throttle, and times each 10 mph.

| Report | Bus | Engine, transmission | Weights | Size | Interval times, s (mean of 6 runs) | Pages |
|---|---|---|---|---|---|---|
| 1209 (2012) | Prevost X3-45 Commuter, 45 ft 5 in coach | Volvo D13H435P, Allison B500 | curb 38,590 lb; SLW = GVW 47,110 lb (58 seats, no standees) | 102.0 in wide, 133.0 in high | 10 mph 3.60, 20 6.93, 30 11.03, 40 17.59, 50 26.45 | PDF pp. 6-7, 40-41 |
| 1211-P (2012) | New Flyer XD40, 40 ft 10 in transit bus | Cummins ISL9 280, Allison B400R | curb 27,730 lb; SLW 33,320 lb | 101.75 in wide, 125.25 in high | 10 mph 4.13, 20 8.05, 30 14.27, 40 27.10, 45.1 36.03 ("unable to acquire a top speed of 50 mph") | PDF pp. 3, 5-6, 18-19 |

- **Rated power.** The reports name the engine but print no rating. The designation carries
  it: the Prevost X3-45 flyer (2023) gives the Volvo D13 at 435 hp (324 kW), and Cummins'
  ISL9 urban-transit sheet (EPA 2013) gives the "280" rating as 280 hp (209 kW). Both were
  read as search excerpts [verify the sheets].

**EPA Test Car List** (`20tstcar-2021-03-02.xlsx`, model year 2020, from epa.gov "Data on
Cars Used for Testing Fuel Economy"): Ford **Transit RWD Wagon Med Roof**, test vehicle
LKA00033: 3.5 L, 275 rated hp, 10-speed automatic, equivalent test weight 6,500 lb, target
road-load coefficients A 43.59 lbf, B 0.3185 lbf/mph, C 0.04522 lbf/mph². The model years
2023 and 2025 lists hold no full-size van: cargo vans above 8,500 lb GVWR are certified as
heavy-duty and are not listed. The Transit wagon is the passenger body of the
highest-volume US cargo van, so it is the van row's source.

**PHEMlight heavy-duty files: none open.** SUMO's `data/emissions/PHEMlight` holds only
`PC_G_EU4` and `PC_D_EU4`, and `PHEMlight5` only `PC_EU4_D_MW` and `PC_EU4_G` (GitHub
listing, 2026-10-28). The HDV, bus, coach and LCV vehicles need TU Graz's licence. Dropped.

**What the rows will be (built in P25.2).**

| Class | Mass | Power | C_dA | c_rr | η | From |
|---|---|---|---|---|---|---|
| truck (rigid) | 9,124 kg (20,116 lb) | 180.9 kW (242.5 hp): 82.9 lb/hp, 50.4 kg/kW | 0.7 × 102 ft² = 6.63 m² | 0.0041 + 0.041·V(mph)/1000 | 0.9 | Table 30's 26 straight trucks pooled (sum of weights over sum of power); eqs. 20, 23 |
| bus | 15,114 kg (SLW 33,320 lb) | 208.8 kW (280 hp) | 0.7 × 101.75 × 125.25 in² = 5.76 m² | as the truck | 0.54, fitted | Altoona 1211-P; NCHRP 505 eqs. 20, 23 |
| coach | 21,369 kg (SLW 47,110 lb) | 324.4 kW (435 hp) | 0.7 × 102.0 × 133.0 in² = 6.13 m² | as the truck | 0.84, fitted | Altoona 1209; NCHRP 505 eqs. 20, 23 |
| delivery (van) | 2,948 kg (ETW 6,500 lb) | 205.1 kW (275 hp) | 2C/ρ = 1.70 m² | A/W = 0.00671 + (B/W)·v | 0.9 | EPA Test Car List MY2020 |

- **η for the buses is measured, not assumed.** At η 0.9 the coach reproduces its last
  interval (40-50 mph) within 8 %, but the transit bus runs 30-40 mph twice as fast as
  the XD40 did. A transit bus spends power on its cooling fan, alternator and compressor,
  and its transmission manages acceleration for standing passengers. So η is the fraction
  of the rated power that reaches the wheels in the **last interval of each Altoona run**,
  where the bus is in its top gears. The fit is coach 0.840 (40-50 mph, 8.86 s) and bus
  0.544 (40-45.1 mph, 8.93 s). The check is a figure the fit did not use: the whole run
  from rest. That is 24.1 s against 26.45 s to 50 mph for the coach (−9 %), and 36.8 s
  against 36.03 s to 45.1 mph for the bus (+2 %), at the class's own comfortable
  acceleration (SUMO bus 1.2, coach 2.0 m/s²).
- **A row's mass is its source's** (plan rule 2): the SUMO class masses (truck 4.5 t,
  bus 12 t, coach 25 t, delivery 5 t) are not changed.
- **The van's B and C** come from a coast-down, so they carry tyre and driveline losses.
  The C_dA of 1.70 m² is an upper estimate of the air term alone.

**The crawl band (decision 3 of the plan).** The TWOPAS equations of App. E give the Green
Book design truck (120 kg/kW = 197.3 lb/hp; W/A 685 lb/ft², the frontal area that gives
C_dA 5.95 m² at TWOPAS's C_D-equivalent 0.549) a crawl speed of **12.04 m/s** on 5 %. The
balance here gives 12.90 m/s, **7 % above** it. The difference is TWOPAS's low-speed power
loss term (`1 + 14080/((W/NHP)·V′²)`, 5 % at the crawl) and its engine-drag term
(`222.6/((W/NHP)·V′)`). So the band of `calibration/truck-grade-crawl` stays the balance,
with TWOPAS reported beside it. The rigid truck's crawl on 5 % is 22.95 m/s here and
22.58 m/s by TWOPAS (W/A 197, +1.6 %).

## Proposed default (owner decision)

`grade: true` on every DEM-draped world: it is SUMO's own KraussPS term and is
byte-identical on flat maps. `power_limit: true` with it once G-TV is re-blessed.
