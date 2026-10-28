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

## Proposed default (owner decision)

`grade: true` on every DEM-draped world: it is SUMO's own KraussPS term and is
byte-identical on flat maps. `power_limit: true` with it once G-TV is re-blessed.
